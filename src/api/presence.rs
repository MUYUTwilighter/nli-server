use std::time::Duration;

use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    model::presence::{Presence, PresenceStatus},
    state::AppState,
};

use super::{
    ApiError,
    instances::{authenticate_instance, redis_error, sanitize_display_text},
};

const MIN_PRESENCE_TTL_SECONDS: u64 = 30;
const MAX_PRESENCE_TTL_SECONDS: u64 = 180;
const PRESENCE_PUBLISH_WINDOW: Duration = Duration::from_secs(10);
const PRESENCE_PUBLISH_LIMIT_PER_WINDOW: u64 = 3;
const MAX_SESSION_ID_CHARS: usize = 128;
const MAX_ENDPOINT_CHARS: usize = 512;

pub async fn publish(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<PublishPresenceRequest>,
) -> Result<Json<PublishPresenceResponse>, ApiError> {
    validate_status(request.status, request.joinable)?;
    let session_id = sanitize_optional_text(
        request.session_id,
        MAX_SESSION_ID_CHARS,
        "INVALID_SESSION_ID",
        "sessionId",
    )?;
    let endpoint = sanitize_optional_text(
        request.endpoint,
        MAX_ENDPOINT_CHARS,
        "INVALID_ENDPOINT",
        "endpoint",
    )?;
    let requested_display_text = request
        .display_text
        .map(|value| sanitize_display_text(Some(value)))
        .transpose()?;
    let (_, instance) = authenticate_instance(&state, &headers).await?;
    enforce_publish_rate_limit(&state, &instance.presence_id).await?;

    let current_presence = state
        .redis
        .presence(&instance.presence_id)
        .await
        .map_err(redis_error)?;
    let display_text = match requested_display_text {
        Some(display_text) => display_text,
        None => current_presence
            .map(|presence| presence.display_text)
            .map_or_else(|| sanitize_display_text(None), Ok)?,
    };
    let ttl_seconds = request
        .ttl_seconds
        .unwrap_or(state.config.presence_ttl.as_secs())
        .clamp(MIN_PRESENCE_TTL_SECONDS, MAX_PRESENCE_TTL_SECONDS);
    let now = Utc::now();
    let presence = Presence {
        profile_id: instance.profile_id,
        presence_id: instance.presence_id.clone(),
        status: request.status,
        joinable: request.joinable,
        session_id,
        endpoint,
        display_text,
        updated_at: now,
        expires_at: now,
    };
    let presence = state
        .redis
        .put_presence(&presence, Duration::from_secs(ttl_seconds))
        .await
        .map_err(redis_error)?;

    Ok(Json(PublishPresenceResponse {
        result: "SUCCESS",
        profile_id: presence.profile_id,
        presence_id: presence.presence_id,
        expires_at: presence.expires_at,
    }))
}

pub async fn clear(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    let (_, instance) = authenticate_instance(&state, &headers).await?;
    state
        .redis
        .delete_presence(&instance.presence_id)
        .await
        .map_err(redis_error)?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PublishPresenceRequest {
    status: PresenceStatus,
    #[serde(default)]
    joinable: bool,
    session_id: Option<String>,
    endpoint: Option<String>,
    ttl_seconds: Option<u64>,
    display_text: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishPresenceResponse {
    result: &'static str,
    profile_id: Uuid,
    presence_id: String,
    expires_at: DateTime<Utc>,
}

fn validate_status(status: PresenceStatus, joinable: bool) -> Result<(), ApiError> {
    if status == PresenceStatus::Offline {
        return Err(ApiError::bad_request(
            "INVALID_PRESENCE_STATUS",
            "Use DELETE /v1/presence to publish OFFLINE",
        ));
    }
    if joinable && status != PresenceStatus::Hosting {
        return Err(ApiError::bad_request(
            "INVALID_PRESENCE_STATE",
            "joinable can only be true when status is HOSTING",
        ));
    }
    Ok(())
}

fn sanitize_optional_text(
    value: Option<String>,
    max_chars: usize,
    code: &'static str,
    field: &'static str,
) -> Result<Option<String>, ApiError> {
    let Some(value) = value else {
        return Ok(None);
    };
    let value = value.trim();
    if value.is_empty() {
        return Ok(None);
    }
    if value.chars().any(char::is_control) || value.chars().count() > max_chars {
        return Err(ApiError::bad_request(code, format!("{field} is invalid")));
    }
    Ok(Some(value.to_owned()))
}

async fn enforce_publish_rate_limit(state: &AppState, presence_id: &str) -> Result<(), ApiError> {
    let count = state
        .redis
        .increment_rate_limit(
            &format!("presence-publish:{presence_id}"),
            PRESENCE_PUBLISH_WINDOW,
        )
        .await
        .map_err(redis_error)?;
    if count > PRESENCE_PUBLISH_LIMIT_PER_WINDOW {
        metrics::counter!("nli_rate_limited_total", "endpoint" => "presence_publish").increment(1);
        return Err(ApiError::rate_limited(
            "Presence publish rate limit exceeded",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_status_and_joinability() {
        assert!(validate_status(PresenceStatus::Online, false).is_ok());
        assert!(validate_status(PresenceStatus::InGame, false).is_ok());
        assert!(validate_status(PresenceStatus::Hosting, true).is_ok());
        assert!(validate_status(PresenceStatus::Offline, false).is_err());
        assert!(validate_status(PresenceStatus::Online, true).is_err());
    }

    #[test]
    fn sanitizes_optional_presence_fields() {
        assert_eq!(
            sanitize_optional_text(Some("  session  ".to_owned()), 128, "INVALID", "sessionId")
                .unwrap(),
            Some("session".to_owned())
        );
        assert_eq!(
            sanitize_optional_text(Some("  ".to_owned()), 128, "INVALID", "sessionId").unwrap(),
            None
        );
        assert!(
            sanitize_optional_text(Some("bad\nvalue".to_owned()), 128, "INVALID", "sessionId")
                .is_err()
        );
    }
}
