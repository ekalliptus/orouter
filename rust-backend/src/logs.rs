//! In-process console log ring buffer + live broadcast.
//!
//! A custom tracing Layer captures every log event the server emits, keeps
//! the last N lines in memory, and publishes them on a tokio broadcast
//! channel so the dashboard Console Log page can stream them over SSE.
//! Mirrors the Node dashboard's console-log tail (best effort — Node tails
//! its own stdout; we tail our structured tracing events).

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use once_cell::sync::Lazy;
use serde_json::json;
use tracing::{Event, Subscriber};
use tracing_subscriber::layer::{Context, Layer};
use tracing_subscriber::registry;

const CAP: usize = 1000;

#[derive(Debug, Clone)]
pub struct LogLine {
    pub ts: String,
    pub level: String,
    pub target: String,
    pub message: String,
}

impl LogLine {
    pub fn to_json(&self) -> serde_json::Value {
        json!({
            "ts": self.ts,
            "level": self.level,
            "target": self.target,
            "message": self.message,
        })
    }
}

static RING: Lazy<Arc<Mutex<VecDeque<LogLine>>>> =
    Lazy::new(|| Arc::new(Mutex::new(VecDeque::with_capacity(CAP))));
static BUS: Lazy<tokio::sync::broadcast::Sender<LogLine>> =
    Lazy::new(|| tokio::sync::broadcast::channel(512).0);

fn push_line(line: LogLine) {
    if let Ok(mut ring) = RING.lock() {
        if ring.len() >= CAP {
            ring.pop_front();
        }
        ring.push_back(line.clone());
    }
    let _ = BUS.send(line);
}

/// Recent lines (oldest first), optionally capped to the last `limit`.
pub fn recent(limit: usize) -> Vec<serde_json::Value> {
    let ring = RING.lock().map(|r| r.clone()).unwrap_or_default();
    let start = ring.len().saturating_sub(limit);
    ring.iter().skip(start).map(LogLine::to_json).collect()
}

pub fn clear() {
    if let Ok(mut ring) = RING.lock() {
        ring.clear();
    }
}

pub fn subscribe() -> tokio::sync::broadcast::Receiver<LogLine> {
    BUS.subscribe()
}

/// tracing Layer feeding the ring buffer + bus.
pub struct CaptureLayer;

struct MessageVisitor {
    message: Option<String>,
}

impl tracing::field::Visit for MessageVisitor {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.message = Some(format!("{value:?}"));
        } else if self.message.is_some() {
            self.message = Some(format!("{} {}={:?}", self.message.as_deref().unwrap_or(""), field.name(), value));
        } else {
            self.message = Some(format!("{}={:?}", field.name(), value));
        }
    }
    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        if field.name() == "message" {
            self.message = Some(value.to_string());
        } else {
            self.message = Some(format!("{} {}={}", self.message.as_deref().unwrap_or(""), field.name(), value));
        }
    }
}

impl<S: Subscriber> Layer<S> for CaptureLayer {
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let mut visitor = MessageVisitor { message: None };
        event.record(&mut visitor);
        let message = visitor.message.unwrap_or_default();
        let ts = {
            let ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or(0);
            format!(
                "{:02}:{:02}:{:02}.{:03}",
                (ms / 3_600_000) % 24,
                (ms / 60_000) % 60,
                (ms / 1000) % 60,
                ms % 1000
            )
        };
        push_line(LogLine {
            ts,
            level: event.metadata().level().to_string(),
            target: event.metadata().target().to_string(),
            message,
        });
    }
}

/// Build the full tracing subscriber (fmt + capture layer + env filter).
pub fn init_tracing(filter: tracing_subscriber::EnvFilter) {
    use tracing_subscriber::prelude::*;
    tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer().with_target(false))
        .with(CaptureLayer)
        .init();
}
