mod minecraft;

use axum::http::{HeaderMap, header::AUTHORIZATION};
use secrecy::SecretString;
use thiserror::Error;

pub use minecraft::{
    MinecraftAuthClient, MinecraftAuthError, MinecraftProfileClient, MinecraftProfileError,
    MinecraftSocialClient, MinecraftSocialError, OfficialFriend, OfficialFriendSnapshot,
    ProfileIdentity,
};

pub fn bearer_token(headers: &HeaderMap) -> Result<SecretString, BearerTokenError> {
    let value = headers
        .get(AUTHORIZATION)
        .ok_or(BearerTokenError::Missing)?
        .to_str()
        .map_err(|_| BearerTokenError::Malformed)?;
    let (scheme, token) = value.split_once(' ').ok_or(BearerTokenError::Malformed)?;

    if !scheme.eq_ignore_ascii_case("Bearer")
        || token.is_empty()
        || token.contains(char::is_whitespace)
    {
        return Err(BearerTokenError::Malformed);
    }

    Ok(SecretString::from(token.to_owned()))
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum BearerTokenError {
    #[error("authorization header is required")]
    Missing,
    #[error("authorization header must use the Bearer scheme")]
    Malformed,
}

#[cfg(test)]
mod tests {
    use axum::http::{HeaderValue, header::AUTHORIZATION};
    use secrecy::ExposeSecret;

    use super::*;

    #[test]
    fn extracts_bearer_token() {
        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_static("Bearer secret-token"),
        );

        let token = bearer_token(&headers).unwrap();
        assert_eq!(token.expose_secret(), "secret-token");
    }

    #[test]
    fn accepts_case_insensitive_bearer_scheme() {
        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_static("bearer secret-token"),
        );
        assert!(bearer_token(&headers).is_ok());
    }

    #[test]
    fn rejects_missing_or_malformed_authorization() {
        assert!(matches!(
            bearer_token(&HeaderMap::new()),
            Err(BearerTokenError::Missing)
        ));

        for value in ["Basic token", "Bearer", "Bearer ", "Bearer two tokens"] {
            let mut headers = HeaderMap::new();
            headers.insert(AUTHORIZATION, HeaderValue::from_str(value).unwrap());
            assert!(
                matches!(bearer_token(&headers), Err(BearerTokenError::Malformed)),
                "value: {value}"
            );
        }
    }
}
