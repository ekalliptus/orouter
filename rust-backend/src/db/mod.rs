//! Shared SQLite access — opens the SAME file Node/Go use and runs the same
//! PRAGMAs (WAL, busy_timeout=5000). For v1 this is read-only on the proxy
//! path: we read provider credentials + settings + validate inbound API keys.
//! No schema migrations: the file is created/owned by Node; we only read.
//!
//! Mirrors backend/internal/database/database.go + repos.go.

use std::path::Path;

use anyhow::{Context, Result};
use rusqlite::Connection;
use serde_json::Value;
use tokio::sync::Mutex;

/// Wrapper holding a pooled connection + write mutex (matches Go's
/// SetMaxOpenConns(1) serialization). For v1 reads we only need the mutex to
/// keep rusqlite's non-Send Connection safely shareable across tasks.
#[derive(Clone)]
pub struct Db {
    inner: std::sync::Arc<Mutex<Connection>>,
}

impl Db {
    /// Open the shared DB read/write. If the file doesn't exist we still return
    /// a handle that fails queries gracefully — the Go server likewise logs but
    /// keeps the proxy working without a DB.
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let conn = Connection::open(path)
            .with_context(|| format!("open sqlite at {}", path.display()))?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.pragma_update(None, "busy_timeout", 5000)?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        Ok(Self { inner: std::sync::Arc::new(Mutex::new(conn)) })
    }

    /// Read a single active credential blob for a provider, picking fill-first
    /// by priority. Returns the parsed `data` JSON for the first safe row, or
    /// None. "Safe" mirrors Go's nativeConnectionSafe: skip OAuth rows
    /// (refreshToken/expiresAt) and model-locked rows.
    pub async fn pick_credential(&self, provider: &str, model: &str) -> Option<Credential> {
        let conn = self.inner.clone();
        let provider = provider.to_string();
        let model = model.to_string();
        tokio::task::spawn_blocking(move || -> Option<Credential> {
            let conn = conn.blocking_lock();
            let mut stmt = conn.prepare(
                "SELECT data FROM providerConnections
                 WHERE provider = ?1 AND isActive = 1
                 ORDER BY priority ASC",
            )
            .ok()?;
            let rows: Vec<String> = stmt
                .query_map([&provider], |r| r.get::<_, String>(0))
                .ok()?
                .filter_map(Result::ok)
                .collect();
            drop(stmt);

            for data_json in rows {
                let Ok(data) = serde_json::from_str::<Value>(&data_json) else { continue };
                let obj = match data.as_object() {
                    Some(o) => o,
                    None => continue,
                };
                // OAuth refresh / expiry → can't handle natively yet.
                if obj.get("refreshToken").and_then(|v| v.as_str()).is_some_and(|s| !s.is_empty())
                    || obj.get("expiresAt").and_then(|v| v.as_str()).is_some()
                {
                    continue;
                }
                // Model locks: modelLock_<model> or modelLock___all with a future RFC3339.
                if is_model_locked(obj, &model) {
                    continue;
                }
                let credential = obj
                    .get("apiKey")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty())
                    .or_else(|| obj.get("accessToken").and_then(|v| v.as_str()).filter(|s| !s.is_empty()))
                    .map(|s| s.to_string());
                if let Some(cred) = credential {
                    return Some(Credential {
                        secret: cred,
                        connection_id: obj
                            .get("id")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                    });
                }
            }
            None
        })
        .await
        .ok()
        .flatten()
    }

    /// Read the settings blob and return whether inbound /v1/* requires an
    /// API key. Mirrors Go reading `settings.requireApiKey` (not the env var).
    pub async fn require_api_key(&self) -> bool {
        let conn = self.inner.clone();
        tokio::task::spawn_blocking(move || {
            let conn = conn.blocking_lock();
            let row: Option<String> = conn
                .query_row(
                    "SELECT value FROM settings WHERE key = 'settings' LIMIT 1",
                    [],
                    |r| r.get::<_, String>(0),
                )
                .ok();
            let Some(json) = row else { return false };
            serde_json::from_str::<Value>(&json)
                .ok()
                .and_then(|v| v.get("requireApiKey").and_then(|v| v.as_bool()))
                .unwrap_or(false)
        })
        .await
        .unwrap_or(false)
    }

    /// Constant-time validate an inbound API key against the active keys table.
    /// Mirrors Go's ValidateApiKey (repos.go:126-148).
    pub async fn validate_api_key(&self, presented: &str) -> bool {
        use subtle::ConstantTimeEq;
        let conn = self.inner.clone();
        let presented = presented.as_bytes().to_vec();
        tokio::task::spawn_blocking(move || -> bool {
            let conn = conn.blocking_lock();
            let Ok(mut stmt) = conn.prepare("SELECT key FROM apiKeys WHERE isActive = 1") else {
                return false;
            };
            let keys: Vec<String> = stmt
                .query_map([], |r| r.get::<_, String>(0))
                .ok()
                .into_iter()
                .flatten()
                .filter_map(Result::ok)
                .collect();
            // Compare in constant time; match on equal length only.
            for k in &keys {
                let kb = k.as_bytes();
                if kb.len() == presented.len() && bool::from(kb.ct_eq(&presented)) {
                    return true;
                }
            }
            false
        })
        .await
        .unwrap_or(false)
    }
}

#[derive(Debug, Clone)]
pub struct Credential {
    pub secret: String,
    pub connection_id: String,
}

/// A connection row is model-locked when it carries a `modelLock_<model>` or
/// `modelLock___all` key whose value is an RFC3339 timestamp still in the
/// future. Mirrors Go's modelLockActive (chat_resolver.go).
fn is_model_locked(obj: &serde_json::Map<String, Value>, model: &str) -> bool {
    let now = chrono_now_secs();
    for key in [format!("modelLock_{model}"), "modelLock___all".to_string()] {
        if let Some(ts) = obj.get(&key).and_then(|v| v.as_str()) {
            if parse_rfc3339_secs(ts).map(|t| t > now).unwrap_or(false) {
                return true;
            }
        }
    }
    false
}

fn chrono_now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Best-effort RFC3339 → unix seconds. Avoids pulling chrono just for this;
/// accepts the common `2025-01-01T00:00:00Z` / with-offset shapes.
fn parse_rfc3339_secs(s: &str) -> Option<i64> {
    // We only need coarse comparison for "locked in the future" decisions.
    // Delegate to chrono-free manual parse of the most common forms.
    let s = s.trim();
    // YYYY-MM-DDTHH:MM:SS[.fff][Z|+HH:MM|-HH:MM]
    let (date, rest) = s.split_once('T')?;
    let d: Vec<u64> = date.split('-').filter_map(|x| x.parse().ok()).collect();
    if d.len() != 3 { return None; }
    // time portion up to an optional offset
    let (timepart, offset) = rest.split_once(|c: char| c == 'Z' || c == '+' || c == '-').unwrap_or((rest, ""));
    let t: Vec<u64> = timepart.split(':').filter_map(|x| x.parse().ok()).collect();
    if t.len() < 3 { return None; }
    let (yy, mm, dd) = (d[0] as i64, d[1] as i64, d[2] as i64);
    let (hh, mi, ss) = (t[0] as i64, t[1] as i64, t[2] as i64);
    // Days since epoch via civil-from-days (Hinnant's algorithm).
    let y = if mm <= 2 { yy - 1 } else { yy };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = (y - era * 400) as u64; // [0, 399]
    let doy = ((153 * (if mm > 2 { mm - 3 } else { mm + 9 }) + 2) / 5 + dd - 1) as u64;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy; // [0, 146096]
    let days = era as i64 * 146097 + doe as i64 - 719468;
    let mut secs = days * 86400 + hh * 3600 + mi * 60 + ss;
    // Apply offset (Z or +HH:MM / -HH:MM): convert to UTC.
    if !offset.is_empty() {
        let sign = if s.contains('+') { 1 } else { -1 };
        let parts: Vec<&str> = offset.trim_start_matches(|c: char| c == '+' || c == '-').split(':').collect();
        if parts.len() >= 2 {
            if let (Ok(h), Ok(m)) = (parts[0].parse::<i64>(), parts[1].parse::<i64>()) {
                secs -= sign * (h * 3600 + m * 60);
            }
        }
    }
    Some(secs)
}
