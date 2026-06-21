use axum::{
    Json,
    extract::{ConnectInfo, Extension, State},
    http::{HeaderMap, StatusCode},
};
use chrono::{DateTime, Utc};
use secrecy::ExposeSecret;
use serde::{Deserialize, Serialize};
use std::net::{IpAddr, SocketAddr};
use tracing::error;
use uuid::Uuid;

use crate::{
    auth::bearer_token,
    model::{
        presence::{Presence, PresenceStatus},
        runtime_instance::RuntimeInstance,
        token::{RuntimeInstanceToken, RuntimeTokenHash},
    },
    state::AppState,
};

use super::{ApiError, auth::authenticate_minecraft};

const DEFAULT_DISPLAY_TEXT: &str = "Minecraft Java instance";
const MAX_DISPLAY_TEXT_CHARS: usize = 96;
const INSTANCE_CREATE_LIMIT_PER_MINUTE: u64 = 10;
const MAX_RUNTIME_INSTANCES_PER_PROFILE: usize = 5;
const INSTANCE_PREAUTH_LIMIT_PER_MINUTE_PER_IP: u64 = 60;
const INSTANCE_PREAUTH_LIMIT_PER_MINUTE_GLOBAL: u64 = 600;

pub async fn create(
    State(state): State<AppState>,
    connect_info: Option<Extension<ConnectInfo<SocketAddr>>>,
    headers: HeaderMap,
    Json(request): Json<CreateInstanceRequest>,
) -> Result<Json<RegisterInstanceResponse>, ApiError> {
    enforce_preauth_rate_limit(&state, &headers, connect_info.map(|value| value.0.0.ip())).await?;
    let authenticated = authenticate_minecraft(&state, &headers).await?;
    let identity = authenticated.identity;
    let display_text = sanitize_display_text(request.display_text)?;
    enforce_creation_rate_limit(&state, identity.profile_id).await?;

    let now = Utc::now();
    let presence_id = Uuid::new_v4().to_string();
    let expires_at = add_duration(now, state.config.instance_token_ttl)?;
    let token = RuntimeInstanceToken::generate();
    let token_hash = token.hash();
    let instance = RuntimeInstance {
        profile_id: identity.profile_id,
        presence_id: presence_id.clone(),
        instance_started_at: now,
        issued_at: now,
        expires_at,
    };

    let registered = state
        .redis
        .register_runtime_instance(
            &token_hash,
            &instance,
            state.config.instance_token_ttl,
            MAX_RUNTIME_INSTANCES_PER_PROFILE,
        )
        .await
        .map_err(redis_error)?;
    if !registered {
        return Err(ApiError::conflict(
            "INSTANCE_LIMIT_REACHED",
            "A Minecraft profile may have at most 5 active runtime instances",
        ));
    }

    let presence = Presence {
        profile_id: identity.profile_id,
        presence_id: presence_id.clone(),
        status: PresenceStatus::Online,
        joinable: false,
        session_id: None,
        endpoint: None,
        display_text,
        updated_at: now,
        expires_at: now,
    };
    if let Err(error) = state
        .redis
        .put_presence(&presence, state.config.presence_ttl)
        .await
    {
        error!(error = %error, %presence_id, "failed to create initial Presence");
        if let Err(cleanup_error) = state.redis.delete_runtime_instance(&token_hash).await {
            error!(error = %cleanup_error, %presence_id, "failed to roll back runtime instance");
        }
        return Err(ApiError::service_unavailable(
            "Runtime instance storage is unavailable",
        ));
    }

    if let Err(error) = state
        .redis
        .cache_profile(
            identity.profile_id,
            &identity.name,
            state.config.profile_cache_ttl,
        )
        .await
    {
        error!(error = %error, profile_id = %identity.profile_id, "failed to cache registered Minecraft profile");
    }
    synchronize_official_friends(&state, &authenticated.access_token, &identity).await;

    Ok(Json(RegisterInstanceResponse::new(
        &instance,
        &token,
        identity.name,
    )))
}

async fn enforce_preauth_rate_limit(
    state: &AppState,
    headers: &HeaderMap,
    peer_ip: Option<IpAddr>,
) -> Result<(), ApiError> {
    let client = registration_client_key(state, headers, peer_ip);
    let window = std::time::Duration::from_secs(60);
    let client_count = state
        .redis
        .increment_rate_limit(&format!("instance-preauth-ip:{client}"), window)
        .await
        .map_err(redis_error)?;
    let global_count = state
        .redis
        .increment_rate_limit("instance-preauth-global", window)
        .await
        .map_err(redis_error)?;
    if client_count > INSTANCE_PREAUTH_LIMIT_PER_MINUTE_PER_IP
        || global_count > INSTANCE_PREAUTH_LIMIT_PER_MINUTE_GLOBAL
    {
        metrics::counter!("nli_rate_limited_total", "endpoint" => "instance_preauth").increment(1);
        return Err(ApiError::rate_limited(
            "Runtime instance authentication rate limit exceeded",
        ));
    }
    Ok(())
}

fn registration_client_key(
    state: &AppState,
    headers: &HeaderMap,
    peer_ip: Option<IpAddr>,
) -> String {
    if state.config.trust_proxy_headers
        && let Some(value) = headers
            .get("x-forwarded-for")
            .and_then(|value| value.to_str().ok())
        && let Some(address) = value.split(',').next().map(str::trim)
        && let Ok(ip) = address.parse::<IpAddr>()
    {
        return ip.to_string();
    }
    peer_ip.map_or_else(|| "unknown".to_owned(), |ip| ip.to_string())
}

async fn synchronize_official_friends(
    state: &AppState,
    access_token: &secrecy::SecretString,
    identity: &crate::auth::ProfileIdentity,
) {
    let snapshot = match state.minecraft_social.friends(access_token).await {
        Ok(snapshot) => snapshot,
        Err(error) => {
            metrics::counter!("nli_official_friend_sync_total", "operation" => "register", "result" => "upstream_error").increment(1);
            tracing::warn!(error = %error, profile_id = %identity.profile_id, "official friend synchronization skipped");
            return;
        }
    };
    for friend in snapshot
        .friends
        .iter()
        .chain(&snapshot.incoming_requests)
        .chain(&snapshot.outgoing_requests)
    {
        if let Err(error) = state
            .redis
            .cache_profile(
                friend.profile_id,
                &friend.name,
                state.config.profile_cache_ttl,
            )
            .await
        {
            tracing::warn!(error = %error, profile_id = %friend.profile_id, "failed to cache official friend profile");
        }
    }
    let friend_ids = snapshot
        .friends
        .iter()
        .map(|friend| friend.profile_id)
        .collect::<Vec<_>>();
    let incoming_ids = snapshot
        .incoming_requests
        .iter()
        .map(|friend| friend.profile_id)
        .collect::<Vec<_>>();
    let outgoing_ids = snapshot
        .outgoing_requests
        .iter()
        .map(|friend| friend.profile_id)
        .collect::<Vec<_>>();
    match crate::db::friends::FriendRepository::new(state.db.clone())
        .replace_with_official_snapshot(
            identity.profile_id,
            &friend_ids,
            &incoming_ids,
            &outgoing_ids,
        )
        .await
    {
        Ok(()) => {
            metrics::counter!("nli_official_friend_sync_total", "operation" => "register", "result" => "success").increment(1);
            tracing::info!(profile_id = %identity.profile_id, friends = friend_ids.len(), incoming = incoming_ids.len(), outgoing = outgoing_ids.len(), "official friends synchronized");
        }
        Err(error) => {
            metrics::counter!("nli_official_friend_sync_total", "operation" => "register", "result" => "storage_error").increment(1);
            tracing::error!(error = %error, profile_id = %identity.profile_id, "failed to synchronize official friends");
        }
    }
}

pub async fn renew(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<InstanceResponse>, ApiError> {
    let (old_hash, mut instance) = authenticate_instance(&state, &headers).await?;
    let now = Utc::now();

    let new_token = RuntimeInstanceToken::generate();
    let new_hash = new_token.hash();
    instance.issued_at = now;
    instance.expires_at = add_duration(now, state.config.instance_token_ttl)?;
    let rotated = state
        .redis
        .rotate_runtime_instance(
            &old_hash,
            &new_hash,
            &instance,
            state.config.instance_token_ttl,
        )
        .await
        .map_err(redis_error)?;
    if !rotated {
        return Err(invalid_instance_token());
    }

    Ok(Json(InstanceResponse::new(&instance, &new_token)))
}

pub async fn close(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    let (token_hash, instance) = authenticate_instance(&state, &headers).await?;
    let closed = state
        .redis
        .close_runtime_instance(&token_hash, &instance)
        .await
        .map_err(redis_error)?;
    if !closed {
        return Err(invalid_instance_token());
    }

    state
        .signaling_connections
        .close(&instance.presence_id)
        .await;
    state
        .redis
        .delete_signaling_sessions_for_presence(&instance.presence_id)
        .await
        .map_err(redis_error)?;

    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateInstanceRequest {
    display_text: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceResponse {
    profile_id: Uuid,
    presence_id: String,
    instance_token: String,
    expires_at: DateTime<Utc>,
}

impl InstanceResponse {
    fn new(instance: &RuntimeInstance, token: &RuntimeInstanceToken) -> Self {
        Self {
            profile_id: instance.profile_id,
            presence_id: instance.presence_id.clone(),
            instance_token: token.expose_secret().to_owned(),
            expires_at: instance.expires_at,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisterInstanceResponse {
    profile_id: Uuid,
    name: String,
    presence_id: String,
    instance_token: String,
    expires_at: DateTime<Utc>,
}

impl RegisterInstanceResponse {
    fn new(instance: &RuntimeInstance, token: &RuntimeInstanceToken, name: String) -> Self {
        Self {
            profile_id: instance.profile_id,
            name,
            presence_id: instance.presence_id.clone(),
            instance_token: token.expose_secret().to_owned(),
            expires_at: instance.expires_at,
        }
    }
}

async fn enforce_creation_rate_limit(state: &AppState, profile_id: Uuid) -> Result<(), ApiError> {
    let count = state
        .redis
        .increment_rate_limit(
            &format!("instance-create:{profile_id}"),
            std::time::Duration::from_secs(60),
        )
        .await
        .map_err(redis_error)?;
    if count > INSTANCE_CREATE_LIMIT_PER_MINUTE {
        metrics::counter!("nli_rate_limited_total", "endpoint" => "instance_create").increment(1);
        return Err(ApiError::rate_limited(
            "Runtime instance creation rate limit exceeded",
        ));
    }
    Ok(())
}

pub(super) fn sanitize_display_text(value: Option<String>) -> Result<String, ApiError> {
    let value = value.unwrap_or_else(|| DEFAULT_DISPLAY_TEXT.to_owned());
    let value = value.trim();
    if value.is_empty() {
        return Ok(DEFAULT_DISPLAY_TEXT.to_owned());
    }
    if value.chars().any(char::is_control) {
        return Err(ApiError::bad_request(
            "INVALID_DISPLAY_TEXT",
            "displayText must not contain control characters",
        ));
    }
    if value.chars().count() > MAX_DISPLAY_TEXT_CHARS {
        return Err(ApiError::bad_request(
            "INVALID_DISPLAY_TEXT",
            "displayText exceeds 96 characters",
        ));
    }
    Ok(value.to_owned())
}

fn add_duration(
    now: DateTime<Utc>,
    duration: std::time::Duration,
) -> Result<DateTime<Utc>, ApiError> {
    let duration = chrono::Duration::from_std(duration)
        .map_err(|_| ApiError::internal("Runtime instance TTL is invalid"))?;
    now.checked_add_signed(duration)
        .ok_or_else(|| ApiError::internal("Runtime instance expiry is out of range"))
}

pub(super) async fn authenticate_instance(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<(RuntimeTokenHash, RuntimeInstance), ApiError> {
    let token = bearer_token(headers)
        .map_err(|error| ApiError::unauthorized("UNAUTHORIZED", error.to_string()))?;
    let token_hash = RuntimeTokenHash::from_token(token.expose_secret());
    let Some(instance) = state
        .redis
        .runtime_instance(&token_hash)
        .await
        .map_err(redis_error)?
    else {
        return Err(invalid_instance_token());
    };

    if instance.is_expired_at(Utc::now()) {
        let _ = state.redis.delete_runtime_instance(&token_hash).await;
        return Err(invalid_instance_token());
    }

    Ok((token_hash, instance))
}

fn invalid_instance_token() -> ApiError {
    ApiError::unauthorized(
        "INVALID_INSTANCE_TOKEN",
        "Runtime instance token is invalid or expired",
    )
}

pub(super) fn redis_error(error: anyhow::Error) -> ApiError {
    error!(error = %error, "runtime instance Redis operation failed");
    ApiError::service_unavailable("Runtime instance storage is unavailable")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitizes_display_text() {
        assert_eq!(sanitize_display_text(None).unwrap(), DEFAULT_DISPLAY_TEXT);
        assert_eq!(
            sanitize_display_text(Some("  Test world  ".to_owned())).unwrap(),
            "Test world"
        );
        assert_eq!(
            sanitize_display_text(Some("  ".to_owned())).unwrap(),
            DEFAULT_DISPLAY_TEXT
        );
        assert!(sanitize_display_text(Some("bad\ntext".to_owned())).is_err());
        assert!(sanitize_display_text(Some("x".repeat(97))).is_err());
        assert!(sanitize_display_text(Some("界".repeat(96))).is_ok());
    }
}
