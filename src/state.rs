use std::{sync::Arc, time::Duration};

use anyhow::{Context, Result};
use reqwest::Client;

use crate::{auth::MinecraftAuthClient, config::AppConfig, db::DbPool, redis::RedisStore};

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<AppConfig>,
    pub db: DbPool,
    pub redis: RedisStore,
    pub http: Client,
    pub minecraft_auth: MinecraftAuthClient,
}

impl AppState {
    pub fn new(config: AppConfig, db: DbPool, redis: RedisStore) -> Result<Self> {
        let http = Client::builder()
            .user_agent(concat!("nli-server/", env!("CARGO_PKG_VERSION")))
            .https_only(true)
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(10))
            .build()
            .context("failed to build HTTP client")?;
        Ok(Self::with_http_client(config, db, redis, http))
    }

    pub fn with_http_client(
        config: AppConfig,
        db: DbPool,
        redis: RedisStore,
        http: Client,
    ) -> Self {
        let minecraft_auth =
            MinecraftAuthClient::new(http.clone(), config.minecraft_profile_url.clone());

        Self {
            config: Arc::new(config),
            db,
            redis,
            http,
            minecraft_auth,
        }
    }

    pub async fn db_health(&self) -> anyhow::Result<()> {
        crate::db::health_check(&self.db).await
    }

    pub async fn redis_health(&self) -> anyhow::Result<()> {
        self.redis.health_check().await
    }
}
