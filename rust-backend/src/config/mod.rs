//! Configuration loaded from the environment.
//!
//! Mirrors the env contract of the Go backend (backend/internal/config/config.go)
//! and `.env.example`, but prefixed `RUST_` so this binary can run alongside the
//! existing Go/Node processes sharing one `.env` without port clashes.
//!
//! The Rust backend opens the SAME SQLite file Node/Go use (`<DATA_DIR>/db/data.sqlite`),
//! so `DATA_DIR` is read unprefixed (it is the shared path contract).

use std::{net::SocketAddr, path::PathBuf, time::Duration};

#[derive(Debug, Clone)]
pub struct Config {
    /// Interface to bind. Default loopback (127.0.0.1) — same defense-in-depth
    /// as the Go backend: the API is unreachable from the network unless exposed.
    pub host: String,
    /// Listen port. Default 20130 (Go uses 20128, Node 20127/20129 — avoid clashes).
    pub port: u16,
    /// Shared data dir (matches Node `dataDir.js` + Go config). Default ~/.9router.
    pub data_dir: PathBuf,
    /// Vite production output served by the Rust process.
    pub static_dir: PathBuf,
    /// Node/Next.js upstream URL for reverse proxy (hybrid mode).
    /// Empty = standalone Rust (serve static React directly).
    pub node_upstream: String,
    /// Outbound request body cap (proxied + native), in bytes.
    pub body_max_bytes: usize,
    pub read_timeout: Duration,
    pub write_timeout: Duration,
    pub shutdown_timeout: Duration,
    /// tracing filter (e.g. "info,orouter_backend=debug").
    pub log_level: String,
}

impl Config {
    pub fn addr(&self) -> SocketAddr {
        format!("{}:{}", self.host, self.port)
            .parse()
            .unwrap_or_else(|e| panic!("invalid bind addr {}:{}: {e}", self.host, self.port))
    }

    /// Path to the shared SQLite file: `<data_dir>/db/data.sqlite`.
    pub fn db_path(&self) -> PathBuf {
        self.data_dir.join("db").join("data.sqlite")
    }
}

const DEFAULT_HOST: &str = "127.0.0.1";
const DEFAULT_PORT: u16 = 20130;
const DEFAULT_BODY_MAX_MB: usize = 128;

impl Default for Config {
    fn default() -> Self {
        Self {
            host: DEFAULT_HOST.to_string(),
            port: DEFAULT_PORT,
            data_dir: default_data_dir(),
            static_dir: default_static_dir(),
            node_upstream: String::new(),
            body_max_bytes: DEFAULT_BODY_MAX_MB * 1024 * 1024,
            read_timeout: Duration::from_secs(30),
            write_timeout: Duration::from_secs(5 * 60),
            shutdown_timeout: Duration::from_secs(10),
            log_level: "info".to_string(),
        }
    }
}

/// Load config from the environment, applying defaults for anything unset.
pub fn load() -> Config {
    let mut cfg = Config::default();

    if let Ok(v) = std::env::var("RUST_HOST").or_else(|_| std::env::var("HOST")) {
        if !v.is_empty() {
            cfg.host = v;
        }
    }
    // RUST_PORT wins; otherwise PORT — but only if it differs from the Node/Go
    // defaults so we don't try to bind an already-used port.
    if let Ok(v) = std::env::var("RUST_PORT") {
        if let Ok(p) = v.parse() {
            cfg.port = p;
        }
    } else if let Ok(v) = std::env::var("PORT") {
        if let Ok(p) = v.parse::<u16>() {
            if p != 20127 && p != 20128 && p != 20129 {
                cfg.port = p;
            }
        }
    }
    // Parity with src/lib/dataDir.js: a Unix-style DATA_DIR (/…) is ignored on
    // Windows since it comes from a Linux-targeted .env/Docker config.
    if let Ok(v) = std::env::var("DATA_DIR") {
        let unix_style_on_windows = v.starts_with('/');
        #[cfg(windows)]
        let skip = unix_style_on_windows;
        #[cfg(not(windows))]
        let skip = false;
        if !v.is_empty() && !skip {
            cfg.data_dir = PathBuf::from(v);
        }
    }
    if let Ok(v) = std::env::var("STATIC_DIR") {
        if !v.is_empty() {
            cfg.static_dir = PathBuf::from(v);
        }
    }
    if let Ok(v) = std::env::var("NODE_UPSTREAM") {
        if !v.is_empty() {
            cfg.node_upstream = v.trim_end_matches('/').to_string();
        }
    }
    if let Ok(v) = std::env::var("RUST_BODY_MAX_MB") {
        if let Ok(mb) = v.parse::<usize>() {
            cfg.body_max_bytes = mb * 1024 * 1024;
        }
    }
    if let Ok(v) = std::env::var("RUST_READ_TIMEOUT") {
        if let Ok(d) = parse_duration(&v) {
            cfg.read_timeout = d;
        }
    }
    if let Ok(v) = std::env::var("RUST_WRITE_TIMEOUT") {
        if let Ok(d) = parse_duration(&v) {
            cfg.write_timeout = d;
        }
    }
    if let Ok(v) = std::env::var("RUST_SHUTDOWN_TIMEOUT") {
        if let Ok(d) = parse_duration(&v) {
            cfg.shutdown_timeout = d;
        }
    }
    if let Ok(v) = std::env::var("RUST_LOG_LEVEL").or_else(|_| std::env::var("GO_LOG_LEVEL")) {
        cfg.log_level = v;
    }
    cfg
}

/// OS default data dir, mirroring src/lib/dataDir.js defaultDir(): on Windows
/// Node uses `%APPDATA%\9router`; elsewhere `$HOME/.9router`. Sharing the same
/// location is required so the Rust binary opens the SAME SQLite file.
pub fn platform_default_data_dir() -> PathBuf {
    #[cfg(windows)]
    {
        let appdata = match std::env::var("APPDATA") {
            Ok(a) if !a.is_empty() => PathBuf::from(a),
            _ => {
                // APPDATA unset → homedir\AppData\Roaming (dataDir.js fallback).
                let home = std::env::var("USERPROFILE")
                    .or_else(|_| std::env::var("HOME"))
                    .unwrap_or_default();
                if home.is_empty() {
                    return PathBuf::from(".9router");
                }
                PathBuf::from(home).join("AppData").join("Roaming")
            }
        };
        appdata.join("9router")
    }
    #[cfg(not(windows))]
    {
        if let Ok(home) = std::env::var("HOME") {
            if !home.is_empty() {
                return PathBuf::from(home).join(".9router");
            }
        }
        PathBuf::from(".9router")
    }
}

fn default_data_dir() -> PathBuf {
    platform_default_data_dir()
}

fn default_static_dir() -> PathBuf {
    // Full-native mode serves the Vue SPA. Prefer vue-web/dist; fall back to
    // the older react-web build, then repo-root-relative variants.
    for candidate in ["vue-web/dist", "../vue-web/dist", "react-web/dist", "../react-web/dist"] {
        let p = PathBuf::from(candidate);
        if p.is_dir() {
            return p;
        }
    }
    PathBuf::from("../vue-web/dist")
}

/// Minimal human duration parser: supports "30s", "5m", "1h", or bare seconds.
fn parse_duration(s: &str) -> Result<Duration, std::num::ParseIntError> {
    let s = s.trim();
    if let Some(n) = s.strip_suffix("ms") {
        return Ok(Duration::from_millis(n.parse()?));
    }
    if let Some(n) = s.strip_suffix('s') {
        return Ok(Duration::from_secs(n.parse()?));
    }
    if let Some(n) = s.strip_suffix('m') {
        return Ok(Duration::from_secs(60u64 * n.parse::<u64>()?));
    }
    if let Some(n) = s.strip_suffix('h') {
        return Ok(Duration::from_secs(3600u64 * n.parse::<u64>()?));
    }
    Ok(Duration::from_secs(s.parse()?))
}
