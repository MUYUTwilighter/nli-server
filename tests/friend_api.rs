use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use anyhow::Result;
use axum::{Json, Router, extract::Path, http::HeaderMap, routing::get};
use chrono::Utc;
use nli_server::{
    api::{AppState, router},
    config::AppConfig,
    db,
    model::presence::{Presence, PresenceStatus},
    redis::RedisStore,
};
use serde_json::{Value, json};
use sqlx::PgPool;
use tokio::net::TcpListener;
use uuid::Uuid;

#[derive(Default)]
struct OfficialTestState {
    friends: bool,
    request_from_a: bool,
}

#[tokio::test]
#[ignore = "requires local PostgreSQL and Redis servers"]
async fn friend_api_lifecycle() -> Result<()> {
    dotenvy::dotenv().ok();
    let player_a = Uuid::new_v4();
    let player_b = Uuid::new_v4();
    let official_removals = Arc::new(AtomicUsize::new(0));
    let official_removals_server = official_removals.clone();
    let official_state = Arc::new(Mutex::new(OfficialTestState::default()));
    let official_get_state = official_state.clone();
    let official_put_state = official_state.clone();
    let minecraft_listener = TcpListener::bind("127.0.0.1:0").await?;
    let minecraft_address = minecraft_listener.local_addr()?;
    let minecraft_server = tokio::spawn(async move {
        let app = Router::new()
            .route(
                "/minecraft/profile",
                get(move |headers: HeaderMap| async move {
                    match headers
                        .get("authorization")
                        .and_then(|value| value.to_str().ok())
                    {
                        Some("Bearer player-a-token") => Json(profile_json(player_a, "PlayerA")),
                        Some("Bearer player-b-token") => Json(profile_json(player_b, "PlayerB")),
                        _ => Json(json!({ "error": "invalid token" })),
                    }
                }),
            )
            .route(
                "/profiles/by-name/{name}",
                get(move |Path(name): Path<String>| async move {
                    if name.eq_ignore_ascii_case("PlayerB") {
                        Json(profile_json(player_b, "PlayerB"))
                    } else {
                        Json(json!({ "error": "not found" }))
                    }
                }),
            )
            .route(
                "/profiles/by-id/{profile_id}",
                get(move |Path(profile_id): Path<String>| async move {
                    if profile_id == player_a.simple().to_string() {
                        Json(profile_json(player_a, "PlayerA"))
                    } else if profile_id == player_b.simple().to_string() {
                        Json(profile_json(player_b, "PlayerB"))
                    } else {
                        Json(json!({ "error": "not found" }))
                    }
                }),
            )
            .route(
                "/friends",
                get(move |headers: HeaderMap| {
                    let official_state = official_get_state.clone();
                    async move {
                        let token = minecraft_token(&headers);
                        Json(official_snapshot(
                            &official_state.lock().unwrap(),
                            token,
                            player_a,
                            player_b,
                        ))
                    }
                })
                .put(move |headers: HeaderMap, Json(body): Json<Value>| {
                    let official_removals = official_removals_server.clone();
                    let official_state = official_put_state.clone();
                    async move {
                        let token = minecraft_token(&headers);
                        let update_type = body["updateType"].as_str().unwrap();
                        let mut state = official_state.lock().unwrap();
                        match (token, update_type) {
                            ("player-a-token", "ADD") => {
                                assert!(
                                    body["name"] == "PlayerB"
                                        || body["profileId"] == player_b.to_string()
                                );
                                if !state.friends {
                                    state.request_from_a = true;
                                }
                            }
                            ("player-b-token", "ADD") => {
                                assert_eq!(body["profileId"], player_a.to_string());
                                assert!(state.request_from_a);
                                state.request_from_a = false;
                                state.friends = true;
                            }
                            ("player-a-token", "REMOVE") => {
                                assert_eq!(body["profileId"], player_b.to_string());
                                state.request_from_a = false;
                                state.friends = false;
                                official_removals.fetch_add(1, Ordering::SeqCst);
                            }
                            _ => panic!("unexpected official friend update: {token} {body}"),
                        }
                        Json(official_snapshot(&state, token, player_a, player_b))
                    }
                }),
            );
        axum::serve(minecraft_listener, app).await
    });

    let mut config = AppConfig::from_env()?;
    config.minecraft_profile_url =
        format!("http://{minecraft_address}/minecraft/profile").parse()?;
    config.minecraft_profile_by_name_url =
        format!("http://{minecraft_address}/profiles/by-name/").parse()?;
    config.minecraft_profile_by_id_url =
        format!("http://{minecraft_address}/profiles/by-id/").parse()?;
    config.minecraft_friends_url = format!("http://{minecraft_address}/friends").parse()?;
    let database = db::connect(&config.database_url).await?;
    cleanup_profiles(&database, &[player_a, player_b]).await?;
    let redis = RedisStore::connect(&config.redis_url).await?;
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
    let player_a_instance = register_instance(&client, address, "player-a-token").await?;
    let player_b_instance = register_instance(&client, address, "player-b-token").await?;

    let response = client
        .get(format!("http://{address}/v1/friends"))
        .bearer_auth("player-a-token")
        .header("x-minecraft-access-token", "player-a-token")
        .send()
        .await?;
    assert_eq!(response.status(), reqwest::StatusCode::UNAUTHORIZED);
    assert_eq!(
        response.json::<Value>().await?["code"],
        "INVALID_INSTANCE_TOKEN"
    );

    let response = client
        .get(format!("http://{address}/v1/friends"))
        .bearer_auth(&player_a_instance)
        .send()
        .await?;
    assert_eq!(response.status(), reqwest::StatusCode::UNAUTHORIZED);
    assert_eq!(
        response.json::<Value>().await?["code"],
        "MINECRAFT_TOKEN_REQUIRED"
    );

    let response = client
        .post(format!("http://{address}/v1/friends/requests"))
        .bearer_auth(&player_a_instance)
        .header("x-minecraft-access-token", "player-a-token")
        .json(&json!({ "name": "PlayerB" }))
        .send()
        .await?;
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    let body: Value = response.json().await?;
    assert_eq!(body["relationship"], "REQUESTED");
    assert_eq!(body["officialSync"], "SUCCESS");

    let snapshot_a =
        friend_snapshot(&client, address, &player_a_instance, "player-a-token").await?;
    assert_eq!(
        snapshot_a["outgoingRequests"][0]["profileId"],
        player_b.to_string()
    );
    assert_eq!(snapshot_a["outgoingRequests"][0]["name"], "PlayerB");
    let snapshot_b =
        friend_snapshot(&client, address, &player_b_instance, "player-b-token").await?;
    assert_eq!(
        snapshot_b["incomingRequests"][0]["profileId"],
        player_a.to_string()
    );
    assert_eq!(snapshot_b["incomingRequests"][0]["name"], "PlayerA");

    let response = client
        .post(format!("http://{address}/v1/friends/requests/{player_a}"))
        .bearer_auth(&player_b_instance)
        .header("x-minecraft-access-token", "player-b-token")
        .send()
        .await?;
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    assert_eq!(response.json::<Value>().await?["relationship"], "ACCEPTED");

    let snapshot_a =
        friend_snapshot(&client, address, &player_a_instance, "player-a-token").await?;
    assert_eq!(snapshot_a["friends"][0]["profileId"], player_b.to_string());
    assert_eq!(snapshot_a["friends"][0]["name"], "PlayerB");
    assert_eq!(snapshot_a["friends"][0]["source"], "minecraft_sync");
    let initial_presences = snapshot_a["friends"][0]["presences"].as_array().unwrap();
    assert_eq!(initial_presences.len(), 1);
    assert_eq!(initial_presences[0]["profileId"], player_b.to_string());
    assert_eq!(initial_presences[0]["status"], "ONLINE");
    assert_eq!(snapshot_a["incomingRequests"], json!([]));
    assert_eq!(snapshot_a["outgoingRequests"], json!([]));

    let friend_presence_id = format!("friend-api-{player_b}");
    let caller_presence_id = format!("friend-api-{player_a}");
    redis
        .put_presence(
            &test_presence(player_b, &friend_presence_id),
            Duration::from_secs(60),
        )
        .await?;
    redis
        .put_presence(
            &test_presence(player_a, &caller_presence_id),
            Duration::from_secs(60),
        )
        .await?;
    let body = friend_snapshot(&client, address, &player_a_instance, "player-a-token").await?;
    let presences = body["friends"][0]["presences"].as_array().unwrap();
    assert_eq!(presences.len(), 2);
    assert!(
        presences
            .iter()
            .all(|presence| presence["profileId"] == player_b.to_string())
    );
    assert!(
        presences
            .iter()
            .any(|presence| presence["presenceId"] == friend_presence_id)
    );

    let response = client
        .get(format!("http://{address}/v1/friends/presence"))
        .bearer_auth(&player_a_instance)
        .send()
        .await?;
    assert_eq!(response.status(), reqwest::StatusCode::METHOD_NOT_ALLOWED);

    let response = client
        .post(format!("http://{address}/v1/friends/{player_a}/accept"))
        .bearer_auth(&player_b_instance)
        .send()
        .await?;
    assert_eq!(response.status(), reqwest::StatusCode::NOT_FOUND);

    let response = client
        .delete(format!("http://{address}/v1/friends/request/{player_b}"))
        .bearer_auth(&player_a_instance)
        .send()
        .await?;
    assert_eq!(response.status(), reqwest::StatusCode::NOT_FOUND);

    let response = client
        .post(format!("http://{address}/v1/friends/requests"))
        .bearer_auth(&player_a_instance)
        .header("x-minecraft-access-token", "player-a-token")
        .json(&json!({ "name": "PlayerB" }))
        .send()
        .await?;
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    assert_eq!(response.json::<Value>().await?["relationship"], "ACCEPTED");

    let response = client
        .delete(format!("http://{address}/v1/friends/{player_b}"))
        .bearer_auth(&player_a_instance)
        .header("x-minecraft-access-token", "player-a-token")
        .send()
        .await?;
    assert_eq!(response.status(), reqwest::StatusCode::NO_CONTENT);
    assert_eq!(official_removals.load(Ordering::SeqCst), 1);

    let response = client
        .post(format!("http://{address}/v1/friends/requests"))
        .bearer_auth(&player_a_instance)
        .header("x-minecraft-access-token", "player-a-token")
        .json(&json!({ "name": "PlayerB" }))
        .send()
        .await?;
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    let response = client
        .delete(format!("http://{address}/v1/friends/requests/{player_b}"))
        .bearer_auth(&player_a_instance)
        .header("x-minecraft-access-token", "player-a-token")
        .send()
        .await?;
    assert_eq!(response.status(), reqwest::StatusCode::NO_CONTENT);
    assert_eq!(
        friend_snapshot(&client, address, &player_a_instance, "player-a-token").await?["outgoingRequests"],
        json!([])
    );

    redis.delete_presence(&friend_presence_id).await?;
    redis.delete_presence(&caller_presence_id).await?;
    close_instance(&client, address, &player_a_instance).await?;
    close_instance(&client, address, &player_b_instance).await?;
    cleanup_profiles(&database, &[player_a, player_b]).await?;
    server.abort();
    minecraft_server.abort();
    Ok(())
}

async fn register_instance(
    client: &reqwest::Client,
    address: std::net::SocketAddr,
    minecraft_token: &str,
) -> Result<String> {
    let response = client
        .post(format!("http://{address}/v1/instances"))
        .bearer_auth(minecraft_token)
        .json(&json!({}))
        .send()
        .await?;
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    Ok(response.json::<Value>().await?["instanceToken"]
        .as_str()
        .unwrap()
        .to_owned())
}

async fn close_instance(
    client: &reqwest::Client,
    address: std::net::SocketAddr,
    instance_token: &str,
) -> Result<()> {
    let response = client
        .delete(format!("http://{address}/v1/instances/current"))
        .bearer_auth(instance_token)
        .send()
        .await?;
    assert_eq!(response.status(), reqwest::StatusCode::NO_CONTENT);
    Ok(())
}

fn test_presence(profile_id: Uuid, presence_id: &str) -> Presence {
    let now = Utc::now();
    Presence {
        profile_id,
        presence_id: presence_id.to_owned(),
        status: PresenceStatus::Hosting,
        joinable: true,
        session_id: Some("test-session".to_owned()),
        endpoint: None,
        display_text: "Friend API test world".to_owned(),
        updated_at: now,
        expires_at: now,
    }
}

async fn friend_snapshot(
    client: &reqwest::Client,
    address: std::net::SocketAddr,
    token: &str,
    minecraft_token: &str,
) -> Result<Value> {
    let response = client
        .get(format!("http://{address}/v1/friends"))
        .bearer_auth(token)
        .header("x-minecraft-access-token", minecraft_token)
        .send()
        .await?;
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    Ok(response.json().await?)
}

fn profile_json(profile_id: Uuid, name: &str) -> Value {
    json!({
        "id": profile_id.simple().to_string(),
        "name": name
    })
}

fn minecraft_token(headers: &HeaderMap) -> &str {
    headers
        .get("authorization")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .expect("official request must carry a Minecraft token")
}

fn official_snapshot(
    state: &OfficialTestState,
    token: &str,
    player_a: Uuid,
    player_b: Uuid,
) -> Value {
    let friend = |profile_id, name| json!({ "profileId": profile_id, "name": name });
    match token {
        "player-a-token" if state.friends => json!({
            "friends": [friend(player_b, "PlayerB")],
            "incomingRequests": [],
            "outgoingRequests": []
        }),
        "player-b-token" if state.friends => json!({
            "friends": [friend(player_a, "PlayerA")],
            "incomingRequests": [],
            "outgoingRequests": []
        }),
        "player-a-token" if state.request_from_a => json!({
            "friends": [],
            "incomingRequests": [],
            "outgoingRequests": [friend(player_b, "PlayerB")]
        }),
        "player-b-token" if state.request_from_a => json!({
            "friends": [],
            "incomingRequests": [friend(player_a, "PlayerA")],
            "outgoingRequests": []
        }),
        "player-a-token" | "player-b-token" => {
            json!({ "friends": [], "incomingRequests": [], "outgoingRequests": [] })
        }
        _ => panic!("unexpected Minecraft token: {token}"),
    }
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
