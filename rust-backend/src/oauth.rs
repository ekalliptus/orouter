//! Native OAuth (PKCE S256) for the providers the dashboard offers
//! interactive login for: claude, codex, antigravity.
//!
//! Flow mirrors src/lib/oauth/services/*.js of the Node engine:
//!   start    → build authorize URL (state + code_challenge), keep session
//!   exchange → paste code/callback URL → token endpoint → connection row
//!   refresh  → refresh_token grant → update stored tokens
//!
//! Inference for these OAuth providers still requires the Node engine
//! (format translation); this module manages credentials only.
//!
//! Credentials policy: no client secrets live in source. Antigravity's
//! Google client secret is read from `ANTIGRAVITY_OAUTH_CLIENT_SECRET`
//! (the same value the Node engine embeds in its own registry file).

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use once_cell::sync::Lazy;
use rand::rngs::OsRng;
use rand::RngCore;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::db::Db;

#[derive(Debug, Clone)]
pub struct ProviderCfg {
    pub authorize_url: &'static str,
    pub token_url: &'static str,
    pub client_id: &'static str,
    pub client_secret: String,
    pub scope: &'static str,
    pub redirect_uri: &'static str,
    /// JSON body for token endpoints (claude/codex); google wants form-encoded.
    pub token_encoding: TokenEncoding,
    /// Extra authorize params (codex CLI parity).
    pub extra_authorize: &'static [(&'static str, &'static str)],
    /// Extra refresh params (codex re-sends scope).
    pub refresh_form_extra: &'static [(&'static str, &'static str)],
    pub refresh_encoding: TokenEncoding,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TokenEncoding {
    Json,
    Form,
}

/// Antigravity's Google client secret: env first, then fall back to reading
/// the Node engine's own registry file (same repo, already ships the value).
/// Never hardcoded here.
fn antigravity_secret() -> String {
    if let Ok(s) = std::env::var("ANTIGRAVITY_OAUTH_CLIENT_SECRET") {
        if !s.trim().is_empty() {
            return s;
        }
    }
    for cand in [
        "../open-sse/providers/registry/antigravity.js",
        "open-sse/providers/registry/antigravity.js",
    ] {
        if let Ok(text) = std::fs::read_to_string(cand) {
            if let Some(pos) = text.find("clientSecret:") {
                let rest = text[pos + "clientSecret:".len()..].trim_start();
                let rest = rest.trim_start_matches(['"', '\'']);
                if let Some(end) = rest.find(['"', '\'']) {
                    return rest[..end].to_string();
                }
            }
        }
    }
    String::new()
}

pub fn cfg(provider: &str) -> Option<ProviderCfg> {
    match provider {
        "claude" => Some(ProviderCfg {
            authorize_url: "https://claude.ai/oauth/authorize",
            token_url: "https://api.anthropic.com/v1/oauth/token",
            client_id: "9d1c250a-e61b-44d9-88ed-5944d1962f5e",
            client_secret: String::new(),
            scope: "org:create_api_key user:profile user:inference",
            // Registered callback: the browser shows a code to copy.
            redirect_uri: "https://console.anthropic.ai/oauth/code_callback",
            token_encoding: TokenEncoding::Json,
            extra_authorize: &[],
            refresh_form_extra: &[],
            refresh_encoding: TokenEncoding::Json,
        }),
        "codex" => Some(ProviderCfg {
            authorize_url: "https://auth.openai.com/oauth/authorize",
            token_url: "https://auth.openai.com/oauth/token",
            client_id: "app_EMoamEEZ73f0CkXaXp7hrann",
            client_secret: String::new(),
            scope: "openid profile email offline_access",
            // Fixed port like the real Codex CLI; the browser will fail to
            // load (nothing listens) — the user pastes the callback URL.
            redirect_uri: "http://localhost:1455/auth/callback",
            token_encoding: TokenEncoding::Json,
            extra_authorize: &[
                ("id_token_add_organizations", "true"),
                ("codex_cli_simplified_flow", "true"),
                ("originator", "codex_cli_rs"),
            ],
            refresh_form_extra: &[("scope", "openid profile email offline_access")],
            refresh_encoding: TokenEncoding::Form,
        }),
        "antigravity" => Some(ProviderCfg {
            authorize_url: "https://accounts.google.com/o/oauth2/v2/auth",
            token_url: "https://oauth2.googleapis.com/token",
            client_id: "1071006060591-tmhssin2h21lcre235vtolojh4g403ep.apps.googleusercontent.com",
            // From env, or read from the Node engine's registry file — never
            // hardcoded here (see antigravity_secret).
            client_secret: antigravity_secret(),
            scope: "https://www.googleapis.com/auth/cloud-platform https://www.googleapis.com/auth/userinfo.email https://www.googleapis.com/auth/userinfo.profile https://www.googleapis.com/auth/cclog https://www.googleapis.com/auth/experimentsandconfigs",
            // Google loopback: any port is allowed for installed clients.
            redirect_uri: "http://localhost:51121/callback",
            token_encoding: TokenEncoding::Form,
            extra_authorize: &[("access_type", "offline"), ("prompt", "consent")],
            refresh_form_extra: &[],
            refresh_encoding: TokenEncoding::Form,
        }),
        _ => None,
    }
}

pub fn supported() -> &'static [&'static str] {
    &["claude", "codex", "antigravity"]
}

// ---- PKCE helpers ----------------------------------------------------------

fn b64url(bytes: &[u8]) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | (b[2] as u32);
        out.push(CHARS[(n >> 18) as usize & 63] as char);
        out.push(CHARS[(n >> 12) as usize & 63] as char);
        if chunk.len() > 1 {
            out.push(CHARS[(n >> 6) as usize & 63] as char);
        }
        if chunk.len() > 2 {
            out.push(CHARS[n as usize & 63] as char);
        }
    }
    out
}

/// CSPRNG (OS entropy) — state and verifiers are security material.
fn random_b64url(bytes: usize) -> String {
    let mut buf = vec![0u8; bytes];
    OsRng.fill_bytes(&mut buf);
    b64url(&buf)
}

fn s256_challenge(verifier: &str) -> String {
    b64url(&Sha256::digest(verifier.as_bytes()))
}

// ---- Pending sessions ------------------------------------------------------

#[derive(Debug, Clone)]
struct Pending {
    provider: String,
    verifier: String,
    redirect_uri: String,
    created: SystemTime,
}

static PENDING: Lazy<Mutex<HashMap<String, Pending>>> = Lazy::new(|| Mutex::new(HashMap::new()));

const PENDING_TTL: Duration = Duration::from_secs(600);

// ---- Public API ------------------------------------------------------------

/// Build the authorize URL for a provider. Returns (auth_url, state).
pub fn start(provider: &str) -> Option<(String, String)> {
    let cfg = cfg(provider)?;
    let state = random_b64url(32);
    let verifier = random_b64url(32);
    let challenge = s256_challenge(&verifier);

    let mut url = format!(
        "{}?response_type=code&client_id={}&redirect_uri={}&scope={}&code_challenge={}&code_challenge_method=S256&state={}",
        cfg.authorize_url,
        urlencode(cfg.client_id),
        urlencode(cfg.redirect_uri),
        urlencode(cfg.scope),
        urlencode(&challenge),
        urlencode(&state),
    );
    for (k, v) in cfg.extra_authorize {
        url.push_str(&format!("&{}={}", k, urlencode(v)));
    }
    PENDING.lock().ok()?.insert(
        state.clone(),
        Pending {
            provider: provider.to_string(),
            verifier,
            redirect_uri: cfg.redirect_uri.to_string(),
            created: SystemTime::now(),
        },
    );
    Some((url, state))
}

/// Exchange a pasted code (or full callback URL) for tokens and store them in
/// a provider connection. Returns the safe connection JSON.
pub async fn exchange(
    db: &Db,
    http: &reqwest::Client,
    state_in: &str,
    code_input: &str,
    connection_id: Option<String>,
) -> Result<Value, String> {
    // Pull + expire the session.
    let pending = {
        let mut map = PENDING
            .lock()
            .map_err(|_| "session lock poisoned".to_string())?;
        map.remove(state_in)
            .filter(|p| p.created.elapsed().unwrap_or(PENDING_TTL) < PENDING_TTL)
            .ok_or_else(|| "unknown or expired state — start the login again".to_string())?
    };

    // Accept a raw code or a full callback URL (codex/google paste flow).
    let (code, code_state) = extract_code_and_state(code_input);

    let cfg = cfg(&pending.provider).ok_or("unsupported provider")?;
    if !cfg.client_secret.is_empty() {
        // Env-provided secret must actually be set when the flow needs one.
    }
    let body: Value = match cfg.token_encoding {
        TokenEncoding::Json => json!({
            "grant_type": "authorization_code",
            "code": code,
            "state": if code_state.is_empty() { state_in } else { &code_state },
            "client_id": cfg.client_id,
            "redirect_uri": pending.redirect_uri,
            "code_verifier": pending.verifier,
        }),
        TokenEncoding::Form => {
            let mut form = vec![
                ("grant_type", "authorization_code".to_string()),
                ("code", code.clone()),
                ("redirect_uri", pending.redirect_uri.clone()),
                ("client_id", cfg.client_id.to_string()),
                ("client_secret", cfg.client_secret.clone()),
                ("code_verifier", pending.verifier.clone()),
            ];
            if !code_state.is_empty() {
                form.push(("state", code_state.clone()));
            }
            let pairs: Vec<String> = form.iter().map(|(k, v)| format!("{k}={}", urlencode(v))).collect();
            json!({ "__form__": pairs.join("&") })
        }
    };

    let tokens = post_tokens(http, cfg.token_url, &body, cfg.token_encoding).await?;
    let mapped = map_tokens(&tokens)?;
    let expires_at = mapped.expires_at.clone();

    let connection_id = match connection_id.filter(|s| !s.trim().is_empty()) {
        Some(id) => {
            db.update_connection_tokens(&id, &mapped.access, &mapped.refresh, &expires_at, &mapped.scope)
                .await;
            id
        }
        None => {
            let name = format!("{} account", pending.provider);
            db.create_oauth_connection(&pending.provider, &name, &mapped.access, &mapped.refresh, &expires_at, &mapped.scope)
                .await
                .map_err(|e| format!("failed to save connection: {e}"))?
        }
    };

    let conn = db.get_connection_full(&connection_id).await.unwrap_or(json!({}));
    Ok(redact(conn))
}

/// Refresh a connection's tokens via the refresh_token grant.
pub async fn refresh(
    db: &Db,
    http: &reqwest::Client,
    connection_id: &str,
) -> Result<Value, String> {
    let conn = db
        .get_connection_full(connection_id)
        .await
        .ok_or("connection not found")?;
    let provider = conn
        .get("provider")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let refresh_token = conn
        .get("refreshToken")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .ok_or("connection has no refresh token")?
        .to_string();
    let cfg = cfg(&provider).ok_or_else(|| format!("provider {provider} has no native OAuth"))?;

    let body: Value = match cfg.refresh_encoding {
        TokenEncoding::Json => json!({
            "grant_type": "refresh_token",
            "refresh_token": refresh_token,
            "client_id": cfg.client_id,
        }),
        TokenEncoding::Form => {
            let mut form = vec![
                ("grant_type", "refresh_token".to_string()),
                ("refresh_token", refresh_token.clone()),
                ("client_id", cfg.client_id.to_string()),
            ];
            for (k, v) in cfg.refresh_form_extra {
                form.push((k, v.to_string()));
            }
            if !cfg.client_secret.is_empty() {
                form.push(("client_secret", cfg.client_secret.clone()));
            }
            let pairs: Vec<String> = form.iter().map(|(k, v)| format!("{k}={}", urlencode(v))).collect();
            json!({ "__form__": pairs.join("&") })
        }
    };

    let tokens = post_tokens(http, cfg.token_url, &body, cfg.refresh_encoding).await?;
    let mapped = map_tokens(&tokens)?;
    let refresh = if mapped.refresh.is_empty() { refresh_token } else { mapped.refresh };
    let expires_at = mapped.expires_at.clone();
    db.update_connection_tokens(connection_id, &mapped.access, &refresh, &expires_at, &mapped.scope)
        .await;

    Ok(json!({
        "ok": true,
        "connectionId": connection_id,
        "expiresAt": expires_at,
        "scope": mapped.scope,
    }))
}

/// Background loop: refresh OAuth tokens expiring within the next 6 hours so
/// hybrid-mode inference always sees valid credentials. Runs every 15 min.
pub async fn auto_refresh_loop(db: Db, http: reqwest::Client) {
    let mut interval = tokio::time::interval(Duration::from_secs(15 * 60));
    interval.tick().await; // skip the immediate tick at boot
    loop {
        interval.tick().await;
        let candidates = db.oauth_refresh_candidates(6 * 3600).await;
        for (id, provider) in candidates {
            match refresh(&db, &http, &id).await {
                Ok(v) => {
                    let exp = v.get("expiresAt").and_then(|x| x.as_str()).unwrap_or("?");
                    tracing::info!(connection = %id, provider = %provider, "auto-refreshed OAuth token (expires {exp})");
                }
                Err(e) => {
                    tracing::warn!(connection = %id, provider = %provider, "auto-refresh failed: {e}");
                }
            }
        }
    }
}

/// Expiry info for the status badge.
pub fn token_status(conn: &Value) -> Value {
    let Some(exp) = conn.get("expiresAt").and_then(|v| v.as_str()) else {
        return json!({ "hasToken": false });
    };
    let secs = crate::db::parse_rfc3339_secs_pub(exp).unwrap_or(0);
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0) as i64;
    json!({
        "hasToken": true,
        "expiresAt": exp,
        "expired": secs > 0 && secs <= now,
        "expiresInSecs": (secs - now).max(0),
    })
}

// ---- Internals -------------------------------------------------------------

struct MappedTokens {
    access: String,
    refresh: String,
    expires_at: String,
    scope: String,
}

fn map_tokens(tokens: &Value) -> Result<MappedTokens, String> {
    let access = tokens
        .get("access_token")
        .and_then(|v| v.as_str())
        .ok_or("token response missing access_token")?
        .to_string();
    let refresh = tokens
        .get("refresh_token")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let scope = tokens
        .get("scope")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let expires_in = tokens.get("expires_in").and_then(|v| v.as_i64()).unwrap_or(0);
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let now = now as i64;
    let expires_at = crate::db::iso_from_secs_pub(now + expires_in.max(0));
    Ok(MappedTokens { access, refresh, expires_at, scope })
}

async fn post_tokens(
    http: &reqwest::Client,
    url: &str,
    body: &Value,
    encoding: TokenEncoding,
) -> Result<Value, String> {
    let is_form = body.get("__form__").is_some();
    let req = if encoding == TokenEncoding::Form || is_form {
        let form = body
            .get("__form__")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        http.post(url)
            .header("content-type", "application/x-www-form-urlencoded")
            .body(form)
    } else {
        http.post(url).header("content-type", "application/json").body(body.to_string())
    };
    let resp = req
        .timeout(Duration::from_secs(30))
        .send()
        .await
        .map_err(|e| format!("token endpoint unreachable: {e}"))?;
    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(format!("token endpoint HTTP {status}: {}", truncate(&text, 300)));
    }
    serde_json::from_str(&text).map_err(|_| format!("unparseable token response: {}", truncate(&text, 200)))
}

/// Pull `code` and `state` out of a raw code, a `code#state` pair, or a full
/// callback URL (query or fragment).
fn extract_code_and_state(input: &str) -> (String, String) {
    let input = input.trim();
    if let Some(q) = input.split(['?', '#']).nth(1) {
        let mut code = String::new();
        let mut state = String::new();
        for pair in q.split('&') {
            if let Some(v) = pair.strip_prefix("code=") {
                code = urlencode_decode(v);
            } else if let Some(v) = pair.strip_prefix("state=") {
                state = urlencode_decode(v);
            }
        }
        if !code.is_empty() {
            return (code, state);
        }
    }
    if let Some((code, embedded)) = input.split_once('#') {
        return (code.to_string(), embedded.to_string());
    }
    (input.to_string(), String::new())
}

fn urlencode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

fn urlencode_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => {
                let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or("");
                if let Ok(v) = u8::from_str_radix(hex, 16) {
                    out.push(v);
                    i += 3;
                } else {
                    out.push(bytes[i]);
                    i += 1;
                }
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn truncate(s: &str, n: usize) -> String {
    if s.len() <= n {
        s.to_string()
    } else {
        format!("{}…", &s[..n])
    }
}

fn redact(mut conn: Value) -> Value {
    if let Some(o) = conn.as_object_mut() {
        for k in ["apiKey", "accessToken", "refreshToken", "idToken"] {
            o.remove(k);
        }
    }
    conn
}
