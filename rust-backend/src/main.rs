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

mod anthropic;
mod antigravity;
mod api;
mod auth;
mod config;
mod db;
mod kiro;
mod logs;
mod modelstore;
mod oauth;
mod proxy;
mod quota;
mod snapshot;
mod webtools;

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

    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| cfg.log_level.clone().into());
    logs::init_tracing(filter);

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

    // No global timeout — AI streaming (SSE) can run for many minutes
    // (Claude extended thinking, long completions). Only connect timeout applies.
    let client = reqwest::Client::builder()
        .pool_idle_timeout(Duration::from_secs(90))
        .tcp_keepalive(Duration::from_secs(60))
        .connect_timeout(cfg.read_timeout)
        .build()
        .expect("reqwest client build");

    let state = AppState {
        db,
        client,
        node_upstream: cfg.node_upstream.clone(),
        proxy_clients: std::sync::Arc::new(std::sync::Mutex::new(std::collections::HashMap::new())),
    };

    // Keep OAuth tokens fresh so hybrid-mode inference always has valid creds.
    tokio::spawn(crate::oauth::auto_refresh_loop(
        state.db.clone(),
        state.client.clone(),
    ));

    // Retention: prune usage + request details older than the configured days.
    {
        let db = state.db.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(6 * 3600));
            interval.tick().await;
            loop {
                interval.tick().await;
                let days = db
                    .get_settings_full()
                    .await
                    .get("usageHistoryRetentionDays")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(30);
                if days > 0 {
                    let (usage, details) = db.prune_history(days).await;
                    if usage + details > 0 {
                        tracing::info!(usage, details, "retention pruned rows older than {days}d");
                    }
                }
            }
        });
    }

    // Public auth routes (login/status/logout) — NOT behind the session gate.
    // They share the same AppState because login reads settings + updates the
    // login limiter using the DB.
    let auth_routes = Router::new()
        .route("/api/auth/login", post(auth::login))
        .route("/api/auth/logout", post(auth::logout))
        .route("/api/auth/status", get(auth::status))
        // /api/version is public in the Node dashboardGuard too.
        .route("/api/version", get(api::version))
        .with_state(state.clone());

    // Protected dashboard routes — session-gated via require_auth middleware.
    let dashboard_routes = Router::new()
        .route(
            "/api/settings",
            get(api::dashboard::settings_get).patch(api::dashboard::settings_patch),
        )
        .route(
            "/api/settings/proxy-test",
            axum::routing::post(api::dashboard::settings_proxy_test),
        )
        .route(
            "/api/keys",
            get(api::dashboard::keys_get).post(api::dashboard::keys_post),
        )
        .route(
            "/api/keys/:id",
            axum::routing::put(api::dashboard::keys_update)
                .delete(api::dashboard::keys_delete),
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
        .route("/api/usage/chart", get(api::dashboard::usage_chart))
        .route(
            "/api/usage/:connectionId",
            get(api::dashboard::connection_quota),
        )
        .route(
            "/api/proxy-pools",
            get(api::dashboard::proxy_pools_get).post(api::dashboard::proxy_pools_post),
        )
        .route(
            "/api/proxy-pools/:id/test",
            axum::routing::post(api::dashboard::proxy_pools_test),
        )
        .route(
            "/api/proxy-pools/:id",
            axum::routing::delete(api::dashboard::proxy_pools_delete),
        )
        .route(
            "/api/console-logs",
            get(api::dashboard::console_logs_get).delete(api::dashboard::console_logs_clear),
        )
        .route(
            "/api/console-logs/stream",
            get(api::dashboard::console_logs_stream),
        )
        .route("/api/models", get(api::dashboard::models_catalog))
        .route(
            "/api/models/alias",
            get(api::dashboard::model_alias_get)
                .put(api::dashboard::model_alias_set)
                .delete(api::dashboard::model_alias_delete),
        )
        .route(
            "/api/models/custom",
            get(api::dashboard::model_custom_get)
                .post(api::dashboard::model_custom_post)
                .delete(api::dashboard::model_custom_delete),
        )
        .route(
            "/api/models/disabled",
            get(api::dashboard::model_disabled_get)
                .post(api::dashboard::model_disabled_post)
                .delete(api::dashboard::model_disabled_delete),
        )
        .route(
            "/api/usage/request-details",
            get(api::dashboard::usage_request_details),
        )
        .route(
            "/api/usage/request-logs",
            get(api::dashboard::usage_request_logs),
        )
        .route(
            "/api/providers/test-batch",
            axum::routing::post(api::dashboard::providers_test_batch),
        )
        .route("/api/oauth/providers", get(api::dashboard::oauth_providers))
        .route(
            "/api/oauth/:provider/start",
            axum::routing::post(api::dashboard::oauth_start),
        )
        .route(
            "/api/oauth/:provider/exchange",
            axum::routing::post(api::dashboard::oauth_exchange),
        )
        .route(
            "/api/oauth/:provider/refresh",
            axum::routing::post(api::dashboard::oauth_refresh),
        )
        .route(
            "/api/translator/dumps",
            get(api::dashboard::translator_dumps_list),
        )
        .route(
            "/api/translator/dumps/:name",
            get(api::dashboard::translator_dumps_get),
        )
        .route(
            "/api/version/shutdown",
            axum::routing::post(api::shutdown),
        )
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

    // Reserve backend prefixes. In hybrid mode, unknown /api and /v1 paths
    // go to the Node reverse proxy; in standalone mode they return JSON 404.
    let backend_not_found = Router::new()
        .route("/api", any(api::not_found))
        .route("/api/*path", any(api::not_found))
        .route("/v1", any(api::not_found))
        .route("/v1/*path", any(api::not_found));

    let app = if !cfg.node_upstream.is_empty() {
        // HYBRID MODE: reverse-proxy everything to Node/Next.js.
        // Rust handles /health + /v1/* natively; Node handles all /api/*,
        // dashboard pages, static assets, SSE, open-sse, etc.
        info!(upstream = %cfg.node_upstream, "hybrid mode: reverse-proxying unhandled routes to Node");
        Router::new()
            .route("/health", get(api::health))
            .route("/v1/models", get(api::models))
            .route("/v1/chat/completions", post(proxy::chat_completions))
            .route("/v1/messages", post(anthropic::messages))
            .route("/v1/messages/count_tokens", post(anthropic::count_tokens))
            .route("/v1/embeddings", post(proxy::embeddings))
            .route("/v1/web/fetch", post(api::web_fetch))
            .route("/v1/search", post(api::web_search))
            .route("/v1/responses", post(api::v1_responses))
            .route("/v1/images/generations", post(api::v1_images))
            .route("/v1/audio/speech", post(api::v1_audio_speech))
            .fallback(proxy::reverse::proxy_to_node)
            .layer(RequestBodyLimitLayer::new(cfg.body_max_bytes))
            .layer(CorsLayer::very_permissive())
            .layer(TraceLayer::new_for_http())
            .with_state(state)
    } else {
        // STANDALONE MODE: serve the React SPA directly from Rust.
        let static_files = ServeDir::new(&cfg.static_dir)
            .fallback(ServeFile::new(cfg.static_dir.join("index.html")));
        Router::new()
            .route("/health", get(api::health))
            .route("/v1/models", get(api::models))
            .route("/v1/chat/completions", post(proxy::chat_completions))
            .route("/v1/messages", post(anthropic::messages))
            .route("/v1/messages/count_tokens", post(anthropic::count_tokens))
            .route("/v1/embeddings", post(proxy::embeddings))
            .route("/v1/web/fetch", post(api::web_fetch))
            .route("/v1/search", post(api::web_search))
            .route("/v1/responses", post(api::v1_responses))
            .route("/v1/images/generations", post(api::v1_images))
            .route("/v1/audio/speech", post(api::v1_audio_speech))
            .merge(auth_routes)
            .merge(dashboard_routes)
            .merge(backend_not_found)
            .fallback_service(static_files)
            .layer(RequestBodyLimitLayer::new(cfg.body_max_bytes))
            .layer(CorsLayer::very_permissive())
            .layer(TraceLayer::new_for_http())
            .with_state(state)
    };

    let listener = tokio::net::TcpListener::bind(cfg.addr())
        .await
        .unwrap_or_else(|e| panic!("bind {}: {e}", cfg.addr()));

    info!("listening on http://{}", cfg.addr());

    // systemd sends SIGTERM; local terminals send SIGINT. Trigger graceful
    // Axum shutdown for either, then enforce the configured ceiling so a stuck
    // SSE client cannot block a deployment indefinitely.
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();

    // ConnectInfo is required by the auth middleware so the loopback bypass
    // uses the real TCP peer address, not a spoofable Host header.
    let mut server = tokio::spawn(async move {
        axum::serve(listener, app.into_make_service_with_connect_info::<std::net::SocketAddr>())
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
