//! Native per-provider live quota probing — port of open-sse/services/usage/*.
//! Supported natively: openrouter, claude, codex, antigravity, glm/glm-cn.
//! Shape returned to the UI:
//!   { available: true, provider, plan?, quotas: [ { name, remainingPct,
//!     usedPct, resetAt?, displayName?, usedDollars? } ], message? }

use serde_json::{json, Value};

use crate::db::Db;

fn pct_window(name: &str, used_pct: f64, reset_at: Option<Value>) -> Value {
    let used = used_pct.clamp(0.0, 100.0);
    json!({
        "name": name,
        "usedPct": used,
        "remainingPct": 100.0 - used,
        "total": 100,
        "resetAt": reset_at,
    })
}

fn parse_reset(v: Value) -> Option<Value> {
    let s = v.as_str().map(String::from).or_else(|| v.as_i64().map(|n| n.to_string()))?;
    Some(Value::String(s))
}

// ---- claude ----------------------------------------------------------------

async fn claude_usage(http: &reqwest::Client, token: &str) -> Value {
    let resp = http
        .get("https://api.anthropic.com/api/oauth/usage")
        .header("authorization", format!("Bearer {token}"))
        .header("anthropic-beta", "oauth-2025-04-20")
        .header("anthropic-version", "2023-06-01")
        .timeout(Duration::from_secs(15))
        .send()
        .await;
    let Ok(resp) = resp else {
        return json!({ "available": false, "message": "Claude connected. Usage endpoint unreachable." });
    };
    if resp.status() == 429 {
        return json!({ "available": false, "message": "Claude usage API rate-limited (429) — try again in a few minutes." });
    }
    if !resp.status().is_success() {
        return json!({ "available": false, "message": format!("Claude connected. Usage API returned HTTP {}.", resp.status()) });
    }
    let Ok(data) = resp.json::<Value>().await else {
        return json!({ "available": false, "message": "Claude usage response was not JSON." });
    };

    let mut quotas: Vec<Value> = Vec::new();
    let mut push_window = |name: &str, w: &Value| {
        if let Some(util) = w.get("utilization").and_then(|v| v.as_f64()) {
            let reset = w.get("resets_at").cloned().unwrap_or(Value::Null);
            quotas.push(pct_window(name, util, parse_reset(reset)));
        }
    };
    if let Some(obj) = data.as_object() {
        if let Some(w) = obj.get("five_hour") {
            push_window("session (5h)", w);
        }
        if let Some(w) = obj.get("seven_day") {
            push_window("weekly (7d)", w);
        }
        for (k, v) in obj {
            if k.starts_with("seven_day_") && k != "seven_day" {
                push_window(&format!("weekly {} (7d)", k.trim_start_matches("seven_day_")), v);
            }
        }
    }
    if quotas.is_empty() {
        return json!({ "available": false, "message": "Claude connected. No quota windows returned (may require admin access)." });
    }
    json!({
        "available": true,
        "plan": "Claude Code",
        "quotas": quotas,
        "extraUsage": data.get("extra_usage").cloned().unwrap_or(Value::Null),
    })
}

// ---- codex -----------------------------------------------------------------

async fn codex_usage(http: &reqwest::Client, token: &str, psd: &Value) -> Value {
    let mut req = http
        .get("https://chatgpt.com/backend-api/wham/usage")
        .header("authorization", format!("Bearer {token}"))
        .header("accept", "application/json");
    let account_id = psd
        .get("workspaceId")
        .or_else(|| psd.get("accountId"))
        .or_else(|| psd.get("chatgptAccountId"))
        .and_then(|v| v.as_str())
        .map(String::from);
    if let Some(id) = &account_id {
        req = req.header("chatgpt-account-id", id);
    }
    let Ok(resp) = req.timeout(Duration::from_secs(15)).send().await else {
        return json!({ "available": false, "message": "Codex connected. Usage endpoint unreachable." });
    };
    if !resp.status().is_success() {
        return json!({ "available": false, "message": format!("Codex connected. Usage API temporarily unavailable ({}).", resp.status()) });
    }
    let Ok(data) = resp.json::<Value>().await else {
        return json!({ "available": false, "message": "Codex usage response was not JSON." });
    };

    let mut quotas: Vec<Value> = Vec::new();
    let rate_limit = data
        .get("rate_limit")
        .or_else(|| data.get("rate_limits"))
        .cloned()
        .unwrap_or_else(|| Value::Object(Default::default()));

    let window_quota = |prefix: &str, w: &Value| -> Option<Value> {
        let used = w
            .get("used_percent")
            .or_else(|| w.get("percent_used"))
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0)
            .clamp(0.0, 100.0);
        let reset = w
            .get("reset_at")
            .or_else(|| w.get("resets_at"))
            .or_else(|| w.get("resetAt"))
            .cloned()
            .unwrap_or(Value::Null);
        Some(pct_window(prefix, used, parse_reset(reset)))
    };

    for (keys, name) in [
        (["primary_window", "primary"], "session"),
        (["secondary_window", "secondary"], "weekly"),
    ] {
        let w = keys
            .iter()
            .find_map(|k| rate_limit.get(k))
            .or_else(|| data.get(keys[0]))
            .or_else(|| data.get(keys[1]));
        if let Some(w) = w {
            if let Some(q) = window_quota(name, w) {
                quotas.push(q);
            }
        }
    }
    // Review rate limit (optional block).
    if let Some(review) = data
        .get("code_review_rate_limit")
        .or_else(|| data.get("review_rate_limit"))
    {
        if let Some(q) = window_quota("review", review) {
            quotas.push(q);
        }
    }

    json!({
        "available": true,
        "plan": data.get("plan_type").and_then(|v| v.as_str()).unwrap_or("unknown"),
        "quotas": quotas,
        "resetCredits": data.pointer("/rate_limit_reset_credits/available_count").cloned().unwrap_or(Value::Null),
    })
}

// ---- antigravity -----------------------------------------------------------

async fn antigravity_usage(http: &reqwest::Client, token: &str) -> Value {
    let platform = if cfg!(windows) {
        5
    } else if cfg!(target_os = "macos") {
        2 // arm assumed; matches Node's darwin arm64/intel split coarsely
    } else if cfg!(target_os = "linux") {
        3
    } else {
        0
    };
    let ua = "antigravity/ide/2.1.1 windows/x86_64";
    let auth = format!("Bearer {token}");

    // 1) loadCodeAssist → projectId + plan tier
    let mut project: Option<String> = None;
    let mut plan = String::from("Unknown");
    let load = http
        .post("https://cloudcode-pa.googleapis.com/v1internal:loadCodeAssist")
        .header("authorization", &auth)
        .header("user-agent", ua)
        .header("content-type", "application/json")
        .json(&json!({ "metadata": { "ideType": 9, "platform": platform, "pluginType": 2 }, "mode": 1 }))
        .timeout(Duration::from_secs(12))
        .send()
        .await;
    if let Ok(r) = load {
        if r.status().is_success() {
            if let Ok(v) = r.json::<Value>().await {
                project = v
                    .get("cloudaicompanionProject")
                    .and_then(|p| p.as_str())
                    .map(String::from);
                plan = v
                    .pointer("/currentTier/name")
                    .and_then(|t| t.as_str())
                    .unwrap_or("Free")
                    .to_string();
            }
        }
    }

    // 2) fetchAvailableModels → per-model quota fractions
    let mut req = http
        .post("https://daily-cloudcode-pa.googleapis.com/v1internal:fetchAvailableModels")
        .header("authorization", &auth)
        .header("user-agent", ua)
        .header("content-type", "application/json")
        .header("x-client-name", "antigravity")
        .header("x-client-version", "2.1.1");
    let body = match &project {
        Some(p) => json!({ "project": p }),
        None => json!({}),
    };
    req = req.json(&body);
    let Ok(resp) = req.timeout(Duration::from_secs(12)).send().await else {
        return json!({ "available": false, "message": "Antigravity connected. Quota endpoint unreachable." });
    };
    if resp.status() == 401 {
        return json!({ "available": false, "message": "Antigravity quota API authentication expired. Refresh the token or re-login." });
    }
    if resp.status() == 403 {
        return json!({ "available": false, "message": "Antigravity quota API access forbidden. Chat may still work." });
    }
    if !resp.status().is_success() {
        return json!({ "available": false, "message": format!("Antigravity API error: {}.", resp.status()) });
    }
    let Ok(data) = resp.json::<Value>().await else {
        return json!({ "available": false, "message": "Antigravity quota response was not JSON." });
    };

    let mut quotas: Vec<Value> = Vec::new();
    if let Some(models) = data.get("models").and_then(|m| m.as_object()) {
        for (model_key, info) in models {
            let Some(qi) = info.get("quotaInfo") else { continue };
            if info.get("isInternal").and_then(|v| v.as_bool()).unwrap_or(false) {
                continue;
            }
            let fraction = qi.get("remainingFraction").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let remaining_pct = fraction * 100.0;
            quotas.push(json!({
                "name": model_key,
                "displayName": info.get("displayName").cloned().unwrap_or(Value::Null),
                "remainingPct": remaining_pct,
                "usedPct": 100.0 - remaining_pct,
                "total": 1000,
                "resetAt": parse_reset(qi.get("resetTime").cloned().unwrap_or(Value::Null)),
            }));
        }
    }
    quotas.sort_by(|a, b| {
        a.get("remainingPct")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0)
            .partial_cmp(&b.get("remainingPct").and_then(|v| v.as_f64()).unwrap_or(0.0))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    json!({ "available": true, "plan": plan, "quotas": quotas })
}

// ---- glm / glm-cn ----------------------------------------------------------

async fn glm_usage(http: &reqwest::Client, api_key: &str, provider: &str) -> Value {
    let url = if provider == "glm-cn" {
        "https://open.bigmodel.cn/api/monitor/usage/quota/limit"
    } else {
        "https://api.z.ai/api/monitor/usage/quota/limit"
    };
    let Ok(resp) = http
        .get(url)
        .header("authorization", format!("Bearer {api_key}"))
        .header("accept", "application/json")
        .timeout(Duration::from_secs(15))
        .send()
        .await
    else {
        return json!({ "available": false, "message": "GLM quota endpoint unreachable." });
    };
    if resp.status() == 401 {
        return json!({ "available": false, "message": "GLM API key invalid or expired." });
    }
    if !resp.status().is_success() {
        return json!({ "available": false, "message": format!("GLM quota API error ({}).", resp.status()) });
    }
    let Ok(json_body) = resp.json::<Value>().await else {
        return json!({ "available": false, "message": "GLM quota response was not JSON." });
    };
    let mut quotas: Vec<Value> = Vec::new();
    if let Some(limits) = json_body.pointer("/data/limits").and_then(|l| l.as_array()) {
        for limit in limits {
            if limit.get("type").and_then(|t| t.as_str()) != Some("TOKENS_LIMIT") {
                continue;
            }
            let used = limit.get("percentage").and_then(|p| p.as_f64()).unwrap_or(0.0);
            let reset_ms = limit.get("nextResetTime").and_then(|t| t.as_i64()).unwrap_or(0);
            quotas.push(pct_window("session", used, if reset_ms > 0 {
                parse_reset(Value::Number(reset_ms.into()))
            } else {
                None
            }));
        }
    }
    let plan = json_body
        .pointer("/data/level")
        .and_then(|l| l.as_str())
        .unwrap_or("unknown")
        .to_string();
    json!({ "available": true, "plan": plan, "quotas": quotas })
}

// ---- dispatcher ------------------------------------------------------------

/// Native quota for a connection. Returns the UI-ready payload.
pub async fn fetch_quota(db: &Db, http: &reqwest::Client, conn: &Value) -> Value {
    let provider = conn.get("provider").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let id = conn.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();

    // OAuth connections: refresh first when the token is at/near expiry.
    let mut conn = conn.clone();
    if conn.get("authType").and_then(|v| v.as_str()) == Some("oauth") {
        let expires_soon = conn
            .get("expiresAt")
            .and_then(|v| v.as_str())
            .and_then(crate::db::parse_rfc3339_secs_pub)
            .map(|exp| exp <= crate::db::chrono_now_secs_pub() + 600)
            .unwrap_or(false);
        if expires_soon && conn.get("refreshToken").and_then(|v| v.as_str()).is_some_and(|s| !s.is_empty()) {
            if let Ok(_) = crate::oauth::refresh(db, http, &id).await {
                if let Some(fresh) = db.get_connection_full(&id).await {
                    conn = fresh;
                }
            }
        }
    }

    let token = conn
        .get("accessToken")
        .and_then(|v| v.as_str())
        .map(String::from);
    let api_key = conn
        .get("apiKey")
        .and_then(|v| v.as_str())
        .map(String::from);
    let psd = conn.get("providerSpecificData").cloned().unwrap_or(json!({}));

    let mut payload = match provider.as_str() {
        "openrouter" => {
            let Some(key) = api_key.clone() else {
                return json!({ "available": false, "provider": provider, "message": "no API key stored" });
            };
            let resp = http
                .get("https://openrouter.ai/api/v1/auth/key")
                .header("authorization", format!("Bearer {key}"))
                .timeout(Duration::from_secs(15))
                .send()
                .await;
            match resp {
                Ok(r) if r.status().is_success() => {
                    let d = r.json::<Value>().await.ok().and_then(|v| v.get("data").cloned()).unwrap_or(json!({}));
                    json!({
                        "available": true,
                        "plan": if d.get("isFreeTier").and_then(|v| v.as_bool()).unwrap_or(false) { "Free tier" } else { "Paid" },
                        "dollars": {
                            "label": d.get("label").cloned().unwrap_or(Value::Null),
                            "limit": d.get("limit").cloned().unwrap_or(Value::Null),
                            "usage": d.get("usage").cloned().unwrap_or(Value::Null),
                            "limitRemaining": d.get("limit_remaining").cloned().unwrap_or(Value::Null),
                            "isFreeTier": d.get("is_free_tier").cloned().unwrap_or(Value::Null),
                        },
                        "quotas": [],
                    })
                }
                Ok(r) => json!({ "available": false, "message": format!("upstream HTTP {}.", r.status()) }),
                Err(e) => json!({ "available": false, "message": format!("network error: {e}") }),
            }
        }
        "claude" => match token.as_deref() {
            Some(t) => claude_usage(http, t).await,
            None => json!({ "available": false, "message": "no access token stored — re-login via OAuth" }),
        },
        "codex" => match token.as_deref() {
            Some(t) => codex_usage(http, t, &psd).await,
            None => json!({ "available": false, "message": "no access token stored — re-login via OAuth" }),
        },
        "antigravity" => match token.as_deref() {
            Some(t) => antigravity_usage(http, t).await,
            None => json!({ "available": false, "message": "no access token stored — re-login via OAuth" }),
        },
        "glm" | "glm-cn" => match api_key.as_deref() {
            Some(k) if !k.is_empty() => glm_usage(http, k, &provider).await,
            _ => json!({ "available": false, "message": "GLM API key not available." }),
        },
        other => {
            return json!({
                "available": false,
                "provider": other,
                "message": format!("live quota for '{other}' is not implemented natively yet"),
            });
        }
    };
    if let Some(obj) = payload.as_object_mut() {
        obj.insert("provider".into(), Value::String(provider));
        obj.insert("connectionId".into(), Value::String(id));
    }
    payload
}

use std::time::Duration;
