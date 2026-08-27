//! Auth middleware — the Rust equivalent of src/dashboardGuard.js. Applied to
//! the `/api/*` tree in main.rs. Requests to PUBLIC_API_PATHS pass through
//! (login, logout, status, health); everything else under /api requires a valid
//! `auth_token` JWT cookie (or, for now, a local/loopback caller — matching the
//! Node guard's local bypass).
//!
//! Scope note: the chat proxy (/v1/*) has its OWN API-key gate inside the
//! handler, so it is NOT routed through this middleware.

use axum::{
    body::Body,
    extract::{ConnectInfo, Request},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};
use serde_json::json;
use std::net::SocketAddr;

/// Paths under /api that never require a session (dashboardGuard.js
/// PUBLIC_API_PATHS + the legacy /api/health alias).
const PUBLIC_API_PATHS: &[&str] = &[
    "/api/health",
    "/api/init",
    "/api/locale",
    "/api/auth/login",
    "/api/auth/logout",
    "/api/auth/status",
    "/api/auth/oidc",
    "/api/version",
    "/api/settings/require-login",
];

fn is_public(path: &str) -> bool {
    PUBLIC_API_PATHS
        .iter()
        .any(|p| path == *p || path.starts_with(&format!("{p}/")))
}

pub async fn require_auth(
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    req: Request<Body>,
    next: Next,
) -> Response {
    let path = req.uri().path().to_string();

    // Public /api paths pass straight through.
    if is_public(&path) {
        return next.run(req).await;
    }

    let headers = req.headers().clone();
    // Loopback peers bypass (matches dashboardGuard.js isLocalRequest, which is
    // based on the TCP peer address — NOT the client-controlled Host header).
    if peer.ip().is_loopback() {
        return next.run(req).await;
    }

    if super::extract_and_verify(&headers) {
        next.run(req).await
    } else {
        (
            StatusCode::UNAUTHORIZED,
            axum::Json(json!({ "error": "Authentication required" })),
        )
            .into_response()
    }
}
