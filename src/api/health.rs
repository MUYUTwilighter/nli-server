use std::time::{Duration, Instant};

use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};
use serde::Serialize;
use tokio::time::timeout;
use tracing::warn;

use crate::state::AppState;

const DEPENDENCY_TIMEOUT: Duration = Duration::from_secs(2);

pub async fn health(State(state): State<AppState>) -> impl IntoResponse {
    let (postgres, redis) = tokio::join!(
        check_dependency("postgres", state.db_health()),
        check_dependency("redis", state.redis_health()),
    );
    let healthy = postgres.healthy && redis.healthy;
    let status = if healthy {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    let body = HealthResponse {
        status: if healthy { "ok" } else { "degraded" },
        dependencies: DependencyHealth { postgres, redis },
    };

    (status, Json(body))
}

async fn check_dependency<F>(name: &'static str, future: F) -> HealthStatus
where
    F: Future<Output = anyhow::Result<()>>,
{
    let started = Instant::now();
    let result = timeout(DEPENDENCY_TIMEOUT, future).await;
    let healthy = match result {
        Ok(Ok(())) => true,
        Ok(Err(error)) => {
            warn!(dependency = name, error = %error, "health dependency check failed");
            false
        }
        Err(_) => {
            warn!(dependency = name, "health dependency check timed out");
            false
        }
    };

    HealthStatus {
        healthy,
        latency_ms: started.elapsed().as_millis(),
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct HealthResponse {
    status: &'static str,
    dependencies: DependencyHealth,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DependencyHealth {
    postgres: HealthStatus,
    redis: HealthStatus,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct HealthStatus {
    healthy: bool,
    latency_ms: u128,
}
