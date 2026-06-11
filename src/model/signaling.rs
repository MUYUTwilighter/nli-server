use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

pub const MAX_ICE_CANDIDATES_PER_SIDE: u16 = 128;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignalingPeer {
    pub profile_id: Uuid,
    pub presence_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignalingSession {
    pub session_id: String,
    pub initiator: SignalingPeer,
    pub target: SignalingPeer,
    pub offer_sent: bool,
    pub answer_sent: bool,
    pub initiator_ice_candidates: u16,
    pub target_ice_candidates: u16,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

impl SignalingSession {
    pub fn register_offer(&mut self) -> Result<(), SignalingLimitError> {
        if self.offer_sent {
            return Err(SignalingLimitError::OfferAlreadySent);
        }
        self.offer_sent = true;
        Ok(())
    }

    pub fn register_answer(&mut self) -> Result<(), SignalingLimitError> {
        if !self.offer_sent {
            return Err(SignalingLimitError::OfferRequired);
        }
        if self.answer_sent {
            return Err(SignalingLimitError::AnswerAlreadySent);
        }
        self.answer_sent = true;
        Ok(())
    }

    pub fn register_ice_candidate(
        &mut self,
        sender_presence_id: &str,
    ) -> Result<(), SignalingLimitError> {
        let count = if sender_presence_id == self.initiator.presence_id {
            &mut self.initiator_ice_candidates
        } else if sender_presence_id == self.target.presence_id {
            &mut self.target_ice_candidates
        } else {
            return Err(SignalingLimitError::UnknownPeer);
        };

        if *count >= MAX_ICE_CANDIDATES_PER_SIDE {
            return Err(SignalingLimitError::TooManyIceCandidates);
        }
        *count += 1;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum SignalingLimitError {
    #[error("the signaling offer has already been sent")]
    OfferAlreadySent,
    #[error("an offer is required before an answer")]
    OfferRequired,
    #[error("the signaling answer has already been sent")]
    AnswerAlreadySent,
    #[error("the sender does not belong to this signaling session")]
    UnknownPeer,
    #[error("the ICE candidate limit has been reached")]
    TooManyIceCandidates,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session() -> SignalingSession {
        let now = Utc::now();
        SignalingSession {
            session_id: "session".to_owned(),
            initiator: SignalingPeer {
                profile_id: Uuid::new_v4(),
                presence_id: "initiator".to_owned(),
            },
            target: SignalingPeer {
                profile_id: Uuid::new_v4(),
                presence_id: "target".to_owned(),
            },
            offer_sent: false,
            answer_sent: false,
            initiator_ice_candidates: 0,
            target_ice_candidates: 0,
            created_at: now,
            expires_at: now,
        }
    }

    #[test]
    fn enforces_offer_and_answer_order() {
        let mut session = session();
        assert_eq!(
            session.register_answer(),
            Err(SignalingLimitError::OfferRequired)
        );
        assert!(session.register_offer().is_ok());
        assert!(session.register_answer().is_ok());
        assert_eq!(
            session.register_answer(),
            Err(SignalingLimitError::AnswerAlreadySent)
        );
    }

    #[test]
    fn rejects_unknown_ice_candidate_sender() {
        assert_eq!(
            session().register_ice_candidate("unknown"),
            Err(SignalingLimitError::UnknownPeer)
        );
    }
}
