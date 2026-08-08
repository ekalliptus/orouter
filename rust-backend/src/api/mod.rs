//! HTTP handlers for the non-chat routes: health + models listing.
//! Both are public (no auth gate), matching the Go backend (server.go:43,49).

use axum::{http::StatusCode, response::IntoResponse};

/// GET /health — pure liveness, no DB ping. Mirrors health.go.
pub async fn health() -> impl IntoResponse {
    (
        StatusCode::OK,
        [("content-type", "application/json"), ("cache-control", "no-store")],
        r#"{"ok":true}"#,
    )
}

/// GET /v1/models — static OpenAI-compatible catalog from the embedded snapshot
/// (models.go native path). No connection/secret data is exposed.
pub async fn models() -> impl IntoResponse {
    let body = crate::snapshot::openai_model_list();
    (StatusCode::OK, axum::Json(body))
}
