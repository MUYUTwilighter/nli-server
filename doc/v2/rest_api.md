# NetherLink v2 REST API 契约

> 状态：`PHASE_6_FROZEN + GATE_F_FROZEN_EXTENSION`
>
> 基线：`common.md`、`nli_account.md`、`provider.md`、`friendship.md`、`game_instance.md`、`notifications.md`；Phase 9 扩展依赖 `signaling.md`。
>
> 本文档冻结客户端可见的 HTTP Path、Principal、请求/响应 DTO、状态码、幂等与并发语义、限流及通知映射。数据库表、索引和内部服务拆分不属于 API 契约。

## 目标

- 为全部 v2 用户端 HTTP 能力给出稳定 Path 和 Method；
- 让每个端点明确可用 Principal 和资源授权；
- 拒绝客户端提交服务端推导字段；
- 为所有写入定义重试、幂等和并发冲突行为；
- 将领域错误映射到 RFC 9457 Problem Details；
- 为列表冻结 Cursor、排序和可见性；
- 把业务事务映射到 Phase 5 通知事件；
- 为 OpenAPI 和后续数据模型提供唯一契约基线。

## 非目标

- 不设计数据库表或 ORM Model；
- 不定义管理员/运营后台 API；
- 不暴露 Provider Adapter 或 Credential Manager；
- 不通过 REST 传输 SDP、ICE Candidate 或实际游戏连接数据；Phase 9 唯一例外是受限 ICE Server 发现与 TURN Credential 签发，实际 TURN/STUN 数据不经过业务 REST；
- 不重新打开 Gate A～D 已冻结的领域语义。

## 公共约定

所有端点继承 `common.md`：

- Base Path `/v2`；
- JSON 字段 `snake_case`，枚举 `UPPER_SNAKE_CASE`；
- UUIDv4 公共资源 ID；
- Unix 毫秒时间；
- 单资源直接响应，列表 `{items,next_cursor}`；
- RFC 9457 `application/problem+json`；
- 严格拒绝未知请求字段；
- `Authorization: Bearer`；
- 写请求按端点要求使用 `Idempotency-Key`；
- Keyset Cursor 默认 20、最大 100；
- `429` 使用 `Retry-After`；
- HTTP/数据库状态是权威，WebSocket 只提示变化。

## Principal

```text
PUBLIC
NLI_ACCOUNT
NLI_INSTANCE
NLI_GUEST
```

- `PUBLIC`：不持有 Token；只能调用明确列出的注册、登录、邮件凭据和公开 Provider Registry 入口；Account ID/username 定位仍要求 NLI Session；
- `NLI_ACCOUNT`：`nli_account` Audience 且 Token Family 未绑定 Instance；
- `NLI_INSTANCE`：`nli_account` Audience 且 Token Family 为 `ACTIVE_BOUND`；普通业务权限与 Account Session 相同，额外可以管理其绑定 Instance；
- `NLI_GUEST`：`nli_guest` Audience，只能访问 Token 绑定的 Instance、Invite 和最多一个 Join Request 流程；
- 未来 `nli_admin` 不属于本文。

若端点写 `NLI_ACCOUNT_OR_INSTANCE`，表示两类 `nli_account` Session 权限相同。资源 Owner、绑定 Family、好友关系、ACL、Source 等仍由服务端推导和复核。

## 端点总表

### Auth 与 Account

| Method | Path | Principal | 语义 |
| --- | --- | --- | --- |
| POST | `/v2/auth/register` | PUBLIC | 邮箱密码注册并创建 PENDING_EMAIL Account |
| POST | `/v2/auth/registration-cancellations` | PUBLIC | 以注册邮箱和密码取消 PENDING_EMAIL 注册 |
| POST | `/v2/auth/email-verification-requests` | PUBLIC | 发送/重发邮箱验证邮件，防枚举 |
| POST | `/v2/auth/email/verify` | PUBLIC + Email Secret | 消费一次性验证凭据并激活账号 |
| POST | `/v2/auth/login` | PUBLIC | 使用已验证邮箱和密码登录 |
| POST | `/v2/auth/refresh` | PUBLIC + Refresh Token | 轮换 Access/Refresh Token |
| POST | `/v2/auth/password-reset-requests` | PUBLIC | 发送密码重置邮件，防枚举 |
| POST | `/v2/auth/password/reset` | PUBLIC + Reset Secret | 消费重置凭据并设置新密码 |
| POST | `/v2/auth/deletion-recovery-requests` | PUBLIC | 发送删除恢复邮件，防枚举 |
| POST | `/v2/auth/deletion/recover` | PUBLIC + Recovery Secret | 恢复 DELETION_PENDING 并设置新密码 |
| POST | `/v2/auth/device/code` | PUBLIC | RFC 8628 Mod 创建 Device Code |
| POST | `/v2/auth/device/token` | PUBLIC + Device Code | RFC 8628 Mod 轮询并消费批准结果 |
| POST | `/v2/auth/device/authorization-lookups` | NLI_ACCOUNT_OR_INSTANCE | 浏览器以 User Code 查询待授权信息 |
| POST | `/v2/auth/device/authorizations/{authorization_id}/approve` | NLI_ACCOUNT_OR_INSTANCE | 显式批准 Device Code |
| POST | `/v2/auth/device/authorizations/{authorization_id}/deny` | NLI_ACCOUNT_OR_INSTANCE | 显式拒绝 Device Code |
| POST | `/v2/auth/provider/{provider_id}/start` | PUBLIC 或 NLI_ACCOUNT_OR_INSTANCE | 开始 LOGIN/REGISTER/LINK/REAUTH Provider 浏览器事务 |
| GET | `/v2/auth/provider/callback` | PUBLIC + Browser State | Provider 登录统一回调，不在 URL 返回 NLI Token |
| POST | `/v2/auth/provider/complete` | PUBLIC + Browser Transaction Cookie | 原子取得登录结果、注册步骤或完成 REAUTH |
| POST | `/v2/auth/provider/register` | PUBLIC + Browser Transaction Cookie | 显式完成未关联 Provider Subject 的账号注册 |
| POST | `/v2/auth/reauthenticate` | NLI_ACCOUNT_OR_INSTANCE | 以当前密码记录 5 分钟近期认证 |
| POST | `/v2/auth/email-reauthentication-requests` | NLI_ACCOUNT_OR_INSTANCE | 发送当前 Family 绑定的邮件 reauth 链接 |
| POST | `/v2/auth/email/reauthenticate` | NLI_ACCOUNT_OR_INSTANCE + Email Secret | 完成当前 Family 的 5 分钟近期认证 |
| GET | `/v2/provider-login-identities` | NLI_ACCOUNT_OR_INSTANCE | 列出当前账号 Provider 登录身份 |
| PUT | `/v2/provider-login-identities/{identity_id}` | NLI_ACCOUNT_OR_INSTANCE | 按 Revision 启用/停用 Provider 登录 |
| DELETE | `/v2/provider-login-identities/{identity_id}` | NLI_ACCOUNT_OR_INSTANCE + Recent Auth | 删除登录映射但不删除业务 Binding |
| GET | `/v2/accounts/me` | NLI_ACCOUNT_OR_INSTANCE | 当前账号私有摘要 |
| PUT | `/v2/accounts/me/profile` | NLI_ACCOUNT_OR_INSTANCE | 替换 display name 等可编辑公开字段 |
| POST | `/v2/accounts/me/username-changes` | NLI_ACCOUNT_OR_INSTANCE | 修改唯一 username |
| POST | `/v2/accounts/me/password-changes` | NLI_ACCOUNT_OR_INSTANCE + Recent Auth | 修改/设置密码 |
| POST | `/v2/accounts/me/email-change-requests` | NLI_ACCOUNT_OR_INSTANCE + Recent Auth | 发起新邮箱验证 |
| POST | `/v2/auth/email/change` | PUBLIC + Email Change Secret | 消费一次性凭据并切换邮箱 |
| POST | `/v2/accounts/me/deletion` | NLI_ACCOUNT_OR_INSTANCE + Recent Auth | 进入 DELETION_PENDING |
| GET | `/v2/accounts/{account_id}` | NLI_ACCOUNT_OR_INSTANCE | 按可见性读取公开账号 |
| GET | `/v2/accounts/by-username/{username}` | NLI_ACCOUNT_OR_INSTANCE | 精确 username 定位公开账号 |
| GET | `/v2/sessions` | NLI_ACCOUNT_OR_INSTANCE | 列出当前账号 Token Family |
| DELETE | `/v2/sessions/{session_id}` | NLI_ACCOUNT_OR_INSTANCE | 撤销一个 Family；可撤销当前 Family |
| POST | `/v2/sessions/revoke-others` | NLI_ACCOUNT_OR_INSTANCE | 幂等撤销当前 Family 以外的 Session |

### Provider

| Method | Path | Principal | 语义 |
| --- | --- | --- | --- |
| GET | `/v2/providers` | PUBLIC | 列出可公开 Provider Registry 元数据 |
| GET | `/v2/providers/{provider_id}` | PUBLIC | 读取一个 Provider 与 Capability |
| GET | `/v2/provider-bindings` | NLI_ACCOUNT_OR_INSTANCE | 列出当前账号 Binding |
| POST | `/v2/provider-bindings/{provider_id}/authorizations` | NLI_ACCOUNT_OR_INSTANCE | 开始 CREATE/REAUTH/REPLACE 浏览器事务 |
| GET | `/v2/provider-bindings/authorization-callback` | PUBLIC + Browser State | Provider Binding 浏览器回调 |
| POST | `/v2/provider-bindings/authorization-completions` | NLI_ACCOUNT_OR_INSTANCE + Browser Cookie | 原子完成 Binding 授权事务 |
| PUT | `/v2/provider-bindings/{binding_id}` | NLI_ACCOUNT_OR_INSTANCE | 按 Revision 完整替换用途开关 |
| DELETE | `/v2/provider-bindings/{binding_id}` | NLI_ACCOUNT_OR_INSTANCE | 本地优先解绑 |

### Friendship 与同步

| Method | Path | Principal | 语义 |
| --- | --- | --- | --- |
| GET | `/v2/friends` | NLI_ACCOUNT_OR_INSTANCE | 好友聚合读模型 |
| GET | `/v2/friend-requests/incoming` | NLI_ACCOUNT_OR_INSTANCE | 可见 Incoming Request |
| GET | `/v2/friend-requests/outgoing` | NLI_ACCOUNT_OR_INSTANCE | 可见 Outgoing Request |
| POST | `/v2/friend-requests` | NLI_ACCOUNT_OR_INSTANCE | 创建申请或接受反向申请 |
| POST | `/v2/friend-requests/{request_id}/accept` | NLI_ACCOUNT_OR_INSTANCE | 接受当前 pending |
| POST | `/v2/friend-requests/{request_id}/reject` | NLI_ACCOUNT_OR_INSTANCE | 拒绝当前 pending |
| POST | `/v2/friend-requests/{request_id}/cancel` | NLI_ACCOUNT_OR_INSTANCE | 发起者取消当前 pending |
| DELETE | `/v2/friends/{account_id}` | NLI_ACCOUNT_OR_INSTANCE | 删除好友关系 |
| GET | `/v2/blocks` | NLI_ACCOUNT_OR_INSTANCE | 仅列出自己设置的有向 ban |
| PUT | `/v2/blocks/{account_id}` | NLI_ACCOUNT_OR_INSTANCE | 设置有向 ban |
| DELETE | `/v2/blocks/{account_id}` | NLI_ACCOUNT_OR_INSTANCE | 显式清除自己的 ban |
| POST | `/v2/friend-sync-tasks` | NLI_ACCOUNT_OR_INSTANCE | 创建 ONE/ALL 异步同步 Task |
| GET | `/v2/friend-sync-tasks` | NLI_ACCOUNT_OR_INSTANCE | 列出近期 Task |
| GET | `/v2/friend-sync-tasks/{task_id}` | NLI_ACCOUNT_OR_INSTANCE | 获取 Task 状态与安全统计 |
| POST | `/v2/friend-sync-tasks/{task_id}/cancel` | NLI_ACCOUNT_OR_INSTANCE | 请求取消 Task |

### Instance、ACL、Proxy 与 Invite

| Method | Path | Principal | 语义 |
| --- | --- | --- | --- |
| POST | `/v2/instances` | 未绑定 NLI_ACCOUNT | 创建 Instance 并将当前 Family 转为 ACTIVE_BOUND |
| GET | `/v2/instances` | NLI_ACCOUNT_OR_INSTANCE | 仅列出当前账号拥有的 Instance |
| GET | `/v2/instances/{instance_id}` | 按 ACL/Owner/Guest | 获取有权可见的 Instance 摘要 |
| PUT | `/v2/instances/{instance_id}` | Bound NLI_INSTANCE | 替换 Owner 可编辑配置 |
| DELETE | `/v2/instances/{instance_id}` | Bound NLI_INSTANCE | 幂等关闭 Instance |
| PUT | `/v2/instances/{instance_id}/acl` | Bound NLI_INSTANCE | 原子替换完整 ACL |
| GET | `/v2/instances/{instance_id}/acl` | Bound NLI_INSTANCE | 获取 ACL 与 Revision |
| GET | `/v2/instances/{instance_id}/join-requests` | Bound NLI_INSTANCE | Target 读取 PENDING/近期请求 |
| GET Upgrade | `/v2/instances/{instance_id}/ws` | Bound NLI_INSTANCE | 建立 Phase 5 通知/保活 WebSocket |
| POST | `/v2/proxy-grants` | NLI_ACCOUNT_OR_INSTANCE + Recent Auth | Issuer 创建代理授权 Secret |
| GET | `/v2/proxy-grants` | NLI_ACCOUNT_OR_INSTANCE | Issuer 列出自己的 Grant |
| POST | `/v2/proxy-grants/{grant_id}/secret-rotations` | NLI_ACCOUNT_OR_INSTANCE + Recent Auth | Issuer 轮换 Secret |
| DELETE | `/v2/proxy-grants/{grant_id}` | NLI_ACCOUNT_OR_INSTANCE + Recent Auth | Issuer 撤销 Grant |
| PUT | `/v2/instances/{instance_id}/proxy-grant` | Bound NLI_INSTANCE + Secret | 绑定/切换 Presented-Under Account |
| DELETE | `/v2/instances/{instance_id}/proxy-grant` | Bound NLI_INSTANCE | 解除当前 Proxy 展示 |
| POST | `/v2/instances/{instance_id}/invites` | Bound NLI_INSTANCE | 创建并单次返回 Invite Secret |
| GET | `/v2/instances/{instance_id}/invites` | Bound NLI_INSTANCE | 列出不含 Secret 的 Invite 元数据 |
| POST | `/v2/instances/{instance_id}/invites/{invite_id}/secret-rotations` | Bound NLI_INSTANCE | 轮换 Invite Secret |
| DELETE | `/v2/instances/{instance_id}/invites/{invite_id}` | Bound NLI_INSTANCE | 幂等撤销 Invite |

### Invite、Guest、Join 与举报

| Method | Path | Principal | 语义 |
| --- | --- | --- | --- |
| POST | `/v2/invite-resolutions` | NLI_INSTANCE | 验证 Invite 并建立 60 秒 Session-bound Resolution |
| POST | `/v2/guest-sessions` | PUBLIC + Invite Secret | 建立 5 分钟单流程 Guest Session |
| POST | `/v2/join-requests` | NLI_INSTANCE 或 NLI_GUEST | 创建 Manual/Auto Join Request |
| GET | `/v2/join-requests` | NLI_INSTANCE | 列当前 Source Instance 的近期 Request |
| GET | `/v2/join-requests/{request_id}` | Target/Requester/绑定 Guest | 获取当前或保留期内结果 |
| POST | `/v2/join-requests/{request_id}/accept` | Target Bound NLI_INSTANCE | Manual Accept |
| POST | `/v2/join-requests/{request_id}/reject` | Target Bound NLI_INSTANCE | Manual Reject |
| POST | `/v2/join-requests/{request_id}/cancel` | Requester NLI_INSTANCE/GUEST | Requester Cancel |
| POST | `/v2/instances/{instance_id}/join-requests/reject-all` | Target Bound NLI_INSTANCE | 幂等批量拒绝 PENDING |
| POST | `/v2/reports` | 参与 Request 的 NLI_INSTANCE 或 NLI_GUEST | 提交服务端关联上下文的举报 Receipt |

### Phase 9 Signaling（Gate F 追加）

| Method | Path | Principal | 语义 |
| --- | --- | --- | --- |
| POST | `/v2/join-requests/{request_id}/signaling-sessions` | 精确 Request 参与方 | create-or-attach 单逻辑 Session |
| GET | `/v2/signaling-sessions/{signaling_session_id}` | 精确 Session 参与方 | HTTP 权威恢复 |
| DELETE | `/v2/signaling-sessions/{signaling_session_id}` | 精确 Session 参与方 | 幂等关闭 |
| GET Upgrade | `/v2/signaling-sessions/{signaling_session_id}/ws` | 精确 Session 参与方的原生客户端 | 独立协议 v1 信令 WS |
| POST | `/v2/signaling-sessions/{signaling_session_id}/ice-servers` | 精确 Session 参与方 | ICE Server / 独立 Relay 授权签发 |

## 批次 1：Auth、Account、Session 与 Device Code（已完成）

已确认：

- Auth 使用常见动作 Path，业务资源保持 REST 风格；
- 注册和邮箱验证不自动签发 Session，激活后显式登录；
- 邮件链接由前端取出 Secret 并通过 JSON Body POST，API 不接受 Query Secret；
- Token 响应返回 Token Pair、Session 摘要和绝对过期时间；
- Device Code Mod 端点使用 RFC 8628 Form 与标准错误；
- Provider Login 使用独立 `/auth/provider/*` 流程，不复用业务 Binding；
- 近期认证在当前 Token Family 保存 5 分钟 `reauthenticated_at`；
- 撤销其他设备使用 `POST /sessions/revoke-others`；
- username 定位使用精确、限流的 `/accounts/by-username/{username}`。

## Auth、Account、Session 与 Device Code 契约

### Token Pair

密码登录、Refresh 和成功的既有账号 Provider Login 返回：

```json
{
  "access_token": "<opaque-secret>",
  "refresh_token": "<opaque-secret>",
  "token_type": "Bearer",
  "access_expires_at": 0,
  "refresh_idle_expires_at": 0,
  "family_expires_at": 0,
  "session": {
    "session_id": "uuid-v4",
    "client_name": "Browser",
    "created_at": 0,
    "last_refreshed_at": 0,
    "absolute_expires_at": 0,
    "bound_instance_id": null,
    "is_current": true
  }
}
```

- 原始 Token 只出现在签发或允许的 60 秒重放响应；
- `token_type` 固定为 `Bearer`；
- `absolute_expires_at` 与 `family_expires_at` 相同；
- Login/Refresh 使用项目 Unix 毫秒绝对时间；
- RFC Device Token 成功响应额外使用标准 `expires_in` 秒字段，并可附带绝对时间与 Session 扩展字段；
- Token Body 和加密重放结果不进入日志、审计或普通幂等记录。

### 邮箱注册

#### `POST /v2/auth/register`

Principal：PUBLIC。必须提供 `Idempotency-Key`。

请求包含 `username`、`display_name`、`email` 和 `password`。成功创建 `PENDING_EMAIL` 后返回 `201 Created`：

```json
{
  "registration_id": "uuid-v4",
  "status": "PENDING_EMAIL",
  "expires_at": 0,
  "verification_email_sent": true
}
```

不返回 Account ID、Session 或 Token Pair。相同 Key/Body 返回相同非敏感结果；不同 Body 返回 `409 IDEMPOTENCY_KEY_REUSED`。

username 冲突可以返回 `409 USERNAME_UNAVAILABLE`，因为 username 是公开定位符。邮箱已使用、被保留或账号状态不允许时统一返回 `409 REGISTRATION_NOT_AVAILABLE`，不确认具体原因。

#### 邮箱验证和取消

`POST /v2/auth/email-verification-requests` 请求只包含 `email`。无论是否存在可重发的 PENDING_EMAIL 注册都返回 `202 Accepted`；它不延长注册的 24 小时总期限。

`POST /v2/auth/email/verify` 请求：

```json
{"verification_token":"<opaque-secret>"}
```

前端从邮件 URL 读取 Secret 后立即清理地址栏，再放入 HTTPS JSON Body。API 不从 Query 接收 Secret。成功激活 Account 并返回 `204`，不签发 Session。同一 Secret 的成功结果可以重放 60 秒；之后无效、过期、已消费和不匹配统一返回 `400 INVALID_OR_EXPIRED_CREDENTIAL`。

`POST /v2/auth/registration-cancellations` 请求包含 `email` 和 `password`。匹配有效 PENDING_EMAIL 时取消并使验证凭据失效；不匹配、已取消或不存在也返回 `204`。端点按邮箱 HMAC 和 IP 严格限流。

### 密码登录与 Refresh

#### `POST /v2/auth/login`

请求包含 `email`、`password` 和 `client_name`，必须提供 `Idempotency-Key`。成功返回 `200` Token Pair；失败统一返回 `401 INVALID_CREDENTIALS`，不能区分邮箱不存在、密码错误或账号不可用。

成功 Token Pair 以受保护方式重放 60 秒。60 秒后相同 Key 不再返回 Secret，返回 `409 IDEMPOTENCY_RESULT_EXPIRED`；非敏感请求摘要和完成标记保留 24 小时。使用新 Key 的真实新登录可以创建另一 Token Family，但仍受 10 Family 上限。

#### `POST /v2/auth/refresh`

请求 Body：

```json
{"refresh_token":"<opaque-secret>"}
```

Refresh Token 不能放入 Authorization Header。必须提供 `Idempotency-Key`。成功返回 `200` 新 Token Pair，旧 Access/Refresh 立即失效；相同旧 Refresh + Key 在 60 秒内返回同一结果。

无效/过期返回 `401 INVALID_REFRESH_TOKEN`。超过重放窗重用已轮换 Refresh 时撤销对应 Family，并返回同一公开错误；不泄漏重用检测内部细节。

### Password Reset、Password Change 与删除恢复

`POST /v2/auth/password-reset-requests` 请求 `{email}`，始终返回 `202`。

`POST /v2/auth/password/reset` 请求 `reset_token` 和 `new_password`。成功返回 `204`，撤销账号全部 Token Family，不自动登录。成功响应可按同一 Secret 重放 60 秒，之后统一 `400 INVALID_OR_EXPIRED_CREDENTIAL`。

`POST /v2/accounts/me/password-changes` 要求当前 Family 的近期认证尚未过期，请求只包含 `new_password`。成功保留当前 Family、撤销其他 Family，返回 `204` 并产生安全通知。

`POST /v2/accounts/me/deletion` 要求近期认证。成功进入 DELETION_PENDING、撤销包括当前 Family 在内的全部 Session，并返回 `204`；响应发送不要求刚撤销的 Token 继续有效。

`POST /v2/auth/deletion-recovery-requests` 请求 `{email}`，始终返回 `202`。`POST /v2/auth/deletion/recover` 请求恢复 Secret 和 `new_password`；成功恢复 ACTIVE，不恢复旧 Session/Instance，返回 `204` 后显式登录。

### 近期重新认证

当前 Token Family 维护：

```text
reauthenticated_at
reauthentication_expires_at = reauthenticated_at + 5 minutes
```

该状态只绑定当前 Family，不跨 Session 共享。每个敏感命令在事务中重新检查截止时间。

- `POST /v2/auth/reauthenticate`：提交当前密码，成功 `204`；
- Provider-only Account：通过 purpose=`REAUTH` 的 Provider 浏览器事务完成；
- `POST /v2/auth/email-reauthentication-requests`：需要当前 Access Token，始终 `202`；
- `POST /v2/auth/email/reauthenticate`：同时要求当前 Access Token 与绑定该 Family/用途的 Email Secret，成功 `204`；
- Email/Provider reauth 不能切换到另一个 Account 或 Family；
- 密码、Provider Subject 或邮箱在流程中变化时，旧 reauth 事务失效。

### Provider Login Identity

`POST /v2/auth/provider/{provider_id}/start` 请求 `purpose = LOGIN / REGISTER / LINK / REAUTH`：

- LOGIN/REGISTER 可以 PUBLIC；LINK/REAUTH 必须携带当前 Access Token，LINK 还要求近期认证；
- 事务固定 Provider、Purpose、Redirect URI、State、PKCE、当前 Family（REAUTH）和 10 分钟期限；
- 返回受 Registry 允许列表约束的 `authorization_url` 和 `expires_at`；
- 客户端不能提交任意回调 URL 或 Provider Scope。

Provider Callback 验证服务端 State 后：

1. 不在 URL、Query、Fragment 或 HTML 中返回 NLI Token；
2. 设置 `Secure + HttpOnly + SameSite=Lax`、Path 最小化、最多 10 分钟的浏览器事务 Cookie；
3. 使用 `303 See Other` 跳转到固定前端完成页；
4. 前端以受信 Origin 调用 `POST /v2/auth/provider/complete`；
5. Complete 原子消费或在 60 秒内安全重放结果。

Complete 结果：

- LOGIN + 已关联 Subject：返回 Token Pair；
- LOGIN + 未关联 Subject：`409 ACCOUNT_LINK_NOT_FOUND`，不能自动注册；
- REAUTH + Subject 与当前 Account 映射一致：更新当前 Family 近期认证并返回 `204`；
- LINK + Subject 尚未属于其他 Account：为当前 Account 创建 Provider Login Identity；
- REGISTER + 未关联 Subject：返回 `200 REGISTRATION_REQUIRED`，不创建账号；
- REGISTER + 已关联 Subject：返回 `409 PROVIDER_IDENTITY_ALREADY_LINKED`。

`POST /v2/auth/provider/register` 使用同一浏览器事务 Cookie，提交 username、display_name 和必要时的 email。可靠 Provider 已验证邮箱可以创建 ACTIVE Account、Provider Login Identity 并返回 Token Pair；否则创建 PENDING_EMAIL、保存待激活 Login Identity、发送 NLI 验证邮件且不签发 Session。Provider Subject、Token 和 Credential 不返回客户端。

`GET /v2/provider-login-identities` 返回 identity_id、Provider 最小元数据、status、revision、created_at 和 last_used_at，不返回 Issuer/Subject。`PUT` 以 `{revision,status}` 在 ACTIVE/DISABLED 间切换；`DELETE` 要求近期认证并终止映射。停用或删除 Login Identity 不修改同 Provider Binding，反之亦然。

### Device Authorization

`POST /v2/auth/device/code` 使用 `application/x-www-form-urlencoded`，只接受 `client_name`。成功 `200` 返回 RFC 8628 字段 `device_code`、`user_code`、`verification_uri`、`verification_uri_complete`、`expires_in=600`、`interval=5`。

`POST /v2/auth/device/authorization-lookups` 使用普通 JSON `{user_code}`。成功返回 Authorization ID、清洗后的 client_name、expires_at 和 PENDING。User Code 不放入 API Path；查询按 IP、Account 和 Code 限流。

批准/拒绝使用 Authorization ID：

```text
POST /v2/auth/device/authorizations/{authorization_id}/approve
POST /v2/auth/device/authorizations/{authorization_id}/deny
```

两者要求显式操作和 `Idempotency-Key`；同方向重试返回 `204`，相反终态或不同账号尝试返回不泄漏细节的 `409 AUTHORIZATION_STATE_CONFLICT`。打开页面或登录不能自动批准。

`POST /v2/auth/device/token` 使用 RFC 8628 Form，提交标准 device_code grant_type 和 Device Code。PENDING、过快、拒绝、过期使用 `authorization_pending / slow_down / access_denied / expired_token`。首次成功消费才创建 Family；成功结果重放 60 秒。Session 上限时保持 APPROVED 并返回可重试扩展错误。

### Account DTO 与更新

公开 Account：

```json
{
  "account_id": "uuid-v4",
  "username": "player_name",
  "display_name": "Player Name"
}
```

只有 ACTIVE 且不存在任一方向 ban 的账号可被其他已登录账号按 ID 或精确 username 获取。不存在、不可见、非 ACTIVE 或被 ban 统一返回 `404 ACCOUNT_NOT_FOUND`。不支持模糊搜索、邮箱或 MC Profile 定位。

`GET /v2/accounts/me` 返回 email、email_verified_at、status、username_change_available_at 和当前 reauthentication_expires_at；不返回密码/哈希、Provider Subject 或其他 Session Token。

`PUT /v2/accounts/me/profile` 当前完整 Schema 只有 `display_name`，成功返回更新后的私有 Account。

`POST /v2/accounts/me/username-changes` 请求 `{username}`。成功返回更新 Account；冷却返回 `409 USERNAME_CHANGE_COOLDOWN` 和 `available_at`，占用返回 `409 USERNAME_UNAVAILABLE`。

`POST /v2/accounts/me/email-change-requests` 要求近期认证，请求 `{new_email}`，成功始终 `202`。`POST /v2/auth/email/change` 通过 JSON Body 消费 Email Change Secret；成功切换邮箱并返回 `204`，不改变 Account ID 或当前 Session。

### Session API

`GET /v2/sessions` 按 `created_at DESC, session_id ASC` 返回最多 10 项，不分页。每项遵循 Phase 1，并额外返回 Family 状态；不返回 IP、Token 哈希或 reauth 状态。

`DELETE /v2/sessions/{session_id}`：

- 只能撤销当前 Account 的 Family；
- 可撤销当前 Family，事务提交后返回 `204`，后续请求立即 `401`；
- 绑定 Instance 的 Family 同时进入 Instance 关闭流程；
- 已撤销但仍能确认归属时重复调用返回 `204`；
- 不可确认归属或其他账号 Session 统一 `404 SESSION_NOT_FOUND`。

`POST /v2/sessions/revoke-others` 从 Access Token 推导当前 Family，幂等撤销其他全部 Family 并返回 `204`。客户端不能提交 keep_session_id。

### Auth 错误和限流

普通 Auth API 使用 Problem Details；只有 `/auth/device/code` 和 `/auth/device/token` 使用 RFC 8628/OAuth 错误结构。

至少冻结：

```text
INVALID_CREDENTIALS
INVALID_REFRESH_TOKEN
INVALID_OR_EXPIRED_CREDENTIAL
REGISTRATION_NOT_AVAILABLE
USERNAME_UNAVAILABLE
SESSION_LIMIT_REACHED
IDEMPOTENCY_RESULT_EXPIRED
ACCOUNT_LINK_NOT_FOUND
PROVIDER_IDENTITY_ALREADY_LINKED
REAUTHENTICATION_REQUIRED
AUTHORIZATION_STATE_CONFLICT
```

邮件、登录、Device Code 和恢复限流沿用 `nli_account.md`。所有秘密字段在结构化日志中删除而不是仅掩码。

## 批次 2：Provider、Friendship 与同步（已完成）

已确认：

- PUBLIC Registry 默认只列 ENABLED/NO_NEW_BINDINGS；既有 Binding 仍可解释 DISABLED Provider；
- Binding 授权沿用短期 HttpOnly Browser Transaction Cookie + Completion；
- Binding 用 `PUT` + Revision 完整替换用途开关；
- 好友申请提交返回统一 24 小时 Submission Receipt，不直接泄漏 Pending 是否创建；
- Accept/Reject/Cancel 返回当前关系/请求结果摘要；
- `/friends` 保持 NLI 与外部 Provider 条目的统一聚合列表；
- 同步任务使用 `mode=ONE/ALL` 判别联合，两者都返回 `202 + task_id`。

## Provider API

### Provider Registry

`GET /v2/providers` 为 PUBLIC、小规模不分页列表，按 `display_name, provider_id` 排序：

- 匿名调用只返回 `ENABLED / NO_NEW_BINDINGS`；
- 已登录调用额外返回当前账号已有 Binding 所引用的 DISABLED Provider；
- Registry DTO 仅含 provider_id、display_name、status、声明 Capability 和受信展示资源；
- 不返回 Adapter 类名、Issuer 内部规则、OAuth Client Secret、任意 Endpoint URL 或 Credential 配置。

`GET /v2/providers/{provider_id}` 遵循相同可见性。不可见或未知统一 `404 PROVIDER_NOT_FOUND`。

### Provider Binding DTO

```json
{
  "binding_id": "uuid-v4",
  "provider": {"provider_id":"provider_slug","display_name":"Provider"},
  "status": "ACTIVE",
  "enabled_usages": ["READ_FRIENDS"],
  "effective_capabilities": ["READ_FRIENDS"],
  "display_profile": {},
  "last_verified_at": 0,
  "last_error_category": null,
  "revision": 1,
  "created_at": 0,
  "updated_at": 0
}
```

不返回 Issuer、Subject、Subject HMAC、Provider Token、Credential Reference 或 Provider 原始错误。display_profile 只能含 Provider Adapter 已声明并清洗的公开展示字段。

`GET /v2/provider-bindings` 最多为 Registry Provider 数量，不分页，按 Provider display_name/provider_id 排序；UNBOUND 不返回。

### Binding Browser Authorization

`POST /v2/provider-bindings/{provider_id}/authorizations` 要求 `Idempotency-Key`，请求：

```json
{
  "purpose": "CREATE",
  "binding_id": null,
  "expected_subject_change": false
}
```

规则：

- CREATE 只能在该账号无有效同 Provider Binding 时使用；
- REAUTH 必须指定现有 Binding，且只接受原 Issuer+Subject；
- REPLACE 必须指定现有 Binding，并要求 `expected_subject_change=true`；
- 服务端固定 Provider、Account、Purpose、Binding、State、PKCE、Redirect、Scope 和 10 分钟期限；
- NO_NEW_BINDINGS 拒绝 CREATE/REPLACE，但允许既有 REAUTH；DISABLED 全部拒绝；
- 返回 Registry 允许的 authorization_url 和 expires_at，客户端不能提交 URL/Scope。

Callback 验证 State 后设置 `Secure + HttpOnly + SameSite=Lax` 的短期 Cookie，并 303 到固定前端。`POST /v2/provider-bindings/authorization-completions` 同时要求当前 Account Access Token 和 Cookie，原子完成：

- CREATE：建立唯一 Binding；
- REAUTH：Subject 相同则恢复 ACTIVE，否则 `409 PROVIDER_SUBJECT_MISMATCH` 且不修改；
- REPLACE：再次显式确认后原子替换 Subject/Credential，并使旧授权不可用；
- 同 Subject 已被其他有效账号绑定时返回不泄漏账号信息的 `409 PROVIDER_IDENTITY_UNAVAILABLE`；
- 完成结果可在 60 秒内安全重放，不返回 Provider Token。

### Binding 用途与解绑

`PUT /v2/provider-bindings/{binding_id}` 请求：

```json
{
  "revision": 4,
  "enabled_usages": ["READ_FRIENDS", "BACKGROUND_SYNC"]
}
```

- PUT 是完整用途配置，缺失用途表示关闭；
- 非 Provider 声明 Capability、非 Grant 允许或不合法组合返回 `422 INVALID_PROVIDER_USAGE`；
- Revision 不一致返回 `409 BINDING_REVISION_CONFLICT` 并附当前 revision；
- 空用途集合使 Binding 进入 DISABLED 并清短期 Access Token 缓存；
- 从 DISABLED 重新启用时重新计算 Grant；不足则进入/保持 REAUTH_REQUIRED；
- 成功返回更新后的 Binding。

`DELETE /v2/provider-bindings/{binding_id}` 本地事务先转 UNBOUND、删除/密码学封存 Credential、停 Task 和来源，再返回 `204`。上游撤销异步尽力执行；Provider 离线不阻止本地解绑。重复删除在仍可确认归属时返回 `204`。

## Friendship API

### 统一聚合好友列表

`GET /v2/friends` 返回 Phase 3 聚合条目判别联合：

```text
kind = NLI_ACCOUNT
kind = EXTERNAL_PROVIDER
```

支持 `limit/cursor`，使用 10 分钟查询快照和来源级 Continuation，按规范化 display_name、kind rank、稳定 ID 排序。NLI 条目返回公开 Account、Relationship Sources 与可见 Instance 摘要；External 条目返回 binding 范围 external_friend_id、Provider 展示字段、新鲜度和降级状态，绝不返回匹配 NLI Account 的线索。

### Friend Request Submission Receipt

`POST /v2/friend-requests` 必须使用 `Idempotency-Key`：

```json
{
  "target_account_id": "uuid-v4",
  "clear_own_ban": false
}
```

对“已接收提交”统一返回 `202 Accepted`：

```json
{
  "submission_id": "uuid-v4",
  "status": "RECEIVED",
  "expires_at": 0
}
```

Submission Receipt 保留 24 小时，只证明服务端接收了提交，不证明目标存在、Pending 已创建、未被 ban、未处于拒绝抑制或仍有 incoming 容量。Receipt 不提供 GET Endpoint，也不能作为 Friend Request ID。

- 同 Key/Body 返回同一 Receipt；
- 同方向已有 PENDING 时内部复用原 Request ID且不重复通知；
- 目标 ban、24 小时抑制、目标不可用或隐藏容量限制使用相同 202；
- 自己的 visible outgoing 配额已满可以返回 `429 OUTGOING_REQUEST_LIMIT_REACHED`；
- 已经 FRIENDS 或反向 PENDING 被本次操作接受时可以返回 `200` 当前 Relationship Result，因为这是调用者自身已可见状态；
- `clear_own_ban=true` 才允许原子清除自己的 ban 并继续。

### Friend Request DTO 与命令

Incoming/Outgoing 列表使用 Cursor。可见 Request 至少包含 request_id、direction、对方公开 Account、created_at、expires_at 和请求者可见 status，不返回 Source 的 Provider Subject/HMAC。

命令：

```text
POST /friend-requests/{id}/accept
POST /friend-requests/{id}/reject
POST /friend-requests/{id}/cancel
```

- Accept/Reject 仅接收方，Cancel 仅发起方；
- Reject Body 可选 `block=false`；`block=true` 在同一事务拒绝并设置自己的有向 ban；
- 成功返回 `200` `{request_id,status,relationship}` 结果摘要；
- 同一命令重试返回同一终态摘要，不重复通知；
- 其他终态、旧 Request ID 或方向错误返回 `409 FRIEND_REQUEST_STATE_CONFLICT` 或不泄漏的 404；
- 命令必须使用 `Idempotency-Key`，记录保留至少覆盖 Request 结果保留期。

### Friend 与 Block

`DELETE /v2/friends/{account_id}` 幂等删除 FRIENDS，并按 Gate C 设置操作者自己的有向 ban，返回 `204`。

`GET /v2/blocks` 只列自己的有向 ban，按创建时间倒序 Cursor 分页。`PUT /v2/blocks/{account_id}` 原子设置 ban、终止双方 Pending/Friendship 和相关自动同步来源，返回 `204`；`DELETE` 只清除自己的 ban，返回 `204`，不会自动恢复关系。

其他账号 ban、拒绝抑制窗口和隐藏 Provider pending 不进入任何读模型。

## Friend Sync Task API

`POST /v2/friend-sync-tasks` 必须使用 `Idempotency-Key`，Body 是严格判别联合：

```json
{"mode":"ALL","binding_id":"uuid-v4"}
```

或全部 Provider：

```json
{"mode":"ALL"}
```

或单个外部好友：

```json
{
  "mode":"ONE",
  "binding_id":"uuid-v4",
  "external_friend_id":"binding-scoped-opaque-id"
}
```

ALL 禁止 external_friend_id；有 binding_id 时表示该 Provider 的 PROVIDER_FULL，省略时表示 ALL_PROVIDERS。ONE 必须同时提供 binding_id 和 external_friend_id。AUTO_PROVIDER 只由内部调度器创建，不是公开 mode。两种公开模式都返回 `202 Accepted`、Location 和 Task：

```json
{
  "task_id": "uuid-v4",
  "mode": "ONE",
  "status": "PENDING",
  "created_at": 0,
  "expires_at": 0
}
```

相同活动 Scope 的 Task 按 Phase 3 合并并返回既有 Task；external_friend_id 只能在所属 Binding 重放。Binding/Capability 明确无权属于调用者自身配置错误，可以返回 409/422；目标匹配、ban、容量、绑定情况不进入响应。

`GET /v2/friend-sync-tasks` 使用 Cursor，按 `created_at DESC, task_id ASC` 列出 7 天内 Task。单 Task GET 只返回 Phase 3 允许的 scanned_count、processed_count、failed_count、Provider 总数/完成数、分类错误、Continuation 和状态；不返回 matched、created、advanced、blocked、unbound、auto_accepted 或目标账号线索。

`POST /v2/friend-sync-tasks/{task_id}/cancel` 使用 `Idempotency-Key`，返回 `202` 当前 Task。Cancellation 是请求状态；已提交的 Pair/Outbox 不回滚。终态重复取消返回当前终态且不创建副作用。

### Provider/Friend 错误和限流

至少冻结：

```text
PROVIDER_NOT_FOUND
PROVIDER_DISABLED
PROVIDER_NO_NEW_BINDINGS
PROVIDER_AUTHORIZATION_EXPIRED
PROVIDER_SUBJECT_MISMATCH
PROVIDER_IDENTITY_UNAVAILABLE
BINDING_NOT_FOUND
BINDING_REVISION_CONFLICT
BINDING_REAUTH_REQUIRED
INVALID_PROVIDER_USAGE
FRIEND_REQUEST_NOT_FOUND
FRIEND_REQUEST_STATE_CONFLICT
OUTGOING_REQUEST_LIMIT_REACHED
FRIEND_SYNC_TASK_NOT_FOUND
FRIEND_SYNC_RATE_LIMITED
```

Provider 上游分类仍由 ProviderService 转换为 409/422/429/502/503，不返回原始响应。好友申请、Block、同步分别按 Account、Pair、Binding、Provider 和 IP 限流；静默分支不能通过状态码、响应时间或 Task 统计泄漏。

## 批次 3：Instance、ACL、Proxy 与 Invite（已完成）

已确认：普通配置与 ACL 分别使用完整 PUT 并各自维护 Revision；创建时可选原子绑定 Proxy Secret；`/instances` 只列 Owner 自有资源；ACL 使用 Body Revision + Idempotency-Key；已有 Rule 回传 ID、新 Rule 由服务端生成；Proxy/Invite Secret 可加密重放 60 秒；NLI Invite 验证返回绑定当前 Instance Session 的 60 秒 Resolution；WebSocket Path 为 `/instances/{id}/ws`。

## Game Instance API

### 创建

`POST /v2/instances` 只能由当前未绑定的 `nli_account` Token Family 调用，必须提供 `Idempotency-Key`。请求包含 CLIENT_CLAIMED game_profile、compatibility、description、approval_mode、game_state、完整 acl，以及可选 `proxy_grant_secret`。

服务端推导 Owner、Session、Token Family、生命周期、在线状态、Rule ID、Revision 和时间。可选 Proxy Secret 与 Instance 创建、Family 绑定在同一事务校验；失败时都不提交。

成功返回 `201 Created`、Location 和 Owner Instance DTO，初始为 `ACTIVE + OFFLINE`、`config_revision=1`、`acl_revision=1`。同 Key/Body 重放相同 Instance；Family 已绑定或账号实例额度满返回 `409 TOKEN_FAMILY_ALREADY_BOUND / INSTANCE_LIMIT_REACHED`。

### DTO 和读取

公共 Instance Summary 仅含 instance_id、Owner 最小公开 Account、可选 Presented-Under Account、`publication_source = DIRECT / PROXY / INVITE`、ONLINE、CLIENT_CLAIMED Host Profile、兼容字段、清洗描述、approval_mode、game_state 和 updated_at。

不返回 Owner Session/Family、ACL、命中 Rule、Invite/Grant ID、Secret、租约截止、offline_since 或内部限流字段。

Owner DTO 额外返回 lifecycle、config_revision、acl_revision、代理绑定摘要、created_at/closed_at；当前绑定 Instance Session 通过专用端点读取 ACL、Invite 和 Join Request。

`GET /v2/instances` 仅列当前 Account 拥有的最多 5 个 ACTIVE Instance（含 ONLINE/OFFLINE），按 `created_at DESC, instance_id ASC`，无需分页。CLOSED 不进入该列表。其他用户的可见实例来自 `/friends` 或 Invite Resolution，不存在全局目录。

`GET /v2/instances/{id}` 每次重新验证：Owner Account 可读自己的 Owner 摘要但只有绑定 Family 能管理；Direct/Proxy Friend 必须有有效 FRIEND_LIST Source、最新 ACL ALLOW 和 ONLINE；NLI Invite 调用者必须提供绑定同一 Session/Instance 的 Resolution ID；Guest 只能读绑定目标。DENY、无 Source、OFFLINE、CLOSED、无效 Resolution与不存在统一 `404 INSTANCE_NOT_AVAILABLE`。

### 普通配置更新

`PUT /v2/instances/{id}` 只允许绑定 Family，必须提供 `Idempotency-Key`。Body 以 `config_revision` 加完整 game_profile、compatibility、description、approval_mode、game_state 替换普通配置，不包含 ACL、Proxy、Invite 或服务端字段。

成功 revision+1 并返回 Owner DTO；过期返回 `409 INSTANCE_REVISION_CONFLICT` 和 current_revision。Profile 变化不取消 Request 或影响可靠授权。

### ACL

`GET /v2/instances/{id}/acl` 只允许绑定 Family，返回 acl_revision 和完整 Rule。

`PUT /v2/instances/{id}/acl` 要求 `Idempotency-Key`，Body 含 `acl_revision` 和完整 `rules`：

- Rule 顺序由唯一 priority 决定；
- 空 Matcher 字段/列表保持 wildcard；
- 已有 rule_id 必须属于该 Instance，不能跨实例移动；
- 新 Rule 省略 ID，由服务端生成并在幂等重放中保持一致；
- 最多 100 Rule，每字段最多 100 值；
- typed Subject 严格验证，MC_USERNAME 在响应标注 WEAK_CLIENT_CLAIMED；
- 成功 revision+1，并在同一事务取消最新 ACL 不再 ALLOW 的 PENDING；
- 旧 revision 返回 `409 ACL_REVISION_CONFLICT` 与 current_revision；
- ACL 为空合法且等价 DENY ALL。

### 关闭和 WebSocket

`DELETE /v2/instances/{id}` 只允许绑定 Family，幂等 `204`。它原子 CLOSED、取消 PENDING、撤销 Invite、解除 Proxy、使 WS Session 失效并解绑仍有效 Family。

`GET /v2/instances/{id}/ws` 是 Upgrade Endpoint，严格遵循 `notifications.md`。它不是普通 GET 资源，也不接受 Query Token。

## Proxy Grant API

`POST /v2/proxy-grants` 要求近期认证和 `Idempotency-Key`，Body 含 grantee_account_id 与可空 expires_at。服务端确认双方 ACTIVE/FRIENDS 和 Issuer 少于 10 个 ACTIVE Grant。成功返回 `201`、Location、元数据和只显示一次的 grant_secret。Secret 加密重放 60 秒；之后同 Key 返回 `409 IDEMPOTENCY_RESULT_EXPIRED`。

`GET /v2/proxy-grants` 只列 Issuer 自己的 Grant，最多 10 个 ACTIVE 加近期终态。DTO 含 grant_id、Grantee 公开 Account、status、expires_at、bound_instance_id、last_bound_at 和时间，不含 Secret/Hash/Generation。

`POST /v2/proxy-grants/{id}/secret-rotations` 要求 Issuer、近期认证和 Idempotency-Key。成功返回新 Secret，旧 Generation 不能用于新绑定，现有绑定不变。`DELETE /v2/proxy-grants/{id}` 要求相同权限，幂等撤销并立即终止代理展示，但不关闭 Grantee Instance。

`PUT /v2/instances/{id}/proxy-grant` 只允许绑定 Instance Family，请求 `{grant_secret}` 并要求 Idempotency-Key。切换时先验证新 Grant 再原子解除旧关联；失败保留旧关联。`DELETE` 只解除当前 Instance 关联，不撤销 Issuer Grant。

Grantee 不能列出未使用的 Incoming Grant；成功绑定后只在自己 Instance DTO 看到 Grant ID 与 Issuer 公开身份。

## Invite API

`POST /v2/instances/{id}/invites` 只允许绑定 Family，要求 Idempotency-Key。Body 含可选 expires_at 与 max_uses；未填到期使用 24 小时，最大 7 天，max_uses 为 1–100；显式 null 不合法。成功返回 `201`、Location、元数据和只显示一次的 invite_secret；每 Instance 最多 3 个未终止 Invite。Secret 加密重放 60 秒。

`GET /v2/instances/{id}/invites` 返回 invite_id、status、expires_at、max_uses、consumed_uses、reserved_uses 和时间，不返回 Secret/Hash/Generation。

`POST /v2/instances/{id}/invites/{invite_id}/secret-rotations` 要求 Idempotency-Key，返回新 Secret；旧 Generation 不能建立新 Resolution/Guest/Request，但不取消已有 Reservation。`DELETE` 幂等撤销并取消关联 PENDING、释放 Reservation。

### NLI Invite Resolution

`POST /v2/invite-resolutions` 只允许已有 Source Instance 的 NLI_INSTANCE，要求 Idempotency-Key，Body 为 `{invite_secret}`。服务端验证 Secret、Invite、目标 ACTIVE+ONLINE、Source Instance、CLIENT_CLAIMED Profile 和最新 ACL。

成功返回 `invite_resolution_id`、公共 Instance Summary 和 expires_at。Resolution 绝对 60 秒，绑定请求者 Account、Token Family、Source/Target Instance、Invite ID 和 Generation；ID 不是独立 Bearer，只能由同一 Family 用于一个 Join Request；此时不预留次数。

Invite 轮换/撤销、ACL DENY、Target OFFLINE 或 Session 失效使 Resolution 不可用。无效 Secret、DENY、耗尽、过期、OFFLINE 和不存在统一 `404 INVITE_NOT_AVAILABLE`。原始 Secret 不进入 Request、通知或 Target。

读取目标摘要时 Resolution ID 使用 `X-Invite-Resolution-ID` Header；Join Request 在 Body 引用 resolution_id。

### Instance/ACL/Secret 错误和限流

至少冻结：

```text
TOKEN_FAMILY_ALREADY_BOUND
INSTANCE_LIMIT_REACHED
INSTANCE_NOT_FOUND
INSTANCE_NOT_AVAILABLE
INSTANCE_REVISION_CONFLICT
ACL_REVISION_CONFLICT
INVALID_ACL
PROXY_GRANT_NOT_FOUND
PROXY_GRANT_NOT_AVAILABLE
PROXY_GRANT_ALREADY_BOUND
INVITE_LIMIT_REACHED
INVITE_NOT_FOUND
INVITE_NOT_AVAILABLE
IDEMPOTENCY_RESULT_EXPIRED
```

创建/更新按 Account、Family、Instance 限流；Secret 验证按 IP、Account/Guest 和 Hash Prefix 防爆破；ACL 更新限制 Body/Rule/Matcher 大小。所有 Secret 字段从日志删除。

## 批次 4：Guest、Join、Lease 与举报（已完成）

已确认：Join Body 只含 target_instance_id 和可选 invite_resolution_id，Source 由服务端推导；MANUAL/AUTO 统一 `201` 返回 PENDING/ACCEPTED Request；不提供独立 Lease Validation API，Phase 9 真正使用时重验；Join 命令返回最新 Request DTO；Requester 按 Source Instance 列表恢复；Guest Token 创建结果加密重放 60 秒；举报必须引用调用者参与的 Request，且只返回不可查询的 RECEIVED Receipt。

## Guest Session API

`POST /v2/guest-sessions` 为 PUBLIC，必须提供 `Idempotency-Key`：

```json
{
  "invite_secret": "<opaque-secret>",
  "game_profile": {
    "source": "offline",
    "uuid": null,
    "username": "GuestPlayer"
  }
}
```

服务端验证 Invite、Target ACTIVE+ONLINE、ANONYMOUS + INVITE_CODE ACL 和 Profile 结构后返回 `201 Created`：

```json
{
  "access_token": "<opaque-secret>",
  "token_type": "Bearer",
  "access_expires_at": 0,
  "guest": {
    "guest_id": "uuid-v4",
    "expires_at": 0,
    "game_profile": {"verification":"CLIENT_CLAIMED"}
  },
  "instance": {}
}
```

Guest 不签 Refresh Token。Token 绑定一个 Guest、Target、Invite Generation、Profile 和最多一个 Request。相同 Key/Body 的 Token 结果加密重放 60 秒；之后同 Key 返回 `409 IDEMPOTENCY_RESULT_EXPIRED`。

无效/撤销/耗尽 Invite、DENY 和 Target 不可用使用统一 `404 INVITE_NOT_AVAILABLE`；客户端自身 Profile 结构错误返回 `422 VALIDATION_FAILED`，但不泄漏 Invite 状态。创建 Guest 不预留 Invite 次数。Guest 没有 WebSocket，结果通过 HTTP 查询。

## Join Request API

### 创建

`POST /v2/join-requests` 要求 `Idempotency-Key`。

NLI_INSTANCE Body：

```json
{
  "target_instance_id": "uuid-v4",
  "invite_resolution_id": null
}
```

- Resolution 有效时建立 INVITE_CODE Source；
- Resolution 为空时只尝试服务端验证的 FRIEND_LIST Direct/Proxy 关系；
- 客户端不能提交 Source、Identity Traits、Requester Account、MC Profile 或 Presented-Under Account；
- MC Profile 从当前 Source Instance 复制为 CLIENT_CLAIMED 快照。

NLI_GUEST Body 必须为空对象；Target、Invite、Generation、ANONYMOUS Trait 和 Profile 都从 Guest Session 推导。

两类调用都重新验证双方 Session、Target ONLINE、Source、ACL、Invite/Proxy 和 pending 限额。Invite 路径在同一事务预留次数；Resolution 在成功创建时标记已使用。

成功统一返回 `201 Created`、Location 和调用者可见 Request：MANUAL 为 PENDING；AUTO 在同一事务直接为 ACCEPTED 并包含 lease_expires_at。相同 Key 重放同一结果；同一 Requester/Target/Source 已有 PENDING 时返回 `200` 既有 Request，不创建重复通知或 Reservation。

### Request DTO

共同字段：request_id、target Instance Summary、requester_type、source、status、decision_mode、created_at、expires_at、terminal_at 和 ACCEPTED 时的 lease_expires_at。

Target 视图额外含请求者最小公开 Account 或 Guest ID、服务端 Identity Traits、可选 Presented-Under Account，以及申请时 CLIENT_CLAIMED Profile 快照。Requester 视图不返回 Target 内部 ACL、Invite/Grant 引用、Target Session 或命中 Rule。双方都不看到 Session ID、Reservation ID、Secret/Hash 或内部封禁原因。

固定公开终态不携带 Owner 自由文本拒绝原因。

### 查询

- `GET /v2/join-requests` 只允许当前 NLI_INSTANCE，列其作为 Source Instance 发起的 PENDING 与 24 小时近期终态；
- `GET /v2/instances/{id}/join-requests` 只允许 Target 绑定 Family，支持 status 过滤并列 PENDING 与近期终态；
- 两者按 `created_at DESC, request_id ASC` Cursor 分页；
- `GET /v2/join-requests/{id}` 只允许 Target 绑定 Family、原 NLI Source Family 或绑定 Guest；
- NLI 终态保留可查 24 小时，Guest 取 5 分钟绝对期和终态后 60 秒的较早值；
- 过期/不可见/其他 Session 统一 `404 JOIN_REQUEST_NOT_FOUND`。

### Accept、Reject、Cancel

命令都要求 `Idempotency-Key`：

```text
POST /v2/join-requests/{id}/accept
POST /v2/join-requests/{id}/reject
POST /v2/join-requests/{id}/cancel
```

Accept/Reject 只允许 Target 绑定 Family，Cancel 只允许原 Source Family或绑定 Guest。每个命令在同一 Request 行锁下复核全部不变量；成功 `200` 返回最新 Request DTO。

Reject 可选 Body `{block_requester:false}`：对 NLI 请求者为 Owner 自己设置有向 ban 并终止关系；对 Guest 为当前 Guest Session ban。Block 不依赖 MC username/UUID。

同一命令重试返回同一结果，不重复 Invite 消费/释放或通知；不同终态竞争返回 `409 JOIN_REQUEST_STATE_CONFLICT` 和调用者可见当前 status。Accept 成功原子消费 Reservation 并建立 60 秒 Lease；Reject/Cancel/Expire 释放 Reservation。

`POST /v2/instances/{id}/join-requests/reject-all` 只允许 Target Family，要求 Idempotency-Key 且 Body 为空对象。它只拒绝当前 PENDING，不批量设置任何 ban。成功 `200` 返回 `{rejected_count, completed_at}`；重复返回同一结果。

### Acceptance Lease

不提供 `/lease-validations`、Ticket 下载或独立 Bearer Token。ACCEPTED Request GET 只暴露 lease_expires_at。

真正使用 Lease 的 Phase 9 信令入口必须同时携带 Request ID 和调用方当前 Session，并重新验证 Target/Requester Session、Instance、Source、最新 ACL 与未过期 Lease。单独调用“检查是否可用”会产生 TOCTOU，因此不作为 REST 业务端点。

## Report API

`POST /v2/reports` 必须使用 Idempotency-Key，只允许仍可读取某 Join Request 的参与方：

```json
{
  "join_request_id": "uuid-v4",
  "category": "HARASSMENT",
  "description": "Optional user-provided description"
}
```

category 初始为 `HARASSMENT / CHEATING / IMPERSONATION / MALICIOUS_CONTENT / OTHER`。description 可空，最多 2000 Unicode 字符，去除控制字符；不接受任意 Metadata JSON、客户端指定 reported_account_id/reported_guest_id、MC Profile、IP、附件 URL 或处罚建议。

服务端从 Request 与调用 Session 推导 Reporter、Counterparty、Instance、NLI Account/Guest ID、Source 和保存的 CLIENT_CLAIMED Profile 快照。调用者不能举报未参与 Request 的第三方，也不能改变快照。

成功返回 `201 Created`：

```json
{
  "report_id": "uuid-v4",
  "status": "RECEIVED",
  "created_at": 0
}
```

不提供用户侧 GET、审核状态或处罚结果。相同 Key/Body 返回同一 Receipt；同 Reporter/Request 的重复滥用受限流。举报记录不自动处罚 NLI Account，也不能仅凭 MC username/UUID 建立账号归属。

## Guest/Join/Report 错误与限流

至少冻结：

```text
GUEST_SESSION_EXPIRED
GUEST_FLOW_ALREADY_USED
JOIN_REQUEST_NOT_FOUND
JOIN_REQUEST_STATE_CONFLICT
JOIN_REQUEST_LIMIT_REACHED
JOIN_SOURCE_NOT_AVAILABLE
ACCEPTANCE_LEASE_EXPIRED
REPORT_CONTEXT_NOT_AVAILABLE
REPORT_RATE_LIMITED
```

枚举敏感的 Invite/ACL/Instance 拒绝继续使用统一不可用响应。Guest/Join 按 IP、Guest、Account、Source Instance、Target Instance 和 Pair 分层限流；Report 按 Reporter、Request、Target 和 IP 限流。

## Principal 与授权矩阵

| 端点类别 | PUBLIC | NLI_ACCOUNT | NLI_INSTANCE | NLI_GUEST |
| --- | --- | --- | --- | --- |
| 注册、登录、公开邮件流程 | 明确列出的端点 | 可调用但 Token 不改变语义 | 可调用但 Token 不改变语义 | 禁止作为账号身份 |
| Account/Friend/Provider 普通 API | 禁止 | 当前 Account | 与 Account Session 相同 | 禁止 |
| 创建 Instance | 禁止 | 仅当前 Family 未绑定时 | Family 已绑定，冲突 | 禁止 |
| 读取 Owner Instance 列表 | 禁止 | 当前 Account | 当前 Account | 禁止 |
| 管理一个 Instance/ACL/Invite/Join Target | 禁止 | 禁止 | 仅与该 Instance 绑定的同一 Family | 禁止 |
| Proxy Grant Issuer 管理 | 禁止 | 当前 Account + 必要 Recent Auth | 当前 Account + 必要 Recent Auth | 禁止 |
| 查看 FRIEND_LIST Instance | 禁止 | 无 Source Instance，不能加入 | 服务端验证 Direct/Proxy + ACL | 禁止 |
| Invite Resolution | 禁止 | 无 Source Instance，禁止 | 同一 Source Instance Family | 禁止 |
| 创建 Guest | Invite Secret 作为专用输入 | 不创建 Guest | 不创建 Guest | 已有 Guest 不重复创建 |
| 创建 Join Request | 禁止 | 无 Source Instance，禁止 | 自己的 Source Instance | 仅绑定目标的一次流程 |
| Join Accept/Reject | 禁止 | 禁止 | 仅 Target 绑定 Family | 禁止 |
| Join Cancel | 禁止 | 禁止 | 仅原 Source Family | 仅绑定自己的 Request |
| Report | 禁止 | 无参与 Instance 时禁止 | 仅参与 Request | 仅绑定且仍可读 Request |
| WebSocket | 禁止 | 未绑定，禁止 | 仅目标绑定 Family | 禁止 |

`nli_account` Audience 本身不授予 Instance 管理权。所有资源操作按 Account ID、Token Family、绑定 Instance、Request 角色、Source、ACL 和生命周期再次鉴权。

Recent Auth 不是独立 Principal；它是当前 Family 上 5 分钟内必须重新检查的附加条件。Browser Transaction Cookie/State/Email Secret/Invite Secret 也都是专用流程凭据，不能作为普通 Bearer Token。

## 服务端推导字段

以下字段若出现在不明确允许的请求 Body，必须以 `400 UNKNOWN_FIELD` 或 Schema 错误拒绝，而不是忽略：

```text
account_id / owner_account_id / requester_account_id / guest_id
session_id / token_family_id / bound_instance_id
lifecycle / online / status / created_at / updated_at / terminal_at
source / identity_traits / publication_source / presented_under_account_id
provider issuer / subject / subject_hmac / credential_reference
invite_id / generation / reservation_id / consumed_uses / reserved_uses
acl match result / matched_rule_id / weak verification result
reported_account_id / reported_guest_id / report profile snapshot
```

允许作为资源引用的 `target_account_id`、`target_instance_id`、`request_id`、`binding_id` 等不因此变成授权声明；服务端仍重新查找并验证。

## HTTP 状态与 Problem 映射

| Status | 使用场景 |
| ---: | --- |
| 200 | GET；返回当前资源的状态转换命令；Token Pair；已有幂等资源 |
| 201 | 注册、Instance、Grant、Invite、Guest、Join Request、Report 等资源创建 |
| 202 | 防枚举邮件请求、Friend Submission Receipt、异步 Sync Task/Cancel |
| 204 | 无 Body 的验证、撤销、删除、近期认证和邮箱切换 |
| 303 | Provider Browser Callback 到固定前端完成页 |
| 400 | JSON/Form/UUID/Cursor/Header 结构错误，或一次性凭据无效/过期的统一错误 |
| 401 | Access/Refresh/Guest Token 缺失、无效、过期或 Audience 错误 |
| 403 | 身份有效但明确没有操作权限，且不会造成资源枚举 |
| 404 | 调用者可见域中资源不存在，或 Invite/ACL/ban/状态必须隐藏的统一不可用 |
| 409 | Revision、状态机、唯一性、Family 绑定、幂等键或并发冲突 |
| 413 | 请求 Body 超过端点上限 |
| 422 | 自身输入值/组合违反业务规则，但资源存在性无需隐藏 |
| 429 | 调用者自身或统一防滥用限制；携带 Retry-After |
| 502 | Provider 返回无效上游响应 |
| 503 | Provider、LeaseStore 或必要依赖暂不可用 |

Problem Body 始终遵循 `common.md`。401 可带标准 `WWW-Authenticate: Bearer`，但 error_description 不泄漏内部账号状态。404 隐藏响应不得通过 body、header 或明显时间差暴露真实分支。

## 幂等与 Secret 重放矩阵

### 强制 Idempotency-Key

- 所有创建资源的 POST：register、login、Instance、Grant、Invite、Guest、Join、Report、Friend Submission、Sync Task；
- 所有显式状态命令：Device approve/deny、Friend accept/reject/cancel、Sync cancel、username/password/deletion、Join accept/reject/cancel/reject-all；
- Provider/Binding Browser Transaction start/complete；
- Instance config/ACL PUT、Proxy bind PUT 和 Secret rotation；
- Invite Resolution。

泛化邮件发送请求不要求 Idempotency-Key，因为它们始终返回统一 202，并由用途/邮箱/IP 发送间隔合并。GET 不接受幂等键语义。天然幂等的普通 DELETE/Block PUT 可以携带但不要求。

### Key Scope 和摘要

继承 `Principal + Method + normalized route + key`。PUBLIC 请求额外绑定安全规范化后的操作类别与匿名流程上下文，不能让一个邮箱/Secret 的 Key 重放另一个目标。

请求摘要使用规范化 Schema 后的 keyed digest。Password、Token、Provider Code、Invite/Grant Secret 等敏感字段只能进入内存中的摘要计算，普通幂等表不保存原文或可离线验证的普通哈希。

### 返回 Secret 的操作

Login、Refresh、Provider Login、Device Token、Guest Token、Grant/Invite Secret 创建或轮换的完整成功响应最多加密保存 60 秒：

- 相同凭据/Key/Body 返回同一结果；
- 60 秒后删除密文；
- 24 小时内保留非敏感完成摘要，重试返回 `409 IDEMPOTENCY_RESULT_EXPIRED`；
- 不能为了重试再次生成第二个 Token/Secret；
- Refresh/Device 的重用风险规则优先于普通幂等规则。

不含新 Secret 的一次性邮件命令，以凭据自身作为一次性键，成功结果可重放 60 秒。

## 并发与 Revision

- Account username/email 唯一性、Token Family 上限/绑定、Friend Pair、Provider Subject、Grant、Invite、Join Request 和 Reservation 使用领域锁/唯一约束；
- Instance config、ACL、Provider Binding、Provider Login Identity 使用显式 revision；
- revision 冲突返回 409 与调用者有权看到的 `current_revision`，不自动覆盖；
- Idempotency 记录和领域事务必须确定唯一赢家；同 Key 尚在执行返回可重试 `409 REQUEST_IN_PROGRESS`；
- 状态转换在事务内重新鉴权，不能只依赖此前 GET；
- 业务数据与 Outbox 同事务提交；通知失败不回滚 HTTP 成功；
- LeaseStore 跨事务协调遵循 `notifications.md` 的 Session ID 复核、补偿和 fail-closed；
- 409 返回当前 status 仅限调用者仍有读取权限，否则使用 404；
- 客户端收到 409 后应 GET 最新资源，再决定是否以新 Key/revision 重试。

## 请求大小、分页与限流

### 大小

- 普通 JSON Body 默认最大 64 KiB；
- ACL PUT 最大 256 KiB，且仍受 100 Rule/每 Matcher 100 值限制；
- Report Body 最大 8 KiB，description 最多 2000 Unicode 字符；
- Provider Callback Query、Authorization Header、Idempotency-Key 和 Cursor 各有独立较小上限；
- 超限在解析完整 Body 前拒绝为 `413 Payload Too Large`；
- 不支持任意 Metadata JSON、文件上传或 URL 抓取。

### 分页

`friends` 使用 Phase 3 的 10 分钟快照 Cursor；Friend Request、Block、Task、Join Request 使用稳定 keyset Cursor；Sessions、Registry、Bindings、Owner Instances、Grant/Invite 因领域硬上限不分页。所有 Cursor 绑定 Principal、过滤、排序和快照/过期语义。

### 初始限流面

| 类别 | 必须组合的 Key |
| --- | --- |
| Login/邮件/恢复 | IP + 规范化邮箱 HMAC + Purpose |
| Device | IP + Account + User/Device Code HMAC |
| Provider | Account + Binding + Provider + Operation |
| Account 解析/好友提交 | Account + IP + Target/Pair |
| Friend Sync | Account + Binding + Provider + External Friend/ALL Scope |
| Instance 创建/更新 | Account + Family + Instance |
| Invite/Proxy Secret 验证 | IP + Account/Guest + Secret Hash Prefix |
| Guest | IP + Invite + Target Instance |
| Join | IP + Account/Guest + Source + Target + Pair |
| Report | IP + Reporter + Request + Counterparty |
| WebSocket | IP + Account + Family + Instance + concurrent handshake |

已冻结的具体限制继续适用：认证邮件 60 秒间隔和每小时上限；Provider 全量/单人同步最短 15 分钟；Guest Invite 失败验证每 IP 15 分钟 10 次、Guest 创建每 IP 15 分钟 20 次、每 Invite 15 分钟 100 次、每 Instance 100 个活跃 Guest；Source outgoing PENDING 3、Target incoming PENDING 100。

隐藏分支共享相同状态码和近似成本路径。限流指标、Hash Prefix 和内部阈值不返回客户端。

## 通知映射

| HTTP 事务 | Event Type | Scope | Recipient |
| --- | --- | --- | --- |
| Account profile/username/email/password/deletion/recovery | `account.updated` | ACCOUNT | 当前 Account 全部 ONLINE Instance；删除前尽力发送 |
| Session 单独/批量撤销 | `session.revoked` | RESOURCE | 当前 Account 其他 ONLINE Instance；受影响 WS 另行关闭 |
| Provider Login Identity 创建/状态/删除 | `account.updated` | ACCOUNT | 当前 Account ONLINE Instances |
| Provider Binding CREATE/REAUTH/REPLACE/PUT/DELETE | `provider_binding.updated` | RESOURCE | 当前 Account ONLINE Instances |
| Friend Request 新 Pending | `friend_request.created` | RESOURCE | 接收方 ONLINE Instances |
| Friend Request 终态/取消 | `friend_request.updated` | RESOURCE | 双方 ONLINE Instances |
| FRIENDS、删除或 Block 导致关系变化 | `friendship.updated` | RESOURCE | 双方 ONLINE Instances |
| Sync Task 状态变化 | `friend_sync_task.updated` | RESOURCE | Task Owner ONLINE Instances |
| Instance 创建/配置/ONLINE 信息变化 | `instance.updated` | RESOURCE | Owner 当前 Instance；必要时 Owner 其他 ONLINE Instances |
| Instance CLOSED | `instance.closed` | RESOURCE | Owner 其他 ONLINE Instances 和仍参与近期 Request 的 NLI Source Instances |
| ACL 替换 | `instance.acl_updated` | RESOURCE | Target current WS；被取消 Request 另发 updated |
| Invite 创建/轮换/撤销/耗尽 | `instance.invite_updated` | INSTANCE | Target current WS |
| Proxy Grant 创建/绑定/解除/轮换/撤销/失效 | `proxy_grant.updated` | RESOURCE | Issuer ONLINE Instances；绑定 Target current WS |
| Join Request 创建 | `join_request.created` | RESOURCE | Target current WS |
| Join Accept/Reject/Cancel/Expire/失效 | `join_request.updated` | RESOURCE | Target 与 NLI Source current WS；Guest 仅 HTTP |
| Report 创建 | 无用户通知 | — | 仅内部审核管道 |

同一业务事务的 Event ID 由 Outbox 创建并在 Fanout/重试中复用。静默 Friend Submission、无在线 Recipient 和通知失败不改变 HTTP 响应。每种事件都可通过 `notifications.md` 恢复矩阵所指向的 GET 获取权威状态。

## 安全、并发与客户端场景走查

### 注册和登录响应丢失

- Register 使用同 Key 返回同一非敏感 Registration；
- Login 成功 Token Pair 只加密重放 60 秒；
- 60 秒后不会生成第二个 Family 或再次返回旧 Secret；
- 邮箱验证只激活账号，客户端随后显式 Login。

结果：通过。

### Refresh 响应丢失

- 首次 Refresh 已使旧 Access/Refresh 失效；
- 相同旧 Refresh + Key 在 60 秒内返回同一新 Pair；
- 不再次轮换；
- 超窗旧 Refresh 重用只撤销对应 Family。

结果：通过。

### Provider LOGIN 未关联 Subject

- State/PKCE 验证成功但无 Login Identity；
- LOGIN 返回 ACCOUNT_LINK_NOT_FOUND，不创建账号；
- 只有新的显式 REGISTER 事务可以收集账号字段；
- Provider Token/Subject 不进入 URL 或前端 DTO。

结果：通过。

### Provider REAUTH 登录了不同 Subject

- Completion 比较原 Issuer+Subject；
- 返回 PROVIDER_SUBJECT_MISMATCH；
- 不修改 Binding/Login Identity/Credential；
- 客户端只能显式启动 REPLACE 或 LINK。

结果：通过。

### Device 批准、拒绝和消费并发

- approve/deny/consume/expire 由状态机选出一个合法赢家；
- Approve 不创建 Family；
- Consume 时才检查账号 ACTIVE 和 Session 上限；
- 同 Device Code 成功结果只创建一个 Family 并重放 60 秒。

结果：通过。

### 撤销绑定 Instance 的当前 Session

- DELETE 自己当前 session_id 提交后返回 204；
- Family 立即 REVOKED；
- Instance 执行 CLOSED、WS 失效、Request/Invite/Proxy 清理；
- 后续使用旧 Access Token 返回 401。

结果：通过。

### 好友申请被 ban 或抑制

- POST 返回与正常接收相同的 202 Submission Receipt；
- 不返回 Request ID、ban、目标容量或账号状态；
- 不创建 Pending/通知；
- Idempotency 重试返回同一 Receipt。

结果：通过。

### Friend Request 反向并发

- A→B 与 B→A 并发锁定同一无序 Pair；
- 最终最多一个 Pair/Request 状态；
- 反向 Pending 被发送动作接受为 FRIENDS；
- 命令重试返回同一结果且不重复通知。

结果：通过。

### 同 Family 并发创建 Instance

- 两个不同 Key 请求竞争同一 Family；
- 仅一个可以从 ACTIVE 绑定为 ACTIVE_BOUND；
- 赢家 201，另一个 409 TOKEN_FAMILY_ALREADY_BOUND；
- 不产生孤立 Instance 或双绑定。

结果：通过。

### ACL 并发替换

- 两个客户端携带相同 acl_revision；
- 一个提交 revision+1并重评 Pending；
- 另一个 409 ACL_REVISION_CONFLICT；
- 同一 Idempotency-Key 重放赢家原结果和相同 Rule ID。

结果：通过。

### Proxy/Invite Secret 响应丢失

- 创建/轮换结果在 60 秒内返回同一 Secret；
- 之后不从列表找回；
- 客户端可显式再次轮换；
- 日志、通知和普通幂等表均无原文。

结果：通过。

### Invite Resolution 被跨 Session 重放

- 攻击者获得 Resolution UUID 但使用其他 Family；
- 服务端发现 Account/Family/Source Instance 绑定不符；
- 返回统一 INVITE_NOT_AVAILABLE/JOIN_SOURCE_NOT_AVAILABLE；
- 不创建 Request 或 Reservation。

结果：通过。

### AUTO Join 与 Invite 次数

- 创建事务锁定 Invite 并验证 Resolution/Guest；
- 同事务创建 Request、预留并立即消费次数、提交 ACCEPTED Lease；
- 失败不消耗次数；
- 同 Key 重试不重复消费。

结果：通过。

### Accept 与 Target OFFLINE 并发

- Accept 在同一事务重新检查 Target Session、ONLINE、Source、ACL 和 Invite/Proxy；
- OFFLINE 先发生则 Accept 失败/Request CANCELLED；
- Accept 先提交时 Lease 仍会在真正使用时再次验证；
- 不存在仅凭旧 GET 成功的授权。

结果：通过。

### Report 注入其他目标/Profile

- Report Schema 不接受 reported_account_id、guest_id 或 Profile；
- 服务端只从调用者仍可读取的 Request 推导 Counterparty 和快照；
- 未参与 Request 返回 REPORT_CONTEXT_NOT_AVAILABLE；
- MC username/UUID 不自动归属或处罚 NLI Account。

结果：通过。

### 通知完全丢失

- HTTP 事务和 Outbox 已提交，EventBus 提示丢失；
- REST GET 仍返回权威 Account/Friend/Instance/Request 状态；
- WebSocket ready、下一事件或主动刷新触发恢复；
- 客户端不按 Event 顺序构造业务状态。

结果：通过。

## 客户端可实现性清单

- Web：可完成邮箱注册→验证→显式登录→Session 管理；
- Provider Web Flow：Start→Provider→Callback 303→Cookie Completion，无 Token URL；
- Mod：RFC Device Code→浏览器显式批准→Poll Token→创建 Instance→连接 WS；
- 好友：精确 username 预览→Submission Receipt→Incoming/Outgoing GET→命令；
- Provider 好友：Binding 授权→用途 PUT→ONE/ALL Task→聚合 `/friends`；
- Host：创建 Instance→WS ONLINE→ACL/Invite/Proxy 管理→处理 Join；
- NLI Requester：好友列表或 Invite Resolution→创建 Request→GET/WS 恢复；
- Anonymous：Invite Secret→Guest Session→一次 Join→HTTP 结果；
- Accepted Join：Request GET 取得 Lease 截止，Phase 9 按 Request ID 使用；
- 举报：参与 Request→最小 Report Receipt，无审核状态依赖。

客户端无需猜测 Source、Identity Trait、Owner、Session、Invite Generation、Reservation、ACL 命中规则或 Report Counterparty。

## Phase 6 完成条件

- [x] 全部 Path/Method 冻结
- [x] Principal 与授权矩阵冻结
- [x] Request/Response DTO 冻结
- [x] 状态码和 Problem Type 冻结
- [x] 幂等、并发与限流冻结
- [x] 通知映射冻结
- [x] 主要场景和客户端可实现性通过复核

## Phase 9 Gate F 扩展：Signaling

> 状态：`GATE_F_FROZEN`。本节只追加 5 个 Operation；Phase 6 / Gate E 的 89 个 Operation、Join、Guest、ACL、Instance 与通知语义不变。`signaling.md` 是实时协议、Relay Authorizer 和客户端恢复的权威来源。

### 端点

| Method | Path | Principal | 语义 |
| --- | --- | --- | --- |
| POST | `/v2/join-requests/{request_id}/signaling-sessions` | 精确绑定的 NLI_INSTANCE 或 NLI_GUEST | 强制 Idempotency-Key 的 create-or-attach；每 Request 终身至多一个逻辑 Session |
| GET | `/v2/signaling-sessions/{signaling_session_id}` | 该 Session 精确绑定参与方 | 读取最小 PostgreSQL 权威生命周期/Phase，用于恢复 |
| DELETE | `/v2/signaling-sessions/{signaling_session_id}` | 该 Session 精确绑定参与方 | 天然幂等地关闭整个 Session；终态仍 204 |
| GET Upgrade | `/v2/signaling-sessions/{signaling_session_id}/ws` | 该 Session 精确绑定参与方 | 唯一受限实时业务 WS 例外；Offer/Answer/ICE 协议 v1 |
| POST | `/v2/signaling-sessions/{signaling_session_id}/ice-servers` | 该 Session 精确绑定参与方 | 强制 Idempotency-Key；签发本 Role 的 STUN/TURN 配置和独立 Relay Grant |

两个 POST Body 均为 closed object：create-or-attach 为 `{}`；ICE Server 请求仅 `{ "requested_policy": "ALL" | "RELAY" }`。客户端不得提交 Role、对端、Region、Endpoint、TTL、额度、Source、ACL、Profile、Grant ID 或网络授权字段。所有入口先校验 Token 与精确参与方，再按 `signaling.md` 重验 ACCEPTED Lease、双方 Instance/ONLINE、Source、关系/Proxy、最新 ACL、Guest/Family 和绝对期限。

### Signaling Session DTO

create/attach 的 `201/200`、GET 的 `200` 使用同一个 closed DTO：

```json
{
  "signaling_session_id": "uuid-v4",
  "join_request_id": "uuid-v4",
  "role": "REQUESTER",
  "offerer": "REQUESTER",
  "status": "ACTIVE",
  "channel": "/v2/signaling-sessions/{signaling_session_id}/ws",
  "protocol_phase": "WAITING_FOR_OFFER",
  "epoch": 0,
  "candidate_exchange_complete": false,
  "terminal_classification": null,
  "terminal_at": null,
  "created_at": 1735689600000,
  "expires_at": 1735689660000
}
```

`role` 按当前调用者推导，固定 `offerer=REQUESTER`。status 为 `ACTIVE|CLOSED|EXPIRED`；Phase 为 `WAITING_FOR_OFFER|WAITING_FOR_ANSWER|EXCHANGING_CANDIDATES`；epoch 为 `0..3`。终态分类封闭为 `CLOSED_BY_REQUESTER|CLOSED_BY_TARGET|CLOSED_BY_AUTHORIZATION|CLOSED_BY_POLICY|EXPIRED_BY_WINDOW`。DTO 不含 connection/node、对端身份/Profile、SDP、ICE、地址、ACL 命中、Invite/Reservation 或 Relay Secret。

create-or-attach 首次创建返回 201 + Location；完整重验后 attach 返回 200；同 Key 重放返回首次快照，客户端随后 GET 当前状态。终态后同 Request 不得重建。DELETE 对可见终态天然返回 204，不撤销 Join Request 的 ACCEPTED 历史。

### ICE Server DTO

成功 `200` 为 closed discriminated union：

```json
{
  "ice_transport_policy": "RELAY",
  "relay_status": "AVAILABLE",
  "region": "ap-east-1",
  "ice_servers": [
    {
      "urls": ["turn:turn.example.net:3478?transport=udp"],
      "username": "opaque-random",
      "credential": "redacted-example",
      "credential_type": "PASSWORD"
    }
  ],
  "credential_issue_deadline": 1735689660000,
  "allocation_create_deadline": 1735689660000,
  "relay_grant_expires_at": 1735693200000
}
```

服务端可把请求的 `ALL` 提升为有效 `RELAY`，不得把请求的 `RELAY` 降为 `ALL`。`RELAY+AVAILABLE` 必须只含 TURN；`ALL+AVAILABLE` 可含 STUN/TURN；只有有效策略仍为 `ALL` 时才允许 `ALL+UNAVAILABLE` 的 STUN-only 成功分支并令 allocation/grant 两个 deadline 显式 null；有效策略为 `RELAY` 且无容量时固定返回 `503 TURN_UNAVAILABLE`，不返回 STUN-only。响应强制 HTTPS、`Cache-Control: no-store`、`Pragma: no-cache`、`Referrer-Policy: no-referrer`，不得重定向。相同 Role/Policy/Key 在 60 秒签发窗口内重放同一 Secret；窗口外不重签。TURN Password 可验证到 Grant 绝对截止，但新 Allocation 只能在 allocation_create_deadline 前激活；维护由独立 Authorizer控制。

### WebSocket Upgrade 与协议暴露

Upgrade 只接受 Header `Authorization: Bearer`；Token 不得进入 Query、Fragment、`Sec-WebSocket-Protocol` 或首帧。v2 仅支持能设置 Upgrade Header并主动发送标准 Ping 的原生/Mod 客户端，浏览器 JavaScript WebSocket 不受支持。成功为 101；首帧为 `ready`。客户端每 5 秒 Ping，连接 Lease 20 秒；每 Role 一个 current connection，Route Document CAS替换旧连接。

OpenAPI 通过 `x-signaling-protocol` 引用 closed client/server Envelope：客户端类型 `offer|answer|ice_candidate|ice_end|ice_restart|delivery_ack|close`，服务端类型 `ready|offer|answer|ice_candidate|ice_end|ice_restart|result|delivery_ack|resend_required|error|session_closed`。Frame 64 KiB、SDP 48 KiB、Candidate 2048 B；每 Role/Epoch 64 Candidate，epoch最多3。`delivery_ack.reply_to` 位于顶层且 payload为空，只表示收到。WS 关闭码为 1000/1002/1009/4401–4407，精确恢复动作见 `signaling.md`。

### 公开错误、幂等与限流

新增 Problem Code：`SIGNALING_SESSION_NOT_FOUND`、`SIGNALING_AUTHORIZATION_LOST`、`SIGNALING_SESSION_STATE_CONFLICT`、`SIGNALING_WINDOW_EXPIRED`、`SIGNALING_UNAVAILABLE`、`TURN_GRANT_STATE_CONFLICT`、`TURN_QUOTA_EXCEEDED`、`TURN_UNAVAILABLE`。继续复用 `INVALID_REQUEST`、`UNAUTHORIZED`、`JOIN_REQUEST_NOT_FOUND`、`IDEMPOTENCY_KEY_REUSED`、`IDEMPOTENCY_IN_PROGRESS`、`RATE_LIMITED`。5 个 Phase 9 Operation 的依赖故障固定返回 `SIGNALING_UNAVAILABLE` 或 ICE 端点专用 `TURN_UNAVAILABLE`，不使用通用 `SERVICE_UNAVAILABLE`。第三方或错误绑定统一 404；已确认参与方后才可返回 409/410。OpenAPI 对这 5 个 Operation 显式列出冻结状态码，不复用会额外暴露 403/413/422 的通用认证错误锚点；DELETE 仍只允许 204/400/401/404/429/503。

create-or-attach 与 ice-servers 使用公共 24 小时 Idempotency Record；Secret Replay 密文只保留到 signaling credential issue deadline，随后留 Tombstone。DELETE 天然幂等。握手为每 Session/Role 6/分钟 burst 2，并受 Account/Guest/IP 聚合限制；ICE Server 端点为每 Session/Role 3/分钟、Account 30/分钟、Guest/IP 10/分钟。HTTP 429/503 带 Retry-After，不公开命中维度。
