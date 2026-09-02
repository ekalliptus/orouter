//! Model management store — native parity with the Node repos that persist
//! into the shared SQLite `kv` table:
//!   modelAliases   key=alias                    → JSON string "provider/model"
//!   customModels   key="providerAlias|id|type"  → JSON model object
//!   disabledModels key=providerAlias            → JSON array of model ids
//! The chat path consults aliases + disabled sets, so Node and Rust stay
//! interchangeable on the same database.

use serde_json::{json, Value};

use crate::db::Db;

pub async fn aliases(db: &Db) -> Value {
    db.kv_get_all("modelAliases").await
}

pub async fn set_alias(db: &Db, alias: &str, model: &str) -> anyhow::Result<()> {
    anyhow::ensure!(!alias.trim().is_empty(), "alias is required");
    anyhow::ensure!(!model.trim().is_empty(), "model is required");
    db.kv_set("modelAliases", alias.trim(), &json!(model.trim())).await
}

pub async fn remove_alias(db: &Db, alias: &str) {
    db.kv_remove("modelAliases", alias).await;
}

pub async fn custom_all(db: &Db) -> Value {
    db.kv_get_all("customModels").await
}

pub async fn set_custom(db: &Db, provider_alias: &str, id: &str, model_type: &str, mut model: Value) -> anyhow::Result<()> {
    anyhow::ensure!(!provider_alias.trim().is_empty(), "providerAlias is required");
    anyhow::ensure!(!id.trim().is_empty(), "id is required");
    let key = format!("{provider_alias}|{id}|{model_type}");
    if let Some(o) = model.as_object_mut() {
        o.entry("providerAlias".to_string()).or_insert_with(|| json!(provider_alias));
        o.entry("id".to_string()).or_insert_with(|| json!(id));
        o.entry("type".to_string()).or_insert_with(|| json!(model_type));
    }
    db.kv_set("customModels", &key, &model).await
}

pub async fn remove_custom(db: &Db, provider_alias: &str, id: &str, model_type: &str) {
    let key = format!("{provider_alias}|{id}|{model_type}");
    db.kv_remove("customModels", &key).await;
}

/// providerAlias → [disabled model ids]
pub async fn disabled_all(db: &Db) -> Value {
    db.kv_get_all("disabledModels").await
}

pub async fn disabled_for(db: &Db, provider_alias: &str) -> Vec<String> {
    db.kv_get_array("disabledModels", provider_alias).await
}

pub async fn disable_models(db: &Db, provider_alias: &str, ids: &[String]) {
    let mut current = db.kv_get_array("disabledModels", provider_alias).await;
    for id in ids {
        if !current.contains(id) {
            current.push(id.clone());
        }
    }
    let _ = db.kv_set("disabledModels", provider_alias, &json!(current)).await;
}

pub async fn enable_models(db: &Db, provider_alias: &str, ids: &[String]) {
    let mut current = db.kv_get_array("disabledModels", provider_alias).await;
    current.retain(|m| !ids.contains(m));
    let _ = db.kv_set("disabledModels", provider_alias, &json!(current)).await;
}

pub async fn enable_all_for(db: &Db, provider_alias: &str) {
    let _ = db.kv_set("disabledModels", provider_alias, &json!([])).await;
}

/// Resolve an alias chain (max 3 hops) to its final model string.
/// Returns None when the model is not an alias.
pub async fn resolve_alias(db: &Db, model: &str) -> Option<String> {
    let mut current = model.to_string();
    for _ in 0..3 {
        let all = aliases(db).await;
        let Some(next) = all.get(&current).and_then(|v| v.as_str()).map(String::from) else {
            if current == model {
                return None;
            }
            return Some(current);
        };
        current = next;
    }
    Some(current)
}

/// Is "provider/model" (or provider + bare id) disabled?
pub async fn is_disabled(db: &Db, provider: &str, model_id: &str) -> bool {
    let disabled = disabled_for(db, provider).await;
    disabled.iter().any(|m| m == model_id)
}
