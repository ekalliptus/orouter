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

    /// Read the merged settings object (DEFAULT_SETTINGS ∪ stored row), minus
    /// secrets. Mirrors src/lib/db/repos/settingsRepo.js getSettings +
    /// mergeWithDefaults, and the GET /api/settings redaction (password /
    /// oidcClientSecret stripped, hasPassword + oidcConfigured added).
    pub async fn get_settings_safe(&self) -> serde_json::Value {
        let merged = self.read_settings_raw_merged().await;
        redact_settings(&merged)
    }

    /// Internal: raw merged settings INCLUDING secrets (used by login +
    /// password-change paths only).
    pub async fn get_settings_full(&self) -> serde_json::Value {
        self.read_settings_raw_merged().await
    }

    async fn read_settings_raw_merged(&self) -> serde_json::Value {
        let conn = self.inner.clone();
        tokio::task::spawn_blocking(move || -> serde_json::Value {
            let conn = conn.blocking_lock();
            let row: Option<String> = conn
                .query_row("SELECT data FROM settings WHERE id = 1", [], |r| r.get::<_, String>(0))
                .ok();
            let raw: Value = row
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_else(|| Value::Object(Default::default()));
            merge_with_defaults(&raw)
        })
        .await
        .unwrap_or_else(|_| Value::Object(Default::default()))
    }

    /// Atomic read-merge-write of a settings patch (mirrors updateSettings'
    /// transaction). Returns the new safe settings. Strips protected keys
    /// (password) and hashes newPassword before writing.
    pub async fn update_settings_safe(&self, patch: Value) -> anyhow::Result<serde_json::Value> {
        use bcrypt::{hash as bcrypt_hash, verify as bcrypt_verify, DEFAULT_COST};
        let conn = self.inner.clone();
        let merged = tokio::task::spawn_blocking(move || -> anyhow::Result<serde_json::Value> {
            let mut conn = conn.blocking_lock();
            let tx = conn.transaction()?;

            let row: Option<String> = tx
                .query_row("SELECT data FROM settings WHERE id = 1", [], |r| r.get::<_, String>(0))
                .ok();
            let raw: Value = row
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_else(|| Value::Object(Default::default()));
            let current = merge_with_defaults(&raw);

            // Apply the patch, stripping protected secrets (CWE-915, parity with Node).
            let mut body = patch;
            if let Some(obj) = body.as_object_mut() {
                obj.remove("password");
                obj.remove("mitmSudoEncrypted");
            }

            // Password change: verify current, hash new.
            if let Some(obj) = body.as_object_mut() {
                if let Some(new_pw) = obj.remove("newPassword").and_then(|v| v.as_str().map(String::from)) {
                    let stored_hash = current.get("password").and_then(|v| v.as_str()).unwrap_or("");
                    if !stored_hash.is_empty() {
                        let cur_pw = obj.remove("currentPassword").and_then(|v| v.as_str().map(String::from)).unwrap_or_default();
                        if !bcrypt_verify(&cur_pw, stored_hash).unwrap_or(false) {
                            anyhow::bail!("Invalid current password");
                        }
                    } else if let Some(cur_pw) = obj.remove("currentPassword").and_then(|v| v.as_str().map(String::from)) {
                        // first-time set: allow empty or the default "123456"
                        if cur_pw != "123456" {
                            anyhow::bail!("Invalid current password");
                        }
                    }
                    let hashed = bcrypt_hash(&new_pw, DEFAULT_COST)?;
                    obj.insert("password".into(), Value::String(hashed));
                }
            }

            let mut merged_obj = current.as_object().cloned().unwrap_or_default();
            if let Some(patch_obj) = body.as_object() {
                for (k, v) in patch_obj {
                    merged_obj.insert(k.clone(), v.clone());
                }
            }
            let merged_val = Value::Object(merged_obj);
            // Strip runtime-only computed fields before persisting.
            let mut to_store = merged_val.clone();
            if let Some(o) = to_store.as_object_mut() {
                o.remove("hasPassword");
                o.remove("oidcConfigured");
                o.remove("enableRequestLogs");
                o.remove("enableTranslator");
            }
            let serialized = serde_json::to_string(&to_store)?;
            tx.execute(
                "INSERT INTO settings(id, data) VALUES(1, ?1)
                 ON CONFLICT(id) DO UPDATE SET data = excluded.data",
                rusqlite::params![serialized],
            )?;
            tx.commit()?;
            Ok(merged_val)
        })
        .await??;
        Ok(redact_settings(&merged))
    }

    /// List API keys (safe — includes the key string itself; the Node route
    /// returns it too, since the dashboard needs to display/manage them).
    pub async fn list_api_keys(&self) -> Vec<serde_json::Value> {
        let conn = self.inner.clone();
        tokio::task::spawn_blocking(move || -> Vec<serde_json::Value> {
            let conn = conn.blocking_lock();
            let mut stmt = match conn.prepare("SELECT id, key, name, machineId, isActive, createdAt FROM apiKeys ORDER BY createdAt ASC") {
                Ok(s) => s,
                Err(_) => return Vec::new(),
            };
            let rows = stmt
                .query_map([], |r| {
                    Ok(serde_json::json!({
                        "id": r.get::<_, String>(0)?,
                        "key": r.get::<_, String>(1)?,
                        "name": r.get::<_, Option<String>>(2)?,
                        "machineId": r.get::<_, Option<String>>(3)?,
                        "isActive": r.get::<_, i64>(4)? == 1,
                        "createdAt": r.get::<_, String>(5)?,
                    }))
                })
                .ok()
                .into_iter()
                .flatten()
                .filter_map(Result::ok);
            rows.collect()
        })
        .await
        .unwrap_or_default()
    }

    /// Create an API key in the `sk-<machineId>-<keyId>-<crc8>` format and
    /// persist it. Mirrors src/shared/utils/apiKey.js generateApiKeyWithMachine
    /// + apiKeysRepo.createApiKey. `machine_id` must be the 16-char machine id.
    pub async fn create_api_key(&self, name: &str, machine_id: &str) -> anyhow::Result<serde_json::Value> {
        anyhow::ensure!(!machine_id.is_empty(), "machineId is required");
        let id = uuid::Uuid::new_v4().to_string();
        let created_at = now_iso8601();
        let key = generate_api_key_string(machine_id);

        let conn = self.inner.clone();
        let name = name.to_string();
        let machine_id = machine_id.to_string();
        let id_c = id.clone();
        let key_c = key.clone();
        let created_c = created_at.clone();
        let name_c = name.clone();
        let machine_id_c = machine_id.clone();
        tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
            let conn = conn.blocking_lock();
            conn.execute(
                "INSERT INTO apiKeys(id, key, name, machineId, isActive, createdAt) VALUES(?1, ?2, ?3, ?4, 1, ?5)",
                rusqlite::params![id_c, key_c, name_c, machine_id_c, created_c],
            )?;
            Ok(())
        })
        .await??;

        Ok(serde_json::json!({
            "id": id,
            "key": key,
            "name": name,
            "machineId": machine_id,
            "createdAt": created_at,
        }))
    }

    /// Delete an API key by id. Returns true if a row was removed.
    pub async fn delete_api_key(&self, id: &str) -> bool {
        let conn = self.inner.clone();
        let id = id.to_string();
        tokio::task::spawn_blocking(move || {
            let conn = conn.blocking_lock();
            let affected = conn.execute("DELETE FROM apiKeys WHERE id = ?1", rusqlite::params![id])?;
            Ok::<_, rusqlite::Error>(affected > 0)
        })
        .await
        .ok()
        .and_then(|r| r.ok())
        .unwrap_or(false)
    }

    /// List all provider connections with secrets stripped (apiKey/
    /// accessToken/refreshToken/idToken removed). Mirrors GET /api/providers.
    pub async fn list_connections_safe(&self) -> Vec<serde_json::Value> {
        let conn = self.inner.clone();
        tokio::task::spawn_blocking(move || -> Vec<serde_json::Value> {
            let conn = conn.blocking_lock();
            let mut stmt = match conn.prepare(
                "SELECT id, provider, authType, name, email, priority, isActive, data, createdAt, updatedAt
                 FROM providerConnections ORDER BY priority ASC",
            ) {
                Ok(s) => s,
                Err(_) => return Vec::new(),
            };
            let rows = stmt
                .query_map([], |r| {
                    // Read every column here — &Row can't escape the closure lifetime.
                    Ok((
                        r.get::<_, String>(0).unwrap_or_default(),   // id
                        r.get::<_, String>(1).unwrap_or_default(),   // provider
                        r.get::<_, String>(2).unwrap_or_default(),   // authType
                        r.get::<_, Option<String>>(3).ok().flatten(), // name
                        r.get::<_, Option<String>>(4).ok().flatten(), // email
                        r.get::<_, i64>(5).unwrap_or(1),             // priority
                        r.get::<_, i64>(6).unwrap_or(1),             // isActive
                        r.get::<_, String>(7).unwrap_or_default(),   // data
                        r.get::<_, String>(8).unwrap_or_default(),   // createdAt
                        r.get::<_, String>(9).unwrap_or_default(),   // updatedAt
                    ))
                })
                .ok()
                .into_iter()
                .flatten()
                .filter_map(Result::ok);
            let mut out = Vec::new();
            for (id, provider, auth_type, name, email, priority, is_active, data_json, created, updated) in rows {
                let mut conn_val: Value = serde_json::from_str(&data_json).unwrap_or_else(|_| Value::Object(Default::default()));
                if let Some(obj) = conn_val.as_object_mut() {
                    obj.insert("id".into(), Value::String(id));
                    obj.insert("provider".into(), Value::String(provider));
                    obj.insert("authType".into(), Value::String(auth_type));
                    if let Some(n) = name { obj.insert("name".into(), Value::String(n)); }
                    if let Some(e) = email { obj.insert("email".into(), Value::String(e)); }
                    obj.insert("priority".into(), Value::Number(priority.into()));
                    obj.insert("isActive".into(), Value::Bool(is_active == 1));
                    obj.insert("createdAt".into(), Value::String(created));
                    obj.insert("updatedAt".into(), Value::String(updated));
                    // Strip secrets (parity with Node GET /api/providers).
                    obj.remove("apiKey");
                    obj.remove("accessToken");
                    obj.remove("refreshToken");
                    obj.remove("idToken");
                }
                out.push(conn_val);
            }
            out
        })
        .await
        .unwrap_or_default()
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

// ============================================================
// Settings helpers — mirror src/lib/db/repos/settingsRepo.js
// ============================================================

/// The full default settings map (parity with DEFAULT_SETTINGS in settingsRepo).
/// Applied on top of stored rows so missing keys keep their documented defaults.
fn default_settings() -> serde_json::Map<String, Value> {
    let mut m = serde_json::Map::new();
    let mut set = |k: &str, v: Value| m.insert(k.into(), v);
    set("cloudEnabled", Value::Bool(false));
    set("tunnelEnabled", Value::Bool(false));
    set("tunnelUrl", Value::String(String::new()));
    set("tunnelProvider", Value::String("cloudflare".into()));
    set("tailscaleEnabled", Value::Bool(false));
    set("tailscaleUrl", Value::String(String::new()));
    set("stickyRoundRobinLimit", Value::Number(3.into()));
    set("providerStrategies", Value::Object(Default::default()));
    set("quotaVisibility", Value::Object(Default::default()));
    set("comboStrategy", Value::String("fallback".into()));
    set("comboStickyRoundRobinLimit", Value::Number(1.into()));
    set("comboStrategies", Value::Object(Default::default()));
    set("requireLogin", Value::Bool(true));
    // Fail-closed: LLM endpoints require an API key unless explicitly disabled.
    set("requireApiKey", Value::Bool(true));
    set("tunnelDashboardAccess", Value::Bool(true));
    set("authMode", Value::String("password".into()));
    set("oidcIssuerUrl", Value::String(String::new()));
    set("oidcClientId", Value::String(String::new()));
    set("oidcClientSecret", Value::String(String::new()));
    set("oidcScopes", Value::String("openid profile email".into()));
    set("oidcLoginLabel", Value::String("Sign in with OIDC".into()));
    set("enableObservability", Value::Bool(true));
    set("observabilityMaxRecords", Value::Number(1000.into()));
    set("observabilityBatchSize", Value::Number(20.into()));
    set("observabilityFlushIntervalMs", Value::Number(5000.into()));
    set("observabilityMaxJsonSize", Value::Number(5.into()));
    set("usageHistoryRetentionDays", Value::Number(30.into()));
    set("outboundProxyEnabled", Value::Bool(false));
    set("outboundProxyUrl", Value::String(String::new()));
    set("outboundNoProxy", Value::String(String::new()));
    set("mitmRouterBaseUrl", Value::String("http://localhost:20128".into()));
    set("dnsToolEnabled", Value::Object(Default::default()));
    set("rtkEnabled", Value::Bool(true));
    set("headroomEnabled", Value::Bool(false));
    set("headroomUrl", Value::String("http://localhost:8787".into()));
    set("headroomCompressUserMessages", Value::Bool(false));
    set("cavemanEnabled", Value::Bool(false));
    set("cavemanLevel", Value::String("full".into()));
    set("ponytailEnabled", Value::Bool(false));
    set("ponytailLevel", Value::String("full".into()));
    set("pxpipeEnabled", Value::Bool(false));
    set("pxpipeAutoInstall", Value::Bool(true));
    set("pxpipeMinChars", Value::Number(25000.into()));
    set("pxpipeTimeoutMs", Value::Number(15000.into()));
    m
}

/// Merge stored settings over defaults (mergeWithDefaults parity).
fn merge_with_defaults(raw: &Value) -> Value {
    let mut merged = default_settings();
    if let Some(raw_obj) = raw.as_object() {
        for (k, v) in raw_obj {
            merged.insert(k.clone(), v.clone());
        }
    }
    // Backfill any undefined default key (parity: outboundProxyEnabled back-compat).
    for (k, def) in default_settings() {
        if merged.get(&k).map(|v| v.is_null()).unwrap_or(true) {
            if k == "outboundProxyEnabled" {
                let url = merged.get("outboundProxyUrl").and_then(|v| v.as_str()).unwrap_or("");
                merged.insert(k, Value::Bool(!url.trim().is_empty()));
            } else {
                merged.insert(k, def);
            }
        }
    }
    Value::Object(merged)
}

/// Strip secrets + add computed flags (GET /api/settings parity).
fn redact_settings(merged: &Value) -> Value {
    let mut out = merged.clone();
    if let Some(obj) = out.as_object_mut() {
        obj.remove("password");
        let oidc_secret = obj.remove("oidcClientSecret").unwrap_or(Value::Null);
        let oidc_issuer = obj.get("oidcIssuerUrl").and_then(|v| v.as_str()).unwrap_or("");
        let oidc_client = obj.get("oidcClientId").and_then(|v| v.as_str()).unwrap_or("");
        let has_password = merged.get("password").and_then(|v| v.as_str()).is_some();
        obj.insert("oidcConfigured".into(), Value::Bool(!oidc_issuer.is_empty() && !oidc_client.is_empty() && oidc_secret.as_str().is_some()));
        obj.insert("hasPassword".into(), Value::Bool(has_password));
    }
    out
}

// ============================================================
// API key format — mirror src/shared/utils/apiKey.js
// sk-<machineId>-<keyId>-<crc8>
// ============================================================

/// HMAC-SHA256 secret for the API key CRC (apiKey.js API_KEY_SECRET).
fn api_key_secret() -> String {
    std::env::var("API_KEY_SECRET").unwrap_or_else(|_| "endpoint-proxy-api-key-secret".to_string())
}

/// Generate a key in the `sk-<machineId>-<keyId>-<crc8>` format.
/// keyId = 6 random [a-z0-9]; crc8 = first 8 hex of HMAC-SHA256(secret, machineId+keyId).
fn generate_api_key_string(machine_id: &str) -> String {
    use rand::Rng;
    const CHARS: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789";
    let mut rng = rand::thread_rng();
    let key_id: String = (0..6).map(|_| CHARS[rng.gen_range(0..CHARS.len())] as char).collect();
    let crc = crc8(machine_id, &key_id);
    format!("sk-{machine_id}-{key_id}-{crc}")
}

/// First 8 hex chars of HMAC-SHA256(secret, machineId + keyId).
fn crc8(machine_id: &str, key_id: &str) -> String {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    type HmacSha256 = Hmac<Sha256>;
    let mut mac = HmacSha256::new_from_slice(api_key_secret().as_bytes()).expect("HMAC key");
    mac.update((machine_id.to_owned() + key_id).as_bytes());
    let bytes = mac.finalize().into_bytes();
    let mut hex = String::with_capacity(8);
    for b in bytes.iter().take(4) {
        hex.push_str(&format!("{:02x}", b));
    }
    hex
}

/// Current time as ISO 8601 UTC (e.g. "2026-08-08T02:49:12.000Z") — matches the
/// Node `new Date().toISOString()` shape used for createdAt fields.
fn now_iso8601() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let (y, mo, d, h, mi, s) = unix_to_civil(secs);
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{s:02}.000Z")
}

/// Unix seconds → (YYYY, MM, DD, HH, MM, SS) in UTC (Hinnant civil_from_days).
fn unix_to_civil(mut secs: u64) -> (i64, u32, u32, u32, u32, u32) {
    let day = (secs / 86400) as i64;
    secs %= 86400;
    let h = (secs / 3600) as u32;
    let mi = ((secs % 3600) / 60) as u32;
    let s = (secs % 60) as u32;
    // civil_from_days
    let z = day + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u64; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365; // [0, 399]
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32; // [1, 12]
    (if m <= 2 { y + 1 } else { y }, m, d, h, mi, s)
}
