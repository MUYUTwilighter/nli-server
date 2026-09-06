# NetherLink v2 Provider 抽象与凭据管理

> 状态：`PHASE_2_COMPLETE / GATE_B_PASSED`
>
> 依赖：`outline.md`、`common.md`、`nli_account.md`，Gate A 已通过。
>
> 本文档定义 Provider Registry、Capability、Adapter、Credential Manager、Provider Login Identity、Provider Binding、错误分类和重新验证边界。

## 目标

- 为 Microsoft、第三方 Yggdrasil 站点和未来来源提供统一 Provider 抽象；
- 让业务模块只依赖受控 Adapter，不接触原始 Provider 凭据；
- 根据 Provider 能力安全降级；
- 明确登录身份与业务绑定的区别；
- 定义凭据失效、重新验证、解绑和换绑；
- 支持有后台授权和无后台授权两类 Provider；
- 确保 Provider 故障不影响 NLI 原生功能。

## 非目标

- 不定义具体 Microsoft 或 LittleSkin API 请求细节；
- 不定义好友聚合和同步状态机；
- 不定义 Game Instance 或 Provider 官方实例的客户端读模型；
- 不允许业务模块代理任意 Provider HTTP 请求。

## 已确认的不变量

- Provider 是能力来源，不代表固定协议；
- 不支持的能力返回稳定 `UNSUPPORTED`；
- Provider 临时故障不改变 Binding 状态；
- 凭据明确失效或授权被撤销时进入 `REAUTH_REQUIRED`；
- 同一 `Provider + Subject` 同时只能绑定一个有效 NLI Account；
- Provider 返回相同邮箱不能自动合并既有 NLI Account；
- Provider Login Identity 与 Provider 业务绑定是独立选择；
- Provider Binding 不验证 Game Instance 声明的 MC Profile；
- 所有 Provider 操作必须经过统一 Adapter 和 Credential Manager；
- 业务模块不能读取、刷新、保存或记录原始 Provider 凭据。

## 设计批次

### 批次 1：Provider Registry、Capability 与 Adapter（已完成）

- [x] Provider 只能由服务端管理员注册和启用；
- [x] Provider ID 使用稳定小写 slug；
- [x] 有效 Capability 是 Provider 声明、Binding 用途开关和实际 Credential Grant 的交集；
- [x] Adapter 使用强类型异步方法；
- [x] Adapter 采用编译期实现和运行时配置实例；
- [x] 一个 NLI Account 对同一 Provider 最多建立一个业务 Binding；
- [x] Provider Login Identity 与业务 Binding 保持独立，通过 `Provider + Issuer + Subject` 关联。

### 批次 2：Credential Manager（已完成）

- [x] 业务模块只调用统一 ProviderService，由其协调 Registry、Credential Manager 和 Adapter；
- [x] 长期凭据使用版本化主密钥和每记录独立 AEAD Nonce 加密；
- [x] Refresh Grant 加密持久化，Access Token 只短期缓存；
- [x] 同一 Binding 的刷新使用进程内单飞和多节点短租约锁；
- [x] Access Token 按需惰性刷新并预留过期安全窗口；
- [x] 无可持久 Refresh Grant 的 Provider 只允许用户在场时主动操作；
- [x] 解绑时本地权限立即失效，上游撤销尽力执行。

### 批次 3：Binding 状态、错误与重验证（已完成）

- [x] Provider Registry 使用 `ENABLED / NO_NEW_BINDINGS / DISABLED`；
- [x] Binding 使用 `ACTIVE / REAUTH_REQUIRED / DISABLED`；
- [x] 只对读取和明确幂等操作自动重试最多 2 次，并使用指数退避和抖动；
- [x] 熔断按 Provider 与操作类别隔离；
- [x] 重新验证得到不同 Subject 时保持原 Binding 不变，必须显式换绑；
- [x] 解绑删除业务凭据和外部投影，但保留 NLI 好友与独立 Login Identity；
- [x] Binding 主动 DISABLED 时清除 Access Token 缓存，但保留加密 Refresh Grant；
- [x] 浏览器回调使用一次性 state、支持时的 PKCE、精确 Redirect 和 10 分钟事务 TTL。

### 批次 4：降级、Mock 与场景走查（已完成）

- [x] Provider 临时宕机不改变 Binding 状态，也不阻断 NLI 原生功能；
- [x] Provider 永久停用保留 Provider ID 和 Binding 记录，不自动删除 NLI 数据；
- [x] 后台授权失效只将对应 Binding 转为 `REAUTH_REQUIRED`；
- [x] Provider 限流遵循上游 Retry-After，不扩散为全局故障；
- [x] Mock Provider 支持能力开关、凭据轮换、错误注入和 Subject 切换；
- [x] 登录、绑定、好友读取、刷新竞争、重验证、解绑和故障场景完成走查。

## Provider Registry

Provider 只能由服务端管理员注册、配置和启用。普通用户和 Mod 不能向 NLI 后端提交任意 Provider Base URL 或动态注册 Provider。

未进入 Registry 的第三方登录站仍可由 Mod 在本地使用，但不能使用 NLI 后端的 Provider 登录、绑定、好友或官方实例能力。

Provider Registry 至少记录：

- `provider_id`；
- 展示名称；
- Adapter 类型；
- 配置状态；
- 声明的 Capability；
- 安全允许的 Endpoint；
- 是否允许新登录；
- 是否允许新业务 Binding；
- 创建与更新时间。

### Provider ID

Provider ID 使用稳定小写 slug，例如：

```text
microsoft
littleskin
example_yggdrasil
```

初始规则：

- 允许 `a-z`、`0-9` 和下划线；
- 必须以字母开头；
- 长度 2–32；
- 一经公开使用不得改变或复用为另一 Provider；
- Provider ID 是服务端配置标识，不属于普通用户创建资源，因此作为 UUIDv4 公共资源规则的明确例外。

同一种编译期 Adapter 可以由多个不同 Provider ID 配置实例化，但每个配置实例具有独立 Endpoint、Capability、状态和凭据命名空间。

即使 Provider 由管理员配置，也必须校验 Endpoint Scheme、Host 和重定向目标。默认要求 HTTPS；不得因重定向访问环回、链路本地或私有网络地址，明确的开发环境例外必须单独配置。

## Capability

初始能力：

- `AUTHENTICATE_NLI`
- `PROVIDE_VERIFIED_EMAIL`
- `READ_FRIENDS`
- `WRITE_FRIENDS`
- `READ_INSTANCES`
- `JOIN_INSTANCES`
- `MANAGE_SETTINGS`
- `BACKGROUND_REFRESH`

对于依赖业务 Binding 的操作，有效 Capability 按以下交集计算：

```text
effective_capabilities
    = provider_declared_capabilities
    ∩ binding_enabled_purposes
    ∩ credential_granted_capabilities
```

`AUTHENTICATE_NLI` 和 `PROVIDE_VERIFIED_EMAIL` 属于认证事务能力，不要求先存在业务 Binding。它们由 Provider Registry、Provider Login Identity 状态和本次认证结果共同约束，不能套用 Binding 用途开关。

规则：

- Provider Registry 声明 Adapter 理论支持的上限；
- Binding 用途开关表达用户主动允许的业务用途；
- Credential Grant 表达 Provider 实际授予的业务能力；
- 任意一层缺失时操作不得执行；
- 客户端请求参数不能临时扩大 Capability；
- Credential Grant 缩减时必须立即重新计算有效能力；
- Capability 不支持返回 `UNSUPPORTED`，用户关闭用途返回 `FORBIDDEN`，凭据授权不足按错误分类决定是否需要重验证。

## Provider Adapter

Adapter 采用编译期 Rust 实现，由运行时 Registry 配置实例化。不加载运行时动态代码，不允许 Adapter 以插件形式绕过服务端发布和安全评审。

业务模块使用强类型异步操作：

```text
begin_authentication
complete_authentication
verify_subject
read_friends
add_friend
remove_friend
read_instances
join_instance
read_settings
update_settings
refresh_credentials
revoke_credentials
```

每个方法必须使用明确的请求、响应和统一错误类型。业务模块不能向 Adapter 传递任意 URL、HTTP Method、Header 或未约束 JSON；Adapter 也不能向业务模块返回原始 Provider Token。

调用前的固定流程：

```text
业务模块请求 Provider 操作
-> Registry 解析 Provider 与 Adapter
-> 检查 Provider 配置状态
-> 计算 effective_capabilities
-> Credential Manager 获取受控调用上下文
-> Adapter 执行强类型操作
-> Adapter 返回强类型结果或统一错误
```

## Provider Login Identity

Provider Login Identity 用于把稳定的 `Provider + Issuer + Subject` 映射到一个 NLI Account。只用于登录时，不要求保存后台 Provider 凭据，也不自动启用好友或其他业务能力。

Provider Login Identity 至少保存 Provider ID、Issuer、Subject、Account ID、状态和时间信息。状态只需 `ACTIVE / DISABLED`：用户可以关闭 Provider 登录而不删除业务 Binding。

它与 Provider Binding 是独立实体：

- 用户可以只启用 Provider 登录，不启用好友等业务；
- 用户可以保留业务 Binding 但关闭 Provider 登录；
- 二者可以通过相同 `Provider + Issuer + Subject` 关联；
- 删除其中一个不能隐式删除另一个；
- 同一个 Provider Login Identity 同时只能属于一个有效 NLI Account。

## Provider Binding

Provider Binding 用于 NLI 账号主动启用 Provider 业务能力。一个 NLI Account 对同一 Provider 最多只能建立一个有效业务 Binding。

Binding 状态：

```text
ACTIVE -> REAUTH_REQUIRED
ACTIVE -> DISABLED
DISABLED -> ACTIVE
DISABLED -> REAUTH_REQUIRED
REAUTH_REQUIRED -> ACTIVE        # 同一 Subject 重新验证成功
任意非终止状态 -> UNBOUND       # 解绑终止，不可恢复
```

状态语义：

- `ACTIVE`：Binding 可按 Effective Capability 执行操作；
- `REAUTH_REQUIRED`：长期授权失效、被撤销或缺少必须重新同意的 Grant，暂停所有依赖凭据的操作；
- `DISABLED`：用户主动停用全部业务用途；清除短期 Access Token 缓存，但保留加密 Refresh Grant；
- `UNBOUND`：概念上的终止状态，本地业务凭据已经删除或密码学封存，不再作为有效 Binding 返回。

Provider 临时故障、限流和无效上游响应不能直接改变 Binding 状态。

这意味着：

- 一个账号不能同时绑定同一 Provider 的多个 Subject；
- 换用同一 Provider 的另一个 Subject 必须走换绑流程；
- 同一 `Provider + Issuer + Subject` 同时只能被一个有效 NLI Account Binding 使用；
- 不同 Provider 可以分别绑定。

字段范围：

- Binding ID；
- Account ID；
- Provider ID；
- Issuer；
- Subject；
- 展示信息缓存；
- 状态；
- 用户启用的用途；
- 最后验证时间；
- 最近分类错误；
- Credential 引用；
- 创建与更新时间。

用途开关至少覆盖：

- 读取好友；
- 修改好友；
- 读取 Provider 实例；
- 加入 Provider 实例；
- 修改 Provider 设置；
- 后台自动操作。

Provider 登录不属于 Binding 用途开关，由独立 Provider Login Identity 管理。

### Provider Registry 状态

```text
ENABLED
NO_NEW_BINDINGS
DISABLED
```

- `ENABLED`：允许新 Provider Login Identity、新 Binding 和既有操作；
- `NO_NEW_BINDINGS`：不允许创建新的 Login Identity 或 Binding，但既有 Login Identity 登录、重验证和既有 Binding 操作仍可继续；
- `DISABLED`：停止该 Provider 的登录、绑定、重验证和业务操作；不批量修改 Binding 自身状态，以便 Provider 恢复后继续使用；
- Registry 状态变化不能影响 NLI 邮箱登录、NLI 好友和 NLI 自有联机。

## ProviderService 与 Credential Manager

### ProviderService 边界

认证模块和业务模块只能调用统一 ProviderService。ProviderService 提供两类强类型入口：

```text
ProviderService.begin_authentication(
    provider_id,
    authentication_purpose,
    nli_session_context
) -> provider_transaction

ProviderService.complete_authentication(
    provider_transaction,
    callback_result
) -> verified_provider_identity | provider_error

ProviderService.execute_binding_operation(
    binding_id,
    required_capability,
    typed_operation
) -> typed_result | provider_error
```

登录、注册、创建 Binding、重新验证和换绑使用认证事务入口；好友、设置和 Provider 实例等业务使用 Binding 操作入口。

ProviderService 负责：

1. 根据操作加载 Provider Registry，以及需要时的 Login Identity 或 Binding；
2. 检查 Provider、NLI Session、Login Identity、Binding 和账号状态；
3. 按认证事务或 Binding 业务操作分别检查 Capability；
4. 在需要时请求 Credential Manager 提供内部调用凭据；
5. 选择并调用强类型 Adapter；
6. 统一处理错误、状态变化、重试、审计和指标；
7. 只向上层返回去除凭据的强类型结果。

业务模块、HTTP Handler 和后台任务都不能绕过 ProviderService 直接调用 Adapter 或 Credential Manager。

### Credential Manager 职责

Credential Manager 独占：

- 加密保存 Provider 长期授权；
- 获取短期可用 Access Token；
- 刷新、撤销和删除凭据；
- 将凭据错误映射为统一 Provider 错误；
- 阻止原始 Token 进入业务模块和日志；
- 对同一 Binding 的并发刷新进行协调。

### 长期凭据加密

Refresh Grant 等长期凭据使用版本化主密钥和 AEAD 加密：

```text
EncryptedCredential
- binding_id
- credential_kind
- key_version
- nonce
- ciphertext
- granted_capabilities
- provider_expires_at
- created_at
- updated_at
```

规则：

- 主密钥来自进程外 Secret Store 或安全环境配置，不能保存在数据库中；
- 每条记录使用独立随机 Nonce；
- Provider ID、Binding ID、Account ID、Credential Kind 和 Key Version 作为 Associated Data，防止密文跨记录替换；
- 数据库只保存密文和非秘密元数据；
- 读取旧 Key Version 时可以解密并渐进迁移到当前版本；
- 缺少所需主密钥时对应凭据不可用，不能以明文或弱加密降级；
- 具体选择 AES-GCM 或 XChaCha20-Poly1305 在实现阶段确定，但必须使用经过审计的 AEAD 库。

### 长期与短期 Token

- Refresh Grant 加密持久化；
- Provider Access Token 默认只缓存在进程内，到期立即删除；
- 多节点确需共享短期 Access Token 时，只能放入受控的短 TTL 加密缓存；
- Access Token、Refresh Grant 和解密结果不得进入普通日志、指标标签、错误响应或业务 DTO；
- 每个 Binding 同时只保留一份当前有效 Grant；重新授权时原子替换；
- Provider 旋转 Refresh Grant 时，必须在同一受控流程中原子保存新 Grant，旧 Grant 随即失效。

### 按需刷新与并发协调

ProviderService 仅在业务操作需要时获取 Access Token：

```text
存在且未接近过期的缓存 Access Token
-> 直接使用
否则
-> 对 Binding 进入单飞刷新
-> 获取分布式短租约锁
-> 再次检查缓存
-> 使用 Refresh Grant 刷新
-> 原子保存可能旋转的新 Grant
-> 更新短期 Access Token 缓存
-> 释放锁
```

规则：

- 不周期性刷新所有 Binding；
- 过期安全窗口由 Adapter 配置；
- 同节点使用 Singleflight 合并请求；
- 多节点使用按 Binding 隔离的短租约锁；
- 等待者复用刷新结果，不能各自刷新；
- 锁超时或刷新失败映射为统一错误，不泄漏凭据；
- 不同 Binding 之间不能使用全局锁互相阻塞。

### 后台与交互式授权

只有同时满足以下条件，Binding 才具有后台操作能力：

- Provider 声明 `BACKGROUND_REFRESH`；
- 用户启用了相应用途；
- Credential Grant 包含所需能力；
- Credential Manager 持有可刷新的长期 Grant；
- Binding 状态为 `ACTIVE`。

没有可持久 Refresh Grant 时：

- 不保存 Provider 密码；
- 不要求 Mod 定期上传 Access Token；
- 自动同步和其他后台任务跳过该 Binding；
- 用户可以在场时重新完成交互式授权，并立即执行一次操作；
- 交互式短期凭据只存在于受控操作上下文中，操作结束后删除。

### 撤销与删除

用户解绑或撤销用途时：

1. 立即阻止新的 Provider 操作；
2. 清除短期 Access Token 缓存；
3. 删除或密码学封存本地长期凭据；
4. 尽力调用 Provider 上游撤销端点；
5. 上游失败时记录可重试任务和安全审计；
6. 上游撤销失败不能恢复本地可用状态。

本地立即失效优先于等待 Provider 在线。

## Provider 错误

统一分类：

| 分类 | Binding 状态影响 | 自动重试 | 熔断 | 对外语义 |
| --- | --- | --- | --- | --- |
| `UNSUPPORTED` | 无 | 否 | 否 | Provider 不支持该能力 |
| `TEMPORARILY_UNAVAILABLE` | 无 | 安全操作最多 2 次 | 是 | 上游暂时不可用 |
| `RATE_LIMITED` | 无 | 按 Retry-After | 通常否 | 上游限流 |
| `REAUTH_REQUIRED` | 转为 `REAUTH_REQUIRED` | 否 | 否 | 用户需要重新授权 |
| `FORBIDDEN` | 无 | 否 | 否 | Provider 设置或权限禁止 |
| `INVALID_RESPONSE` | 无 | 安全读取最多 2 次 | 是 | 上游响应不兼容或无效 |
| `MISCONFIGURED` | 无，阻止 Provider 操作 | 否 | Provider 级 | 服务端 Provider 配置错误 |
| `SUBJECT_MISMATCH` | 保持原状态 | 否 | 否 | 重新验证登录了不同账号 |

规则：

- Adapter 必须根据协议事实分类，业务模块不能自行猜测；
- `401` 或无效 Grant 只有明确表示凭据失效时才映射 `REAUTH_REQUIRED`；
- Provider 业务设置关闭但凭据仍有效时映射 `FORBIDDEN`，不能误报重验证；
- `MISCONFIGURED` 只向运维暴露配置详情，用户只看到 Provider 暂不可用；
- 错误响应不包含上游 Body、Token、账号密码或内部 Endpoint；
- 状态变化、重验证通知和异常错误峰值需要记录审计或结构化指标。

## 重试、退避与熔断

自动重试仅适用于：

- 读取操作；
- Provider 明确保证幂等的操作；
- 可以确认请求尚未发送到上游的操作。

默认最多重试 2 次，采用指数退避和随机抖动。好友添加、删除、设置修改等写操作若没有 Provider 幂等保证，不得因未知结果自动重放。

熔断按 `Provider ID + 操作类别` 隔离，例如登录、好友读取、好友写入、实例读取分别统计。某个 Provider 的好友读取故障不能停止其登录，也不能影响其他 Provider。

`RATE_LIMITED` 优先遵循 Provider Retry-After，不应被普通熔断误判为服务故障。

## 重新验证、换绑与解绑

### 重新验证

```text
REAUTH_REQUIRED
-> 创建 10 分钟重验证事务
-> 用户完成 Provider 认证
-> 验证 Provider、Issuer、Subject
-> Subject 与原 Binding 完全一致
-> 原子替换 Credential Grant
-> 重新计算 Effective Capability
-> Binding 恢复 ACTIVE
```

如果 Subject 不一致：

- 返回 `SUBJECT_MISMATCH`；
- 原 Binding、凭据引用和状态保持不变；
- 不自动创建新 Binding；
- 用户必须明确进入换绑流程。

### 显式换绑

同一 Provider 换用另一个 Subject 时：

1. 用户完成近期 NLI 重新认证；
2. 验证新 Provider Subject；
3. 检查新 Subject 未被其他有效 NLI Account 使用；
4. 明确展示旧 Subject 与新 Subject 并要求确认；
5. 立即禁用旧 Binding 的本地操作；
6. 删除/封存旧凭据并尽力执行上游撤销；
7. 原子替换 Binding Subject 和新 Credential Grant；
8. 清除旧外部投影并安排新数据刷新；
9. 保留 NLI 自有好友关系；
10. 不自动修改独立 Provider Login Identity；
11. 写入安全审计并发送通知。

### 解绑

解绑需要近期 NLI 重新认证。解绑后：

- 立即停止本地 Provider 业务操作；
- 清除 Access Token 缓存并删除/封存长期 Grant；
- 尽力撤销上游授权；
- 删除或过期该 Binding 的外部好友/实例投影；
- 取消对应后台任务；
- 保留已经建立的 NLI 好友关系；
- 保留独立 Provider Login Identity；
- 写入安全审计并发送通知。

## Provider 浏览器事务安全

登录、绑定、重新验证和换绑使用用途隔离的浏览器事务：

- `state` 使用高熵随机值，服务端只保存哈希；
- 事务绑定发起 NLI Session、Provider ID、用途、Redirect URI 和创建时间；
- 事务有效期为 10 分钟；
- 回调时原子消费 `state`；
- Provider 支持时必须使用 PKCE；
- Redirect URI 必须精确匹配服务端配置，不允许客户端提供任意地址；
- 登录、绑定、重验证和换绑的 state 不能跨用途使用；
- 重复、过期、Provider 不匹配或 Session 不匹配的回调必须拒绝；
- 回调参数和授权码不得进入日志。

## Provider 降级与永久停用

### 临时故障

Provider 暂时不可用时：

- Binding 保持原状态；
- 当前操作返回统一暂时不可用错误；
- 安全读取按规则有限重试；
- 达到阈值后只熔断对应 Provider 的对应操作类别；
- 后台任务记录失败和下一次允许重试时间；
- NLI 邮箱登录、NLI 好友和 NLI 自有实例继续工作；
- 聚合查询如何表示来源暂不可用，由 `friendship.md` 定义。

### Provider 限流

- 优先遵循 Provider `Retry-After`；
- 不通过并发重试绕过上游限制；
- 限流按 Provider 和 Binding 隔离；
- 后台任务延后，不把 Binding 标记为 `REAUTH_REQUIRED`；
- 对用户返回 NLI 统一限流或上游暂不可用语义，不暴露上游敏感 Body。

### 永久停用

Provider 永久退出服务时：

- Registry 先转为 `NO_NEW_BINDINGS`，再转为 `DISABLED`；
- Provider ID 永久保留，不复用；
- 既有 Binding 和 Login Identity 保留为历史/恢复记录，不自动删除；
- 所有 Provider 操作停止，并向受影响用户通知；
- 用户仍可通过 NLI 邮箱登录和恢复；
- 用户可以解绑 Binding 或删除 Provider Login Identity；
- 已建立的 NLI 好友关系不自动删除；
- 外部投影按各模块保留策略过期或删除；
- 如果未来提供迁移，必须是显式、可审计流程，不能静默替换 Provider。

## Mock Provider

测试环境提供编译期 Mock Provider Adapter，使用与生产 Adapter 相同的 ProviderService 和 Credential Manager 边界。

Mock 必须支持：

- 动态启用/禁用每项 Capability；
- 登录成功、拒绝和 verified email；
- 固定 Subject 和切换 Subject；
- 短期 Access Token 到期；
- Refresh Grant 旋转；
- 支持或不支持 `BACKGROUND_REFRESH`；
- 好友读取和写入；
- Provider 设置读取和修改；
- `UNSUPPORTED`、临时故障、限流、重验证、Forbidden、无效响应和配置错误注入；
- 人为延迟，用于刷新竞争和熔断测试；
- 上游撤销成功或失败。

Mock 不能使用绕过 Credential Manager 的测试捷径，否则无法验证真实边界。

## 安全与故障场景走查

### Provider 登录但不建立业务 Binding

- Adapter 验证 Provider Subject；
- 创建独立 Provider Login Identity；
- 不保存后台 Refresh Grant；
- 不启用好友或实例 Capability；
- 用户仍可另行建立或拒绝业务 Binding。

结果：通过。

### 绑定并读取好友

- 管理员 Registry 提供启用的 Provider；
- 用户建立唯一业务 Binding；
- Effective Capability 同时检查 Provider、用途开关和 Grant；
- ProviderService 获取内部调用凭据并调用强类型 Adapter；
- 好友业务模块只收到去除凭据的结果。

结果：通过。

### 并发 Access Token 刷新

- 多个请求同时发现 Access Token 接近过期；
- 同节点 Singleflight 合并刷新；
- 多节点按 Binding 获取短租约锁；
- 刷新后原子保存旋转 Grant；
- 所有等待者复用同一新 Access Token；
- 不会由竞争写回旧 Refresh Grant。

结果：通过。

### Provider 临时宕机和熔断

- 安全读取有限重试后返回暂时不可用；
- Binding 不进入 `REAUTH_REQUIRED`；
- 熔断只影响该 Provider 的该操作类别；
- NLI 原生功能和其他 Provider 不受影响。

结果：通过。

### Refresh Grant 被撤销

- Adapter 明确分类为 `REAUTH_REQUIRED`；
- ProviderService 原子更新对应 Binding；
- 后台任务停止使用该 Binding；
- 用户收到重新验证提示；
- NLI 好友和邮箱登录不受影响。

结果：通过。

### 重验证登录了不同 Subject

- 回调 state、Session、Provider 和用途验证通过；
- Subject 与原 Binding 不一致；
- 返回 `SUBJECT_MISMATCH`；
- 原 Binding 和凭据保持不变；
- 只有显式换绑流程能够替换。

结果：通过。

### 解绑时 Provider 离线

- 本地立即阻止操作并清除缓存/长期 Grant；
- 上游撤销失败进入可重试任务；
- Binding 不恢复 ACTIVE；
- NLI 好友和独立 Login Identity 保留。

结果：通过。

### Provider 永久停用

- Registry 保留原 Provider ID 并转为 DISABLED；
- Provider API 停止，但 NLI 邮箱登录和原生功能继续；
- 用户可以查看影响并主动清理 Login Identity 或 Binding；
- 不静默迁移 Subject 或删除 NLI 好友。

结果：通过。

## Phase 2 完成条件

- [x] Provider Registry 和 Capability 冻结
- [x] Adapter 强类型边界冻结
- [x] Credential Manager 边界冻结
- [x] Provider Login Identity 与 Binding 边界冻结
- [x] Binding 状态机冻结
- [x] Provider 错误分类和状态影响冻结
- [x] 重新验证、解绑和换绑流程冻结
- [x] 后台授权和客户端主动操作规则冻结
- [x] Mock Provider 场景通过走查
- [x] Gate B 评审通过
