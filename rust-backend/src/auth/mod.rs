//! Dashboard auth: JWT cookie issuance + verification, the login rate limiter,
//! and the login/status/logout handlers.
//!
//! Mirrors src/lib/auth/dashboardSession.js (jose HS256, 24h, httpOnly
//! `auth_token` cookie) + src/lib/auth/loginLimiter.js (5 fails → escalating
//! lockout: 30s, 2m, 10m, 30m) + src/app/api/auth/login/route.js.

use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tracing::warn;

use crate::proxy::AppState;

pub mod middleware;

const COOKIE_NAME: &str = "auth_token";
const DEFAULT_PASSWORD: &str = "123456";
/// 5 failed attempts → lockout (loginLimiter.js MAX_FAILS_BEFORE_LOCK).
const MAX_FAILS_BEFORE_LOCK: u32 = 5;
/// Lock durations escalate per lockLevel (loginLimiter.js LOCK_STEPS_MS).
const LOCK_STEPS_MS: [u64; 4] = [30_000, 120_000, 600_000, 1_800_000]; // 30s, 2m, 10m, 30m
const FAIL_WINDOW_MS: u64 = 60 * 60 * 1000; // 1h since last fail → auto reset

// ============================================================
// JWT secret — load from JWT_SECRET env or <data_dir>/jwt-secret file,
// generating + persisting one on first run (dashboardSession.js loadJwtSecret).
// ============================================================

static JWT_SECRET_STRING: Lazy<String> = Lazy::new(load_jwt_secret);
static JWT_ENCODING: Lazy<EncodingKey> = Lazy::new(|| EncodingKey::from_secret(JWT_SECRET_STRING.as_bytes()));
static JWT_DECODING: Lazy<DecodingKey> = Lazy::new(|| DecodingKey::from_secret(JWT_SECRET_STRING.as_bytes()));

#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    authenticated: bool,
    exp: usize,
    iat: usize,
}

fn load_jwt_secret() -> String {
    if let Ok(env) = std::env::var("JWT_SECRET") {
        if !env.is_empty() {
            return env;
        }
    }
    let data_dir = data_dir();
    let file = data_dir.join("jwt-secret");
    if let Ok(secret) = std::fs::read_to_string(&file) {
        let trimmed = secret.trim().to_string();
        if !trimmed.is_empty() {
            return trimmed;
        }
    }
    let _ = std::fs::create_dir_all(&data_dir);
    let generated = random_hex(32);
    // Best-effort 0600 perms (Unix). Errors are non-fatal.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(metadata) = std::fs::metadata(&file) {
            let _ = std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600));
            let _ = metadata;
        }
    }
    let _ = std::fs::write(&file, &generated);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600));
    }
    generated
}

fn data_dir() -> PathBuf {
    if let Ok(d) = std::env::var("DATA_DIR") {
        if !d.is_empty() {
            return PathBuf::from(d);
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        if !home.is_empty() {
            return PathBuf::from(home).join(".9router");
        }
    }
    PathBuf::from(".9router")
}

fn random_hex(nbytes: usize) -> String {
    use rand::RngCore;
    let mut bytes = vec![0u8; nbytes];
    rand::thread_rng().fill_bytes(&mut bytes);
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

/// Stable per-install machine id (16 hex chars). Used as the machineId embedded
/// in generated API keys and, optionally, as a CLI token seed.
///
/// Mirrors the intent of src/shared/utils/machineId.js getConsistentMachineId:
/// a stable, per-host identifier persisted in DATA_DIR. We derive it from
/// MACHINE_ID_SALT + a persisted random value (machine-id file) rather than
/// host fingerprinting, to stay portable.
pub fn machine_id() -> String {
    if let Ok(env) = std::env::var("OROUTER_MACHINE_ID") {
        if !env.is_empty() {
            return env;
        }
    }
    let dir = data_dir();
    let file = dir.join("machine-id");
    if let Ok(id) = std::fs::read_to_string(&file) {
        let trimmed = id.trim().to_string();
        if trimmed.len() == 16 && trimmed.chars().all(|c| c.is_ascii_hexdigit()) {
            return trimmed;
        }
    }
    let _ = std::fs::create_dir_all(&dir);
    let id = random_hex(8); // 16 hex chars
    let _ = std::fs::write(&file, &id);
    id
}

/// Issue a 24h HS256 JWT for an authenticated dashboard session.
pub fn create_token() -> Result<String, jsonwebtoken::errors::Error> {
    let now = unix_secs();
    let claims = Claims { authenticated: true, exp: (now + 24 * 3600) as usize, iat: now as usize };
    encode(&Header::default(), &claims, &JWT_ENCODING)
}

/// Verify a token's signature + expiry. Returns true on a valid token.
pub fn verify_token(token: &str) -> bool {
    decode::<Claims>(token, &JWT_DECODING, &Validation::default()).is_ok()
}

/// Whether the request came over https (x-forwarded-proto) or AUTH_COOKIE_SECURE
/// forces it — decides the `secure` cookie attribute.
fn use_secure_cookie(headers: &HeaderMap) -> bool {
    if std::env::var("AUTH_COOKIE_SECURE").as_deref() == Ok("true") {
        return true;
    }
    headers.get("x-forwarded-proto").and_then(|v| v.to_str().ok()) == Some("https")
}

/// Build the Set-Cookie header value for the auth token (or deletion on logout).
fn set_cookie(value: &str, secure: bool) -> String {
    let flags = format!(
        "; HttpOnly; SameSite=Lax; Path=/{}",
        if secure { "; Secure" } else { "" }
    );
    format!("{COOKIE_NAME}={value}{flags}")
}

// ============================================================
// Login rate limiter — in-memory per-IP (loginLimiter.js parity).
// ============================================================

#[derive(Default, Clone)]
struct LimiterEntry {
    fails: u32,
    lock_level: u32,
    last_fail_at: Option<Instant>,
    lock_until: Option<Instant>,
}

#[derive(Default)]
struct Limiter {
    map: std::collections::HashMap<String, LimiterEntry>,
}

static LIMITER: Lazy<Mutex<Limiter>> = Lazy::new(|| Mutex::new(Limiter { map: Default::default() }));

fn unix_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Extract client IP (loginLimiter.getClientIp): x-forwarded-for first, then
/// socket peer — here we approximate with x-forwarded-for / x-real-ip / "local".
fn client_ip(headers: &HeaderMap) -> String {
    if let Some(xff) = headers.get("x-forwarded-for").and_then(|v| v.to_str().ok()) {
        if let Some(first) = xff.split(',').next() {
            return first.trim().to_string();
        }
    }
    if let Some(real) = headers.get("x-real-ip").and_then(|v| v.to_str().ok()) {
        return real.trim().to_string();
    }
    "local".to_string()
}

struct LockState {
    locked: bool,
    retry_after: u64,
}

fn check_lock(ip: &str) -> LockState {
    let mut limiter = LIMITER.lock().unwrap();
    let entry = limiter.map.entry(ip.to_string()).or_default();
    let now = Instant::now();
    // Auto-reset if the fail window has elapsed since the last fail and we're
    // past any lockout (loginLimiter.js auto-reset branch).
    if let Some(last) = entry.last_fail_at {
        if now.duration_since(last) > Duration::from_millis(FAIL_WINDOW_MS) {
            if let Some(until) = entry.lock_until {
                if now >= until {
                    *entry = LimiterEntry::default();
                }
            }
        }
    }
    if let Some(until) = entry.lock_until {
        if now < until {
            let remaining = until.duration_since(now);
            return LockState { locked: true, retry_after: remaining.as_secs() + 1 };
        }
    }
    LockState { locked: false, retry_after: 0 }
}

/// Record a failure: increment fails, escalate lockout when threshold hit.
/// Returns remaining attempts before lockout (for the error message).
fn record_fail(ip: &str) -> u32 {
    let mut limiter = LIMITER.lock().unwrap();
    let entry = limiter.map.entry(ip.to_string()).or_default();
    entry.fails += 1;
    entry.last_fail_at = Some(Instant::now());
    let remaining = MAX_FAILS_BEFORE_LOCK.saturating_sub(entry.fails);
    if entry.fails >= MAX_FAILS_BEFORE_LOCK {
        let step_ms = LOCK_STEPS_MS[std::cmp::min(entry.lock_level as usize, LOCK_STEPS_MS.len() - 1)];
        entry.lock_until = Some(Instant::now() + Duration::from_millis(step_ms));
        entry.lock_level += 1;
        entry.fails = 0; // reset so the next window counts fresh
    }
    remaining
}

fn record_success(ip: &str) {
    let mut limiter = LIMITER.lock().unwrap();
    limiter.map.insert(ip.to_string(), LimiterEntry::default());
}

// ============================================================
// Handlers
// ============================================================

const RESET_HINT: &str =
    "Forgot password? Reset to default via 9Router CLI → Settings → Reset Password to Default.";

/// POST /api/auth/login — verify password (bcrypt or INITIAL_PASSWORD env),
/// set the httpOnly JWT cookie. Mirrors src/app/api/auth/login/route.js.
pub async fn login(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Response {
    let ip = client_ip(&headers);
    let lock = check_lock(&ip);
    if lock.locked {
        return error_json(
            StatusCode::TOO_MANY_REQUESTS,
            &format!("Too many failed attempts. Try again in {}s. {RESET_HINT}", lock.retry_after),
            json!({ "retryAfter": lock.retry_after, "resetHint": RESET_HINT }),
        );
    }

    let password = body.get("password").and_then(|v| v.as_str()).unwrap_or("");
    let settings = state.db.get_settings_full().await;

    // OIDC mode disables password login.
    let auth_mode = settings.get("authMode").and_then(|v| v.as_str()).unwrap_or("password");
    if auth_mode == "oidc" {
        let issuer = settings.get("oidcIssuerUrl").and_then(|v| v.as_str()).unwrap_or("");
        let client = settings.get("oidcClientId").and_then(|v| v.as_str()).unwrap_or("");
        let secret = settings.get("oidcClientSecret").and_then(|v| v.as_str()).unwrap_or("");
        if !issuer.is_empty() && !client.is_empty() && !secret.is_empty() {
            return error_json(StatusCode::FORBIDDEN, "Password login is disabled. Use OIDC sign in.", json!({}));
        }
    }

    let stored_hash = settings.get("password").and_then(|v| v.as_str()).unwrap_or("");
    let is_valid = if !stored_hash.is_empty() {
        bcrypt::verify(password, stored_hash).unwrap_or(false)
    } else {
        // No stored hash: fall back to INITIAL_PASSWORD env or default "123456".
        let initial = std::env::var("INITIAL_PASSWORD").unwrap_or_else(|_| DEFAULT_PASSWORD.to_string());
        password == initial
    };

    if is_valid {
        record_success(&ip);
        let token = match create_token() {
            Ok(t) => t,
            Err(e) => {
                warn!("jwt sign failed: {e}");
                return error_json(StatusCode::INTERNAL_SERVER_ERROR, "token sign failed", json!({}));
            }
        };
        let secure = use_secure_cookie(&headers);
        let is_local = ip == "local";
        // Default password still in use on a remote client → require a change.
        let must_change_password = stored_hash.is_empty()
            && std::env::var("INITIAL_PASSWORD").is_err()
            && !is_local;
        let body = json!({ "success": true, "mustChangePassword": must_change_password });
        let mut resp = (StatusCode::OK, Json(body)).into_response();
        resp.headers_mut().insert("cache-control", "no-store".parse().unwrap());
        resp.headers_mut().insert(
            "set-cookie",
            set_cookie(&token, secure).parse().unwrap(),
        );
        return resp;
    }

    let remaining = record_fail(&ip);
    let after = check_lock(&ip);
    if after.locked {
        return error_json(
            StatusCode::TOO_MANY_REQUESTS,
            &format!("Too many failed attempts. Try again in {}s. {RESET_HINT}", after.retry_after),
            json!({ "retryAfter": after.retry_after, "resetHint": RESET_HINT }),
        );
    }
    error_json(
        StatusCode::UNAUTHORIZED,
        &format!("Invalid password. {remaining} attempt(s) left before lockout."),
        json!({ "remainingBeforeLock": remaining }),
    )
}

/// POST /api/auth/logout — clear the auth cookie.
pub async fn logout(headers: HeaderMap) -> Response {
    let secure = use_secure_cookie(&headers);
    let mut resp = (StatusCode::OK, Json(json!({ "success": true }))).into_response();
    resp.headers_mut().insert("cache-control", "no-store".parse().unwrap());
    resp.headers_mut().insert(
        "set-cookie",
        set_cookie("", secure).parse().unwrap(),
    );
    resp
}

/// GET /api/auth/status — whether the request carries a valid session.
pub async fn status(headers: HeaderMap) -> Response {
    let authed = extract_and_verify(&headers);
    (StatusCode::OK, Json(json!({ "authenticated": authed }))).into_response()
}

/// Middleware helper: extract + verify the auth_token cookie from a request.
pub fn extract_and_verify(headers: &HeaderMap) -> bool {
    let cookie = headers.get("cookie").and_then(|v| v.to_str().ok()).unwrap_or("");
    for part in cookie.split(';') {
        let part = part.trim();
        if let Some(rest) = part.strip_prefix(&format!("{COOKIE_NAME}=")) {
            return verify_token(rest);
        }
    }
    false
}

fn error_json(status: StatusCode, message: &str, extra: Value) -> Response {
    let mut body = json!({ "error": message });
    if let Some(obj) = body.as_object_mut() {
        if let Some(ex) = extra.as_object() {
            for (k, v) in ex {
                obj.insert(k.clone(), v.clone());
            }
        }
    }
    (status, Json(body)).into_response()
}
