use std::env;

use anyhow::Result;
use nli_server::{
    api::{AppState, router},
    config::AppConfig,
    db,
    observability::install_metrics,
    redis::RedisStore,
};
use serde_json::Value;
use tokio::net::TcpListener;

#[tokio::test]
#[ignore = "requires local PostgreSQL and Redis servers"]
async fn health_endpoint_reports_ready_dependencies() -> Result<()> {
    dotenvy::dotenv().ok();
    let config = AppConfig::from_env()?;
    let database = db::connect(&env::var("DATABASE_URL")?).await?;
    let redis = RedisStore::connect(&env::var("REDIS_URL")?).await?;
    let metrics = install_metrics()?;
    let state = AppState::new(config, database, redis)?.with_metrics(metrics);
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let server = tokio::spawn(async move { axum::serve(listener, router(state)).await });
    let client = reqwest::Client::new();

    let response = client
        .get(format!("http://{address}/health"))
        .send()
        .await?;
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    assert!(response.headers().contains_key("x-request-id"));
    let body: Value = response.json().await?;
    assert_eq!(body["status"], "ok");
    assert_eq!(body["dependencies"]["postgres"]["healthy"], true);
    assert_eq!(body["dependencies"]["redis"]["healthy"], true);

    let response = client
        .get(format!("http://{address}/metrics"))
        .send()
        .await?;
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    let body = response.text().await?;
    assert!(body.contains("nli_http_requests_total"));
    assert!(body.contains("route=\"/health\""));

    let response = client
        .get(format!("http://{address}/missing"))
        .send()
        .await?;
    assert_eq!(response.status(), reqwest::StatusCode::NOT_FOUND);
    let body: Value = response.json().await?;
    assert_eq!(body["code"], "NOT_FOUND");

    server.abort();
    Ok(())
}
