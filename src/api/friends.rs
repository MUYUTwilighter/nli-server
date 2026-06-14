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
    auth::{MinecraftProfileError, ProfileIdentity},
    db::friends::{FriendRepository, RequestOutcome},
    model::friend::{FriendRequest, FriendSource, Friendship},
    model::presence::Presence,
    state::AppState,
};

use super::{ApiError, instances::authenticate_instance};

const FRIEND_MUTATION_LIMIT_PER_MINUTE: u64 = 10;

pub async fn snapshot(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<FriendSnapshotResponse>, ApiError> {
    let (_, caller) = authenticate_instance(&state, &headers).await?;
    let snapshot = FriendRepository::new(state.db.clone())
        .snapshot(caller.profile_id)
        .await
        .map_err(repository_error)?;

    let mut profile_ids = Vec::new();
    for friendship in &snapshot.friends {
        profile_ids.push(friend_profile_id(friendship, caller.profile_id));
    }
    for request in &snapshot.incoming_requests {
        profile_ids.push(request.requester_profile_id);
    }
    for request in &snapshot.outgoing_requests {
        profile_ids.push(request.target_profile_id);
    }
    profile_ids.sort_unstable();
    profile_ids.dedup();
    let names = resolve_names(&state, profile_ids).await?;
    let mut presences =
        resolve_friend_presences(&state, &snapshot.friends, caller.profile_id).await?;

    Ok(Json(FriendSnapshotResponse {
        friends: snapshot
            .friends
            .into_iter()
            .map(|friendship| {
                let profile_id = friend_profile_id(&friendship, caller.profile_id);
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
    let (_, caller) = authenticate_instance(&state, &headers).await?;
    let name = validate_player_name(&request.name)?;
    let target = resolve_profile_by_name(&state, name).await?;
    if caller.profile_id == target.profile_id {
        return Err(ApiError::bad_request(
            "SELF_FRIEND_REQUEST",
            "Cannot send a friend request to yourself",
        ));
    }
    enforce_mutation_rate_limit(&state, caller.profile_id).await?;

    let repository = FriendRepository::new(state.db.clone());
    if repository
        .are_friends(caller.profile_id, target.profile_id)
        .await
        .map_err(repository_error)?
    {
        return Err(ApiError::conflict(
            "ALREADY_FRIENDS",
            "Players are already friends",
        ));
    }
    let outcome = repository
        .request_or_accept(
            caller.profile_id,
            target.profile_id,
            FriendSource::Netherlink,
        )
        .await
        .map_err(repository_error)?;

    Ok(Json(FriendMutationResponse {
        result: "SUCCESS",
        relationship: match outcome {
            RequestOutcome::Requested => "REQUESTED",
            RequestOutcome::Accepted => "ACCEPTED",
        },
        official_sync: "SKIPPED",
    }))
}

pub async fn accept_request(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(profile_id): Path<String>,
) -> Result<Json<FriendMutationResponse>, ApiError> {
    let (_, caller) = authenticate_instance(&state, &headers).await?;
    let requester = parse_profile_id(&profile_id)?;
    enforce_mutation_rate_limit(&state, caller.profile_id).await?;
    let accepted = FriendRepository::new(state.db.clone())
        .accept(caller.profile_id, requester)
        .await
        .map_err(repository_error)?;
    if !accepted {
        return Err(ApiError::not_found(
            "FRIEND_REQUEST_NOT_FOUND",
            "Incoming friend request was not found",
        ));
    }

    Ok(Json(FriendMutationResponse {
        result: "SUCCESS",
        relationship: "ACCEPTED",
        official_sync: "SKIPPED",
    }))
}

pub async fn delete_request(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(profile_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let (_, caller) = authenticate_instance(&state, &headers).await?;
    let peer = parse_profile_id(&profile_id)?;
    enforce_mutation_rate_limit(&state, caller.profile_id).await?;
    FriendRepository::new(state.db.clone())
        .delete_request(caller.profile_id, peer)
        .await
        .map_err(repository_error)?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn remove_friend(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(profile_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let (_, caller) = authenticate_instance(&state, &headers).await?;
    let peer = parse_profile_id(&profile_id)?;
    enforce_mutation_rate_limit(&state, caller.profile_id).await?;
    FriendRepository::new(state.db.clone())
        .remove_friend(caller.profile_id, peer)
        .await
        .map_err(repository_error)?;
    bridge_official_removal(&state, &headers, caller.profile_id, peer).await;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AddFriendRequest {
    name: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FriendSnapshotResponse {
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
        let mut presences = state
            .redis
            .presences_for_profile(profile_id)
            .await
            .map_err(redis_error)?;
        presences.sort_unstable_by(|left, right| left.presence_id.cmp(&right.presence_id));
        result.insert(profile_id, presences);
    }
    Ok(result)
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

async fn resolve_profile_by_name(
    state: &AppState,
    name: &str,
) -> Result<ProfileIdentity, ApiError> {
    if let Some((profile_id, name)) = state
        .redis
        .cached_profile_by_name(name)
        .await
        .map_err(redis_error)?
    {
        return Ok(ProfileIdentity { profile_id, name });
    }
    let profile = state
        .minecraft_profiles
        .lookup_by_name(name)
        .await
        .map_err(profile_lookup_error)?;
    state
        .redis
        .cache_profile(
            profile.profile_id,
            &profile.name,
            state.config.profile_cache_ttl,
        )
        .await
        .map_err(redis_error)?;
    Ok(profile)
}

async fn bridge_official_removal(
    state: &AppState,
    headers: &HeaderMap,
    caller_profile_id: Uuid,
    peer: Uuid,
) {
    let Some(access_token) = official_bridge_token(headers) else {
        metrics::counter!("nli_official_friend_sync_total", "operation" => "remove", "result" => "skipped").increment(1);
        return;
    };
    let verified = match state.minecraft_auth.verify(&access_token).await {
        Ok(identity) if identity.profile_id == caller_profile_id => true,
        Ok(_) => false,
        Err(error) => {
            warn!(error = %error, profile_id = %caller_profile_id, "official removal token verification failed");
            false
        }
    };
    if !verified {
        metrics::counter!("nli_official_friend_sync_total", "operation" => "remove", "result" => "invalid_token").increment(1);
        return;
    }
    match state
        .minecraft_social
        .remove_friend(&access_token, peer)
        .await
    {
        Ok(()) => {
            metrics::counter!("nli_official_friend_sync_total", "operation" => "remove", "result" => "success").increment(1);
        }
        Err(error) => {
            metrics::counter!("nli_official_friend_sync_total", "operation" => "remove", "result" => "upstream_error").increment(1);
            warn!(error = %error, profile_id = %caller_profile_id, target_profile_id = %peer, "official friend removal failed");
        }
    }
}

fn official_bridge_token(headers: &HeaderMap) -> Option<SecretString> {
    let value = headers
        .get("x-minecraft-access-token")?
        .to_str()
        .ok()?
        .trim();
    if value.is_empty() || value.chars().any(char::is_control) {
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
            source: FriendSource::Netherlink,
            created_at: now,
            updated_at: now,
        };
        assert_eq!(friend_profile_id(&friendship, low), high);
        assert_eq!(friend_profile_id(&friendship, high), low);
    }
}
