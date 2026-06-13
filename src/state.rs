use std::sync::Arc;

use anyhow::{Context, Result};
use reqwest::Client;

use crate::{config::AppConfig, db::DbPool, redis::RedisStore};

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<AppConfig>,
    pub db: DbPool,
    pub redis: RedisStore,
    pub http: Client,
}

impl AppState {
    pub fn new(config: AppConfig, db: DbPool, redis: RedisStore) -> Result<Self> {
        let http = Client::builder()
            .user_agent(concat!("nli-server/", env!("CARGO_PKG_VERSION")))
            .https_only(true)
            .build()
            .context("failed to build HTTP client")?;

        Ok(Self {
            config: Arc::new(config),
            db,
            redis,
            http,
        })
    }

    pub async fn db_health(&self) -> anyhow::Result<()> {
        crate::db::health_check(&self.db).await
    }

    pub async fn redis_health(&self) -> anyhow::Result<()> {
        self.redis.health_check().await
    }
}
