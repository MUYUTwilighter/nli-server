use std::{collections::HashMap, env, time::Duration};

use redis::AsyncCommands;
use sqlx::postgres::PgPoolOptions;

fn required_env(name: &str) -> String {
    env::var(name).unwrap_or_else(|_| panic!("{name} must be set by the WSL gate"))
}

#[tokio::test]
async fn postgres_test_database_is_reachable() {
    let database_url = required_env("NLI_TEST_DATABASE_URL");
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(Duration::from_secs(5))
        .connect(&database_url)
        .await
        .expect("connect to the dedicated PostgreSQL test database");

    let database_name: String = sqlx::query_scalar("SELECT current_database()")
        .fetch_one(&pool)
        .await
        .expect("read the current PostgreSQL database name");
    assert!(
        database_name.ends_with("_test"),
        "dependency tests refuse to use a database without an _test suffix"
    );

    let value: i32 = sqlx::query_scalar("SELECT 1::int4")
        .fetch_one(&pool)
        .await
        .expect("execute a PostgreSQL smoke query");
    assert_eq!(value, 1);

    pool.close().await;
}

#[tokio::test]
async fn redis_test_instance_is_reachable_and_non_persistent() {
    let redis_url = required_env("NLI_TEST_REDIS_URL");
    let client = redis::Client::open(redis_url).expect("parse the Redis test URL");
    let mut connection = client
        .get_multiplexed_async_connection()
        .await
        .expect("connect to the dedicated Redis test instance");

    let pong: String = redis::cmd("PING")
        .query_async(&mut connection)
        .await
        .expect("execute Redis PING");
    assert_eq!(pong, "PONG");

    let appendonly: HashMap<String, String> = redis::cmd("CONFIG")
        .arg("GET")
        .arg("appendonly")
        .query_async(&mut connection)
        .await
        .expect("read Redis appendonly configuration");
    assert_eq!(appendonly.get("appendonly").map(String::as_str), Some("no"));

    let save: HashMap<String, String> = redis::cmd("CONFIG")
        .arg("GET")
        .arg("save")
        .query_async(&mut connection)
        .await
        .expect("read Redis snapshot configuration");
    assert_eq!(save.get("save").map(String::as_str), Some(""));

    let _: () = connection
        .set_ex("nli:v2:test:p0-03", "ephemeral", 30)
        .await
        .expect("write a short-lived Redis smoke key");
    let value: Option<String> = connection
        .get("nli:v2:test:p0-03")
        .await
        .expect("read the Redis smoke key");
    assert_eq!(value.as_deref(), Some("ephemeral"));
    let _: usize = connection
        .del("nli:v2:test:p0-03")
        .await
        .expect("delete the Redis smoke key");
}
