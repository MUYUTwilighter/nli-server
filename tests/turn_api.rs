use std::time::Duration;

use anyhow::Result;
use chrono::{Duration as ChronoDuration, Utc};
use nli_server::{
    api::{AppState, router},
    config::AppConfig,
    db,
    model::{runtime_instance::RuntimeInstance, token::RuntimeTokenHash},
    redis::RedisStore,
};
use secrecy::SecretString;
use serde_json::Value;
use tokio::net::TcpListener;
use uuid::Uuid;

#[tokio::test]
#[ignore = "requires local PostgreSQL and Redis servers"]
async fn turn_api_issues_temporary_credentials_and_rate_limits() -> Result<()> {
    dotenvy::dotenv().ok();
    let profile_id = Uuid::new_v4();
    let presence_id = format!("turn-test-{profile_id}");
    let instance_token = format!("turn-token-{profile_id}");
    let token_hash = RuntimeTokenHash::from_token(&instance_token);
    let now = Utc::now();

    let mut config = AppConfig::from_env()?;
    config.turn_urls = vec![
        "stun:turn.test:3478".to_owned(),
        "turn:turn.test:3478?transport=udp".to_owned(),
    ];
    config.turn_shared_secret = SecretString::from("integration-secret".to_owned());
    config.turn_credential_ttl = Duration::from_secs(600);
    let database = db::connect(&config.database_url).await?;
    let redis = RedisStore::connect(&config.redis_url).await?;
    redis
        .put_runtime_instance(
            &token_hash,
            &RuntimeInstance {
                profile_id,
                presence_id: presence_id.clone(),
                instance_started_at: now,
                issued_at: now,
                expires_at: now + ChronoDuration::minutes(30),
            },
            Duration::from_secs(1_800),
        )
        .await?;

    let state = AppState::new(config, database, redis.clone())?;
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let server = tokio::spawn(async move { axum::serve(listener, router(state)).await });
    let client = reqwest::Client::new();

    let issued_after = Utc::now();
    let response = client
        .post(format!("http://{address}/v1/turn"))
        .bearer_auth(&instance_token)
        .send()
        .await?;
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    let body: Value = response.json().await?;
    assert_eq!(
        body["urls"],
        serde_json::json!(["stun:turn.test:3478", "turn:turn.test:3478?transport=udp"])
    );
    let username = body["username"].as_str().unwrap();
    assert!(username.ends_with(&format!(":{profile_id}")));
    assert!(!body["credential"].as_str().unwrap().is_empty());
    let expires_at = body["expiresAt"]
        .as_str()
        .unwrap()
        .parse::<chrono::DateTime<Utc>>()?;
    assert!(expires_at >= issued_after + ChronoDuration::seconds(599));
    assert!(expires_at <= issued_after + ChronoDuration::seconds(601));

    let response = client
        .post(format!("http://{address}/v1/turn"))
        .bearer_auth("invalid-token")
        .send()
        .await?;
    assert_eq!(response.status(), reqwest::StatusCode::UNAUTHORIZED);
    assert_eq!(
        response.json::<Value>().await?["code"],
        "INVALID_INSTANCE_TOKEN"
    );

    for _ in 1..10 {
        let response = client
            .post(format!("http://{address}/v1/turn"))
            .bearer_auth(&instance_token)
            .send()
            .await?;
        assert_eq!(response.status(), reqwest::StatusCode::OK);
    }
    let response = client
        .post(format!("http://{address}/v1/turn"))
        .bearer_auth(&instance_token)
        .send()
        .await?;
    assert_eq!(response.status(), reqwest::StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(response.json::<Value>().await?["code"], "RATE_LIMITED");

    redis.delete_runtime_instance(&token_hash).await?;
    server.abort();
    Ok(())
}
