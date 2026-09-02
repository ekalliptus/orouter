//! Native Kiro (AWS CodeWhisperer) client — refresh, EventStream framing,
//! chat completion with SSE. Direct port of open-sse/services/kiro* and the
//! executor's request shape and EventStream binary protocol used by
//! GenerateAssistantResponse.
//!
//! Wire format: each EventStream frame is
//!   prelude (12B) | headers (varint) | payload (varint) | CRC32 (4B)
//! Headers are 1-byte type + 2-byte name length + name + value.
//! We write the request as one JSON frame; the response is a stream of
//! event frames we translate back to OpenAI-shaped SSE chunks via
//! proxy::kiro_sse::SseState.

use std::collections::HashMap;
use std::time::Duration;

use reqwest::Client;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::db::Db;
use crate::proxy::kiro_sse::{default_profile_arn, sse_chunk, SseState, StreamReader};

const KIRO_ENDPOINTS: &[&str] = &[
    "https://runtime.us-east-1.kiro.dev/generateAssistantResponse",
    "https://codewhisperer.us-east-1.amazonaws.com/generateAssistantResponse",
    "https://q.us-east-1.amazonaws.com/generateAssistantResponse",
];
const KIRO_REFRESH_URL: &str = "https://prod.us-east-1.auth.desktop.kiro.dev/refreshToken";
const KIRO_OIDC_REFRESH_DEFAULT: &str = "https://oidc.us-east-1.amazonaws.com/token";
const KIRO_DEFAULT_HEADERS: &[(&str, &str)] = &[
    ("Content-Type", "application/json"),
    ("Accept", "application/vnd.amazon.eventstream"),
    ("User-Agent", "AWS-SDK-JS/3.0.0 kiro-ide/1.0.0"),
    ("X-Amz-User-Agent", "aws-sdk-js/3.0.0 kiro-ide/1.0.0"),
];
const KIRO_CODEWHISPERER_TARGET: &str =
    "AmazonCodeWhispererStreamingService.GenerateAssistantResponse";

// ===================================================================
// EventStream frame writer (request body) + CRC32
// ===================================================================

const CRC32_TABLE: [u32; 256] = make_crc32_table();
const fn make_crc32_table() -> [u32; 256] {
    let mut t = [0u32; 256];
    let mut i = 0;
    while i < 256 {
        let mut v = i as u32;
        let mut bit = 0;
        while bit < 8 {
            v = (v >> 1) ^ if v & 1 != 0 { 0xedb88320 } else { 0 };
            bit += 1;
        }
        t[i] = v;
        i += 1;
    }
    t
}
fn crc32(buf: &[u8]) -> u32 {
    let mut crc: u32 = 0xffffffff;
    for &b in buf {
        crc = CRC32_TABLE[((crc ^ b as u32) & 0xff) as usize] ^ (crc >> 8);
    }
    !crc
}
fn write_varint(out: &mut Vec<u8>, mut n: usize) {
    while n >= 0x80 {
        out.push((n as u8 & 0x7f) | 0x80);
        n >>= 7;
    }
    out.push(n as u8);
}

pub fn build_request_body(payload_json: &str) -> Vec<u8> {
    let payload = payload_json.as_bytes();
    let mut headers_buf: Vec<u8> = Vec::new();
    for (name, value) in &[(":content-type", "application/json"), (":event-type", "userInputMessage")] {
        let n = name.as_bytes();
        let v = value.as_bytes();
        headers_buf.push(0x00);
        headers_buf.push((n.len() >> 8) as u8);
        headers_buf.push((n.len() & 0xff) as u8);
        headers_buf.extend_from_slice(n);
        headers_buf.push((v.len() >> 8) as u8);
        headers_buf.push((v.len() & 0xff) as u8);
        headers_buf.extend_from_slice(v);
    }
    let mut prelude: Vec<u8> = Vec::with_capacity(20);
    write_varint(&mut prelude, headers_buf.len());
    write_varint(&mut prelude, payload.len());
    prelude.extend_from_slice(&crc32(&headers_buf).to_be_bytes());
    prelude.extend_from_slice(&crc32(payload).to_be_bytes());
    let mut out = Vec::with_capacity(12 + headers_buf.len() + payload.len());
    out.extend_from_slice(&prelude);
    out.extend_from_slice(&headers_buf);
    out.extend_from_slice(payload);
    out
}

// ===================================================================
// EventStream frame parser (response stream)
// ===================================================================

#[derive(Debug, Default, Clone)]
pub struct ParsedFrame {
    pub headers: HashMap<String, String>,
    pub payload: Vec<u8>,
}

pub struct FrameStream<R: std::io::Read> {
    pub reader: R,
    pub buf: Vec<u8>,
    pub done: bool,
}

impl<R: std::io::Read> FrameStream<R> {
    pub fn new(reader: R) -> Self { Self { reader, buf: Vec::new(), done: false } }

    /// Read the next frame. Returns Ok(None) on clean EOF.
    pub fn next_frame(&mut self) -> std::io::Result<Option<ParsedFrame>> {
        while self.buf.len() < 16 && !self.done {
            let mut tmp = [0u8; 4096];
            let n = match self.reader.read(&mut tmp) {
                Ok(0) => { self.done = true; break; }
                Ok(n) => n,
                Err(e) => return Err(e),
            };
            self.buf.extend_from_slice(&tmp[..n]);
        }
        if self.done || self.buf.len() < 16 {
            return Ok(None);
        }
        let mut i = 0;
        let (mut headers_len, mut read) = read_varint(&self.buf, i)?;
        i += read;
        let (mut payload_len, read) = read_varint(&self.buf, i)?;
        i += read;
        if i + 8 > self.buf.len() { return Ok(None); }
        i += 8; // skip CRC trailers
        let total = i + headers_len + payload_len + 4;
        if total > 24 * 1024 * 1024 {
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "frame too large"));
        }
        while self.buf.len() < total {
            let mut tmp = [0u8; 4096];
            let n = match self.reader.read(&mut tmp) {
                Ok(0) => { self.done = true; break; }
                Ok(n) => n,
                Err(e) => return Err(e),
            };
            self.buf.extend_from_slice(&tmp[..n]);
        }
        if self.buf.len() < total { return Ok(None); }
        let headers_buf = self.buf[i..i + headers_len].to_vec();
        i += headers_len;
        let payload_buf = self.buf[i..i + payload_len].to_vec();
        i += payload_len + 4;
        self.buf.drain(..i);
        let mut headers = HashMap::new();
        let mut h = 0;
        while h < headers_buf.len() {
            let ty = headers_buf[h];
            h += 1;
            if ty != 0 { break; }
            if h + 2 > headers_buf.len() { break; }
            let name_len = ((headers_buf[h] as usize) << 8) | (headers_buf[h + 1] as usize);
            h += 2;
            if h + name_len > headers_buf.len() { break; }
            let name = String::from_utf8_lossy(&headers_buf[h..h + name_len]).to_string();
            h += name_len;
            if h + 2 > headers_buf.len() { break; }
            let val_len = ((headers_buf[h] as usize) << 8) | (headers_buf[h + 1] as usize);
            h += 2;
            if h + val_len > headers_buf.len() { break; }
            let val = String::from_utf8_lossy(&headers_buf[h..h + val_len]).to_string();
            h += val_len;
            headers.insert(name, val);
        }
        Ok(Some(ParsedFrame { headers, payload: payload_buf }))
    }
}

fn read_varint(buf: &[u8], mut pos: usize) -> std::io::Result<(usize, usize)> {
    let mut n: usize = 0;
    let mut shift = 0;
    let mut read = 0;
    while pos < buf.len() {
        let b = buf[pos];
        pos += 1;
        read += 1;
        n |= ((b & 0x7f) as usize) << shift;
        if b & 0x80 == 0 { return Ok((n, read)); }
        shift += 7;
        if shift > 63 {
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "varint overflow"));
        }
    }
    Err(std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "varint eof"))
}

// ===================================================================
// Token refresh
// ===================================================================

#[derive(Debug, Clone)]
pub struct RefreshedToken {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: Option<i64>,
}

pub async fn refresh(db: &Db, http: &Client, connection_id: &str) -> Option<RefreshedToken> {
    let conn = db.get_connection_full(connection_id).await?;
    let psd = conn.get("providerSpecificData")?.as_object()?;
    let refresh_token = conn.get("refreshToken")?.as_str()?.to_string();
    let auth_method = psd.get("authMethod").and_then(|v| v.as_str()).unwrap_or("builder-id").to_string();
    let client_id = psd.get("clientId").and_then(|v| v.as_str()).map(String::from);
    let client_secret = psd.get("clientSecret").and_then(|v| v.as_str()).map(String::from);
    let region = psd.get("region").and_then(|v| v.as_str()).map(String::from);

    let proxy = db.resolve_connection_proxy(connection_id).await;
    let client = crate::proxy::AppState::client_for_default(http, proxy.as_deref());

    // 1) external_idp (Microsoft / generic OIDC) — form-encoded
    if auth_method == "external_idp" {
        let Some(body) = build_external_idp_body(&refresh_token, psd) else { return None; };
        let endpoint = psd.get("tokenEndpoint").and_then(|v| v.as_str()).unwrap_or(KIRO_REFRESH_URL);
        let Some(r) = http_ok(client.post(endpoint)
            .header("content-type", "application/x-www-form-urlencoded")
            .header("accept", "application/json")
            .body(body)
            .timeout(Duration::from_secs(20))
            .send().await).await else { return None; };
        let v: Value = r.json().await.ok()?;
        let next = RefreshedToken {
            access_token: v.get("access_token")?.as_str()?.to_string(),
            refresh_token: v.get("refresh_token").and_then(|x| x.as_str()).unwrap_or(refresh_token.as_str()).to_string(),
            expires_in: v.get("expires_in").and_then(|x| x.as_i64()),
        };
        if next.access_token.is_empty() { return None; }
        let _ = db.update_connection_tokens(connection_id, &next.access_token, &next.refresh_token, &iso_in(next.expires_in), "").await;
        return Some(next);
    }

    // 2) AWS IDC (Builder-ID / social) — JSON {clientId, clientSecret, refreshToken, grantType}
    if let (Some(cid), Some(sec)) = (client_id.as_deref(), client_secret.as_deref()) {
        let is_idc = auth_method == "idc";
        let endpoint = if is_idc {
            region.as_deref().map(|r| format!("https://oidc.{r}.amazonaws.com/token"))
                .unwrap_or_else(|| KIRO_OIDC_REFRESH_DEFAULT.to_string())
        } else { KIRO_OIDC_REFRESH_DEFAULT.to_string() };
        let Some(r) = http_ok(client.post(&endpoint)
            .header("content-type", "application/json")
            .header("accept", "application/json")
            .json(&json!({ "clientId": cid, "clientSecret": sec, "refreshToken": refresh_token, "grantType": "refresh_token" }))
            .timeout(Duration::from_secs(20))
            .send().await).await else { return None; };
        let v: Value = r.json().await.ok()?;
        let next = RefreshedToken {
            access_token: v.get("access_token")?.as_str()?.to_string(),
            refresh_token: v.get("refresh_token").and_then(|x| x.as_str()).unwrap_or(refresh_token.as_str()).to_string(),
            expires_in: v.get("expires_in").and_then(|x| x.as_i64()),
        };
        if next.access_token.is_empty() { return None; }
        let _ = db.update_connection_tokens(connection_id, &next.access_token, &next.refresh_token, &iso_in(next.expires_in), "").await;
        return Some(next);
    }

    // 3) Builder-ID / social — kiro.dev/refreshToken JSON shape
    let Some(r) = http_ok(client.post(KIRO_REFRESH_URL)
        .header("content-type", "application/json")
        .header("accept", "application/json")
        .json(&json!({ "refreshToken": refresh_token }))
        .timeout(Duration::from_secs(20))
        .send().await).await else { return None; };
    let v: Value = r.json().await.ok()?;
    let next = RefreshedToken {
        access_token: v.get("accessToken").or_else(|| v.get("access_token"))?.as_str()?.to_string(),
        refresh_token: v.get("refreshToken").or_else(|| v.get("refresh_token")).and_then(|x| x.as_str()).unwrap_or(refresh_token.as_str()).to_string(),
        expires_in: v.get("expiresIn").or_else(|| v.get("expires_in")).and_then(|x| x.as_i64()),
    };
    if next.access_token.is_empty() { return None; }
    let _ = db.update_connection_tokens(connection_id, &next.access_token, &next.refresh_token, &iso_in(next.expires_in), "").await;
    Some(next)
}

fn build_external_idp_body(refresh_token: &str, psd: &serde_json::Map<String, Value>) -> Option<String> {
    let endpoint = psd.get("tokenEndpoint")?.as_str()?;
    let scope = psd.get("scope").and_then(|v| v.as_str())
        .unwrap_or("codewhisperer:read codewhisperer:invoke");
    let client_id = psd.get("clientId")?.as_str()?;
    let client_secret = psd.get("clientSecret")?.as_str()?;
    Some(format!(
        "grant_type=refresh_token&refresh_token={rt}&client_id={cid}&client_secret={sec}&scope={sc}",
        rt = url_encode(refresh_token),
        cid = url_encode(client_id),
        sec = url_encode(client_secret),
        sc = url_encode(scope),
    ))
}

fn url_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

fn iso_in(expires_in: Option<i64>) -> String {
    let secs = expires_in.unwrap_or(3600);
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    crate::db::iso_from_secs_pub(now + secs)
}

async fn http_ok(r: Result<reqwest::Response, reqwest::Error>) -> Option<reqwest::Response> {
    match r { Ok(r) if r.status().is_success() => Some(r), _ => None }
}

// ===================================================================
// Quota
// ===================================================================

pub async fn quota(db: &Db, http: &Client, connection_id: &str) -> Value {
    let Some(conn) = db.get_connection_full(connection_id).await else {
        return json!({ "available": false, "message": "Connection not found" });
    };
    let Some(token) = conn.get("accessToken").and_then(|v| v.as_str()) else {
        return json!({ "available": false, "message": "no access token" });
    };
    let psd = conn.get("providerSpecificData").cloned().unwrap_or(json!({}));
    let auth_method = psd.get("authMethod").and_then(|v| v.as_str()).unwrap_or("");
    let cw = psd.get("cwHost").and_then(|v| v.as_str()).unwrap_or("https://codewhisperer.us-east-1.amazonaws.com");
    let url = format!("{cw}/getUsageLimits");

    let mut req = http.get(&url).header("authorization", format!("Bearer {token}")).timeout(Duration::from_secs(15));
    if auth_method == "api_key" { req = req.header("tokentype", "API_KEY"); }
    if auth_method == "external_idp" { req = req.header("TokenType", "EXTERNAL_IDP"); }

    match req.send().await {
        Ok(r) if r.status().is_success() => {
            let v: Value = r.json().await.unwrap_or(json!({}));
            let usage_list = v.get("usageBreakdownList").and_then(|x| x.as_array()).cloned().unwrap_or_default();
            let reset = v.get("nextDateReset").and_then(|x| x.as_str()).map(String::from);
            let mut quotas = serde_json::Map::new();
            for b in usage_list {
                if let Some(rt) = b.get("resourceType").and_then(|x| x.as_str()) {
                    let used = b.get("currentUsageWithPrecision").and_then(|x| x.as_f64()).unwrap_or(0.0);
                    let total = b.get("usageLimitWithPrecision").and_then(|x| x.as_f64()).unwrap_or(0.0);
                    let remaining = (total - used).max(0.0);
                    quotas.insert(
                        rt.to_lowercase(),
                        json!({
                            "used": used, "total": total, "remaining": remaining,
                            "remainingPct": if total > 0.0 { (remaining / total) * 100.0 } else { 100.0 },
                            "resetAt": reset,
                        }),
                    );
                }
            }
            json!({
                "available": true,
                "provider": "kiro",
                "connectionId": connection_id,
                "plan": v.get("subscriptionInfo").and_then(|s| s.get("subscriptionTitle")).cloned().unwrap_or(json!("Kiro")),
                "quotas": Value::Object(quotas),
            })
        }
        Ok(r) => json!({ "available": false, "provider": "kiro", "connectionId": connection_id, "error": format!("upstream HTTP {}", r.status().as_u16()) }),
        Err(e) => json!({ "available": false, "provider": "kiro", "connectionId": connection_id, "error": format!("network error: {e}") }),
    }
}

// ===================================================================
// Chat
// ===================================================================

/// Convert an OpenAI message into a Kiro history entry
/// (userInputMessage / assistantResponseMessage shapes).
pub fn history_entry(m: &Value) -> Option<Value> {
    let role = m.get("role")?.as_str()?;
    let content = m.get("content")?.as_str()?.to_string();
    if content.is_empty() { return None; }
    match role {
        "user" => Some(json!({ "userInputMessage": { "content": content, "origin": "AI_EDITOR" } })),
        "assistant" => Some(json!({ "assistantResponseMessage": { "content": content } })),
        _ => None, // system/tool handled separately
    }
}

/// Convert the final OpenAI message into the Kiro userInputMessage shape.
pub fn user_input_message(m: &Value) -> Option<Value> {
    let content = m
        .get("content")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    Some(json!({
        "content": content,
        "origin": "AI_EDITOR",
    }))
}

#[derive(Debug, Clone)]
pub struct ChatRequest {
    pub conversation_id: String,
    pub profile_arn: String,
    pub upstream_model: String,
    pub system_prompt: Option<String>,
    pub history: Vec<Value>,
    pub current_user: Value,
    pub inference: Option<Value>,
    pub additional: Option<Value>,
}

fn build_payload(req: &ChatRequest) -> Value {
    let mut body = json!({
        "conversationState": {
            "chatTriggerType": "MANUAL",
            "conversationId": req.conversation_id,
            "agentContinuationId": Uuid::new_v4().to_string(),
            "agentTaskType": "vibe",
            "currentMessage": { "userInputMessage": req.current_user },
            "history": req.history,
        },
        "agentMode": "vibe",
        "profileArn": req.profile_arn,
    });
    if let Some(s) = &req.system_prompt {
        if !s.is_empty() { body["systemPrompt"] = json!(s); }
    }
    if let Some(i) = &req.inference { body["inferenceConfig"] = i.clone(); }
    if let Some(a) = &req.additional { body["additionalModelRequestFields"] = a.clone(); }
    body
}

pub async fn chat(
    db: &Db,
    http: &Client,
    connection_id: &str,
    req: &ChatRequest,
    stream: bool,
) -> Result<(axum::http::StatusCode, axum::http::HeaderMap, axum::body::Body), String> {
    let Some(conn) = db.get_connection_full(connection_id).await else { return Err("connection not found".into()); };
    let Some(token) = conn.get("accessToken").and_then(|v| v.as_str()) else { return Err("no access token".into()); };
    let psd = conn.get("providerSpecificData").cloned().unwrap_or(json!({}));
    let auth_method = psd.get("authMethod").and_then(|v| v.as_str()).unwrap_or("builder-id").to_string();
    let is_api_key = auth_method == "api_key";
    let is_external_idp = auth_method == "external_idp";
    let profile_arn = psd.get("profileArn").and_then(|v| v.as_str()).map(String::from)
        .or_else(|| default_profile_arn(&auth_method))
        .unwrap_or_default();
    if profile_arn.is_empty() { return Err("no profileArn available for this connection".into()); }
    let mut req_with_arn = req.clone();
    req_with_arn.profile_arn = profile_arn;

    let body_json = build_payload(&req_with_arn);
    let body_bytes = build_request_body(&body_json.to_string());
    let proxy = db.resolve_connection_proxy(connection_id).await;
    let client = crate::proxy::AppState::client_for_default(http, proxy.as_deref());

    let mut last_err: Option<String> = None;
    for url in KIRO_ENDPOINTS {
        let mut builder = client.post(*url)
            .header("accept", "application/vnd.amazon.eventstream")
            .timeout(Duration::from_secs(if stream { 300 } else { 120 }));
        for (k, v) in KIRO_DEFAULT_HEADERS { builder = builder.header(*k, *v); }
        builder = builder.header("authorization", format!("Bearer {token}"));
        builder = builder.header("amz-sdk-invocation-id", Uuid::new_v4().to_string());
        if url.contains("codewhisperer") { builder = builder.header("x-amz-target", KIRO_CODEWHISPERER_TARGET); }
        if is_api_key { builder = builder.header("tokentype", "API_KEY"); }
        if is_external_idp { builder = builder.header("TokenType", "EXTERNAL_IDP"); }

        let resp = match builder.body(body_bytes.clone()).send().await {
            Ok(r) => r,
            Err(e) => { last_err = Some(format!("{url}: {e}")); continue; }
        };
        let status = resp.status().as_u16();
        if status == 401 || status == 403 || status == 404 {
            last_err = Some(format!("{url}: {status}")); continue;
        }
        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("upstream {status}: {}", &body[..body.len().min(200)]));
        }

        let mut headers = axum::http::HeaderMap::new();
        headers.insert("content-type", "text/event-stream; charset=utf-8".parse().unwrap());
        headers.insert("cache-control", "no-cache".parse().unwrap());
        if stream {
            return Ok((axum::http::StatusCode::OK, headers, stream_kiro_sse(resp)));
        }
        let (sse, _usage) = collect_kiro_sse(resp).await;
        return Ok((axum::http::StatusCode::OK, headers, axum::body::Body::from(sse)));
    }
    Err(last_err.unwrap_or_else(|| "all Kiro endpoints failed".to_string()))
}

fn stream_kiro_sse(resp: reqwest::Response) -> axum::body::Body {
    use async_stream::stream;
    let stream = resp.bytes_stream();
    axum::body::Body::from_stream(stream! {
        let mut s = SseState::new();
        let mut frames = FrameStream::new(StreamReader::new(stream));
        while let Ok(Some(frame)) = frames.next_frame() {
            for chunk in s.feed(&frame) {
                yield Ok::<_, std::io::Error>(bytes::Bytes::from(chunk));
            }
        }
        for chunk in s.finish() { yield Ok(bytes::Bytes::from(chunk)); }
    })
}

async fn collect_kiro_sse(resp: reqwest::Response) -> (String, Value) {
    let mut s = SseState::new();
    let mut sse = String::new();
    let mut usage = json!(null);
    let mut frames = FrameStream::new(StreamReader::new(resp.bytes_stream()));
    while let Ok(Some(frame)) = frames.next_frame() {
        for chunk in s.feed(&frame) {
            sse.push_str(&chunk);
        }
    }
    for chunk in s.finish() { sse.push_str(&chunk); }
    let _ = usage;
    (sse, json!(null))
}

// Suppress unused warning when only sse_chunk is re-exported.
#[allow(dead_code)]
fn _unused_marker() {
    let _ = sse_chunk;
}
