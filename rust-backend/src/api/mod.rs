//! HTTP handlers: public (health + models) and protected dashboard routes.
//! The protected routes live in `dashboard`; the auth gate itself is the
//! `require_auth` middleware wired in main.rs (mirrors dashboardGuard.js).

pub mod dashboard;

use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
};
use once_cell::sync::Lazy;
use serde_json::{json, Value};
use std::time::Instant;

use crate::proxy::AppState;

static STARTED_AT: Lazy<Instant> = Lazy::new(Instant::now);

/// POST /v1/web/fetch — URL extraction via configured web providers.
pub async fn web_fetch(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> impl IntoResponse {
    if gate_chat(&state, &headers).await.is_err() {
        return (
            StatusCode::UNAUTHORIZED,
            axum::Json(json!({ "error": "Missing or invalid API key" })),
        );
    }
    let payload: Value = serde_json::from_slice(&body).unwrap_or(json!({}));
    let result = crate::webtools::web_fetch(&state.db, &state.client, &payload).await;
    (StatusCode::OK, axum::Json(result))
}

/// POST /v1/search — web search via configured search providers.
pub async fn web_search(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> impl IntoResponse {
    if gate_chat(&state, &headers).await.is_err() {
        return (
            StatusCode::UNAUTHORIZED,
            axum::Json(json!({ "error": "Missing or invalid API key" })),
        );
    }
    let payload: Value = serde_json::from_slice(&body).unwrap_or(json!({}));
    let result = crate::webtools::web_search(&state.db, &state.client, &payload).await;
    (StatusCode::OK, axum::Json(result))
}

async fn gate_chat(state: &AppState, headers: &HeaderMap) -> Result<(), ()> {
    if !state.db.require_api_key().await {
        return Ok(());
    }
    let key = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|a| a.strip_prefix("Bearer "))
        .map(|s| s.trim().to_string())
        .or_else(|| {
            headers
                .get("x-api-key")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.trim().to_string())
        });
    match key {
        Some(k) if state.db.validate_api_key(&k).await => Ok(()),
        _ => Err(()),
    }
}

/// Native /v1 passthrough wrappers (shared gate policy with chat).
pub async fn v1_responses(
    state: State<AppState>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Result<axum::response::Response, crate::proxy::ChatError> {
    gate_or_err(&state, &headers).await?;
    crate::proxy::relay_provider_endpoint(state, headers, body, "/responses").await
}

pub async fn v1_images(
    state: State<AppState>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Result<axum::response::Response, crate::proxy::ChatError> {
    gate_or_err(&state, &headers).await?;
    crate::proxy::relay_provider_endpoint(state, headers, body, "/images/generations").await
}

pub async fn v1_audio_speech(
    state: State<AppState>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Result<axum::response::Response, crate::proxy::ChatError> {
    gate_or_err(&state, &headers).await?;
    crate::proxy::relay_provider_endpoint(state, headers, body, "/audio/speech").await
}

async fn gate_or_err(state: &AppState, headers: &HeaderMap) -> Result<(), crate::proxy::ChatError> {
    if !state.db.require_api_key().await {
        return Ok(());
    }
    let key = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|a| a.strip_prefix("Bearer "))
        .map(|s| s.trim().to_string())
        .or_else(|| {
            headers
                .get("x-api-key")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.trim().to_string())
        });
    match key {
        Some(k) if state.db.validate_api_key(&k).await => Ok(()),
        _ => Err(crate::proxy::ChatError {
            status: StatusCode::UNAUTHORIZED,
            message: "Missing or invalid API key".into(),
        }),
    }
}

/// GET /health — pure liveness, no DB ping. Mirrors health.go.
pub async fn health() -> impl IntoResponse {
    (
        StatusCode::OK,
        [
            ("content-type", "application/json"),
            ("cache-control", "no-store"),
        ],
        r#"{"ok":true}"#,
    )
}

/// GET /v1/models — static OpenAI-compatible catalog from the embedded snapshot
/// plus combo names (clients can call a combo like any model). No secrets.
pub async fn models(State(state): State<AppState>) -> impl IntoResponse {
    let mut body = crate::snapshot::openai_model_list();
    if let Some(arr) = body
        .get_mut("data")
        .and_then(|v| v.as_array_mut())
    {
        for (name, _models) in state.db.combo_chains().await {
            arr.push(json!({
                "id": name,
                "object": "model",
                "owned_by": "combo",
            }));
        }
    }
    (StatusCode::OK, axum::Json(body))
}

/// GET /api/version — engine identity + uptime (parity-lite with Node's
/// /api/version; enough for the Settings page info card).
pub async fn version() -> impl IntoResponse {
    let body = json!({
        "version": env!("CARGO_PKG_VERSION"),
        "engine": "rust",
        "uptimeSecs": STARTED_AT.elapsed().as_secs(),
    });
    (StatusCode::OK, axum::Json(body))
}

/// POST /api/version/shutdown — graceful-ish stop requested from the UI.
/// Answers first, then exits the process (mirrors Node's version/shutdown).
pub async fn shutdown() -> impl IntoResponse {
    tracing::info!("shutdown requested via /api/version/shutdown");
    tokio::spawn(async {
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        std::process::exit(0);
    });
    (StatusCode::OK, axum::Json(json!({ "ok": true })))
}

/// JSON fallback for unknown backend paths. Kept separate from the SPA fallback
/// so typos under /api and /v1 never return index.html.
pub async fn not_found() -> impl IntoResponse {
    (
        StatusCode::NOT_FOUND,
        axum::Json(json!({ "error": "Not found" })),
    )
}
