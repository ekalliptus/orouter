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
    routing::{get, post},
    Router,
};
use proxy::AppState;
use tower_http::{cors::CorsLayer, limit::RequestBodyLimitLayer, trace::TraceLayer};
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
        .timeout(Duration::from_secs(600)) // generous; SSE streams long
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
        .layer(middleware::from_fn(auth::middleware::require_auth))
        .with_state(state.clone());

    let app = Router::new()
        .route("/health", get(api::health))
        .route("/v1/models", get(api::models))
        .route("/v1/chat/completions", post(proxy::chat_completions))
        .merge(auth_routes)
        .merge(dashboard_routes)
        .layer(RequestBodyLimitLayer::new(cfg.body_max_bytes))
        .layer(CorsLayer::very_permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(cfg.addr())
        .await
        .unwrap_or_else(|e| panic!("bind {}: {e}", cfg.addr()));

    info!("listening on http://{}", cfg.addr());

    let shutdown = async move {
        let _ = tokio::signal::ctrl_c().await;
        info!("ctrl-c received, shutting down");
    };

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown)
        .await
        .expect("server error");
}
