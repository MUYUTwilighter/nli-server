mod auth;
mod error;
mod health;

use std::time::Duration;

use axum::{
    Router,
    extract::DefaultBodyLimit,
    http::{HeaderName, Method},
    routing::{get, post},
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
        .route("/v1/auth/verify", post(auth::verify))
        .fallback(error::not_found)
        .with_state(state)
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
