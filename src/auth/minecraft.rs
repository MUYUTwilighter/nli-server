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

#[derive(Clone)]
pub struct MinecraftProfileClient {
    http: Client,
    profile_by_name_base_url: Url,
    profile_by_id_base_url: Url,
}

#[derive(Clone)]
pub struct MinecraftSocialClient {
    http: Client,
    friends_url: Url,
}

impl MinecraftProfileClient {
    pub fn new(http: Client, profile_by_name_base_url: Url, profile_by_id_base_url: Url) -> Self {
        Self {
            http,
            profile_by_name_base_url,
            profile_by_id_base_url,
        }
    }

    pub async fn lookup_by_name(
        &self,
        name: &str,
    ) -> Result<ProfileIdentity, MinecraftProfileError> {
        let url = self
            .profile_by_name_base_url
            .join(name)
            .map_err(|error| MinecraftProfileError::InvalidLookupUrl(error.to_string()))?;
        self.lookup(url).await
    }

    pub async fn lookup_by_id(
        &self,
        profile_id: Uuid,
    ) -> Result<ProfileIdentity, MinecraftProfileError> {
        let url = self
            .profile_by_id_base_url
            .join(&profile_id.simple().to_string())
            .map_err(|error| MinecraftProfileError::InvalidLookupUrl(error.to_string()))?;
        self.lookup(url).await
    }

    async fn lookup(&self, url: Url) -> Result<ProfileIdentity, MinecraftProfileError> {
        let response = self
            .http
            .get(url)
            .send()
            .await
            .map_err(MinecraftProfileError::Request)?;
        match response.status() {
            StatusCode::OK => {}
            StatusCode::NO_CONTENT | StatusCode::NOT_FOUND => {
                return Err(MinecraftProfileError::NotFound);
            }
            status => return Err(MinecraftProfileError::UpstreamStatus(status)),
        }

        let profile = response
            .json::<MinecraftProfileResponse>()
            .await
            .map_err(MinecraftProfileError::InvalidResponse)?;
        let profile_id = Uuid::parse_str(&profile.id)
            .map_err(|_| MinecraftProfileError::InvalidProfileId(profile.id))?;
        Ok(ProfileIdentity {
            profile_id,
            name: profile.name,
        })
    }
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

impl MinecraftSocialClient {
    pub fn new(http: Client, friends_url: Url) -> Self {
        Self { http, friends_url }
    }

    pub async fn friends(
        &self,
        access_token: &SecretString,
    ) -> Result<OfficialFriendSnapshot, MinecraftSocialError> {
        let response = self
            .http
            .get(self.friends_url.clone())
            .bearer_auth(access_token.expose_secret())
            .send()
            .await
            .map_err(MinecraftSocialError::Request)?;
        match response.status() {
            status if status.is_success() => response
                .json::<OfficialFriendSnapshot>()
                .await
                .map_err(MinecraftSocialError::InvalidResponse),
            StatusCode::UNAUTHORIZED => Err(MinecraftSocialError::InvalidToken),
            StatusCode::FORBIDDEN => Err(MinecraftSocialError::Forbidden),
            StatusCode::TOO_MANY_REQUESTS => Err(MinecraftSocialError::RateLimited),
            status => Err(MinecraftSocialError::UpstreamStatus(status)),
        }
    }

    pub async fn add_friend_by_name(
        &self,
        access_token: &SecretString,
        name: &str,
    ) -> Result<OfficialFriendSnapshot, MinecraftSocialError> {
        self.update_friends(access_token, OfficialFriendUpdate::by_name(name, "ADD"))
            .await
    }

    pub async fn add_friend_by_id(
        &self,
        access_token: &SecretString,
        profile_id: Uuid,
    ) -> Result<OfficialFriendSnapshot, MinecraftSocialError> {
        self.update_friends(access_token, OfficialFriendUpdate::by_id(profile_id, "ADD"))
            .await
    }

    pub async fn remove_friend_by_id(
        &self,
        access_token: &SecretString,
        profile_id: Uuid,
    ) -> Result<OfficialFriendSnapshot, MinecraftSocialError> {
        self.update_friends(
            access_token,
            OfficialFriendUpdate::by_id(profile_id, "REMOVE"),
        )
        .await
    }

    async fn update_friends(
        &self,
        access_token: &SecretString,
        update: OfficialFriendUpdate<'_>,
    ) -> Result<OfficialFriendSnapshot, MinecraftSocialError> {
        let response = self
            .http
            .put(self.friends_url.clone())
            .bearer_auth(access_token.expose_secret())
            .json(&update)
            .send()
            .await
            .map_err(MinecraftSocialError::Request)?;
        match response.status() {
            status if status.is_success() => response
                .json::<OfficialFriendSnapshot>()
                .await
                .map_err(MinecraftSocialError::InvalidResponse),
            StatusCode::UNAUTHORIZED => Err(MinecraftSocialError::InvalidToken),
            StatusCode::BAD_REQUEST | StatusCode::NOT_FOUND => {
                Err(MinecraftSocialError::UnknownProfile)
            }
            StatusCode::FORBIDDEN => Err(MinecraftSocialError::Forbidden),
            StatusCode::TOO_MANY_REQUESTS => Err(MinecraftSocialError::RateLimited),
            status => Err(MinecraftSocialError::UpstreamStatus(status)),
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OfficialFriendSnapshot {
    #[serde(default)]
    pub friends: Vec<OfficialFriend>,
    #[serde(default)]
    pub incoming_requests: Vec<OfficialFriend>,
    #[serde(default)]
    pub outgoing_requests: Vec<OfficialFriend>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OfficialFriend {
    pub profile_id: Uuid,
    pub name: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct OfficialFriendUpdate<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    profile_id: Option<Uuid>,
    update_type: &'static str,
}

impl<'a> OfficialFriendUpdate<'a> {
    fn by_name(name: &'a str, update_type: &'static str) -> Self {
        Self {
            name: Some(name),
            profile_id: None,
            update_type,
        }
    }

    fn by_id(profile_id: Uuid, update_type: &'static str) -> Self {
        Self {
            name: None,
            profile_id: Some(profile_id),
            update_type,
        }
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

#[derive(Debug, Error)]
pub enum MinecraftProfileError {
    #[error("Minecraft player was not found")]
    NotFound,
    #[error("Minecraft profile lookup URL is invalid")]
    InvalidLookupUrl(String),
    #[error("Minecraft profile lookup request failed")]
    Request(#[source] reqwest::Error),
    #[error("Minecraft profile service returned {0}")]
    UpstreamStatus(StatusCode),
    #[error("Minecraft profile service returned an invalid response")]
    InvalidResponse(#[source] reqwest::Error),
    #[error("Minecraft profile service returned an invalid profile id")]
    InvalidProfileId(String),
}

#[derive(Debug, Error)]
pub enum MinecraftSocialError {
    #[error("Minecraft access token is invalid")]
    InvalidToken,
    #[error("Minecraft profile is unknown")]
    UnknownProfile,
    #[error("Minecraft friends operation is forbidden")]
    Forbidden,
    #[error("Minecraft friends operation was rate limited")]
    RateLimited,
    #[error("Minecraft social request failed")]
    Request(#[source] reqwest::Error),
    #[error("Minecraft social service returned {0}")]
    UpstreamStatus(StatusCode),
    #[error("Minecraft social service returned an invalid response")]
    InvalidResponse(#[source] reqwest::Error),
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

    #[tokio::test]
    async fn reads_and_updates_official_friends() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let friend_id = Uuid::new_v4();
        let expected_id = friend_id;
        let app = Router::new().route(
            "/friends",
            get(move |headers: HeaderMap| async move {
                assert_eq!(
                    headers.get("authorization").unwrap(),
                    "Bearer minecraft-token"
                );
                Json(json!({
                    "friends": [{ "profileId": expected_id, "name": "Friend" }],
                    "incomingRequests": [],
                    "outgoingRequests": []
                }))
            })
            .put(
                move |headers: HeaderMap, Json(body): Json<serde_json::Value>| async move {
                    assert_eq!(
                        headers.get("authorization").unwrap(),
                        "Bearer minecraft-token"
                    );
                    assert!(body.get("name").is_some() || body.get("profileId").is_some());
                    assert!(body["updateType"] == "ADD" || body["updateType"] == "REMOVE");
                    Json(json!({
                        "friends": [{ "profileId": friend_id, "name": "Friend" }],
                        "incomingRequests": [],
                        "outgoingRequests": []
                    }))
                },
            ),
        );
        let server = tokio::spawn(async move { axum::serve(listener, app).await });
        let client = MinecraftSocialClient::new(
            Client::new(),
            format!("http://{address}/friends").parse().unwrap(),
        );
        let token = SecretString::from("minecraft-token".to_owned());
        let snapshot = client.friends(&token).await.unwrap();
        assert_eq!(snapshot.friends[0].profile_id, friend_id);
        client.add_friend_by_name(&token, "Friend").await.unwrap();
        client.add_friend_by_id(&token, friend_id).await.unwrap();
        client.remove_friend_by_id(&token, friend_id).await.unwrap();
        server.abort();
    }
}
