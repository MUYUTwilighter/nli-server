# NetherLink v2 数据模型

> 状态：`PHASE_7_FROZEN + GATE_F_FROZEN_EXTENSION`
>
> 输入基线：`common.md`、`nli_account.md`、`provider.md`、`friendship.md`、`game_instance.md`、`notifications.md`、`rest_api.md`；Phase 9 扩展依赖 `signaling.md`。
>
> 本文档从已冻结的领域和 API 契约推导 PostgreSQL 持久模型、Redis/LeaseStore 易失模型、约束、索引、事务边界、TTL 与清理规则。数据模型不得反向改变客户端契约。

## 目标

- 每个领域不变量有数据库约束、事务锁或易失存储原子操作保证；
- 持久业务状态可以完整恢复 HTTP 权威读模型；
- Redis 丢失不会删除账号、好友、Instance、Invite 或 Join 等业务事实；
- 原始 Token、Secret 与 Provider Credential 不以明文持久化；
- 高并发创建、状态转换、次数预留和 Session Fencing 有明确原子边界；
- 所有临时记录有 TTL 或可重试清理任务；
- 表结构支持 Phase 6 的查询、排序、可见性和幂等语义。

## 非目标

- 不在本文生成具体 SQL Migration；
- 不选择云厂商、KMS、Redis 产品或数据库托管方案；
- 不修改 REST Path、DTO 或领域状态机；
- 不建立通用 EAV、任意 Metadata JSON 或跨领域万能资源表；
- 不将审计日志当作业务事件重放源。

## 存储权威边界

### PostgreSQL

PostgreSQL 是全部业务事实和 HTTP 权威状态的唯一持久来源：

- Account、Email、Authenticator、Provider Login Identity；
- Token Family、Token 哈希和 Instance 绑定；
- Device Authorization、邮件一次性凭据和短期加密结果重放；
- Provider Registry、Binding、Credential Ciphertext；
- Friendship Pair、Request、ban、Source、Provider Friend Projection；
- Sync Task 与 Continuation；
- Game Instance 生命周期、配置、ACL、Proxy Grant、Invite；
- Guest Session、Join Request、Invite Reservation、Report；
- Idempotency、Audit 和 Transactional Outbox；
- Phase 9 Signaling Session/最小 Candidate Receipt，以及独立 Relay Grant、Allocation Slot和集群配额账本。

短生命周期不等于易失。Guest、Join、Device 和 Invite Resolution 需要与持久资源、次数或 Outbox 原子协调，因此仍进入 PostgreSQL，并由 TTL 索引和清理任务回收。

### Redis / LeaseStore / EventBus

Redis 类共享实现只保存允许丢失或可由 PostgreSQL 重建的易失状态：

- Instance current WS Session ID、90 秒 Lease 和 Node 路由；Signaling 的 session-scoped 双 Role Route Document、20 秒连接 Lease与短投递状态；
- 连接控制/业务提示 Pub/Sub；
- IP、Account、Family、Secret Hash Prefix 的限流桶；
- Provider Browser Transaction 的短期路由状态、State/PKCE Nonce；
- 可丢失读缓存、Singleflight 和分布式短租约锁。

Redis 丢失可以导致 WebSocket 重新握手、Instance 暂时 OFFLINE、浏览器流程重启或限流进入保守模式，但不能删除或回滚业务事实。禁止 PostgreSQL 与 Redis 双写为共同业务权威；跨存储流程按 `notifications.md` 使用事务、LeaseStore CAS、补偿和协调任务收敛。

## 公共 PostgreSQL 约定

### 标识、时间与命名

- 表名与列名使用 `snake_case`；
- 公共资源主键使用 `uuid`，由应用生成 UUIDv4；
- 公共 ID 不使用数据库自增值或可推测序列；
- 外键统一 `<resource>_id`，时间列统一 `_at`；
- 数据库时间使用 `timestamptz`，Repository 在 API 边界转换为 int64 Unix 毫秒；
- 数据库过期判断统一 `expires_at <= now()`；
- `created_at` 不可修改，`updated_at` 由写事务设置；
- API 毫秒精度不要求数据库截断内部精度，排序追加 UUID tie-breaker。

### 枚举与 CHECK

领域枚举使用 `text`/有界 `varchar` 加 CHECK，不使用 PostgreSQL ENUM。Rust 层仍使用强类型 Enum。新增值按“先部署读取能力、再扩展 CHECK、最后写入”的顺序；禁止无 CHECK 的自由状态字符串。

计数使用 `integer` 或 `bigint` 并加非负/上限 CHECK；Revision 使用 `bigint NOT NULL` 且从 1 开始，只允许事务递增。

### JSONB

JSONB 只用于版本化、非授权关键且非唯一性关键的快照：

- 清洗后的 Provider 展示快照；
- 有白名单和 schema_version 的审计详情；
- 幂等成功结果中的非秘密响应快照。

Owner、Status、Source、Identity Trait、ACL、Invite 次数、Session 绑定、过期时间以及所有用于授权/唯一约束的值必须列化或进入规范化子表。禁止通用 Metadata JSON。

### 删除与保留

不采用全局 soft-delete：

- 有领域终态的资源保存 status 与 terminal/revoked/closed/deleted_at；
- 恢复、结果重放、审计或法定窗口结束后，由清理任务硬删或匿名化；
- 唯一约束明确终态是否继续占用名称/身份；
- 每个 Repository 查询显式选择允许状态；
- 不使用容易漏过滤的全局默认 Scope。

### 授权边界

不依赖 PostgreSQL RLS。API、Worker 和 Repository 显式携带 Account、Token Family、Instance 或任务 Owner 条件；数据库通过外键、唯一约束、CHECK 和事务锁保证不变量。

敏感查询必须在 SQL 条件中同时约束资源 ID 与 Owner/Family/角色，不能先无条件读取再只在 Controller 判断。原始 SQL、约束名和受影响行数不进入公开错误。

### 密文与哈希

Provider Credential 和短期 Token/Secret Replay 的 AEAD 密文分列保存：

```text
ciphertext bytea
nonce bytea
key_version integer
algorithm text
aad_version integer
created_at timestamptz
expires_at timestamptz nullable
```

主密钥在进程外；AAD 至少绑定表用途、资源 UUID、Account/Binding/Family 和 schema version；Nonce 对同 key_version 唯一；解密失败 fail-closed 并审计；key_version 支持渐进重加密。

高熵 Access/Refresh/Device/Email/Invite/Grant Secret 和 Idempotency Key 使用用途隔离、带 Pepper 与 key_version 的 keyed lookup digest（例如 HMAC-SHA-256）；Password 独立使用 Argon2id。普通可离线验证哈希不能替代 Pepper keyed digest，比较使用常量时间。

## 设计批次

### 批次 1：Account/Auth/Provider（已完成）

- [x] username/email 使用独立占用注册表；
- [x] Token Family 与每代 Token 分表；
- [x] Account 行锁保证 10/5 上限；
- [x] Provider Subject 使用共享 Principal、AEAD 密文和 keyed digest；
- [x] Credential 按版本存储，旧版 RETIRED 24 小时；
- [x] 外键默认 RESTRICT，纯从属子表才 CASCADE。

## Account 与认证表

### `accounts`

```text
account_id uuid PK
registration_id uuid UNIQUE
status text CHECK PENDING_EMAIL|ACTIVE|DISABLED|DELETION_PENDING|DELETED
display_name text
display_name_normalized text
last_username_changed_at timestamptz null
created_at timestamptz
activated_at timestamptz null
disabled_at timestamptz null
deletion_requested_at timestamptz null
deletion_due_at timestamptz null
deleted_at timestamptz null
updated_at timestamptz
```

registration_id 是注册 Receipt 的独立 UUIDv4，不等于也不暴露 account_id。CHECK 保证状态相关时间基本一致；ACTIVE 必须有 activated_at，DELETION_PENDING 必须有 deletion_due_at。跨表“ACTIVE 必须有已验证 CURRENT Email”由状态转换事务锁定 Account 和 Email 行后保证。

索引：status + deletion_due_at 用于清理；ACTIVE Account 公开读取按 account_id 主键。账号删除不直接级联业务表，默认 RESTRICT，并由删除编排任务按领域规则匿名化/清理。

### `account_usernames`

```text
username_id uuid PK
account_id uuid FK accounts RESTRICT
username varchar(24)
kind text CHECK CURRENT|FORMER
reserved_until timestamptz null
created_at timestamptz
retired_at timestamptz null
```

- username 只保存规范化小写 ASCII，并 CHECK `^[a-z0-9_]{3,24}$`；
- `UNIQUE(username)` 同时覆盖当前和 90 天 Former 保留；
- `UNIQUE(account_id) WHERE kind='CURRENT'`；
- CURRENT 的 reserved_until/retired_at 必须空；FORMER 两者必填；
- 改名事务锁 Account 与新旧 Username：旧行转 FORMER 并设置 90 天，新行成为 CURRENT；
- 清理只删除 `FORMER AND reserved_until <= now()`。

### `account_emails`

```text
email_id uuid PK
account_id uuid FK accounts RESTRICT
email_display text
email_normalized text
kind text CHECK CURRENT|PENDING_CHANGE
verified_at timestamptz null
reservation_expires_at timestamptz null
created_at timestamptz
updated_at timestamptz
```

- `UNIQUE(email_normalized)` 保证当前、未验证注册和待变更邮箱不能跨账号冲突；
- `UNIQUE(account_id) WHERE kind='CURRENT'`；
- `UNIQUE(account_id) WHERE kind='PENDING_CHANGE'`；
- PENDING_EMAIL 注册的 CURRENT Email 可暂未验证，但 reservation_expires_at 固定为注册 24 小时截止；
- ACTIVE Account 的 CURRENT Email 必须 verified_at 非空；
- 邮箱变更先插入 PENDING_CHANGE，验证事务再删除/归档旧 CURRENT 并提升新行；
- 注册过期、变更取消和账号最终清理负责释放唯一占用。

`email_display` 只用于邮件发送/本人读取，`email_normalized` 用于唯一和 keyed 限流输入；两者均不得进入公开读模型或普通日志。

### `password_authenticators`

```text
account_id uuid PK/FK accounts RESTRICT
password_phc text
algorithm text CHECK algorithm='ARGON2ID'
parameter_version integer
created_at timestamptz
changed_at timestamptz
```

PHC 字符串包含 Salt 与参数，不额外保存可逆材料。更新密码锁定 Account；Password Reset 撤销全部 Family，Password Change 保留当前 Family并撤销其他 Family。

## Token Family 与 Token

### `token_families`

```text
token_family_id uuid PK
session_id uuid UNIQUE
account_id uuid FK accounts RESTRICT
client_name text
status text CHECK ACTIVE|ACTIVE_BOUND|REVOKED|COMPROMISED|EXPIRED
bound_instance_id uuid null UNIQUE
created_at timestamptz
last_refreshed_at timestamptz
idle_expires_at timestamptz
absolute_expires_at timestamptz
reauthenticated_at timestamptz null
reauthentication_expires_at timestamptz null
revoked_at timestamptz null
compromised_at timestamptz null
updated_at timestamptz
```

CHECK：client_name 为 1–64 Unicode 且已清洗；idle_expires_at 不晚于 absolute_expires_at；ACTIVE_BOUND 当且仅当 bound_instance_id 非空；终止状态不能绑定 Instance；reauthentication_expires_at 固定为 reauthenticated_at + 5 分钟且不超过 Family 有效期。

`bound_instance_id` 的延迟 FK 在 Game Instance 表建立后添加。Account 创建/消费 Family 时先锁 `accounts(account_id)`，COUNT 非终止 Family <10；绑定时同一锁下 COUNT ACTIVE_BOUND <5。唯一 bound_instance_id 和 Family 单列共同保证一对一，不维护易漂移计数器。

索引：`(account_id, status, created_at DESC, session_id)` 支持 Session 列表与限额；`absolute_expires_at`/`idle_expires_at` partial index 支持过期 Worker。

### `token_generations`

```text
token_generation_id uuid PK
token_family_id uuid FK token_families CASCADE
generation bigint
status text CHECK CURRENT|ROTATED
access_digest bytea
access_digest_key_version integer
refresh_digest bytea
refresh_digest_key_version integer
access_expires_at timestamptz
refresh_idle_expires_at timestamptz
issued_at timestamptz
rotated_at timestamptz null
```

约束：`UNIQUE(token_family_id,generation)`；每 Family 只有一个 CURRENT partial unique；Access/Refresh digest 分别按 digest+key_version 唯一。ROTATED 记录保留到 Family 绝对过期后安全窗口，用于检测旧 Refresh 重用；其 Access 从状态改变提交起立即无效。

Refresh 锁定 Family 和 CURRENT Generation，插入下一代、旋转旧行、更新 Family 时间并写 Secret Replay/Audit/Outbox。超过重放窗命中 ROTATED Refresh 时只把该 Family 标记 COMPROMISED。

## Device 与邮件凭据

### `device_authorizations`

```text
device_authorization_id uuid PK
device_code_digest bytea UNIQUE
device_digest_key_version integer
user_code_digest bytea
user_digest_key_version integer
client_name text
status text CHECK PENDING|APPROVED|DENIED|CONSUMED|EXPIRED
approved_account_id uuid FK accounts RESTRICT null
consumed_token_family_id uuid FK token_families RESTRICT null
poll_interval_seconds integer
last_polled_at timestamptz null
violation_count integer
created_at timestamptz
expires_at timestamptz
approved_at timestamptz null
denied_at timestamptz null
consumed_at timestamptz null
```

有效 User Code 使用 `(user_code_digest,user_digest_key_version)` partial unique；低熵 User Code 只保存 keyed digest。CHECK 约束状态对应 Account/时间。批准不创建 Family；消费事务锁 Authorization 与 Account，检查上限后创建 Family。过期索引为 `(expires_at) WHERE status IN ('PENDING','APPROVED','CONSUMED')`。

### `email_credentials`

```text
email_credential_id uuid PK
purpose text CHECK EMAIL_VERIFY|PASSWORD_RESET|EMAIL_CHANGE|DELETION_RECOVERY|EMAIL_REAUTH
token_digest bytea UNIQUE
digest_key_version integer
account_id uuid FK accounts RESTRICT
email_id uuid FK account_emails RESTRICT null
token_family_id uuid FK token_families RESTRICT null
token_generation_id uuid FK token_generations RESTRICT null
status text CHECK ACTIVE|CONSUMED|INVALIDATED|EXPIRED
created_at timestamptz
expires_at timestamptz
consumed_at timestamptz null
invalidated_at timestamptz null
```

Purpose CHECK 约束必要引用：EMAIL_REAUTH 必须同时绑定 Family 与发起时 CURRENT Token Generation，完成时当前 Access 必须仍是该 Generation；EMAIL_CHANGE/VERIFY 必须绑定 Email。每 Account+Purpose 最多一个 ACTIVE partial unique；重发事务先 INVALIDATE 旧记录。消费锁 Credential、Account、Email/Family，并写同事务 Replay/Audit/Outbox。

### `email_delivery_jobs`

```text
email_delivery_job_id uuid PK
email_credential_id uuid FK email_credentials RESTRICT
email_id uuid FK account_emails RESTRICT
template_type text CHECK EMAIL_VERIFY|PASSWORD_RESET|EMAIL_CHANGE|DELETION_RECOVERY|EMAIL_REAUTH
status text CHECK PENDING|CLAIMED|SENT|FAILED|ABANDONED
attempt_count integer
next_attempt_at timestamptz
claim_owner uuid null
claim_expires_at timestamptz null
provider_message_digest bytea null
provider_message_digest_key_version integer null
last_error_category text null
created_at timestamptz
sent_at timestamptz null
completed_at timestamptz null
```

Credential 创建/重发与 Job 在同一事务提交，`UNIQUE(email_credential_id,template_type)` 防重复发送任务。Worker 使用 SKIP LOCKED + Claim Lease，在 Credential expires_at 前指数退避；过期后 ABANDONED。Job 只引用 Email/Credential，不保存渲染 Body、Secret 或 SMTP 原始响应；发送成功/终态 24 小时后清理，分类结果进入 Audit。邮件 Provider 调用不发生在业务事务内。

## Provider 表

### `providers` 与 `provider_capabilities`

```text
providers(
  provider_id varchar(64) PK,
  display_name text,
  display_name_normalized text,
  display_assets_json jsonb,
  display_assets_schema_version integer,
  adapter_key text,
  status text CHECK ENABLED|NO_NEW_BINDINGS|DISABLED,
  issuer text,
  config_revision bigint,
  created_at timestamptz,
  updated_at timestamptz
)
provider_capabilities(
  provider_id FK providers CASCADE,
  capability text CHECK AUTHENTICATE_NLI|PROVIDE_VERIFIED_EMAIL|READ_FRIENDS|WRITE_FRIENDS|READ_INSTANCES|JOIN_INSTANCES|MANAGE_SETTINGS|BACKGROUND_REFRESH,
  PRIMARY KEY(provider_id,capability)
)
```

provider_id CHECK 稳定小写 slug。display_assets_json 仅含 Registry 管理端白名单的受信图标/主页等展示资源并随 Schema Version 校验。adapter_key/issuer 属于内部配置，不进入公共 DTO；任意 Endpoint、Client Secret 和私钥不放通用 JSONB。

### `provider_principals`

Provider Login Identity 与 Binding 共享规范化外部主体归属，解决跨两表唯一性：

```text
provider_principal_id uuid PK
provider_id FK providers RESTRICT
issuer text
subject_ciphertext bytea
subject_nonce bytea
subject_key_version integer
subject_algorithm text
subject_aad_version integer
subject_digest bytea
subject_digest_key_version integer
account_id uuid FK accounts RESTRICT
created_at timestamptz
last_verified_at timestamptz
```

`UNIQUE(provider_id,issuer,subject_digest,subject_digest_key_version)`。创建/换绑在 Provider+Issuer 级 advisory/应用锁下，同时用当前与仍接受的旧 digest key 查重，避免 Digest Key 轮换期跨版本重复；重加密事务更新 Ciphertext/Digest 并复核唯一性。

### `provider_login_identities`

```text
provider_login_identity_id uuid PK
provider_principal_id uuid UNIQUE FK provider_principals RESTRICT
status text CHECK ACTIVE|DISABLED
revision bigint
created_at timestamptz
updated_at timestamptz
last_used_at timestamptz null
```

删除 Login Identity 显式删除该行并写 Audit；若 Principal 仍被 Binding 引用则保留，否则在安全保留后清理。停用/删除不修改 Binding。

### `provider_bindings` 与用途

```text
provider_bindings(
  provider_binding_id uuid PK,
  account_id uuid FK accounts RESTRICT,
  provider_id FK providers RESTRICT,
  provider_principal_id uuid FK provider_principals RESTRICT,
  status text CHECK ACTIVE|REAUTH_REQUIRED|DISABLED,
  display_snapshot jsonb,
  display_schema_version integer,
  last_verified_at timestamptz,
  last_error_category text null,
  revision bigint,
  created_at timestamptz,
  updated_at timestamptz
)
provider_binding_usages(
  provider_binding_id FK provider_bindings CASCADE,
  usage text CHECK READ_FRIENDS|WRITE_FRIENDS|READ_INSTANCES|JOIN_INSTANCES|MANAGE_SETTINGS|BACKGROUND_SYNC,
  PRIMARY KEY(provider_binding_id,usage)
)
```

`UNIQUE(account_id,provider_id)`；数据库/事务还检查 Principal.account_id、Provider ID 与 Binding 一致。UNBOUND 不作为长期状态行返回：解绑事务删除/终止用途与 Credential，保存 Audit 后删除 Binding，避免终止行永久占用唯一键。

### `provider_credentials` 与 Grants

```text
provider_credentials(
  provider_credential_id uuid PK,
  provider_binding_id FK provider_bindings CASCADE,
  version bigint,
  status text CHECK CURRENT|RETIRED,
  grant_ciphertext bytea,
  grant_nonce bytea,
  grant_key_version integer,
  grant_algorithm text,
  grant_aad_version integer,
  created_at timestamptz,
  retired_at timestamptz null,
  purge_after timestamptz null
)
provider_credential_capabilities(
  provider_credential_id FK provider_credentials CASCADE,
  capability text CHECK READ_FRIENDS|WRITE_FRIENDS|READ_INSTANCES|JOIN_INSTANCES|MANAGE_SETTINGS|BACKGROUND_REFRESH,
  PRIMARY KEY(provider_credential_id,capability)
)
```

每 Binding 一个 CURRENT partial unique，`UNIQUE(provider_binding_id,version)`。新 Credential 在 Binding 锁下插入，旧版立即 RETIRED、不可用于业务并设置 `purge_after=retired_at+24h`。Access Token 只进短期缓存，不进 PostgreSQL。

### 批次 2：Friendship/Projection/Task（已完成）

- [x] 一个规范化 Pair 行锁定关系、双向 ban 与抑制窗口；
- [x] Request 历史分表并以 partial unique 保证一个 PENDING；
- [x] Request Source 与 Relationship Source 分表；
- [x] Provider Projection 使用 Subject AEAD+digest 和随机 external_friend_id；
- [x] 交互式候选快照进入 PostgreSQL，15 分钟 TTL；
- [x] Task Provider 结果与 Continuation 规范化分表；
- [x] REST ALL 可选 Binding，覆盖 PROVIDER_FULL 与 ALL_PROVIDERS。

## Friendship Pair 与 Request

### `friend_pairs`

```text
friend_pair_id uuid PK
account_low_id uuid FK accounts RESTRICT
account_high_id uuid FK accounts RESTRICT
relationship_status text CHECK NONE|PENDING|FRIENDS
ban_by_low boolean
ban_by_high boolean
ban_by_low_at timestamptz null
ban_by_high_at timestamptz null
suppress_low_to_high_until timestamptz null
suppress_high_to_low_until timestamptz null
friends_since timestamptz null
revision bigint
created_at timestamptz
updated_at timestamptz
```

约束：`account_low_id < account_high_id`；`UNIQUE(account_low_id,account_high_id)`；账号不能与自己成 Pair；ban Boolean 与时间成对；FRIENDS 必须有 friends_since，其他状态为空。

所有申请、接受、拒绝、取消、删除、Block、Provider 自动同步都先按 `(low,high)` 创建或锁定这一行。锁顺序永远使用 UUID low→high。Pair 为 NONE 且无 ban、无抑制、无历史保留引用时才可清理。

### `friend_requests`

```text
friend_request_id uuid PK
friend_pair_id uuid FK friend_pairs RESTRICT
account_low_id uuid
account_high_id uuid
requester_is_low boolean
requester_account_id uuid GENERATED STORED
recipient_account_id uuid GENERATED STORED
visibility text CHECK NLI_VISIBLE|PROVIDER_HIDDEN
status text CHECK PENDING|ACCEPTED|REJECTED|CANCELLED|EXPIRED
created_at timestamptz
expires_at timestamptz
terminal_at timestamptz null
```

`(friend_pair_id,account_low_id,account_high_id)` 复合 FK 指向 Pair 的同列唯一键，Generated requester/recipient 由 requester_is_low 选择，避免请求挂到 Pair 外账号。

`UNIQUE(friend_pair_id) WHERE status='PENDING'` 是最终并发防线。索引 `(recipient_account_id,status,created_at DESC,friend_request_id)` 和 requester 对称索引支持 Incoming/Outgoing；Provider-hidden 不进入普通 Outgoing 查询。PENDING 到期索引支持惰性/后台 EXPIRED。

Pair.relationship_status 与 PENDING Request 在同一事务更新；不存在通过 Event 重建关系。终态 Request 按 NLI 24 小时可读窗口后清理，安全审计独立保留。

## 来源表

### `friend_request_sources`

```text
friend_request_source_id uuid PK
friend_request_id uuid FK friend_requests CASCADE
source_type text CHECK NLI_SEARCH|PROVIDER_SYNC
provider_id FK providers RESTRICT null
issuer text null
initiator_binding_id uuid null
subject_reference_digest bytea null
subject_digest_key_version integer null
provider_relationship_type text null
verified_at timestamptz null
friend_sync_task_id uuid null
created_at timestamptz
```

CHECK：NLI_SEARCH 的 Provider 字段全空；PROVIDER_SYNC 的 Provider/Binding/Digest/关系/验证时间必填。同一 Request 的 NLI_SEARCH 最多一行；Provider Source 按 `(friend_request_id,provider_id,issuer,initiator_binding_id,subject_reference_digest,subject_digest_key_version)` 唯一。

### `friend_relationship_sources`

```text
friend_relationship_source_id uuid PK
friend_pair_id uuid FK friend_pairs RESTRICT
source_type text CHECK NLI_SEARCH|PROVIDER_SYNC
status text CHECK CURRENT|HISTORICAL|UNAVAILABLE
provider_id varchar(64) FK providers RESTRICT null
issuer text null
provider_binding_id uuid null
subject_reference_digest bytea null
subject_digest_key_version integer null
first_verified_at timestamptz
last_verified_at timestamptz
created_at timestamptz
updated_at timestamptz
```

接受 Request 时在同一 Pair 事务从 Request Source upsert 到 Relationship Source。来源只解释建立途径，不决定当前 FRIENDS 或联机授权。Provider 关系解除转 HISTORICAL；临时不可用转 UNAVAILABLE；解绑前先去除 Binding FK/转历史，不能因 CASCADE 删除好友事实。

## Provider Friend Projection

### `provider_friend_projections`

```text
provider_friend_projection_id uuid PK
external_friend_id uuid
provider_binding_id uuid FK provider_bindings CASCADE
provider_id FK providers RESTRICT
issuer text
subject_ciphertext bytea
subject_nonce bytea
subject_key_version integer
subject_algorithm text
subject_aad_version integer
subject_digest bytea
subject_digest_key_version integer
relationship_type text
display_name_normalized text
display_snapshot jsonb
display_schema_version integer
status text CHECK ACTIVE|REMOVED
last_confirmed_at timestamptz
unavailable_since timestamptz null
purge_after timestamptz
created_at timestamptz
updated_at timestamptz
```

约束：`UNIQUE(provider_binding_id,external_friend_id)`；同 Binding/Issuer/Subject Digest 唯一。external_friend_id 为随机 UUIDv4，仅在 Binding 范围作为 opaque ID；不等于 Subject/HMAC。

FRESH/STALE 在读取时由 last_confirmed_at、15 分钟阈值和 Provider/Binding 可用性推导。明确解除可立即 REMOVED；连续 30 天未确认由 purge_after 清理。display_snapshot 只含 Adapter 白名单公开字段。Projection 不持久化 matched_account_id，NLI 解析使用同 Provider/Issuer Digest 与 provider_principals 等值匹配并重新检查目标状态。

Digest Key 轮换期按 Provider/Issuer 锁并检查当前/旧版本，规则与 provider_principals 相同。

## Friend Sync Task

### `friend_sync_tasks`

```text
friend_sync_task_id uuid PK
owner_account_id uuid FK accounts RESTRICT
task_type text CHECK SINGLE_EXTERNAL_FRIEND|PROVIDER_FULL|ALL_PROVIDERS|AUTO_PROVIDER
parent_task_id uuid FK friend_sync_tasks RESTRICT null
provider_binding_id uuid null
external_friend_id uuid null
status text CHECK PENDING|RUNNING|CANCEL_REQUESTED|CANCELLED|SUCCEEDED|PARTIALLY_SUCCEEDED|FAILED
merge_scope_digest bytea
merge_scope_digest_key_version integer
attempt_count integer
next_attempt_at timestamptz
claim_owner uuid null
claim_expires_at timestamptz null
scanned_count integer
processed_count integer
failed_count integer
continuation_available boolean
created_at timestamptz
started_at timestamptz null
completed_at timestamptz null
cancel_requested_at timestamptz null
updated_at timestamptz
expires_at timestamptz
```

CHECK：SINGLE 必须 Binding+External ID；PROVIDER_FULL/AUTO 必须 Binding 且无 External ID；ALL_PROVIDERS 两者都空；子任务必须 parent。计数非负且 processed<=scanned。

`UNIQUE(owner_account_id,merge_scope_digest_key_version,merge_scope_digest) WHERE status IN ('PENDING','RUNNING','CANCEL_REQUESTED')` 合并活动 Scope。索引 `(owner_account_id,created_at DESC,friend_sync_task_id)` 支持 7 天列表，`(status,created_at)` 支持 Worker Claim。Worker 用 `FOR UPDATE SKIP LOCKED` 领取 PENDING，并设置有界执行 Lease/attempt，崩溃可回收。

### `friend_sync_task_provider_results`

```text
friend_sync_task_provider_result_id uuid PK
friend_sync_task_id uuid FK friend_sync_tasks CASCADE
provider_id FK providers RESTRICT
provider_binding_id uuid
status text CHECK PENDING|RUNNING|SUCCEEDED|PARTIAL|FAILED|CANCELLED
scanned_count integer
processed_count integer
failed_count integer
error_category text null
continuation_id uuid null
created_at timestamptz
updated_at timestamptz
completed_at timestamptz null
```

`UNIQUE(friend_sync_task_id,provider_id,provider_binding_id)`。父 Task 只从这些行聚合允许公开的 Provider 总数、完成数和分类错误，不保存 matched/blocked/unbound/auto-accepted 计数。

### `friend_sync_candidates`

无后台 Grant 的交互式读取写入短期候选：

```text
friend_sync_candidate_id uuid PK
friend_sync_task_id uuid FK friend_sync_tasks CASCADE
provider_binding_id uuid
issuer text
subject_ciphertext bytea
subject_nonce bytea
subject_key_version integer
subject_algorithm text
subject_aad_version integer
subject_digest bytea
subject_digest_key_version integer
relationship_type text
display_snapshot jsonb
display_schema_version integer
confirmed_at timestamptz
expires_at timestamptz
processed_at timestamptz null
```

同 Task/Issuer/Subject 唯一，expires_at 不晚于 confirmed_at+15 分钟。Worker 逐候选独立 Pair 事务，处理结果只更新 processed_at/安全计数；任务完成或过期后清理。

### `friend_sync_continuations`

```text
friend_sync_continuation_id uuid PK
owner_account_id uuid FK accounts RESTRICT
provider_binding_id uuid
source_task_id uuid FK friend_sync_tasks RESTRICT
status text CHECK ACTIVE|CONSUMED|EXPIRED
cursor_ciphertext bytea
cursor_nonce bytea
cursor_key_version integer
cursor_algorithm text
cursor_aad_version integer
snapshot_version text
created_at timestamptz
expires_at timestamptz
consumed_at timestamptz null
```

Continuation 绝对 24 小时且一次消费；Cursor AAD 绑定 Owner/Binding/Provider/Source Task/Snapshot。API 只返回独立不透明 continuation token/ID，不返回 Provider 原始 Cursor。ACTIVE 的过期索引支持清理。

### `provider_sync_schedules`

```text
provider_binding_id uuid PK/FK provider_bindings CASCADE
enabled boolean
minimum_interval_seconds integer CHECK >=21600
last_started_at timestamptz null
next_run_at timestamptz null
updated_at timestamptz
```

仅具有 BACKGROUND_REFRESH 和有效 Grant 的 ACTIVE Binding 可 enabled。调度器锁/claim 到期行并加入随机抖动；Provider 批量硬下限 15 分钟仍在 Task 创建事务复核，Schedule 默认不小于 6 小时。

### 批次 3：Instance/ACL/Invite/Join/Report（已完成）

- [x] ONLINE 仅由 LeaseStore 派生，PostgreSQL 保存 lifecycle/offline_since；
- [x] Instance 固定列保存配置并维护 config_revision/acl_revision；
- [x] ACL Rule 与三类 Matcher 子表规范化；
- [x] Proxy 绑定使用独立历史关联表和双侧 partial unique；
- [x] Invite 计数与一 Request 一 Reservation 同时保存；
- [x] Join Trait 与 Source Proof 使用受约束子表；
- [x] Report 默认保留 180 天后匿名化/清理。

## Game Instance

### `game_instances`

```text
instance_id uuid PK
owner_account_id uuid FK accounts RESTRICT
owner_token_family_id uuid FK token_families RESTRICT null
owner_session_id uuid
lifecycle text CHECK ACTIVE|CLOSED
profile_source varchar(32)
profile_uuid uuid null
profile_username text
profile_username_normalized text
game_version varchar(64)
loader_id varchar(64) null
loader_version varchar(64) null
mod_version varchar(64)
description text
approval_mode text CHECK MANUAL|AUTO
game_state text CHECK IN_WORLD|BUSY|UNKNOWN
config_revision bigint
acl_revision bigint
offline_since timestamptz null
auto_close_due_at timestamptz null
created_at timestamptz
updated_at timestamptz
closed_at timestamptz null
```

不保存 `online`、current WS Session ID 或 Lease expiry；ONLINE 是 `lifecycle=ACTIVE` 与 LeaseStore 有效 current Lease 的交集。数据库只保存 OFFLINE 开始和 10 分钟自动关闭截止，协调器负责与 LeaseStore 收敛。

约束：每 Family 最多一个 ACTIVE Instance partial unique；ACTIVE 必须保留 Family FK 且 Owner 与 Family.account_id 一致；CLOSED 后可清空 Family FK但保留不可授权的 owner_session_id 快照；ACTIVE 的 closed_at 为空，CLOSED 必填。loader_id/loader_version 同时为空或同时非空。Profile/兼容/描述采用 Phase 4 长度、NFC、控制字符和可打印 ASCII CHECK/写前验证，不使用 JSONB。

创建锁 Account→Family，在一个事务插入 Instance、默认 ACL、绑定 Family 并写 Outbox。普通 PUT 锁 Instance 并要求 config_revision 精确相等，只更新客户端可写固定列并递增。ACL PUT 独立递增 acl_revision。

索引：`(owner_account_id,created_at DESC,instance_id)`；ACTIVE auto-close partial index `(auto_close_due_at,instance_id) WHERE lifecycle='ACTIVE' AND auto_close_due_at IS NOT NULL`；`owner_token_family_id WHERE lifecycle='ACTIVE'` 唯一。

### ONLINE/OFFLINE 协调

- WebSocket CAS 成功后，短事务按预期 ws_session_id 复核 LeaseStore并清空 offline_since/auto_close_due_at；
- current Lease 消失时，短事务设置 offline_since 与 `auto_close_due_at=offline_since+10m`，并取消以该 Instance 为 Source 或 Target 的 PENDING Join、释放 Reservation、写 Outbox；
- 自动关闭 Worker 重新检查无有效 Lease，按 Family→Instance 顺序锁定后执行完整 Close；
- PostgreSQL事务失败时对刚写入 Lease 使用 compare-delete 补偿；
- Instance 对外 ONLINE 永远不读取数据库缓存 Boolean。

## Instance ACL

### `instance_acl_rules`

```text
instance_acl_rule_id uuid PK
instance_id uuid FK game_instances CASCADE
priority integer
position_revision bigint
action text CHECK ALLOW|DENY
created_at timestamptz
```

`UNIQUE(instance_id,priority)`；每 Instance 最多 100 行。position_revision 等于创建它的 acl_revision，便于诊断但不参与客户端并发判断。

### Matcher 子表

```text
instance_acl_rule_identity_types(
  rule_id FK instance_acl_rules CASCADE,
  identity_type text CHECK NLI_ACCOUNT|DIRECT_FRIEND|PROXY_FRIEND|ANONYMOUS,
  PRIMARY KEY(rule_id,identity_type)
)
instance_acl_rule_sources(
  rule_id FK instance_acl_rules CASCADE,
  source text CHECK FRIEND_LIST|INVITE_CODE,
  PRIMARY KEY(rule_id,source)
)
instance_acl_rule_subjects(
  subject_id uuid PK,
  rule_id FK instance_acl_rules CASCADE,
  subject_type text CHECK NLI_ACCOUNT_ID|MC_USERNAME,
  nli_account_id uuid FK accounts RESTRICT null,
  mc_username_normalized text null,
  trust text CHECK RELIABLE_NLI|WEAK_CLIENT_CLAIMED
)
```

Subject CHECK 保证 NLI_ACCOUNT_ID 只有 account_id+RELIABLE_NLI，MC_USERNAME 只有规范化文本+WEAK_CLIENT_CLAIMED；每 Rule/typed value 唯一。每类 Matcher 最多 100 值。零子行明确表示“任意”，不能插入 ANY sentinel。

ACL PUT 锁 Instance，CAS acl_revision，验证全部 Rule/Matcher；客户端提供的既有 Rule ID 必须属于该 Instance并在重建子树时保留，省略 ID 的新 Rule/Subject 由服务端生成；随后递增 Revision，并在同事务取消最新 ACL 已不允许的 PENDING Request 与写 Outbox。任何失败整批回滚。

## Proxy Grant

### `proxy_grants`

```text
proxy_grant_id uuid PK
issuer_account_id uuid FK accounts RESTRICT
grantee_account_id uuid FK accounts RESTRICT
friend_pair_id uuid FK friend_pairs RESTRICT
status text CHECK ACTIVE|REVOKED
secret_digest bytea null
secret_digest_key_version integer null
secret_generation bigint
expires_at timestamptz null
created_at timestamptz
updated_at timestamptz
revoked_at timestamptz null
last_bound_at timestamptz null
revocation_reason text null
```

Issuer != Grantee，Pair 必须恰为双方且创建时 FRIENDS。Secret Generation 从 1 起；轮换只更新 digest/generation，不结束已有绑定。`expires_at` 为空合法；API 有效状态额外要求未过期、双方 ACTIVE 且 Pair 仍 FRIENDS。

Issuer Account 行锁+COUNT 有效 Grant 保证最多 10；索引 `(issuer_account_id,status,created_at DESC,proxy_grant_id)` 与 ACTIVE expires_at 清理索引。好友解除/账号失效/显式撤销锁 Pair→Grant，将状态 REVOKED 并结束绑定。

### `instance_proxy_bindings`

```text
instance_proxy_binding_id uuid PK
proxy_grant_id uuid FK proxy_grants RESTRICT
instance_id uuid FK game_instances RESTRICT
status text CHECK ACTIVE|ENDED
bound_at timestamptz
ended_at timestamptz null
end_reason text null
```

`UNIQUE(proxy_grant_id) WHERE status='ACTIVE'` 与 `UNIQUE(instance_id) WHERE status='ACTIVE'` 保证双侧一对一。绑定事务按 Pair→Grant→Instance 顺序锁定；验证 Instance.owner=Grant.grantee。Presented-Under Account 始终从 ACTIVE Binding→Grant.issuer 推导，不冗余写入 Instance。

切换先锁并验证新 Grant，再结束旧 Binding、插入新 Binding；失败时旧关联保持。历史 Binding 不用于授权，按审计窗口清理。

## Instance Invite 与 Resolution

### `instance_invites`

```text
instance_invite_id uuid PK
instance_id uuid FK game_instances RESTRICT
lifecycle text CHECK ACTIVE|REVOKED
secret_digest bytea null
secret_digest_key_version integer null
secret_generation bigint
max_uses integer CHECK 1..100
consumed_uses integer
reserved_uses integer
expires_at timestamptz
created_at timestamptz
updated_at timestamptz
rotated_at timestamptz null
revoked_at timestamptz null
last_consumed_at timestamptz null
```

CHECK：计数非负且 `consumed_uses+reserved_uses<=max_uses`。对外 status 动态推导：REVOKED 优先；然后 expires_at→EXPIRED；容量为零→EXHAUSTED；其余 ACTIVE。因 PENDING Reservation 释放后容量可恢复，所以不把临时 EXHAUSTED 固化为不可逆生命周期。

创建时锁 Instance并 COUNT `lifecycle=ACTIVE AND expires_at>now() AND consumed_uses<max_uses`，最多 3 个可继续使用的 Invite。轮换递增 Generation；旧 Generation 不能建立新 Resolution/Guest，但既有 Reservation 不受影响。关闭/撤销在同事务释放相关 PENDING Reservation。

索引：Instance 管理列表；`expires_at WHERE lifecycle='ACTIVE'`；非空 Secret `(secret_digest_key_version,secret_digest)` partial unique 查找。终态/到期清理可清空 Digest而不影响元数据保留。

### `invite_resolutions`

```text
invite_resolution_id uuid PK
requester_account_id uuid FK accounts RESTRICT
requester_token_family_id uuid FK token_families RESTRICT
source_instance_id uuid FK game_instances RESTRICT
target_instance_id uuid FK game_instances RESTRICT
instance_invite_id uuid FK instance_invites RESTRICT
invite_generation bigint
status text CHECK ACTIVE|CONSUMED|INVALIDATED|EXPIRED
created_at timestamptz
expires_at timestamptz
consumed_at timestamptz null
invalidated_at timestamptz null
consumed_join_request_id uuid null
```

Resolution 绝对 60 秒、Source != Target、只可由同一 Family 使用一次。它没有额外 Bearer Secret；UUID 必须与当前认证 Family 同时匹配。轮换/撤销、ACL DENY、Session/Instance 失效可惰性视为 INVALID，并由清理任务收敛；旧 Generation Guest/Resolution 不能建立新 Request。

## Guest Session

### `guest_sessions`

```text
guest_id uuid PK
target_instance_id uuid FK game_instances RESTRICT
instance_invite_id uuid FK instance_invites RESTRICT
invite_generation bigint
token_digest bytea null
token_digest_key_version integer null
profile_source varchar(32)
profile_uuid uuid null
profile_username text
profile_username_normalized text
status text CHECK ACTIVE|BLOCKED|TERMINATED|EXPIRED
ip_digest bytea
ip_digest_key_version integer
created_at timestamptz
absolute_expires_at timestamptz
blocked_at timestamptz null
terminated_at timestamptz null
terminal_result_expires_at timestamptz null
```

Guest 绝对 5 分钟，无 Refresh。Profile 是不可变 CLIENT_CLAIMED 快照。创建锁 Target Instance，COUNT 未过期 ACTIVE/BLOCKED Guest <100；Invite 此时只验证但不预留次数。

BLOCKED 只能读取已有关联 Request 固定结果，不能创建/取消/使用 Lease。Join 终态后设置 terminal_result_expires_at=min(absolute_expires_at,terminal_at+60s)。可读截止后先置 EXPIRED 并清空 Token Digest；Guest 最小去敏元数据可保留 30 天供安全关联，之后在无 Report/Legal Hold 引用时硬删。IP Digest 最长保留 24 小时且不返回 Owner。`UNIQUE(token_digest,token_digest_key_version) WHERE token_digest IS NOT NULL` 支持认证且允许去敏。

## Join Request

### `join_requests`

```text
join_request_id uuid PK
target_instance_id uuid FK game_instances RESTRICT
requester_type text CHECK NLI_ACCOUNT|ANONYMOUS
requester_account_id uuid FK accounts RESTRICT null
requester_token_family_id uuid FK token_families RESTRICT null
requester_session_id uuid null
source_instance_id uuid FK game_instances RESTRICT null
guest_id uuid FK guest_sessions RESTRICT null
source text CHECK FRIEND_LIST|INVITE_CODE
presented_under_account_id uuid FK accounts RESTRICT null
status text CHECK PENDING|ACCEPTED|REJECTED|CANCELLED|EXPIRED
decision_mode text CHECK MANUAL|AUTO
decider_session_id uuid null
public_rejection_reason text null
profile_source varchar(32)
profile_uuid uuid null
profile_username text
profile_username_normalized text
created_at timestamptz
expires_at timestamptz
terminal_at timestamptz null
acceptance_lease_expires_at timestamptz null
query_expires_at timestamptz null
purge_after timestamptz null
```

CHECK：NLI requester 必须 Account+requester_session_id+Source Instance 且无 Guest，并在 PENDING/查询可读窗口内保留可复核的 Family FK；窗口结束后可清空 Family FK但 Session Snapshot 不再授权。ANONYMOUS 必须 Guest 且无 Account/Family/Source Instance。MANUAL ACCEPTED 必须 decider_session_id，AUTO 不得有 Decider。Profile 固定复制自 Source Instance/Guest。ACCEPTED 必须有 terminal_at 与 `acceptance_lease_expires_at=terminal_at+60s`；其他终态无 Lease；PENDING 无 terminal_at。NLI 终态 query_expires_at=terminal_at+24h，Guest 为 terminal_at+60s 且不超过 Guest 绝对期限；purge_after 至少 terminal_at+30d。AUTO 只能在创建事务直接成为 ACCEPTED。

活动唯一：`(requester_token_family_id,target_instance_id) WHERE status='PENDING'`；`UNIQUE(guest_id)` 保证 Guest 一生最多一个 Request。Target 列表索引 `(target_instance_id,status,created_at,join_request_id)`，NLI Source 恢复索引 `(source_instance_id,created_at DESC,join_request_id)`，PENDING expires_at 支持过期 Worker。

创建先锁请求 Family/相关 Pair/Grant，再按 UUID 顺序锁 Source/Target Instance，最后锁 Invite/Guest；检查 Source outgoing PENDING <3、Target incoming <100。AUTO 不进入 PENDING quota，但仍在同事务创建完整终态行、消费 Reservation并写 Outbox。

### `join_request_identity_traits`

```text
join_request_id uuid FK join_requests CASCADE
identity_trait text CHECK NLI_ACCOUNT|DIRECT_FRIEND|PROXY_FRIEND|ANONYMOUS
PRIMARY KEY(join_request_id,identity_trait)
```

这是创建时服务端推导快照，只用于展示、审计和复核上下文；Accept/信令仍从最新 Account/Friend/Proxy 状态重新推导可靠 Trait。

### `join_request_source_proofs`

```text
join_request_id uuid PK/FK join_requests CASCADE
source text CHECK FRIEND_LIST|INVITE_CODE
friend_path text CHECK DIRECT|PROXY null
requester_friend_pair_id uuid FK friend_pairs RESTRICT null
owner_presenter_friend_pair_id uuid FK friend_pairs RESTRICT null
instance_proxy_binding_id uuid FK instance_proxy_bindings RESTRICT null
instance_invite_id uuid FK instance_invites RESTRICT null
invite_generation bigint null
invite_resolution_id uuid FK invite_resolutions RESTRICT null
guest_id uuid FK guest_sessions RESTRICT null
created_at timestamptz
```

CHECK：DIRECT FRIEND_LIST 只有 requester_friend_pair；PROXY FRIEND_LIST 必须两条 Pair+ACTIVE Proxy Binding；INVITE_CODE 必须 Invite/Generation，NLI 使用 Resolution、Anonymous 使用 Guest。source 必须与 join_requests.source 相同，由写事务保证并定期一致性检查。

### `invite_reservations`

```text
invite_reservation_id uuid PK
instance_invite_id uuid FK instance_invites RESTRICT
join_request_id uuid UNIQUE FK join_requests RESTRICT
invite_generation bigint
status text CHECK RESERVED|CONSUMED|RELEASED
created_at timestamptz
expires_at timestamptz
consumed_at timestamptz null
released_at timestamptz null
release_reason text null
```

创建锁 Invite，插入一 Request 一 Reservation并 `reserved_uses+1`。ACCEPTED 原子改 CONSUMED、reserved-1、consumed+1；其他终态改 RELEASED、reserved-1。expires_at 不晚于 Request 60 秒。修复 Worker 锁 Invite→Request→Reservation，对过期 RESERVED 根据 Request 终态释放/消费并校正计数；计数与行不一致触发安全告警而非静默超卖。

## Report

### `reports`

```text
report_id uuid PK
join_request_id uuid FK join_requests RESTRICT
reporter_role text CHECK REQUESTER|TARGET
reporter_account_id uuid FK accounts RESTRICT null
reporter_session_id uuid null
reporter_guest_id uuid FK guest_sessions RESTRICT null
reported_account_id uuid FK accounts RESTRICT null
reported_guest_id uuid FK guest_sessions RESTRICT null
target_instance_id uuid FK game_instances RESTRICT
category text CHECK HARASSMENT|CHEATING|IMPERSONATION|MALICIOUS_CONTENT|OTHER
description text null
reported_profile_source varchar(32) null
reported_profile_uuid uuid null
reported_profile_username text null
reported_profile_username_normalized text null
status text CHECK RECEIVED
created_at timestamptz
retain_until timestamptz
legal_hold_until timestamptz null
anonymized_at timestamptz null
```

所有身份、Target、Session Snapshot 和 Profile 都从 Join Request/参与者服务端复制，只有 category/description 来自请求。`(join_request_id,reporter_role,created_at,report_id)` 建普通索引；不同 Key 的重复提交由分层限流而非额外唯一约束处理，Idempotency 只处理网络重试。description 最多 2000 Unicode 且去控制字符。

默认 `retain_until=created_at+180d`；legal_hold_until 非空且更晚时延后清理，并只能由独立受审计的 Moderation 管理能力修改。到期后按 Moderation/Legal Hold 决策删除自由描述和 Profile、匿名化主体或硬删；最小安全审计另保留至少 30 天，只含分类、角色、资源 ID 与时间。Report 不因 Guest/Request 常规 TTL 提前级联删除，因此相关 FK 默认 RESTRICT，由清理编排先匿名化引用。

### 批次 4：Audit/Outbox/Idempotency/Redis（已完成）

- [x] 普通幂等记录与业务事务原子提交并保留 24 小时；
- [x] Secret Replay 独立 AEAD 保存 60 秒，过窗保留 tombstone；
- [x] Outbox Event 与 Delivery 分表，24 小时重试、7 天留存；
- [x] 安全审计独立 append-only 权限，180/30 天分级保留；
- [x] Lease/路由、EventBus、GCRA、Browser Flow 和 Lock Key 冻结；
- [x] Redis 故障按风险 fail-closed，不成为业务权威。

## 幂等与 Secret Replay

### `idempotency_records`

```text
idempotency_record_id uuid PK
principal_kind text CHECK PUBLIC|ACCOUNT|TOKEN_FAMILY|GUEST
principal_id uuid null
principal_scope_digest bytea
principal_scope_digest_key_version integer
http_method text
route_template text
idempotency_key_digest bytea
idempotency_digest_key_version integer
request_digest bytea
request_digest_key_version integer
canonicalization_version integer
status text CHECK PROCESSING|COMPLETED
result_kind text CHECK NON_SECRET|SECRET_REPLAY|NO_CONTENT|SECRET_EXPIRED
response_status integer null
response_content_type text null
response_json jsonb null
response_location text null
resource_type text null
resource_id uuid null
processing_owner uuid null
processing_lease_expires_at timestamptz null
created_at timestamptz
updated_at timestamptz
expires_at timestamptz
```

唯一键是 `(principal_scope_digest,principal_scope_digest_key_version,http_method,route_template,idempotency_key_digest,idempotency_digest_key_version)`。PUBLIC 不使用全站共享 Scope：按端点加入 keyed operation identity（例如注册 username+email digest、登录 email digest、Guest Invite digest、Provider Flow Handle）；ACCOUNT 业务命令按 Account，Instance 管理按 Family，Guest 命令按 Guest。Key 与规范化请求均使用用途隔离 keyed digest，含 Secret 的 Body 不能留下可离线猜测摘要。

短 PostgreSQL业务命令在同一事务插入 Record、执行业务写、填充结果；唯一冲突后比较 request_digest：相同返回已有结果，不同返回 IDEMPOTENCY_KEY_REUSED。长流程可使用 PROCESSING Lease 和 fencing owner；过期接管必须先查询是否已有业务资源，不能盲目重放副作用。

NON_SECRET response_json 只保存严格 DTO 且最大 64 KiB，禁止 Token/Secret/Credential/Email/IP；若成功 DTO 本身必须含 CLIENT_CLAIMED Profile，可在数据库权限和 24 小时清理边界内保存该最小快照，但绝不写普通日志。默认 expires_at=created_at+24h。Digest Key 切换前所有写节点必须先识别新版本，再统一切换 active writer version，避免滚动部署跨版本重复键。

### `secret_replays`

```text
secret_replay_id uuid PK
idempotency_record_id uuid UNIQUE FK idempotency_records CASCADE
operation_input_digest bytea
operation_digest_key_version integer
result_ciphertext bytea
result_nonce bytea
result_key_version integer
result_algorithm text
result_aad_version integer
created_at timestamptz
expires_at timestamptz
consumed_after_expiry_at timestamptz null
```

Token Pair、Guest Token、Invite/Proxy Secret 和其他一次性成功结果以 AEAD 保存最多 60 秒；AAD 绑定 Idempotency Record、Principal、Route、request_digest 和业务 Resource ID。

到期清理先把父 Record.result_kind 原子改为 SECRET_EXPIRED，再删除密文。除安全重用凭据外，同 Key/同请求在 24 小时幂等期内随后返回 `409 IDEMPOTENCY_RESULT_EXPIRED`，绝不重新创建资源或轮换 Secret。Refresh/Device 必须先按输入 Credential 检查安全状态：超出 60 秒的旧 Refresh 命中 ROTATED Generation 时优先把对应 Family 标记 COMPROMISED 并统一返回 401；已消费 Device Code 超窗返回 RFC 8628 终态错误，不被普通 tombstone 掩盖。

## Transactional Outbox

### `notification_event_types`

```text
notification_event_type text PK
scope text CHECK ACCOUNT|INSTANCE|RESOURCE
payload_schema_version integer
status text CHECK ACTIVE|RETIRED
```

Registry 由 Migration 写入 `notifications.md` 冻结的事件类型；应用不能在运行时创建任意类型。RETIRED 类型只允许读取历史 Outbox，不能产生新 Event。

### `outbox_events`

```text
event_id uuid PK
event_type text FK notification_event_types
resource_type text
resource_id uuid
scope text CHECK ACCOUNT|INSTANCE|RESOURCE
scope_resource_type text null
scope_resource_id uuid null
payload_json jsonb
payload_schema_version integer
occurred_at timestamptz
created_at timestamptz
```

Event 与业务状态同一 PostgreSQL事务插入。payload_json 只能是 `notifications.md` 白名单提示字段；Event ID 是客户端 Envelope event_id，重试和多接收者 Fanout 始终复用。

### `outbox_deliveries`

```text
outbox_delivery_id uuid PK
event_id uuid FK outbox_events CASCADE
recipient_kind text CHECK ACCOUNT|INSTANCE
recipient_id uuid
status text CHECK PENDING|CLAIMED|PUBLISHED|ABANDONED
attempt_count integer
next_attempt_at timestamptz
claim_owner uuid null
claim_expires_at timestamptz null
last_error_category text null
published_at timestamptz null
abandoned_at timestamptz null
created_at timestamptz
updated_at timestamptz
```

`UNIQUE(event_id,recipient_kind,recipient_id)`。ACCOUNT recipient 在 EventBus/WS 层 Fanout 到该账号全部 ONLINE Instance；INSTANCE 只发 current Lease 路由。RESOURCE 是 Envelope scope，不替代实际 recipient。

Dispatcher 使用 `FOR UPDATE SKIP LOCKED` Claim 并带 Lease；发布失败指数退避最多 24 小时，随后 ABANDONED。PUBLISHED 只表示已交给 best-effort EventBus，不表示客户端收到。终态 Delivery/Event 再保留 7 天；HTTP 状态始终可恢复。

索引：`(status,next_attempt_at,event_id)`；Claim expiry；Event occurred_at 清理。Event 只有在全部 Delivery 终态后才可删。

## 安全审计

### `audit_event_types` 与 `security_audit_events`

建议置于独立 PostgreSQL schema，由应用角色仅授予 INSERT/SELECT 必要列，UPDATE/DELETE 只授予专用保留任务：

```text
audit_event_types(
  event_type text PK,
  default_tier text CHECK HIGH_RISK|BUSINESS,
  status text CHECK ACTIVE|RETIRED
)

security_audit_events:
audit_event_id uuid PK
event_type text FK audit_event_types
tier text CHECK HIGH_RISK|BUSINESS
occurred_at timestamptz
request_id uuid null
actor_kind text CHECK PUBLIC|ACCOUNT|TOKEN_FAMILY|GUEST|ADMIN|SYSTEM
actor_id uuid null
target_type text null
target_id uuid null
outcome text CHECK SUCCEEDED|DENIED|FAILED
reason_category text null
network_digest bytea null
network_digest_key_version integer null
details_json jsonb
details_schema_version integer
retain_until timestamptz
exported_at timestamptz null
```

Audit ID 引用不设业务 FK，确保业务匿名化/删除后记录仍完整。高风险操作无法在同一事务插入 Audit 时整体 fail-closed；普通物理清理的 Audit 失败进入独立告警/补偿队列，不能伪造成功记录。event_type 通过受控 `audit_event_types` 注册表扩展；details 只允许事件级白名单，禁止原始 Token、Secret、Provider Subject/Credential、Email、IP、MC Profile 和请求/响应 Body。

认证器/密码/Session/Provider Credential/ban/Proxy/Invite/Account 状态等高风险事件默认 180 天；普通业务审计默认 30 天，部署可按合规延长。主库不建立全局或 Account Hash Chain，避免串行热点；需要更强防篡改时增量导出到权限隔离的 WORM/SIEM，exported_at 只记录投递进度而不构成真实性证明。

## Redis / LeaseStore / EventBus

所有 Key 以 `nli:v2:` 开头，Value 有 schema_version。Key 中不出现原始 Token、Secret、Email、IP、Provider Subject 或 MC Profile；相关维度先做用途隔离 HMAC 并截取足够长度的编码 Digest。

### Instance WS Lease

```text
Key: nli:v2:instance:{instance_id}:ws
Value:
  schema_version
  ws_session_id
  node_id
  owner_account_id
  token_family_id
  connected_at_ms
  renewed_at_ms
TTL: 90 seconds
```

LeaseStore 必须原子提供：

1. `replace(instance_id,new_value,ttl)`：设置新 current Session并返回旧值；
2. `renew_if(instance_id,ws_session_id,ttl)`：仅 current Session 可续租；
3. `delete_if(instance_id,ws_session_id)`：旧连接 Close 不能删除新连接；
4. `get_current(instance_id)`：授权和协调读取。

Lua/Transaction 比较完整 UUID，不接受客户端时间。握手先验证 PostgreSQL ACTIVE+Family Binding，再 replace，提交前后复核并补偿；Ping 在最新授权检查后合并为最多每 10 秒一次续租。TTL 丢失即 OFFLINE，不能从节点本地连接表恢复为 ONLINE。

node_id 只用于把控制消息路由到拥有 Socket 的节点；Socket 对象仍是节点内存状态。旧 Session 收到 replaced 后尽力关闭，原子 Lease 已先确定新 current。

### EventBus

```text
Channel: nli:v2:eventbus
Message: schema_version + event_id + recipient_kind/id + envelope
```

EventBus 是 best-effort Pub/Sub，不用作队列、Replay 或业务状态。订阅节点按 current Lease/本地 Socket 投递；重复 Event 不影响授权。Pub/Sub 故障只让 Outbox 重试，24 小时后可放弃并依赖 HTTP 恢复。

### GCRA / Token Bucket

```text
Key: nli:v2:rate:{policy_id}:{subject_digest}
Value: theoretical_arrival_time_ms + schema_version
TTL: policy burst recovery horizon + grace
```

Redis Lua/Function 使用服务端时间原子判断、更新并返回 retry_after_ms。一个请求适用的 IP/Account/Family/Instance/Invite/Secret-prefix 多层 Bucket 必须全部允许；实现应提供原子多 Bucket 决策，或在无法跨 Key 原子时采取不放宽额度的保守策略。拒绝响应不返回 Bucket Key、容量或剩余额度。

### Provider Browser Transaction

```text
Key: nli:v2:provider-flow:{flow_handle_digest}
Value: AEAD encrypted schema-versioned flow payload
TTL: 10 minutes
```

Payload 固定 Provider、Purpose、Redirect、State Digest、PKCE Verifier、当前 Account/Family/Binding、Callback Result 和一次性状态。Cookie 只保存高熵 Handle；Callback 用 State 原子推进 PENDING→CALLBACK_READY。Completion 使用 `CALLBACK_READY→COMPLETING(owner,lease)→COMPLETED` fencing：先 Claim，随后以 Flow Digest 作为 PostgreSQL Idempotency Scope 提交 Account/Binding/Replay，最后 compare-set COMPLETED。Worker/请求崩溃可在 Lease 后按 Idempotency Record 查证并接管，不能在数据库提交前不可逆 CONSUME。Provider Code/Token 不进日志；完成后立即删除或用短 tombstone 防二次消费。

### Cursor

普通 Keyset Cursor 不落 PostgreSQL/Redis：它是带 key_version 的 AEAD 或 MAC 不透明值，绑定 Principal、route、filter、sort、最后排序键、UUID tie-breaker、签发和过期时间。修改、跨端点或跨 Principal 使用均返回统一 INVALID_CURSOR。

`/friends` 按 Phase 3 使用 Redis 服务端快照：

```text
Key: nli:v2:friend-snapshot:{snapshot_handle_digest}
Value: AEAD(viewer_account_id + filter_digest + ordered [{kind,resource_id,sort_name,tie_id}])
TTL: 10 minutes
```

第一页在一个读视图中只收集可见 ID 和排序键；Cursor 绑定随机 snapshot handle 与下一 offset。翻页重新验证每个资源当前可见性，删除/ban 项跳过，但排序仍使用快照键。Redis 丢失使 Cursor 统一 INVALID_CURSOR，不影响好友事实。Provider 原始分页 Cursor 不进入此快照，只存在 `friend_sync_continuations` AEAD。Cursor Key 轮换同时接受有限旧读版本，不允许旧 Key 新签发。

### Singleflight、短锁与缓存

```text
nli:v2:lock:{purpose}:{resource_digest}   TTL <= 30s, value=fencing_token+owner
nli:v2:cache:{purpose}:{resource_digest}  TTL 按用途，永远短于权威 freshness
```

锁使用 `SET NX PX`、唯一 owner 和 compare-delete；涉及持久写时 fencing token/数据库 Revision 才是最终防线。Provider Access Token Cache 本身必须 AEAD/内存安全处理且 TTL 不超过上游 expiry；Redis Cache Miss 只触发权威读取，不能改变授权。

### Redis 故障模式

- WS Lease/续租、Invite/Secret 防爆破关键 Bucket、Provider Browser Flow 和分布式 Credential Refresh 必须 fail-closed；
- 普通低风险 HTTP 读可绕过缓存继续读 PostgreSQL；
- 写操作在全局 Rate Store 不可用时使用容量更小、恢复更慢的进程内临时桶；无法证明安全时返回 503/429，绝不 fail-open；
- EventBus 故障不回滚已提交业务事务，由 Outbox 重试；
- Redis 恢复后不从陈旧本地状态回填 Lease，客户端必须重新握手。

## 全局约束与引用策略

### 外键默认行为

- 默认 `ON DELETE RESTRICT`；Account、Family、Pair、Instance、Invite、Join、Report 等业务根不得被隐式级联删除；
- `CASCADE` 只用于无独立生命周期的纯从属：Token Generation、Provider Capability/Usage/Credential Capability、ACL Matcher、Request Trait、Idempotency Secret Replay、Outbox Delivery 等；
- Audit Actor/Target、Outbox Recipient、Idempotency Principal，以及 Source/Task/Report 中仅用于历史相关性的 Session/Binding ID 不设 FK；业务删除不能改写既有审计事实；
- 允许匿名化的 Report Account/Guest FK 可设空，但只能由保留任务显式更新；
- PostgreSQL 不自动为 FK 建索引，Migration 必须为所有高频父删检查与 Join FK 建普通索引。

### 复合一致性约束

除各表局部 CHECK 外，Migration 建立以下复合 UNIQUE/FK，减少只靠应用检查：

- `token_families(token_family_id,account_id)`；Game Instance 与 NLI Join Request 同时引用 Family+Account；
- `friend_pairs(friend_pair_id,account_low_id,account_high_id)`；Friend Request 复合引用 Pair 双方；
- `provider_principals(provider_principal_id,account_id,provider_id)`；Binding 复合引用 Principal/Account/Provider；
- `provider_bindings(provider_binding_id,account_id,provider_id)`；Projection、Task 和 Source 在适用处复合引用；
- `game_instances(instance_id,owner_account_id,owner_token_family_id)`；ACTIVE 时 Family Binding 和 Owner 可复核，CLOSED 清空 Family FK 后仅保留 Session Snapshot；
- `instance_invites(instance_invite_id,instance_id)`；Resolution、Guest、Proof 和 Reservation 复合引用 Target；
- `join_requests(join_request_id,target_instance_id)`；Report 与 Reservation 复合引用上下文。

跨表动态事实（Account ACTIVE、Pair FRIENDS、Lease 有效、ACL 首条 ALLOW）不能由 FK/CHECK 表达，必须在锁事务中重验。允许使用小型延迟 Constraint Trigger 检查“Pair=PENDING 恰有一个 PENDING Request”“ACTIVE Account 有 verified CURRENT Email”等提交时不变量，但触发器不得调用网络、Redis 或 Provider。

### 唯一约束分类

- 永久资源身份：公共 UUID 主键；username/email 占用、Provider Subject Digest；
- 当前唯一：CURRENT Token Generation/Credential、ACTIVE Instance per Family、PENDING Friend/Join、ACTIVE Proxy Binding；
- 有限额度：Account Family/Binding Instance/Grant、Instance Invite/Guest/Incoming Request 使用父行锁+COUNT，不靠无锁查询；
- Secret Lookup：`(digest_key_version,digest)` 唯一并只在仍需识别期间保留；
- `expires_at > now()` 不出现在 partial index predicate，因为 PostgreSQL predicate 要求不可变表达式；使用静态状态 partial index并在查询条件中过滤数据库时间。

## 索引基线

每个索引必须对应已冻结查询或清理路径；不为 JSONB 快照建立通用 GIN：

| 查询族 | 索引键 |
|---|---|
| Account 精确 username/email | account_usernames.username / account_emails.email_normalized 唯一；状态过滤 |
| Session 列表/额度 | account_id, status, created_at DESC, session_id |
| Access/Refresh/Secret 定位 | digest_key_version, digest 唯一 |
| Incoming/Outgoing Friend Request | recipient_account_id/requester_account_id, status, created_at DESC, friend_request_id |
| Pair/Friend/Block 定位 | account_low_id, account_high_id 唯一；low/high 两侧按 relationship_status 与对应 ban Boolean 建 partial index |
| Friend/Provider Projection 列表 | account/binding + display_name_normalized + stable UUID |
| Sync Worker | status, next_attempt_at/created_at, friend_sync_task_id；到期 expires_at |
| Owner Instance 列表 | owner_account_id, created_at DESC, instance_id |
| Instance 自动关闭 | auto_close_due_at, instance_id，静态 ACTIVE partial |
| ACL 读取 | instance_id, priority；各 Matcher rule_id |
| Grant/Invite 管理 | issuer_account_id/instance_id, static status, created_at, proxy_grant_id/instance_invite_id |
| Join Target/Requester 列表 | target/source_instance_id, status, created_at, join_request_id |
| PENDING/TTL Worker | static status, expires_at, UUID |
| Report 保留 | retain_until, report_id；Legal Hold 状态 |
| Idempotency/Replay 清理 | expires_at, idempotency_record_id/secret_replay_id |
| Outbox Dispatch | status, next_attempt_at, outbox_delivery_id |
| Audit 保留/导出 | retain_until/exported_at, occurred_at, audit_event_id |

Cursor 查询索引顺序必须与 Phase 6 排序完全一致，UUID 始终为最后 tie-breaker。低选择度状态列不得单独索引；优先组合或 partial index。大表上线索引使用 `CREATE INDEX CONCURRENTLY`，但约束验证按 expand/backfill/validate 分阶段。

## TTL 与保留矩阵

| 数据 | 可用/可查询期限 | 业务元数据保留与清理 |
|---|---|---|
| PENDING_EMAIL Account | 24 小时总窗口 | 过期转 DELETED，释放 Email，Username 转 90 天 Former |
| Former Username | 90 天 | 到期硬删占用行 |
| DELETED Account Tombstone | 至少 90 天 | 无 Report/Legal Hold/FK 后硬删；有引用则延长 |
| Access / Recent Auth | 15 分钟 / 5 分钟 | 由 Generation/Family 状态即时拒绝 |
| Refresh / Family | 30 天 idle / 90 天 absolute | Family 终止或绝对到期后 24 小时删 Token Generation/Secret 历史，只留不可授权的 Family 最小行与 Audit；Family 根行按 Account 删除编排且须晚于 Relay Grant 清理 |
| Device Authorization | 10 分钟 | 消费/过期 24 小时后删元数据；Secret Replay 仅 60 秒 |
| Email Credential / Delivery Job | 30 分钟 / 不晚于 Credential 到期 | 终态 24 小时后删元数据 |
| Retired Provider Credential | 立即不可用 | 24 小时后删除密文版本 |
| Friend Request | PENDING 30 天 | 终态 30 天后删业务历史，Audit 独立 |
| Refusal Suppression | 24 小时 | Pair 无其他事实后可清理 |
| Provider Projection | 15 分钟 Fresh | 连续 30 天未确认/明确移除后清理 |
| Sync Candidate / Continuation / Task | 15 分钟 / 24 小时 / 7 天 | 到期分批硬删，父任务最后删 |
| OFFLINE Instance | 10 分钟可重连 | 满 10 分钟自动 CLOSED |
| CLOSED Instance | 不再公开可用 | 默认 30 天；引用先匿名化/清理后再删 |
| Proxy Grant / Invite 终态 | 立即不可用 | Secret Digest 终态立即清空，元数据 30 天 |
| Invite Resolution | 60 秒且一次消费 | 到期/消费 24 小时内删 |
| Guest | 5 分钟绝对期；终态后最多读 60 秒 | 可读截止清空 Token，IP Digest 最多 24 小时；最小上下文 30 天 |
| Join Request | PENDING 60 秒；NLI 终态查 24 小时；Guest 最多 60 秒 | 最小安全上下文 30 天；Report/Legal Hold 可延长 |
| Acceptance Lease | ACCEPTED 后 60 秒 | 仅保留 Request 截止字段，不续期 |
| Report | 不提供状态查询 | 默认 180 天后匿名化/清理，Legal Hold 可延长 |
| Idempotency / Secret Replay | 24 小时 / 60 秒 | Replay 先 tombstone 再删密文 |
| Outbox | 最多重试 24 小时 | PUBLISHED/ABANDONED 后 7 天 |
| Audit | 高风险 180 天、普通 30 天 | 专用权限 Worker/WORM 策略清理 |
| WS Lease / Browser Flow / Friend Snapshot / Lock | 90 秒 / 10 分钟 / 10 分钟 / 最多 30 秒 | Redis TTL 自动删除，不回填旧状态 |
| Signaling Session / Candidate Receipt / Route | 60 秒内绝对协商窗口 / 同窗口 / 20 秒连接 Lease | Receipt 最迟随 Session 终态+24小时硬删；Route TTL 不晚于 Session 截止且不回填 Payload |
| Relay Grant / Slot / Runtime Permit | 最长1小时 / Grant内 / 最长15秒 | 按 Permit→Slot→Grant 清理，且全部早于 Session 终态+24小时硬删 |
| Rate Bucket / Cache | 策略恢复窗口+grace / 用途 TTL | 不得超过权威 freshness；并发权威桶不得因时间窗口或 TTL 自动归零 |

清理时间是默认产品策略；合规配置只能在文档允许处延长，不能延长凭据有效性或授权能力。

## 清理与删除编排

所有清理 Worker：

1. 使用数据库 `now()`、有界批次和 `FOR UPDATE SKIP LOCKED`；
2. 先做状态转换/匿名化，再删从属，最后删根；
3. 每批独立提交，可重复执行并以 UUID/状态 fencing；
4. 同事务写必要 Audit/Outbox，但纯物理清理不制造客户端业务事件；
5. 检测计数、Reservation、Pair/PENDING 等不一致时 fail-closed、告警并进入修复队列，不直接猜测授权结论。

Account 删除在 30 天恢复期结束后按以下顺序编排：锁 Account；撤销 Family并关闭 Instance；终止关联 Signaling Session并撤销 Relay Grant，等待/驱动 Permit→Slot→Grant 清理且确认不存在 Relay 的 RESTRICT引用；撤销/解绑 Provider Credential、Proxy Grant、Invite；取消 PENDING Friend/Join；移除公开 Username/Email 与 Projection；按保留策略匿名化 Report/Source；最后转 DELETED Tombstone。任何步骤失败可从状态检查点恢复，不能依赖一个超大跨全表事务。

Guest/Join 清理先清 Token/IP、可空 Family FK 与查询能力，再保留不可授权的 Session ID 最小安全上下文；Report 已复制必要服务端上下文，不依赖过期 Session 继续授权。Outbox/Audit 清理使用专用角色，不能由普通 API 请求同步触发。

## 全局事务与锁顺序

默认隔离级别为 PostgreSQL READ COMMITTED；关键不变量依靠显式行锁、静态唯一约束和提交时复核，不全局使用 SERIALIZABLE。

需要多个领域行时遵循：

```text
Idempotency Record
-> Account（UUID 升序）
-> Token Family（UUID 升序）
-> Friend Pair（low/high 升序）
-> Provider Principal / Binding 或 Proxy Grant（UUID 升序）
-> Game Instance（UUID 升序）
-> Instance Invite
-> Guest Session
-> Friend/Join Request
-> Invite Reservation
-> Audit / Outbox append
```

不需要的层级跳过，但不能反向锁。读取 Request 以发现依赖时先做无锁快照，再按上述顺序锁依赖和 Request，最后重读/复核状态。Provider 外部调用、邮件发送和 EventBus Publish 不得发生在持有长数据库锁期间；WebSocket Lease CAS 是明确的跨存储短协调例外，必须按 `notifications.md` 做提交前后复核与 compare-delete 补偿。先持久化 Task/Outbox/Flow 状态，再由 Worker 执行。

数据库 Deadlock/Serialization Failure 只允许服务端做有界抖动重试，并复用同一 Idempotency 语义。客户端看到的是稳定冲突/暂时不可用，不能收到约束名或半提交结果。

## REST 契约映射复核

| Phase 6 API 族 | 权威模型 | 说明 |
|---|---|---|
| Register/Login/Refresh/Recovery | accounts、username/email、authenticator、family/generation、credential/email job、idempotency/replay | registration_id 与 account_id 分离；Token 仅 Generation/Replay 密文可恢复 |
| Device Code | device_authorizations + family/generation + replay | Approve 不建 Family，首次成功 Poll 原子消费并签发 |
| Account/Profile/Session | accounts、username/email、token_families | Account 公开字段、当前 Email、username cooldown 与 Session 列表均可直接投影 |
| Provider Registry/Login | providers/capabilities、provider_principals、login_identities、Redis Flow | Login Identity 与业务 Binding 独立但共享 Subject 唯一归属 |
| Provider Binding | provider_bindings/usages/credentials | revision CAS 和 effective capability 三层交集可表达 |
| Friendship/Block | friend_pairs、friend_requests、两类 sources | Submission Receipt 不需要泄漏隐藏 Pair 状态；Incoming/Outgoing 有确定索引 |
| Friend Aggregation/Sync | projections、tasks/results/candidates/continuations/schedules | 10 分钟 Cursor 快照由 Cursor 层实现，数据源 freshness 与 7 天 Task 可恢复 |
| Instance/ACL/WS | game_instances、ACL 子表、token family binding、LeaseStore | HTTP 生命周期持久；ONLINE/current WS 只读 Lease；config/ACL revision 分离 |
| Proxy/Invite/Resolution | proxy_grants/bindings、invites/resolutions | 单次 Secret、Generation、3/10/100 限额、Session-bound Resolution 均有约束/锁 |
| Guest/Join/Report | guest_sessions、join_requests/traits/proofs/reservations、reports | Source/Traits/Profile/Counterparty 服务端推导；Guest 单流程与 Lease 截止可表达 |
| Notifications | outbox_events/deliveries + EventBus | 事务内 Event、跨接收者同 event_id、best-effort 无 Replay |

所有 Body 禁止字段（owner_account_id、token_family_id、source、identity_traits、reservation_id、reported identity/Profile、Lease/WS 字段）都只由上述事务填充。Repository DTO 映射使用显式列清单，禁止 `SELECT *` 直接序列化；数据库内部 status、Digest、Revision Lock、Claim Owner 和保留字段不进入公共 Schema。

Phase 7 没有新增 Path、客户端字段或 Token Scope。Friend Sync 唯一澄清是 Phase 3 已存在的 ALL_PROVIDERS：Phase 6 `mode=ALL` 在 binding_id 有值时为 PROVIDER_FULL、缺失时为 ALL_PROVIDERS；这不暴露数据库结构。

## 安全与并发场景复核

### 同 username/email 并发注册

两个事务分别尝试占用相同 Username 或 Email 时，注册表唯一约束只允许一个提交；失败事务不留下 Account、Password、Credential 或 Outbox。公开错误继续区分公开 username 与隐藏 email，约束名不外泄。

结果：通过。

### Login/Refresh 网络重试与旧 Token 重用

Login/Device 首次签发在 Account 行锁下检查 Family 数量。Refresh 锁 Family+CURRENT Generation，旧行先转 ROTATED再插入新代；同 Key/输入 60 秒内解密同一 Replay，过窗旧 Refresh 命中 ROTATED 后只撤销该 Family。旧 Access 因 Generation 非 CURRENT 立即无效。

结果：通过。

### Provider Subject 跨 Login/Binding 抢占

两种用途先按 Provider/Issuer Digest 查询同一 provider_principals，并在迁移 Key 版本锁下唯一归属 Account；Provider Login Identity 和 Binding 不能分别把同 Subject 绑定不同账号。Subject 明文只在 Credential Manager 解密边界短暂存在。

结果：通过。

### Friend Request、反向接受与 ban 并发

所有路径先锁同一 low/high Pair；一个 PENDING partial unique 阻止重复请求。ban 提交后 Pair 不再 FRIENDS/PENDING并终止 Source；稍后的 Provider Candidate 必须重验 ban，不能凭旧 Projection 自动恢复。

结果：通过。

### Token Family 并发创建/关闭 Instance

Account→Family→Instance 锁序与 Family ACTIVE Instance partial unique 只允许一个创建。Close/自动关闭按 Family→Instance 解绑；不同 Idempotency Key 不能产生第二个 ACTIVE Instance。已签发 Access 仍因数据库 Family/Binding 状态被拒绝。

结果：通过。

### WebSocket 替换、数据库失败与旧 Close

新握手先验证 PostgreSQL，再原子 replace Lease；事务失败 compare-delete 新 Session。续租/delete 都比较 ws_session_id，旧 Socket 的 Ping/Close 不能覆盖新 Session。Redis 丢失令所有 Instance OFFLINE，而不删除 game_instances。

结果：通过。

### ACL 替换与 Join Accept 并发

ACL PUT 锁 Instance、校验 acl_revision、整树替换并取消不再允许的 PENDING。Accept 先按全局顺序锁 Proof 依赖、Instance、Invite/Guest、Request，最后读取最新 acl_revision并重算首条 Matcher；任一提交顺序都不能绕过最新 DENY。

结果：通过。

### Invite 轮换、撤销、超卖与修复

Secret 轮换递增 Generation，旧 Resolution/新请求不可用，已有 Reservation 保持。创建/终态都锁 Invite，计数 CHECK 与 Request 唯一 Reservation 防超卖；撤销取消 PENDING并释放。修复 Worker 只能根据 Request 终态校正，异常计数 fail-closed。

结果：通过。

### AUTO Join 与 MANUAL 审批

AUTO 在一个事务创建 ACCEPTED Request、消费 Reservation、写 60 秒 Lease 截止和 Outbox，不暴露 PENDING。MANUAL partial unique/额度只计算真实 PENDING；接受时同样重验 Session、Source、Proxy/Friend、Invite、ONLINE 和 ACL。

结果：通过。

### Guest 过期、封禁与 Report 保留

Guest Token 在 5 分钟绝对期或终态后 60 秒清空；BLOCKED 只保留固定结果读能力。Join/Report 已复制服务端上下文，不依赖 Token 或 IP 原文。IP Digest 24 小时清除，最小 Join 上下文 30 天，Report 默认 180 天且 Legal Hold 显式受审计。

结果：通过。

### Idempotency 崩溃窗口

短操作把 Idempotency Record、业务状态、Audit 和 Outbox 放同一事务，无“业务成功但结果记录缺失”。长操作 PROCESSING Lease 接管前按资源引用查证，不重复调用 Provider/邮件或生成 Secret。Secret 密文到期先写 tombstone，重试不会再执行。

结果：通过。

### Account 删除部分失败

删除编排使用 DELETION_PENDING/DELETED 状态检查点，每步幂等；Account 非 ACTIVE 后所有授权先失效，即使后续 Projection/历史物理清理延迟也不会恢复可见性。恢复只在 30 天内走原邮箱证明并创建新 Session。

结果：通过。

### Outbox/EventBus 完全丢失

业务事务不等待 Pub/Sub；Dispatcher 重试最多 24 小时并可 ABANDON。客户端在 ready/下一提示/主动刷新后从 PostgreSQL+LeaseStore重建，不能按 Event 顺序推导授权。

结果：通过。

## 命名与 Migration 约定

- 表名使用复数 `snake_case`；Schema 清单中未标 `null` 的列一律 `NOT NULL`；
- 主键使用领域名 `_id`，不统一伪装成含义不明的 `id`；
- 外键列与目标公共 ID 同名；
- 时间列使用 `_at`，计数使用 `_count`，Revision 使用 `_revision`；
- keyed lookup 值使用 `_digest` + `_digest_key_version`；仅 Password 使用 `_hash`；
- AEAD 列使用用途前缀的 `_ciphertext / _nonce / _key_version / _algorithm / _aad_version`；
- 约束和索引使用可预测的 `pk_ / fk_ / uq_ / ck_ / ix_` 前缀；
- Migration 只能前向、可审查地追加，生产环境不依赖 ORM 自动建表或 schema sync；
- 破坏性迁移遵循 expand→backfill→switch reads/writes→contract，多版本部署期间保持兼容；
- SQL Migration 与数据回填任务分离，长时间回填不得持有全表事务锁。

## Phase 7 完成条件

- [x] PostgreSQL/Redis 权威边界冻结
- [x] 全部表、列与敏感字段表示冻结
- [x] 主键、外键、唯一约束和 CHECK 冻结
- [x] 查询索引和清理索引冻结
- [x] TTL、保留与删除行为冻结
- [x] 关键事务和锁顺序冻结
- [x] Redis Key/TTL/CAS 语义冻结
- [x] API 映射、安全与竞态场景通过复核

## Phase 9 Gate F 数据模型扩展

> 状态：`GATE_F_FROZEN`。本节追加 Signaling/Relay 模型，不重排或改变 Gate E 表的含义。PostgreSQL 只保存生命周期、协议进度、最小 Receipt、Relay 授权和配额；SDP、Candidate、地址、TURN Password、Peer Pin 原值和消息 Payload 永不进入业务表、Outbox 或持久队列。

### `signaling_sessions`

| 列 | 类型/约束 | 含义 |
| --- | --- | --- |
| signaling_session_id | UUID PK，UUIDv4 | 公共相关 ID，不是凭据 |
| join_request_id | UUID FK join_requests，NOT NULL，UNIQUE | 每 Request 整个生命周期最多一个逻辑 Session |
| status | enum `ACTIVE/CLOSED/EXPIRED` | 生命周期权威 |
| protocol_phase | enum `WAITING_FOR_OFFER/WAITING_FOR_ANSWER/EXCHANGING_CANDIDATES` | 协议权威，不含 CONNECTED |
| epoch | smallint，CHECK 0..3 | 0 初始，最多两个 Restart |
| offer_digest / answer_digest | bytea nullable | 当前 epoch 原始 SDP 的 Session-scoped keyed digest |
| digest_key_version | integer nullable | 专用 Secret Store Key 版本；不保存 Key |
| requester_candidate_count / target_candidate_count | smallint CHECK 0..64 | 当前 epoch 已接受新 Candidate 数 |
| requester_ice_end_epoch / target_ice_end_epoch | smallint NOT NULL DEFAULT 0，CHECK 0..3 | 0 表示当前 epoch 尚未结束；非0为对应 Role最后完成 epoch，Restart时重置0 |
| terminal_classification | 封闭 enum nullable | 五类公开终态 |
| created_at / expires_at | timestamptz NOT NULL | 绝对窗口，不续期 |
| terminal_at / updated_at | timestamptz nullable/NOT NULL | 终态与维护时间 |
| revision | bigint NOT NULL DEFAULT 1 | CAS/Worker fencing |

约束：ACTIVE 必须 `database_now()<expires_at` 才可使用；过期行即使尚未由 Worker 改状态也不得授权。epoch=0 必须 WAITING_FOR_OFFER且 digest/count/end为空或0；WAITING_FOR_ANSWER 必须有 Offer、无当前 Answer；EXCHANGING_CANDIDATES 必须有二者。终态必须有 terminal_at/classification，ACTIVE 必须二者为空。CHECK 只表达静态形状，时间与跨行授权在锁内重验。

索引：`uq_signaling_sessions_join_request`；活动清理 `(status,expires_at,signaling_session_id)` partial；终态硬删 `(terminal_at,signaling_session_id)`；依赖撤销通过 `join_requests` 先发现后按全局锁序重读，不复制可漂移 ACL/关系真值。

### `signaling_candidate_receipts`

| 列 | 类型/约束 | 含义 |
| --- | --- | --- |
| signaling_session_id | UUID FK signaling_sessions ON DELETE CASCADE | 所属会话 |
| sender_role | enum `REQUESTER/TARGET` | 消耗额度的发送方 |
| epoch | smallint CHECK 1..3 | Candidate epoch |
| message_id | UUIDv4 | 客户端稳定消息 ID |
| payload_digest | bytea NOT NULL | HMAC-SHA-256，不含 Candidate/地址 |
| digest_key_version | integer NOT NULL | 只引用 Secret Store Key 版本 |
| accepted_at / expires_at | timestamptz NOT NULL | 接受时间和不晚于 Session 截止的清理时间 |

PK `(signaling_session_id,sender_role,epoch,message_id)`；每 Session 静态上限 384 行。处理顺序为 Session 行锁→查 Receipt：同 ID/同 digest 为 DUPLICATE且不增计数，同 ID/不同 digest 为协议违例，不存在才检查 ice_end/64额度、插入并加计数。Receipt 在终态/截止逻辑失效，建立 `ix_signaling_candidate_receipts_expiry(expires_at,signaling_session_id)`；Worker 按该索引有界硬删。Key 丢失时终止会话，不能绕过判等。

### `relay_grants`

| 列 | 类型/约束 | 含义 |
| --- | --- | --- |
| grant_id | UUIDv4 PK | 非 Bearer 相关 ID |
| signaling_session_id | UUID NOT NULL FK signaling_sessions ON DELETE RESTRICT | 来源会话；自然 EXPIRED 后可独立维护；删除顺序见保留规则 |
| role | enum `REQUESTER/TARGET` | 每 Session/Role 唯一逻辑 Grant |
| principal_type | enum `ACCOUNT_FAMILY/GUEST` | 凭据绑定类别 |
| account_id / token_family_id / guest_id | UUID nullable FK | 互斥的最小参与方绑定 |
| source_instance_id / target_instance_id | UUID nullable/NOT NULL FK | 撤销重验上下文；Guest 可无 Source Instance |
| policy | enum `ALL/RELAY` | 不允许静默降低 RELAY |
| region | text受控 Registry FK/标识 | 数据驻留与容量域，不含精确位置 |
| status | enum `ACTIVE/REVOKED/EXPIRED` | 独立 Relay 权威 |
| credential_issue_deadline | timestamptz NOT NULL | Secret 签发/Replay截止，等于 Signaling绝对截止 |
| allocation_create_deadline | timestamptz NOT NULL | 新 Allocation 激活截止，不晚于 Signaling窗口 |
| grant_expires_at | timestamptz NOT NULL | 创建后最多1小时，不滚动 |
| revision | bigint NOT NULL | 撤销/Permit fencing |
| concurrent_allocation_limit / cumulative_allocation_limit | smallint NOT NULL DEFAULT 6/24 | 固定 Grant 限额 |
| active_or_pending_count / cumulative_active_count | smallint NOT NULL DEFAULT 0 | 原子配额水位 |
| byte_limit / byte_reserved / byte_consumed | bigint NOT NULL | 默认2GiB及单调 credit账本 |
| created_at / revoked_at / revoke_reason_class | timestamptz / nullable / enum nullable | 生命周期/Audit分类 |

UNIQUE `(signaling_session_id,role)`。Principal CHECK：ACCOUNT_FAMILY 必须 account+family且 guest空；GUEST 只允许 guest。deadline 顺序 `created_at < credential_issue_deadline = allocation_create_deadline <= grant_expires_at <= created_at+1h`。计数和字节满足非负、reserved/consumed不超过 limit；REVOKED/EXPIRED 永不回 ACTIVE。Signaling自然到期不是撤销；主动 Close、显式身份/Instance撤销、ACL DENY、关系/Proxy失效、ban、超额与 Grant截止按 `signaling.md` 撤销。

索引：活动授权 `(status,grant_expires_at,grant_id)`；按 Session/Role unique；按 account/family/guest/source/target 的 ACTIVE partial 索引用于配额和撤销。Secret Replay复用版本化 AEAD专用表并以 Grant/Role/Idempotency Record关联，到 credential_issue_deadline 删除密文留 Tombstone；PG 不存 TURN Password或长期验证材料。

### `relay_allocation_slots`

| 列 | 类型/约束 | 含义 |
| --- | --- | --- |
| allocation_id | UUIDv4 PK | 内部 Allocation ID |
| grant_id | UUID FK relay_grants ON DELETE RESTRICT | 所属 Grant |
| node_id / boot_id / fence | 受控 node ID / UUID / UUID | 固定 Adapter进程和 Slot |
| status | enum `PENDING/ACTIVE/CLOSED` | Prepare/Activate状态 |
| transaction_digest / five_tuple_digest | bytea NOT NULL | Grant-scoped keyed digest，不存原 STUN transaction/5-tuple |
| digest_key_version | integer NOT NULL | Secret Store Key版本 |
| activation_deadline | timestamptz NOT NULL | 单次 Prepare许可≤2秒 |
| max_permit_not_after | timestamptz nullable | 已签发运行许可安全上界 |
| permit_sequence | bigint NOT NULL DEFAULT 0 | 单调防旧许可 |
| cumulative_counted | boolean NOT NULL DEFAULT false | 首次 ACTIVE 后永久 true |
| reserved_byte_count / consumed_byte_watermark | bigint NOT NULL | 有限 credit与单调报告 |
| activated_at / closed_at / close_reason_class | nullable timestamptz/enum | 生命周期 |
| created_at / updated_at | timestamptz NOT NULL | 清理与CAS |

UNIQUE `(grant_id,node_id,boot_id,transaction_digest,five_tuple_digest)` 合并同一 Allocate事务；不同 5-tuple、NAT rebind、TCP重连或 boot均是新 Slot。PENDING 不得转发/返回成功地址；只有 ACTIVE可取得 Runtime Permit。合法转换 `PENDING→ACTIVE|CLOSED`、`ACTIVE→CLOSED`，CLOSED不可复活；`cumulative_counted` 只可 false→true。原始 Peer IP/endpoint、Permission/Channel与Socket只在 Adapter易失内存，丢失即关闭，不从 PG猜测恢复。

Reaper 每5秒无锁发现后按锁序重读：PENDING超过 activation_deadline+5秒关闭；ACTIVE超过 max_permit_not_after+5秒且无并发新许可时关闭。索引为 `(status,activation_deadline,allocation_id)` PENDING partial、`(status,max_permit_not_after,allocation_id)` ACTIVE partial和 `(grant_id,status,allocation_id)`；另建 `UNIQUE(grant_id,node_id,boot_id,five_tuple_digest) WHERE status IN ('PENDING','ACTIVE')`，保证同一 Grant/Adapter boot/five-tuple 即使换 STUN transaction ID也只有一个 live Slot。Adapter 在调用 Prepare 前还必须按 TURN 协议拒绝已被其他 Grant占用的同一 live five-tuple，不以第二行双计配额。旧 node/boot/fence Close只能作用于自身匹配 Slot。

### `relay_quota_buckets` 与 byte-credit

Region、规范化源IP digest、Account、Guest、Source/Target Instance等需要集群原子额度的维度使用：`(bucket_type,bucket_key_digest,window_id)` PK，保存 active_count、reserved_bytes、consumed_bytes、revision、expires_at；不保存原始IP。每个受控 `bucket_type` 在部署配置Registry中固定模式和上限：并发/存量 `LIVE_GAUGE` 固定 `window_id=0`，不得按时间或TTL归零，只能在对应 Grant/Slot 安全关闭事务中递减；速率 `FIXED_WINDOW` 的 `window_id` 由数据库 `now()` 和固定UTC窗口确定，不保存并发水位；累计账本 `LEDGER` 使用受控账期且未结清credit不得因换窗消失。`expires_at` 只决定 active/reserved均为0且所有相关 Permit安全截止后的物理GC，不改变授权结果。事务先由数据库时间确定全部精确 `(bucket_type,bucket_key_digest,window_id)`，再按完整三元组排序锁定/插入并预留；释放并发额度不能返还已累计 ACTIVE次数，崩溃未确认byte-credit不返还。

`relay_runtime_permits` 以 `permit_id` UUIDv4为PK，`allocation_id UUID NOT NULL REFERENCES relay_allocation_slots(allocation_id) ON DELETE RESTRICT`，并以 `(request_nonce_digest,allocation_id)` 唯一；保存 allocation/fence、grant_revision、permit_sequence、issued_at/not_after、credit_id/byte_count和单调消费水位，不含Secret、地址或Payload。同 nonce只返回原许可/credit，不重算 not_after或重复装载额度；not_after不超过决策时15秒和Grant截止。按 `(not_after,permit_id)` 批量清理，但 Slot 的 `max_permit_not_after` 保留到安全关闭，防止先删Permit导致过早回收。

### LeaseStore / 易失状态

Signaling 路由是单一原子 Document：

```text
signaling-route:{signaling_session_id}
-> route_revision
 + REQUESTER {connection_id,node_id,boot_id,expires_at}
 + TARGET    {connection_id,node_id,boot_id,expires_at}
```

TTL 不晚于 `signaling_expires_at`；每5秒 Ping，连接 Lease 20秒。replace/renew/compare-delete和双Role check-both均是单脚本/事务原子操作，任何 Slot/revision变化使整体失败。Payload缓存、ACK、限流、Peer Pin和TURN验证材料使用隔离 Namespace，禁用RDB/AOF/Swap/跨环境复制/通用备份；EventBus不参与投递授权。Store丢失不回填Payload或Route，客户端按PG状态重连重发。

### 锁、保留、审计与迁移

全局顺序在既有 Invite Reservation 后追加：

```text
... -> Join Request -> Invite Reservation -> Signaling Session
    -> Relay quota buckets（type/key固定顺序）
    -> Relay Grant -> Allocation Slot -> Runtime Permit -> Audit / Outbox append
```

先无锁发现依赖，再从既有 Account/Family/Friend/Proxy/Instance/Invite/Guest/Request顺序加锁，禁止从 Session/Grant/Slot反锁领域行。跨存储 Route CAS是短协调例外，提交前后重验并在失败时compare-delete自身 fence；事务中不发送网络Payload。

Signaling终态供NLI参与方查询24小时（Guest受更短Token期限），随后先删Receipt再删Session；逻辑截止即时拒绝，不等待Worker。Relay Grant/Slot不随自然Session到期立即删除：最多维持至1小时Grant截止且最后Permit安全结束；同一幂等清理状态机必须先完成 Permit→Slot→Grant，再删Receipt→Session，任一步失败即保留后续父行、告警并有界重试，不能跳序或用FK错误当成功。全部Relay业务行必须在Session终态+24小时硬删前完成上述清理，因此 `relay_grants.signaling_session_id ON DELETE RESTRICT` 是顺序兜底而非级联；临时积压不延长任何授权。需要30/180天保留的分类、ID与用量桶已复制到独立Audit，不延长Relay业务行或授权。Audit普通30天、高风险沿既有180天策略；新增固定类型 `signaling.session_created/closed/authorization_lost/connection_replaced/protocol_rejected`、`relay.grant_issued/revoked/expired/allocation_abuse`、`diagnostic.breakglass_enabled/download/purged`、`audit.backfill_dropped` 只能由Migration注册。

迁移顺序：先注册enum/Audit Type和Secret用途→建表/约束/索引→部署只读兼容代码→启用写入→最后开放5个HTTP Operation与独立WS。普通coturn不能满足Slot/Permit/Pin/credit约束；生产Relay开关默认关闭，直到自定义Adapter通过真实WebRTC/TURN兼容、撤销、配额、崩溃与故障验收。
