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
    body::Body,
    extract::{Path, State},
    http::{HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde_json::{json, Value};

use crate::db::CreateConnection;
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

pub async fn settings_patch(State(state): State<AppState>, Json(body): Json<Value>) -> Response {
    match state.db.update_settings_safe(body).await {
        Ok(safe) => {
            let mut resp = (StatusCode::OK, Json(safe)).into_response();
            resp.headers_mut()
                .insert("cache-control", "no-store".parse().unwrap());
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

pub async fn keys_post(State(state): State<AppState>, Json(body): Json<Value>) -> Response {
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

pub async fn keys_delete(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    let removed = state.db.delete_api_key(&id).await;
    if removed {
        (StatusCode::OK, Json(json!({ "success": true }))).into_response()
    } else {
        error_response(StatusCode::NOT_FOUND, "Key not found")
    }
}

/// PUT /api/keys/:id — {isActive: bool} kill switch (Node parity).
pub async fn keys_update(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> Response {
    let Some(active) = body.get("isActive").and_then(|v| v.as_bool()) else {
        return error_response(StatusCode::BAD_REQUEST, "isActive (boolean) is required");
    };
    if state.db.set_api_key_active(&id, active).await {
        (StatusCode::OK, Json(json!({ "success": true, "isActive": active }))).into_response()
    } else {
        error_response(StatusCode::NOT_FOUND, "Key not found")
    }
}

// ---- /api/providers ------------------------------------------------------

pub async fn providers_get(State(state): State<AppState>) -> impl IntoResponse {
    let connections = state.db.list_connections_safe().await;
    (StatusCode::OK, Json(json!({ "connections": connections })))
}

/// POST /api/providers — create a connection (apikey). Minimal validation
/// (provider non-empty, apiKey present unless ollama-local, name present).
/// Mirrors the Node create route's validation surface.
pub async fn providers_post(State(state): State<AppState>, Json(body): Json<Value>) -> Response {
    let provider = body
        .get("provider")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    let api_key = body
        .get("apiKey")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let name = body
        .get("name")
        .or_else(|| body.get("displayName"))
        .and_then(|v| v.as_str())
        .unwrap_or(&provider)
        .to_string();

    if provider.is_empty() {
        return error_response(StatusCode::BAD_REQUEST, "Invalid provider");
    }
    if api_key.is_empty() && provider != "ollama-local" {
        return error_response(StatusCode::BAD_REQUEST, "API Key is required");
    }
    if name.is_empty() {
        return error_response(StatusCode::BAD_REQUEST, "Name is required");
    }

    let input = CreateConnection {
        provider: provider.clone(),
        auth_type: "apikey".to_string(),
        name,
        api_key,
        priority: body.get("priority").and_then(|v| v.as_i64()),
        is_active: true,
        test_status: "unknown".to_string(),
        email: body.get("email").and_then(|v| v.as_str()).map(String::from),
        provider_specific_data: body.get("providerSpecificData").cloned(),
        extra: serde_json::Map::new(),
    };

    match state.db.create_connection(input).await {
        Ok(id) => {
            // Return the created connection (safe — secrets stripped).
            let conn = state
                .db
                .get_connection_full(&id)
                .await
                .map(|mut c| {
                    if let Some(obj) = c.as_object_mut() {
                        obj.remove("apiKey");
                        obj.remove("accessToken");
                        obj.remove("refreshToken");
                        obj.remove("idToken");
                    }
                    c
                })
                .unwrap_or(json!({ "id": id }));
            (StatusCode::CREATED, Json(json!({ "connection": conn }))).into_response()
        }
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

/// PUT /api/providers/:id — update a connection.
pub async fn providers_update(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> Response {
    match state.db.update_connection_safe(&id, body).await {
        Ok(Some(conn)) => (StatusCode::OK, Json(json!({ "connection": conn }))).into_response(),
        Ok(None) => error_response(StatusCode::NOT_FOUND, "Connection not found"),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

/// DELETE /api/providers/:id — hard-delete + reorder.
pub async fn providers_delete(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    if state.db.delete_connection(&id).await {
        (
            StatusCode::OK,
            Json(json!({ "message": "Connection deleted successfully" })),
        )
            .into_response()
    } else {
        error_response(StatusCode::NOT_FOUND, "Connection not found")
    }
}

/// POST /api/providers/:id/test — probe the upstream connection.
///
/// For apikey providers with a native OpenAI transport, GET the provider's
/// /models endpoint with the stored key; valid iff HTTP 200. Writes testStatus
/// (active|error) + lastError/lastErrorAt back to the row. Returns {valid,error}.
pub async fn providers_test(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    let Some(conn) = state.db.get_connection_full(&id).await else {
        return error_response(StatusCode::NOT_FOUND, "Connection not found");
    };
    let provider = conn.get("provider").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let api_key = conn.get("apiKey").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let access_token = conn.get("accessToken").and_then(|v| v.as_str()).unwrap_or("").to_string();

    // OAuth-style connection (no api key, has access token): validate via the
    // provider's native quota probe — same call the Quota Tracker uses.
    if api_key.is_empty() && !access_token.is_empty() {
        let http = state.client_for(state.db.resolve_connection_proxy(&id).await.as_deref());
        let quota = crate::quota::fetch_quota(&state.db, &http, &conn).await;
        let available = quota.get("available").and_then(|v| v.as_bool()).unwrap_or(false);
        let error = quota
            .get("error")
            .or_else(|| quota.get("message"))
            .and_then(|v| v.as_str())
            .map(|s| Value::String(s.to_string()))
            .unwrap_or(Value::Null);
        let now = iso_now();
        let _ = state.db.update_connection_safe(&id, json!({
            "testStatus": if available { "active" } else { "error" },
            "lastError": if available { Value::Null } else { error.clone() },
            "lastErrorAt": if available { Value::Null } else { Value::String(now) },
        })).await;
        return (
            StatusCode::OK,
            Json(json!({ "valid": available, "error": if available { Value::Null } else { error }, "refreshed": false })),
        ).into_response();
    }

    if api_key.is_empty() {
        return error_response(StatusCode::BAD_REQUEST, "No API key on this connection");
    }

    // Resolve the provider's native transport to find its base URL + auth.
    // Derive the probe URL: snapshot transport first, then connection's own baseUrl.
    let (models_url, auth_header, auth_scheme, extra_headers) = match crate::snapshot::resolve(&format!("{provider}/__probe__")) {
        Some(r) => {
            let base = r.transport.base_url.trim_end_matches('/').to_string();
            let mu = base.trim_end_matches("/chat/completions").trim_end_matches("/completions").to_string();
            let probe_url = if provider == "openrouter" {
                "https://openrouter.ai/api/v1/auth/key".to_string()
            } else {
                format!("{mu}/models")
            };
            (probe_url, r.transport.auth_header, r.transport.auth_scheme, r.transport.headers)
        }
        None => {
            // Fallback: use the connection's own baseUrl from its data blob.
            let base = conn.get("baseUrl").and_then(|v| v.as_str()).unwrap_or("").to_string();
            if base.is_empty() || base == "none" {
                return error_response(StatusCode::NOT_IMPLEMENTED, "No probe URL available for this provider");
            }
            let mu = base.trim_end_matches('/').trim_end_matches("/chat/completions").to_string();
            (format!("{mu}/models"), "authorization".to_string(), "bearer".to_string(), std::collections::HashMap::new())
        }
    };

    let client = &state.client;
    // Build the full header set up front (reqwest consumes it via .headers()).
    let mut hdrs = axum::http::HeaderMap::new();
    let token = if auth_scheme.eq_ignore_ascii_case("bearer") {
        format!("Bearer {api_key}")
    } else {
        api_key.to_string()
    };
    let header_name = if auth_header.is_empty() {
        "authorization"
    } else {
        auth_header.as_str()
    };
    if let (Ok(name), Ok(val)) = (
        axum::http::HeaderName::try_from(header_name),
        axum::http::HeaderValue::from_str(&token),
    ) {
        hdrs.insert(name, val);
    }
    for (k, v) in &extra_headers {
        if let (Ok(name), Ok(val)) = (
            axum::http::HeaderName::try_from(k.as_str()),
            axum::http::HeaderValue::from_str(v),
        ) {
            hdrs.insert(name, val);
        }
    }
    let req = client.get(&models_url).headers(hdrs);

    let result = req.send().await;
    let (valid, error_msg) = match result {
        Ok(r) => {
            let status = r.status().as_u16();
            if status == 200 {
                (true, Value::Null)
            } else {
                (false, Value::String(format!("HTTP {status}")))
            }
        }
        Err(e) => (false, Value::String(format!("network error: {e}"))),
    };

    // Write testStatus back to the row.
    let now = iso_now();
    let patch = json!({
        "testStatus": if valid { "active" } else { "error" },
        "lastError": if valid { Value::Null } else { error_msg.clone() },
        "lastErrorAt": if valid { Value::Null } else { Value::String(now) },
    });
    let _ = state.db.update_connection_safe(&id, patch).await;

    (
        StatusCode::OK,
        Json(json!({ "valid": valid, "error": if valid { Value::Null } else { error_msg }, "refreshed": false })),
    ).into_response()
}

// ---- /api/oauth/:provider — native PKCE login/refresh ---------------------

/// GET /api/oauth/providers — which providers support native login here.
pub async fn oauth_providers() -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(json!({
            "providers": crate::oauth::supported(),
            "note": "credential login/refresh is native; inference for these providers needs the Node engine (hybrid mode)",
        })),
    )
}

/// POST /api/oauth/:provider/start → { authUrl, state }
pub async fn oauth_start(Path(provider): Path<String>) -> Response {
    match crate::oauth::start(&provider) {
        Some((auth_url, state)) => (
            StatusCode::OK,
            Json(json!({ "authUrl": auth_url, "state": state })),
        )
            .into_response(),
        None => error_response(
            StatusCode::NOT_FOUND,
            "unsupported OAuth provider (native: claude, codex, antigravity)",
        ),
    }
}

/// POST /api/oauth/:provider/exchange { state, code, connectionId? }
pub async fn oauth_exchange(
    State(state): State<AppState>,
    Path(_provider): Path<String>,
    Json(body): Json<Value>,
) -> Response {
    let state_in = body.get("state").and_then(|v| v.as_str()).unwrap_or("");
    let code = body.get("code").and_then(|v| v.as_str()).unwrap_or("");
    let connection_id = body
        .get("connectionId")
        .and_then(|v| v.as_str())
        .map(String::from);
    if state_in.is_empty() || code.is_empty() {
        return error_response(StatusCode::BAD_REQUEST, "state and code are required");
    }
    match crate::oauth::exchange(&state.db, &state.client, state_in, code, connection_id).await {
        Ok(conn) => (StatusCode::OK, Json(json!({ "connection": conn }))).into_response(),
        Err(e) => error_response(StatusCode::BAD_REQUEST, &e),
    }
}

/// POST /api/oauth/:provider/refresh { connectionId }
pub async fn oauth_refresh(
    State(state): State<AppState>,
    Path(_provider): Path<String>,
    Json(body): Json<Value>,
) -> Response {
    let Some(connection_id) = body.get("connectionId").and_then(|v| v.as_str()) else {
        return error_response(StatusCode::BAD_REQUEST, "connectionId is required");
    };
    match crate::oauth::refresh(&state.db, &state.client, connection_id).await {
        Ok(v) => (StatusCode::OK, Json(v)).into_response(),
        Err(e) => error_response(StatusCode::BAD_GATEWAY, &e),
    }
}

/// POST /api/settings/proxy-test {proxyUrl} — Node profile-page parity:
/// verify an outbound proxy by fetching an IP echo through it.
pub async fn settings_proxy_test(State(state): State<AppState>, Json(body): Json<Value>) -> Response {
    let Some(proxy_url) = body.get("proxyUrl").and_then(|v| v.as_str()).map(str::trim) else {
        return error_response(StatusCode::BAD_REQUEST, "proxyUrl is required");
    };
    if proxy_url.is_empty() {
        return error_response(StatusCode::BAD_REQUEST, "proxyUrl is empty");
    }
    let client = state.client_for(Some(proxy_url));
    match client
        .get("https://api.ipify.org?format=json")
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await
    {
        Ok(r) => {
            let ok = r.status().is_success();
            let ip = r
                .json::<Value>()
                .await
                .ok()
                .and_then(|v| v.get("ip").and_then(|x| x.as_str()).map(String::from));
            (
                StatusCode::OK,
                Json(json!({ "ok": ok, "ip": ip })),
            )
                .into_response()
        }
        Err(e) => (
            StatusCode::OK,
            Json(json!({ "ok": false, "error": format!("network error: {e}") })),
        )
            .into_response(),
    }
}

// ---- /api/translator — native parity with the Node translator builder ----
//
// The Node builder reads captured translation-pipeline dumps from
// <cwd>/logs/translator/ (1_req_client.json … 7_res_client.txt). We serve the
// SAME files so captures written by a hybrid Node run are inspectable here.

fn translator_dump_dir() -> std::path::PathBuf {
    for cand in ["logs/translator", "../logs/translator"] {
        let p = std::path::PathBuf::from(cand);
        if p.is_dir() {
            return p;
        }
    }
    std::path::PathBuf::from("../logs/translator")
}

pub async fn translator_dumps_list() -> impl IntoResponse {
    let dir = translator_dump_dir();
    let mut files: Vec<Value> = Vec::new();
    if let Ok(rd) = std::fs::read_dir(&dir) {
        for entry in rd.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.') {
                continue;
            }
            let (size, modified) = match entry.metadata() {
                Ok(m) => (
                    m.len(),
                    m.modified()
                        .ok()
                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|d| d.as_secs())
                        .unwrap_or(0),
                ),
                Err(_) => (0, 0),
            };
            files.push(json!({ "name": name, "size": size, "modifiedSecs": modified }));
        }
    }
    files.sort_by(|a, b| {
        b.get("modifiedSecs")
            .and_then(|v| v.as_u64())
            .unwrap_or(0)
            .cmp(&a.get("modifiedSecs").and_then(|v| v.as_u64()).unwrap_or(0))
    });
    (StatusCode::OK, Json(json!({ "dir": dir.display().to_string(), "files": files })))
}

/// GET /api/translator/dumps/:name — raw file content. The filename is
/// validated (no separators, Node's stage-dump naming shape).
pub async fn translator_dumps_get(Path(name): Path<String>) -> Response {
    let safe = !name.is_empty()
        && !name.contains(['/', '\\'])
        && !name.contains("..")
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'));
    if !safe {
        return error_response(StatusCode::BAD_REQUEST, "invalid file name");
    }
    let path = translator_dump_dir().join(&name);
    match std::fs::read(&path) {
        Ok(bytes) => Response::builder()
            .status(StatusCode::OK)
            .header("content-type", "text/plain; charset=utf-8")
            .body(Body::from(bytes))
            .unwrap(),
        Err(_) => error_response(StatusCode::NOT_FOUND, "capture file not found"),
    }
}

// ---- /api/models management (alias / custom / disabled) -------------------

pub async fn model_alias_get(State(state): State<AppState>) -> impl IntoResponse {
    (StatusCode::OK, Json(json!({ "aliases": crate::modelstore::aliases(&state.db).await })))
}

pub async fn model_alias_set(State(state): State<AppState>, Json(body): Json<Value>) -> Response {
    let alias = body.get("alias").and_then(|v| v.as_str()).unwrap_or("");
    let model = body.get("model").and_then(|v| v.as_str()).unwrap_or("");
    match crate::modelstore::set_alias(&state.db, alias, model).await {
        Ok(()) => (StatusCode::OK, Json(json!({ "success": true }))).into_response(),
        Err(e) => error_response(StatusCode::BAD_REQUEST, &e.to_string()),
    }
}

pub async fn model_alias_delete(State(state): State<AppState>, axum::extract::Query(q): axum::extract::Query<std::collections::HashMap<String, String>>) -> Response {
    let Some(alias) = q.get("alias") else {
        return error_response(StatusCode::BAD_REQUEST, "alias query param required");
    };
    crate::modelstore::remove_alias(&state.db, alias).await;
    (StatusCode::OK, Json(json!({ "success": true }))).into_response()
}

pub async fn model_custom_get(State(state): State<AppState>) -> impl IntoResponse {
    (StatusCode::OK, Json(json!({ "models": crate::modelstore::custom_all(&state.db).await })))
}

pub async fn model_custom_post(State(state): State<AppState>, Json(body): Json<Value>) -> Response {
    let provider_alias = body.get("providerAlias").and_then(|v| v.as_str()).unwrap_or("");
    let id = body.get("id").and_then(|v| v.as_str()).unwrap_or("");
    let model_type = body.get("type").and_then(|v| v.as_str()).unwrap_or("llm");
    match crate::modelstore::set_custom(&state.db, provider_alias, id, model_type, body.clone()).await {
        Ok(()) => (StatusCode::CREATED, Json(json!({ "success": true }))).into_response(),
        Err(e) => error_response(StatusCode::BAD_REQUEST, &e.to_string()),
    }
}

pub async fn model_custom_delete(State(state): State<AppState>, axum::extract::Query(q): axum::extract::Query<std::collections::HashMap<String, String>>) -> Response {
    let (Some(pa), Some(id)) = (q.get("providerAlias"), q.get("id")) else {
        return error_response(StatusCode::BAD_REQUEST, "providerAlias and id required");
    };
    crate::modelstore::remove_custom(&state.db, pa, id, q.get("type").map(|s| s.as_str()).unwrap_or("llm"));
    (StatusCode::OK, Json(json!({ "success": true }))).into_response()
}

pub async fn model_disabled_get(State(state): State<AppState>, axum::extract::Query(q): axum::extract::Query<std::collections::HashMap<String, String>>) -> impl IntoResponse {
    match q.get("providerAlias") {
        Some(pa) => (
            StatusCode::OK,
            Json(json!({ "ids": crate::modelstore::disabled_for(&state.db, pa).await })),
        )
            .into_response(),
        None => (
            StatusCode::OK,
            Json(json!({ "disabled": crate::modelstore::disabled_all(&state.db).await })),
        )
            .into_response(),
    }
}

pub async fn model_disabled_post(State(state): State<AppState>, Json(body): Json<Value>) -> Response {
    let Some(pa) = body.get("providerAlias").and_then(|v| v.as_str()) else {
        return error_response(StatusCode::BAD_REQUEST, "providerAlias required");
    };
    let Some(ids) = body.get("ids").and_then(|v| v.as_array()).map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect::<Vec<_>>()) else {
        return error_response(StatusCode::BAD_REQUEST, "ids[] required");
    };
    crate::modelstore::disable_models(&state.db, pa, &ids).await;
    (StatusCode::OK, Json(json!({ "success": true }))).into_response()
}

pub async fn model_disabled_delete(State(state): State<AppState>, axum::extract::Query(q): axum::extract::Query<std::collections::HashMap<String, String>>) -> Response {
    let Some(pa) = q.get("providerAlias") else {
        return error_response(StatusCode::BAD_REQUEST, "providerAlias required");
    };
    match q.get("id") {
        Some(id) => crate::modelstore::enable_models(&state.db, pa, &[id.clone()]).await,
        None => crate::modelstore::enable_all_for(&state.db, pa).await,
    }
    (StatusCode::OK, Json(json!({ "success": true }))).into_response()
}

// ---- /api/usage/request-details (observability) ---------------------------

pub async fn usage_request_details(
    State(state): State<AppState>,
    axum::extract::Query(q): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    let page = q.get("page").and_then(|v| v.parse().ok()).unwrap_or(1);
    let page_size = q.get("pageSize").and_then(|v| v.parse().ok()).unwrap_or(20);
    let rows = state.db.request_details(page, page_size, q.get("provider").map(|s| s.as_str())).await;
    (StatusCode::OK, Json(json!({ "details": rows, "page": page, "pageSize": page_size })))
}

/// GET /api/usage/request-logs — compact log lines from requestDetails.
pub async fn usage_request_logs(State(state): State<AppState>) -> impl IntoResponse {
    let rows = state.db.request_details(1, 100, None).await;
    let lines: Vec<String> = rows
        .iter()
        .map(|r| {
            format!(
                "{} | {} | {} | {} | {}",
                r.get("timestamp").and_then(|v| v.as_str()).unwrap_or("?"),
                r.get("provider").and_then(|v| v.as_str()).unwrap_or("-").to_uppercase(),
                r.get("model").and_then(|v| v.as_str()).unwrap_or("-"),
                r.get("connectionId").and_then(|v| v.as_str()).map(|s| s.get(..8).unwrap_or(s).to_string()).unwrap_or_default(),
                r.get("status").and_then(|v| v.as_str()).unwrap_or("-"),
            )
        })
        .collect();
    (StatusCode::OK, Json(lines))
}

// ---- POST /api/providers/test-batch — server-side group test --------------

pub async fn providers_test_batch(State(state): State<AppState>, Json(body): Json<Value>) -> Response {
    let mode = body.get("mode").and_then(|v| v.as_str()).unwrap_or("all").to_string();
    let conns = state.db.list_connections_safe().await;
    let mut results: Vec<Value> = Vec::new();
    for c in conns.iter().filter(|c| c.get("isActive").and_then(|v| v.as_bool()) != Some(false)) {
        let is_oauth = c.get("authType").and_then(|v| v.as_str()) == Some("oauth");
        let matches_mode = mode == "all"
            || (mode == "oauth" && is_oauth)
            || (mode == "apikey" && !is_oauth);
        if !matches_mode {
            continue;
        }
        let Some(id) = c.get("id").and_then(|v| v.as_str()) else { continue };
        let t0 = std::time::Instant::now();
        let ok = match state.db.get_connection_full(id).await {
            Some(full) => {
                let api_key = full.get("apiKey").and_then(|v| v.as_str()).unwrap_or("");
                if api_key.is_empty() {
                    false
                } else {
                    let provider = full.get("provider").and_then(|v| v.as_str()).unwrap_or("");
                    let Some(r) = crate::snapshot::resolve(&format!("{provider}/__probe__")) else {
                        continue;
                    };
                    let base = r.transport.base_url.trim_end_matches('/').trim_end_matches("/chat/completions").to_string();
                    let http = state.client_for(state.db.resolve_connection_proxy(id).await.as_deref());
                    match http
                        .get(format!("{base}/models"))
                        .header("authorization", format!("Bearer {api_key}"))
                        .timeout(std::time::Duration::from_secs(15))
                        .send()
                        .await
                    {
                        Ok(resp) => resp.status().is_success(),
                        Err(_) => false,
                    }
                }
            }
            None => false,
        };
        results.push(json!({
            "connectionId": id,
            "connectionName": c.get("name").cloned().unwrap_or(json!("?")),
            "provider": c.get("provider").cloned().unwrap_or(json!("?")),
            "ok": ok,
            "latencyMs": t0.elapsed().as_millis() as u64,
        }));
    }
    (
        StatusCode::OK,
        Json(json!({ "mode": mode, "results": results })),
    )
        .into_response()
}

// ---- /api/usage (M7) -----------------------------------------------------

// ---- /api/proxy-pools -----------------------------------------------------

pub async fn proxy_pools_get(State(state): State<AppState>) -> impl IntoResponse {
    let pools = state.db.list_proxy_pools().await;
    (StatusCode::OK, Json(json!({ "pools": pools })))
}

pub async fn proxy_pools_post(State(state): State<AppState>, Json(mut body): Json<Value>) -> Response {
    // id in body = update (PUT semantics through POST create/update merge).
    match state.db.upsert_proxy_pool(std::mem::take(&mut body)).await {
        Ok(id) => match state.db.get_proxy_pool(&id).await {
            Some(pool) => (StatusCode::CREATED, Json(json!({ "pool": pool }))).into_response(),
            None => error_response(StatusCode::INTERNAL_SERVER_ERROR, "pool vanished after save"),
        },
        Err(e) => error_response(StatusCode::BAD_REQUEST, &e.to_string()),
    }
}

pub async fn proxy_pools_delete(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    if state.db.delete_proxy_pool(&id).await {
        (StatusCode::OK, Json(json!({ "success": true }))).into_response()
    } else {
        error_response(StatusCode::NOT_FOUND, "Pool not found")
    }
}

/// POST /api/proxy-pools/:id/test — verify the proxy answers by fetching an
/// IP echo endpoint through it; persists testStatus/lastError/lastTestedAt.
pub async fn proxy_pools_test(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    let Some(pool) = state.db.get_proxy_pool(&id).await else {
        return error_response(StatusCode::NOT_FOUND, "Pool not found");
    };
    let Some(proxy_url) = pool.get("proxyUrl").and_then(|v| v.as_str()).map(String::from) else {
        return error_response(StatusCode::BAD_REQUEST, "Pool has no proxyUrl");
    };

    let client = state.client_for(Some(&proxy_url));
    let result = client
        .get("https://api.ipify.org?format=json")
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await;
    let (ok, detail) = match result {
        Ok(r) => {
            let status = r.status();
            match r.json::<Value>().await {
                Ok(v) => (status.is_success(), v.get("ip").and_then(|x| x.as_str()).map(String::from)),
                Err(_) => (status.is_success(), None),
            }
        }
        Err(e) => (false, Some(format!("network error: {e}"))),
    };
    state
        .db
        .mark_proxy_pool_tested(&id, ok, if ok { detail.clone() } else { detail.clone().or(Some("unreachable".into())) })
        .await;

    (
        StatusCode::OK,
        Json(json!({
            "valid": ok,
            "exitIp": if ok { detail.clone().map(Value::String).unwrap_or(Value::Null) } else { Value::Null },
            "error": if ok { Value::Null } else { Value::String(detail.unwrap_or_else(|| "unreachable".into())) },
        })),
    )
        .into_response()
}

// ---- /api/usage/:connectionId — per-connection quota ----------------------

pub async fn connection_quota(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    let Some(conn) = state.db.get_connection_full(&id).await else {
        return error_response(StatusCode::NOT_FOUND, "Connection not found");
    };
    // Proxy-aware: quota probes follow the connection's pool/URL directive.
    let http = state.client_for(state.db.resolve_connection_proxy(&id).await.as_deref());
    let payload = crate::quota::fetch_quota(&state.db, &http, &conn).await;
    (StatusCode::OK, Json(payload)).into_response()
}

// ---- /api/console-logs — in-process tail ----------------------------------

pub async fn console_logs_get(
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    let limit = params
        .get("limit")
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(500);
    (StatusCode::OK, Json(json!({ "logs": crate::logs::recent(limit) })))
}

pub async fn console_logs_clear() -> impl IntoResponse {
    crate::logs::clear();
    (StatusCode::OK, Json(json!({ "success": true })))
}

/// GET /api/console-logs/stream — SSE: snapshot first, then live tail.
pub async fn console_logs_stream() -> impl IntoResponse {
    let rx = crate::logs::subscribe();
    let stream = async_stream::stream! {
        // Initial snapshot so a fresh page shows history immediately.
        for line in crate::logs::recent(300) {
            let payload = serde_json::to_string(&line).unwrap_or_default();
            yield Ok::<bytes::Bytes, std::io::Error>(bytes::Bytes::from(format!("data: {payload}\n\n")));
        }
        let mut rx = rx;
        loop {
            match rx.recv().await {
                Ok(line) => {
                    let payload = serde_json::to_string(&line.to_json()).unwrap_or_default();
                    yield Ok(bytes::Bytes::from(format!("data: {payload}\n\n")));
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(_) => break,
            }
        }
    };
    let mut resp = Response::new(Body::from_stream(stream));
    *resp.status_mut() = StatusCode::OK;
    resp.headers_mut()
        .insert("content-type", HeaderValue::from_static("text/event-stream"));
    resp.headers_mut()
        .insert("cache-control", HeaderValue::from_static("no-cache"));
    resp
}

// ---- /api/cli-tools (M7 stub kept above) ----------------------------------

pub async fn usage_logs(State(state): State<AppState>) -> impl IntoResponse {
    let logs = state.db.recent_logs(200).await;
    (StatusCode::OK, Json(logs))
}

pub async fn usage_stats(
    State(state): State<AppState>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    let period = params.get("period").map(|s| s.as_str()).unwrap_or("7d");
    let stats = state.db.usage_stats(period).await;
    (StatusCode::OK, Json(stats))
}

/// GET /api/usage/chart?period= — per-day series for the usage bar chart.
pub async fn usage_chart(
    State(state): State<AppState>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    let period = params.get("period").map(|s| s.as_str()).unwrap_or("7d");
    let chart = state.db.usage_chart(period).await;
    (StatusCode::OK, Json(chart))
}

/// GET /api/models — rich dashboard catalog (name/kind/native/pricing per
/// provider) from the embedded snapshot. Read-only, no secrets.
pub async fn models_catalog() -> impl IntoResponse {
    let body = crate::snapshot::dashboard_model_catalog();
    (StatusCode::OK, Json(body))
}

// ---- /api/combos ---------------------------------------------------------

pub async fn combos_get(State(state): State<AppState>) -> impl IntoResponse {
    let combos = state.db.list_combos().await;
    (StatusCode::OK, Json(json!({ "combos": combos })))
}

pub async fn combos_post(State(state): State<AppState>, Json(body): Json<Value>) -> Response {
    let name = body
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim();
    if name.is_empty() {
        return error_response(StatusCode::BAD_REQUEST, "Name is required");
    }
    let kind = body.get("kind").and_then(|v| v.as_str());
    let models = body.get("models").cloned().unwrap_or_else(|| json!([]));

    match state.db.create_combo(name, kind, models).await {
        Ok(combo) => (StatusCode::CREATED, Json(combo)).into_response(),
        Err(e) => error_response(StatusCode::BAD_REQUEST, &e.to_string()),
    }
}

pub async fn combos_delete(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    if state.db.delete_combo(&id).await {
        (StatusCode::OK, Json(json!({ "success": true }))).into_response()
    } else {
        error_response(StatusCode::NOT_FOUND, "Combo not found")
    }
}

// ---- /api/cli-tools ------------------------------------------------------

pub async fn cli_tools_get() -> impl IntoResponse {
    let tools = json!([
        { "id": "claude", "name": "Claude Code", "description": "Anthropic CLI agent" },
        { "id": "cursor", "name": "Cursor / Windsurf", "description": "AI IDE integration" },
        { "id": "kiro", "name": "Kiro CLI", "description": "Kiro AI agent" },
        { "id": "antigravity", "name": "Google Antigravity", "description": "Google AI IDE" },
        { "id": "cowork", "name": "Cowork Tools", "description": "Collaborative CLI suite" }
    ]);
    (StatusCode::OK, Json(json!({ "tools": tools })))
}

fn error_response(status: StatusCode, message: &str) -> Response {
    (status, Json(json!({ "error": message }))).into_response()
}

fn iso_now() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let (y, mo, d, h, mi, s) = crate::db::unix_to_civil(secs);
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{s:02}.000Z")
}
