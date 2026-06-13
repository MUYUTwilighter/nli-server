use axum::{Json, extract::State, http::HeaderMap};
use serde::Serialize;
use tracing::warn;
use uuid::Uuid;

use crate::{
    auth::{BearerTokenError, MinecraftAuthError, bearer_token},
    state::AppState,
};

use super::ApiError;

pub async fn verify(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<AuthVerifyResponse>, ApiError> {
    let access_token = bearer_token(&headers).map_err(map_bearer_error)?;
    let identity = state
        .minecraft_auth
        .verify(&access_token)
        .await
        .map_err(map_minecraft_error)?;

    Ok(Json(AuthVerifyResponse {
        profile_id: identity.profile_id,
        name: identity.name,
    }))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthVerifyResponse {
    profile_id: Uuid,
    name: String,
}

fn map_bearer_error(error: BearerTokenError) -> ApiError {
    ApiError::unauthorized("UNAUTHORIZED", error.to_string())
}

fn map_minecraft_error(error: MinecraftAuthError) -> ApiError {
    match error {
        MinecraftAuthError::InvalidToken => ApiError::unauthorized(
            "INVALID_MINECRAFT_TOKEN",
            "Minecraft access token is invalid",
        ),
        MinecraftAuthError::Request(error) => {
            warn!(error = %error, "Minecraft authentication request failed");
            ApiError::service_unavailable("Minecraft authentication service is unavailable")
        }
        MinecraftAuthError::UpstreamStatus(status) => {
            warn!(%status, "Minecraft authentication service returned an error");
            ApiError::service_unavailable("Minecraft authentication service is unavailable")
        }
        MinecraftAuthError::InvalidResponse(error) => {
            warn!(error = %error, "Minecraft authentication response was invalid");
            ApiError::bad_gateway(
                "AUTH_SERVICE_ERROR",
                "Minecraft authentication service returned an invalid response",
            )
        }
        MinecraftAuthError::InvalidProfileId(_) => {
            warn!("Minecraft authentication response contained an invalid profile id");
            ApiError::bad_gateway(
                "AUTH_SERVICE_ERROR",
                "Minecraft authentication service returned an invalid response",
            )
        }
    }
}
