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

/// POST /v1/messages — Anthropic-format relay with connection fallback.
pub async fn messages(
    state: State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, ChatError> {
    relay_anthropic(state, headers, body, false).await
}

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

    // The model id sent upstream: strip a leading "claude/" or "anthropic/".
    let upstream_model = requested_model
        .strip_prefix("claude/")
        .or_else(|| requested_model.strip_prefix("anthropic/"))
        .unwrap_or(&requested_model)
        .to_string();
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
