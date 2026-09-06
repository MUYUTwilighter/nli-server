# NetherLink v2 公共约定

> 状态：`PHASE_0_COMPLETE`
>
> 本文档记录所有 v2 HTTP 模块共享的公共约定。模块级细节由后续子设计补充，但不得在本文件之外自行定义冲突规则。

## 目标

统一定义：

- API 版本入口；
- 公共 ID 与安全凭据的表示；
- JSON 命名和基础数据格式；
- 时间与时长表示；
- 成功和失败响应结构；
- 鉴权、幂等、分页、限流、日志和通知的公共行为。

## 非目标

本文档不负责：

- 枚举所有模块业务错误码；
- 设计具体数据库表；
- 定义 Account、Provider、Friendship 或 Game Instance 的完整字段；
- 定义 P2P 信令协议。

## 规范用语

- “必须”：实现和客户端均不得违反；
- “应”：默认遵循，偏离时必须在对应模块文档说明原因；
- “可以”：可选能力，不得作为其他功能正常工作的隐含前提。

## API 版本

状态：`CONFIRMED`

v2 HTTP API 使用 URL 路径版本：

```text
/v2/*
```

公共健康检查、指标等运维端点是否进入 `/v2`，在 REST API 阶段单独确定。

## ID 与安全凭据

状态：`CONFIRMED`

### 公共资源与会话 ID

公共资源与会话标识统一使用 UUIDv4，例如：

- NLI Account ID；
- Provider Binding ID；
- Game Instance ID；
- Friend Request ID；
- Join Request ID；
- Guest ID；
- WS Session ID。

HTTP JSON 中 UUID 使用标准小写连字符字符串：

```json
"550e8400-e29b-41d4-a716-446655440000"
```

规则：

- UUID 只用于唯一寻址，不作为访问凭据；
- 客户端不得根据 UUID 推断权限或资源类型；
- 服务端必须在执行操作前重新验证资源归属和权限；
- 数据库内部是否额外使用其他键，不得影响公共 API。

### 安全凭据

以下内容不使用 UUID 直接充当秘密：

- Access Token；
- Refresh Token；
- Device Code Secret；
- 实例邀请码；
- 代理发布授权码；
- 一次性密码重置凭据。

安全凭据必须使用独立的高熵不透明随机值或安全签名格式。仅知道资源 UUID 不能获得任何权限。

## JSON

状态：`CONFIRMED`

HTTP JSON 字段统一使用 `snake_case`：

```json
{
  "account_id": "550e8400-e29b-41d4-a716-446655440000",
  "display_name": "Player",
  "created_at": 1788163200000
}
```

规则：

- 枚举值默认使用稳定的 `UPPER_SNAKE_CASE`；
- 公共字段改名视为 API 契约变化；
- 服务端必须拒绝请求中的未知 JSON 字段，避免拼写错误或过期客户端字段被静默忽略；
- 客户端必须忽略响应中的未知字段，以允许 v2 响应进行向后兼容扩展；
- 在不提供通用 PATCH 的前提下，`null` 只有在 Schema 明确允许时才表示空值，字段缺失按对应请求 Schema 的 required 规则处理。

## 时间与时长

状态：`CONFIRMED`

时间使用 Unix 毫秒整数表示，并在 OpenAPI 中使用 64 位整数：

```json
{
  "created_at": 1788163200000,
  "expires_at": 1788163290000
}
```

公共规则：

- Unix 时间戳以 UTC Unix Epoch 为基准；
- 字段名使用 `_at` 表示绝对时间；
- 相对时长默认使用整数毫秒，并使用 `_ms` 后缀；
- 时间字段必须能安全表示在有符号 64 位整数中；
- 客户端不能依赖本地时钟决定服务端授权是否有效；
- 服务端返回的 `expires_at` 是权威过期时间；
- HTTP 标准头或明确采用的外部协议若规定了其他单位或格式，遵循对应标准，不受 JSON 毫秒规则影响；此类例外必须在模块文档中标明。

示例：

```json
{
  "expires_at": 1788163290000,
  "retry_after_ms": 5000
}
```

## 成功响应

状态：`CONFIRMED`

成功响应不使用统一的 `data` Envelope。

单资源响应直接返回资源或操作结果：

```json
{
  "account_id": "550e8400-e29b-41d4-a716-446655440000",
  "display_name": "Player"
}
```

列表响应使用统一外层结构：

```json
{
  "items": [],
  "next_cursor": null
}
```

规则：

- `204 No Content` 不返回 JSON Body；
- 资源创建统一返回 `201 Created`，并在存在稳定资源 URL 时返回 `Location`；
- 操作型端点需要返回资源还是结果对象，由对应模块文档确定。

## 错误响应

状态：`CONFIRMED_BASE_STRUCTURE`

普通 v2 HTTP API 错误响应采用 RFC 9457 Problem Details，媒体类型为：

```text
application/problem+json
```

在标准字段之外增加稳定的 NLI `code` 和 `request_id`：

```json
{
  "type": "urn:netherlink:problem:rate_limited",
  "title": "Too Many Requests",
  "status": 429,
  "detail": "Too many requests",
  "code": "RATE_LIMITED",
  "request_id": "550e8400-e29b-41d4-a716-446655440000"
}
```

规则：

- `status` 必须与实际 HTTP 状态码一致；
- `code` 是供客户端稳定判断的机器码，使用 `UPPER_SNAKE_CASE`；
- `detail` 面向人类，不得作为客户端分支条件；
- `request_id` 用于日志关联，不得包含隐私或认证信息；
- 错误响应不得返回内部异常、SQL、Provider 凭据或调用栈；
- `type` 使用稳定 URN：`urn:netherlink:problem:<lower_snake_case_code>`；
- `title` 和 `detail` 由服务端固定使用英文；客户端使用稳定 `code` 显示本地化文本；
- 字段级错误通过可选 `errors` 数组返回，每项包含 JSON Pointer `field`、稳定 `code` 和英文 `message`。

明确采用 OAuth、Device Authorization 等外部标准协议的端点，可以按对应协议返回标准错误结构和错误码；这种例外必须在模块文档与 OpenAPI 中明确，不能扩散到普通业务 API。

字段错误示例：

```json
{
  "type": "urn:netherlink:problem:validation_failed",
  "title": "Unprocessable Content",
  "status": 422,
  "detail": "One or more fields are invalid",
  "code": "VALIDATION_FAILED",
  "request_id": "550e8400-e29b-41d4-a716-446655440000",
  "errors": [
    {
      "field": "/email",
      "code": "INVALID_EMAIL",
      "message": "The email address is invalid"
    }
  ]
}
```

## Request ID

状态：`CONFIRMED`

每个 HTTP 请求都由服务端生成一个新的 UUIDv4 Request ID：

- 服务端不接受客户端提供的 `X-Request-ID` 作为权威请求标识；
- 所有响应都通过 `X-Request-ID` Header 返回该 ID；
- RFC 9457 错误响应还必须在 `request_id` 字段中返回相同 ID；
- Request ID 只用于请求日志和问题排查，不表示幂等关系；
- Request ID 不得包含用户、Token、IP 或其他隐私信息。

未来如需分布式追踪，可以另行支持 W3C `traceparent`，不能改变 Request ID 的服务端生成规则。

## HTTP 鉴权与 Token Audience

状态：`CONFIRMED_BASE_STRUCTURE`

所有 Access Token 统一通过以下 Header 传递：

```http
Authorization: Bearer <access_token>
```

Audience 用于区分不同的安全域，而不是为每个普通业务 API 设计细粒度 Scope：

- `nli_account`：NLI Account 的 Account Session 与 Instance Session；可以调用全部普通 NLI Account API，具体操作仍需通过账号身份、会话状态、资源归属和业务关系鉴权；
- `nli_guest`：匿名联机 Guest Session；只能执行匿名加入流程明确允许的操作；
- `nli_admin`：为未来管理后台保留；普通 NLI Account Token 和 Guest Token 不能访问管理接口。

Account Session 与 Instance Session 使用相同 Token 格式、`nli_account` Audience 和普通 API 权限。两者名称只表示会话生命周期和实例绑定状态：

- Account Session 尚未绑定 Game Instance；
- 一个 Token Family 绑定并管理 Game Instance 后，作为 Instance Session 使用；实例关闭并解绑后恢复为 Account Session；
- 每个 NLI Account Token Family 都具有服务端签发的临时 Session ID；
- 一个 Token Family 同一时刻最多绑定一个 Game Instance，但解绑后可以复用于后续实例；
- Session ID 用于区分同一账号的并发临时会话，不能代替持久 Account ID。

普通 NLI Account API 当前不使用细粒度 Scope。服务端不能因为 Audience 有效就跳过资源归属、好友关系、ban、实例状态等业务鉴权。未来 `nli_admin` 如需 Role 或 Scope，在独立管理后台设计中定义，不能扩张普通用户 Token 的权限。

Guest Session 使用独立 `nli_guest` Audience 和 Guest ID，并与匿名用户当前游戏运行时的生命周期绑定；结束后立即失效，不能访问账号、好友、Provider 或普通实例管理 API。Guest 只获得短期 Access Token，不签发长期 Refresh Token。

其他公共规则：

- Refresh Token 不能作为普通 Bearer 凭据访问业务资源，只能提交给专用 Token Refresh 端点；
- Access Token 和 Refresh Token 不得出现在 URL、Query、日志或错误响应中；
- Device Code 验证网页所需的内部浏览器 Cookie 不属于公共 API Access Token 传递规则；
- Token Audience 不匹配时必须拒绝，不能尝试把 Token 降级解释为其他 Principal。

通用鉴权失败：

- 缺少、格式错误、Audience 不匹配、无效、过期或已撤销的 Access Token 返回 `401 Unauthorized`；
- 身份有效但不拥有目标资源或不满足业务权限时返回 `403 Forbidden`；
- 具体 Token Claims、Session 状态和 Guest 生命周期在 `nli_account.md` 与 `game_instance.md` 中冻结。

## 幂等与网络重试

状态：`CONFIRMED_BASE_STRUCTURE`

只有模块文档明确标记为需要幂等保护的非天然幂等写操作，才强制要求：

```http
Idempotency-Key: <opaque-client-generated-key>
```

典型适用操作包括资源创建、发送好友申请、创建 Provider 同步任务等。天然幂等的 PUT、DELETE 和只读操作不强制携带。

公共规则：

- Key 在“认证 Principal + HTTP Method + 规范化路由 + Key”范围内唯一；
- Key 是客户端生成的不透明值，最大长度为 128 个 ASCII 字符，不能包含空白或控制字符；
- 服务端必须保存规范化请求摘要；
- 同一个 Key 和同一个请求重复提交时，返回第一次已完成操作的相同业务结果；
- 同一个 Key 配合不同请求内容时返回 `409 Conflict` 和稳定错误码；
- 相同请求仍在处理中时，返回可重试的冲突结果，不能并发执行两次副作用；
- 默认幂等记录保留 24 小时，模块可以声明更长时间；
- Request ID 与 Idempotency Key 相互独立，每次网络请求都有新的 Request ID。

Refresh Token 轮换必须支持短期安全重试：同一个 Refresh Token 与 Idempotency Key 在规定短窗口内重试时，返回同一轮换结果。短窗口结束后的旧 Refresh Token 重用按 Token 重用风险处理。具体窗口、加密暂存和 Token Family 行为在 `nli_account.md` 中冻结。

## 分页与排序

状态：`CONFIRMED`

集合查询统一使用不透明 Cursor：

```http
GET /v2/example?limit=20&cursor=<opaque-cursor>
```

列表响应：

```json
{
  "items": [],
  "next_cursor": null
}
```

公共规则：

- `limit` 默认值为 20，最大值为 100；
- Cursor 由服务端生成，客户端不得解析、修改或构造；
- Cursor 必须绑定原查询的过滤和排序条件；
- 无效、过期或与当前查询条件不匹配的 Cursor 返回 `400 Bad Request`；
- 每个端点必须定义稳定排序键，并使用资源 UUID 作为最终并列排序键；
- Cursor 使用最后排序键继续查询，不要求服务端维持完整结果快照；
- 并发数据变化时允许弱一致，客户端应按资源 ID 去重；
- 需要获取最新完整结果时，客户端从不带 Cursor 的第一页重新查询；
- 除非模块文档明确声明，客户端不能指定任意排序字段。

Provider 聚合好友等复杂集合如何编码来源级游标，在对应模块中细化，但必须保持同一 `items/next_cursor` 外层契约。

## HTTP 写操作与通用状态码

状态：`CONFIRMED_BASE_STRUCTURE`

写操作使用以下风格：

- `POST`：创建资源或执行具有明确名称的状态转换命令；
- `PUT`：完整替换一个可配置资源；
- `DELETE`：删除、撤销或关闭资源；
- v2 不提供具有全局语义的通用 `PATCH`。

模块需要部分更新时，应优先提供语义明确的命令端点，而不是自行发明 PATCH 规则。PUT 请求必须满足完整资源配置 Schema；字段缺失不表示“保持原值”。

通用失败映射：

- `400 Bad Request`：JSON 无法解析、字段类型错误或请求结构不符合 Schema；
- `401 Unauthorized`：缺少或无效的身份凭据；
- `403 Forbidden`：身份有效但没有目标操作权限；
- `404 Not Found`：调用者可见范围内不存在目标资源；
- `409 Conflict`：请求与资源当前状态、唯一约束或同一幂等键的既有请求冲突；
- `422 Unprocessable Content`：请求结构有效，但字段值或字段组合不满足业务语义；
- `429 Too Many Requests`：超过限流；
- `500 Internal Server Error`：未预期的服务端故障；
- `502 Bad Gateway`：Provider 等上游返回无效响应；
- `503 Service Unavailable`：所需依赖或上游暂时不可用。

资源创建成功统一返回 `201 Created`，并在能够形成稳定资源 URL 时返回 `Location` Header。命令型 POST 根据结果返回 `200` 或 `204`。

DELETE 采用天然幂等语义：如果服务端仍能安全确认调用者对该资源范围具有权限，则资源已经不存在时也返回 `204 No Content`；如果无法在不泄漏资源存在性的前提下确认权限，可以返回 `404 Not Found`。

通用写并发不强制使用 ETag。服务端必须在事务或原子操作中重新校验当前状态；状态已经变化且无法应用请求时返回 `409 Conflict`。确实需要版本条件的模块可以另行定义，但不能把版本字段作为所有资源的公共负担。

## 限流

状态：`CONFIRMED_BASE_STRUCTURE`

超过限流时返回 `429 Too Many Requests`、RFC 9457 错误和 `Retry-After` Header：

```http
Retry-After: 5
```

错误 Body 可以同时提供更精确的毫秒值：

```json
{
  "type": "urn:netherlink:problem:rate_limited",
  "title": "Too Many Requests",
  "status": 429,
  "detail": "Too many requests",
  "code": "RATE_LIMITED",
  "request_id": "550e8400-e29b-41d4-a716-446655440000",
  "retry_after_ms": 5000
}
```

规则：

- `Retry-After` 使用 HTTP 规定的秒数，无法整除时向上取整；
- v2 默认不返回剩余额度、桶容量或完整 RateLimit Headers；
- 具体模块可以按 Account、Instance、Guest、IP、Provider、目标资源或其组合限流；
- 对外错误不能暴露内部桶键或允许攻击者推断敏感账号是否存在；
- 具体阈值由模块文档和部署配置确定。

## 日志、审计和隐私

状态：`CONFIRMED_BASE_STRUCTURE`

### 普通应用日志

普通应用日志默认不记录 HTTP 请求或响应 Body，只记录允许的结构化元数据，例如：

- 服务端生成的 Request ID；
- 路由模板、HTTP Method 和状态码；
- 延迟和响应大小；
- Principal 类型；
- 经过允许的内部资源 ID；
- 稳定错误码；
- Provider 类型和经过分类的上游结果。

日志应记录路由模板而不是包含敏感 Query 的原始 URL。

以下内容禁止进入普通日志：

- Authorization、Cookie 和 Set-Cookie；
- Access Token、Refresh Token、Provider 凭据；
- Device Code Secret、密码重置凭据；
- 实例邀请码和代理发布授权码；
- 完整邮箱、原始 IP 地址和客户端声明的 MC Profile；
- SDP、ICE Candidate 及其他未来信令敏感内容。

### 安全审计

安全审计与普通应用日志分离，使用追加式记录。适用事件至少包括：

- 密码和认证器变更；
- 密码重置；
- Account/Instance Session 撤销；
- Provider 绑定、重新验证、解绑和换绑；
- 代理发布授权码创建和撤销；
- 其他模块明确标记的高风险操作。

审计记录可以包含事件类型、时间、Request ID、Actor ID、Target ID、结果，以及调查所必需的规范化或哈希网络标识。普通应用日志不记录的 IP、邮箱或 MC Profile 不能未经评审直接以原值写入审计。

审计存储必须独立设置访问权限和保留策略。具体保留时间在部署与安全设计阶段确定。

## 已确认的跨模块约束

以下约束继承自 `outline.md`：

- HTTP 查询结果是权威业务状态；
- Phase 5 WebSocket 只用于通知与实例保活，不承担关键业务状态写入；Phase 9 可在 `signaling.md` 中定义唯一的独立短期信令 WS 例外，但不得复用通知通道、改变 HTTP 权威或扩大到其他业务写入；
- WebSocket 通知丢失后，客户端必须能通过 HTTP 恢复；
- NLI 只验证 NLI Account、Session 和权限，不验证客户端声明的 MC Profile；
- Account ID、Instance 归属、请求者类型、ACL Identity Traits 和 Source 等服务端推导字段不得信任客户端输入；
- Provider 操作必须通过统一 Provider 抽象和 Credential Manager；
- Provider 不可用不能阻断 NLI 原生账号、好友和联机功能。

## v1 经验处理

可以保留的经验：

- 使用稳定机器错误码；
- 请求身份和资源归属由服务端从认证上下文推导；
- 直接返回资源，不增加无必要的成功 Envelope；
- Token、凭据和上游错误不得写入响应或日志。

明确不继承：

- 使用 Minecraft Token 作为 NLI 身份；
- `X-Minecraft-Access-Token` 等 v1 专用 Header；
- v1 的 camelCase JSON；
- 仅包含 `code/message` 的旧错误 Envelope；
- 将认证 Token 生命周期等同于实例在线生命周期。

## Phase 0 决策批次

### 批次 2：请求与资源行为（已完成）

- [x] HTTP 鉴权与 Token Audience；
- [x] HTTP 方法和状态码；
- [x] 未知字段、`null` 和字段缺失；
- [x] Request ID。

### 批次 3：重试与集合查询（已完成）

- [x] 幂等键；
- [x] Token Refresh 重试；
- [x] 分页与排序；
- [x] 并发冲突。

### 批次 4：运维与隐私（已完成）

- [x] 限流响应；
- [x] 日志脱敏；
- [x] 审计边界；
- [x] 三个公共示例场景走查。

## 公共场景走查

以下路径只用于验证公共约定，具体资源命名仍由 `rest_api.md` 冻结。

### 账号查询

```http
GET /v2/accounts/{account_id}
Authorization: Bearer <access_token>
```

验证结果：

- Account ID 使用 UUIDv4；
- Access Token 必须具有 `nli_account` Audience，并通过账号可见性鉴权；
- 成功时直接返回 `snake_case` 账号对象；
- 时间字段使用 Unix 毫秒；
- 不存在或不可见时返回 RFC 9457 `404`；
- 响应携带服务端生成的 `X-Request-ID`；
- 普通日志不记录 Token 或响应 Body。

### 好友申请

```http
POST /v2/friend_requests
Authorization: Bearer <access_token>
Idempotency-Key: <opaque-key>
Content-Type: application/json

{
  "target_account_id": "550e8400-e29b-41d4-a716-446655440000"
}
```

验证结果：

- 请求方 Account ID 由 Access Token 推导，Body 不接受 requester 字段；
- 创建成功返回 `201` 和可用时的 `Location`；
- 相同 Key 重试返回同一结果；
- 相同 Key 配合不同目标返回 `409`；
- 向自己发送申请等语义错误返回 `422`；
- 未知请求字段返回 `400`。

### 实例发布

```http
POST /v2/instances
Authorization: Bearer <access_token>
Idempotency-Key: <opaque-key>
Content-Type: application/json

{
  "game_profile": {
    "source": "offline",
    "uuid": null,
    "username": "Player"
  },
  "approval_mode": "MANUAL",
  "acl": [
    {
      "priority": 100,
      "matcher": {
        "identity_types": ["DIRECT_FRIEND", "PROXY_FRIEND"],
        "sources": ["FRIEND_LIST"]
      },
      "action": "ALLOW"
    },
    {
      "priority": 200,
      "matcher": {
        "sources": ["INVITE_CODE"]
      },
      "action": "ALLOW"
    },
    {
      "priority": 1000,
      "matcher": {},
      "action": "DENY"
    }
  ]
}
```

验证结果：

- Instance 归属账号由 Token 推导，Body 不接受 owner Account ID；
- Token 必须具有 `nli_account` Audience，且对应 Token Family 尚未绑定其他实例；
- 第一次创建返回 `201`；
- 同一 Token Family 已绑定实例后再次使用不同 Key 创建返回 `409`；
- MC Profile 明确为 `CLIENT_CLAIMED`，只做结构清洗，不进入普通请求 Body 日志；
- ACL Matcher 空项表示任意，并按 priority 首条匹配；无匹配默认 DENY；
- 创建结果直接返回实例资源，不增加 `data` Envelope。

## Phase 0 完成条件

- [x] API 版本入口
- [x] 持久 ID 与安全凭据原则
- [x] JSON 命名
- [x] 时间与时长
- [x] 成功响应基础结构
- [x] 错误响应基础结构
- [x] HTTP 鉴权与 Token Audience
- [x] HTTP 方法和通用状态码
- [x] 未知字段与更新语义
- [x] 幂等和并发
- [x] 分页和排序
- [x] 限流响应
- [x] 日志、审计和隐私
- [x] 账号查询、好友申请、实例发布三个示例通过走查
