use std::time::Duration;

use axum::{
    extract::{
        State, WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    http::HeaderMap,
    response::Response,
};
use chrono::Utc;
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::time::{Instant, MissedTickBehavior, interval};
use tracing::{debug, warn};
use uuid::Uuid;

use crate::{
    db::friends::FriendRepository,
    model::{
        runtime_instance::RuntimeInstance,
        signaling::{SignalingLimitError, SignalingPeer, SignalingSession},
        token::RuntimeTokenHash,
    },
    signaling::RegisterError,
    state::AppState,
};

use super::{ApiError, instances::authenticate_instance};

const MAX_WS_MESSAGE_BYTES: usize = 256 * 1024;
const MAX_ID_CHARS: usize = 128;
const MAX_PRESENCE_ID_CHARS: usize = 128;
const MAX_SESSION_ID_CHARS: usize = 128;
const MAX_SDP_PAYLOAD_BYTES: usize = 128 * 1024;
const MAX_ICE_PAYLOAD_BYTES: usize = 8 * 1024;
const MAX_CONTROL_PAYLOAD_BYTES: usize = 8 * 1024;
const SIGNALING_MESSAGES_PER_MINUTE: u64 = 60;
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(30);
const IDLE_TIMEOUT: Duration = Duration::from_secs(90);

pub async fn connect(
    State(state): State<AppState>,
    headers: HeaderMap,
    websocket: WebSocketUpgrade,
) -> Result<Response, ApiError> {
    let (token_hash, instance) = authenticate_instance(&state, &headers).await?;
    Ok(websocket
        .max_message_size(MAX_WS_MESSAGE_BYTES)
        .on_upgrade(move |socket| serve_socket(state, token_hash, instance, socket)))
}

async fn serve_socket(
    state: AppState,
    token_hash: RuntimeTokenHash,
    instance: RuntimeInstance,
    mut socket: WebSocket,
) {
    let registered = match state
        .signaling_connections
        .register(instance.profile_id, instance.presence_id.clone())
        .await
    {
        Ok(registered) => registered,
        Err(RegisterError::ProfileConnectionLimit) => {
            let _ = socket
                .send(error_message(
                    None,
                    SignalError::new(
                        "CONNECTION_LIMIT",
                        "Profile signaling connection limit exceeded",
                    ),
                ))
                .await;
            let _ = socket.send(Message::Close(None)).await;
            return;
        }
    };
    let connection_id = registered.id;
    metrics::gauge!("nli_websocket_connections").increment(1.0);
    let _connection_metric = WebSocketMetricGuard;
    let mut outgoing = registered.receiver;
    let (mut sender, mut receiver) = socket.split();
    let mut heartbeat = interval(HEARTBEAT_INTERVAL);
    heartbeat.set_missed_tick_behavior(MissedTickBehavior::Skip);
    heartbeat.tick().await;
    let mut last_activity = Instant::now();

    loop {
        tokio::select! {
            incoming = receiver.next() => {
                match incoming {
                    Some(Ok(Message::Text(text))) => {
                        last_activity = Instant::now();
                        if !instance_token_is_current(&state, &token_hash, &instance).await {
                            let _ = sender
                                .send(error_message(
                                    None,
                                    SignalError::new(
                                        "INVALID_INSTANCE_TOKEN",
                                        "Runtime instance token is invalid or expired",
                                    ),
                                ))
                                .await;
                            let _ = sender.send(Message::Close(None)).await;
                            break;
                        }
                        if let Some(response) = handle_text(&state, &instance, text.as_str()).await
                            && sender.send(response).await.is_err()
                        {
                            break;
                        }
                    }
                    Some(Ok(Message::Ping(payload))) => {
                        last_activity = Instant::now();
                        if sender.send(Message::Pong(payload)).await.is_err() {
                            break;
                        }
                    }
                    Some(Ok(Message::Close(_))) | None | Some(Err(_)) => break,
                    Some(Ok(Message::Binary(_))) => {
                        last_activity = Instant::now();
                        let response = error_message(None, SignalError::bad_request("Binary messages are not supported"));
                        if sender.send(response).await.is_err() {
                            break;
                        }
                    }
                    Some(Ok(Message::Pong(_))) => {
                        last_activity = Instant::now();
                    }
                }
            }
            message = outgoing.recv() => {
                let Some(message) = message else {
                    break;
                };
                let should_close = matches!(message, Message::Close(_));
                if sender.send(message).await.is_err() || should_close {
                    break;
                }
            }
            _ = heartbeat.tick() => {
                if last_activity.elapsed() >= IDLE_TIMEOUT {
                    let _ = sender.send(Message::Close(None)).await;
                    break;
                }
                if !instance_token_is_current(&state, &token_hash, &instance).await {
                    let _ = sender
                        .send(error_message(
                            None,
                            SignalError::new(
                                "INVALID_INSTANCE_TOKEN",
                                "Runtime instance token is invalid or expired",
                            ),
                        ))
                        .await;
                    let _ = sender.send(Message::Close(None)).await;
                    break;
                }
                if sender.send(Message::Ping(Vec::new().into())).await.is_err() {
                    break;
                }
            }
        }
    }

    state
        .signaling_connections
        .unregister(&instance.presence_id, connection_id)
        .await;
    if let Err(error) = state
        .redis
        .delete_signaling_sessions_for_presence(&instance.presence_id)
        .await
    {
        warn!(error = %error, presence_id = %instance.presence_id, "failed to clean signaling sessions after disconnect");
    }
    debug!(
        profile_id = %instance.profile_id,
        presence_id = %instance.presence_id,
        "signaling WebSocket disconnected"
    );
}

struct WebSocketMetricGuard;

impl Drop for WebSocketMetricGuard {
    fn drop(&mut self) {
        metrics::gauge!("nli_websocket_connections").decrement(1.0);
    }
}

async fn instance_token_is_current(
    state: &AppState,
    token_hash: &RuntimeTokenHash,
    expected: &RuntimeInstance,
) -> bool {
    matches!(
        state.redis.runtime_instance(token_hash).await,
        Ok(Some(instance))
            if instance.profile_id == expected.profile_id
                && instance.presence_id == expected.presence_id
                && !instance.is_expired_at(Utc::now())
    )
}

async fn handle_text(state: &AppState, sender: &RuntimeInstance, text: &str) -> Option<Message> {
    let envelope = match serde_json::from_str::<ClientEnvelope>(text) {
        Ok(envelope) => envelope,
        Err(_) => {
            return Some(error_message(
                None,
                SignalError::bad_request("Invalid signaling envelope"),
            ));
        }
    };
    let id = Some(envelope.id.clone());
    match process_message(state, sender, envelope).await {
        Ok(forward) => {
            let message = Message::Text(
                serde_json::to_string(&forward.envelope)
                    .expect("serializable signaling envelope")
                    .into(),
            );
            if state
                .signaling_connections
                .send(&forward.target_presence_id, message)
                .await
            {
                None
            } else {
                let _ = state
                    .redis
                    .delete_signaling_session(&forward.envelope.session_id)
                    .await;
                Some(error_message(id, SignalError::target_unavailable()))
            }
        }
        Err(error) => Some(error_message(id, error)),
    }
}

async fn process_message(
    state: &AppState,
    sender: &RuntimeInstance,
    envelope: ClientEnvelope,
) -> Result<ForwardMessage, SignalError> {
    validate_envelope(&envelope)?;
    enforce_rate_limit(state, sender.profile_id, envelope.to).await?;
    if !FriendRepository::new(state.db.clone())
        .are_friends(sender.profile_id, envelope.to)
        .await
        .map_err(SignalError::storage)?
    {
        return Err(SignalError::new(
            "NOT_FRIENDS",
            "Target profile is not a friend",
        ));
    }

    if envelope.kind == SignalType::JoinRequest {
        return begin_session(state, sender, envelope).await;
    }

    continue_session(state, sender, envelope).await
}

async fn begin_session(
    state: &AppState,
    sender: &RuntimeInstance,
    envelope: ClientEnvelope,
) -> Result<ForwardMessage, SignalError> {
    if state
        .redis
        .signaling_session(&envelope.session_id)
        .await
        .map_err(SignalError::storage)?
        .is_some()
    {
        return Err(SignalError::new(
            "BAD_REQUEST",
            "Signaling session already exists",
        ));
    }
    let presence = state
        .redis
        .presence(&envelope.to_presence_id)
        .await
        .map_err(SignalError::storage)?
        .ok_or_else(|| SignalError::new("INSTANCE_NOT_FOUND", "Target Presence was not found"))?;
    if presence.profile_id != envelope.to {
        return Err(SignalError::new(
            "INSTANCE_NOT_FOUND",
            "Target Presence does not belong to target profile",
        ));
    }
    if !presence.is_joinable() {
        return Err(SignalError::new(
            "TARGET_NOT_JOINABLE",
            "Target Presence is not joinable",
        ));
    }

    let now = Utc::now();
    let expires_at = now
        + chrono::Duration::from_std(state.config.signaling_session_ttl)
            .map_err(|_| SignalError::storage(anyhow::anyhow!("invalid signaling session TTL")))?;
    let session = SignalingSession {
        session_id: envelope.session_id.clone(),
        initiator: SignalingPeer {
            profile_id: sender.profile_id,
            presence_id: sender.presence_id.clone(),
        },
        target: SignalingPeer {
            profile_id: envelope.to,
            presence_id: envelope.to_presence_id.clone(),
        },
        join_accepted: false,
        offer_sent: false,
        answer_sent: false,
        initiator_ice_candidates: 0,
        target_ice_candidates: 0,
        created_at: now,
        expires_at,
    };
    state
        .redis
        .put_signaling_session(&session, state.config.signaling_session_ttl)
        .await
        .map_err(SignalError::storage)?;

    Ok(forward_message(sender, envelope))
}

async fn continue_session(
    state: &AppState,
    sender: &RuntimeInstance,
    envelope: ClientEnvelope,
) -> Result<ForwardMessage, SignalError> {
    let mut session = state
        .redis
        .signaling_session(&envelope.session_id)
        .await
        .map_err(SignalError::storage)?
        .ok_or_else(|| SignalError::new("SESSION_NOT_FOUND", "Signaling session was not found"))?;
    if session.expires_at <= Utc::now() {
        let _ = state
            .redis
            .delete_signaling_session(&session.session_id)
            .await;
        return Err(SignalError::new(
            "SESSION_NOT_FOUND",
            "Signaling session has expired",
        ));
    }

    let sender_is_initiator = peer_matches(&session.initiator, sender);
    let sender_is_target = peer_matches(&session.target, sender);
    if !sender_is_initiator && !sender_is_target {
        return Err(SignalError::new(
            "FORBIDDEN",
            "Sender does not belong to signaling session",
        ));
    }
    let recipient = if sender_is_initiator {
        &session.target
    } else {
        &session.initiator
    };
    if recipient.profile_id != envelope.to || recipient.presence_id != envelope.to_presence_id {
        return Err(SignalError::new(
            "FORBIDDEN",
            "Message target does not match signaling session",
        ));
    }

    match envelope.kind {
        SignalType::JoinAccepted | SignalType::JoinRejected if !sender_is_target => {
            return Err(SignalError::new(
                "FORBIDDEN",
                "Only the host may answer a join request",
            ));
        }
        SignalType::InviteDeclined if !sender_is_initiator => {
            return Err(SignalError::new(
                "FORBIDDEN",
                "Only the invited peer may decline",
            ));
        }
        SignalType::Offer if !sender_is_initiator => {
            return Err(SignalError::new(
                "FORBIDDEN",
                "Only the join initiator may send an offer",
            ));
        }
        SignalType::Answer if !sender_is_target => {
            return Err(SignalError::new(
                "FORBIDDEN",
                "Only the host may send an answer",
            ));
        }
        _ => {}
    }

    if matches!(envelope.kind, SignalType::JoinAccepted | SignalType::Offer)
        && let Err(error) = ensure_host_joinable(state, &session.target).await
    {
        let _ = state
            .redis
            .delete_signaling_session(&session.session_id)
            .await;
        return Err(error);
    }

    match envelope.kind {
        SignalType::JoinAccepted => session.register_join_accepted().map_err(limit_error)?,
        SignalType::Offer => session.register_offer().map_err(limit_error)?,
        SignalType::Answer => session.register_answer().map_err(limit_error)?,
        SignalType::IceCandidate => session
            .register_ice_candidate(&sender.presence_id)
            .map_err(limit_error)?,
        SignalType::JoinRejected => {
            session.register_join_rejected().map_err(limit_error)?;
            state
                .redis
                .delete_signaling_session(&session.session_id)
                .await
                .map_err(SignalError::storage)?;
            return Ok(forward_message(sender, envelope));
        }
        SignalType::InviteDeclined => {
            session.register_invite_declined().map_err(limit_error)?;
            state
                .redis
                .delete_signaling_session(&session.session_id)
                .await
                .map_err(SignalError::storage)?;
            return Ok(forward_message(sender, envelope));
        }
        SignalType::JoinRequest => {}
    }
    state
        .redis
        .put_signaling_session(&session, remaining_ttl(&session)?)
        .await
        .map_err(SignalError::storage)?;
    Ok(forward_message(sender, envelope))
}

async fn ensure_host_joinable(state: &AppState, host: &SignalingPeer) -> Result<(), SignalError> {
    let presence = state
        .redis
        .presence(&host.presence_id)
        .await
        .map_err(SignalError::storage)?
        .ok_or_else(|| SignalError::new("INSTANCE_NOT_FOUND", "Host Presence was not found"))?;
    if presence.profile_id != host.profile_id {
        return Err(SignalError::new(
            "INSTANCE_NOT_FOUND",
            "Host Presence does not belong to the signaling host",
        ));
    }
    if !presence.is_joinable() {
        return Err(SignalError::new(
            "TARGET_NOT_JOINABLE",
            "Host Presence is no longer joinable",
        ));
    }
    Ok(())
}

fn forward_message(sender: &RuntimeInstance, envelope: ClientEnvelope) -> ForwardMessage {
    ForwardMessage {
        target_presence_id: envelope.to_presence_id.clone(),
        envelope: ServerEnvelope {
            id: Uuid::new_v4().to_string(),
            kind: envelope.kind,
            from: sender.profile_id,
            from_presence_id: sender.presence_id.clone(),
            to_presence_id: envelope.to_presence_id,
            session_id: envelope.session_id,
            payload: envelope.payload,
        },
    }
}

fn validate_envelope(envelope: &ClientEnvelope) -> Result<(), SignalError> {
    validate_text(&envelope.id, MAX_ID_CHARS, "id")?;
    validate_text(
        &envelope.to_presence_id,
        MAX_PRESENCE_ID_CHARS,
        "toPresenceId",
    )?;
    validate_text(&envelope.session_id, MAX_SESSION_ID_CHARS, "sessionId")?;
    let payload_size = serde_json::to_vec(&envelope.payload)
        .map_err(|_| SignalError::bad_request("payload is invalid"))?
        .len();
    let limit = match envelope.kind {
        SignalType::Offer | SignalType::Answer => MAX_SDP_PAYLOAD_BYTES,
        SignalType::IceCandidate => MAX_ICE_PAYLOAD_BYTES,
        _ => MAX_CONTROL_PAYLOAD_BYTES,
    };
    if payload_size > limit {
        return Err(SignalError::bad_request(
            "payload exceeds the message type limit",
        ));
    }
    Ok(())
}

fn validate_text(value: &str, max_chars: usize, field: &str) -> Result<(), SignalError> {
    if value.is_empty() || value.chars().count() > max_chars || value.chars().any(char::is_control)
    {
        return Err(SignalError::bad_request(format!("{field} is invalid")));
    }
    Ok(())
}

async fn enforce_rate_limit(
    state: &AppState,
    sender: Uuid,
    target: Uuid,
) -> Result<(), SignalError> {
    let count = state
        .redis
        .increment_rate_limit(
            &format!("signaling:{sender}:{target}"),
            Duration::from_secs(60),
        )
        .await
        .map_err(SignalError::storage)?;
    if count > SIGNALING_MESSAGES_PER_MINUTE {
        return Err(SignalError::new(
            "RATE_LIMITED",
            "Signaling message rate limit exceeded",
        ));
    }
    Ok(())
}

fn peer_matches(peer: &SignalingPeer, instance: &RuntimeInstance) -> bool {
    peer.profile_id == instance.profile_id && peer.presence_id == instance.presence_id
}

fn remaining_ttl(session: &SignalingSession) -> Result<Duration, SignalError> {
    (session.expires_at - Utc::now())
        .to_std()
        .map_err(|_| SignalError::new("SESSION_NOT_FOUND", "Signaling session has expired"))
}

fn limit_error(error: SignalingLimitError) -> SignalError {
    SignalError::new("INVALID_SESSION_STATE", error.to_string())
}

fn error_message(id: Option<String>, error: SignalError) -> Message {
    if error.code == "SERVICE_UNAVAILABLE" {
        warn!("signaling dependency operation failed");
    }
    Message::Text(
        serde_json::to_string(&ErrorEnvelope {
            id,
            kind: "ERROR",
            code: error.code,
            message: error.message,
        })
        .expect("serializable signaling error")
        .into(),
    )
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ClientEnvelope {
    id: String,
    #[serde(rename = "type")]
    kind: SignalType,
    to: Uuid,
    to_presence_id: String,
    session_id: String,
    #[serde(default)]
    payload: Value,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum SignalType {
    JoinRequest,
    JoinAccepted,
    JoinRejected,
    InviteDeclined,
    Offer,
    Answer,
    IceCandidate,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ServerEnvelope {
    id: String,
    #[serde(rename = "type")]
    kind: SignalType,
    from: Uuid,
    from_presence_id: String,
    to_presence_id: String,
    session_id: String,
    payload: Value,
}

struct ForwardMessage {
    target_presence_id: String,
    envelope: ServerEnvelope,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ErrorEnvelope {
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<String>,
    #[serde(rename = "type")]
    kind: &'static str,
    code: &'static str,
    message: String,
}

struct SignalError {
    code: &'static str,
    message: String,
}

impl SignalError {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    fn bad_request(message: impl Into<String>) -> Self {
        Self::new("BAD_REQUEST", message)
    }

    fn target_unavailable() -> Self {
        Self::new("TARGET_UNAVAILABLE", "Target is not connected to signaling")
    }

    fn storage(error: anyhow::Error) -> Self {
        warn!(error = %error, "signaling storage operation failed");
        Self::new("SERVICE_UNAVAILABLE", "Signaling service is unavailable")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_payload_limits() {
        let mut envelope = ClientEnvelope {
            id: "id".to_owned(),
            kind: SignalType::IceCandidate,
            to: Uuid::new_v4(),
            to_presence_id: "presence".to_owned(),
            session_id: "session".to_owned(),
            payload: Value::String("x".repeat(MAX_ICE_PAYLOAD_BYTES)),
        };
        assert!(validate_envelope(&envelope).is_err());
        envelope.payload = Value::String("candidate".to_owned());
        assert!(validate_envelope(&envelope).is_ok());
    }

    #[test]
    fn rejects_control_characters_in_routing_fields() {
        assert!(validate_text("bad\nvalue", 128, "field").is_err());
        assert!(validate_text("valid", 128, "field").is_ok());
    }
}
