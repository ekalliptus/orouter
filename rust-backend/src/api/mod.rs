//! HTTP handlers: public (health + models) and protected dashboard routes.
//! The protected routes live in `dashboard`; the auth gate itself is the
//! `require_auth` middleware wired in main.rs (mirrors dashboardGuard.js).

pub mod dashboard;

use axum::{http::StatusCode, response::IntoResponse};
use once_cell::sync::Lazy;
use serde_json::json;
use std::time::Instant;

static STARTED_AT: Lazy<Instant> = Lazy::new(Instant::now);

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
/// (models.go native path). No connection/secret data is exposed.
pub async fn models() -> impl IntoResponse {
    let body = crate::snapshot::openai_model_list();
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
