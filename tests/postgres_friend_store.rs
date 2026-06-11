use std::env;

use anyhow::Result;
use nli_server::{
    db::{
        self,
        friends::{FriendRepository, RequestOutcome},
    },
    model::friend::{FriendSource, normalize_friend_pair},
};
use sqlx::PgPool;
use uuid::Uuid;

#[tokio::test]
#[ignore = "requires a local PostgreSQL server"]
async fn friend_repository_lifecycle() -> Result<()> {
    dotenvy::dotenv().ok();
    let pool = db::connect(&env::var("DATABASE_URL")?).await?;
    let repository = FriendRepository::new(pool.clone());
    let requester = Uuid::new_v4();
    let target = Uuid::new_v4();
    let explicit_requester = Uuid::new_v4();
    let explicit_target = Uuid::new_v4();

    cleanup_profiles(
        &pool,
        &[requester, target, explicit_requester, explicit_target],
    )
    .await?;

    assert_eq!(
        repository
            .request_or_accept(requester, target, FriendSource::MinecraftImport)
            .await?,
        RequestOutcome::Requested
    );
    let requester_snapshot = repository.snapshot(requester).await?;
    assert_eq!(requester_snapshot.outgoing_requests.len(), 1);
    assert!(requester_snapshot.incoming_requests.is_empty());
    let target_snapshot = repository.snapshot(target).await?;
    assert_eq!(target_snapshot.incoming_requests.len(), 1);
    assert_eq!(
        target_snapshot.incoming_requests[0].source,
        FriendSource::MinecraftImport
    );

    assert_eq!(
        repository
            .request_or_accept(target, requester, FriendSource::Netherlink)
            .await?,
        RequestOutcome::Accepted
    );
    assert!(repository.are_friends(requester, target).await?);
    let snapshot = repository.snapshot(requester).await?;
    assert_eq!(snapshot.friends.len(), 1);
    assert_eq!(snapshot.friends[0].source, FriendSource::MinecraftImport);
    assert!(snapshot.incoming_requests.is_empty());
    assert!(snapshot.outgoing_requests.is_empty());
    assert!(
        repository
            .request_or_accept(requester, target, FriendSource::Netherlink)
            .await
            .is_err()
    );
    assert!(repository.remove_friend(target, requester).await?);
    assert!(!repository.are_friends(requester, target).await?);

    assert_eq!(
        repository
            .request_or_accept(
                explicit_requester,
                explicit_target,
                FriendSource::MinecraftSync,
            )
            .await?,
        RequestOutcome::Requested
    );
    assert!(
        repository
            .accept(explicit_target, explicit_requester)
            .await?
    );
    assert!(
        repository
            .are_friends(explicit_requester, explicit_target)
            .await?
    );
    assert_eq!(
        repository.snapshot(explicit_target).await?.friends[0].source,
        FriendSource::MinecraftSync
    );

    assert!(
        repository
            .remove_friend(explicit_requester, explicit_target)
            .await?
    );
    assert_eq!(
        repository
            .request_or_accept(
                explicit_requester,
                explicit_target,
                FriendSource::Netherlink
            )
            .await?,
        RequestOutcome::Requested
    );
    assert!(
        repository
            .delete_request(explicit_target, explicit_requester)
            .await?
    );
    assert!(
        repository
            .snapshot(explicit_requester)
            .await?
            .outgoing_requests
            .is_empty()
    );

    cleanup_profiles(
        &pool,
        &[requester, target, explicit_requester, explicit_target],
    )
    .await?;
    Ok(())
}

#[tokio::test]
#[ignore = "requires a local PostgreSQL server"]
async fn database_constraints_reject_invalid_friend_graph_rows() -> Result<()> {
    dotenvy::dotenv().ok();
    let pool = db::connect(&env::var("DATABASE_URL")?).await?;
    let first = Uuid::new_v4();
    let second = Uuid::new_v4();
    let (low, high) = normalize_friend_pair(first, second).unwrap();
    let mut transaction = pool.begin().await?;

    assert!(
        sqlx::query(
            "INSERT INTO friend_requests (requester_profile_id, target_profile_id, source) VALUES ($1, $1, 'netherlink')",
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
            "INSERT INTO friendships (profile_low, profile_high, source) VALUES ($1, $2, 'netherlink')",
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
        "INSERT INTO friend_requests (requester_profile_id, target_profile_id, source) VALUES ($1, $2, 'netherlink')",
    )
    .bind(first)
    .bind(second)
    .execute(&mut *transaction)
    .await?;
    assert!(
        sqlx::query(
            "INSERT INTO friend_requests (requester_profile_id, target_profile_id, source) VALUES ($1, $2, 'netherlink')",
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
