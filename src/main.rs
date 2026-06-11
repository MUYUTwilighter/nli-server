use anyhow::Result;
use nli_server::{config::AppConfig, db, redis::RedisStore};
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    init_tracing();

    let config = AppConfig::from_env()?;
    let _db_pool = db::connect(&config.database_url).await?;
    let _redis = RedisStore::connect(&config.redis_url).await?;

    info!(
        env = %config.env,
        bind_addr = %config.bind_addr,
        "PostgreSQL and Redis connections established"
    );

    Ok(())
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
