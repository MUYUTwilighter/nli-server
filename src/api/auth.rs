use axum::http::HeaderMap;
use tracing::warn;

use crate::{
    auth::{BearerTokenError, MinecraftAuthError, ProfileIdentity, bearer_token},
    state::AppState,
};

use super::ApiError;

pub(super) async fn authenticate_minecraft(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<ProfileIdentity, ApiError> {
    let access_token = bearer_token(headers).map_err(map_bearer_error)?;
    state
        .minecraft_auth
        .verify(&access_token)
        .await
        .map_err(map_minecraft_error)
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
