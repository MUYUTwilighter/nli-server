use std::{sync::Arc, time::Duration};

use anyhow::{Context, Result};
use metrics_exporter_prometheus::PrometheusHandle;
use reqwest::Client;

use crate::{
    auth::{MinecraftAuthClient, MinecraftProfileClient, MinecraftSocialClient},
    config::AppConfig,
    db::DbPool,
    redis::RedisStore,
    signaling::SignalingConnections,
};

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<AppConfig>,
    pub db: DbPool,
    pub redis: RedisStore,
    pub http: Client,
    pub minecraft_auth: MinecraftAuthClient,
    pub minecraft_profiles: MinecraftProfileClient,
    pub minecraft_social: MinecraftSocialClient,
    pub signaling_connections: SignalingConnections,
    pub metrics: Option<PrometheusHandle>,
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
        let minecraft_profiles = MinecraftProfileClient::new(
            http.clone(),
            config.minecraft_profile_by_name_url.clone(),
            config.minecraft_profile_by_id_url.clone(),
        );
        let minecraft_social =
            MinecraftSocialClient::new(http.clone(), config.minecraft_friends_url.clone());

        Self {
            config: Arc::new(config),
            db,
            redis,
            http,
            minecraft_auth,
            minecraft_profiles,
            minecraft_social,
            signaling_connections: SignalingConnections::default(),
            metrics: None,
        }
    }

    pub fn with_metrics(mut self, metrics: PrometheusHandle) -> Self {
        self.metrics = Some(metrics);
        self
    }

    pub async fn db_health(&self) -> anyhow::Result<()> {
        crate::db::health_check(&self.db).await
    }

    pub async fn redis_health(&self) -> anyhow::Result<()> {
        self.redis.health_check().await
    }
}
