use std::str::FromStr;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::model::friend::{
    FriendRequest, FriendSnapshot, FriendSource, Friendship, normalize_friend_pair,
};

#[derive(Clone)]
pub struct FriendRepository {
    pool: PgPool,
}

impl FriendRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn snapshot(&self, profile_id: Uuid) -> Result<FriendSnapshot> {
        let friendships = sqlx::query_as::<_, FriendshipRow>(
            r#"
            SELECT profile_low, profile_high, source, created_at, updated_at
            FROM friendships
            WHERE profile_low = $1 OR profile_high = $1
            ORDER BY created_at
            "#,
        )
        .bind(profile_id)
        .fetch_all(&self.pool)
        .await
        .context("failed to load friendships")?;

        let incoming = self.requests_by_target(profile_id).await?;
        let outgoing = self.requests_by_requester(profile_id).await?;

        Ok(FriendSnapshot {
            friends: friendships
                .into_iter()
                .map(Friendship::try_from)
                .collect::<Result<Vec<_>>>()?,
            incoming_requests: incoming,
            outgoing_requests: outgoing,
        })
    }

    pub async fn are_friends(&self, first: Uuid, second: Uuid) -> Result<bool> {
        let Some((profile_low, profile_high)) = normalize_friend_pair(first, second) else {
            return Ok(false);
        };

        friendship_exists_pool(&self.pool, profile_low, profile_high).await
    }

    pub async fn replace_with_official_snapshot(
        &self,
        profile_id: Uuid,
        friend_ids: &[Uuid],
        incoming_ids: &[Uuid],
        outgoing_ids: &[Uuid],
    ) -> Result<()> {
        let mut transaction = self.pool.begin().await?;

        sqlx::query(
            "DELETE FROM friend_requests WHERE requester_profile_id = $1 OR target_profile_id = $1",
        )
        .bind(profile_id)
        .execute(&mut *transaction)
        .await?;
        sqlx::query("DELETE FROM friendships WHERE profile_low = $1 OR profile_high = $1")
            .bind(profile_id)
            .execute(&mut *transaction)
            .await?;

        for friend_id in friend_ids {
            let Some((profile_low, profile_high)) = normalize_friend_pair(profile_id, *friend_id)
            else {
                continue;
            };
            insert_friendship(
                &mut transaction,
                profile_low,
                profile_high,
                FriendSource::MinecraftSync,
            )
            .await?;
        }
        for requester in incoming_ids {
            insert_official_request(&mut transaction, *requester, profile_id).await?;
        }
        for target in outgoing_ids {
            insert_official_request(&mut transaction, profile_id, *target).await?;
        }

        transaction.commit().await?;
        Ok(())
    }

    async fn requests_by_target(&self, profile_id: Uuid) -> Result<Vec<FriendRequest>> {
        request_rows(&self.pool, "target_profile_id", profile_id).await
    }

    async fn requests_by_requester(&self, profile_id: Uuid) -> Result<Vec<FriendRequest>> {
        request_rows(&self.pool, "requester_profile_id", profile_id).await
    }
}

#[derive(FromRow)]
struct FriendshipRow {
    profile_low: Uuid,
    profile_high: Uuid,
    source: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(FromRow)]
struct FriendRequestRow {
    requester_profile_id: Uuid,
    target_profile_id: Uuid,
    source: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl TryFrom<FriendshipRow> for Friendship {
    type Error = anyhow::Error;

    fn try_from(row: FriendshipRow) -> Result<Self> {
        Ok(Self {
            profile_low: row.profile_low,
            profile_high: row.profile_high,
            source: FriendSource::from_str(&row.source)?,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }
}

impl TryFrom<FriendRequestRow> for FriendRequest {
    type Error = anyhow::Error;

    fn try_from(row: FriendRequestRow) -> Result<Self> {
        Ok(Self {
            requester_profile_id: row.requester_profile_id,
            target_profile_id: row.target_profile_id,
            source: FriendSource::from_str(&row.source)?,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }
}

async fn request_rows(pool: &PgPool, column: &str, profile_id: Uuid) -> Result<Vec<FriendRequest>> {
    let query = format!(
        r#"
        SELECT requester_profile_id, target_profile_id, source, created_at, updated_at
        FROM friend_requests
        WHERE {column} = $1
        ORDER BY created_at
        "#
    );
    let rows = sqlx::query_as::<_, FriendRequestRow>(sqlx::AssertSqlSafe(query))
        .bind(profile_id)
        .fetch_all(pool)
        .await?;

    rows.into_iter().map(FriendRequest::try_from).collect()
}

async fn friendship_exists_pool(
    pool: &PgPool,
    profile_low: Uuid,
    profile_high: Uuid,
) -> Result<bool> {
    Ok(sqlx::query_scalar::<_, bool>(
        r#"
        SELECT EXISTS(
            SELECT 1 FROM friendships
            WHERE profile_low = $1 AND profile_high = $2
        )
        "#,
    )
    .bind(profile_low)
    .bind(profile_high)
    .fetch_one(pool)
    .await?)
}

async fn insert_friendship(
    transaction: &mut Transaction<'_, Postgres>,
    profile_low: Uuid,
    profile_high: Uuid,
    source: FriendSource,
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO friendships (profile_low, profile_high, source)
        VALUES ($1, $2, $3)
        ON CONFLICT (profile_low, profile_high)
        DO UPDATE SET source = EXCLUDED.source, updated_at = NOW()
        "#,
    )
    .bind(profile_low)
    .bind(profile_high)
    .bind(source.as_str())
    .execute(&mut **transaction)
    .await?;

    Ok(())
}

async fn insert_official_request(
    transaction: &mut Transaction<'_, Postgres>,
    requester: Uuid,
    target: Uuid,
) -> Result<()> {
    if requester == target {
        return Ok(());
    }
    sqlx::query(
        r#"
        INSERT INTO friend_requests (requester_profile_id, target_profile_id, source)
        VALUES ($1, $2, $3)
        ON CONFLICT (requester_profile_id, target_profile_id)
        DO UPDATE SET source = EXCLUDED.source, updated_at = NOW()
        "#,
    )
    .bind(requester)
    .bind(target)
    .bind(FriendSource::MinecraftSync.as_str())
    .execute(&mut **transaction)
    .await?;
    Ok(())
}
