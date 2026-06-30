use anyhow::Result;
use axum::{Json, Router, routing::get};
use nli_server::{
    api::{AppState, router},
    config::AppConfig,
    db,
    model::{presence::PresenceStatus, token::RuntimeTokenHash},
    redis::RedisStore,
};
use serde_json::{Value, json};
use tokio::net::TcpListener;
use uuid::Uuid;

#[tokio::test]
#[ignore = "requires local PostgreSQL and Redis servers"]
async fn creates_and_renews_runtime_instance() -> Result<()> {
    dotenvy::dotenv().ok();
    let profile_id = Uuid::new_v4();
    let minecraft_listener = TcpListener::bind("127.0.0.1:0").await?;
    let minecraft_address = minecraft_listener.local_addr()?;
    let minecraft_server = tokio::spawn(async move {
        let app = Router::new().route(
            "/minecraft/profile",
            get(move || async move {
                Json(json!({
                    "id": profile_id.simple().to_string(),
                    "name": "RuntimeTestPlayer"
                }))
            }),
        );
        axum::serve(minecraft_listener, app).await
    });

    let mut config = AppConfig::from_env()?;
    config.minecraft_profile_url =
        format!("http://{minecraft_address}/minecraft/profile").parse()?;
    let database = db::connect(&config.database_url).await?;
    let redis = RedisStore::connect(&config.redis_url).await?;
    let state = AppState::with_http_client(config, database, redis.clone(), reqwest::Client::new());
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let server = tokio::spawn(async move { axum::serve(listener, router(state)).await });
    let client = reqwest::Client::new();

    let response = client
        .post(format!("http://{address}/v1/instances"))
        .bearer_auth("minecraft-token")
        .json(&json!({
            "displayText": "  Test world  "
        }))
        .send()
        .await?;
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    let created: Value = response.json().await?;
    assert_eq!(created["profileId"], profile_id.to_string());
    assert_eq!(created["name"], "RuntimeTestPlayer");
    let presence_id = created["presenceId"].as_str().unwrap().to_owned();
    let old_token = created["instanceToken"].as_str().unwrap().to_owned();
    assert_eq!(old_token.len(), 43);

    let old_hash = RuntimeTokenHash::from_token(&old_token);
    let instance = redis.runtime_instance(&old_hash).await?.unwrap();
    assert_eq!(instance.profile_id, profile_id);
    assert_eq!(instance.presence_id, presence_id);
    let presence = redis.presence(&presence_id).await?.unwrap();
    assert_eq!(presence.status, PresenceStatus::Online);
    assert!(!presence.joinable);
    assert_eq!(presence.display_text, "Test world");

    let response = client
        .post(format!("http://{address}/v1/instances/renew"))
        .bearer_auth(&old_token)
        .send()
        .await?;
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    let renewed: Value = response.json().await?;
    assert_eq!(renewed["profileId"], profile_id.to_string());
    assert_eq!(renewed["presenceId"], presence_id);
    let new_token = renewed["instanceToken"].as_str().unwrap().to_owned();
    assert_ne!(new_token, old_token);
    assert_eq!(redis.runtime_instance(&old_hash).await?, None);
    let new_hash = RuntimeTokenHash::from_token(&new_token);
    assert!(redis.runtime_instance(&new_hash).await?.is_some());

    let response = client
        .put(format!("http://{address}/v1/presence"))
        .bearer_auth(&old_token)
        .json(&json!({
            "status": "HOSTING",
            "joinable": true
        }))
        .send()
        .await?;
    assert_eq!(response.status(), reqwest::StatusCode::UNAUTHORIZED);

    let response = client
        .put(format!("http://{address}/v1/presence"))
        .bearer_auth(&new_token)
        .json(&json!({
            "status": "ONLINE",
            "joinable": true
        }))
        .send()
        .await?;
    assert_eq!(response.status(), reqwest::StatusCode::BAD_REQUEST);
    assert_eq!(
        response.json::<Value>().await?["code"],
        "INVALID_PRESENCE_STATE"
    );

    let publish_started = chrono::Utc::now();
    let response = client
        .put(format!("http://{address}/v1/presence"))
        .bearer_auth(&new_token)
        .json(&json!({
            "status": "HOSTING",
            "joinable": true,
            "sessionId": " host-session ",
            "endpoint": " test-endpoint ",
            "ttlSeconds": 999,
            "displayText": "  Hosted test world  "
        }))
        .send()
        .await?;
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    let published: Value = response.json().await?;
    assert_eq!(published["result"], "SUCCESS");
    assert_eq!(published["profileId"], profile_id.to_string());
    assert_eq!(published["presenceId"], presence_id);
    let presence = redis.presence(&presence_id).await?.unwrap();
    assert_eq!(presence.status, PresenceStatus::Hosting);
    assert!(presence.joinable);
    assert_eq!(presence.session_id.as_deref(), Some("host-session"));
    assert_eq!(presence.endpoint.as_deref(), Some("test-endpoint"));
    assert_eq!(presence.display_text, "Hosted test world");
    let ttl = presence.expires_at - publish_started;
    assert!(ttl >= chrono::Duration::seconds(178));
    assert!(ttl <= chrono::Duration::seconds(181));

    let response = client
        .put(format!("http://{address}/v1/presence"))
        .bearer_auth(&new_token)
        .json(&json!({
            "status": "IN_GAME",
            "joinable": false
        }))
        .send()
        .await?;
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    let presence = redis.presence(&presence_id).await?.unwrap();
    assert_eq!(presence.status, PresenceStatus::InGame);
    assert!(!presence.joinable);
    assert_eq!(presence.display_text, "Hosted test world");

    let response = client
        .put(format!("http://{address}/v1/presence"))
        .bearer_auth(&new_token)
        .json(&json!({
            "status": "ONLINE",
            "joinable": false
        }))
        .send()
        .await?;
    assert_eq!(response.status(), reqwest::StatusCode::OK);

    let response = client
        .put(format!("http://{address}/v1/presence"))
        .bearer_auth(&new_token)
        .json(&json!({
            "status": "HOSTING",
            "joinable": true
        }))
        .send()
        .await?;
    assert_eq!(response.status(), reqwest::StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(response.json::<Value>().await?["code"], "RATE_LIMITED");

    let response = client
        .delete(format!("http://{address}/v1/presence"))
        .bearer_auth(&new_token)
        .send()
        .await?;
    assert_eq!(response.status(), reqwest::StatusCode::NO_CONTENT);
    assert_eq!(redis.presence(&presence_id).await?, None);

    let response = client
        .post(format!("http://{address}/v1/instances/renew"))
        .bearer_auth(&old_token)
        .send()
        .await?;
    assert_eq!(response.status(), reqwest::StatusCode::UNAUTHORIZED);
    assert_eq!(
        response.json::<Value>().await?["code"],
        "INVALID_INSTANCE_TOKEN"
    );

    assert!(redis.delete_runtime_instance(&new_hash).await?);
    server.abort();
    minecraft_server.abort();
    Ok(())
}

#[tokio::test]
#[ignore = "requires local PostgreSQL and Redis servers"]
async fn rejects_invalid_instance_request() -> Result<()> {
    dotenvy::dotenv().ok();
    let profile_id = Uuid::new_v4();
    let minecraft_listener = TcpListener::bind("127.0.0.1:0").await?;
    let minecraft_address = minecraft_listener.local_addr()?;
    let minecraft_server = tokio::spawn(async move {
        let app = Router::new().route(
            "/minecraft/profile",
            get(move || async move {
                Json(json!({
                    "id": profile_id.simple().to_string(),
                    "name": "RuntimeValidationPlayer"
                }))
            }),
        );
        axum::serve(minecraft_listener, app).await
    });

    let mut config = AppConfig::from_env()?;
    config.minecraft_profile_url =
        format!("http://{minecraft_address}/minecraft/profile").parse()?;
    let database = db::connect(&config.database_url).await?;
    let redis = RedisStore::connect(&config.redis_url).await?;
    let state = AppState::with_http_client(config, database, redis, reqwest::Client::new());
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let server = tokio::spawn(async move { axum::serve(listener, router(state)).await });

    let response = reqwest::Client::new()
        .post(format!("http://{address}/v1/instances"))
        .bearer_auth("minecraft-token")
        .json(&json!({ "displayText": "x".repeat(97) }))
        .send()
        .await?;
    assert_eq!(response.status(), reqwest::StatusCode::BAD_REQUEST);
    assert_eq!(
        response.json::<Value>().await?["code"],
        "INVALID_DISPLAY_TEXT"
    );

    server.abort();
    minecraft_server.abort();
    Ok(())
}

#[tokio::test]
#[ignore = "requires local PostgreSQL and Redis servers"]
async fn limits_runtime_instances_per_profile() -> Result<()> {
    dotenvy::dotenv().ok();
    let profile_id = Uuid::new_v4();
    let minecraft_listener = TcpListener::bind("127.0.0.1:0").await?;
    let minecraft_address = minecraft_listener.local_addr()?;
    let minecraft_server = tokio::spawn(async move {
        let app = Router::new().route(
            "/minecraft/profile",
            get(move || async move {
                Json(json!({
                    "id": profile_id.simple().to_string(),
                    "name": "InstanceLimitPlayer"
                }))
            }),
        );
        axum::serve(minecraft_listener, app).await
    });

    let mut config = AppConfig::from_env()?;
    config.minecraft_profile_url =
        format!("http://{minecraft_address}/minecraft/profile").parse()?;
    let database = db::connect(&config.database_url).await?;
    let redis = RedisStore::connect(&config.redis_url).await?;
    let state = AppState::with_http_client(config, database, redis, reqwest::Client::new());
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let server = tokio::spawn(async move { axum::serve(listener, router(state)).await });
    let client = reqwest::Client::new();
    let mut instance_tokens = Vec::new();

    for _ in 0..5 {
        let response = client
            .post(format!("http://{address}/v1/instances"))
            .bearer_auth("minecraft-token")
            .json(&json!({}))
            .send()
            .await?;
        assert_eq!(response.status(), reqwest::StatusCode::OK);
        instance_tokens.push(
            response.json::<Value>().await?["instanceToken"]
                .as_str()
                .unwrap()
                .to_owned(),
        );
    }

    let response = client
        .post(format!("http://{address}/v1/instances"))
        .bearer_auth("minecraft-token")
        .json(&json!({}))
        .send()
        .await?;
    assert_eq!(response.status(), reqwest::StatusCode::CONFLICT);
    assert_eq!(
        response.json::<Value>().await?["code"],
        "INSTANCE_LIMIT_REACHED"
    );

    let released_token = instance_tokens.pop().unwrap();
    let response = client
        .delete(format!("http://{address}/v1/instances/current"))
        .bearer_auth(&released_token)
        .send()
        .await?;
    assert_eq!(response.status(), reqwest::StatusCode::NO_CONTENT);

    let response = client
        .post(format!("http://{address}/v1/instances"))
        .bearer_auth("minecraft-token")
        .json(&json!({}))
        .send()
        .await?;
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    instance_tokens.push(
        response.json::<Value>().await?["instanceToken"]
            .as_str()
            .unwrap()
            .to_owned(),
    );

    for token in instance_tokens {
        let response = client
            .delete(format!("http://{address}/v1/instances/current"))
            .bearer_auth(token)
            .send()
            .await?;
        assert_eq!(response.status(), reqwest::StatusCode::NO_CONTENT);
    }

    server.abort();
    minecraft_server.abort();
    Ok(())
}
