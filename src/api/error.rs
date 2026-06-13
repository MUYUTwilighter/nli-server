use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;

#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    code: &'static str,
    message: String,
}

impl ApiError {
    pub fn new(status: StatusCode, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status,
            code,
            message: message.into(),
        }
    }

    pub fn service_unavailable(message: impl Into<String>) -> Self {
        Self::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "SERVICE_UNAVAILABLE",
            message,
        )
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = ErrorEnvelope {
            code: self.code,
            message: self.message,
        };
        (self.status, Json(body)).into_response()
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ErrorEnvelope {
    code: &'static str,
    message: String,
}

pub async fn not_found() -> ApiError {
    ApiError::new(StatusCode::NOT_FOUND, "NOT_FOUND", "Route not found")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn service_unavailable_has_stable_code() {
        let error = ApiError::service_unavailable("unavailable");
        assert_eq!(error.status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(error.code, "SERVICE_UNAVAILABLE");
    }
}
