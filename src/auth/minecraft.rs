use reqwest::{Client, StatusCode, Url};
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

#[derive(Clone)]
pub struct MinecraftAuthClient {
    http: Client,
    profile_url: Url,
}

impl MinecraftAuthClient {
    pub fn new(http: Client, profile_url: Url) -> Self {
        Self { http, profile_url }
    }

    pub async fn verify(
        &self,
        access_token: &SecretString,
    ) -> Result<ProfileIdentity, MinecraftAuthError> {
        let response = self
            .http
            .get(self.profile_url.clone())
            .bearer_auth(access_token.expose_secret())
            .send()
            .await
            .map_err(MinecraftAuthError::Request)?;

        match response.status() {
            StatusCode::OK => {}
            status if status.is_client_error() => return Err(MinecraftAuthError::InvalidToken),
            status => return Err(MinecraftAuthError::UpstreamStatus(status)),
        }

        let profile = response
            .json::<MinecraftProfileResponse>()
            .await
            .map_err(MinecraftAuthError::InvalidResponse)?;
        let profile_id = Uuid::parse_str(&profile.id)
            .map_err(|_| MinecraftAuthError::InvalidProfileId(profile.id))?;

        Ok(ProfileIdentity {
            profile_id,
            name: profile.name,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileIdentity {
    pub profile_id: Uuid,
    pub name: String,
}

#[derive(Deserialize)]
struct MinecraftProfileResponse {
    id: String,
    name: String,
}

#[derive(Debug, Error)]
pub enum MinecraftAuthError {
    #[error("Minecraft access token is invalid")]
    InvalidToken,
    #[error("Minecraft authentication request failed")]
    Request(#[source] reqwest::Error),
    #[error("Minecraft authentication service returned {0}")]
    UpstreamStatus(StatusCode),
    #[error("Minecraft authentication service returned an invalid response")]
    InvalidResponse(#[source] reqwest::Error),
    #[error("Minecraft authentication service returned an invalid profile id")]
    InvalidProfileId(String),
}

#[cfg(test)]
mod tests {
    use axum::{Json, Router, http::HeaderMap, routing::get};
    use serde_json::json;
    use tokio::net::TcpListener;

    use super::*;

    #[tokio::test]
    async fn verifies_profile_and_sends_bearer_token() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let app = Router::new().route(
            "/profile",
            get(|headers: HeaderMap| async move {
                assert_eq!(
                    headers.get("authorization").unwrap(),
                    "Bearer minecraft-token"
                );
                Json(json!({
                    "id": "069a79f444e94726a5befca90e38aaf5",
                    "name": "Notch"
                }))
            }),
        );
        let server = tokio::spawn(async move { axum::serve(listener, app).await });
        let client = MinecraftAuthClient::new(
            Client::new(),
            format!("http://{address}/profile").parse().unwrap(),
        );

        let identity = client
            .verify(&SecretString::from("minecraft-token".to_owned()))
            .await
            .unwrap();
        assert_eq!(
            identity.profile_id,
            Uuid::parse_str("069a79f444e94726a5befca90e38aaf5").unwrap()
        );
        assert_eq!(identity.name, "Notch");
        server.abort();
    }

    #[tokio::test]
    async fn maps_client_error_to_invalid_token() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let app = Router::new().route("/profile", get(|| async { StatusCode::UNAUTHORIZED }));
        let server = tokio::spawn(async move { axum::serve(listener, app).await });
        let client = MinecraftAuthClient::new(
            Client::new(),
            format!("http://{address}/profile").parse().unwrap(),
        );

        let result = client
            .verify(&SecretString::from("invalid".to_owned()))
            .await;
        assert!(matches!(result, Err(MinecraftAuthError::InvalidToken)));
        server.abort();
    }
}
