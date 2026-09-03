//! Native Antigravity chat — Google Cloud Code v1internal generateContent.
//! Port of the request shape from open-sse/executors/antigravity.js:
//!   POST {apiEndpoint}/v1internal:streamGenerateContent?alt=sse   (stream)
//!   POST {apiEndpoint}/v1internal:generateContent                  (else)
//! Auth: OAuth bearer (refreshed by oauth::refresh when expired).
//! Project resolution: loadCodeAssist → cloudaicompanionProject.
//!
//! Wire format: Gemini generateContent JSON. Streaming uses SSE with
//! `data: {...}` lines carrying candidates.

use std::time::Duration;

use reqwest::Client;
use serde_json::{json, Value};
use uuid::Uuid;

const API_ENDPOINT: &str = "https://daily-cloudcode-pa.googleapis.com";
const IDE_VERSION: &str = "2.1.1";
const USER_AGENT: &str = "antigravity/ide/2.1.1 windows/x86_64";

fn platform_enum() -> i64 {
    if cfg!(windows) { 5 }
    else if cfg!(target_os = "macos") { 2 }
    else if cfg!(target_os = "linux") { 3 }
    else { 0 }
}

fn client_metadata() -> Value {
    json!({ "ideType": 9, "platform": platform_enum(), "pluginType": 2 })
}

/// Resolve the Cloud Code project for this account (loadCodeAssist).
pub async fn resolve_project(http: &Client, token: &str) -> Option<String> {
    let resp = http
        .post(format!("{API_ENDPOINT}/v1internal:loadCodeAssist"))
        .header("authorization", format!("Bearer {token}"))
        .header("user-agent", USER_AGENT)
        .header("content-type", "application/json")
        .json(&json!({ "metadata": client_metadata(), "mode": 1 }))
        .timeout(Duration::from_secs(15))
        .send()
        .await
        .ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let v: Value = resp.json().await.ok()?;
    v.get("cloudaicompanionProject")
        .and_then(|p| p.as_str())
        .map(String::from)
}

/// Chat: build + POST the generateContent request. Returns (status, content_type, body)
/// so callers can relay in OpenAI-compatible or raw form.
pub async fn generate(
    http: &Client,
    token: &str,
    model: &str,
    connection_id: &str,
    contents: &Value,
    system: Option<&str>,
    max_tokens: Option<i64>,
    temperature: Option<f64>,
    stream: bool,
) -> Result<(reqwest::StatusCode, String, bytes::Bytes), String> {
    let session_id = format!("orouter-{}", connection_id);
    let project = resolve_project(http, token)
        .await
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    // Cloud Code v1internal generateContent: Gemini format inside `request`.
    let mut gemini_request = json!({ "contents": contents });
    if let Some(sys) = system {
        if !sys.is_empty() {
            gemini_request["systemInstruction"] = json!({ "parts": [{ "text": sys }] });
        }
    }
    let mut gc = json!({});
    if let Some(mt) = max_tokens {
        gc["maxOutputTokens"] = json!(mt);
    }
    if let Some(t) = temperature {
        gc["temperature"] = json!(t);
    }
    gemini_request["generationConfig"] = gc;
    gemini_request["sessionId"] = json!(session_id);
    // project goes at top level, NOT inside request

    // sessionId and requestId required by Cloud Code API (antigravity parity)
    let request_id = Uuid::new_v4().to_string();

    let body = json!({
        "model": model,
        "project": project,
        "userAgent": "antigravity",
        "requestType": "agent",
        "request": gemini_request,
    });

    let action = if stream { "streamGenerateContent?alt=sse" } else { "generateContent" };
    let url = format!("{API_ENDPOINT}/v1internal:{action}");
    let resp = http
        .post(&url)
        .header("authorization", format!("Bearer {token}"))
        .header("user-agent", USER_AGENT)
        .header("content-type", "application/json")
        .header("x-client-name", "antigravity")
        .header("x-client-version", IDE_VERSION)
        .body(body.to_string())
        .timeout(Duration::from_secs(if stream { 300 } else { 120 }))
        .send()
        .await
        .map_err(|e| format!("upstream unreachable: {e}"))?;
    let status = resp.status();
    let ct = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("application/json")
        .to_string();
    let bytes = resp.bytes().await.map_err(|e| format!("body read failed: {e}"))?;
    Ok((status, ct, bytes))
}

/// Quota probe (fetchAvailableModels) — same call the Quota Tracker uses.
pub async fn quota(http: &Client, token: &str, connection_id: &str) -> Value {
    let project = resolve_project(http, token)
        .await
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    let resp = http
        .post(format!("{API_ENDPOINT}/v1internal:fetchAvailableModels"))
        .header("authorization", format!("Bearer {token}"))
        .header("user-agent", USER_AGENT)
        .header("content-type", "application/json")
        .header("x-client-name", "antigravity")
        .header("x-client-version", IDE_VERSION)
        .json(&json!({ "project": project }))
        .timeout(Duration::from_secs(15))
        .send()
        .await;
    match resp {
        Ok(r) if r.status().is_success() => {
            let v: Value = r.json().await.unwrap_or(json!({}));
            let mut quotas = serde_json::Map::new();
            if let Some(models) = v.get("models").and_then(|m| m.as_object()) {
                for (key, info) in models {
                    if let Some(qi) = info.get("quotaInfo") {
                        let fraction = qi.get("remainingFraction").and_then(|f| f.as_f64()).unwrap_or(0.0);
                        quotas.insert(
                            key.clone(),
                            json!({ "remainingPct": fraction * 100.0 }),
                        );
                    }
                }
            }
            json!({ "available": true, "provider": "antigravity", "connectionId": connection_id, "quotas": quotas })
        }
        Ok(r) => json!({ "available": false, "provider": "antigravity", "connectionId": connection_id, "error": format!("HTTP {}", r.status().as_u16()) }),
        Err(e) => json!({ "available": false, "error": format!("network error: {e}") }),
    }
}
