use std::env;

use anyhow::Result;
use axum::{Json, Router, http::HeaderMap, routing::get};
use nli_server::{
    api::{AppState, router},
    config::AppConfig,
    db::{self, friends::FriendRepository},
    redis::RedisStore,
};
use serde_json::{Value, json};
use tokio::net::TcpListener;
use uuid::Uuid;

#[tokio::test]
#[ignore = "requires local PostgreSQL and Redis servers"]
async fn instance_registration_maps_minecraft_identity_and_errors() -> Result<()> {
    dotenvy::dotenv().ok();
    let minecraft_listener = TcpListener::bind("127.0.0.1:0").await?;
    let minecraft_address = minecraft_listener.local_addr()?;
    let imported_friend = Uuid::new_v4();
    let minecraft_server = tokio::spawn(async move {
        let app = Router::new()
            .route(
                "/minecraft/profile",
                get(|headers: HeaderMap| async move {
                    match headers
                        .get("authorization")
                        .and_then(|value| value.to_str().ok())
                    {
                        Some("Bearer valid-token") => (
                            reqwest::StatusCode::OK,
                            Json(json!({
                                "id": "069a79f444e94726a5befca90e38aaf5",
                                "name": "Notch"
                            })),
                        ),
                        Some("Bearer upstream-error") => (
                            reqwest::StatusCode::INTERNAL_SERVER_ERROR,
                            Json(json!({ "error": "upstream failure" })),
                        ),
                        _ => (
                            reqwest::StatusCode::UNAUTHORIZED,
                            Json(json!({ "error": "invalid token" })),
                        ),
                    }
                }),
            )
            .route(
                "/friends",
                get(move || async move {
                    Json(json!({
                        "friends": [{ "profileId": imported_friend, "name": "ImportedFriend" }],
                        "incomingRequests": [],
                        "outgoingRequests": []
                    }))
                }),
            );
        axum::serve(minecraft_listener, app).await
    });

    let mut config = AppConfig::from_env()?;
    config.minecraft_profile_url =
        format!("http://{minecraft_address}/minecraft/profile").parse()?;
    config.minecraft_friends_url = format!("http://{minecraft_address}/friends").parse()?;
    let database = db::connect(&env::var("DATABASE_URL")?).await?;
    let notch = Uuid::parse_str("069a79f4-44e9-4726-a5be-fca90e38aaf5")?;
    FriendRepository::new(database.clone())
        .remove_friend(notch, imported_friend)
        .await?;
    let redis = RedisStore::connect(&env::var("REDIS_URL")?).await?;
    let state = AppState::with_http_client(
        config,
        database.clone(),
        redis.clone(),
        reqwest::Client::new(),
    );
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let server = tokio::spawn(async move { axum::serve(listener, router(state)).await });
    let client = reqwest::Client::new();

    let response = client
        .post(format!("http://{address}/v1/instances"))
        .bearer_auth("valid-token")
        .json(&json!({}))
        .send()
        .await?;
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    let body: Value = response.json().await?;
    assert_eq!(body["profileId"], "069a79f4-44e9-4726-a5be-fca90e38aaf5");
    assert_eq!(body["name"], "Notch");
    assert!(body["presenceId"].is_string());
    assert!(body["instanceToken"].is_string());
    let instance_token = body["instanceToken"].as_str().unwrap();
    assert!(
        FriendRepository::new(database.clone())
            .are_friends(notch, imported_friend)
            .await?
    );

    let response = client
        .post(format!("http://{address}/v1/instances"))
        .json(&json!({}))
        .send()
        .await?;
    assert_eq!(response.status(), reqwest::StatusCode::UNAUTHORIZED);
    assert_eq!(response.json::<Value>().await?["code"], "UNAUTHORIZED");

    let response = client
        .post(format!("http://{address}/v1/instances"))
        .bearer_auth("invalid-token")
        .json(&json!({}))
        .send()
        .await?;
    assert_eq!(response.status(), reqwest::StatusCode::UNAUTHORIZED);
    assert_eq!(
        response.json::<Value>().await?["code"],
        "INVALID_MINECRAFT_TOKEN"
    );

    let response = client
        .post(format!("http://{address}/v1/instances"))
        .bearer_auth("upstream-error")
        .json(&json!({}))
        .send()
        .await?;
    assert_eq!(response.status(), reqwest::StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        response.json::<Value>().await?["code"],
        "SERVICE_UNAVAILABLE"
    );

    let response = client
        .delete(format!("http://{address}/v1/instances/current"))
        .bearer_auth(instance_token)
        .send()
        .await?;
    assert_eq!(response.status(), reqwest::StatusCode::NO_CONTENT);

    FriendRepository::new(database)
        .remove_friend(notch, imported_friend)
        .await?;

    server.abort();
    minecraft_server.abort();
    Ok(())
}
