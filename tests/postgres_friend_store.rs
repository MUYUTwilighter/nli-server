use std::env;

use anyhow::Result;
use nli_server::{
    db::{self, friends::FriendRepository},
    model::friend::{FriendSource, normalize_friend_pair},
};
use sqlx::PgPool;
use uuid::Uuid;

#[tokio::test]
#[ignore = "requires a local PostgreSQL server"]
async fn database_constraints_reject_invalid_friend_graph_rows() -> Result<()> {
    dotenvy::dotenv().ok();
    let pool = db::connect(&database_url()).await?;
    let first = Uuid::new_v4();
    let second = Uuid::new_v4();
    let (low, high) = normalize_friend_pair(first, second).unwrap();
    let mut transaction = pool.begin().await?;

    assert!(
        sqlx::query(
            "INSERT INTO friend_requests (requester_profile_id, target_profile_id, source) VALUES ($1, $1, 'minecraft_sync')",
        )
        .bind(first)
        .execute(&mut *transaction)
        .await
        .is_err()
    );
    transaction.rollback().await?;

    let mut transaction = pool.begin().await?;
    assert!(
        sqlx::query(
            "INSERT INTO friendships (profile_low, profile_high, source) VALUES ($1, $2, 'minecraft_sync')",
        )
        .bind(high)
        .bind(low)
        .execute(&mut *transaction)
        .await
        .is_err()
    );
    transaction.rollback().await?;

    let mut transaction = pool.begin().await?;
    sqlx::query(
        "INSERT INTO friend_requests (requester_profile_id, target_profile_id, source) VALUES ($1, $2, 'minecraft_sync')",
    )
    .bind(first)
    .bind(second)
    .execute(&mut *transaction)
    .await?;
    assert!(
        sqlx::query(
            "INSERT INTO friend_requests (requester_profile_id, target_profile_id, source) VALUES ($1, $2, 'minecraft_sync')",
        )
        .bind(second)
        .bind(first)
        .execute(&mut *transaction)
        .await
        .is_err()
    );
    transaction.rollback().await?;

    Ok(())
}

#[tokio::test]
#[ignore = "requires a local PostgreSQL server"]
async fn official_snapshot_replaces_caller_graph() -> Result<()> {
    dotenvy::dotenv().ok();
    let pool = db::connect(&database_url()).await?;
    let repository = FriendRepository::new(pool.clone());
    let caller = Uuid::new_v4();
    let stale_friend = Uuid::new_v4();
    let official_friend = Uuid::new_v4();
    let incoming = Uuid::new_v4();
    let outgoing = Uuid::new_v4();
    let profiles = [caller, stale_friend, official_friend, incoming, outgoing];
    cleanup_profiles(&pool, &profiles).await?;

    repository
        .replace_with_official_snapshot(caller, &[stale_friend], &[], &[])
        .await?;
    repository
        .replace_with_official_snapshot(caller, &[official_friend], &[incoming], &[outgoing])
        .await?;

    let snapshot = repository.snapshot(caller).await?;
    assert_eq!(snapshot.friends.len(), 1);
    assert!(repository.are_friends(caller, official_friend).await?);
    assert!(!repository.are_friends(caller, stale_friend).await?);
    assert_eq!(snapshot.friends[0].source, FriendSource::MinecraftSync);
    assert_eq!(snapshot.incoming_requests[0].requester_profile_id, incoming);
    assert_eq!(snapshot.outgoing_requests[0].target_profile_id, outgoing);

    repository
        .replace_with_official_snapshot(caller, &[], &[], &[])
        .await?;
    let snapshot = repository.snapshot(caller).await?;
    assert!(snapshot.friends.is_empty());
    assert!(snapshot.incoming_requests.is_empty());
    assert!(snapshot.outgoing_requests.is_empty());

    cleanup_profiles(&pool, &profiles).await?;
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

fn database_url() -> String {
    env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@127.0.0.1:5432/nli_server".to_owned())
}
