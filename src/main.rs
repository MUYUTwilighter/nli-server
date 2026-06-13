use anyhow::{Context, Result};
use nli_server::{api, config::AppConfig, db, redis::RedisStore, state::AppState};
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
    let state = AppState::new(config, db_pool, redis)?;
    let app = api::router(state);
    let listener = TcpListener::bind(bind_addr)
        .await
        .with_context(|| format!("failed to bind server to {bind_addr}"))?;

    info!(
        bind_addr = %bind_addr,
        "NetherLink server listening"
    );

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("HTTP server failed")?;

    Ok(())
}

async fn shutdown_signal() {
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

    info!("shutdown signal received");
}

fn init_tracing() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "nli_server=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();
}
