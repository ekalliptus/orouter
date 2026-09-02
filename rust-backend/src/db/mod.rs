//! Shared SQLite access — opens the SAME file Node/Go use and runs the same
//! PRAGMAs (WAL, busy_timeout=5000). Reads + writes provider credentials,
//! settings, API keys, usage history/daily. No schema migrations: the file's
//! tables are created/owned by Node; we reuse them as-is.
//!
//! Mirrors backend/internal/database/database.go + repos.go + chat_writes.go.

use std::path::Path;

use anyhow::{Context, Result};
use rusqlite::{Connection, OptionalExtension};
use serde_json::Value;
use tokio::sync::Mutex;

/// Wrapper holding a pooled connection + write mutex (matches Go's
/// SetMaxOpenConns(1) serialization). For v1 reads we only need the mutex to
/// keep rusqlite's non-Send Connection safely shareable across tasks.
#[derive(Clone)]
pub struct Db {
    inner: std::sync::Arc<Mutex<Connection>>,
}

/// Idempotent schema bootstrap. Rust can now start on an empty DATA_DIR without
/// Node/Go creating tables first. Existing databases are untouched.
fn ensure_schema(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS settings (
          id INTEGER PRIMARY KEY,
          data TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS apiKeys (
          id TEXT PRIMARY KEY,
          key TEXT UNIQUE,
          name TEXT,
          machineId TEXT,
          isActive INTEGER DEFAULT 1,
          createdAt TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS providerConnections (
          id TEXT PRIMARY KEY,
          provider TEXT NOT NULL,
          authType TEXT NOT NULL,
          name TEXT,
          email TEXT,
          priority INTEGER,
          isActive INTEGER DEFAULT 1,
          data TEXT NOT NULL,
          createdAt TEXT NOT NULL,
          updatedAt TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_pc_provider ON providerConnections(provider);
        CREATE INDEX IF NOT EXISTS idx_pc_provider_active ON providerConnections(provider, isActive);
        CREATE INDEX IF NOT EXISTS idx_pc_priority ON providerConnections(provider, priority);
        CREATE TABLE IF NOT EXISTS usageHistory (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          timestamp TEXT NOT NULL,
          provider TEXT,
          model TEXT,
          connectionId TEXT,
          apiKey TEXT,
          endpoint TEXT,
          promptTokens INTEGER DEFAULT 0,
          completionTokens INTEGER DEFAULT 0,
          cost REAL DEFAULT 0,
          status TEXT,
          tokens TEXT,
          meta TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_uh_ts ON usageHistory(timestamp DESC);
        CREATE INDEX IF NOT EXISTS idx_uh_provider ON usageHistory(provider);
        CREATE INDEX IF NOT EXISTS idx_uh_model ON usageHistory(model);
        CREATE INDEX IF NOT EXISTS idx_uh_conn ON usageHistory(connectionId);
        CREATE TABLE IF NOT EXISTS usageDaily (
          dateKey TEXT PRIMARY KEY,
          data TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS _meta (
          key TEXT PRIMARY KEY,
          value TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS combos (
          id TEXT PRIMARY KEY,
          name TEXT UNIQUE NOT NULL,
          kind TEXT,
          models TEXT NOT NULL,
          createdAt TEXT NOT NULL,
          updatedAt TEXT NOT NULL
        );
        INSERT INTO settings(id, data)
        VALUES(1, '{"requireLogin":true,"requireApiKey":true}')
        ON CONFLICT(id) DO NOTHING;
        "#,
    )
}

impl Db {
    /// Open the shared DB read/write. If the file doesn't exist we still return
    /// a handle that fails queries gracefully — the Go server likewise logs but
    /// keeps the proxy working without a DB.
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let conn =
            Connection::open(path).with_context(|| format!("open sqlite at {}", path.display()))?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.pragma_update(None, "busy_timeout", 5000)?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        ensure_schema(&conn)?;
        Ok(Self {
            inner: std::sync::Arc::new(Mutex::new(conn)),
        })
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
            let mut stmt = conn
                .prepare(
                    "SELECT id, data FROM providerConnections
                 WHERE provider = ?1 AND isActive = 1
                 ORDER BY priority ASC",
                )
                .ok()?;
            let rows: Vec<(String, String)> = stmt
                .query_map([&provider], |r| {
                    Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
                })
                .ok()?
                .filter_map(Result::ok)
                .collect();
            drop(stmt);

            for (connection_id, data_json) in rows {
                let Ok(data) = serde_json::from_str::<Value>(&data_json) else {
                    continue;
                };
                let obj = match data.as_object() {
                    Some(o) => o,
                    None => continue,
                };
                // OAuth refresh / expiry → can't handle natively yet.
                if obj
                    .get("refreshToken")
                    .and_then(|v| v.as_str())
                    .is_some_and(|s| !s.is_empty())
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
                    .or_else(|| {
                        obj.get("accessToken")
                            .and_then(|v| v.as_str())
                            .filter(|s| !s.is_empty())
                    })
                    .map(|s| s.to_string());
                if let Some(cred) = credential {
                    return Some(Credential {
                        secret: cred,
                        connection_id,
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
                .query_row("SELECT data FROM settings WHERE id = 1", [], |r| {
                    r.get::<_, String>(0)
                })
                .ok();
            let Some(json) = row else { return true }; // fail-closed
            serde_json::from_str::<Value>(&json)
                .ok()
                .and_then(|v| v.get("requireApiKey").and_then(|v| v.as_bool()))
                .unwrap_or(true)
        })
        .await
        // Join/spawn failure must fail CLOSED (require the key), matching the
        // missing-row branch above — returning false here would disable auth.
        .unwrap_or(true)
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
                .query_row("SELECT data FROM settings WHERE id = 1", [], |r| {
                    r.get::<_, String>(0)
                })
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
                .query_row("SELECT data FROM settings WHERE id = 1", [], |r| {
                    r.get::<_, String>(0)
                })
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
                if let Some(new_pw) = obj
                    .remove("newPassword")
                    .and_then(|v| v.as_str().map(String::from))
                {
                    let stored_hash = current
                        .get("password")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    if !stored_hash.is_empty() {
                        let cur_pw = obj
                            .remove("currentPassword")
                            .and_then(|v| v.as_str().map(String::from))
                            .unwrap_or_default();
                        if !bcrypt_verify(&cur_pw, stored_hash).unwrap_or(false) {
                            anyhow::bail!("Invalid current password");
                        }
                    } else if let Some(cur_pw) = obj
                        .remove("currentPassword")
                        .and_then(|v| v.as_str().map(String::from))
                    {
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
    pub async fn create_api_key(
        &self,
        name: &str,
        machine_id: &str,
    ) -> anyhow::Result<serde_json::Value> {
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
            let affected =
                conn.execute("DELETE FROM apiKeys WHERE id = ?1", rusqlite::params![id])?;
            Ok::<_, rusqlite::Error>(affected > 0)
        })
        .await
        .ok()
        .and_then(|r| r.ok())
        .unwrap_or(false)
    }

    /// Enable/disable an API key (parity with Node PUT /api/keys/:id, which the
    /// Endpoint page uses as a rotate-free kill switch).
    pub async fn set_api_key_active(&self, id: &str, is_active: bool) -> bool {
        let conn = self.inner.clone();
        let id = id.to_string();
        tokio::task::spawn_blocking(move || {
            let conn = conn.blocking_lock();
            let affected = conn.execute(
                "UPDATE apiKeys SET isActive = ?1 WHERE id = ?2",
                rusqlite::params![if is_active { 1 } else { 0 }, id],
            )?;
            Ok::<_, rusqlite::Error>(affected > 0)
        })
        .await
        .ok()
        .and_then(|r| r.ok())
        .unwrap_or(false)
    }

    // ============================================================
    // Combos CRUD
    // ============================================================

    pub async fn list_combos(&self) -> Vec<serde_json::Value> {
        let conn = self.inner.clone();
        tokio::task::spawn_blocking(move || -> Vec<serde_json::Value> {
            let conn = conn.blocking_lock();
            let mut stmt = match conn.prepare("SELECT id, name, kind, models, createdAt, updatedAt FROM combos ORDER BY createdAt ASC") {
                Ok(s) => s,
                Err(_) => return Vec::new(),
            };
            let rows = stmt
                .query_map([], |r| {
                    let models_json: String = r.get::<_, String>(3)?;
                    let models_val: Value = serde_json::from_str(&models_json).unwrap_or_else(|_| serde_json::json!([]));
                    Ok(serde_json::json!({
                        "id": r.get::<_, String>(0)?,
                        "name": r.get::<_, String>(1)?,
                        "kind": r.get::<_, Option<String>>(2)?,
                        "models": models_val,
                        "createdAt": r.get::<_, String>(4)?,
                        "updatedAt": r.get::<_, String>(5)?,
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

    pub async fn create_combo(
        &self,
        name: &str,
        kind: Option<&str>,
        models: Value,
    ) -> anyhow::Result<serde_json::Value> {
        anyhow::ensure!(!name.is_empty(), "Name is required");
        let id = uuid::Uuid::new_v4().to_string();
        let now = now_iso8601();
        let models_json = serde_json::to_string(&models)?;

        let conn = self.inner.clone();
        let name_c = name.to_string();
        let kind_c = kind.map(String::from);
        let id_c = id.clone();
        let now_c = now.clone();
        tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
            let conn = conn.blocking_lock();
            conn.execute(
                "INSERT INTO combos(id, name, kind, models, createdAt, updatedAt) VALUES(?1, ?2, ?3, ?4, ?5, ?6)",
                rusqlite::params![id_c, name_c, kind_c, models_json, now_c, now_c],
            )?;
            Ok(())
        })
        .await??;

        Ok(serde_json::json!({
            "id": id,
            "name": name,
            "kind": kind,
            "models": models,
            "createdAt": now,
            "updatedAt": now,
        }))
    }

    pub async fn delete_combo(&self, id: &str) -> bool {
        let conn = self.inner.clone();
        let id = id.to_string();
        tokio::task::spawn_blocking(move || {
            let conn = conn.blocking_lock();
            let affected =
                conn.execute("DELETE FROM combos WHERE id = ?1", rusqlite::params![id])?;
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

    // ============================================================
    // Provider connection CRUD — port of src/lib/db/repos/connectionsRepo.js.
    // Row contract: 9 base columns + a `data` JSON blob holding everything else
    // (apiKey, testStatus, providerSpecificData, …). Reads spread `data` then
    // overlay base columns, so any non-base field the frontend needs must live
    // in `data`.
    // ============================================================

    /// Create a connection (apikey authType). Mirrors createProviderConnection's
    /// new-row branch: uuid id, now timestamps, priority defaults to max+1,
    /// then reorder priorities 1..N within the provider. Returns the created id.
    pub async fn create_connection(&self, input: CreateConnection) -> anyhow::Result<String> {
        let conn = self.inner.clone();
        let created_id = tokio::task::spawn_blocking(move || -> anyhow::Result<String> {
            let mut conn = conn.blocking_lock();
            let tx = conn.transaction()?;
            let now = now_iso8601();
            let id = uuid::Uuid::new_v4().to_string();

            // priority: use provided, else max(existing)+1
            let priority: i64 = match input.priority {
                Some(p) => p,
                None => {
                    let max: i64 = tx
                        .query_row(
                            "SELECT COALESCE(MAX(priority), 0) FROM providerConnections WHERE provider = ?1",
                            rusqlite::params![&input.provider],
                            |r| r.get(0),
                        )
                        .unwrap_or(0);
                    max + 1
                }
            };

            // Build the `data` JSON blob: apiKey, testStatus, providerSpecificData,
            // plus any optional fields the caller passed through `extra`.
            let mut data = serde_json::Map::new();
            data.insert("apiKey".into(), Value::String(input.api_key));
            data.insert("testStatus".into(), Value::String(input.test_status.clone()));
            if let Some(psd) = input.provider_specific_data {
                data.insert("providerSpecificData".into(), psd);
            }
            for (k, v) in input.extra {
                data.insert(k, v);
            }
            let data_json = serde_json::to_string(&Value::Object(data))?;

            tx.execute(
                "INSERT INTO providerConnections(id, provider, authType, name, email, priority, isActive, data, createdAt, updatedAt)
                 VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                rusqlite::params![
                    &id, &input.provider, input.auth_type, &input.name, input.email.as_deref(),
                    priority, if input.is_active { 1 } else { 0 }, &data_json, &now, &now
                ],
            )?;
            reorder_priorities(&tx, &input.provider)?;
            tx.commit()?;
            Ok(id)
        })
        .await??;
        Ok(created_id)
    }

    /// Update a connection (PUT /api/providers/:id). Shallow-merges incoming
    /// fields over the existing row, rebuilds the `data` blob, sets updatedAt.
    /// Returns the updated safe connection (secrets stripped) or None if missing.
    pub async fn update_connection_safe(
        &self,
        id: &str,
        patch: Value,
    ) -> anyhow::Result<Option<Value>> {
        let conn = self.inner.clone();
        let id = id.to_string();
        let updated = tokio::task::spawn_blocking(move || -> anyhow::Result<Option<Value>> {
            let mut conn = conn.blocking_lock();
            let tx = conn.transaction()?;
            let row = tx.query_row(
                "SELECT id, provider, authType, name, email, priority, isActive, data, createdAt FROM providerConnections WHERE id = ?1",
                rusqlite::params![&id],
                |r| {
                    Ok((
                        r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?,
                        r.get::<_, Option<String>>(3)?, r.get::<_, Option<String>>(4)?,
                        r.get::<_, i64>(5)?, r.get::<_, i64>(6)?,
                        r.get::<_, String>(7)?, r.get::<_, String>(8)?,
                    ))
                },
            );
            let (rid, provider, auth_type, mut name, mut email, mut priority, mut is_active, data_json, created_at) = match row {
                Ok(r) => r,
                Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(None),
                Err(e) => return Err(e.into()),
            };

            // Parse existing data blob.
            let mut data: serde_json::Map<String, Value> = serde_json::from_str(&data_json).unwrap_or_default();
            // Apply patch: base columns override, everything else merges into data.
            if let Some(po) = patch.as_object() {
                if let Some(v) = po.get("name").and_then(|v| v.as_str()) { name = Some(v.to_string()); }
                if let Some(v) = po.get("email").and_then(|v| v.as_str()) { email = Some(v.to_string()); }
                if let Some(v) = po.get("priority").and_then(|v| v.as_i64()) { priority = v; }
                if let Some(v) = po.get("isActive") { is_active = if v.as_bool().unwrap_or(true) { 1 } else { 0 }; }
                // apiKey only for apikey authType.
                if auth_type == "apikey" {
                    if let Some(v) = po.get("apiKey").and_then(|v| v.as_str()).filter(|s| !s.is_empty()) {
                        data.insert("apiKey".into(), Value::String(v.to_string()));
                    }
                }
                // testStatus / lastError / lastErrorAt.
                for k in ["testStatus", "lastError", "lastErrorAt"] {
                    if let Some(v) = po.get(k) {
                        if v.is_null() { data.remove(k); } else { data.insert(k.into(), v.clone()); }
                    }
                }
                // providerSpecificData shallow-merge.
                if let Some(incoming) = po.get("providerSpecificData").and_then(|v| v.as_object()) {
                    let existing = data.entry("providerSpecificData").or_insert_with(|| Value::Object(Default::default()));
                    if let Some(eo) = existing.as_object_mut() {
                        for (k, v) in incoming { eo.insert(k.clone(), v.clone()); }
                    } else {
                        data.insert("providerSpecificData".into(), Value::Object(incoming.clone()));
                    }
                }
                // globalPriority / defaultModel → data.
                for k in ["globalPriority", "defaultModel"] {
                    if let Some(v) = po.get(k) {
                        if v.is_null() { data.remove(k); } else { data.insert(k.into(), v.clone()); }
                    }
                }
            }

            let now = now_iso8601();
            let data_json = serde_json::to_string(&Value::Object(data))?;
            tx.execute(
                "UPDATE providerConnections SET provider=?1, authType=?2, name=?3, email=?4, priority=?5, isActive=?6, data=?7, updatedAt=?8 WHERE id=?9",
                rusqlite::params![&provider, &auth_type, &name, &email, priority, is_active, &data_json, &now, &rid],
            )?;
            let needs_reorder = patch.get("priority").is_some();
            if needs_reorder {
                reorder_priorities(&tx, &provider)?;
            }
            tx.commit()?;

            // Build safe output: spread data, overlay base, strip secrets.
            let mut out: Value = serde_json::from_str(&data_json).unwrap_or_else(|_| Value::Object(Default::default()));
            if let Some(obj) = out.as_object_mut() {
                obj.insert("id".into(), Value::String(rid));
                obj.insert("provider".into(), Value::String(provider));
                obj.insert("authType".into(), Value::String(auth_type));
                if let Some(n) = name { obj.insert("name".into(), Value::String(n)); }
                if let Some(e) = email { obj.insert("email".into(), Value::String(e)); }
                obj.insert("priority".into(), Value::Number(priority.into()));
                obj.insert("isActive".into(), Value::Bool(is_active == 1));
                obj.insert("createdAt".into(), Value::String(created_at));
                obj.insert("updatedAt".into(), Value::String(now));
                obj.remove("apiKey");
                obj.remove("accessToken");
                obj.remove("refreshToken");
                obj.remove("idToken");
            }
            Ok(Some(out))
        })
        .await??;
        Ok(updated)
    }

    /// Hard-delete a connection by id, then renumber priorities for its provider.
    /// Returns true if a row was deleted.
    pub async fn delete_connection(&self, id: &str) -> bool {
        let conn = self.inner.clone();
        let id = id.to_string();
        tokio::task::spawn_blocking(move || -> rusqlite::Result<bool> {
            let mut conn = conn.blocking_lock();
            let tx = conn.transaction()?;
            let provider: Option<String> = tx
                .query_row(
                    "SELECT provider FROM providerConnections WHERE id = ?1",
                    rusqlite::params![&id],
                    |r| r.get(0),
                )
                .ok();
            let Some(provider) = provider else {
                return Ok(false);
            };
            let affected = tx.execute(
                "DELETE FROM providerConnections WHERE id = ?1",
                rusqlite::params![&id],
            )?;
            if affected > 0 {
                reorder_priorities(&tx, &provider)?;
            }
            tx.commit()?;
            Ok(affected > 0)
        })
        .await
        .ok()
        .and_then(|r| r.ok())
        .unwrap_or(false)
    }

    /// Create an OAuth connection row (authType "oauth") storing tokens in
    /// the data blob. Mirrors the Node oauth → connectionsRepo save shape.
    pub async fn create_oauth_connection(
        &self,
        provider: &str,
        name: &str,
        access: &str,
        refresh: &str,
        expires_at: &str,
        scope: &str,
    ) -> anyhow::Result<String> {
        let conn = self.inner.clone();
        let provider = provider.to_string();
        let name = name.to_string();
        let access = access.to_string();
        let refresh = refresh.to_string();
        let expires_at = expires_at.to_string();
        let scope = scope.to_string();
        let created_id = tokio::task::spawn_blocking(move || -> anyhow::Result<String> {
            let mut conn = conn.blocking_lock();
            let tx = conn.transaction()?;
            let now = now_iso8601();
            let id = uuid::Uuid::new_v4().to_string();
            let priority: i64 = tx
                .query_row(
                    "SELECT COALESCE(MAX(priority), 0) FROM providerConnections WHERE provider = ?1",
                    rusqlite::params![&provider],
                    |r| r.get(0),
                )
                .unwrap_or(0)
                + 1;
            let data = serde_json::json!({
                "accessToken": access,
                "refreshToken": refresh,
                "expiresAt": expires_at,
                "scope": scope,
                "testStatus": "unknown",
            });
            tx.execute(
                "INSERT INTO providerConnections(id, provider, authType, name, email, priority, isActive, data, createdAt, updatedAt)
                 VALUES(?1, ?2, 'oauth', ?3, NULL, ?4, 1, ?5, ?6, ?6)",
                rusqlite::params![id, provider, name, priority, serde_json::to_string(&data)?, now],
            )?;
            reorder_priorities(&tx, &provider)?;
            tx.commit()?;
            Ok(id)
        })
        .await??;
        Ok(created_id)
    }

    /// Merge refreshed tokens back into a connection's data blob.
    pub async fn update_connection_tokens(
        &self,
        id: &str,
        access: &str,
        refresh: &str,
        expires_at: &str,
        scope: &str,
    ) {
        let conn = self.inner.clone();
        let id = id.to_string();
        let access = access.to_string();
        let refresh = refresh.to_string();
        let expires_at = expires_at.to_string();
        let scope = scope.to_string();
        let _ = tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
            let conn = conn.blocking_lock();
            let data: Option<String> = conn
                .query_row(
                    "SELECT data FROM providerConnections WHERE id = ?1",
                    rusqlite::params![&id],
                    |r| r.get(0),
                )
                .ok();
            let Some(data) = data else { return Ok(()) };
            let mut val: Value = serde_json::from_str(&data).unwrap_or_else(|_| Value::Object(Default::default()));
            if let Some(obj) = val.as_object_mut() {
                obj.insert("accessToken".into(), Value::String(access));
                if !refresh.is_empty() {
                    obj.insert("refreshToken".into(), Value::String(refresh));
                }
                obj.insert("expiresAt".into(), Value::String(expires_at));
                if !scope.is_empty() {
                    obj.insert("scope".into(), Value::String(scope));
                }
                obj.insert("lastError".into(), Value::Null);
            }
            conn.execute(
                "UPDATE providerConnections SET data = ?1, updatedAt = ?2 WHERE id = ?3",
                rusqlite::params![serde_json::to_string(&val)?, now_iso8601(), id],
            )?;
            Ok(())
        })
        .await;
    }


    /// Connections whose OAuth token expires within `lead_secs` (or already
    /// expired) AND carry a refresh token. Used by the background refresher.
    pub async fn oauth_refresh_candidates(&self, lead_secs: i64) -> Vec<(String, String)> {
        let conn = self.inner.clone();
        tokio::task::spawn_blocking(move || -> Vec<(String, String)> {
            let conn = conn.blocking_lock();
            let Ok(mut stmt) = conn
                .prepare("SELECT id, provider, data FROM providerConnections WHERE authType = 'oauth' AND isActive = 1")
            else {
                return Vec::new();
            };
            let rows = stmt
                .query_map([], |r| {
                    Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?))
                })
                .ok()
                .into_iter()
                .flatten()
                .filter_map(Result::ok);
            let now = chrono_now_secs();
            rows.filter_map(|(id, provider, data)| {
                let v: Value = serde_json::from_str(&data).ok()?;
                let has_refresh = v.get("refreshToken").and_then(|v| v.as_str()).is_some_and(|s| !s.is_empty());
                let exp = v.get("expiresAt").and_then(|v| v.as_str()).and_then(parse_rfc3339_secs).unwrap_or(0);
                (has_refresh && exp > 0 && exp - now <= lead_secs).then_some((id, provider))
            })
            .collect()
        })
        .await
        .unwrap_or_default()
    }

    /// Read a single connection's full data blob (including secrets) — used by
    /// the connection-test handler to fetch the apiKey + transport info.
    pub async fn get_connection_full(&self, id: &str) -> Option<Value> {
        let conn = self.inner.clone();
        let id = id.to_string();
        tokio::task::spawn_blocking(move || -> Option<Value> {
            let conn = conn.blocking_lock();
            let row = conn.query_row(
                "SELECT id, provider, authType, name, email, priority, isActive, data, createdAt, updatedAt FROM providerConnections WHERE id = ?1",
                rusqlite::params![&id],
                |r| {
                    Ok((
                        r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?,
                        r.get::<_, Option<String>>(3)?, r.get::<_, Option<String>>(4)?,
                        r.get::<_, i64>(5)?, r.get::<_, i64>(6)?,
                        r.get::<_, String>(7)?, r.get::<_, String>(8)?, r.get::<_, String>(9)?,
                    ))
                },
            );
            let (id, provider, auth_type, name, email, priority, is_active, data_json, created, updated) = row.ok()?;
            let mut val: Value = serde_json::from_str(&data_json).unwrap_or_else(|_| Value::Object(Default::default()));
            if let Some(obj) = val.as_object_mut() {
                obj.insert("id".into(), Value::String(id));
                obj.insert("provider".into(), Value::String(provider));
                obj.insert("authType".into(), Value::String(auth_type));
                if let Some(n) = name { obj.insert("name".into(), Value::String(n)); }
                if let Some(e) = email { obj.insert("email".into(), Value::String(e)); }
                obj.insert("priority".into(), Value::Number(priority.into()));
                obj.insert("isActive".into(), Value::Bool(is_active == 1));
                obj.insert("createdAt".into(), Value::String(created));
                obj.insert("updatedAt".into(), Value::String(updated));
            }
            Some(val)
        })
        .await
        .ok()
        .flatten()
    }

    // ============================================================
    // Proxy pools CRUD — parity with Node proxyPoolsRepo. The pool payload
    // lives in the `data` JSON blob: {name, proxyUrl, noProxy, type,
    // strictProxy, lastTestedAt, lastError}.
    // ============================================================

    pub async fn list_proxy_pools(&self) -> Vec<serde_json::Value> {
        let conn = self.inner.clone();
        tokio::task::spawn_blocking(move || -> Vec<serde_json::Value> {
            let conn = conn.blocking_lock();
            let mut stmt = match conn.prepare(
                "SELECT id, isActive, testStatus, data, createdAt, updatedAt FROM proxyPools ORDER BY createdAt ASC",
            ) {
                Ok(s) => s,
                Err(_) => return Vec::new(),
            };
            let rows = stmt
                .query_map([], |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, i64>(1)?,
                        r.get::<_, Option<String>>(2)?,
                        r.get::<_, String>(3)?,
                        r.get::<_, String>(4)?,
                        r.get::<_, String>(5)?,
                    ))
                })
                .ok()
                .into_iter()
                .flatten()
                .filter_map(Result::ok);
            rows.map(|(id, is_active, test_status, data, created, updated)| {
                let mut val: Value = serde_json::from_str(&data).unwrap_or_else(|_| Value::Object(Default::default()));
                if let Some(obj) = val.as_object_mut() {
                    obj.insert("id".into(), Value::String(id));
                    obj.insert("isActive".into(), Value::Bool(is_active == 1));
                    obj.insert("testStatus".into(), test_status.into());
                    obj.insert("createdAt".into(), Value::String(created));
                    obj.insert("updatedAt".into(), Value::String(updated));
                }
                val
            })
            .collect()
        })
        .await
        .unwrap_or_default()
    }

    pub async fn get_proxy_pool(&self, id: &str) -> Option<Value> {
        let conn = self.inner.clone();
        let id = id.to_string();
        tokio::task::spawn_blocking(move || -> Option<Value> {
            let conn = conn.blocking_lock();
            let row = conn
                .query_row(
                    "SELECT isActive, testStatus, data, createdAt, updatedAt FROM proxyPools WHERE id = ?1",
                    rusqlite::params![&id],
                    |r| {
                        Ok((
                            r.get::<_, i64>(0)?,
                            r.get::<_, Option<String>>(1)?,
                            r.get::<_, String>(2)?,
                            r.get::<_, String>(3)?,
                            r.get::<_, String>(4)?,
                        ))
                    },
                )
                .ok()?;
            let (is_active, test_status, data, created, updated) = row;
            let mut val: Value = serde_json::from_str(&data).unwrap_or_else(|_| Value::Object(Default::default()));
            if let Some(obj) = val.as_object_mut() {
                obj.insert("id".into(), Value::String(id));
                obj.insert("isActive".into(), Value::Bool(is_active == 1));
                obj.insert("testStatus".into(), test_status.into());
                obj.insert("createdAt".into(), Value::String(created));
                obj.insert("updatedAt".into(), Value::String(updated));
            }
            Some(val)
        })
        .await
        .ok()
        .flatten()
    }

    /// Create/update a pool from the merged JSON value. Returns the pool id.
    pub async fn upsert_proxy_pool(&self, mut pool: Value) -> anyhow::Result<String> {
        use uuid::Uuid;
        let obj = pool
            .as_object_mut()
            .ok_or_else(|| anyhow::anyhow!("pool body must be an object"))?;
        let id = obj
            .entry("id".to_string())
            .or_insert_with(|| Value::String(Uuid::new_v4().to_string()))
            .as_str()
            .unwrap_or("")
            .to_string();
        anyhow::ensure!(!id.is_empty(), "pool id is required");
        anyhow::ensure!(
            obj.get("proxyUrl").and_then(|v| v.as_str()).is_some_and(|s| !s.trim().is_empty()),
            "proxyUrl is required"
        );
        let now = now_iso8601();
        obj.insert("updatedAt".into(), Value::String(now.clone()));
        let created = obj
            .get("createdAt")
            .and_then(|v| v.as_str())
            .map(String::from)
            .unwrap_or_else(|| now.clone());
        obj.entry("createdAt".to_string())
            .or_insert_with(|| Value::String(created.clone()));
        obj.entry("isActive".to_string()).or_insert(Value::Bool(true));
        obj.entry("testStatus".to_string()).or_insert(Value::String("unknown".into()));
        let is_active = obj.get("isActive").and_then(|v| v.as_bool()).unwrap_or(true);
        let test_status = obj
            .get("testStatus")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();
        let data_json = serde_json::to_string(&pool)?;

        let conn = self.inner.clone();
        let id_ret = id.clone();
        tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
            let conn = conn.blocking_lock();
            conn.execute(
                "INSERT INTO proxyPools(id, isActive, testStatus, data, createdAt, updatedAt)
                 VALUES(?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT(id) DO UPDATE SET isActive=excluded.isActive, testStatus=excluded.testStatus,
                   data=excluded.data, updatedAt=excluded.updatedAt",
                rusqlite::params![id, is_active as i64, test_status, data_json, created, now],
            )?;
            Ok(())
        })
        .await??;
        Ok(id_ret)
    }

    pub async fn delete_proxy_pool(&self, id: &str) -> bool {
        let conn = self.inner.clone();
        let id = id.to_string();
        tokio::task::spawn_blocking(move || {
            let conn = conn.blocking_lock();
            let affected = conn.execute("DELETE FROM proxyPools WHERE id = ?1", rusqlite::params![id])?;
            Ok::<_, rusqlite::Error>(affected > 0)
        })
        .await
        .ok()
        .and_then(|r| r.ok())
        .unwrap_or(false)
    }

    /// Persist a pool test outcome (testStatus/lastError/lastTestedAt).
    pub async fn mark_proxy_pool_tested(&self, id: &str, ok: bool, error: Option<String>) {
        let conn = self.inner.clone();
        let id = id.to_string();
        tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
            let conn = conn.blocking_lock();
            let data: Option<String> = conn
                .query_row("SELECT data FROM proxyPools WHERE id = ?1", rusqlite::params![&id], |r| r.get(0))
                .ok();
            let Some(data) = data else { return Ok(()) };
            let mut val: Value = serde_json::from_str(&data).unwrap_or_else(|_| Value::Object(Default::default()));
            if let Some(obj) = val.as_object_mut() {
                obj.insert("testStatus".into(), Value::String(if ok { "active".into() } else { "error".into() }));
                obj.insert("lastError".into(), error.clone().map(Value::String).unwrap_or(Value::Null));
                obj.insert("lastTestedAt".into(), Value::String(now_iso8601()));
            }
            conn.execute(
                "UPDATE proxyPools SET testStatus = ?1, data = ?2, updatedAt = ?3 WHERE id = ?4",
                rusqlite::params![
                    if ok { "active" } else { "error" },
                    serde_json::to_string(&val)?,
                    now_iso8601(),
                    id
                ],
            )?;
            Ok(())
        })
        .await
        .ok();
    }

    /// Resolve the proxy a connection must use, mirroring chatCore.js:
    /// providerSpecificData.connectionProxyEnabled+connectionProxyUrl wins,
    /// then connectionProxyPoolId → that pool's proxyUrl (isActive only).
    /// Returns Some(proxy_url) or None (direct).
    pub async fn resolve_connection_proxy(&self, connection_id: &str) -> Option<String> {
        let conn_full = self.get_connection_full(connection_id).await?;
        let psd = conn_full.get("providerSpecificData")?.as_object()?;
        if psd.get("connectionProxyEnabled").and_then(|v| v.as_bool()).unwrap_or(false) {
            if let Some(url) = psd.get("connectionProxyUrl").and_then(|v| v.as_str()).filter(|s| !s.trim().is_empty()) {
                return Some(url.trim().to_string());
            }
        }
        let pool_id = psd.get("connectionProxyPoolId").and_then(|v| v.as_str())?;
        if pool_id.is_empty() || pool_id == "none" {
            return None;
        }
        let pool = self.get_proxy_pool(pool_id).await?;
        if pool.get("isActive").and_then(|v| v.as_bool()).unwrap_or(false) {
            pool.get("proxyUrl")
                .and_then(|v| v.as_str())
                .filter(|s| !s.trim().is_empty())
                .map(|s| s.trim().to_string())
        } else {
            None
        }
    }

    // ============================================================
    // Per-connection quota (GET /api/usage/:connectionId). Only providers
    // with a plain authenticated GET are natively supported today
    // (openrouter credits); everything else reports unavailable.
    // ============================================================

    /// Load a connection's credential + provider for quota probing.
    pub async fn connection_for_quota(&self, id: &str) -> Option<(String, String)> {
        let conn = self.get_connection_full(id).await?;
        let provider = conn.get("provider")?.as_str()?.to_string();
        let secret = conn
            .get("apiKey")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .or_else(|| conn.get("accessToken").and_then(|v| v.as_str()))
            .map(|s| s.to_string())?;
        Some((provider, secret))
    }


    /// Persist a completed chat request's usage: usageHistory row + usageDaily
    /// rollup + lifetime counter, in one transaction. Dedup suppresses repeated
    /// stream-completion callbacks. Mirrors SaveChatUsage (chat_writes.go:38).
    pub async fn save_chat_usage(&self, entry: ChatUsageEntry) -> anyhow::Result<()> {
        let conn = self.inner.clone();
        tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
            let mut conn = conn.blocking_lock();
            let tx = conn.transaction()?;
            let timestamp = if entry.timestamp.is_empty() { now_iso8601() } else { entry.timestamp.clone() };
            let status = if entry.status.is_empty() { "ok".to_string() } else { entry.status.clone() };
            let total = if entry.tokens.total == 0 { entry.tokens.prompt + entry.tokens.completion } else { entry.tokens.total };

            let tokens = serde_json::json!({
                "prompt_tokens": entry.tokens.prompt,
                "completion_tokens": entry.tokens.completion,
                "total_tokens": total,
                "cached_tokens": entry.tokens.cached,
                "reasoning_tokens": entry.tokens.reasoning,
                "cache_creation_input_tokens": entry.tokens.cache_creation,
            });
            let tokens_json = serde_json::to_string(&tokens)?;

            // Dedup check (same composite equality as Node).
            let dup: Option<i64> = tx.query_row(
                "SELECT id FROM usageHistory WHERE timestamp = ?1
                 AND COALESCE(provider,'') = COALESCE(?2,'')
                 AND COALESCE(model,'') = COALESCE(?3,'')
                 AND COALESCE(connectionId,'') = COALESCE(?4,'')
                 AND COALESCE(apiKey,'') = COALESCE(?5,'')
                 AND promptTokens = ?6 AND completionTokens = ?7
                 ORDER BY id DESC LIMIT 1",
                rusqlite::params![&timestamp, nullable(&entry.provider), nullable(&entry.model), nullable(&entry.connection_id), nullable(&entry.api_key), entry.tokens.prompt, entry.tokens.completion],
                |r| r.get(0),
            ).optional()?;
            if dup.is_some() {
                tx.commit()?;
                return Ok(());
            }

            tx.execute(
                "INSERT INTO usageHistory(timestamp, provider, model, connectionId, apiKey, endpoint, promptTokens, completionTokens, cost, status, tokens, meta)
                 VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, '{}')",
                rusqlite::params![
                    &timestamp, nullable(&entry.provider), nullable(&entry.model), nullable(&entry.connection_id),
                    nullable(&entry.api_key), nullable(&entry.endpoint), entry.tokens.prompt, entry.tokens.completion,
                    entry.cost, &status, &tokens_json
                ],
            )?;

            // Daily rollup.
            let date_key = local_date_key(&timestamp);
            let mut day: serde_json::Map<String, Value> = tx.query_row(
                "SELECT data FROM usageDaily WHERE dateKey = ?1", rusqlite::params![&date_key], |r| r.get::<_, String>(0),
            )
            .optional()?
            .and_then(|s| serde_json::from_str::<serde_json::Map<String, Value>>(&s).ok())
            .unwrap_or_else(new_usage_day);
            aggregate_chat_usage(&mut day, &entry, total);
            let day_json = serde_json::to_string(&Value::Object(day))?;
            tx.execute(
                "INSERT INTO usageDaily(dateKey, data) VALUES(?1, ?2) ON CONFLICT(dateKey) DO UPDATE SET data = excluded.data",
                rusqlite::params![&date_key, &day_json],
            )?;

            // Lifetime counter.
            let current: String = tx.query_row(
                "SELECT value FROM _meta WHERE key = 'totalRequestsLifetime'", [], |r| r.get::<_, String>(0),
            )
            .optional()?
            .unwrap_or_else(|| "0".to_string());
            let n: i64 = current.parse().unwrap_or(0);
            tx.execute(
                "INSERT INTO _meta(key, value) VALUES('totalRequestsLifetime', ?1) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                rusqlite::params![(n + 1).to_string()],
            )?;

            tx.commit()?;
            Ok(())
        })
        .await??;
        Ok(())
    }

    /// Recent formatted log lines (GET /api/usage/logs). Each line:
    /// "DD-MM-YYYY HH:mm:ss | model | PROVIDER | account | sent | received | status".
    pub async fn recent_logs(&self, limit: i64) -> Vec<String> {
        let conn = self.inner.clone();
        let limit = if limit <= 0 { 200 } else { limit };
        tokio::task::spawn_blocking(move || -> Vec<String> {
            let conn = conn.blocking_lock();
            // Connection names (resolved first to avoid cursor deadlock).
            let mut name_stmt = match conn.prepare("SELECT id, name FROM providerConnections") {
                Ok(s) => s,
                Err(_) => return Vec::new(),
            };
            let names: std::collections::HashMap<String, String> = name_stmt
                .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?.unwrap_or_default())))
                .ok()
                .into_iter()
                .flatten()
                .filter_map(Result::ok)
                .filter(|(_, n)| !n.is_empty())
                .collect();
            drop(name_stmt);

            let mut stmt = match conn.prepare(
                "SELECT timestamp, provider, model, connectionId, promptTokens, completionTokens, status, tokens
                 FROM usageHistory ORDER BY id DESC LIMIT ?1",
            ) {
                Ok(s) => s,
                Err(_) => return Vec::new(),
            };
            let rows = stmt
                .query_map(rusqlite::params![limit], |r| {
                    Ok((
                        r.get::<_, Option<String>>(0)?.unwrap_or_default(),
                        r.get::<_, Option<String>>(1)?.unwrap_or_default(),
                        r.get::<_, Option<String>>(2)?.unwrap_or_default(),
                        r.get::<_, Option<String>>(3)?.unwrap_or_default(),
                        r.get::<_, Option<i64>>(4)?,
                        r.get::<_, Option<i64>>(5)?,
                        r.get::<_, Option<String>>(6)?.unwrap_or_default(),
                        r.get::<_, Option<String>>(7)?.unwrap_or_default(),
                    ))
                })
                .ok()
                .into_iter()
                .flatten()
                .filter_map(Result::ok);
            let mut out = Vec::new();
            for (ts, provider, model, conn_id, prompt, completion, status, tokens_json) in rows {
                let p = if provider.is_empty() { "-".to_string() } else { provider.to_uppercase() };
                let m = if model.is_empty() { "-".to_string() } else { model };
                let account = if conn_id.is_empty() {
                    "-".to_string()
                } else if let Some(n) = names.get(&conn_id) {
                    n.clone()
                } else if conn_id.len() >= 8 {
                    conn_id[..8].to_string()
                } else {
                    conn_id
                };
                let sent = prompt.map(|n| n.to_string())
                    .or_else(|| field_from_tokens(&tokens_json, "prompt_tokens"))
                    .unwrap_or_else(|| "-".into());
                let received = completion.map(|n| n.to_string())
                    .or_else(|| field_from_tokens(&tokens_json, "completion_tokens"))
                    .unwrap_or_else(|| "-".into());
                let date = format_log_date(&ts);
                let st = if status.is_empty() { "-".to_string() } else { status };
                out.push(format!("{date} | {m} | {p} | {account} | {sent} | {received} | {st}"));
            }
            out
        })
        .await
        .unwrap_or_default()
    }

    /// Aggregated usage stats (GET /api/usage/stats?period=). Returns a JSON
    /// object shaped like the Node response: totals + byProvider/byModel +
    /// recentRequests. Live-only fields (pending/activeRequests/errorProvider)
    /// are emitted empty. Period ∈ {today,24h,7d,30d,60d,all}, default 7d.
    pub async fn usage_stats(&self, period: &str) -> serde_json::Value {
        let conn = self.inner.clone();
        let period = period.to_string();
        tokio::task::spawn_blocking(move || -> serde_json::Value {
            let conn = conn.blocking_lock();
            // Determine the cutoff timestamp for the period.
            let now_secs = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0);
            let cutoff_secs = match period.as_str() {
                "today" => Some(now_secs - (now_secs % 86400)),
                "24h" => Some(now_secs - 86400),
                "7d" => Some(now_secs - 7 * 86400),
                "30d" => Some(now_secs - 30 * 86400),
                "60d" => Some(now_secs - 60 * 86400),
                "all" => None,
                _ => Some(now_secs - 7 * 86400), // default 7d
            };
            let cutoff_ts = cutoff_secs.map(|s| iso_from_secs(s));

            // Aggregate usageHistory rows in range.
            let (sql, params): (&str, Vec<Box<dyn rusqlite::ToSql>>) = match &cutoff_ts {
                Some(ts) => (
                    "SELECT provider, model, connectionId, apiKey, endpoint, promptTokens, completionTokens, cost, tokens FROM usageHistory WHERE timestamp >= ?1",
                    vec![Box::new(ts.clone())],
                ),
                None => (
                    "SELECT provider, model, connectionId, apiKey, endpoint, promptTokens, completionTokens, cost, tokens FROM usageHistory",
                    vec![],
                ),
            };
            let mut stmt = match conn.prepare(sql) {
                Ok(s) => s,
                Err(_) => return serde_json::json!({ "totalRequests": 0, "byProvider": {}, "byModel": {}, "recentRequests": [] }),
            };
            let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
            let rows = stmt.query_map(param_refs.as_slice(), |r| {
                Ok((
                    r.get::<_, Option<String>>(0)?.unwrap_or_default(),
                    r.get::<_, Option<String>>(1)?.unwrap_or_default(),
                    r.get::<_, Option<String>>(2)?.unwrap_or_default(),
                    r.get::<_, Option<String>>(3)?.unwrap_or_default(),
                    r.get::<_, Option<String>>(4)?.unwrap_or_default(),
                    r.get::<_, Option<i64>>(5)?.unwrap_or(0),
                    r.get::<_, Option<i64>>(6)?.unwrap_or(0),
                    r.get::<_, Option<f64>>(7)?.unwrap_or(0.0),
                    r.get::<_, Option<String>>(8)?.unwrap_or_default(),
                ))
            });
            let mut total_requests = 0i64;
            let mut total_prompt = 0i64;
            let mut total_completion = 0i64;
            let mut total_cached = 0i64;
            let mut total_cost = 0.0;
            // Collect rows first so the stmt borrow ends before we reuse conn.
            let collected: Vec<(String, String, String, String, String, i64, i64, f64, String)> = if let Ok(rows) = rows {
                rows.flatten()
                    .map(|(provider, model, conn_id, api_key, endpoint, prompt, completion, cost, tokens_json)| {
                        let cached = field_from_tokens(&tokens_json, "cached_tokens").and_then(|s| s.parse::<i64>().ok()).unwrap_or(0);
                        (provider, model, conn_id, api_key, endpoint, prompt, completion, cost, format!("{cached}"))
                    })
                    .collect()
            } else {
                Vec::new()
            };
            // Aggregate into a single day-shaped object, then split out sections.
            let mut day = new_usage_day();
            for (provider, model, conn_id, api_key, endpoint, prompt, completion, cost, cached_s) in &collected {
                let cached: i64 = cached_s.parse().unwrap_or(0);
                total_requests += 1;
                total_prompt += prompt;
                total_completion += completion;
                total_cached += cached;
                total_cost += cost;
                let vals = serde_json::json!({
                    "requests": 1, "promptTokens": prompt, "completionTokens": completion,
                    "cachedTokens": cached, "cost": cost
                });
                if !provider.is_empty() {
                    add_bucket(&mut day, "byProvider", provider, &vals, None);
                }
                let mkey = if provider.is_empty() { model.clone() } else { format!("{model}|{provider}") };
                let meta = serde_json::json!({ "rawModel": model, "provider": provider });
                add_bucket(&mut day, "byModel", &mkey, &vals, Some(&meta));
                if !conn_id.is_empty() {
                    let acct_meta = serde_json::json!({ "provider": provider });
                    add_bucket(&mut day, "byAccount", conn_id, &vals, Some(&acct_meta));
                }
                if !api_key.is_empty() {
                    add_bucket(&mut day, "byApiKey", &format!("{api_key}|{provider}"), &vals, None);
                }
                if !endpoint.is_empty() {
                    let ep_meta = serde_json::json!({ "endpoint": endpoint });
                    add_bucket(&mut day, "byEndpoint", endpoint, &vals, Some(&ep_meta));
                }
            }
            let by_provider = day.remove("byProvider").unwrap_or_else(|| Value::Object(Default::default()));
            let by_model = day.remove("byModel").unwrap_or_else(|| Value::Object(Default::default()));

            // recentRequests (last 20).
            let mut recent = Vec::new();
            if let Ok(mut stmt) = conn.prepare("SELECT timestamp, provider, model, promptTokens, completionTokens, status FROM usageHistory ORDER BY id DESC LIMIT 20") {
                let rows = stmt.query_map([], |r| Ok(serde_json::json!({
                    "timestamp": r.get::<_, Option<String>>(0)?.unwrap_or_default(),
                    "provider": r.get::<_, Option<String>>(1)?.unwrap_or_default(),
                    "model": r.get::<_, Option<String>>(2)?.unwrap_or_default(),
                    "promptTokens": r.get::<_, Option<i64>>(3)?.unwrap_or(0),
                    "completionTokens": r.get::<_, Option<i64>>(4)?.unwrap_or(0),
                    "status": r.get::<_, Option<String>>(5)?.unwrap_or_default(),
                })));
                if let Ok(rows) = rows {
                    recent = rows.flatten().collect();
                }
            }

            serde_json::json!({
                "totalRequests": total_requests,
                "totalPromptTokens": total_prompt,
                "totalCompletionTokens": total_completion,
                "totalCachedTokens": total_cached,
                "totalCost": total_cost,
                "byProvider": by_provider,
                "byModel": by_model,
                "byAccount": day.remove("byAccount").unwrap_or_else(|| Value::Object(Default::default())),
                "byApiKey": day.remove("byApiKey").unwrap_or_else(|| Value::Object(Default::default())),
                "byEndpoint": day.remove("byEndpoint").unwrap_or_else(|| Value::Object(Default::default())),
                "recentRequests": recent,
                "pending": { "byModel": {}, "byAccount": {} },
                "activeRequests": [],
                "errorProvider": ""
            })
        })
        .await
        .unwrap_or_else(|_| serde_json::json!({ "totalRequests": 0 }))
    }

    /// Per-day usage series for the dashboard chart (GET /api/usage/chart).
    /// Buckets usageHistory rows by local date within the period window.
    pub async fn usage_chart(&self, period: &str) -> serde_json::Value {
        let conn = self.inner.clone();
        let period = period.to_string();
        tokio::task::spawn_blocking(move || -> serde_json::Value {
            let conn = conn.blocking_lock();
            let now_secs = chrono_now_secs();
            let cutoff_secs = match period.as_str() {
                "today" => Some(now_secs - (now_secs % 86400)),
                "24h" => Some(now_secs - 86400),
                "7d" => Some(now_secs - 7 * 86400),
                "30d" => Some(now_secs - 30 * 86400),
                "60d" => Some(now_secs - 60 * 86400),
                _ => Some(now_secs - 7 * 86400),
            };
            let (sql, params): (&str, Vec<Box<dyn rusqlite::ToSql>>) = match &cutoff_secs {
                Some(secs) => (
                    "SELECT timestamp, promptTokens, completionTokens, cost FROM usageHistory WHERE timestamp >= ?1",
                    vec![Box::new(iso_from_secs(*secs))],
                ),
                None => (
                    "SELECT timestamp, promptTokens, completionTokens, cost FROM usageHistory",
                    vec![],
                ),
            };
            let mut stmt = match conn.prepare(sql) {
                Ok(s) => s,
                Err(_) => return serde_json::json!({ "series": [] }),
            };
            let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
            let rows = stmt.query_map(param_refs.as_slice(), |r| {
                Ok((
                    r.get::<_, Option<String>>(0)?.unwrap_or_default(),
                    r.get::<_, Option<i64>>(1)?.unwrap_or(0),
                    r.get::<_, Option<i64>>(2)?.unwrap_or(0),
                    r.get::<_, Option<f64>>(3)?.unwrap_or(0.0),
                ))
            });
            // dateKey → [requests, prompt, completion, cost]
            let mut buckets: std::collections::BTreeMap<String, (i64, i64, i64, f64)> = Default::default();
            if let Ok(rows) = rows {
                for (ts, prompt, completion, cost) in rows.flatten() {
                    let key = local_date_key(&ts);
                    let e = buckets.entry(key).or_insert((0, 0, 0, 0.0));
                    e.0 += 1;
                    e.1 += prompt;
                    e.2 += completion;
                    e.3 += cost;
                }
            }
            let series: Vec<serde_json::Value> = buckets
                .into_iter()
                .map(|(date, (requests, prompt, completion, cost))| {
                    serde_json::json!({
                        "date": date, "requests": requests,
                        "promptTokens": prompt, "completionTokens": completion, "cost": cost,
                    })
                })
                .collect();
            serde_json::json!({ "series": series })
        })
        .await
        .unwrap_or_else(|_| serde_json::json!({ "series": [] }))
    }
}

#[derive(Debug, Clone)]
pub struct Credential {
    pub secret: String,
    pub connection_id: String,
}

// ============================================================
// Connection create input (M6)
// ============================================================

#[derive(Debug, Clone)]
pub struct CreateConnection {
    pub provider: String,
    pub auth_type: String, // "apikey" | "cookie" | "access_token"
    pub name: String,
    pub api_key: String,
    pub priority: Option<i64>,
    pub is_active: bool,
    pub test_status: String,
    pub email: Option<String>,
    pub provider_specific_data: Option<Value>,
    pub extra: serde_json::Map<String, Value>,
}

// ============================================================
// Usage types + helpers (M7) — port of chat_writes.go / usage.go
// ============================================================

#[derive(Debug, Clone, Default)]
pub struct ChatUsage {
    pub prompt: i64,
    pub completion: i64,
    pub total: i64,
    pub cached: i64,
    pub reasoning: i64,
    pub cache_creation: i64,
}

#[derive(Debug, Clone)]
pub struct ChatUsageEntry {
    pub timestamp: String,
    pub provider: String,
    pub model: String,
    pub connection_id: String,
    pub api_key: String,
    pub endpoint: String,
    pub status: String,
    pub cost: f64,
    pub tokens: ChatUsage,
}

/// Renumber priorities 1..N for one provider (sorted by priority, then updatedAt
/// desc), inside an existing transaction. Mirrors connectionsRepo reorderInTx.
fn reorder_priorities(tx: &rusqlite::Transaction, provider: &str) -> rusqlite::Result<()> {
    let mut stmt = tx.prepare("SELECT id FROM providerConnections WHERE provider = ?1 ORDER BY priority ASC, updatedAt DESC")?;
    let ids: Vec<String> = stmt
        .query_map(rusqlite::params![provider], |r| r.get::<_, String>(0))?
        .filter_map(Result::ok)
        .collect();
    drop(stmt);
    for (i, id) in ids.iter().enumerate() {
        tx.execute(
            "UPDATE providerConnections SET priority = ?1 WHERE id = ?2",
            rusqlite::params![(i + 1) as i64, id],
        )?;
    }
    Ok(())
}

/// "" → NULL for COALESCE-friendly SQL params.
fn nullable(s: &str) -> Option<&str> {
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

/// Local-zone date key "YYYY-MM-DD" for the daily rollup (usageDaily.dateKey).
/// We use the system local date to match Node's getLocalDateKey behavior.
fn local_date_key(iso_ts: &str) -> String {
    let secs = parse_rfc3339_secs(iso_ts).unwrap_or_else(|| chrono_now_secs());
    // Convert UTC secs to local. Node uses local date; we approximate with the
    // UTC date + local offset. For simplicity (and since usageDaily is an
    // internal rollup), we use the UTC date key. This keeps totals correct;
    // day boundaries may differ by the local TZ offset.
    let (y, m, d, _, _, _) = unix_to_civil(secs.max(0) as u64);
    format!("{y:04}-{m:02}-{d:02}")
}

fn new_usage_day() -> serde_json::Map<String, Value> {
    let mut m = serde_json::Map::new();
    m.insert("requests".into(), Value::Number(0.into()));
    m.insert("promptTokens".into(), Value::Number(0.into()));
    m.insert("completionTokens".into(), Value::Number(0.into()));
    m.insert("cachedTokens".into(), Value::Number(0.into()));
    m.insert("cost".into(), num_f64(0.0));
    for k in [
        "byProvider",
        "byModel",
        "byAccount",
        "byApiKey",
        "byEndpoint",
    ] {
        m.insert(k.into(), Value::Object(Default::default()));
    }
    m
}

/// serde_json Number from f64 (falls back to 0 if NaN/inf).
fn num_f64(v: f64) -> Value {
    Value::Number(serde_json::Number::from_f64(v).unwrap_or_else(|| serde_json::Number::from(0)))
}

/// Aggregate one entry into a daily rollup object (chat_writes.go aggregateChatUsage).
fn aggregate_chat_usage(day: &mut serde_json::Map<String, Value>, e: &ChatUsageEntry, total: i64) {
    add_num(day, "requests", 1.0);
    add_num(day, "promptTokens", e.tokens.prompt as f64);
    add_num(day, "completionTokens", e.tokens.completion as f64);
    add_num(day, "cachedTokens", e.tokens.cached as f64);
    add_num(day, "cost", e.cost);
    let vals = serde_json::json!({
        "requests": 1, "promptTokens": e.tokens.prompt, "completionTokens": e.tokens.completion,
        "cachedTokens": e.tokens.cached, "cost": e.cost
    });
    if !e.provider.is_empty() {
        add_bucket(day, "byProvider", &e.provider, &vals, None);
    }
    let model_key = if e.provider.is_empty() {
        e.model.clone()
    } else {
        format!("{}|{}", e.model, e.provider)
    };
    let model_meta = serde_json::json!({ "rawModel": e.model, "provider": e.provider });
    add_bucket(day, "byModel", &model_key, &vals, Some(&model_meta));
    if !e.connection_id.is_empty() {
        let acct_meta = serde_json::json!({ "rawModel": e.model, "provider": e.provider });
        add_bucket(day, "byAccount", &e.connection_id, &vals, Some(&acct_meta));
    }
    let api_key = if e.api_key.is_empty() {
        "local-no-key".to_string()
    } else {
        e.api_key.clone()
    };
    let ak_meta = serde_json::json!({ "rawModel": e.model, "provider": e.provider });
    add_bucket(
        day,
        "byApiKey",
        &format!(
            "{api_key}|{}|{}",
            e.model,
            if e.provider.is_empty() {
                "unknown"
            } else {
                &e.provider
            }
        ),
        &vals,
        Some(&ak_meta),
    );
    let endpoint = if e.endpoint.is_empty() {
        "Unknown"
    } else {
        &e.endpoint
    };
    let ep_meta =
        serde_json::json!({ "endpoint": endpoint, "rawModel": e.model, "provider": e.provider });
    add_bucket(
        day,
        "byEndpoint",
        &format!(
            "{endpoint}|{}|{}",
            e.model,
            if e.provider.is_empty() {
                "unknown"
            } else {
                &e.provider
            }
        ),
        &vals,
        Some(&ep_meta),
    );
    let _ = total;
}

fn add_num(m: &mut serde_json::Map<String, Value>, key: &str, delta: f64) {
    let cur = m.get(key).and_then(|v| v.as_f64()).unwrap_or(0.0);
    m.insert(key.into(), num_f64(cur + delta));
}

fn add_bucket(
    day: &mut serde_json::Map<String, Value>,
    section: &str,
    key: &str,
    vals: &Value,
    meta: Option<&Value>,
) {
    let section_obj = day
        .entry(section.to_string())
        .or_insert_with(|| Value::Object(Default::default()));
    let obj = section_obj
        .as_object_mut()
        .expect("usage day section is object");
    let bucket = obj
        .entry(key.to_string())
        .or_insert_with(|| Value::Object(Default::default()));
    let b = bucket.as_object_mut().expect("bucket is object");
    for field in [
        "requests",
        "promptTokens",
        "completionTokens",
        "cachedTokens",
        "cost",
    ] {
        let delta = vals.get(field).and_then(|v| v.as_f64()).unwrap_or(0.0);
        let cur = b.get(field).and_then(|v| v.as_f64()).unwrap_or(0.0);
        b.insert(field.into(), num_f64(cur + delta));
    }
    if let Some(meta_obj) = meta.and_then(|v| v.as_object()) {
        for (k, v) in meta_obj {
            b.insert(k.clone(), v.clone());
        }
    }
}

/// Extract a numeric field from a tokens JSON string (fallback for NULL columns).
fn field_from_tokens(tokens_json: &str, key: &str) -> Option<String> {
    let v: Value = serde_json::from_str(tokens_json).ok()?;
    let n = v.get(key).and_then(|v| v.as_f64())?;
    Some((n as i64).to_string())
}

/// Format a usage log timestamp as "DD-MM-YYYY HH:mm:ss" (local-ish).
fn format_log_date(iso: &str) -> String {
    let secs = match parse_rfc3339_secs(iso) {
        Some(s) => s,
        None => return iso.to_string(),
    };
    let (y, mo, d, h, mi, s) = unix_to_civil(secs.max(0) as u64);
    format!("{d:02}-{mo:02}-{y:04} {h:02}:{mi:02}:{s:02}")
}

/// Unix secs → ISO 8601 UTC "YYYY-MM-DDTHH:MM:SS.000Z".
fn iso_from_secs(secs: i64) -> String {
    let (y, mo, d, h, mi, s) = unix_to_civil(secs.max(0) as u64);
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{s:02}.000Z")
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

/// Best-effort RFC3339 → unix seconds (crate-public for the oauth module).
pub fn parse_rfc3339_secs_pub(s: &str) -> Option<i64> {
    parse_rfc3339_secs(s)
}

/// Unix secs → ISO 8601 UTC (crate-public for the oauth module).
pub fn iso_from_secs_pub(secs: i64) -> String {
    iso_from_secs(secs)
}

fn chrono_now_secs() -> i64 {    std::time::SystemTime::now()
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
    if d.len() != 3 {
        return None;
    }
    // time portion up to an optional offset
    let (timepart, offset) = rest
        .split_once(|c: char| c == 'Z' || c == '+' || c == '-')
        .unwrap_or((rest, ""));
    let t: Vec<u64> = timepart
        .split(':')
        .filter_map(|x| x.split('.').next()?.parse().ok())
        .collect();
    if t.len() < 3 {
        return None;
    }
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
        let parts: Vec<&str> = offset
            .trim_start_matches(|c: char| c == '+' || c == '-')
            .split(':')
            .collect();
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
    set(
        "mitmRouterBaseUrl",
        Value::String("http://localhost:20128".into()),
    );
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
                let url = merged
                    .get("outboundProxyUrl")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
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
        let oidc_issuer = obj
            .get("oidcIssuerUrl")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let oidc_client = obj
            .get("oidcClientId")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let has_password = merged.get("password").and_then(|v| v.as_str()).is_some();
        obj.insert(
            "oidcConfigured".into(),
            Value::Bool(
                !oidc_issuer.is_empty()
                    && !oidc_client.is_empty()
                    && oidc_secret.as_str().is_some(),
            ),
        );
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
    let key_id: String = (0..6)
        .map(|_| CHARS[rng.gen_range(0..CHARS.len())] as char)
        .collect();
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
pub fn unix_to_civil(mut secs: u64) -> (i64, u32, u32, u32, u32, u32) {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_db() -> (Db, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!("orouter-rust-test-{}", uuid::Uuid::new_v4()));
        let path = dir.join("db").join("data.sqlite");
        (Db::open(&path).expect("open fresh DB"), dir)
    }

    #[test]
    fn parses_fractional_rfc3339() {
        assert_eq!(
            parse_rfc3339_secs("2026-08-08T16:41:12.449Z"),
            parse_rfc3339_secs("2026-08-08T16:41:12Z")
        );
    }

    #[test]
    fn bootstraps_fresh_schema() {
        let (db, dir) = temp_db();
        let path = dir.join("db").join("data.sqlite");
        let check = Connection::open(&path).expect("reopen DB");
        let tables: i64 = check
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name IN ('settings','apiKeys','providerConnections','usageHistory','usageDaily','_meta')",
                [],
                |r| r.get(0),
            )
            .expect("count schema tables");
        let settings: String = check
            .query_row("SELECT data FROM settings WHERE id=1", [], |r| r.get(0))
            .expect("seed settings");
        assert_eq!(tables, 6);
        assert_eq!(
            serde_json::from_str::<Value>(&settings).unwrap()["requireApiKey"],
            true
        );
        drop(check);
        drop(db);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn usage_round_trip() {
        let (db, dir) = temp_db();
        db.save_chat_usage(ChatUsageEntry {
            timestamp: "2026-08-08T16:41:12.449Z".into(),
            provider: "openrouter".into(),
            model: "openai/gpt-test".into(),
            connection_id: "conn-1".into(),
            api_key: "sk-test".into(),
            endpoint: "/v1/chat/completions".into(),
            status: "ok".into(),
            cost: 0.0,
            tokens: ChatUsage {
                prompt: 12,
                completion: 4,
                total: 16,
                cached: 2,
                ..Default::default()
            },
        })
        .await
        .expect("save usage");

        let stats = db.usage_stats("all").await;
        assert_eq!(stats["totalRequests"], 1);
        assert_eq!(stats["totalPromptTokens"], 12);
        assert_eq!(stats["totalCompletionTokens"], 4);
        assert_eq!(stats["totalCachedTokens"], 2);
        assert_eq!(db.recent_logs(10).await.len(), 1);

        drop(db);
        let _ = std::fs::remove_dir_all(dir);
    }
}
