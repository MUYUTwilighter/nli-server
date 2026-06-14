mod auth;
mod error;
mod friends;
mod health;
mod instances;
mod presence;
mod signaling;
mod turn;

use std::time::Duration;

use axum::{
    Router,
    extract::DefaultBodyLimit,
    http::{HeaderName, Method},
    middleware,
    routing::{delete, get, post, put},
};
use tower_http::{
    cors::{Any, CorsLayer},
    request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer},
    timeout::TimeoutLayer,
    trace::TraceLayer,
};

pub use crate::state::AppState;
pub use error::ApiError;

const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(15);
const DEFAULT_BODY_LIMIT: usize = 256 * 1024;

pub fn router(state: AppState) -> Router {
    let request_id_header = HeaderName::from_static("x-request-id");
    let cors_origin = state.config.cors_allow_origin.clone();

    let app = Router::new()
        .route("/health", get(health::health))
        .route("/metrics", get(crate::observability::metrics))
        .route("/v1/instances", post(instances::create))
        .route("/v1/instances/renew", post(instances::renew))
        .route("/v1/instances/current", delete(instances::close))
        .route("/v1/friends", get(friends::snapshot))
        .route("/v1/friends/requests", post(friends::add_request))
        .route(
            "/v1/friends/request/{profile_id}",
            post(friends::accept_request).delete(friends::delete_request),
        )
        .route("/v1/friends/{profile_id}", delete(friends::remove_friend))
        .route(
            "/v1/presence",
            put(presence::publish).delete(presence::clear),
        )
        .route("/v1/signaling/ws", get(signaling::connect))
        .route("/v1/turn", post(turn::credentials))
        .fallback(error::not_found)
        .with_state(state)
        .layer(middleware::from_fn(crate::observability::track_http))
        .layer(DefaultBodyLimit::max(DEFAULT_BODY_LIMIT))
        .layer(TimeoutLayer::with_status_code(
            axum::http::StatusCode::REQUEST_TIMEOUT,
            DEFAULT_REQUEST_TIMEOUT,
        ))
        .layer(TraceLayer::new_for_http())
        .layer(PropagateRequestIdLayer::new(request_id_header.clone()))
        .layer(SetRequestIdLayer::new(request_id_header, MakeRequestUuid));

    if let Some(origin) = cors_origin {
        app.layer(
            CorsLayer::new()
                .allow_origin(origin)
                .allow_headers(Any)
                .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE]),
        )
    } else {
        app
    }
}
