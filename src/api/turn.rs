use std::time::Duration;

use axum::{Json, extract::State, http::HeaderMap};
use base64::{Engine, engine::general_purpose::STANDARD};
use chrono::{DateTime, Utc};
use hmac::{Hmac, KeyInit, Mac};
use secrecy::ExposeSecret;
use serde::Serialize;
use sha1::Sha1;
use uuid::Uuid;

use crate::{model::runtime_instance::RuntimeInstance, state::AppState};

use super::{ApiError, instances::authenticate_instance};

const TURN_REQUESTS_PER_MINUTE_PER_PROFILE: u64 = 20;
const TURN_REQUESTS_PER_MINUTE_PER_INSTANCE: u64 = 10;

pub async fn credentials(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<TurnCredentialsResponse>, ApiError> {
    let (_, instance) = authenticate_instance(&state, &headers).await?;
    enforce_rate_limit(&state, &instance).await?;
    let expires_at = Utc::now()
        .checked_add_signed(
            chrono::Duration::from_std(state.config.turn_credential_ttl)
                .map_err(|_| ApiError::internal("TURN credential TTL is invalid"))?,
        )
        .ok_or_else(|| ApiError::internal("TURN credential expiry is out of range"))?;
    let username = turn_username(expires_at, instance.profile_id);
    let credential = turn_credential(state.config.turn_shared_secret.expose_secret(), &username)?;

    Ok(Json(TurnCredentialsResponse {
        urls: state.config.turn_urls.clone(),
        username,
        credential,
        expires_at,
    }))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnCredentialsResponse {
    urls: Vec<String>,
    username: String,
    credential: String,
    expires_at: DateTime<Utc>,
}

fn turn_username(expires_at: DateTime<Utc>, profile_id: Uuid) -> String {
    format!("{}:{profile_id}", expires_at.timestamp())
}

fn turn_credential(secret: &str, username: &str) -> Result<String, ApiError> {
    let mut mac = Hmac::<Sha1>::new_from_slice(secret.as_bytes())
        .map_err(|_| ApiError::internal("TURN shared secret is invalid"))?;
    mac.update(username.as_bytes());
    Ok(STANDARD.encode(mac.finalize().into_bytes()))
}

async fn enforce_rate_limit(state: &AppState, instance: &RuntimeInstance) -> Result<(), ApiError> {
    let window = Duration::from_secs(60);
    let profile_count = state
        .redis
        .increment_rate_limit(&format!("turn-profile:{}", instance.profile_id), window)
        .await
        .map_err(turn_storage_error)?;
    let instance_count = state
        .redis
        .increment_rate_limit(&format!("turn-instance:{}", instance.presence_id), window)
        .await
        .map_err(turn_storage_error)?;
    if profile_count > TURN_REQUESTS_PER_MINUTE_PER_PROFILE
        || instance_count > TURN_REQUESTS_PER_MINUTE_PER_INSTANCE
    {
        return Err(ApiError::rate_limited(
            "TURN credential request rate limit exceeded",
        ));
    }
    Ok(())
}

fn turn_storage_error(error: anyhow::Error) -> ApiError {
    tracing::error!(error = %error, "TURN rate-limit storage operation failed");
    ApiError::service_unavailable("TURN credential service is unavailable")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_coturn_rest_api_hmac_sha1_credential() {
        assert_eq!(
            turn_credential("key", "The quick brown fox jumps over the lazy dog").unwrap(),
            "3nybhbi3iqa8ino29wqQcBydtNk="
        );
    }

    #[test]
    fn username_contains_expiry_and_profile() {
        let profile_id = Uuid::nil();
        let expires_at = DateTime::from_timestamp(1_700_000_000, 0).unwrap();
        assert_eq!(
            turn_username(expires_at, profile_id),
            "1700000000:00000000-0000-0000-0000-000000000000"
        );
    }
}
