use std::{collections::HashMap, time::Duration};

use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
};
use secrecy::SecretString;
use serde::{Deserialize, Serialize};
use tracing::{error, warn};
use uuid::Uuid;

use crate::{
    auth::{
        MinecraftAuthError, MinecraftProfileError, MinecraftSocialError, OfficialFriend,
        OfficialFriendSnapshot,
    },
    db::friends::FriendRepository,
    model::friend::{FriendRequest, FriendSource, Friendship},
    model::presence::Presence,
    model::runtime_instance::RuntimeInstance,
    state::AppState,
};

use super::{ApiError, instances::authenticate_instance};

const FRIEND_MUTATION_LIMIT_PER_MINUTE: u64 = 10;

pub async fn snapshot(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<FriendSnapshotResponse>, ApiError> {
    let (caller, access_token) = authenticate_official_friend_request(&state, &headers).await?;
    let official = state
        .minecraft_social
        .friends(&access_token)
        .await
        .map_err(|error| social_error("refresh", error))?;
    synchronize_official_snapshot(&state, caller.profile_id, &official).await?;
    render_snapshot(&state, caller.profile_id).await
}

async fn render_snapshot(
    state: &AppState,
    caller_profile_id: Uuid,
) -> Result<Json<FriendSnapshotResponse>, ApiError> {
    let snapshot = FriendRepository::new(state.db.clone())
        .snapshot(caller_profile_id)
        .await
        .map_err(repository_error)?;

    let mut profile_ids = Vec::new();
    for friendship in &snapshot.friends {
        profile_ids.push(friend_profile_id(friendship, caller_profile_id));
    }
    for request in &snapshot.incoming_requests {
        profile_ids.push(request.requester_profile_id);
    }
    for request in &snapshot.outgoing_requests {
        profile_ids.push(request.target_profile_id);
    }
    profile_ids.sort_unstable();
    profile_ids.dedup();
    let names = resolve_names(state, profile_ids).await?;
    let self_presences = sorted_presences_for_profile(state, caller_profile_id).await?;
    let mut presences =
        resolve_friend_presences(state, &snapshot.friends, caller_profile_id).await?;

    Ok(Json(FriendSnapshotResponse {
        self_presences,
        friends: snapshot
            .friends
            .into_iter()
            .map(|friendship| {
                let profile_id = friend_profile_id(&friendship, caller_profile_id);
                FriendResponse {
                    profile_id,
                    name: names.get(&profile_id).cloned().flatten(),
                    source: friendship.source,
                    presences: presences.remove(&profile_id).unwrap_or_default(),
                }
            })
            .collect(),
        incoming_requests: snapshot
            .incoming_requests
            .into_iter()
            .map(|request| request_response(request, true, &names))
            .collect(),
        outgoing_requests: snapshot
            .outgoing_requests
            .into_iter()
            .map(|request| request_response(request, false, &names))
            .collect(),
    }))
}

pub async fn add_request(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<AddFriendRequest>,
) -> Result<Json<FriendMutationResponse>, ApiError> {
    let (caller, access_token) = authenticate_official_friend_request(&state, &headers).await?;
    let name = validate_player_name(&request.name)?;
    enforce_mutation_rate_limit(&state, caller.profile_id).await?;
    let official = state
        .minecraft_social
        .add_friend_by_name(&access_token, name)
        .await
        .map_err(|error| social_error("add", error))?;
    let relationship = relationship_for_name(&official, name).ok_or_else(|| {
        ApiError::bad_gateway(
            "INVALID_OFFICIAL_FRIEND_RESPONSE",
            "Minecraft friends service returned no relationship for the requested player",
        )
    })?;
    synchronize_official_snapshot(&state, caller.profile_id, &official).await?;

    Ok(Json(FriendMutationResponse {
        result: "SUCCESS",
        relationship,
        official_sync: "SUCCESS",
    }))
}

pub async fn update_settings(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<FriendSettingsRequest>,
) -> Result<Json<FriendSettingsResponse>, ApiError> {
    let (caller, access_token) = authenticate_official_friend_request(&state, &headers).await?;
    enforce_mutation_rate_limit(&state, caller.profile_id).await?;
    state
        .minecraft_social
        .update_friend_settings(
            &access_token,
            request.friends_enabled,
            request.accept_invites,
        )
        .await
        .map_err(|error| social_error("settings", error))?;

    Ok(Json(FriendSettingsResponse {
        friends_enabled: request.friends_enabled,
        accept_invites: request.accept_invites,
    }))
}

pub async fn settings(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<FriendSettingsResponse>, ApiError> {
    let (_, access_token) = authenticate_official_friend_request(&state, &headers).await?;
    let settings = state
        .minecraft_social
        .friend_settings(&access_token)
        .await
        .map_err(|error| social_error("settings", error))?;

    Ok(Json(FriendSettingsResponse {
        friends_enabled: settings.friends_enabled,
        accept_invites: settings.accept_invites,
    }))
}

pub async fn accept_request(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(profile_id): Path<String>,
) -> Result<Json<FriendMutationResponse>, ApiError> {
    let (caller, access_token) = authenticate_official_friend_request(&state, &headers).await?;
    let requester = parse_profile_id(&profile_id)?;
    enforce_mutation_rate_limit(&state, caller.profile_id).await?;
    let official = state
        .minecraft_social
        .add_friend_by_id(&access_token, requester)
        .await
        .map_err(|error| social_error("accept", error))?;
    if !official
        .friends
        .iter()
        .any(|friend| friend.profile_id == requester)
    {
        return Err(ApiError::bad_gateway(
            "INVALID_OFFICIAL_FRIEND_RESPONSE",
            "Minecraft friends service did not return the accepted friendship",
        ));
    }
    synchronize_official_snapshot(&state, caller.profile_id, &official).await?;

    Ok(Json(FriendMutationResponse {
        result: "SUCCESS",
        relationship: "ACCEPTED",
        official_sync: "SUCCESS",
    }))
}

pub async fn delete_request(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(profile_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let (caller, access_token) = authenticate_official_friend_request(&state, &headers).await?;
    let peer = parse_profile_id(&profile_id)?;
    enforce_mutation_rate_limit(&state, caller.profile_id).await?;
    let official = state
        .minecraft_social
        .remove_friend_by_id(&access_token, peer)
        .await
        .map_err(|error| social_error("delete_request", error))?;
    synchronize_official_snapshot(&state, caller.profile_id, &official).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn remove_friend(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(profile_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let (caller, access_token) = authenticate_official_friend_request(&state, &headers).await?;
    let peer = parse_profile_id(&profile_id)?;
    enforce_mutation_rate_limit(&state, caller.profile_id).await?;
    let official = state
        .minecraft_social
        .remove_friend_by_id(&access_token, peer)
        .await
        .map_err(|error| social_error("remove", error))?;
    synchronize_official_snapshot(&state, caller.profile_id, &official).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AddFriendRequest {
    name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FriendSettingsRequest {
    friends_enabled: bool,
    accept_invites: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FriendSettingsResponse {
    friends_enabled: bool,
    accept_invites: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FriendSnapshotResponse {
    self_presences: Vec<Presence>,
    friends: Vec<FriendResponse>,
    incoming_requests: Vec<FriendRequestResponse>,
    outgoing_requests: Vec<FriendRequestResponse>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FriendResponse {
    profile_id: Uuid,
    name: Option<String>,
    source: FriendSource,
    presences: Vec<Presence>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FriendRequestResponse {
    profile_id: Uuid,
    name: Option<String>,
    source: FriendSource,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FriendMutationResponse {
    result: &'static str,
    relationship: &'static str,
    official_sync: &'static str,
}

fn friend_profile_id(friendship: &Friendship, caller: Uuid) -> Uuid {
    if friendship.profile_low == caller {
        friendship.profile_high
    } else {
        friendship.profile_low
    }
}

fn request_response(
    request: FriendRequest,
    incoming: bool,
    names: &HashMap<Uuid, Option<String>>,
) -> FriendRequestResponse {
    let profile_id = if incoming {
        request.requester_profile_id
    } else {
        request.target_profile_id
    };
    FriendRequestResponse {
        profile_id,
        name: names.get(&profile_id).cloned().flatten(),
        source: request.source,
    }
}

async fn resolve_friend_presences(
    state: &AppState,
    friendships: &[Friendship],
    caller_profile_id: Uuid,
) -> Result<HashMap<Uuid, Vec<Presence>>, ApiError> {
    let mut result = HashMap::with_capacity(friendships.len());
    for friendship in friendships {
        let profile_id = friend_profile_id(friendship, caller_profile_id);
        let presences = sorted_presences_for_profile(state, profile_id).await?;
        result.insert(profile_id, presences);
    }
    Ok(result)
}

async fn sorted_presences_for_profile(
    state: &AppState,
    profile_id: Uuid,
) -> Result<Vec<Presence>, ApiError> {
    let mut presences = state
        .redis
        .presences_for_profile(profile_id)
        .await
        .map_err(redis_error)?;
    presences.sort_unstable_by(|left, right| left.presence_id.cmp(&right.presence_id));
    Ok(presences)
}

async fn resolve_names(
    state: &AppState,
    profile_ids: Vec<Uuid>,
) -> Result<HashMap<Uuid, Option<String>>, ApiError> {
    let mut names = HashMap::with_capacity(profile_ids.len());
    for profile_id in profile_ids {
        if let Some((_, name)) = state
            .redis
            .cached_profile_by_id(profile_id)
            .await
            .map_err(redis_error)?
        {
            names.insert(profile_id, Some(name));
            continue;
        }
        match state.minecraft_profiles.lookup_by_id(profile_id).await {
            Ok(profile) => {
                if let Err(error) = state
                    .redis
                    .cache_profile(
                        profile.profile_id,
                        &profile.name,
                        state.config.profile_cache_ttl,
                    )
                    .await
                {
                    warn!(error = %error, %profile_id, "failed to cache Minecraft profile");
                }
                names.insert(profile_id, Some(profile.name));
            }
            Err(MinecraftProfileError::NotFound) => {
                names.insert(profile_id, None);
            }
            Err(error) => return Err(profile_lookup_error(error)),
        }
    }
    Ok(names)
}

async fn authenticate_official_friend_request(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<(RuntimeInstance, SecretString), ApiError> {
    let (_, caller) = authenticate_instance(state, headers).await?;
    let access_token = minecraft_friend_token(headers).ok_or_else(|| {
        ApiError::unauthorized(
            "MINECRAFT_TOKEN_REQUIRED",
            "X-Minecraft-Access-Token is required for friend operations",
        )
    })?;
    match state.minecraft_auth.verify(&access_token).await {
        Ok(identity) if identity.profile_id == caller.profile_id => Ok((caller, access_token)),
        Ok(_) | Err(MinecraftAuthError::InvalidToken) => Err(ApiError::unauthorized(
            "INVALID_MINECRAFT_TOKEN",
            "Minecraft access token does not belong to the runtime instance",
        )),
        Err(error) => {
            metrics::counter!("nli_upstream_errors_total", "operation" => "minecraft_auth")
                .increment(1);
            warn!(error = %error, profile_id = %caller.profile_id, "friend token verification failed");
            Err(ApiError::service_unavailable(
                "Minecraft authentication service is unavailable",
            ))
        }
    }
}

async fn synchronize_official_snapshot(
    state: &AppState,
    profile_id: Uuid,
    snapshot: &OfficialFriendSnapshot,
) -> Result<(), ApiError> {
    for friend in all_official_entries(snapshot) {
        if let Err(error) = state
            .redis
            .cache_profile(
                friend.profile_id,
                &friend.name,
                state.config.profile_cache_ttl,
            )
            .await
        {
            warn!(error = %error, profile_id = %friend.profile_id, "failed to cache official friend profile");
        }
    }

    FriendRepository::new(state.db.clone())
        .replace_with_official_snapshot(
            profile_id,
            &snapshot
                .friends
                .iter()
                .map(|friend| friend.profile_id)
                .collect::<Vec<_>>(),
            &snapshot
                .incoming_requests
                .iter()
                .map(|friend| friend.profile_id)
                .collect::<Vec<_>>(),
            &snapshot
                .outgoing_requests
                .iter()
                .map(|friend| friend.profile_id)
                .collect::<Vec<_>>(),
        )
        .await
        .map_err(repository_error)?;
    metrics::counter!("nli_official_friend_sync_total", "operation" => "reconcile", "result" => "success").increment(1);
    Ok(())
}

fn all_official_entries(
    snapshot: &OfficialFriendSnapshot,
) -> impl Iterator<Item = &OfficialFriend> {
    snapshot
        .friends
        .iter()
        .chain(&snapshot.incoming_requests)
        .chain(&snapshot.outgoing_requests)
}

fn relationship_for_name(snapshot: &OfficialFriendSnapshot, name: &str) -> Option<&'static str> {
    if snapshot
        .friends
        .iter()
        .any(|friend| friend.name.eq_ignore_ascii_case(name))
    {
        Some("ACCEPTED")
    } else if snapshot
        .outgoing_requests
        .iter()
        .any(|friend| friend.name.eq_ignore_ascii_case(name))
    {
        Some("REQUESTED")
    } else {
        None
    }
}

fn minecraft_friend_token(headers: &HeaderMap) -> Option<SecretString> {
    let value = headers
        .get("x-minecraft-access-token")?
        .to_str()
        .ok()?
        .trim();
    if value.is_empty() || value.chars().any(char::is_whitespace) {
        return None;
    }
    Some(SecretString::from(value.to_owned()))
}

fn validate_player_name(name: &str) -> Result<&str, ApiError> {
    let name = name.trim();
    if !(3..=16).contains(&name.len())
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
    {
        return Err(ApiError::bad_request(
            "INVALID_PLAYER_NAME",
            "Minecraft player name must be 3-16 ASCII letters, digits, or underscores",
        ));
    }
    Ok(name)
}

fn parse_profile_id(value: &str) -> Result<Uuid, ApiError> {
    Uuid::parse_str(value)
        .map_err(|_| ApiError::bad_request("INVALID_PROFILE_ID", "profileId must be a valid UUID"))
}

async fn enforce_mutation_rate_limit(state: &AppState, profile_id: Uuid) -> Result<(), ApiError> {
    let count = state
        .redis
        .increment_rate_limit(
            &format!("friend-mutation:{profile_id}"),
            Duration::from_secs(60),
        )
        .await
        .map_err(redis_error)?;
    if count > FRIEND_MUTATION_LIMIT_PER_MINUTE {
        metrics::counter!("nli_rate_limited_total", "endpoint" => "friend_mutation").increment(1);
        return Err(ApiError::rate_limited(
            "Friend mutation rate limit exceeded",
        ));
    }
    Ok(())
}

fn profile_lookup_error(error: MinecraftProfileError) -> ApiError {
    match error {
        MinecraftProfileError::NotFound => {
            ApiError::not_found("PLAYER_NOT_FOUND", "Minecraft player was not found")
        }
        error => {
            metrics::counter!("nli_upstream_errors_total", "operation" => "minecraft_profile")
                .increment(1);
            warn!(error = %error, "Minecraft profile lookup failed");
            ApiError::service_unavailable("Minecraft profile service is unavailable")
        }
    }
}

fn social_error(operation: &'static str, error: MinecraftSocialError) -> ApiError {
    let result = match error {
        MinecraftSocialError::InvalidToken => ApiError::unauthorized(
            "INVALID_MINECRAFT_TOKEN",
            "Minecraft access token was rejected by the friends service",
        ),
        MinecraftSocialError::UnknownProfile => {
            ApiError::not_found("PLAYER_NOT_FOUND", "Minecraft player was not found")
        }
        MinecraftSocialError::Forbidden => ApiError::new(
            StatusCode::FORBIDDEN,
            "OFFICIAL_FRIENDS_FORBIDDEN",
            "Minecraft friends operation is not permitted for this account",
        ),
        MinecraftSocialError::RateLimited => {
            ApiError::rate_limited("Minecraft friends service rate limit exceeded")
        }
        MinecraftSocialError::InvalidResponse(_) => ApiError::bad_gateway(
            "INVALID_OFFICIAL_FRIEND_RESPONSE",
            "Minecraft friends service returned an invalid response",
        ),
        MinecraftSocialError::Request(_) | MinecraftSocialError::UpstreamStatus(_) => {
            ApiError::service_unavailable("Minecraft friends service is unavailable")
        }
    };
    metrics::counter!("nli_official_friend_sync_total", "operation" => operation, "result" => "failed").increment(1);
    warn!(error = %error, operation, "official friend operation failed");
    result
}

fn repository_error(error: anyhow::Error) -> ApiError {
    error!(error = %error, "friend repository operation failed");
    ApiError::internal("Friend storage operation failed")
}

fn redis_error(error: anyhow::Error) -> ApiError {
    error!(error = %error, "friend rate-limit operation failed");
    ApiError::service_unavailable("Friend service is unavailable")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_minecraft_player_names() {
        assert_eq!(
            validate_player_name("  Player_123  ").unwrap(),
            "Player_123"
        );
        assert!(validate_player_name("ab").is_err());
        assert!(validate_player_name("player-name").is_err());
        assert!(validate_player_name("界界界").is_err());
        assert!(validate_player_name("a1234567890123456").is_err());
    }

    #[test]
    fn selects_the_other_friend_profile() {
        let low = Uuid::from_u128(1);
        let high = Uuid::from_u128(2);
        let now = chrono::Utc::now();
        let friendship = Friendship {
            profile_low: low,
            profile_high: high,
            source: FriendSource::MinecraftSync,
            created_at: now,
            updated_at: now,
        };
        assert_eq!(friend_profile_id(&friendship, low), high);
        assert_eq!(friend_profile_id(&friendship, high), low);
    }
}
