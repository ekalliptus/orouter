//! HTTP handlers: public (health + models) and protected dashboard routes.
//! The protected routes live in `dashboard`; the auth gate itself is the
//! `require_auth` middleware wired in main.rs (mirrors dashboardGuard.js).

pub mod dashboard;

use axum::{http::StatusCode, response::IntoResponse};

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
