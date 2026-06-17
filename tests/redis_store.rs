use std::{env, time::Duration};

use anyhow::Result;
use chrono::{Duration as ChronoDuration, Utc};
use nli_server::{
    model::{
        presence::{Presence, PresenceStatus},
        runtime_instance::RuntimeInstance,
        signaling::{SignalingPeer, SignalingSession},
        token::RuntimeTokenHash,
    },
    redis::RedisStore,
};
use uuid::Uuid;

#[tokio::test]
#[ignore = "requires a local Redis server"]
async fn redis_runtime_models_round_trip() -> Result<()> {
    dotenvy::dotenv().ok();
    let redis_url = env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379/0".to_owned());
    let store = RedisStore::connect(&redis_url).await?;
    let test_id = Uuid::new_v4();
    let profile_id = Uuid::new_v4();
    let presence_id = format!("test-presence-{test_id}");
    let token_hash = RuntimeTokenHash::from_token(&format!("test-token-{test_id}"));
    let rotated_token_hash = RuntimeTokenHash::from_token(&format!("test-token-rotated-{test_id}"));
    let now = Utc::now();
    let ttl = Duration::from_secs(30);

    store.cache_profile(profile_id, "CachedPlayer", ttl).await?;
    assert_eq!(
        store.cached_profile_by_id(profile_id).await?,
        Some((profile_id, "CachedPlayer".to_owned()))
    );
    assert_eq!(
        store.cached_profile_by_name("cachedplayer").await?,
        Some((profile_id, "CachedPlayer".to_owned()))
    );

    let instance = RuntimeInstance {
        profile_id,
        presence_id: presence_id.clone(),
        instance_started_at: now,
        issued_at: now,
        expires_at: now + ChronoDuration::seconds(30),
    };
    store
        .put_runtime_instance(&token_hash, &instance, ttl)
        .await?;
    assert_eq!(
        store.runtime_instance(&token_hash).await?,
        Some(instance.clone())
    );

    store
        .put_runtime_instance(&rotated_token_hash, &instance, ttl)
        .await?;
    assert_eq!(store.runtime_instance(&token_hash).await?, None);
    assert_eq!(
        store.runtime_instance(&rotated_token_hash).await?,
        Some(instance.clone())
    );

    let presence = Presence {
        profile_id,
        presence_id: presence_id.clone(),
        status: PresenceStatus::Hosting,
        joinable: true,
        session_id: Some(format!("host-session-{test_id}")),
        endpoint: None,
        display_text: "Redis integration test".to_owned(),
        updated_at: now,
        expires_at: now,
    };
    let stored_presence = store.put_presence(&presence, ttl).await?;
    assert!(stored_presence.updated_at >= now);
    assert!(stored_presence.expires_at > stored_presence.updated_at);
    assert_eq!(
        store.presence(&presence_id).await?,
        Some(stored_presence.clone())
    );
    assert_eq!(
        store.presences_for_profile(profile_id).await?,
        vec![stored_presence]
    );

    let signaling_session_id = format!("test-signaling-{test_id}");
    let signaling_session = SignalingSession {
        session_id: signaling_session_id.clone(),
        initiator: SignalingPeer {
            profile_id,
            presence_id: presence_id.clone(),
        },
        target: SignalingPeer {
            profile_id: Uuid::new_v4(),
            presence_id: format!("test-target-{test_id}"),
        },
        join_accepted: false,
        offer_sent: false,
        answer_sent: false,
        initiator_ice_candidates: 0,
        target_ice_candidates: 0,
        created_at: now,
        expires_at: now + ChronoDuration::seconds(30),
    };
    store.put_signaling_session(&signaling_session, ttl).await?;
    assert_eq!(
        store.signaling_session(&signaling_session_id).await?,
        Some(signaling_session.clone())
    );

    assert_eq!(
        store
            .delete_signaling_sessions_for_presence(&presence_id)
            .await?,
        1
    );
    assert_eq!(store.signaling_session(&signaling_session_id).await?, None);

    let nonce_namespace = format!("test-{test_id}");
    store.put_nonce(&nonce_namespace, "nonce", ttl).await?;
    assert!(store.consume_nonce(&nonce_namespace, "nonce").await?);
    assert!(!store.consume_nonce(&nonce_namespace, "nonce").await?);

    let rate_bucket = format!("test-{test_id}");
    assert_eq!(
        store
            .increment_rate_limit(&rate_bucket, Duration::from_secs(2))
            .await?,
        1
    );
    assert_eq!(
        store
            .increment_rate_limit(&rate_bucket, Duration::from_secs(2))
            .await?,
        2
    );

    assert!(
        !store
            .delete_signaling_session(&signaling_session_id)
            .await?
    );
    assert!(store.delete_presence(&presence_id).await?);
    assert!(store.delete_runtime_instance(&rotated_token_hash).await?);
    assert_eq!(store.presence(&presence_id).await?, None);
    assert_eq!(store.runtime_instance(&rotated_token_hash).await?, None);

    Ok(())
}
