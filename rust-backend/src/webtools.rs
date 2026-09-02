//! Native web tools: /v1/web/fetch (jina-reader / firecrawl / tavily) and
//! /v1/search (searxng / tavily / brave-search). Ports the request shapes of
//! open-sse/handlers/fetch/index.js + search/callers.js, reading provider
//! credentials from the shared connections table.
//!
//! SSRF policy: the caller-supplied target URL must be http/https with a
//! public host — localhost, private, link-local and reserved ranges are
//! rejected before any request leaves the box.

use std::net::{IpAddr, ToSocketAddrs};
use std::time::Duration;

use serde_json::{json, Value};

use crate::db::Db;

// re-export reqwest's Url so the SSRF guard needs no extra dependency
use reqwest::Url;

// ---- SSRF guard -----------------------------------------------------------

pub fn validate_public_url(raw: &str) -> Result<reqwest::Url, String> {
    let parsed = reqwest::Url::parse(raw).map_err(|_| format!("invalid URL: {raw}"))?;
    match parsed.scheme() {
        "http" | "https" => {}
        other => return Err(format!("scheme '{other}' not allowed (http/https only)")),
    }
    let host = parsed
        .host_str()
        .ok_or_else(|| "URL has no host".to_string())?
        .to_lowercase();
    if host == "localhost" || host.ends_with(".localhost") || host.ends_with(".local") || host.ends_with(".internal") {
        return Err(format!("host '{host}' is not allowed"));
    }
    // Validate every resolved address (DNS pinning guard).
    let addrs: Vec<IpAddr> = if let Ok(ip) = host.parse::<IpAddr>() {
        vec![ip]
    } else {
        (host.as_str(), parsed.port_or_known_default().unwrap_or(80))
            .to_socket_addrs()
            .map_err(|e| format!("DNS resolve failed for '{host}': {e}"))?
            .map(|a| a.ip())
            .collect()
    };
    for ip in &addrs {
        if is_disallowed_ip(ip) {
            return Err(format!("host '{host}' resolves to a non-public address ({ip})"));
        }
    }
    Ok(parsed)
}

fn is_disallowed_ip(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let o = v4.octets();
            o[0] == 0                                   // this-network
                || o[0] == 10                            // private
                || o[0] == 127                           // loopback
                || (o[0] == 169 && o[1] == 254)          // link-local
                || (o[0] == 172 && (16..=31).contains(&o[1])) // private
                || (o[0] == 192 && o[1] == 168)          // private
                || (o[0] == 100 && (64..=127).contains(&o[1])) // CGNAT
                || (o[0] == 192 && o[1] == 0 && o[2] == 2)     // TEST-NET
                || (o[0] == 198 && (o[1] == 18 || o[1] == 19)) // benchmarking
                || o[0] >= 224                           // multicast + reserved
        }
        IpAddr::V6(v6) => {
            let s = v6.segments();
            v6.is_loopback()
                || (s[0] & 0xfe00) == 0xfc00  // ULA
                || (s[0] & 0xffc0) == 0xfe80  // link-local
                || s[0] == 0 && s[1] == 0 && s[2] == 0 && s[3] == 0 && s[4] == 0 && s[5] == 0 && s[6] == 0 && s[7] == 1
                || (s[0] & 0xff00) == 0xff00  // multicast
        }
    }
}

// ---- Credential lookup ----------------------------------------------------

async fn web_credential(db: &Db, provider: &str) -> Option<String> {
    let conns = db.connections_for_provider(provider, "").await;
    for c in &conns {
        if let Some(k) = c.get("apiKey").and_then(|v| v.as_str()).filter(|s| !s.is_empty()) {
            return Some(k.to_string());
        }
        if let Some(k) = c.get("accessToken").and_then(|v| v.as_str()).filter(|s| !s.is_empty()) {
            return Some(k.to_string());
        }
    }
    // searxng often runs keyless; a connection may still carry its baseUrl.
    if let Some(c) = conns.first() {
        for key in ["baseUrl", "url", "searxngUrl"] {
            if let Some(u) = c.get(key).and_then(|v| v.as_str()).filter(|s| !s.is_empty()) {
                return Some(format!("__url__:{u}"));
            }
        }
    }
    None
}

fn truncate(s: &str, n: usize) -> String {
    if s.len() <= n { s.to_string() } else { format!("{}…", &s[..n]) }
}

// ---- Web fetch ------------------------------------------------------------

/// POST /v1/web/fetch  body: { provider | model, url, format?, maxCharacters? }
pub async fn web_fetch(db: &Db, http: &reqwest::Client, body: &Value) -> Value {
    let provider = body
        .get("provider")
        .or_else(|| body.get("model"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let target = body.get("url").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let fmt = body.get("format").and_then(|v| v.as_str()).unwrap_or("markdown").to_string();
    let max_characters = body.get("maxCharacters").and_then(|v| v.as_u64()).unwrap_or(200_000) as usize;

    if provider.is_empty() || target.is_empty() {
        return json!({ "success": false, "status": 400, "error": "provider/model and url are required" });
    }
    if let Err(e) = validate_public_url(&target) {
        return json!({ "success": false, "status": 400, "error": e });
    }

    let Some(key_or_url) = web_credential(db, &provider).await else {
        return json!({ "success": false, "status": 400, "error": format!("no connection configured for web provider '{provider}'") });
    };
    let (api_key, base_override) = if let Some(u) = key_or_url.strip_prefix("__url__:") {
        (String::new(), Some(u.to_string()))
    } else {
        (key_or_url, None)
    };

    let client_req = |method: reqwest::Method, url: String| -> reqwest::RequestBuilder {
        let mut r = http.request(method, url).timeout(Duration::from_secs(30));
        if !api_key.is_empty() {
            r = r.header("authorization", format!("Bearer {api_key}"));
        }
        r
    };

    let outcome = match provider.as_str() {
        "firecrawl" => {
            let resp = client_req(reqwest::Method::POST, "https://api.firecrawl.dev/v1/scrape".into())
                .header("content-type", "application/json")
                .json(&json!({ "url": target, "formats": [fmt] }))
                .send()
                .await;
            handle_json(resp, |v| {
                let d = v.get("data").cloned().unwrap_or(json!({}));
                let text = d.get("markdown").or_else(|| d.get("html")).or_else(|| d.get("text"))
                    .and_then(|t| t.as_str()).map(|s| truncate(s, max_characters));
                (d.pointer("/metadata/title").cloned().unwrap_or(Value::Null), text)
            }).await
        }
        "jina-reader" => {
            let resp = client_req(reqwest::Method::POST, "https://r.jina.ai/".into())
                .header("content-type", "application/json")
                .json(&json!({ "url": target }))
                .send()
                .await;
            match resp {
                Ok(r) => {
                    let status = r.status();
                    let text = r.text().await.unwrap_or_default();
                    if !status.is_success() {
                        json!({ "success": false, "status": status.as_u16(), "error": truncate(&text, 300) })
                    } else {
                        let title = text.lines().find_map(|l| l.strip_prefix("Title:")).map(|t| t.trim().to_string());
                        json!({ "success": true, "data": { "provider": "jina-reader", "url": target, "title": title, "format": fmt, "text": truncate(&text, max_characters) } })
                    }
                }
                Err(e) => json!({ "success": false, "status": 502, "error": format!("jina: {e}") }),
            }
        }
        "tavily" => {
            let resp = client_req(reqwest::Method::POST, "https://api.tavily.com/extract".into())
                .header("content-type", "application/json")
                .json(&json!({ "urls": [target], "extract_depth": "basic" }))
                .send()
                .await;
            handle_json(resp, |v| {
                let first = v.pointer("/results/0").cloned().unwrap_or(json!({}));
                let text = first.get("raw_content").and_then(|t| t.as_str()).map(|s| truncate(s, max_characters));
                (first.get("title").cloned().unwrap_or(Value::Null), text)
            }).await
        }
        other => json!({ "success": false, "status": 400, "error": format!("unsupported fetch provider: {other} (native: firecrawl, jina-reader, tavily)") }),
    };

    match base_override {
        Some(_) => json!({ "success": false, "status": 400, "error": "searxng is a search provider, not a fetch provider" }),
        None => outcome,
    }
}

async fn handle_json(
    resp: Result<reqwest::Response, reqwest::Error>,
    extract: impl Fn(Value) -> (Value, Option<String>),
) -> Value {
    let Ok(r) = resp else {
        return json!({ "success": false, "status": 502, "error": "upstream unreachable" });
    };
    let status = r.status();
    let text = r.text().await.unwrap_or_default();
    if !status.is_success() {
        return json!({ "success": false, "status": status.as_u16(), "error": truncate(&text, 300) });
    }
    let v: Value = serde_json::from_str(&text).unwrap_or(json!({}));
    let (title, maybe_text) = extract(v);
    match maybe_text {
        Some(text) => json!({ "success": true, "data": { "title": title, "text": text } }),
        None => json!({ "success": false, "status": 502, "error": "no content in upstream response" }),
    }
}

// ---- Web search -----------------------------------------------------------

/// POST /v1/search  body: { provider | model, query, maxResults?, searchType? }
pub async fn web_search(db: &Db, http: &reqwest::Client, body: &Value) -> Value {
    let provider = body
        .get("provider")
        .or_else(|| body.get("model"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let query = body.get("query").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let max_results = body.get("maxResults").and_then(|v| v.as_u64()).unwrap_or(5);

    if provider.is_empty() || query.is_empty() {
        return json!({ "error": "provider/model and query are required" });
    }

    let Some(key_or_url) = web_credential(db, &provider).await else {
        return json!({ "error": format!("no connection configured for search provider '{provider}'") });
    };
    let (token, base_override) = if let Some(u) = key_or_url.strip_prefix("__url__:") {
        (String::new(), Some(u.to_string()))
    } else {
        (key_or_url, None)
    };

    match provider.as_str() {
        "searxng" => {
            let Some(base) = base_override else {
                return json!({ "error": "searxng connection must carry its instance baseUrl" });
            };
            let url = if base.ends_with("/search") { base } else { format!("{base}/search") };
            let resp = http
                .get(&url)
                .query(&[("q", query.as_str()), ("format", "json"), ("categories", "general")])
                .header("accept", "application/json")
                .timeout(Duration::from_secs(20))
                .send()
                .await;
            normalize_results(resp, "searxng", |v| {
                v.get("results")
                    .and_then(|r| r.as_array())
                    .map(|arr| {
                        arr.iter()
                            .take(max_results as usize)
                            .filter_map(|r| {
                                Some(json!({
                                    "title": r.get("title")?,
                                    "url": r.get("url")?,
                                    "snippet": r.get("content").cloned().unwrap_or(Value::Null),
                                }))
                            })
                            .collect()
                    })
            })
            .await
        }
        "tavily" => {
            let resp = http
                .post("https://api.tavily.com/search")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {token}"))
                .json(&json!({ "query": query, "max_results": max_results }))
                .timeout(Duration::from_secs(20))
                .send()
                .await;
            normalize_results(resp, "tavily", |v| {
                v.get("results")
                    .and_then(|r| r.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|r| {
                                Some(json!({
                                    "title": r.get("title")?,
                                    "url": r.get("url")?,
                                    "snippet": r.get("content").cloned().unwrap_or(Value::Null),
                                }))
                            })
                            .collect()
                    })
            })
            .await
        }
        "brave-search" => {
            let resp = http
                .get("https://api.search.brave.com/res/v1/web/search")
                .query(&[("q", query.as_str()), ("count", max_results.to_string().as_str())])
                .header("accept", "application/json")
                .header("x-subscription-token", token)
                .timeout(Duration::from_secs(20))
                .send()
                .await;
            normalize_results(resp, "brave-search", |v| {
                v.pointer("/web/results")
                    .and_then(|r| r.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|r| {
                                Some(json!({
                                    "title": r.get("title")?,
                                    "url": r.get("url")?,
                                    "snippet": r.get("description").cloned().unwrap_or(Value::Null),
                                }))
                            })
                            .collect()
                    })
            })
            .await
        }
        other => json!({ "error": format!("unsupported search provider: {other} (native: searxng, tavily, brave-search)") }),
    }
}

async fn normalize_results(
    resp: Result<reqwest::Response, reqwest::Error>,
    provider: &str,
    map: impl Fn(Value) -> Option<Vec<Value>>,
) -> Value {
    let Ok(r) = resp else {
        return json!({ "error": format!("{provider}: upstream unreachable") });
    };
    let status = r.status();
    let text = r.text().await.unwrap_or_default();
    if !status.is_success() {
        return json!({ "error": format!("{provider}: HTTP {} {}", status.as_u16(), truncate(&text, 200)) });
    }
    let v: Value = serde_json::from_str(&text).unwrap_or(json!({}));
    match map(v) {
        Some(results) => json!({ "provider": provider, "results": results }),
        None => json!({ "error": format!("{provider}: unparseable response") }),
    }
}
