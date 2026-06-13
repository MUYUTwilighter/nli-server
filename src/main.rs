use anyhow::{Context, Result};
use nli_server::{
    api::{AppState, router},
    config::AppConfig,
    db,
    observability::install_metrics,
    redis::RedisStore,
};
use std::net::SocketAddr;
use tokio::{net::TcpListener, signal};
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    init_tracing();

    let config = AppConfig::from_env()?;
    let db_pool = db::connect(&config.database_url).await?;
    let redis = RedisStore::connect(&config.redis_url).await?;
    let bind_addr = config.bind_addr;
    let metrics = install_metrics().context("failed to install metrics recorder")?;
    let state = AppState::new(config, db_pool, redis)?.with_metrics(metrics);
    let shutdown_state = state.clone();
    let app = router(state);
    let listener = TcpListener::bind(bind_addr)
        .await
        .with_context(|| format!("failed to bind server to {bind_addr}"))?;

    info!(
        bind_addr = %bind_addr,
        "NetherLink server listening"
    );

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal(shutdown_state))
    .await
    .context("HTTP server failed")?;

    Ok(())
}

async fn shutdown_signal(state: AppState) {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }

    let closed_connections = state.signaling_connections.close_all().await;
    info!(closed_connections, "shutdown signal received");
}

fn init_tracing() {
    let json = std::env::var("NLI_ENV").is_ok_and(|value| value.eq_ignore_ascii_case("production"));
    if json {
        tracing_subscriber::registry()
            .with(
                tracing_subscriber::EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| "nli_server=info,tower_http=info".into()),
            )
            .with(tracing_subscriber::fmt::layer().json())
            .init();
        return;
    }
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "nli_server=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();
}
