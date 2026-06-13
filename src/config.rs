use std::{env, net::SocketAddr, time::Duration};

use anyhow::{Context, Result};
use axum::http::HeaderValue;
use reqwest::Url;

#[derive(Clone)]
pub struct AppConfig {
    pub env: String,
    pub bind_addr: SocketAddr,
    pub database_url: String,
    pub redis_url: String,
    pub instance_token_ttl: Duration,
    pub presence_ttl: Duration,
    pub signaling_session_ttl: Duration,
    pub cors_allow_origin: Option<HeaderValue>,
    pub minecraft_profile_url: Url,
}

impl AppConfig {
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            env: env::var("NLI_ENV").unwrap_or_else(|_| "development".to_owned()),
            bind_addr: env::var("NLI_BIND_ADDR")
                .unwrap_or_else(|_| "127.0.0.1:8080".to_owned())
                .parse()
                .context("NLI_BIND_ADDR must be a valid socket address")?,
            database_url: required_var("DATABASE_URL")?,
            redis_url: required_var("REDIS_URL")?,
            instance_token_ttl: seconds_var("INSTANCE_TOKEN_TTL_SECONDS", 1_800)?,
            presence_ttl: seconds_var("PRESENCE_TTL_SECONDS", 90)?,
            signaling_session_ttl: seconds_var("SIGNALING_SESSION_TTL_SECONDS", 300)?,
            cors_allow_origin: env::var("NLI_CORS_ALLOW_ORIGIN")
                .ok()
                .map(|value| {
                    value
                        .parse()
                        .context("NLI_CORS_ALLOW_ORIGIN must be a valid HTTP header value")
                })
                .transpose()?,
            minecraft_profile_url: env::var("MINECRAFT_PROFILE_URL")
                .unwrap_or_else(|_| {
                    "https://api.minecraftservices.com/minecraft/profile".to_owned()
                })
                .parse()
                .context("MINECRAFT_PROFILE_URL must be a valid URL")?,
        })
    }
}

fn required_var(name: &str) -> Result<String> {
    env::var(name).with_context(|| format!("{name} must be set"))
}

fn seconds_var(name: &str, default: u64) -> Result<Duration> {
    let raw = env::var(name).unwrap_or_else(|_| default.to_string());
    let seconds = raw
        .parse::<u64>()
        .with_context(|| format!("{name} must be an unsigned integer"))?;

    Ok(Duration::from_secs(seconds))
}
