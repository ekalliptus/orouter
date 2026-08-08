//! Dashboard API routes (protected by the auth middleware):
//!   GET   /api/settings    — merged settings, secrets redacted
//!   PATCH /api/settings    — atomic merge update (password change hashes)
//!   GET   /api/keys        — list API keys
//!   POST  /api/keys        — create an sk- key (needs machineId)
//!   DELETE /api/keys/:id   — delete a key
//!   GET   /api/providers   — list connections, secrets stripped
//!
//! Mirrors the Node routes in src/app/api/{settings,keys,providers}/route.js,
//! minus the Node-only side effects (combo rotation reset, auto-ping config,
//! outbound proxy env mutation) which are out of v1 scope.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde_json::{json, Value};

use crate::proxy::AppState;

// ---- /api/settings -------------------------------------------------------

pub async fn settings_get(State(state): State<AppState>) -> impl IntoResponse {
    let mut settings = state.db.get_settings_safe().await;
    // Two env-derived flags the Node GET adds (the rest come from the DB).
    let enable_request_logs = std::env::var("ENABLE_REQUEST_LOGS").as_deref() == Ok("true");
    let enable_translator = std::env::var("ENABLE_TRANSLATOR").as_deref() == Ok("true");
    if let Some(obj) = settings.as_object_mut() {
        obj.insert("enableRequestLogs".into(), Value::Bool(enable_request_logs));
        obj.insert("enableTranslator".into(), Value::Bool(enable_translator));
    }
    (StatusCode::OK, Json(settings))
}

pub async fn settings_patch(
    State(state): State<AppState>,
    Json(body): Json<Value>,
) -> Response {
    match state.db.update_settings_safe(body).await {
        Ok(safe) => {
            let mut resp = (StatusCode::OK, Json(safe)).into_response();
            resp.headers_mut().insert("cache-control", "no-store".parse().unwrap());
            resp
        }
        Err(e) => {
            let msg = e.to_string();
            let status = if msg.contains("current password") {
                StatusCode::UNAUTHORIZED
            } else {
                StatusCode::INTERNAL_SERVER_ERROR
            };
            error_response(status, &msg)
        }
    }
}

// ---- /api/keys -----------------------------------------------------------

pub async fn keys_get(State(state): State<AppState>) -> impl IntoResponse {
    let keys = state.db.list_api_keys().await;
    (StatusCode::OK, Json(json!({ "keys": keys })))
}

pub async fn keys_post(
    State(state): State<AppState>,
    Json(body): Json<Value>,
) -> Response {
    let name = body.get("name").and_then(|v| v.as_str()).unwrap_or("");
    if name.is_empty() {
        return error_response(StatusCode::BAD_REQUEST, "Name is required");
    }
    // machineId: the Node route derives it server-side (getConsistentMachineId).
    // We derive a stable id from the data dir if no env override is set.
    let machine_id = crate::auth::machine_id();
    match state.db.create_api_key(name, &machine_id).await {
        Ok(key) => (StatusCode::CREATED, Json(key)).into_response(),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

pub async fn keys_delete(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Response {
    let removed = state.db.delete_api_key(&id).await;
    if removed {
        (StatusCode::OK, Json(json!({ "success": true }))).into_response()
    } else {
        error_response(StatusCode::NOT_FOUND, "Key not found")
    }
}

// ---- /api/providers ------------------------------------------------------

pub async fn providers_get(State(state): State<AppState>) -> impl IntoResponse {
    let connections = state.db.list_connections_safe().await;
    (StatusCode::OK, Json(json!({ "connections": connections })))
}

use axum::response::Response;

fn error_response(status: StatusCode, message: &str) -> Response {
    (status, Json(json!({ "error": message }))).into_response()
}
