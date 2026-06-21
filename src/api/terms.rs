use axum::{
    Json,
    http::{HeaderMap, HeaderValue, header},
    response::{IntoResponse, Response},
};
use serde::Deserialize;

const EN_TERMS: &str = r#"NetherLink Service Terms (Version 1)

Effective date: June 21, 2026

1. Service
NetherLink provides account identity verification, friend relationships, online presence, WebRTC signaling, and temporary TURN credentials for Minecraft Java peer-to-peer multiplayer. It does not provide Minecraft accounts, game licenses, worlds, or game content.

2. Acceptance and eligibility
By using the service, you agree to these terms. You must have the right to use the Minecraft account and access token submitted to the service, comply with applicable law, and comply with the terms of Minecraft and any relevant third-party service.

3. Information used by the service
When a runtime instance is registered, the service sends the supplied Minecraft access token to the official Minecraft profile service to verify the account. Friend-list reads and friend changes also require the current token so the service can verify the caller and access the official Minecraft friends service. The token is discarded after each request and is not persisted.

The service may persist Minecraft profile UUIDs, cached profile names, a synchronized projection of official friendships and friend requests, their sources, and timestamps. Runtime instance tokens, presence entries, rate-limit counters, and signaling-session state are temporary data stored for service operation. Presence information may include profile and presence identifiers, status, display text, joinability, and update or expiry times, and is shared with accepted friends.

WebRTC offers, answers, and ICE candidates are relayed to the selected friend while a signaling session is active. They are not intended for persistent storage or application logs. TURN credentials are short-lived. A TURN operator necessarily processes network addresses, connection metadata, and relayed traffic required to provide the relay.

Operational logs and aggregate metrics may be retained for security, reliability, and abuse prevention. Minecraft access tokens, instance tokens, TURN credentials, and full signaling payloads must not be recorded in those logs.

4. Acceptable use
You may not impersonate another user, submit credentials without authorization, probe or disrupt the service, evade limits, automate abusive traffic, attack other users, or use the service to violate law or third-party rights. Access may be limited or terminated to protect users and infrastructure.

5. Peer-to-peer connections
Joining a world may reveal network addresses to peers or TURN infrastructure. World owners control their worlds and are responsible for access decisions, content, backups, and player conduct. NetherLink cannot guarantee peer identity beyond the Minecraft profile verification performed at instance registration.

6. Availability and security
The service is provided on an as-available basis. Features may change, fail, or be suspended, and uninterrupted operation or successful peer-to-peer connectivity is not guaranteed. Keep issued instance tokens private and close instances you no longer use. Report suspected credential exposure promptly.

7. Third-party services
Minecraft, Microsoft identity services, network providers, and independently operated TURN nodes are third-party services governed by their own terms and policies. NetherLink is not affiliated with or endorsed by Mojang Studios or Microsoft.

8. Disclaimer and liability
To the maximum extent permitted by law, the service is provided without warranties, and the operator is not liable for indirect, incidental, special, consequential, or game-data losses arising from use, interruption, peer conduct, or third-party services. Rights that cannot lawfully be excluded remain unaffected.

9. Changes and contact
These terms may be updated when the service or legal requirements change. The current text and effective date are returned by this endpoint. Questions or reports may be sent through https://muyucloud.cool/ or https://github.com/MUYUTwilighter.
"#;

const ZH_TERMS: &str = r#"NetherLink 服务条款（版本 1）

生效日期：2026 年 6 月 21 日

1. 服务内容
NetherLink 为 Minecraft Java 点对点联机提供账号身份验证、好友关系、在线状态、WebRTC 信令转发和临时 TURN 凭据。服务不提供 Minecraft 账号、游戏许可、世界存档或游戏内容。

2. 接受条款与使用资格
使用本服务即表示你同意本条款。你必须有权使用提交给服务的 Minecraft 账号及访问令牌，遵守适用法律，并遵守 Minecraft 及相关第三方服务的条款。

3. 服务使用的信息
注册运行实例时，服务会将提交的 Minecraft 访问令牌发送至 Minecraft 官方档案服务，以验证账号身份。读取或变更好友时也需要提交当前令牌，以便服务验证调用者并访问 Minecraft 官方好友服务。令牌会在每次请求结束后丢弃，不会被持久化。

服务可能持久化 Minecraft 档案 UUID、缓存的档案名称、官方好友关系及好友申请的同步副本、数据来源和时间戳。运行实例令牌、Presence 在线状态、频率限制计数器和信令会话状态是为服务运行而保存的临时数据。Presence 可能包含档案与 Presence 标识符、状态、显示文本、是否允许加入以及更新或过期时间，并会提供给已接受的好友。

WebRTC offer、answer 和 ICE candidate 仅在信令会话有效期间转发给选定好友，不应被持久化或写入应用日志。TURN 凭据为短期凭据。TURN 运营方为提供中继服务，必然会处理网络地址、连接元数据及需要中继的流量。

为保障安全性、可靠性和防止滥用，服务可能保留运行日志和汇总指标。Minecraft 访问令牌、实例令牌、TURN 凭据及完整信令内容不得写入这些日志。

4. 合理使用
你不得冒充其他用户、未经授权提交凭据、探测或破坏服务、规避限制、自动产生滥用流量、攻击其他用户，或利用本服务违反法律及第三方权利。为保护用户和基础设施，服务可能限制或终止相关访问。

5. 点对点连接
加入世界可能会向对等用户或 TURN 基础设施公开网络地址。世界所有者负责其世界的访问决定、内容、备份和玩家行为。除注册实例时完成的 Minecraft 档案验证外，NetherLink 不保证对等方的其他身份信息。

6. 可用性与安全
服务按现状和可用状态提供。功能可能变更、故障或暂停，不保证持续运行或一定能建立点对点连接。请妥善保管实例令牌，及时关闭不再使用的实例，并尽快报告疑似凭据泄露事件。

7. 第三方服务
Minecraft、Microsoft 身份服务、网络提供商及独立运营的 TURN 节点均属于第三方服务，适用各自的条款和政策。NetherLink 与 Mojang Studios 或 Microsoft 不存在隶属或官方认可关系。

8. 免责声明与责任限制
在法律允许的最大范围内，本服务不提供任何保证；对于因服务使用或中断、对等用户行为、第三方服务导致的间接、附带、特殊、后果性损失或游戏数据损失，运营方不承担责任。法律规定不得排除的权利不受影响。

9. 条款变更与联系
服务内容或法律要求变化时，本条款可能更新。此接口返回当前条款文本及生效日期。如有问题或需要报告事件，可通过 https://muyucloud.cool/ 或 https://github.com/MUYUTwilighter 联系。
"#;

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

    fn terms(self) -> &'static str {
        match self {
            Self::En => EN_TERMS,
            Self::Zh => ZH_TERMS,
        }
    }
}

pub async fn get(headers: HeaderMap) -> Response {
    terms_response(select_language(None, &headers))
}

pub async fn post(headers: HeaderMap, body: Option<Json<TermsRequest>>) -> Response {
    let requested = body.and_then(|Json(body)| body.language);
    terms_response(select_language(requested.as_deref(), &headers))
}

fn terms_response(language: Language) -> Response {
    let mut response = language.terms().into_response();
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
    async fn post_returns_selected_plain_text_language() {
        let mut headers = HeaderMap::new();
        headers.insert(header::ACCEPT_LANGUAGE, HeaderValue::from_static("en"));
        let response = post(
            headers,
            Some(Json(TermsRequest {
                language: Some("zh-CN".to_owned()),
            })),
        )
        .await;

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
}
