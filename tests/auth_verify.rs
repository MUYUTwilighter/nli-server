use std::env;

use anyhow::Result;
use axum::{Json, Router, http::HeaderMap, routing::get};
use nli_server::{
    api::{AppState, router},
    config::AppConfig,
    db,
    redis::RedisStore,
};
use serde_json::{Value, json};
use tokio::net::TcpListener;

#[tokio::test]
#[ignore = "requires local PostgreSQL and Redis servers"]
async fn auth_verify_maps_minecraft_identity_and_errors() -> Result<()> {
    dotenvy::dotenv().ok();
    let minecraft_listener = TcpListener::bind("127.0.0.1:0").await?;
    let minecraft_address = minecraft_listener.local_addr()?;
    let minecraft_server = tokio::spawn(async move {
        let app = Router::new().route(
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
        );
        axum::serve(minecraft_listener, app).await
    });

    let mut config = AppConfig::from_env()?;
    config.minecraft_profile_url =
        format!("http://{minecraft_address}/minecraft/profile").parse()?;
    let database = db::connect(&env::var("DATABASE_URL")?).await?;
    let redis = RedisStore::connect(&env::var("REDIS_URL")?).await?;
    let state = AppState::with_http_client(config, database, redis, reqwest::Client::new());
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let server = tokio::spawn(async move { axum::serve(listener, router(state)).await });
    let client = reqwest::Client::new();

    let response = client
        .post(format!("http://{address}/v1/auth/verify"))
        .bearer_auth("valid-token")
        .send()
        .await?;
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    let body: Value = response.json().await?;
    assert_eq!(body["profileId"], "069a79f4-44e9-4726-a5be-fca90e38aaf5");
    assert_eq!(body["name"], "Notch");
    assert!(body.get("sessionToken").is_none());

    let response = client
        .post(format!("http://{address}/v1/auth/verify"))
        .send()
        .await?;
    assert_eq!(response.status(), reqwest::StatusCode::UNAUTHORIZED);
    assert_eq!(response.json::<Value>().await?["code"], "UNAUTHORIZED");

    let response = client
        .post(format!("http://{address}/v1/auth/verify"))
        .bearer_auth("invalid-token")
        .send()
        .await?;
    assert_eq!(response.status(), reqwest::StatusCode::UNAUTHORIZED);
    assert_eq!(
        response.json::<Value>().await?["code"],
        "INVALID_MINECRAFT_TOKEN"
    );

    let response = client
        .post(format!("http://{address}/v1/auth/verify"))
        .bearer_auth("upstream-error")
        .send()
        .await?;
    assert_eq!(response.status(), reqwest::StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        response.json::<Value>().await?["code"],
        "SERVICE_UNAVAILABLE"
    );

    server.abort();
    minecraft_server.abort();
    Ok(())
}
