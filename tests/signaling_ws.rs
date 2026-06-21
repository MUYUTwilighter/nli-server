use std::time::Duration;

use anyhow::{Context, Result};
use chrono::{Duration as ChronoDuration, Utc};
use futures_util::{SinkExt, StreamExt};
use nli_server::{
    api::{AppState, router},
    config::AppConfig,
    db::{self, friends::FriendRepository},
    model::{
        presence::{Presence, PresenceStatus},
        runtime_instance::RuntimeInstance,
        signaling::{SignalingPeer, SignalingSession},
        token::RuntimeTokenHash,
    },
    redis::RedisStore,
};
use serde_json::{Value, json};
use sqlx::PgPool;
use tokio::{net::TcpListener, time::timeout};
use tokio_tungstenite::{
    MaybeTlsStream, WebSocketStream, connect_async,
    tungstenite::{Message, client::IntoClientRequest, http::HeaderValue},
};
use uuid::Uuid;

type ClientSocket = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;

#[tokio::test]
#[ignore = "requires local PostgreSQL and Redis servers"]
async fn signaling_ws_relays_join_and_webrtc_messages() -> Result<()> {
    dotenvy::dotenv().ok();
    let initiator_profile = Uuid::new_v4();
    let host_profile = Uuid::new_v4();
    let outsider_profile = Uuid::new_v4();
    let initiator_presence = format!("ws-initiator-{initiator_profile}");
    let host_presence = format!("ws-host-{host_profile}");
    let outsider_presence = format!("ws-outsider-{outsider_profile}");
    let initiator_token = format!("ws-token-{initiator_profile}");
    let host_token = format!("ws-token-{host_profile}");
    let outsider_token = format!("ws-token-{outsider_profile}");

    let config = AppConfig::from_env()?;
    let database = db::connect(&config.database_url).await?;
    cleanup_profiles(
        &database,
        &[initiator_profile, host_profile, outsider_profile],
    )
    .await?;
    FriendRepository::new(database.clone())
        .replace_with_official_snapshot(initiator_profile, &[host_profile], &[], &[])
        .await?;

    let redis = RedisStore::connect(&config.redis_url).await?;
    put_instance(
        &redis,
        initiator_profile,
        &initiator_presence,
        &initiator_token,
        false,
    )
    .await?;
    put_instance(&redis, host_profile, &host_presence, &host_token, true).await?;
    put_instance(
        &redis,
        outsider_profile,
        &outsider_presence,
        &outsider_token,
        false,
    )
    .await?;

    let state = AppState::new(config, database.clone(), redis.clone())?;
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let server = tokio::spawn(async move { axum::serve(listener, router(state)).await });

    let mut host = connect(address, &host_token).await?;
    let mut initiator = connect(address, &initiator_token).await?;
    let mut outsider = connect(address, &outsider_token).await?;
    let session_id = Uuid::new_v4().to_string();

    send(
        &mut initiator,
        json!({
            "id": "join",
            "type": "JOIN_REQUEST",
            "to": host_profile,
            "toPresenceId": host_presence,
            "sessionId": session_id,
            "from": outsider_profile,
            "fromPresenceId": outsider_presence,
            "payload": { "world": "test" }
        }),
    )
    .await?;
    let forwarded = receive(&mut host).await?;
    assert_eq!(forwarded["type"], "JOIN_REQUEST");
    assert_eq!(forwarded["from"], initiator_profile.to_string());
    assert_eq!(forwarded["fromPresenceId"], initiator_presence);
    assert_eq!(forwarded["toPresenceId"], host_presence);

    send(
        &mut host,
        frame(
            "accepted",
            "JOIN_ACCEPTED",
            initiator_profile,
            &initiator_presence,
            &session_id,
            json!({}),
        ),
    )
    .await?;
    assert_eq!(receive(&mut initiator).await?["type"], "JOIN_ACCEPTED");

    send(
        &mut initiator,
        frame(
            "offer",
            "OFFER",
            host_profile,
            &host_presence,
            &session_id,
            json!({ "sdp": "offer-sdp" }),
        ),
    )
    .await?;
    assert_eq!(receive(&mut host).await?["payload"]["sdp"], "offer-sdp");

    send(
        &mut host,
        frame(
            "answer",
            "ANSWER",
            initiator_profile,
            &initiator_presence,
            &session_id,
            json!({ "sdp": "answer-sdp" }),
        ),
    )
    .await?;
    assert_eq!(receive(&mut initiator).await?["type"], "ANSWER");

    send(
        &mut initiator,
        frame(
            "ice",
            "ICE_CANDIDATE",
            host_profile,
            &host_presence,
            &session_id,
            json!({ "candidate": "candidate" }),
        ),
    )
    .await?;
    assert_eq!(receive(&mut host).await?["type"], "ICE_CANDIDATE");

    send(
        &mut initiator,
        frame(
            "duplicate-offer",
            "OFFER",
            host_profile,
            &host_presence,
            &session_id,
            json!({ "sdp": "second-offer" }),
        ),
    )
    .await?;
    let error = receive(&mut initiator).await?;
    assert_eq!(error["type"], "ERROR");
    assert_eq!(error["code"], "INVALID_SESSION_STATE");

    send(
        &mut host,
        frame(
            "late-rejection",
            "JOIN_REJECTED",
            initiator_profile,
            &initiator_presence,
            &session_id,
            json!({}),
        ),
    )
    .await?;
    let error = receive(&mut host).await?;
    assert_eq!(error["type"], "ERROR");
    assert_eq!(error["code"], "INVALID_SESSION_STATE");

    put_instance(&redis, host_profile, &host_presence, &host_token, true).await?;
    let closed_world_session = Uuid::new_v4().to_string();
    send(
        &mut initiator,
        frame(
            "join-before-close",
            "JOIN_REQUEST",
            host_profile,
            &host_presence,
            &closed_world_session,
            json!({}),
        ),
    )
    .await?;
    assert_eq!(receive(&mut host).await?["type"], "JOIN_REQUEST");
    send(
        &mut host,
        frame(
            "accept-before-close",
            "JOIN_ACCEPTED",
            initiator_profile,
            &initiator_presence,
            &closed_world_session,
            json!({}),
        ),
    )
    .await?;
    assert_eq!(receive(&mut initiator).await?["type"], "JOIN_ACCEPTED");
    put_instance(&redis, host_profile, &host_presence, &host_token, false).await?;
    send(
        &mut initiator,
        frame(
            "offer-after-close",
            "OFFER",
            host_profile,
            &host_presence,
            &closed_world_session,
            json!({ "sdp": "offer-after-close" }),
        ),
    )
    .await?;
    let error = receive(&mut initiator).await?;
    assert_eq!(error["type"], "ERROR");
    assert_eq!(error["code"], "TARGET_NOT_JOINABLE");
    assert_eq!(redis.signaling_session(&closed_world_session).await?, None);

    send(
        &mut initiator,
        frame(
            "not-joinable",
            "JOIN_REQUEST",
            host_profile,
            &host_presence,
            &Uuid::new_v4().to_string(),
            json!({}),
        ),
    )
    .await?;
    let error = receive(&mut initiator).await?;
    assert_eq!(error["type"], "ERROR");
    assert_eq!(error["code"], "TARGET_NOT_JOINABLE");

    put_instance(&redis, host_profile, &host_presence, &host_token, true).await?;
    let disconnected_session = Uuid::new_v4().to_string();
    send(
        &mut initiator,
        frame(
            "join-before-disconnect",
            "JOIN_REQUEST",
            host_profile,
            &host_presence,
            &disconnected_session,
            json!({}),
        ),
    )
    .await?;
    assert_eq!(receive(&mut host).await?["type"], "JOIN_REQUEST");
    send(
        &mut host,
        frame(
            "accept-before-disconnect",
            "JOIN_ACCEPTED",
            initiator_profile,
            &initiator_presence,
            &disconnected_session,
            json!({}),
        ),
    )
    .await?;
    assert_eq!(receive(&mut initiator).await?["type"], "JOIN_ACCEPTED");
    host.close(None).await?;
    tokio::time::sleep(Duration::from_millis(50)).await;
    send(
        &mut initiator,
        frame(
            "offer-after-disconnect",
            "OFFER",
            host_profile,
            &host_presence,
            &disconnected_session,
            json!({ "sdp": "offer-after-disconnect" }),
        ),
    )
    .await?;
    let error = receive(&mut initiator).await?;
    assert_eq!(error["type"], "ERROR");
    assert_eq!(error["code"], "SESSION_NOT_FOUND");
    assert_eq!(redis.signaling_session(&disconnected_session).await?, None);

    send(
        &mut outsider,
        frame(
            "forbidden",
            "JOIN_REQUEST",
            host_profile,
            &host_presence,
            &Uuid::new_v4().to_string(),
            json!({}),
        ),
    )
    .await?;
    let error = receive(&mut outsider).await?;
    assert_eq!(error["type"], "ERROR");
    assert_eq!(error["code"], "NOT_FRIENDS");

    redis
        .delete_runtime_instance(&RuntimeTokenHash::from_token(&outsider_token))
        .await?;
    send(
        &mut outsider,
        frame(
            "revoked-token",
            "JOIN_REQUEST",
            host_profile,
            &host_presence,
            &Uuid::new_v4().to_string(),
            json!({}),
        ),
    )
    .await?;
    let error = receive(&mut outsider).await?;
    assert_eq!(error["type"], "ERROR");
    assert_eq!(error["code"], "INVALID_INSTANCE_TOKEN");

    let shutdown_session_id = Uuid::new_v4().to_string();
    let shutdown_now = Utc::now();
    redis
        .put_signaling_session(
            &SignalingSession {
                session_id: shutdown_session_id.clone(),
                initiator: SignalingPeer {
                    profile_id: initiator_profile,
                    presence_id: initiator_presence.clone(),
                },
                target: SignalingPeer {
                    profile_id: host_profile,
                    presence_id: host_presence.clone(),
                },
                join_accepted: false,
                offer_sent: false,
                answer_sent: false,
                initiator_ice_candidates: 0,
                target_ice_candidates: 0,
                created_at: shutdown_now,
                expires_at: shutdown_now + ChronoDuration::minutes(5),
            },
            Duration::from_secs(300),
        )
        .await?;
    let response = reqwest::Client::new()
        .delete(format!("http://{address}/v1/instances/current"))
        .bearer_auth(&initiator_token)
        .send()
        .await?;
    assert_eq!(response.status(), reqwest::StatusCode::NO_CONTENT);
    assert_eq!(
        redis
            .runtime_instance(&RuntimeTokenHash::from_token(&initiator_token))
            .await?,
        None
    );
    assert_eq!(redis.presence(&initiator_presence).await?, None);
    assert_eq!(redis.signaling_session(&shutdown_session_id).await?, None);
    let close = timeout(Duration::from_secs(2), initiator.next())
        .await
        .context("timed out waiting for instance-close WebSocket frame")?
        .context("WebSocket ended without a close frame")??;
    assert!(matches!(close, Message::Close(_)));

    outsider.close(None).await?;
    redis.delete_signaling_session(&session_id).await?;
    cleanup_instance(&redis, &initiator_presence, &initiator_token).await?;
    cleanup_instance(&redis, &host_presence, &host_token).await?;
    cleanup_instance(&redis, &outsider_presence, &outsider_token).await?;
    cleanup_profiles(
        &database,
        &[initiator_profile, host_profile, outsider_profile],
    )
    .await?;
    server.abort();
    Ok(())
}

async fn connect(address: std::net::SocketAddr, token: &str) -> Result<ClientSocket> {
    let mut request = format!("ws://{address}/v1/signaling/ws").into_client_request()?;
    request.headers_mut().insert(
        "authorization",
        HeaderValue::from_str(&format!("Bearer {token}"))?,
    );
    let (socket, response) = connect_async(request).await?;
    assert_eq!(response.status(), 101);
    Ok(socket)
}

async fn send(socket: &mut ClientSocket, value: Value) -> Result<()> {
    socket.send(Message::Text(value.to_string().into())).await?;
    Ok(())
}

async fn receive(socket: &mut ClientSocket) -> Result<Value> {
    let message = timeout(Duration::from_secs(2), socket.next())
        .await
        .context("timed out waiting for WebSocket frame")?
        .context("WebSocket closed before receiving a frame")??;
    let text = message.into_text()?;
    Ok(serde_json::from_str(&text)?)
}

fn frame(
    id: &str,
    kind: &str,
    to: Uuid,
    to_presence_id: &str,
    session_id: &str,
    payload: Value,
) -> Value {
    json!({
        "id": id,
        "type": kind,
        "to": to,
        "toPresenceId": to_presence_id,
        "sessionId": session_id,
        "payload": payload
    })
}

async fn put_instance(
    redis: &RedisStore,
    profile_id: Uuid,
    presence_id: &str,
    token: &str,
    joinable: bool,
) -> Result<()> {
    let now = Utc::now();
    let ttl = Duration::from_secs(60);
    redis
        .put_runtime_instance(
            &RuntimeTokenHash::from_token(token),
            &RuntimeInstance {
                profile_id,
                presence_id: presence_id.to_owned(),
                instance_started_at: now,
                issued_at: now,
                expires_at: now + ChronoDuration::seconds(60),
            },
            ttl,
        )
        .await?;
    redis
        .put_presence(
            &Presence {
                profile_id,
                presence_id: presence_id.to_owned(),
                status: if joinable {
                    PresenceStatus::Hosting
                } else {
                    PresenceStatus::Online
                },
                joinable,
                session_id: None,
                endpoint: None,
                display_text: "WebSocket integration test".to_owned(),
                updated_at: now,
                expires_at: now,
            },
            ttl,
        )
        .await?;
    Ok(())
}

async fn cleanup_instance(redis: &RedisStore, presence_id: &str, token: &str) -> Result<()> {
    redis.delete_presence(presence_id).await?;
    redis
        .delete_runtime_instance(&RuntimeTokenHash::from_token(token))
        .await?;
    Ok(())
}

async fn cleanup_profiles(pool: &PgPool, profile_ids: &[Uuid]) -> Result<()> {
    for profile_id in profile_ids {
        sqlx::query(
            "DELETE FROM friend_requests WHERE requester_profile_id = $1 OR target_profile_id = $1",
        )
        .bind(profile_id)
        .execute(pool)
        .await?;
        sqlx::query("DELETE FROM friendships WHERE profile_low = $1 OR profile_high = $1")
            .bind(profile_id)
            .execute(pool)
            .await?;
    }
    Ok(())
}
