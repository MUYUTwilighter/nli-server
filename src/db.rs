use anyhow::{Context, Result};
use sqlx::{PgPool, postgres::PgPoolOptions};
use std::time::Duration;

pub type DbPool = PgPool;

pub async fn connect(database_url: &str) -> Result<DbPool> {
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .min_connections(1)
        .acquire_timeout(Duration::from_secs(5))
        .connect(database_url)
        .await
        .context("failed to connect to PostgreSQL")?;

    sqlx::query("SELECT 1")
        .execute(&pool)
        .await
        .context("failed to verify PostgreSQL connection")?;

    Ok(pool)
}
