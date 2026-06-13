use std::time::Instant;

use axum::{
    body::Body,
    extract::{MatchedPath, State},
    http::{Request, StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};
use metrics::{describe_counter, describe_gauge, describe_histogram, histogram};
use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};

use crate::state::AppState;

pub fn install_metrics() -> anyhow::Result<PrometheusHandle> {
    let handle = PrometheusBuilder::new().install_recorder()?;
    describe_counter!(
        "nli_http_requests_total",
        "HTTP responses by route, method, and status"
    );
    describe_histogram!(
        "nli_http_request_duration_seconds",
        "HTTP request duration by route and method"
    );
    describe_gauge!(
        "nli_websocket_connections",
        "Active signaling WebSocket connections"
    );
    describe_counter!(
        "nli_upstream_errors_total",
        "Minecraft upstream failures by operation"
    );
    describe_counter!(
        "nli_rate_limited_total",
        "Rate-limited operations by endpoint"
    );
    describe_counter!(
        "nli_official_friend_sync_total",
        "Official friend bridge results"
    );
    Ok(handle)
}

pub async fn track_http(request: Request<Body>, next: Next) -> Response {
    let method = request.method().as_str().to_owned();
    let route = request
        .extensions()
        .get::<MatchedPath>()
        .map(MatchedPath::as_str)
        .unwrap_or("unmatched")
        .to_owned();
    let started = Instant::now();
    let response = next.run(request).await;
    let status = response.status().as_u16().to_string();
    metrics::counter!(
        "nli_http_requests_total",
        "route" => route.clone(),
        "method" => method.clone(),
        "status" => status
    )
    .increment(1);
    histogram!(
        "nli_http_request_duration_seconds",
        "route" => route,
        "method" => method
    )
    .record(started.elapsed().as_secs_f64());
    response
}

pub async fn metrics(State(state): State<AppState>) -> Response {
    let Some(handle) = &state.metrics else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };
    (
        [(
            header::CONTENT_TYPE,
            "text/plain; version=0.0.4; charset=utf-8",
        )],
        handle.render(),
    )
        .into_response()
}
