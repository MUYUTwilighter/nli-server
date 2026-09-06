# NetherLink v2 NLI Account 与认证

> 状态：`PHASE_1_COMPLETE / GATE_A_PASSED`
>
> 依赖：`outline.md`、`common.md`
>
> 本文档细化 NLI Account、认证器、NLI Account Session、Instance Session、Device Code、Token Family 和账号恢复。Provider 凭据内部实现留给 `provider.md`，Guest Session 的完整生命周期留给 `game_instance.md`。

## 目标

- 定义持久 NLI Account 身份；
- 定义邮箱、密码和 Provider 登录身份的认证关系；
- 定义统一 NLI Account Token 的签发、刷新、撤销和实例绑定；
- 定义 Account Session 与 Instance Session 的生命周期差异；
- 定义 NLI Device Code Flow；
- 定义邮箱验证、密码重置和账号恢复；
- 确保 Provider 不可用时用户仍可恢复 NLI Account。

## 非目标

- 不设计 Provider Adapter 和原始 Provider 凭据存储；
- 不设计好友、Game Instance 字段或 Join Request；
- 不设计 `nli_admin` 管理后台认证；
- 不在本阶段设计 Guest Session 的完整 API。

## 已确认的不变量

- Account ID 使用服务端生成的 UUIDv4，永久不可变；
- 邮箱是认证、通知和恢复信息，不是 Account ID；
- 普通 NLI Account Access Token 使用 `nli_account` Audience；
- 普通 NLI Account API 不使用细粒度 Scope；
- Account Session 与 Instance Session 使用相同 Token 格式和普通 API 权限；
- 每个 Token Family 具有服务端生成的临时 Session ID；
- 一个 Token Family 同一时刻最多绑定一个 Game Instance，实例关闭解绑后可以复用于后续实例；
- Account Session 绑定实例后作为 Instance Session 使用，解绑后恢复为 Account Session；
- Refresh Token 不能访问普通业务 API；
- Refresh Token 必须轮换并支持短期幂等重试；
- 密码重置后撤销账号现有 Account Session 与 Instance Session；
- Provider 不可用时，已验证邮箱仍可用于恢复账号；
- Guest 使用独立 `nli_guest` Audience，不属于本文件定义的 NLI Account Token Family。

## 设计批次

### 批次 1：账号、邮箱与密码（已完成）

已确认：

- [x] 使用唯一 `username` 公开定位账号；
- [x] 使用非唯一 `display_name` 展示昵称；
- [x] 每个有效账号必须具有已验证邮箱；
- [x] 邮箱整体大小写不敏感，不应用 Provider 特有别名合并规则；
- [x] 邮箱注册先创建 `PENDING_EMAIL` 账号，验证后转为 `ACTIVE`；
- [x] 只信任明确声明可靠已验证邮箱能力的 Provider；
- [x] 密码长度 10–128 字符，不设置大小写、数字和符号组合要求；
- [x] 密码哈希使用 Argon2id，并支持参数版本升级。

后续确认：

- [x] 密码登录只接受已验证邮箱，不使用 username 登录；
- [x] username 修改后进入 30 天冷却，旧 username 为原账号保留 90 天；
- [x] `PENDING_EMAIL` 有效期为 24 小时；
- [x] display_name 为 1–32 个 Unicode 字符，Trim 后不能为空并禁止控制字符；
- [x] 账号状态为 `PENDING_EMAIL / ACTIVE / DISABLED / DELETION_PENDING / DELETED`；
- [x] 主动删除使用 30 天恢复宽限期。

### 批次 2：Session 与 Token Family（已完成）

已确认：

- [x] Access Token 和 Refresh Token 都使用高熵不透明随机值；
- [x] Access Token 有效期为 15 分钟；
- [x] Refresh Token 空闲有效期为 30 天、绝对有效期为 90 天；
- [x] 每账号最多 10 个有效 Token Family，其中最多 5 个同时绑定 Game Instance；
- [x] Token Family 同时最多绑定一个实例，但实例关闭后可以解绑并复用于后续实例；
- [x] Refresh Token 轮换结果可在 60 秒内幂等重放；
- [x] 超出重放窗口的旧 Refresh Token 重用只撤销对应 Token Family，并通知用户；
- [x] Session ID 对账号本人可见，可用于查看和单独撤销会话。

后续确认：

- [x] Refresh 成功后旧 Access Token 立即失效；
- [x] Session 列表返回客户端自报名称、服务端时间和绑定状态，不返回 IP；
- [x] 达到 10 个 Token Family 上限时拒绝新登录，不自动踢出旧 Session；
- [x] 30 天空闲期只在成功 Refresh 时续期；
- [x] 支持撤销指定 Session，以及保留当前 Session 并撤销其他全部 Session。

### 批次 3：Device Code Flow（已完成）

- [x] 使用 RFC 8628 兼容字段和轮询错误语义；
- [x] User Code 使用 8 位 Crockford Base32，显示为 `XXXX-XXXX`；
- [x] 已登录用户必须显式确认并看到客户端 `client_name`；
- [x] Token Family 在 Mod 首次成功消费授权时创建；
- [x] 已消费结果可以在 60 秒内安全重放同一 Token Pair；
- [x] 过快轮询返回 `slow_down` 并增加后续轮询间隔。

### 批次 4：账号恢复与生命周期（已完成）

- [x] 邮箱验证使用 30 分钟高熵一次性链接；
- [x] 密码重置使用 30 分钟高熵一次性链接；
- [x] 主动修改密码保留当前 Session，撤销其他全部 Session；
- [x] 邮箱变更要求近期重新认证、验证新邮箱并通知旧邮箱；
- [x] `DELETION_PENDING` 恢复要求邮箱一次性链接并设置新密码；
- [x] 所有 `DISABLED` 账号只能通过人工流程恢复；
- [x] 重发新邮件凭据时立即使同用途旧凭据失效；
- [x] 认证、恢复和邮件接口必须按多维度限流并防止账号枚举。

## NLI Account

### 身份字段

```text
NliAccount
- account_id
- username
- display_name
- email
- email_verified_at
- status
- created_at
- updated_at
```

规则：

- `account_id` 是服务端生成的 UUIDv4，永久不可变；
- `username` 是公开、唯一、大小写不敏感的账号定位符；
- `display_name` 不要求唯一，只用于展示；
- `email` 是私有认证、通知和恢复信息，不进入公开账号读模型；
- 有效账号必须具有已验证邮箱；
- Provider 邮箱和 NLI 邮箱不能用于自动合并两个既有账号。

### Username

初始规范：

- 规范化后使用小写 ASCII；
- 允许 `a-z`、`0-9` 和下划线；
- 长度 3–24 个字符；
- 大小写不同不能注册为两个账号；
- 保留字和敏感冒充名称由服务端配置拒绝；
- 最终操作始终使用 Account ID，username 只用于定位候选账号。

Username 可以修改，但必须满足：

- 距离上次修改至少 30 天；
- 新 username 通过与注册相同的唯一性、保留字和格式校验；
- 旧 username 在修改后 90 天内为原账号保留，其他账号不能注册；
- 旧 username 在保留期内是否用于重定向搜索，由好友模块决定；
- 修改 username 不改变 Account ID、好友关系或 Session；
- 修改操作写入安全审计。

### Display Name

`display_name` 允许 Unicode，不参与唯一性、身份绑定或权限判断。

初始限制：

- Trim 后长度为 1–32 个 Unicode 字符；
- 禁止控制字符；
- 保存 Trim 后的值；
- 不执行唯一性校验；
- 不用于登录、绑定或权限判断。

### Email

规范化规则：

1. 去除首尾空白；
2. 规范化域名表示；
3. 唯一性比较时整体大小写不敏感；
4. 不移除 Gmail 点号；
5. 不移除 `+tag`；
6. 不应用任何邮箱 Provider 特有别名规则。

相同规范化邮箱同时只能关联一个未删除的 NLI Account。邮箱不得出现在公开账号查询、好友列表或普通日志中。

### 注册与激活

邮箱密码注册流程：

```text
提交 username、display_name、email、password
-> 原子检查 username/email 唯一性
-> 创建 PENDING_EMAIL 账号
-> 保存 Password Authenticator
-> 发送邮箱验证邮件
-> 原子消费验证凭据
-> 设置 email_verified_at
-> PENDING_EMAIL 转为 ACTIVE
```

`PENDING_EMAIL` 账号不能使用普通 NLI Account API，只能执行验证邮件重发、邮箱验证和取消注册等明确允许的操作。

`PENDING_EMAIL` 有效期为 24 小时：

- 在有效期内保留 username 和规范化邮箱；
- 到期后注册事务失效，验证凭据不能继续使用；
- 清理后释放 username 和邮箱；
- 重发验证邮件不能无限延长总有效期；
- 清理操作必须能够安全重试。

Provider 注册流程：

- 后端通过统一 Provider 抽象取得稳定 `Issuer + Subject`；
- 只有 Provider 明确声明可靠的已验证邮箱能力时，其邮箱才可以直接满足邮箱验证；
- 其他 Provider 邮箱仍需 NLI 自己发送验证邮件；
- Provider 返回相同邮箱不能自动登录或合并一个既有 NLI Account；
- Provider Login Identity 与是否绑定 Provider MC 账号是两个独立选择。

### 账号状态

```text
PENDING_EMAIL -> ACTIVE
PENDING_EMAIL -> DELETED        # 注册过期或取消
ACTIVE -> DISABLED              # 安全、管理或用户停用
DISABLED -> ACTIVE              # 仅人工恢复流程
ACTIVE -> DELETION_PENDING      # 用户主动删除
DELETION_PENDING -> ACTIVE      # 宽限期内通过邮箱恢复
DELETION_PENDING -> DELETED     # 30 天宽限期结束
```

规则：

- `ACTIVE` 才能正常使用 NLI Account API；
- `DISABLED` 必须记录不公开的原因；所有 `DISABLED` 账号只能通过人工流程恢复，公开 API 不提供自助恢复；
- 进入 `DISABLED` 或 `DELETION_PENDING` 时立即撤销全部 NLI Account Token Family；
- `DELETION_PENDING` 账号立即从搜索、好友展示和实例查询中隐藏；
- 用户可以在 30 天内通过已验证邮箱恢复 `DELETION_PENDING` 账号；
- `DELETED` 表示业务删除已完成，跨模块数据清理和法定保留规则在数据模型阶段确定；
- 用户名和邮箱何时最终释放，以删除清理完成为准，不能在进入 `DELETION_PENDING` 时立即复用。

## Authenticator

认证器至少包括：

- Password Authenticator；
- Provider Login Identity；
- 已验证邮箱恢复能力。

### Password Authenticator

密码登录只接受规范化后的已验证邮箱。username 和 display_name 不能作为密码登录标识。无论邮箱是否存在、账号状态如何或密码是否正确，公开失败响应都必须避免泄漏账号存在性。

密码规则：

- 长度为 10–128 个 Unicode 字符；
- 不要求固定的大写、小写、数字或符号组合；
- 不得静默 Trim 或 Unicode 规范化用户密码；
- 实现必须设置合理的 UTF-8 字节上限，防止异常输入造成资源消耗；
- 密码、密码哈希和验证中间值不得进入日志或错误响应。

密码使用 Argon2id 哈希：

- 每个密码使用独立随机 Salt；
- 保存算法、参数和版本信息；
- 参数由部署配置决定，但必须满足项目最低安全基线；
- 用户成功登录时，可以将旧参数哈希渐进升级到当前参数；
- 不保存可逆密码。

Provider Login Identity 只保存稳定认证映射；原始 Provider Token 和 Credential Manager 不在本文件中定义。

## Session 与 Token Family

### 标识与存储边界

需要区分：

- 持久 Account ID；
- 临时 Session ID；
- Token Family；
- 当前 Access Token；
- 当前 Refresh Token；
- 可选绑定的 Game Instance ID。

Access Token 和 Refresh Token 都使用至少 256 bit 随机熵的高熵不透明值。服务端长期只保存 Token 的密码学哈希以及验证所需元数据，不保存可直接使用的原始 Token。唯一例外是 Refresh 和 Device Code 成功响应的 60 秒幂等重放：该结果必须短期加密保存、严格限制访问，并在窗口结束后立即删除。Audience、Account ID、Session ID、状态和实例绑定由服务端 Token 记录确定，不信任客户端声明。

Session ID 是 UUIDv4 非秘密标识：

- 账号本人可以在会话管理中查看；
- 可以用于撤销自己的指定 Session；
- 不能作为 Bearer 凭据；
- 不能用于访问其他账号的会话信息。

### Account Session 与 Instance Session

两者不代表不同 API 权限，只表示同一个 Token Family 当前是否绑定 Game Instance：

```text
ACCOUNT_SESSION <-> INSTANCE_SESSION
       bind instance     close/unbind instance
```

规则：

- 未绑定实例时作为 Account Session；
- 创建实例时原子检查账号实例额度和当前绑定状态，然后绑定 Instance ID；
- 已绑定实例时不能创建第二个并发实例；
- 实例正常关闭后解除绑定，Token Family 恢复为 Account Session；
- 同一个 Token Family 之后可以创建新的实例；
- 撤销、过期或标记为泄漏的 Token Family 不能通过解绑恢复；
- 账号最多同时存在 10 个有效 Token Family，其中最多 5 个处于 Instance Session 状态。

实例异常断线只影响实例在线租约，不自动撤销 NLI Token Family。实例何时自动关闭和解除绑定在 `game_instance.md` 中确定。

### Session 管理

账号本人可以查看自己的 Session 列表。每项至少返回：

- `session_id`；
- 客户端自报的 `client_name`；
- `created_at`；
- `last_refreshed_at`；
- `absolute_expires_at`；
- 可选 `bound_instance_id`；
- 是否为当前请求 Session。

`client_name` 仅用于展示，由客户端声明；Trim 后长度限制为 1–64 个 Unicode 字符并禁止控制字符，不能用于设备认证。普通 Session 列表不返回最近 IP。

会话管理支持：

- 撤销指定 Session ID；
- 保留当前 Session，并撤销同账号其他全部 Session；
- 撤销当前 Session 后，当前 Access Token 立即失效；
- 撤销绑定实例的 Session 时，对应实例和 WebSocket 必须进入关闭流程；
- 创建新 Token Family 时，如果账号已有 10 个有效 Family，则返回 `SESSION_LIMIT_REACHED`，不自动撤销旧会话。

### Token 生命周期

- Access Token 有效期：15 分钟；
- Refresh Token 空闲有效期：30 天；
- Token Family 绝对有效期：90 天；
- 只有成功 Refresh 才更新空闲过期时间，普通 Access Token API 请求不续期；
- 更新后的空闲过期时间不能超过绝对过期时间；
- 绝对过期后必须重新登录或重新执行 Device Code Flow；
- 每次刷新都签发新的 Access Token 和 Refresh Token；
- Refresh 成功提交后，旧 Access Token 与旧 Refresh Token 立即退出当前有效集合；
- 旧 Access Token 不提供并发宽限期；客户端刷新时应暂停使用旧 Token 发起新请求；
- 原始 Token 只在签发响应中返回一次。

### Refresh Token 轮换和重试

正常刷新：

```text
提交当前 Refresh Token + Idempotency-Key
-> 验证 Token Family 状态和过期时间
-> 原子轮换 Access Token 与 Refresh Token
-> 在受保护的短期重放记录中保存本次结果
-> 返回新 Token Pair
```

规则：

- 相同旧 Refresh Token 与相同 Idempotency Key 在 60 秒内重试时返回同一轮换结果；
- 同一旧 Refresh Token 配合不同请求上下文不能得到另一组新 Token；
- 60 秒后再次使用已经轮换的旧 Refresh Token，视为重用风险；
- 检测到重用后将对应 Token Family 标记为 `COMPROMISED` 并撤销；
- 不自动撤销账号的其他 Token Family；
- 通过安全通知和审计记录告知用户；
- 短期重放数据必须受保护，过期后立即删除。

短期幂等重放返回首次刷新已经签发的同一组新 Token，不再次轮换。

## Device Code Flow

### 创建授权

Mod 请求 Device Code 时提交经过清理的 `client_name`。服务端返回 RFC 8628 兼容字段：

```json
{
  "device_code": "<high-entropy-secret>",
  "user_code": "ABCD-EFGH",
  "verification_uri": "https://example.invalid/device",
  "verification_uri_complete": "https://example.invalid/device?user_code=ABCD-EFGH",
  "expires_in": 600,
  "interval": 5
}
```

说明：

- `device_code` 是高熵秘密，只提供给 Mod；
- `user_code` 使用 8 位 Crockford Base32，显示时分为 `XXXX-XXXX`；
- 服务端只保存两者的安全哈希和流程元数据；
- Device Code 总有效期为 10 分钟；
- 初始最小轮询间隔为 5 秒；
- `expires_in` 和 `interval` 遵循 RFC 8628 的秒单位，是公共 Unix 毫秒 JSON 约定的协议例外。

### 浏览器确认

用户打开验证页面后：

1. 登录自己的 NLI Account；
2. 输入或确认 User Code；
3. 查看请求授权的 `client_name`；
4. 显式选择批准或拒绝。

登录后不能自动批准。服务端可以对会话过旧或其他高风险情况要求重新认证，但不要求所有批准都重新输入密码。

User Code 查询必须按 IP、账号和 Code 组合限流；错误响应不能泄漏 Code 是否已被其他账号批准。

### 状态机

```text
PENDING -> APPROVED -> CONSUMED
        -> DENIED
        -> EXPIRED
APPROVED -> EXPIRED
CONSUMED -> EXPIRED             # 仅保留短期结果重放
```

规则：

- 批准、拒绝、消费和过期转换必须原子执行；
- 同一个授权只能绑定一个 NLI Account；
- Token Family 不在批准时创建；
- Mod 首次成功消费时重新检查账号状态和 10 个 Token Family 上限；
- 达到上限时不创建 Token Family，授权在剩余有效期内保持 APPROVED，用户释放旧 Session 后可以再次轮询；
- 消费成功后创建新的 Account Session Token Family；
- Device Code 本身不能用于普通 API。

### Mod 轮询

轮询错误使用 RFC 8628 兼容错误码：

- `authorization_pending`；
- `slow_down`；
- `access_denied`；
- `expired_token`。

客户端快于允许间隔轮询时返回 `slow_down`，并增加后续最小间隔；持续违规时可以进一步返回 429 或使授权失效。

首次成功消费后，服务端在受保护的短期记录中保存 Token Pair。相同 Device Code 在 60 秒内因网络丢包重试时返回同一 Token Pair，不创建第二个 Token Family；60 秒后不再返回原始 Token。

## Token Family 状态机

```text
ACTIVE <-> ACTIVE_BOUND
  |            |
  +-------> REVOKED
  +-------> COMPROMISED
  +-------> EXPIRED
```

说明：

- `ACTIVE`：有效且未绑定实例，对应 Account Session；
- `ACTIVE_BOUND`：有效且绑定一个实例，对应 Instance Session；
- `REVOKED`：用户、密码重置或安全操作主动撤销；
- `COMPROMISED`：检测到超出安全重放窗口的 Refresh Token 重用；
- `EXPIRED`：达到 Refresh 空闲过期或 Token Family 绝对过期；
- `ACTIVE_BOUND -> ACTIVE` 只在实例关闭或合法解绑后发生；
- 所有终止状态都不能恢复为 ACTIVE。

## 邮箱验证与账号恢复

### 一次性邮件凭据

邮箱验证、密码重置、邮箱变更和删除恢复均使用用途隔离的高熵一次性链接凭据：

- 原始 Token 至少具有 256 bit 随机熵；
- 服务端只保存密码学哈希、用途、目标账号、目标邮箱和过期时间；
- 邮箱验证和密码重置链接有效期均为 30 分钟；
- Token 只能用于签发时指定的用途；
- 原子消费成功后立即失效；
- 同一账号和同一用途重新签发时，旧 Token 立即失效；
- 错误、过期或已消费响应不能泄漏账号和邮箱状态；
- 原始 Token 不进入日志、审计详情或 URL Query 之外的分析系统。

### 邮箱验证

```text
PENDING_EMAIL 注册
-> 发送 30 分钟验证链接
-> 原子消费
-> 验证目标邮箱仍属于该注册事务
-> 设置 email_verified_at
-> Account 转为 ACTIVE
```

验证链接过期后可以在 24 小时注册总有效期内重发；重发不能延长注册总有效期。

### 密码修改

已登录用户主动修改密码时：

- 要求近期重新认证；
- 已有密码的账号默认验证当前密码；
- 仅有 Provider Authenticator 的账号通过对应 Provider 或邮箱邮件完成重新认证；
- 成功后保留当前 Session；
- 撤销同账号其他全部 Token Family；
- 写入安全审计并发送安全通知。

### 密码重置

```text
提交邮箱
-> 始终返回相同公开响应
-> 对符合条件的账号发送 30 分钟一次性链接
-> 用户设置新密码
-> 原子消费重置 Token
-> 更新 Password Authenticator
-> 撤销账号全部 Token Family
-> 写入安全审计并发送通知
```

仅通过 Provider 注册的账号也可以使用此流程设置首个 NLI 密码。

### 邮箱变更

邮箱变更流程：

1. 用户完成近期重新认证；
2. 提交新邮箱；
3. 原子检查规范化新邮箱未被使用或保留；
4. 向新邮箱发送 30 分钟验证链接；
5. 验证成功后切换账号邮箱并更新时间；
6. 向旧邮箱发送安全通知；
7. 写入安全审计。

新邮箱验证前，旧邮箱继续作为当前登录和恢复邮箱。旧邮箱不需要批准变更，以允许用户在失去旧邮箱访问权时完成换绑。

### 删除恢复

进入 `DELETION_PENDING` 时账号已经撤销全部 Session 并从公开查询中隐藏。30 天内恢复要求：

1. 向原已验证邮箱发送用途独立的一次性恢复链接；
2. 用户通过链接设置新密码；
3. 原子恢复账号为 `ACTIVE`；
4. 不恢复任何旧 Session、实例或短期授权；
5. 记录安全审计并发送恢复通知。

超过 30 天后进入 `DELETED`，不能通过公开恢复接口恢复。

## 认证、恢复与邮件限流

所有响应都必须避免暴露邮箱是否注册、账号状态或 Provider 绑定情况。

初始规则：

- 登录失败按规范化邮箱哈希与 IP 组合限流，并叠加 IP 总量限制；
- 初始限制为同一邮箱哈希与 IP 组合 15 分钟内最多 10 次失败，同一 IP 15 分钟内最多 50 次失败；
- 不能只根据失败次数永久锁定账号，避免攻击者造成账号拒绝服务；
- 同一用途邮件发送间隔至少 60 秒；
- 同一邮箱哈希每小时最多发送 5 封同用途邮件；
- 同一 IP 每小时最多触发 20 封认证或恢复邮件；
- Device Code 创建、User Code 查询和轮询分别限流；
- 达到限制时仍保持账号防枚举响应，只返回通用 429 与重试时间；
- 阈值由部署配置管理，但不能高于此处默认值而不经过安全评审。

## 安全审计与通知

至少记录：

- 注册完成；
- 密码设置、修改和重置；
- 邮箱变更；
- Token Family 创建、单独撤销和批量撤销；
- Refresh Token 重用；
- Device Code 批准、拒绝和消费；
- 账号进入或退出 `DISABLED`；
- 账号进入、恢复或完成 `DELETION_PENDING`。

通知至少覆盖密码、邮箱、Provider 登录身份、异常 Token 重用、账号禁用和删除恢复等安全敏感变化。审计与通知不得包含原始 Token。

## 安全场景走查

### 邮箱注册与验证

- 注册原子保留 username 和规范化邮箱并创建 `PENDING_EMAIL`；
- 验证链接 30 分钟过期，但注册事务总计只保留 24 小时；
- 重发链接使旧链接失效，不能延长 24 小时总期限；
- 成功消费后账号转为 `ACTIVE`，同一链接不能再次消费。

结果：通过。

### 登录失败与账号枚举

- 密码登录只接收邮箱；
- 不存在邮箱、错误密码、非 ACTIVE 状态使用一致的公开失败结构；
- 邮箱哈希与 IP 组合以及 IP 总量同时限流；
- 攻击者不能通过大量失败尝试永久锁定指定账号。

结果：通过。

### Device Code 批准与响应丢失

- 用户必须看到 `client_name` 并显式批准；
- Token Family 只在 Mod 首次消费时创建；
- Token 响应丢失后，60 秒内返回同一 Token Pair；
- 不会创建第二个 Session；
- 超过 Session 上限时不消费授权，用户可以先撤销旧 Session。

结果：通过。

### Refresh 响应丢失与旧 Token 重用

- 首次 Refresh 原子轮换 Token；
- 响应丢失后，相同 Idempotency Key 在 60 秒内得到同一结果；
- 旧 Access Token 在 Refresh 成功后立即失效；
- 超过窗口的旧 Refresh Token 重用将对应 Family 标记为 `COMPROMISED`；
- 其他设备 Session 不被自动撤销，用户收到安全通知。

结果：通过。

### Instance Session 绑定与复用

- Account Session 创建实例时原子绑定 Instance ID；
- 已绑定时创建第二个实例返回冲突；
- 实例关闭后合法解绑并恢复为 Account Session；
- 同一 Token Family 可以创建后续实例，但同一时刻只管理一个；
- 账号同时最多存在 5 个绑定实例。

结果：通过。

### Provider 不可用时恢复

- 每个有效 NLI Account 都有已验证邮箱；
- 即使唯一 Provider 登录失效，用户仍可请求密码重置邮件；
- Provider-only 用户可以通过重置流程设置首个 NLI 密码；
- 重置完成后全部旧 Token Family 被撤销。

结果：通过。

### 密码修改、邮箱变更和删除恢复

- 修改密码要求近期重新认证，保留当前 Session 并撤销其他 Session；
- 邮箱变更先验证新邮箱，再切换并通知旧邮箱；
- 删除立即撤销全部 Session 并隐藏账号；
- 30 天内恢复必须通过原邮箱链接设置新密码，不恢复旧 Session；
- `DISABLED` 账号不能通过公开恢复流程绕过人工处理。

结果：通过。

## Phase 1 完成条件

- [x] Account ID、公开账号定位、邮箱和账号状态冻结
- [x] 注册、邮箱验证和密码认证冻结
- [x] Account/Instance Session 边界冻结
- [x] Token 格式、生命周期、轮换与重用检测冻结
- [x] Device Code Flow 冻结
- [x] 密码重置与账号恢复冻结
- [x] 认证与恢复限流及防枚举规则冻结
- [x] 成功、拒绝、过期、重放和 Provider 不可用场景通过走查
- [x] Gate A 评审通过
