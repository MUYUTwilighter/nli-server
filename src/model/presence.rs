use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PresenceStatus {
    Offline,
    Online,
    InGame,
    Hosting,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Presence {
    pub profile_id: Uuid,
    pub presence_id: String,
    pub pmid: Option<String>,
    pub status: PresenceStatus,
    pub joinable: bool,
    pub session_id: Option<String>,
    pub endpoint: Option<String>,
    pub display_text: String,
    pub updated_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

impl Presence {
    pub fn is_joinable(&self) -> bool {
        self.status == PresenceStatus::Hosting && self.joinable
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_hosting_presence_can_be_joinable() {
        let now = Utc::now();
        let mut presence = Presence {
            profile_id: Uuid::new_v4(),
            presence_id: Uuid::new_v4().to_string(),
            pmid: None,
            status: PresenceStatus::Online,
            joinable: true,
            session_id: None,
            endpoint: None,
            display_text: "Minecraft Java instance".to_owned(),
            updated_at: now,
            expires_at: now,
        };

        assert!(!presence.is_joinable());
        presence.status = PresenceStatus::Hosting;
        assert!(presence.is_joinable());
    }

    #[test]
    fn presence_status_uses_wire_protocol_values() {
        for (status, value) in [
            (PresenceStatus::Offline, "OFFLINE"),
            (PresenceStatus::Online, "ONLINE"),
            (PresenceStatus::InGame, "IN_GAME"),
            (PresenceStatus::Hosting, "HOSTING"),
        ] {
            assert_eq!(
                serde_json::to_string(&status).unwrap(),
                format!("\"{value}\"")
            );
            assert_eq!(
                serde_json::from_str::<PresenceStatus>(&format!("\"{value}\"")).unwrap(),
                status
            );
        }
    }

    #[test]
    fn presence_round_trip_preserves_camel_case_fields() {
        let now = Utc::now();
        let presence = Presence {
            profile_id: Uuid::new_v4(),
            presence_id: "presence-id".to_owned(),
            pmid: Some("pmid".to_owned()),
            status: PresenceStatus::Hosting,
            joinable: true,
            session_id: Some("session-id".to_owned()),
            endpoint: None,
            display_text: "Test world".to_owned(),
            updated_at: now,
            expires_at: now,
        };
        let value = serde_json::to_value(&presence).unwrap();

        assert_eq!(value["profileId"], presence.profile_id.to_string());
        assert_eq!(value["presenceId"], "presence-id");
        assert_eq!(value["displayText"], "Test world");
        assert_eq!(serde_json::from_value::<Presence>(value).unwrap(), presence);
    }
}
