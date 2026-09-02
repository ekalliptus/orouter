//! Native Anthropic-format inference: POST /v1/messages (+ count_tokens).
//!
//! Claude-format clients (Claude Code & friends) talk to the router exactly
//! like they would to Anthropic: the body is already in Anthropic format, so
//! the handler is a format-preserving relay to the upstream
//! `api.anthropic.com/v1/messages` — with OAuth-token auto-refresh, correct
//! auth headers per connection type (OAuth Bearer vs x-api-key), and
//! fallback across every active claude connection in priority order.

use std::time::Duration;

use axum::{
    body::Body,
    extract::State,
    http::{HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
};
use serde_json::json;
use uuid::Uuid;
use bytes::Bytes;
use serde_json::Value;

use crate::proxy::{extract_api_key_pub, AppState, ChatError};

const UPSTREAM_MESSAGES: &str = "https://api.anthropic.com/v1/messages";
const UPSTREAM_COUNT_TOKENS: &str = "https://api.anthropic.com/v1/messages/count_tokens";
const OAUTH_BETA: &str = "oauth-2025-04-20";
const API_VERSION: &str = "2023-06-01";

fn error(status: StatusCode, msg: String) -> Response {
    let body = serde_json::json!({
        "type": "error",
        "error": { "type": "api_error", "message": msg },
    });
    (status, axum::Json(body)).into_response()
}

/// POST /v1/messages — Anthropic-format relay with smart provider routing.
pub async fn messages(
    state: State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, ChatError> {
    // Gate first (same policy as before).
    if state.db.require_api_key().await {
        let presented = crate::proxy::extract_api_key_pub(&headers);
        let valid = match presented {
            Some(k) => state.db.validate_api_key(&k).await,
            None => false,
        };
        if !valid {
            return Err(ChatError {
                status: StatusCode::UNAUTHORIZED,
                message: "Missing or invalid API key".into(),
            });
        }
    }

    let payload: Value = serde_json::from_slice(&body).map_err(|_| ChatError {
        status: StatusCode::BAD_REQUEST,
        message: "Invalid JSON body".into(),
    })?;
    let requested_model = payload
        .get("model")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    if requested_model.is_empty() {
        return Err(ChatError {
            status: StatusCode::BAD_REQUEST,
            message: "Missing model".into(),
        });
    }

    // Provider routing: resolve the provider token from the model prefix.
    // Anthropic-upstream providers (claude family) use the passthrough relay;
    // everything else is translated to OpenAI format and routed through the
    // native chat pipeline (chat_completions), then translated back.
    let provider_token = requested_model
        .split_once('/')
        .map(|(p, _)| p.to_string())
        .unwrap_or_default();
    let claude_family = matches!(
        provider_token.as_str(),
        "claude" | "anthropic" | "cc" | "claude-code"
    ) || provider_token.starts_with("anthropic-compatible");

    if claude_family || !requested_model.contains('/') {
        return relay_anthropic(state, headers, body, false).await;
    }
    // OpenAI-upstream bridge: translate Anthropic → OpenAI, call the native
    // chat pipeline, translate the response back to Anthropic format.
    let is_stream = payload
        .get("stream")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let oai_body = an_to_oai_body(&payload);
    let oai_bytes = serde_json::to_vec(&oai_body).unwrap_or_default();

    let upstream = crate::proxy::chat_completions(state, headers, axum::body::Bytes::from(oai_bytes)).await;

    if is_stream {
        let resp = upstream?;
        let (parts, stream_body) = resp.into_parts();
        let converted = convert_oai_sse_to_anthropic(stream_body);
        let mut out_resp = Response::new(converted);
        *out_resp.status_mut() = parts.status;
        out_resp.headers_mut().insert("content-type", "text/event-stream; charset=utf-8".parse().unwrap());
        out_resp.headers_mut().insert("cache-control", "no-cache".parse().unwrap());
        Ok(out_resp)
    } else {
        let resp = upstream?;
        let (parts, body) = resp.into_parts();
        let bytes = axum::body::to_bytes(body, 32 * 1024 * 1024).await.unwrap_or_default();
        let status = parts.status;
        if !status.is_success() {
            let msg = serde_json::from_slice::<Value>(&bytes)
                .ok()
                .and_then(|v| v.pointer("/error/message").and_then(|m| m.as_str()).map(String::from))
                .unwrap_or_else(|| String::from_utf8_lossy(&bytes).to_string());
            return Ok(error(status, msg));
        }
        let oai: Value = serde_json::from_slice(&bytes).unwrap_or(json!({}));
        let an = oai_to_an_message(&oai, &requested_model);
        Ok((StatusCode::OK, axum::Json(an)).into_response())
    }
}

/// Translate an Anthropic /v1/messages body into an OpenAI chat-completions
/// body (text + image blocks; system hoisted; tool support basic).
pub fn an_to_oai_body(an: &Value) -> Value {
    let mut messages: Vec<Value> = Vec::new();

    if let Some(system) = an.get("system") {
        let text = match system {
            Value::String(s) => s.clone(),
            Value::Array(blocks) => blocks
                .iter()
                .filter_map(|b| b.get("text").and_then(|t| t.as_str()))
                .collect::<Vec<_>>()
                .join("\n"),
            _ => String::new(),
        };
        if !text.is_empty() {
            messages.push(json!({ "role": "system", "content": text }));
        }
    }

    if let Some(msgs) = an.get("messages").and_then(|m| m.as_array()) {
        for m in msgs {
            let role = m.get("role").and_then(|r| r.as_str()).unwrap_or("user");
            let oai_role = if role == "assistant" { "assistant" } else { "user" };
            match m.get("content") {
                Some(Value::String(s)) => {
                    messages.push(json!({ "role": oai_role, "content": s }));
                }
                Some(Value::Array(blocks)) => {
                    let mut text_parts = Vec::new();
                    let mut images = Vec::new();
                    for b in blocks {
                        match b.get("type").and_then(|t| t.as_str()) {
                            Some("text") => {
                                if let Some(t) = b.get("text").and_then(|t| t.as_str()) {
                                    text_parts.push(t.to_string());
                                }
                            }
                            Some("image") => {
                                if let Some(source) = b.get("source") {
                                    let media = source.get("media_type").and_then(|x| x.as_str()).unwrap_or("image/png");
                                    if let Some(data) = source.get("data").and_then(|x| x.as_str()) {
                                        images.push(json!({
                                            "type": "image_url",
                                            "image_url": { "url": format!("data:{media};base64,{data}") },
                                        }));
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                    if images.is_empty() {
                        messages.push(json!({ "role": oai_role, "content": text_parts.join("\n") }));
                    } else {
                        let mut parts: Vec<Value> = Vec::new();
                        if !text_parts.is_empty() {
                            parts.push(json!({ "type": "text", "text": text_parts.join("\n") }));
                        }
                        parts.extend(images);
                        messages.push(json!({ "role": oai_role, "content": parts }));
                    }
                }
                _ => {}
            }
        }
    }

    let mut oai = json!({ "model": an.get("model").cloned().unwrap_or(json!("")), "messages": messages });
    if let Some(mt) = an.get("max_tokens").and_then(|v| v.as_u64()) {
        oai["max_tokens"] = json!(mt);
    }
    if let Some(t) = an.get("temperature") { oai["temperature"] = t.clone(); }
    if let Some(t) = an.get("top_p") { oai["top_p"] = t.clone(); }
    if an.get("stream").and_then(|v| v.as_bool()).unwrap_or(false) {
        oai["stream"] = json!(true);
    }
    oai
}

/// Translate a non-streaming OpenAI response into an Anthropic message.
pub fn oai_to_an_message(oai: &Value, requested_model: &str) -> Value {
    let choice = oai.pointer("/choices/0").cloned().unwrap_or(json!({}));
    let message = choice.get("message").cloned().unwrap_or(json!({}));
    let content_str = message
        .get("content")
        .and_then(|c| c.as_str())
        .unwrap_or("")
        .to_string();
    let mut content_blocks = vec![json!({ "type": "text", "text": content_str })];
    if let Some(tcs) = message.get("tool_calls").and_then(|t| t.as_array()) {
        for tc in tcs {
            if let Some(fn_obj) = tc.get("function") {
                let name = fn_obj.get("name").and_then(|n| n.as_str()).unwrap_or("");
                let args_raw = fn_obj.get("arguments").and_then(|a| a.as_str()).unwrap_or("{}");
                let input: Value = serde_json::from_str(args_raw).unwrap_or(json!({}));
                content_blocks.push(json!({
                    "type": "tool_use",
                    "id": tc.get("id").cloned().unwrap_or(json!("toolu_unknown")),
                    "name": name,
                    "input": input,
                }));
            }
        }
    }
    let stop_reason = match choice.get("finish_reason").and_then(|f| f.as_str()) {
        Some("length") => "max_tokens",
        Some("tool_calls") => "tool_use",
        _ => "end_turn",
    };
    let input_tokens = oai
        .pointer("/usage/prompt_tokens")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let output_tokens = oai
        .pointer("/usage/completion_tokens")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    json!({
        "id": oai.get("id").cloned().unwrap_or(json!("msg_orouter")),
        "type": "message",
        "role": "assistant",
        "model": requested_model,
        "content": content_blocks,
        "stop_reason": stop_reason,
        "stop_sequence": Value::Null,
        "usage": { "input_tokens": input_tokens, "output_tokens": output_tokens },
    })
}

/// Streaming converter: OpenAI chat.completion.chunk SSE in → Anthropic
/// message events out (message_start / content_block_* / message_delta /
/// message_stop).
pub struct AnSseConverter {
    started: bool,
    block_open: bool,
    next_index: u64,
    model: String,
    output_tokens: i64,
    input_tokens: i64,
    finish_seen: bool,
    stopped: bool,
    pending_tool: Option<(u64, String, String)>, // index, id, name
}
impl Default for AnSseConverter {
    fn default() -> Self { Self::new() }
}
impl AnSseConverter {
    pub fn new() -> Self {
        Self {
            started: false,
            block_open: false,
            next_index: 0,
            model: String::new(),
            output_tokens: 0,
            input_tokens: 0,
            finish_seen: false,
            stopped: false,
            pending_tool: None,
        }
    }

    fn message_start(&self) -> String {
        sse_event("message_start", &json!({
            "type": "message_start",
            "message": {
                "id": format!("msg_{}", Uuid::new_v4()),
                "type": "message",
                "role": "assistant",
                "model": self.model,
                "content": [],
                "stop_reason": Value::Null,
                "usage": { "input_tokens": self.input_tokens, "output_tokens": 0 },
            },
        }))
    }

    fn block_start(&mut self, block: Value) -> Vec<String> {
        let idx = self.next_index;
        self.next_index += 1;
        self.block_open = true;
        vec![sse_event("content_block_start", &json!({
            "type": "content_block_start", "index": idx, "content_block": block,
        }))]
    }

    fn block_delta(&mut self, delta: Value) -> String {
        sse_event("content_block_delta", &json!({
            "type": "content_block_delta", "index": self.next_index.saturating_sub(1), "delta": delta,
        }))
    }

    fn block_stop(&mut self) -> Vec<String> {
        if !self.block_open { return vec![]; }
        self.block_open = false;
        vec![sse_event("content_block_stop", &json!({
            "type": "content_block_stop", "index": self.next_index.saturating_sub(1),
        }))]
    }

    /// Feed one raw OpenAI SSE line (may contain "data: {...}").
    /// Returns the Anthropic SSE events to emit (possibly empty).
    pub fn feed_line(&mut self, line: &str) -> Vec<String> {
        let line = line.trim();
        let Some(data) = line.strip_prefix("data: ") else { return vec![] };
        let data = data.trim();
        if data == "[DONE]" {
            let mut out = self.finish();
            self.stopped = true;
            out.push("data: [DONE]\n\n".to_string());
            return out;
        }
        let Ok(v) = serde_json::from_str::<Value>(data) else { return vec![] };

        if self.model.is_empty() {
            self.model = v.get("model").and_then(|m| m.as_str()).unwrap_or("").to_string();
        }
        if let Some(u) = v.get("usage") {
            if let Some(n) = u.get("prompt_tokens").and_then(|x| x.as_i64()) {
                self.input_tokens = n;
            }
            if let Some(n) = u.get("completion_tokens").and_then(|x| x.as_i64()) {
                self.output_tokens = n;
            }
        }

        let Some(choice) = v.pointer("/choices/0") else { return vec![] };
        let mut out = Vec::new();
        if !self.started {
            self.started = true;
            out.push(self.message_start());
        }

        let delta = choice.get("delta").cloned().unwrap_or(json!({}));
        if let Some(rc) = delta.get("reasoning_content").and_then(|x| x.as_str()) {
            if !rc.is_empty() {
                if !self.block_open {
                    out.extend(self.block_start(json!({ "type": "thinking", "thinking": "" })));
                }
                out.push(self.block_delta(json!({ "type": "thinking_delta", "thinking": rc })));
            }
        }
        if let Some(content) = delta.get("content").and_then(|x| x.as_str()) {
            if !content.is_empty() {
                if !self.block_open || self.pending_tool.is_some() {
                    out.extend(self.block_stop());
                    out.extend(self.block_start(json!({ "type": "text", "text": "" })));
                }
                out.push(self.block_delta(json!({ "type": "text_delta", "text": content })));
            }
        }
        if let Some(tcs) = delta.get("tool_calls").and_then(|t| t.as_array()) {
            for tc in tcs {
                if let Some(fn_obj) = tc.get("function") {
                    let name = fn_obj.get("name").and_then(|n| n.as_str()).unwrap_or("");
                    if !name.is_empty() && self.pending_tool.is_none() {
                        out.extend(self.block_stop());
                        let idx = self.next_index;
                        self.next_index += 1;
                        self.block_open = true;
                    let id = tc.get("id").and_then(|x| x.as_str()).unwrap_or("toolu_unknown").to_string();
                    out.push(sse_event("content_block_start", &json!({
                        "type": "content_block_start", "index": idx,
                        "content_block": { "type": "tool_use", "id": id, "name": name, "input": {} },
                    })));
                    self.pending_tool = Some((idx, id, name.to_string()));
                    }
                    if let Some(args) = fn_obj.get("arguments").and_then(|a| a.as_str()) {
                        out.push(self.block_delta(json!({ "type": "input_json_delta", "partial_json": args })));
                    }
                }
            }
        }

        if let Some(fr) = choice.get("finish_reason").and_then(|f| f.as_str()) {
            self.finish_seen = true;
            out.extend(self.block_stop());
            let stop_reason = match fr {
                "length" => "max_tokens",
                "tool_calls" => "tool_use",
                _ => "end_turn",
            };
            out.push(sse_event("message_delta", &json!({
                "type": "message_delta",
                "delta": { "stop_reason": stop_reason, "stop_sequence": Value::Null },
                "usage": { "output_tokens": self.output_tokens },
            })));
        }
        out
    }

    pub fn finish(&mut self) -> Vec<String> {
        let mut out = Vec::new();
        if !self.started {
            out.push(self.message_start());
        }
        out.extend(self.block_stop());
        if !self.stopped {
            out.push(sse_event("message_delta", &json!({
                "type": "message_delta",
                "delta": { "stop_reason": "end_turn", "stop_sequence": Value::Null },
                "usage": { "output_tokens": self.output_tokens },
            })));
            out.push(sse_event("message_stop", &json!({ "type": "message_stop" })));
            out.push("data: [DONE]\n\n".to_string());
        }
        out
    }
}

fn sse_event(event: &str, data: &Value) -> String {
    format!("event: {}\ndata: {}\n\n", event, data)
}

/// Convert a stream of OpenAI chat chunks into Anthropic message SSE.
pub fn convert_oai_sse_to_anthropic(
    body: axum::body::Body,
) -> axum::body::Body {
    use async_stream::stream;
    use futures_util::StreamExt;
    let mut conv = AnSseConverter::new();
    let stream = stream! {
        let mut stream = body.into_data_stream();
        let mut line_buf = String::new();
        while let Some(chunk) = stream.next().await {
            match chunk {
                Ok(bytes) => {
                    line_buf.push_str(&String::from_utf8_lossy(&bytes));
                    while let Some(pos) = line_buf.find("\n\n") {
                        let event_block: String = line_buf.drain(..pos + 2).collect();
                        for line in event_block.lines() {
                            for out in conv.feed_line(line) {
                                yield Ok::<Bytes, std::io::Error>(Bytes::from(out));
                            }
                        }
                    }
                }
                Err(_) => break,
            }
        }
        for out in conv.finish() {
            yield Ok(Bytes::from(out));
        }
    };
    axum::body::Body::from_stream(stream)
}

/// POST /v1/messages/count_tokens — same relay, never streamed.

/// POST /v1/messages/count_tokens — same relay, never streamed.
pub async fn count_tokens(
    state: State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, ChatError> {
    relay_anthropic(state, headers, body, true).await
}

async fn relay_anthropic(
    state: State<AppState>,
    headers: HeaderMap,
    body: Bytes,
    count_tokens: bool,
) -> Result<Response, ChatError> {
    let mut payload: Value = serde_json::from_slice(&body).map_err(|_| ChatError {
        status: StatusCode::BAD_REQUEST,
        message: "Invalid JSON body".into(),
    })?;

    // Inbound API-key gate (same policy as the OpenAI path).
    if state.db.require_api_key().await {
        let presented = crate::proxy::extract_api_key_pub(&headers);
        let valid = match presented {
            Some(k) => state.db.validate_api_key(&k).await,
            None => false,
        };
        if !valid {
            return Err(ChatError {
                status: StatusCode::UNAUTHORIZED,
                message: "Missing or invalid API key".into(),
            });
        }
    }

    let requested_model = payload
        .get("model")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    if requested_model.is_empty() {
        return Err(ChatError {
            status: StatusCode::BAD_REQUEST,
            message: "Missing model".into(),
        });
    }
    let is_stream = payload
        .get("stream")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    // Route model → claude connections. "claude/x" and "anthropic/x" prefixes
    // are stripped; bare ids pass through (Anthropic ids are canonical).
    let _ = &requested_model;

    let conns = state.db.connections_for_provider("claude", &requested_model).await;
    if conns.is_empty() {
        // Hybrid fallback: the Node translator can also serve claude via
        // other upstreams.
        if !state.node_upstream.is_empty() {
            return Ok(proxy_to_node_messages(state, headers, body).await);
        }
        return Err(ChatError {
            status: StatusCode::NOT_FOUND,
            message: "no active claude connection — add one in Providers".into(),
        });
    }

    // The model id sent upstream: strip a known provider prefix.
    let upstream_model = requested_model
        .split_once('/')
        .map(|(_, m)| m.to_string())
        .unwrap_or_else(|| requested_model.clone());
    if let Some(obj) = payload.as_object_mut() {
        obj.insert("model".into(), Value::String(upstream_model));
    }
    let payload_bytes = serde_json::to_vec(&payload).unwrap_or_default();

    let url = if count_tokens { UPSTREAM_COUNT_TOKENS } else { UPSTREAM_MESSAGES };

    let mut last_status: Option<StatusCode> = None;
    let mut last_body = Bytes::new();

    for conn0 in &conns {
        let id = conn0.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();

        // Auto-refresh expired OAuth tokens before the attempt.
        let expires_soon = conn0
            .get("expiresAt")
            .and_then(|v| v.as_str())
            .and_then(crate::db::parse_rfc3339_secs_pub)
            .map(|exp| exp <= crate::db::chrono_now_secs_pub() + 60)
            .unwrap_or(false);
        if expires_soon && conn0.get("refreshToken").and_then(|v| v.as_str()).is_some_and(|s| !s.is_empty()) {
            if let Err(e) = crate::oauth::refresh(&state.db, &state.client, &id).await {
                tracing::warn!(connection = %id, "token refresh failed, trying stored token: {e}");
            }
        }

        // Re-read the freshest row (the refresh may have rotated tokens).
        let Some(conn) = state.db.get_connection_full(&id).await else { continue };
        let access = conn
            .get("accessToken")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(String::from);
        let api_key = conn
            .get("apiKey")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(String::from);

        let (auth_style, secret) = if access.is_some() {
            ("oauth", access.unwrap())
        } else if api_key.is_some() {
            ("apikey", api_key.unwrap())
        } else {
            continue;
        };

        let mut req = http_req(&state, url, payload_bytes.clone(), is_stream && !count_tokens)
            .header("anthropic-version", API_VERSION);
        req = if auth_style == "oauth" {
            req.header("authorization", format!("Bearer {secret}"))
                .header("anthropic-beta", OAUTH_BETA)
        } else {
            req.header("x-api-key", secret)
        };

        let upstream = match req.send().await {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!(provider = "claude", "anthropic attempt failed: {e}");
                last_status = Some(StatusCode::BAD_GATEWAY);
                continue;
            }
        };

        let status = upstream.status();
        if !status.is_success() {
            last_status = Some(status);
            last_body = upstream.bytes().await.unwrap_or_default();
            // 401/403/429 → try the next connection; keep other errors too.
            continue;
        }

        let ct = upstream
            .headers()
            .get("content-type")
            .cloned()
            .unwrap_or_else(|| HeaderValue::from_static("application/json"));
        let mut resp = Response::new(Body::from_stream(upstream.bytes_stream()));
        *resp.status_mut() = StatusCode::OK;
        resp.headers_mut().insert("content-type", ct);
        resp.headers_mut()
            .insert("cache-control", HeaderValue::from_static("no-cache"));
        return Ok(resp);
    }

    if let Some(status) = last_status {
        let ct = "application/json";
        return Ok(Response::builder()
            .status(status)
            .header("content-type", ct)
            .body(Body::from(last_body))
            .unwrap());
    }
    Err(ChatError {
        status: StatusCode::BAD_GATEWAY,
        message: "no claude connection could be reached".into(),
    })
}

fn http_req(
    state: &AppState,
    url: &str,
    body: Vec<u8>,
    stream: bool,
) -> reqwest::RequestBuilder {
    let mut req = state
        .client
        .post(url)
        .header("content-type", "application/json")
        .body(body);
    if stream {
        req = req.header("accept", "text/event-stream");
    }
    req
}

async fn proxy_to_node_messages(state: State<AppState>, headers: HeaderMap, body: Bytes) -> Response {
    use futures_util::StreamExt;
    let url = format!("{}/v1/messages", state.node_upstream);
    let mut hdrs = HeaderMap::new();
    for (name, value) in &headers {
        if matches!(name.as_str(), "host" | "content-length" | "connection" | "transfer-encoding") {
            continue;
        }
        hdrs.append(name.clone(), value.clone());
    }
    match state.client.post(&url).headers(hdrs).body(body).send().await {
        Ok(up) => {
            let status = up.status();
            let ct = up
                .headers()
                .get("content-type")
                .cloned()
                .unwrap_or_else(|| HeaderValue::from_static("application/json"));
            let stream = up.bytes_stream().map(|c| c.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e)));
            let mut resp = Response::new(Body::from_stream(stream));
            *resp.status_mut() = status;
            resp.headers_mut().insert("content-type", ct);
            resp
        }
        Err(e) => error(StatusCode::BAD_GATEWAY, format!("Node upstream error: {e}")),
    }
}
