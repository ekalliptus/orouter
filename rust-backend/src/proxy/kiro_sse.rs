//! Shared Kiro SSE state machine + streaming helpers used by the kiro
//! executor. The state converts Kiro EventStream frames into OpenAI-style
//! `chat.completion.chunk` SSE.

use std::collections::HashMap;
use std::task::{Context, Poll};

use bytes::Bytes;
use futures_util::Stream;
use futures_util::StreamExt as _;
use reqwest::Error as ReqwestError;
use serde_json::{json, Value};
use uuid::Uuid;

pub struct StreamReader<S: futures_util::Stream<Item = Result<Bytes, ReqwestError>> + Unpin> {
    inner: S,
    pending: Option<Bytes>,
}
impl<S: futures_util::Stream<Item = Result<Bytes, ReqwestError>> + Unpin> StreamReader<S> {
    pub fn new(inner: S) -> Self { Self { inner, pending: None } }
}
impl<S: futures_util::Stream<Item = Result<Bytes, ReqwestError>> + Unpin> std::io::Read for StreamReader<S> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let mut cx = Context::from_waker(&mut std::task::Waker::noop());
        loop {
            if let Some(p) = self.pending.take() {
                if p.is_empty() { continue; }
                let n = p.len().min(buf.len());
                buf[..n].copy_from_slice(&p[..n]);
                if n < p.len() { self.pending = Some(p.slice(n..)); }
                return Ok(n);
            }
            match self.inner.poll_next_unpin(&mut cx) {
                std::task::Poll::Ready(Some(Ok(chunk))) => {
                    if chunk.is_empty() { continue; }
                    let n = chunk.len().min(buf.len());
                    buf[..n].copy_from_slice(&chunk[..n]);
                    if n < chunk.len() { self.pending = Some(chunk.slice(n..)); }
                    return Ok(n);
                }
                std::task::Poll::Ready(Some(Err(e))) => {
                    return Err(std::io::Error::new(std::io::ErrorKind::Other, e.to_string()));
                }
                std::task::Poll::Ready(None) => return Ok(0),
                std::task::Poll::Pending => return Err(std::io::Error::new(std::io::ErrorKind::Interrupted, "would block")),
            }
        }
    }
}

pub fn sse_chunk(v: &Value) -> String {    format!("data: {}\n\n", v)
}

pub fn default_profile_arn(auth_method: &str) -> Option<String> {
    match auth_method {
        "google" | "github" => Some("arn:aws:codewhisperer:us-east-1:699475941385:profile/EHGA3GRVQMUK".to_string()),
        _ => Some("arn:aws:codewhisperer:us-east-1:638616132270:profile/AAAACCCCXXXX".to_string()),
    }
}

pub struct SseState {
    id: String,
    model: String,
    created: i64,
    assistant_text: String,
    reasoning_text: String,
    tool_calls: Vec<Value>,
    finish_reason: Option<String>,
    usage: Option<Value>,
    sent_role: bool,
    sent_tool_calls: bool,
}
impl SseState {
    pub fn new() -> Self {
        Self {
            id: format!("chatcmpl-{}", Uuid::new_v4().to_string().split('-').next().unwrap_or("kiro")),
            model: "kiro".into(),
            created: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0),
            assistant_text: String::new(),
            reasoning_text: String::new(),
            tool_calls: Vec::new(),
            finish_reason: None,
            usage: None,
            sent_role: false,
            sent_tool_calls: false,
        }
    }

    pub fn ensure_role_chunk(&mut self) -> Vec<String> {
        if self.sent_role { return vec![]; }
        self.sent_role = true;
        vec![sse_chunk(&json!({
            "id": self.id, "object": "chat.completion.chunk", "created": self.created,
            "model": self.model, "choices": [{ "index": 0, "delta": { "role": "assistant" }, "finish_reason": null }],
        }))]
    }

    pub fn feed(&mut self, frame: &crate::kiro::ParsedFrame) -> Vec<String> {
        let event_type = frame.headers.get(":event-type").cloned().unwrap_or_default();
        let payload = match std::str::from_utf8(&frame.payload) {
            Ok(s) => s,
            Err(_) => return vec![],
        };
        let v: Value = serde_json::from_str(payload).unwrap_or(Value::Null);
        let mut out = self.ensure_role_chunk();

        match event_type.as_str() {
            "assistantResponseEvent" => {
                if let Some(content) = v.pointer("/content").and_then(|x| x.as_str()) {
                    self.assistant_text.push_str(content);
                    out.push(sse_chunk(&json!({
                        "id": self.id, "object": "chat.completion.chunk", "created": self.created,
                        "model": self.model, "choices": [{ "index": 0, "delta": { "content": content }, "finish_reason": null }],
                    })));
                }
            }
            "reasoningContentEvent" => {
                if let Some(content) = v.pointer("/content").and_then(|x| x.as_str()) {
                    self.reasoning_text.push_str(content);
                    out.push(sse_chunk(&json!({
                        "id": self.id, "object": "chat.completion.chunk", "created": self.created,
                        "model": self.model,
                        "choices": [{ "index": 0, "delta": { "reasoning_content": content }, "finish_reason": null }],
                    })));
                }
            }
            "toolUseEvent" => {
                if let (Some(name), Some(args)) = (
                    v.pointer("/name").and_then(|x| x.as_str()),
                    v.pointer("/input").cloned().or_else(|| v.pointer("/arguments").cloned()),
                ) {
                    let id = v.pointer("/toolUseId").and_then(|x| x.as_str()).unwrap_or("call").to_string();
                    self.tool_calls.push(json!({
                        "id": id, "type": "function", "function": { "name": name, "arguments": args.to_string() },
                    }));
                }
            }
            "messageStopEvent" => { self.finish_reason = Some("stop".into()); }
            "metadataEvent" | "MetadataEvent" | "meteringEvent" | "metricsEvent" => {
                if let Some(u) = v.get("usage") { self.usage = Some(u.clone()); }
            }
            _ => {}
        }
        out
    }

    pub fn finish(&mut self) -> Vec<String> {
        let mut out = vec![];
        if self.finish_reason.is_none() { self.finish_reason = Some("stop".into()); }
        let mut delta = serde_json::Map::new();
        if !self.tool_calls.is_empty() {
            delta.insert("tool_calls".into(), Value::Array(self.tool_calls.clone()));
            self.sent_tool_calls = true;
        }
        delta.insert("".into(), Value::String(String::new()));
        let choice = json!({ "index": 0, "delta": delta, "finish_reason": self.finish_reason });
        out.push(sse_chunk(&json!({
            "id": self.id, "object": "chat.completion.chunk", "created": self.created,
            "model": self.model, "choices": [choice],
        })));
        out.push("data: [DONE]\n\n".to_string());
        out
    }
}

#[allow(dead_code)]
pub fn frame_headers<'a>(headers: &'a HashMap<String, String>, key: &str) -> Option<&'a str> {
    headers.get(key).map(|s| s.as_str())
}
