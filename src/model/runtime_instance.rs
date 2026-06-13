use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeInstance {
    pub profile_id: Uuid,
    pub presence_id: String,
    pub instance_started_at: DateTime<Utc>,
    pub issued_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

impl RuntimeInstance {
    pub fn is_expired_at(&self, now: DateTime<Utc>) -> bool {
        self.expires_at <= now
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expiry_is_inclusive() {
        let now = Utc::now();
        let instance = RuntimeInstance {
            profile_id: Uuid::new_v4(),
            presence_id: "presence".to_owned(),
            instance_started_at: now,
            issued_at: now,
            expires_at: now + chrono::Duration::seconds(30),
        };

        assert!(!instance.is_expired_at(now));
        assert!(instance.is_expired_at(instance.expires_at));
        assert!(instance.is_expired_at(instance.expires_at + chrono::Duration::milliseconds(1)));
    }

    #[test]
    fn runtime_instance_round_trips_through_json() {
        let now = Utc::now();
        let instance = RuntimeInstance {
            profile_id: Uuid::new_v4(),
            presence_id: "presence".to_owned(),
            instance_started_at: now,
            issued_at: now,
            expires_at: now,
        };
        let value = serde_json::to_value(&instance).unwrap();

        assert!(value.get("profileId").is_some());
        assert!(value.get("instanceStartedAt").is_some());
        assert_eq!(
            serde_json::from_value::<RuntimeInstance>(value).unwrap(),
            instance
        );
    }
}
