//! The chat proxy: receive an OpenAI Chat Completions request, resolve the
//! model → provider + credential, call the upstream provider, and stream the
//! SSE response back to the client.
//!
//! Direct port of the Go native path (backend/internal/httpapi/chat.go +
//! chat_transport.go), scoped to the OpenAI→OpenAI same-format passthrough the
//! Go backend already serves. No format translation in v1.

use std::time::Duration;

use axum::{
    body::Body,
    extract::State,
    http::{HeaderMap, HeaderName, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
};
use bytes::Bytes;
use futures_util::StreamExt;
use serde_json::Value;
use tracing::{debug, warn};

pub mod reverse;

use crate::{db::Db, snapshot};

#[derive(Clone)]
pub struct AppState {
    pub db: Db,
    pub client: reqwest::Client,
    /// Node/Next.js upstream URL (e.g. "http://127.0.0.1:21129") for the
    /// reverse-proxy catch-all. Empty = no fallback (standalone Rust mode).
    pub node_upstream: String,
}

/// Errors surfaced to the client as OpenAI-style JSON.
pub struct ChatError {
    pub status: StatusCode,
    pub message: String,
}

impl IntoResponse for ChatError {
    fn into_response(self) -> Response {
        let body = serde_json::json!({
            "error": { "message": self.message, "type": "orouter_proxy_error" }
        });
        (self.status, axum::Json(body)).into_response()
    }
}

/// Handle POST /v1/chat/completions.
pub async fn chat_completions(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, ChatError> {
    let mut payload: Value = serde_json::from_slice(&body).map_err(|_| ChatError {
        status: StatusCode::BAD_REQUEST,
        message: "Invalid JSON body".into(),
    })?;

    let model = payload
        .get("model")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    if model.is_empty() {
        return Err(ChatError {
            status: StatusCode::BAD_REQUEST,
            message: "Missing model".into(),
        });
    }

    // Optional inbound API-key gate.
    if state.db.require_api_key().await {
        let presented = extract_api_key(&headers);
        match presented {
            Some(k) if state.db.validate_api_key(&k).await => {}
            _ => {
                return Err(ChatError {
                    status: StatusCode::UNAUTHORIZED,
                    message: "Missing or invalid API key".into(),
                });
            }
        }
    }

    // Try to resolve natively. If the model isn't a native OpenAI-format
    // provider (Claude, Gemini, OAuth, combo, etc.), fall through to Node
    // which can handle it via the open-sse translator.
    let resolved = match snapshot::resolve(&model) {
        Some(r) => r,
        None => {
            tracing::info!(model = %model, "non-native model, proxying to Node");
            return proxy_request_to_node(state, headers, body).await;
        }
    };

    // Pick ONE credential up front. If none found natively (e.g. OAuth-only
    // connections), fall through to Node which has the full credential picker.
    let cred = match state
        .db
        .pick_credential(&resolved.provider_id, &model)
        .await
    {
        Some(c) => c,
        None => {
            tracing::info!(provider = %resolved.provider_id, "no native credential, proxying to Node");
            return proxy_request_to_node(state, headers, body).await;
        }
    };

    // Overwrite model with the upstream id (Go: out["model"] = upstream).
    if let Some(obj) = payload.as_object_mut() {
        obj.insert(
            "model".into(),
            Value::String(resolved.upstream_model.clone()),
        );
    }
    let payload_bytes = serde_json::to_vec(&payload).map_err(|_| ChatError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: "encode upstream request".into(),
    })?;

    let is_stream = payload
        .get("stream")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let inbound_api_key = extract_api_key(&headers);

    let upstream = execute_upstream(
        &state,
        &resolved,
        cred.secret.clone(),
        payload_bytes,
        is_stream,
    )
    .await?;

    // Context for usage logging (M7). The SSE relay logs after the stream ends;
    // the JSON path logs from the handler.
    let usage_ctx = UsageCtx {
        provider: resolved.provider_id.clone(),
        model: model.clone(),
        connection_id: cred.connection_id.clone(),
        api_key: inbound_api_key.unwrap_or_default(),
        endpoint: "/v1/chat/completions".to_string(),
    };

    if is_stream {
        Ok(relay_sse(upstream, &payload, state.clone(), usage_ctx))
    } else {
        Ok(relay_json_async(upstream, state.clone(), usage_ctx).await)
    }
}

/// Forward a chat request to the Node upstream (for non-native models like
/// Claude/Gemini/OAuth/combo). Node's open-sse translator handles format
/// conversion, OAuth refresh, combo routing, etc.
async fn proxy_request_to_node(
    state: AppState,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, ChatError> {
    let node_upstream = &state.node_upstream;
    if node_upstream.is_empty() {
        return Err(ChatError {
            status: StatusCode::NOT_FOUND,
            message: "Model not available for native proxying and no Node upstream configured."
                .into(),
        });
    }

    let target_url = format!("{node_upstream}/v1/chat/completions");
    let mut hdrs = HeaderMap::new();
    for (name, value) in &headers {
        let name_str = name.as_str();
        if matches!(
            name_str,
            "host" | "content-length" | "connection" | "transfer-encoding"
        ) {
            continue;
        }
        hdrs.insert(name.clone(), value.clone());
    }
    hdrs.insert("content-type", HeaderValue::from_static("application/json"));

    let upstream_req = state.client.post(&target_url).headers(hdrs).body(body);
    let upstream_resp = upstream_req.send().await.map_err(|e| ChatError {
        status: StatusCode::BAD_GATEWAY,
        message: format!("Node upstream error: {e}"),
    })?;

    let status = upstream_resp.status();
    let mut resp_headers = HeaderMap::new();
    for (name, value) in upstream_resp.headers() {
        if matches!(
            name.as_str(),
            "connection" | "transfer-encoding" | "content-length"
        ) {
            continue;
        }
        resp_headers.insert(name.clone(), value.clone());
    }

    let stream = upstream_resp
        .bytes_stream()
        .map(|chunk| chunk.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e)));
    let mut resp = Response::new(Body::from_stream(stream));
    *resp.status_mut() = status;
    *resp.headers_mut() = resp_headers;
    Ok(resp)
}

/// Context threaded into the relays so they can persist usage after the request.
struct UsageCtx {
    provider: String,
    model: String,
    connection_id: String,
    api_key: String,
    endpoint: String,
}

/// Extract inbound API key from Authorization: Bearer or x-api-key
/// (mirrors src/sse/services/auth.js extractApiKey).
fn extract_api_key(headers: &HeaderMap) -> Option<String> {
    if let Some(auth) = headers.get("authorization").and_then(|v| v.to_str().ok()) {
        if let Some(token) = auth.strip_prefix("Bearer ") {
            return Some(token.trim().to_string());
        }
        return Some(auth.trim().to_string());
    }
    headers
        .get("x-api-key")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.trim().to_string())
}

/// Build + send the upstream request, applying the Go retry policy
/// (502×3, 503×3, 504×2, network×3 — chat_transport.go:63-113).
async fn execute_upstream(
    state: &AppState,
    resolved: &snapshot::ResolvedModel,
    secret: String,
    payload: Vec<u8>,
    is_stream: bool,
) -> Result<reqwest::Response, ChatError> {
    let transport = &resolved.transport;
    let mut attempts_502 = 3u32;
    let mut attempts_503 = 3u32;
    let mut attempts_504 = 2u32;
    let mut net_retries = 3u32;

    let mut token = secret;
    let header_name = if transport.auth_header.is_empty() {
        "authorization"
    } else {
        transport.auth_header.as_str()
    };
    if transport.auth_scheme.eq_ignore_ascii_case("bearer") {
        token = format!("Bearer {token}");
    }
    let static_headers = transport.headers.clone();

    loop {
        // Build the full header set up front (reqwest consumes it via .headers()).
        let mut hdrs = HeaderMap::new();
        hdrs.insert("content-type", HeaderValue::from_static("application/json"));
        if is_stream {
            hdrs.insert("accept", HeaderValue::from_static("text/event-stream"));
        }
        if let Ok(val) = HeaderValue::from_str(&token) {
            if let Ok(name) = HeaderName::try_from(header_name) {
                hdrs.insert(name, val);
            }
        }
        for (k, v) in &static_headers {
            if let (Ok(name), Ok(val)) =
                (HeaderName::try_from(k.as_str()), HeaderValue::from_str(v))
            {
                hdrs.insert(name, val);
            }
        }
        let req = state
            .client
            .post(&transport.base_url)
            .headers(hdrs)
            .body(payload.clone());

        match req.send().await {
            Err(e) => {
                if net_retries > 0 {
                    net_retries -= 1;
                    debug!("upstream network error, retrying: {e}");
                    tokio::time::sleep(Duration::from_secs(3)).await;
                    continue;
                }
                return Err(ChatError {
                    status: StatusCode::BAD_GATEWAY,
                    message: format!("upstream fetch: {e}"),
                });
            }
            Ok(r) => {
                let s = r.status().as_u16();
                if s == 502 && attempts_502 > 0 {
                    attempts_502 -= 1;
                    tokio::time::sleep(Duration::from_secs(3)).await;
                    continue;
                }
                if s == 503 && attempts_503 > 0 {
                    attempts_503 -= 1;
                    tokio::time::sleep(Duration::from_secs(2)).await;
                    continue;
                }
                if s == 504 && attempts_504 > 0 {
                    attempts_504 -= 1;
                    tokio::time::sleep(Duration::from_secs(3)).await;
                    continue;
                }
                return Ok(r);
            }
        }
    }
}

/// Relay a streaming upstream response to the client as text/event-stream.
///
/// Ported from Go's relayOpenAISSE (chat_transport.go:232): for each `data:`
/// line, parse → normalize → re-emit as `data: <json>\n\n`; accumulate usage;
/// inject the final usage chunk (+2000 buffer) on finish; emit `data: [DONE]\n\n`.
/// After the stream ends, persists the accumulated usage (M7).
fn relay_sse(
    upstream: reqwest::Response,
    request_body: &Value,
    state: AppState,
    usage_ctx: UsageCtx,
) -> Response {
    let request_body = request_body.clone();
    let stream = async_stream::stream! {
        let mut byte_stream = upstream.bytes_stream();
        let mut buf: Vec<u8> = Vec::with_capacity(8192);
        let mut usage = StreamUsage::default();
        let mut content_chars = 0usize;
        let mut done_seen = false;

        while let Some(chunk_res) = byte_stream.next().await {
            let chunk = match chunk_res { Ok(c) => c, Err(_) => break };
            buf.extend_from_slice(&chunk);
            // Process complete lines, keep the trailing partial line in buf.
            while let Some(nl) = memchr::memchr(b'\n', &buf) {
                let line: Vec<u8> = buf.drain(..=nl).collect();
                let line_str = String::from_utf8_lossy(&line);
                let trimmed = line_str.trim();
                if !trimmed.starts_with("data:") { continue; }
                let payload = trimmed.trim_start_matches("data:").trim().to_string();

                if payload == "[DONE]" {
                    done_seen = true;
                    yield Ok::<Bytes, std::io::Error>(Bytes::from_static(b"data: [DONE]\n\n"));
                    continue;
                }
                if payload.is_empty() { continue; }

                let Ok(mut chunk_val) = serde_json::from_str::<Value>(&payload) else { continue };
                normalize_chunk(&mut chunk_val, &mut content_chars, &mut usage);
                if !is_valuable(&chunk_val) { continue; }
                if has_finish(&chunk_val) {
                    inject_final_usage(&mut chunk_val, &usage, content_chars, &request_body);
                }
                let json = serde_json::to_vec(&chunk_val).unwrap_or_default();
                let mut out = b"data: ".to_vec();
                out.extend_from_slice(&json);
                out.extend_from_slice(b"\n\n");
                yield Ok(Bytes::from(out));
            }
        }
        if !done_seen {
            yield Ok::<Bytes, std::io::Error>(Bytes::from_static(b"data: [DONE]\n\n"));
        }
        // Persist usage after the stream completes (M7). If the provider did
        // not report usage, estimate the same way the client-visible chunk did.
        if !usage.valid && content_chars > 0 {
            usage.prompt = estimate_json_tokens(&request_body);
            usage.completion = ((content_chars + 3) / 4) as i64;
            usage.valid = true;
        }
        // `usage.prompt` is raw; +2000 exists only in the emitted chunk.
        let prompt = usage.prompt.max(0);
        let completion = usage.completion.max(0);
        if prompt > 0 || completion > 0 {
            let entry = crate::db::ChatUsageEntry {
                timestamp: String::new(),
                provider: usage_ctx.provider,
                model: usage_ctx.model,
                connection_id: usage_ctx.connection_id,
                api_key: usage_ctx.api_key,
                endpoint: usage_ctx.endpoint,
                status: "ok".to_string(),
                cost: 0.0,
                tokens: crate::db::ChatUsage {
                    prompt,
                    completion,
                    total: prompt + completion,
                    cached: usage.cached,
                    reasoning: usage.reasoning,
                    cache_creation: usage.cache_creation,
                },
            };
            if let Err(e) = state.db.save_chat_usage(entry).await {
                tracing::warn!("usage save failed: {e}");
            }
        }
    };

    let mut resp = Response::new(Body::from_stream(stream));
    *resp.status_mut() = StatusCode::OK;
    let h = resp.headers_mut();
    h.insert(
        "content-type",
        HeaderValue::from_static("text/event-stream"),
    );
    h.insert("cache-control", HeaderValue::from_static("no-cache"));
    h.insert("connection", HeaderValue::from_static("keep-alive"));
    h.insert("access-control-allow-origin", HeaderValue::from_static("*"));
    resp
}

/// Non-streaming JSON path: read the whole upstream body, apply the same-format
/// normalization (Go normalizeNonStreaming), persist usage (M7), and return JSON.
async fn relay_json_async(
    upstream: reqwest::Response,
    state: AppState,
    usage_ctx: UsageCtx,
) -> Response {
    let status = upstream.status();
    let bytes = match upstream.bytes().await {
        Ok(b) => b,
        Err(e) => {
            warn!("upstream body read failed: {e}");
            return ChatError {
                status: StatusCode::BAD_GATEWAY,
                message: "upstream body read failed".into(),
            }
            .into_response();
        }
    };
    let mut val: Value = match serde_json::from_slice(&bytes) {
        Ok(v) => v,
        Err(_) => {
            return Response::builder()
                .status(status)
                .body(Body::from(bytes))
                .unwrap()
        }
    };
    if let Some(obj) = val.as_object_mut() {
        obj.entry("object")
            .or_insert(Value::String("chat.completion".into()));
        obj.entry("created")
            .or_insert(Value::Number(serde_json::Number::from(now_unix_secs())));
        obj.remove("prompt_filter_results");
        if let Some(Value::Array(choices)) = obj.get_mut("choices") {
            for c in choices.iter_mut() {
                if let Some(co) = c.as_object_mut() {
                    co.remove("content_filter_results");
                }
            }
        }
    }

    // Persist usage (M7). Client-visible usage gets +2000 added below; store the
    // raw provider counts (Go subtracts the buffer before persisting).
    let prompt_raw = val
        .get("usage")
        .and_then(|u| u.get("prompt_tokens"))
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let completion_raw = val
        .get("usage")
        .and_then(|u| u.get("completion_tokens"))
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    if prompt_raw > 0 || completion_raw > 0 {
        let entry = crate::db::ChatUsageEntry {
            timestamp: String::new(),
            provider: usage_ctx.provider,
            model: usage_ctx.model,
            connection_id: usage_ctx.connection_id,
            api_key: usage_ctx.api_key,
            endpoint: usage_ctx.endpoint,
            status: "ok".to_string(),
            cost: 0.0,
            tokens: crate::db::ChatUsage {
                prompt: prompt_raw.max(0),
                completion: completion_raw,
                total: prompt_raw + completion_raw,
                cached: 0,
                reasoning: 0,
                cache_creation: 0,
            },
        };
        // add the +2000 buffer to the client-visible usage (Go parity)
        if let Some(u) = val.get_mut("usage").and_then(|v| v.as_object_mut()) {
            if let Some(p) = u.get("prompt_tokens").and_then(|v| v.as_i64()) {
                u.insert("prompt_tokens".into(), Value::Number((p + 2000).into()));
            }
            if let Some(t) = u.get("total_tokens").and_then(|v| v.as_i64()) {
                u.insert("total_tokens".into(), Value::Number((t + 2000).into()));
            }
        }
        if let Err(e) = state.db.save_chat_usage(entry).await {
            tracing::warn!("usage save failed: {e}");
        }
    }

    Response::builder()
        .status(status)
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::to_vec(&val).unwrap_or_else(|_| Vec::new()),
        ))
        .unwrap()
}

#[derive(Default)]
struct StreamUsage {
    prompt: i64,
    completion: i64,
    cached: i64,
    reasoning: i64,
    cache_creation: i64,
    valid: bool,
}

/// Same-format chunk normalization (Go relayOpenAISSE): id fallback,
/// object/created defaults, drop prompt_filter_results/content_filter_results,
/// count content chars, accumulate max usage.
fn normalize_chunk(chunk: &mut Value, content_chars: &mut usize, usage: &mut StreamUsage) {
    let Some(obj) = chunk.as_object_mut() else {
        return;
    };

    let id_needs_fix = match obj.get("id").and_then(|v| v.as_str()) {
        Some(s) if s == "chat" || s == "completion" => true,
        Some(s) if !s.is_empty() && s.len() < 8 => true,
        _ => false,
    };
    if id_needs_fix {
        obj.insert(
            "id".into(),
            Value::String(format!("chatcmpl-{}", now_unix_nanos_b36())),
        );
    }

    let has_choices = matches!(obj.get("choices"), Some(Value::Array(_)));
    if has_choices {
        obj.entry("object")
            .or_insert(Value::String("chat.completion.chunk".into()));
        obj.entry("created")
            .or_insert(Value::Number(serde_json::Number::from(now_unix_secs())));
    }
    obj.remove("prompt_filter_results");

    if let Some(Value::Array(choices)) = obj.get_mut("choices") {
        for c in choices.iter_mut() {
            let Some(co) = c.as_object_mut() else {
                continue;
            };
            co.remove("content_filter_results");
            if let Some(delta) = co.get_mut("delta").and_then(|v| v.as_object_mut()) {
                if let Some(Value::String(s)) = delta.get("content") {
                    *content_chars += s.len();
                }
                if let Some(Value::String(s)) = delta.get("reasoning_content") {
                    *content_chars += s.len();
                }
            }
        }
    }

    if let Some(u) = obj.get("usage").and_then(|v| v.as_object()) {
        let max = |k: &str, cur: &mut i64| {
            if let Some(n) = u.get(k).and_then(|v| v.as_i64()) {
                if n > *cur {
                    *cur = n;
                }
            }
        };
        max("prompt_tokens", &mut usage.prompt);
        max("completion_tokens", &mut usage.completion);
        max("cached_tokens", &mut usage.cached);
        max("reasoning_tokens", &mut usage.reasoning);
        max("cache_creation_input_tokens", &mut usage.cache_creation);
        usage.valid = usage.prompt > 0 || usage.completion > 0;
    }
}

/// "Valuable" chunks carry content/reasoning/role/finish/tool_calls (Go parity).
fn is_valuable(chunk: &Value) -> bool {
    let Some(choices) = chunk.get("choices").and_then(|v| v.as_array()) else {
        return true; // non-choice chunks (usage-only) pass through
    };
    for c in choices {
        if let Some(delta) = c.get("delta") {
            if delta.get("content").and_then(|v| v.as_str()).is_some() {
                return true;
            }
            if delta
                .get("reasoning_content")
                .and_then(|v| v.as_str())
                .is_some()
            {
                return true;
            }
            if delta.get("role").and_then(|v| v.as_str()).is_some() {
                return true;
            }
            if delta
                .get("tool_calls")
                .and_then(|v| v.as_array())
                .is_some_and(|a| !a.is_empty())
            {
                return true;
            }
        }
        if c.get("finish_reason").is_some() {
            return true;
        }
    }
    false
}

fn has_finish(chunk: &Value) -> bool {
    chunk
        .get("choices")
        .and_then(|v| v.as_array())
        .is_some_and(|cs| cs.iter().any(|c| c.get("finish_reason").is_some()))
}

/// Inject final usage on the finish chunk (+2000 buffer, Go parity). Estimate
/// when upstream reported no usage.
fn inject_final_usage(
    chunk: &mut Value,
    usage: &StreamUsage,
    content_chars: usize,
    request_body: &Value,
) {
    let (prompt, completion, estimated) = if usage.valid {
        (usage.prompt, usage.completion, false)
    } else {
        let prompt = estimate_json_tokens(request_body);
        let mut completion = (content_chars / 4) as i64;
        if content_chars > 0 && completion == 0 {
            completion = 1;
        }
        (prompt, completion, true)
    };
    let mut u = serde_json::Map::new();
    u.insert(
        "prompt_tokens".into(),
        Value::Number((prompt + 2000).into()),
    );
    u.insert("completion_tokens".into(), Value::Number(completion.into()));
    u.insert(
        "total_tokens".into(),
        Value::Number((prompt + completion + 2000).into()),
    );
    if usage.cached > 0 {
        u.insert("cached_tokens".into(), Value::Number(usage.cached.into()));
    }
    if usage.reasoning > 0 {
        u.insert(
            "reasoning_tokens".into(),
            Value::Number(usage.reasoning.into()),
        );
    }
    if usage.cache_creation > 0 {
        u.insert(
            "cache_creation_input_tokens".into(),
            Value::Number(usage.cache_creation.into()),
        );
    }
    if estimated {
        u.insert("estimated".into(), Value::Bool(true));
    }
    if let Some(obj) = chunk.as_object_mut() {
        obj.insert("usage".into(), Value::Object(u));
    }
}

fn now_unix_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
fn now_unix_nanos_b36() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let mut n = nanos;
    const D: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    let mut out = Vec::new();
    while n > 0 {
        out.push(D[(n % 36) as usize]);
        n /= 36;
    }
    out.reverse();
    String::from_utf8(out).unwrap_or_else(|_| "0".into())
}
fn estimate_json_tokens(body: &Value) -> i64 {
    let raw = serde_json::to_vec(body).unwrap_or_default();
    if raw.is_empty() {
        0
    } else {
        ((raw.len() as i64) + 3) / 4
    }
}
