use axum::{
    Json,
    extract::State,
    http::{HeaderMap, HeaderValue, header},
    response::{IntoResponse, Response},
};
use serde::Deserialize;

use crate::{config::TermsConfig, state::AppState};

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TermsRequest {
    language: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Language {
    En,
    Zh,
}

impl Language {
    fn code(self) -> &'static str {
        match self {
            Self::En => "en",
            Self::Zh => "zh",
        }
    }

    fn terms(self, terms: &TermsConfig) -> &str {
        match self {
            Self::En => &terms.en,
            Self::Zh => &terms.zh,
        }
    }
}

pub async fn get(State(state): State<AppState>, headers: HeaderMap) -> Response {
    terms_response(select_language(None, &headers), &state.config.terms)
}

pub async fn post(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Option<Json<TermsRequest>>,
) -> Response {
    let requested = body.and_then(|Json(body)| body.language);
    terms_response(
        select_language(requested.as_deref(), &headers),
        &state.config.terms,
    )
}

fn terms_response(language: Language, terms: &TermsConfig) -> Response {
    let mut response = language.terms(terms).to_owned().into_response();
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/plain; charset=utf-8"),
    );
    response.headers_mut().insert(
        header::CONTENT_LANGUAGE,
        HeaderValue::from_static(language.code()),
    );
    response
}

fn select_language(requested: Option<&str>, headers: &HeaderMap) -> Language {
    if let Some(requested) = requested {
        return parse_language_tag(requested).unwrap_or(Language::En);
    }

    headers
        .get(header::ACCEPT_LANGUAGE)
        .and_then(|value| value.to_str().ok())
        .and_then(preferred_supported_language)
        .unwrap_or(Language::En)
}

fn parse_language_tag(value: &str) -> Option<Language> {
    let primary = value.trim().split(['-', '_']).next().unwrap_or_default();
    if primary.eq_ignore_ascii_case("zh") {
        Some(Language::Zh)
    } else if primary.eq_ignore_ascii_case("en") {
        Some(Language::En)
    } else {
        None
    }
}

fn preferred_supported_language(value: &str) -> Option<Language> {
    value
        .split(',')
        .filter_map(|entry| {
            let mut parts = entry.trim().split(';');
            let language = parse_language_tag(parts.next()?)?;
            let quality = parts
                .find_map(|part| part.trim().strip_prefix("q="))
                .and_then(|value| value.parse::<f32>().ok())
                .unwrap_or(1.0);
            (quality > 0.0).then_some((language, quality))
        })
        .max_by(|left, right| left.1.total_cmp(&right.1))
        .map(|(language, _)| language)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_to_english() {
        assert_eq!(select_language(None, &HeaderMap::new()), Language::En);
    }

    #[test]
    fn accepts_regional_chinese_header() {
        let mut headers = HeaderMap::new();
        headers.insert(header::ACCEPT_LANGUAGE, HeaderValue::from_static("zh-CN"));
        assert_eq!(select_language(None, &headers), Language::Zh);
    }

    #[test]
    fn honors_quality_values() {
        assert_eq!(
            preferred_supported_language("zh-CN;q=0.7, en-US;q=0.9"),
            Some(Language::En)
        );
    }

    #[test]
    fn body_language_overrides_header() {
        let mut headers = HeaderMap::new();
        headers.insert(header::ACCEPT_LANGUAGE, HeaderValue::from_static("zh-CN"));
        assert_eq!(select_language(Some("en-US"), &headers), Language::En);
    }

    #[tokio::test]
    async fn response_returns_selected_plain_text_language() {
        let mut headers = HeaderMap::new();
        headers.insert(header::ACCEPT_LANGUAGE, HeaderValue::from_static("en"));
        let terms = test_terms();
        let response = terms_response(select_language(Some("zh-CN"), &headers), &terms);

        assert_eq!(response.headers()[header::CONTENT_LANGUAGE], "zh");
        assert_eq!(
            response.headers()[header::CONTENT_TYPE],
            "text/plain; charset=utf-8"
        );
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        assert!(
            String::from_utf8(body.to_vec())
                .unwrap()
                .starts_with("NetherLink 服务条款")
        );
    }

    fn test_terms() -> TermsConfig {
        TermsConfig {
            en: "NetherLink Service Terms\n\nEnglish test terms.".to_owned(),
            zh: "NetherLink 服务条款\n\n中文测试条款。".to_owned(),
        }
    }
}
