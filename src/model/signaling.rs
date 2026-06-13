use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

pub const MAX_ICE_CANDIDATES_PER_SIDE: u16 = 128;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SignalingPhase {
    PendingJoin,
    Accepted,
    OfferSent,
    AnswerSent,
}

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
    #[serde(default)]
    pub join_accepted: bool,
    pub offer_sent: bool,
    pub answer_sent: bool,
    pub initiator_ice_candidates: u16,
    pub target_ice_candidates: u16,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

impl SignalingSession {
    pub fn phase(&self) -> SignalingPhase {
        if self.answer_sent {
            SignalingPhase::AnswerSent
        } else if self.offer_sent {
            SignalingPhase::OfferSent
        } else if self.join_accepted {
            SignalingPhase::Accepted
        } else {
            SignalingPhase::PendingJoin
        }
    }

    pub fn register_join_accepted(&mut self) -> Result<(), SignalingLimitError> {
        if self.phase() != SignalingPhase::PendingJoin {
            return Err(SignalingLimitError::JoinAlreadyAccepted);
        }
        self.join_accepted = true;
        Ok(())
    }

    pub fn register_offer(&mut self) -> Result<(), SignalingLimitError> {
        match self.phase() {
            SignalingPhase::PendingJoin => {
                return Err(SignalingLimitError::JoinAcceptanceRequired);
            }
            SignalingPhase::Accepted => {}
            SignalingPhase::OfferSent | SignalingPhase::AnswerSent => {
                return Err(SignalingLimitError::OfferAlreadySent);
            }
        }
        self.offer_sent = true;
        Ok(())
    }

    pub fn register_answer(&mut self) -> Result<(), SignalingLimitError> {
        match self.phase() {
            SignalingPhase::PendingJoin | SignalingPhase::Accepted => {
                return Err(SignalingLimitError::OfferRequired);
            }
            SignalingPhase::OfferSent => {}
            SignalingPhase::AnswerSent => {
                return Err(SignalingLimitError::AnswerAlreadySent);
            }
        }
        self.answer_sent = true;
        Ok(())
    }

    pub fn register_join_rejected(&self) -> Result<(), SignalingLimitError> {
        if self.phase() != SignalingPhase::PendingJoin {
            return Err(SignalingLimitError::JoinDecisionAlreadyMade);
        }
        Ok(())
    }

    pub fn register_invite_declined(&self) -> Result<(), SignalingLimitError> {
        if matches!(
            self.phase(),
            SignalingPhase::OfferSent | SignalingPhase::AnswerSent
        ) {
            return Err(SignalingLimitError::NegotiationAlreadyStarted);
        }
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

        if !self.offer_sent {
            return Err(SignalingLimitError::OfferRequired);
        }
        if *count >= MAX_ICE_CANDIDATES_PER_SIDE {
            return Err(SignalingLimitError::TooManyIceCandidates);
        }
        *count += 1;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum SignalingLimitError {
    #[error("the join request has already been accepted")]
    JoinAlreadyAccepted,
    #[error("the join request has already been decided")]
    JoinDecisionAlreadyMade,
    #[error("WebRTC negotiation has already started")]
    NegotiationAlreadyStarted,
    #[error("the join request must be accepted before an offer")]
    JoinAcceptanceRequired,
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
            join_accepted: false,
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
            session.register_offer(),
            Err(SignalingLimitError::JoinAcceptanceRequired)
        );
        assert_eq!(
            session.register_answer(),
            Err(SignalingLimitError::OfferRequired)
        );
        assert!(session.register_join_accepted().is_ok());
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

    #[test]
    fn enforces_one_offer() {
        let mut session = session();
        session.register_join_accepted().unwrap();
        assert!(session.register_offer().is_ok());
        assert_eq!(
            session.register_offer(),
            Err(SignalingLimitError::OfferAlreadySent)
        );
    }

    #[test]
    fn counts_ice_candidates_per_peer() {
        let mut session = session();
        session.register_join_accepted().unwrap();
        session.register_offer().unwrap();
        for _ in 0..MAX_ICE_CANDIDATES_PER_SIDE {
            session.register_ice_candidate("initiator").unwrap();
        }
        assert_eq!(
            session.initiator_ice_candidates,
            MAX_ICE_CANDIDATES_PER_SIDE
        );
        assert_eq!(session.target_ice_candidates, 0);
        assert_eq!(
            session.register_ice_candidate("initiator"),
            Err(SignalingLimitError::TooManyIceCandidates)
        );
        assert!(session.register_ice_candidate("target").is_ok());
        assert_eq!(session.target_ice_candidates, 1);
    }

    #[test]
    fn signaling_session_round_trips_through_json() {
        let session = session();
        let value = serde_json::to_value(&session).unwrap();

        assert!(value.get("sessionId").is_some());
        assert!(value.get("offerSent").is_some());
        assert_eq!(
            serde_json::from_value::<SignalingSession>(value).unwrap(),
            session
        );
    }

    #[test]
    fn enforces_terminal_join_decisions() {
        let mut session = session();
        assert!(session.register_join_rejected().is_ok());
        assert!(session.register_invite_declined().is_ok());
        session.register_join_accepted().unwrap();
        assert_eq!(
            session.register_join_rejected(),
            Err(SignalingLimitError::JoinDecisionAlreadyMade)
        );
        assert!(session.register_invite_declined().is_ok());
        session.register_offer().unwrap();
        assert_eq!(
            session.register_invite_declined(),
            Err(SignalingLimitError::NegotiationAlreadyStarted)
        );
    }
}
