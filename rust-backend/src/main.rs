//! ORouter Rust backend entry point.
//!
//! v1 surface (mirrors the Go backend's native slice):
//!   GET  /health                — liveness
//!   GET  /v1/models             — static OpenAI-compatible catalog
//!   POST /v1/chat/completions   — proxy + SSE relay (OpenAI→OpenAI passthrough)
//!
//! Everything else is intentionally not implemented yet. The Go backend's
//! value was as a reverse-proxy bridge to the Node engine; this Rust server
//! replaces the Go native slice and will grow the remaining /api/* surface in
//! later milestones. See PLAN.md / the conversation for the milestone map.

mod api;
mod auth;
mod config;
mod db;
mod proxy;
mod snapshot;

use std::time::Duration;

use axum::{
    middleware,
    routing::{any, get, post},
    Router,
};
use proxy::AppState;
use tower_http::{
    cors::CorsLayer,
    limit::RequestBodyLimitLayer,
    services::{ServeDir, ServeFile},
    trace::TraceLayer,
};
use tracing::info;

#[tokio::main]
async fn main() {
    let cfg = config::load();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| cfg.log_level.clone().into()),
        )
        .with_target(false)
        .init();

    info!(
        port = cfg.port,
        host = %cfg.host,
        data_dir = %cfg.data_dir.display(),
        db_path = %cfg.db_path().display(),
        static_dir = %cfg.static_dir.display(),
        "orouter-backend starting"
    );

    // Open the shared SQLite store. Failure is non-fatal: health + models still
    // work; the chat path returns a clear error until a DB/credential exists.
    let db = match db::Db::open(&cfg.db_path()) {
        Ok(db) => {
            info!("sqlite opened");
            db
        }
        Err(e) => {
            tracing::error!(error = %e, "sqlite open failed (chat proxy disabled until DB available)");
            // A no-op handle would be cleaner; for v1 we just exit — the Node
            // app owns the DB and should be started first.
            std::process::exit(1);
        }
    };

    let client = reqwest::Client::builder()
        .pool_idle_timeout(Duration::from_secs(90))
        .tcp_keepalive(Duration::from_secs(60))
        .connect_timeout(cfg.read_timeout)
        .timeout(cfg.write_timeout)
        .build()
        .expect("reqwest client build");

    let state = AppState { db, client };

    // Public auth routes (login/status/logout) — NOT behind the session gate.
    // They share the same AppState because login reads settings + updates the
    // login limiter using the DB.
    let auth_routes = Router::new()
        .route("/api/auth/login", post(auth::login))
        .route("/api/auth/logout", post(auth::logout))
        .route("/api/auth/status", get(auth::status))
        .with_state(state.clone());

    // Protected dashboard routes — session-gated via require_auth middleware.
    let dashboard_routes = Router::new()
        .route(
            "/api/settings",
            get(api::dashboard::settings_get).patch(api::dashboard::settings_patch),
        )
        .route(
            "/api/keys",
            get(api::dashboard::keys_get).post(api::dashboard::keys_post),
        )
        .route(
            "/api/keys/:id",
            axum::routing::delete(api::dashboard::keys_delete),
        )
        .route(
            "/api/providers",
            get(api::dashboard::providers_get).post(api::dashboard::providers_post),
        )
        .route(
            "/api/providers/:id",
            axum::routing::put(api::dashboard::providers_update)
                .delete(api::dashboard::providers_delete),
        )
        .route(
            "/api/providers/:id/test",
            axum::routing::post(api::dashboard::providers_test),
        )
        .route("/api/usage/logs", get(api::dashboard::usage_logs))
        .route("/api/usage/stats", get(api::dashboard::usage_stats))
        .route(
            "/api/combos",
            get(api::dashboard::combos_get).post(api::dashboard::combos_post),
        )
        .route(
            "/api/combos/:id",
            axum::routing::delete(api::dashboard::combos_delete),
        )
        .route("/api/cli-tools", get(api::dashboard::cli_tools_get))
        .layer(middleware::from_fn(auth::middleware::require_auth))
        .with_state(state.clone());

    // Reserve backend prefixes before the SPA fallback. Unknown API paths must
    // return JSON 404 rather than index.html.
    let backend_not_found = Router::new()
        .route("/api", any(api::not_found))
        .route("/api/*path", any(api::not_found))
        .route("/v1", any(api::not_found))
        .route("/v1/*path", any(api::not_found));

    // Serve the Vite output from the same Rust process. Missing frontend routes
    // fall back to index.html for React Router; static assets remain cacheable by
    // the reverse proxy.
    let static_files =
        ServeDir::new(&cfg.static_dir).fallback(ServeFile::new(cfg.static_dir.join("index.html")));

    let app = Router::new()
        .route("/health", get(api::health))
        .route("/v1/models", get(api::models))
        .route("/v1/chat/completions", post(proxy::chat_completions))
        .merge(auth_routes)
        .merge(dashboard_routes)
        .merge(backend_not_found)
        .fallback_service(static_files)
        .layer(RequestBodyLimitLayer::new(cfg.body_max_bytes))
        .layer(CorsLayer::very_permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(cfg.addr())
        .await
        .unwrap_or_else(|e| panic!("bind {}: {e}", cfg.addr()));

    info!("listening on http://{}", cfg.addr());

    // systemd sends SIGTERM; local terminals send SIGINT. Trigger graceful
    // Axum shutdown for either, then enforce the configured ceiling so a stuck
    // SSE client cannot block a deployment indefinitely.
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
    let mut server = tokio::spawn(async move {
        axum::serve(listener, app)
            .with_graceful_shutdown(async {
                let _ = shutdown_rx.await;
            })
            .await
    });

    tokio::select! {
        result = &mut server => {
            result.expect("server task panicked").expect("server error");
        }
        signal = shutdown_signal() => {
            info!(signal, "shutdown requested");
            let _ = shutdown_tx.send(());
            if tokio::time::timeout(cfg.shutdown_timeout, &mut server).await.is_err() {
                tracing::warn!(timeout = ?cfg.shutdown_timeout, "graceful shutdown timed out; aborting");
                server.abort();
            }
        }
    }
}

async fn shutdown_signal() -> &'static str {
    #[cfg(unix)]
    {
        let mut terminate =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                .expect("install SIGTERM handler");
        tokio::select! {
            _ = tokio::signal::ctrl_c() => "SIGINT",
            _ = terminate.recv() => "SIGTERM",
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
        "SIGINT"
    }
}
