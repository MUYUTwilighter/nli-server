use std::{env, net::SocketAddr, time::Duration};

use anyhow::{Context, Result};
use axum::http::HeaderValue;
use reqwest::Url;
use secrecy::{ExposeSecret, SecretString};

#[derive(Clone)]
pub struct AppConfig {
    pub env: String,
    pub bind_addr: SocketAddr,
    pub database_url: String,
    pub redis_url: String,
    pub instance_token_ttl: Duration,
    pub presence_ttl: Duration,
    pub signaling_session_ttl: Duration,
    pub profile_cache_ttl: Duration,
    pub turn_urls: Vec<String>,
    pub turn_shared_secret: SecretString,
    pub turn_credential_ttl: Duration,
    pub cors_allow_origin: Option<HeaderValue>,
    pub trust_proxy_headers: bool,
    pub minecraft_profile_url: Url,
    pub minecraft_profile_by_name_url: Url,
    pub minecraft_profile_by_id_url: Url,
    pub minecraft_friends_url: Url,
}

impl AppConfig {
    pub fn from_env() -> Result<Self> {
        let config = Self {
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
            profile_cache_ttl: seconds_var("PROFILE_CACHE_TTL_SECONDS", 21_600)?,
            turn_urls: turn_urls()?,
            turn_shared_secret: SecretString::from(required_var("TURN_SHARED_SECRET")?),
            turn_credential_ttl: bounded_seconds_var(
                "TURN_CREDENTIAL_TTL_SECONDS",
                600,
                60,
                3_600,
            )?,
            cors_allow_origin: env::var("NLI_CORS_ALLOW_ORIGIN")
                .ok()
                .map(|value| {
                    value
                        .parse()
                        .context("NLI_CORS_ALLOW_ORIGIN must be a valid HTTP header value")
                })
                .transpose()?,
            trust_proxy_headers: bool_var("NLI_TRUST_PROXY_HEADERS", false)?,
            minecraft_profile_url: env::var("MINECRAFT_PROFILE_URL")
                .unwrap_or_else(|_| {
                    "https://api.minecraftservices.com/minecraft/profile".to_owned()
                })
                .parse()
                .context("MINECRAFT_PROFILE_URL must be a valid URL")?,
            minecraft_profile_by_name_url: directory_url(
                "MINECRAFT_PROFILE_BY_NAME_URL",
                "https://api.mojang.com/users/profiles/minecraft/",
            )?,
            minecraft_profile_by_id_url: directory_url(
                "MINECRAFT_PROFILE_BY_ID_URL",
                "https://sessionserver.mojang.com/session/minecraft/profile/",
            )?,
            minecraft_friends_url: env::var("MINECRAFT_FRIENDS_URL")
                .unwrap_or_else(|_| "https://api.minecraftservices.com/friends".to_owned())
                .parse()
                .context("MINECRAFT_FRIENDS_URL must be a valid URL")?,
        };
        config.validate()?;
        Ok(config)
    }

    fn validate(&self) -> Result<()> {
        if self.profile_cache_ttl.is_zero() {
            anyhow::bail!("PROFILE_CACHE_TTL_SECONDS must be greater than zero");
        }
        if self.env.eq_ignore_ascii_case("production") {
            if self.bind_addr.ip().is_loopback() {
                anyhow::bail!("NLI_BIND_ADDR must not use a loopback address in production");
            }
            if self.turn_shared_secret.expose_secret().len() < 32
                || self
                    .turn_shared_secret
                    .expose_secret()
                    .contains("change-me")
            {
                anyhow::bail!("TURN_SHARED_SECRET must be a strong production secret");
            }
            for (name, url) in [
                ("MINECRAFT_PROFILE_URL", &self.minecraft_profile_url),
                (
                    "MINECRAFT_PROFILE_BY_NAME_URL",
                    &self.minecraft_profile_by_name_url,
                ),
                (
                    "MINECRAFT_PROFILE_BY_ID_URL",
                    &self.minecraft_profile_by_id_url,
                ),
                ("MINECRAFT_FRIENDS_URL", &self.minecraft_friends_url),
            ] {
                if url.scheme() != "https" {
                    anyhow::bail!("{name} must use HTTPS in production");
                }
            }
            if let Some(origin) = &self.cors_allow_origin {
                let origin = origin
                    .to_str()
                    .context("NLI_CORS_ALLOW_ORIGIN must be text")?;
                if !origin.starts_with("https://") {
                    anyhow::bail!("NLI_CORS_ALLOW_ORIGIN must use HTTPS in production");
                }
            }
        }
        Ok(())
    }
}

fn bool_var(name: &str, default: bool) -> Result<bool> {
    match env::var(name) {
        Ok(value) if value.eq_ignore_ascii_case("true") || value == "1" => Ok(true),
        Ok(value) if value.eq_ignore_ascii_case("false") || value == "0" => Ok(false),
        Ok(_) => anyhow::bail!("{name} must be true, false, 1, or 0"),
        Err(env::VarError::NotPresent) => Ok(default),
        Err(error) => Err(error).with_context(|| format!("failed to read {name}")),
    }
}

fn directory_url(name: &str, default: &str) -> Result<Url> {
    let value = env::var(name).unwrap_or_else(|_| default.to_owned());
    let mut url: Url = value
        .parse()
        .with_context(|| format!("{name} must be a valid URL"))?;
    if !url.path().ends_with('/') {
        let path = format!("{}/", url.path());
        url.set_path(&path);
    }
    Ok(url)
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

fn bounded_seconds_var(name: &str, default: u64, min: u64, max: u64) -> Result<Duration> {
    let duration = seconds_var(name, default)?;
    if !(min..=max).contains(&duration.as_secs()) {
        anyhow::bail!("{name} must be between {min} and {max} seconds");
    }
    Ok(duration)
}

fn turn_urls() -> Result<Vec<String>> {
    let raw = required_var("TURN_URLS")?;
    let urls = raw
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(validate_turn_url)
        .collect::<Result<Vec<_>>>()?;
    if urls.is_empty() {
        anyhow::bail!("TURN_URLS must contain at least one URL");
    }
    Ok(urls)
}

fn validate_turn_url(value: &str) -> Result<String> {
    if value.chars().any(char::is_control)
        || !["stun:", "stuns:", "turn:", "turns:"]
            .iter()
            .any(|scheme| value.starts_with(scheme))
    {
        anyhow::bail!("TURN_URLS contains an invalid STUN or TURN URL");
    }
    Ok(value.to_owned())
}
