use std::{fmt, str::FromStr};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FriendSource {
    Netherlink,
    MinecraftImport,
    MinecraftSync,
}

impl FriendSource {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Netherlink => "netherlink",
            Self::MinecraftImport => "minecraft_import",
            Self::MinecraftSync => "minecraft_sync",
        }
    }
}

impl fmt::Display for FriendSource {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for FriendSource {
    type Err = UnknownFriendSource;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "netherlink" => Ok(Self::Netherlink),
            "minecraft_import" => Ok(Self::MinecraftImport),
            "minecraft_sync" => Ok(Self::MinecraftSync),
            _ => Err(UnknownFriendSource(value.to_owned())),
        }
    }
}

#[derive(Debug, Error)]
#[error("unknown friend source: {0}")]
pub struct UnknownFriendSource(String);

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Friendship {
    pub profile_low: Uuid,
    pub profile_high: Uuid,
    pub source: FriendSource,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FriendRequest {
    pub requester_profile_id: Uuid,
    pub target_profile_id: Uuid,
    pub source: FriendSource,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FriendSnapshot {
    pub friends: Vec<Friendship>,
    pub incoming_requests: Vec<FriendRequest>,
    pub outgoing_requests: Vec<FriendRequest>,
}

pub fn normalize_friend_pair(first: Uuid, second: Uuid) -> Option<(Uuid, Uuid)> {
    match first.as_bytes().cmp(second.as_bytes()) {
        std::cmp::Ordering::Less => Some((first, second)),
        std::cmp::Ordering::Greater => Some((second, first)),
        std::cmp::Ordering::Equal => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_friend_pair() {
        let low = Uuid::from_u128(1);
        let high = Uuid::from_u128(2);

        assert_eq!(normalize_friend_pair(high, low), Some((low, high)));
        assert_eq!(normalize_friend_pair(low, high), Some((low, high)));
    }

    #[test]
    fn rejects_self_friendship() {
        let profile_id = Uuid::new_v4();
        assert_eq!(normalize_friend_pair(profile_id, profile_id), None);
    }

    #[test]
    fn friend_source_uses_stable_storage_and_json_values() {
        for (source, value) in [
            (FriendSource::Netherlink, "netherlink"),
            (FriendSource::MinecraftImport, "minecraft_import"),
            (FriendSource::MinecraftSync, "minecraft_sync"),
        ] {
            assert_eq!(source.as_str(), value);
            assert_eq!(source.to_string(), value);
            assert_eq!(FriendSource::from_str(value).unwrap(), source);
            assert_eq!(
                serde_json::to_string(&source).unwrap(),
                format!("\"{value}\"")
            );
            assert_eq!(
                serde_json::from_str::<FriendSource>(&format!("\"{value}\"")).unwrap(),
                source
            );
        }
        assert!(FriendSource::from_str("unknown").is_err());
    }

    #[test]
    fn friend_snapshot_serializes_api_field_names() {
        let snapshot = FriendSnapshot::default();
        let value = serde_json::to_value(snapshot).unwrap();

        assert!(value.get("friends").is_some());
        assert!(value.get("incomingRequests").is_some());
        assert!(value.get("outgoingRequests").is_some());
        assert!(value.get("incoming_requests").is_none());
    }
}
