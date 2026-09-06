# NetherLink v2 好友关系与聚合

> 状态：`PHASE_3_COMPLETE / GATE_C_PASSED`
>
> 依赖：`outline.md`、`common.md`、`nli_account.md`、`provider.md`，Gate A、Gate B 已通过。
>
> 本文档定义 NLI 好友关系、好友申请、有向 ban、Provider 外部好友投影、同步任务、聚合读模型和隐私边界。

## 目标

- 保证任意两个 NLI Account 之间只有一个确定的社交关系状态；
- 支持有方向的好友申请和双向好友关系；
- 使用有方向的 ban 阻止删除后被申请或自动同步立即恢复；
- 将 Provider 好友视为外部数据和申请来源，而不是 NLI 权限；
- 支持单个、Provider 批量、全 Provider 和自动同步；
- 在不泄漏 Provider Subject 到 NLI Account 映射的前提下完成内部解析；
- Provider 故障时仍能返回 NLI 原生好友；
- 让聚合结果能够解释来源、时效和降级状态。

## 非目标

- 不定义 Game Instance、代理授权码或 Join Request 的完整生命周期；
- 不允许 Provider 好友关系直接授予 NLI 好友权限；
- 不提供公开 `Provider Subject -> NLI Account` 查询；
- 不让客户端自行声明可信 Provider 同步来源；
- 不为未登录 NLI 的客户端提供 NLI 好友服务。

## 已确认的不变量

- 好友关系以不可变 NLI Account ID 为单位；
- 好友申请有方向，已接受关系是双向的；
- A 删除 B 时设置 A 方向的 ban；
- A 只能清除自己设置的 ban，不能清除 B 的 ban；
- Provider 自动同步不能清除任何 ban；
- Provider 同步只创建或推进好友申请，不直接绕过关系状态机；
- 外部 Provider 关系解除不会自动删除 NLI 好友；
- 同步来源只能由后端验证后生成；
- Provider 解析严格限制在同一 Provider 范围；
- 外部好友不能凭 Provider 关系获得 NLI 联机权限；
- 批量同步不得逐项泄漏外部 Subject 是否绑定 NLI。

## 设计批次

### 批次 1：NLI 好友关系、请求、ban 与并发（已完成）

- [x] 每个无序 Account Pair 使用一条规范化关系记录；
- [x] 同方向重复申请幂等返回现有请求；
- [x] 反向申请视为接受已有申请；
- [x] 普通拒绝不设 ban，另提供“拒绝并屏蔽”；
- [x] 好友申请 30 天过期；
- [x] 有向 ban 阻止对方申请和双方 Provider 同步恢复；
- [x] 设置 ban 的用户可用 `clear_own_ban=true` 显式解除并继续手动操作；
- [x] 删除宽限期内关系保留但完全隐藏，恢复账号后原样恢复；
- [x] Pair 唯一约束、行级锁、Request ID 与状态复核共同处理并发。

### 批次 2：Provider 外部投影与身份解析（已完成）

- [x] 外部投影按 `Binding ID + Adapter 规范化 Subject` 唯一；
- [x] Subject 原值加密保存，并使用 keyed HMAC 建立同域解析索引；
- [x] 投影 15 分钟内为 `FRESH`，之后为 `STALE`，30 天未确认则删除；
- [x] 未建立 NLI 关系的外部好友只返回 Binding 范围 opaque ID；
- [x] 同步只信任本次 ProviderService 读取或 15 分钟内的可信缓存；
- [x] Provider 同步 pending 在接受前不向发起方展示目标 NLI 身份；
- [x] Provider 不可用时保留投影但标记 `STALE / UNAVAILABLE`；
- [x] Provider 解绑或换绑时清理旧 Subject 的投影。

### 批次 3：好友同步任务（已完成）

- [x] 单个、Provider 批量、全 Provider 和自动同步统一使用任务模型；
- [x] 单个同步也返回 `202 + task_id`，不在响应中暴露匹配结果；
- [x] 相同账号和同步范围的活动任务合并并返回现有 Task；
- [x] 自动同步默认最短周期 6 小时，任何配置不得低于 15 分钟硬下限；
- [x] Mutual 关系可由一侧新鲜证明自动接受，方向性关系要求双方新鲜证明；
- [x] 自动接受要求接收方 Account 和对应 Binding 配置同时允许；
- [x] 单任务最多处理 500 个外部好友，超出时返回不透明 Continuation；
- [x] 取消不回滚已提交的关系变化；
- [x] 任务只返回运行统计，不返回 matched、blocked 或 unbound 数量。

### 批次 4：聚合读模型与降级（已完成）

- [x] 只有已有 NLI 好友且具有对应可信 Provider Relationship Source 时才合并外部条目；
- [x] 聚合 GET 只读 NLI 数据与投影，不实时调用 Provider；
- [x] Provider 部分不可用时返回 `200` 和 `source_statuses`；
- [x] NLI 好友优先，各组按规范化展示名和稳定 ID 排序；
- [x] 聚合分页使用短期服务端查询快照和 opaque Cursor；
- [x] 用户只能查看自己设置的 ban，并通过独立屏蔽列表管理；
- [x] 申请来源接受后转为 Relationship Source，外部关系解除后标记历史；
- [x] 代理实例明确返回真实 Owner 和展示归属账号；
- [x] 不基于显示信息或内部解析自动合并多个 Provider 条目。

## NLI 好友 Pair

每个无序 Account Pair 使用一条规范化记录：

```text
account_low_id = min(account_a_id, account_b_id)
account_high_id = max(account_a_id, account_b_id)
UNIQUE(account_low_id, account_high_id)
CHECK(account_low_id < account_high_id)
```

自身不能与自身建立 Pair、申请、好友或 ban。

字段范围：

- Pair ID（UUIDv4）；
- `account_low_id`；
- `account_high_id`；
- 关系状态；
- 当前 Request ID；
- 待处理申请发起方；
- 申请创建与过期时间；
- 双方有方向的 ban 和设置时间；
- 成为好友的时间；
- 最近拒绝的申请方和拒绝时间；
- 最近人工操作时间；
- 创建与更新时间；
- 并发版本号。

不存在 Pair 行等价于 `NONE + 双方无 ban`。当关系回到 `NONE` 且双方均无 ban、拒绝抑制窗口已结束、也无必须保留的短期并发信息时可以物理删除 Pair 行。

申请来源证明不塞入 Pair 单列；使用独立的 Request Source 记录，以支持同一 pending 被多个可信来源幂等命中。具体来源在 Provider 投影批次定义。

## 关系与 ban 状态

核心关系状态：

```text
NONE
PENDING
FRIENDS
```

ban 与核心状态正交保存：

```text
ban_by_low
ban_by_high
```

因此可以表达 `NONE + A ban B`、`NONE + 双方互相 ban` 等状态。正常情况下 `PENDING` 和 `FRIENDS` 不应与任意 ban 同时存在；所有写操作必须在事务内维持此不变量。

### 基本转换

```text
NONE -> PENDING                    # 发起好友申请
PENDING -> FRIENDS                 # 接收方接受
PENDING -> FRIENDS                 # 接收方发送反向申请
PENDING -> NONE                    # 发起方取消
PENDING -> NONE                    # 接收方拒绝
PENDING -> NONE + recipient_ban    # 接收方拒绝并屏蔽
PENDING -> NONE                    # 30 天过期
FRIENDS -> NONE + actor_ban        # 任一方删除好友
任意状态 -> NONE + actor_ban       # 主动屏蔽
NONE + own_ban -> PENDING/FRIENDS  # 显式清除自己的 ban 后继续人工操作
```

### 发送申请

- 最终写操作必须使用目标 Account ID；username 或 display name 只用于查找候选；
- 同方向已经 `PENDING` 时内部复用同一 Request ID，不重复创建或通知；REST 提交响应使用统一 Submission Receipt，不直接暴露本次是否创建了 Pending；
- 已经 `FRIENDS` 时按幂等成功返回当前关系；
- 反方向已经 `PENDING` 时，当前发送动作视为接受并原子转为 `FRIENDS`；
- 目标设置的 ban 会阻止申请，但响应不能向发起方暴露 ban；
- 发起方自己设置了 ban 时，只有明确提交 `clear_own_ban=true` 才能原子清除并继续；
- 接收方普通拒绝后，同一发起方在 24 小时内再次申请会被静默抑制：返回与已接收提交一致的响应，但不创建 pending 或通知；
- 拒绝方在抑制窗口内主动向原发起方申请不受影响，因为这是对方主动表达同意；
- 每账号最多同时拥有 100 个可见的 NLI 搜索 outgoing pending、500 个隐藏的 Provider-sync outgoing pending，以及 500 个 incoming pending；
- 可见 outgoing 达到上限时返回账号自身限流错误；隐藏 Provider pending 或目标 incoming 达到上限时静默处理，不能通过错误或 Task 统计暴露容量；
- 两类 outgoing 分开计数，避免手动申请额度变化泄漏隐藏 Provider pending 是否存在；
- 申请有效期为 30 天；过期申请在读取或写入时可以惰性转为 `NONE`，并由后台清理任务兜底；
- 新的 pending 使用新的 UUIDv4 Request ID，不能复用已结束申请的 ID。

### 接受、拒绝与取消

- 接受和拒绝只能由当前 Request ID 的接收方执行；
- 取消只能由当前 Request ID 的发起方执行；
- 普通拒绝回到 `NONE`，不设置 ban，但为相同原申请方记录 24 小时静默抑制窗口；
- “拒绝并屏蔽”回到 `NONE` 并设置接收方方向 ban；
- 取消、重复取消和对已过期请求的处理必须使用稳定幂等语义；
- 对已经被另一并发操作替换的新 Request ID，旧接受/拒绝请求返回冲突，不能作用于新申请。

### 删除、屏蔽与恢复

A 删除好友 B 时：

- 双向 `FRIENDS` 关系立即解除；
- 设置 `ban_by_a = true`；
- B 不能清除 A 的 ban；
- B 后续申请以及双方 Provider 同步都不能恢复关系；
- 重复删除保持同一结果。

A 可以：

- 单独解除自己的 ban；或
- 在手动重新添加/接受操作中提交 `clear_own_ban=true`，由后端原子解除并继续。

自动同步、批量同步和 Provider 回调永远不能携带 `clear_own_ban=true`。

### 账号生命周期

- `DELETION_PENDING` 账号相关 Pair 在 30 天宽限期内保留；
- 该账号从好友列表、申请列表、搜索和实例聚合中完全隐藏；
- 对其发起的新申请或执行 Provider 同步不会产生可见关系变化；
- 账号恢复后原 Pair、ban 和好友关系恢复可见，但宽限期内自然到期的 pending 不恢复；
- 账号永久进入 `DELETED` 后，清理其 pending、好友关系、ban 和 Provider 来源记录；
- 清理过程保留必要的最小安全审计，但不保留可继续使用的社交图占位。

## 并发与事务规则

所有 Pair 写操作必须：

1. 规范化两个 Account ID；
2. 通过唯一约束创建或加载 Pair；
3. 对 Pair 加行级写锁；
4. 在锁内处理 pending 过期；
5. 重新检查账号状态、关系状态、Request ID 和双方 ban；
6. 执行一次状态转换；
7. 原子写入事件/通知 Outbox；
8. 提交后再异步发送通知。

确定性规则：

- 双方并发申请：第一个创建 `PENDING`，第二个看到反向 pending 后转为 `FRIENDS`；
- 接受与删除/屏蔽并发：按锁顺序执行，后执行者必须基于最新状态复核；若屏蔽后执行，最终一定为 `NONE + ban`；
- 接受与拒绝并发：只有第一个匹配当前 Request ID 的转换成功，另一个获得已结束或冲突结果；
- 旧请求操作不能影响后来创建的新 Request ID；
- HTTP 重试同时依赖 Idempotency Key 与关系状态幂等，不能重复通知；
- 数据库唯一约束是最终防线，不能只依赖应用层查询。

## Provider 外部好友投影

外部投影缓存通过 ProviderService 读取的好友数据。投影不能证明目标当前绑定 NLI，也不能单独授予好友或联机权限。

### 标识与唯一性

每条投影使用 UUIDv4 `external_friend_id`，并满足：

```text
UNIQUE(owner_binding_id, subject_lookup_hmac)
```

`external_friend_id` 只在所属 Binding 和 NLI Account 范围内有效：

- 不同 Binding 即使指向同一 Subject，也使用不同 ID；
- 客户端不能用一个 Binding 的 ID 操作另一个 Binding；
- ID 不编码 Provider Subject、HMAC 或 NLI Account ID；
- 删除后不能将旧 ID 分配给其他外部好友。

字段范围：

- External Friend ID；
- Owner Binding ID；
- Provider ID 与 Issuer 快照；
- `subject_lookup_hmac`；
- 加密的规范化 Subject；
- 经过清洗的 Provider 展示信息；
- Provider 关系类型；
- 首次发现时间；
- 最后确认时间；
- 新鲜度状态；
- 最近同步批次；
- 创建与更新时间。

### Subject 保护与同域索引

Adapter 负责把 Provider Subject 规范化为稳定字节序列。Friendship 服务随后计算：

```text
subject_lookup_hmac = HMAC(
    versioned_lookup_key,
    provider_id || issuer || normalized_subject
)
```

规则：

- HMAC 密钥来自进程外 Secret Store，并与 Provider Token 加密密钥分离；
- 保存 HMAC Key Version，支持渐进轮换；
- Subject 原值使用 AEAD 加密，只有 ProviderService 的受控调用路径可以解密；
- 对加密值使用 External Friend ID、Binding ID、Provider ID、Issuer 和 Key Version 作为 Associated Data；
- 普通查询、日志、指标和错误响应不能返回 HMAC 或加密 Subject；
- Provider Binding 保存兼容的同域 lookup HMAC，使服务端可做等值解析；
- 不允许跨 Provider ID 或 Issuer 比较、合并 Subject。

### 新鲜度和保留期

```text
FRESH       # 距最后成功确认不超过 15 分钟
STALE       # 超过 15 分钟，或 Provider 当前不可用
```

- 成功读取 Provider 好友列表时更新 `last_confirmed_at`；
- 15 分钟内的可信结果可以被同一 Binding 的同步操作复用；
- 超过 15 分钟的投影只能用于带时间标记的展示，不能作为新同步申请的关系证明；
- 连续 30 天未再次确认的投影删除；
- Provider 临时故障不立即删除投影，但将相关来源标记为 `UNAVAILABLE`；
- Provider 明确返回好友已不存在时，可立即标记失效，但不会删除已经建立的 NLI 好友。

### Provider 关系语义

Adapter 必须将上游关系映射为明确的强类型关系，例如 `MUTUAL_FRIEND`、`FOLLOWING` 或其他 Provider 特有类型。只有 Adapter 明确声明足以代表好友的关系类型才能作为同步证明。

客户端展示文本、昵称或头像不能作为身份或关系证明。

## Provider 范围内身份解析

内部解析严格使用：

```text
Provider ID + Issuer + Subject Lookup HMAC
```

单个同步流程：

1. 验证 External Friend ID 属于当前 Account 的指定 Binding；
2. 验证 Binding 为 `ACTIVE` 且具有读取好友用途；
3. 通过本次 ProviderService 读取结果，或不超过 15 分钟的可信缓存，确认关系仍存在；
4. 只在相同 Provider ID 与 Issuer 下查找目标有效 Binding；
5. 验证目标账号为 `ACTIVE`、允许接收好友请求且目标 Binding 未禁用；
6. 检查规范化 Pair 和双方 ban；
7. 根据同步规则创建或推进申请；
8. 对发起方只返回统一处理结果。

禁止：

- 公开 Subject/HMAC 到 NLI Account 查询接口；
- 使用邮箱、显示名或 MC Profile 推断绑定；
- 跨 Provider 或跨 Issuer 解析相同字符串；
- 信任客户端声明的目标 NLI Account 或同步来源；
- 使用 `STALE` 投影创建新申请。

## 未建立 NLI 关系的外部好友

聚合查询可以显示 Provider 公开资料，但只返回：

- `external_friend_id`；
- Provider ID 和展示名称；
- 经清洗的昵称、头像等 Provider 公开字段；
- Provider 关系类型；
- `last_confirmed_at`；
- 新鲜度和来源可用状态。

不得返回：

- 原始 Provider Subject；
- Subject Lookup HMAC；
- 是否存在匹配 NLI Account；
- 匹配的 Account ID、username 或 Binding ID；
- 对方是否设置 ban、关闭好友请求或处于非 ACTIVE 状态。

## Provider 同步 pending 的可见性

Provider 同步成功匹配 NLI Account 并创建 pending 后：

- 接收方可以看到发起方的 NLI 公开身份、可信 Provider 来源和申请时间；
- 发起方在接受前看不到目标 NLI Account、username 或 Binding 信息；
- 发起方只看到外部好友条目和统一“同步已处理”结果，不能根据字段变化判断是否创建 pending；
- 只有接收方接受，或满足自动接受条件后，双方才在 NLI 好友列表中看到对方；
- 普通 NLI 用户搜索发起的申请仍可在发起方的 outgoing 列表中展示目标，因为目标身份已经由用户搜索得到。

## 投影生命周期

- Binding 进入 `REAUTH_REQUIRED`：保留投影并标记来源不可用；
- Binding 主动 `DISABLED`：隐藏其外部投影，不作为同步证明；
- 同 Subject 重验证成功：可以保留投影，并在下次读取后恢复 FRESH；
- 显式换绑：立即删除或过期旧 Subject 投影，不能映射到新 Subject；
- 解绑：按 `provider.md` 清理对应投影；
- Provider `DISABLED`：保留至 30 天上限并标记不可用；
- Account `DELETION_PENDING`：隐藏投影；
- Account `DELETED`：删除投影和 Subject 密文。

## 好友同步任务

### 任务类型

```text
SINGLE_EXTERNAL_FRIEND
PROVIDER_FULL
ALL_PROVIDERS
AUTO_PROVIDER
```

所有类型都异步执行并返回：

```text
HTTP 202 Accepted
{
  "task_id": "uuid",
  "status": "PENDING"
}
```

即使同步单个外部好友，也不能在创建任务响应中返回“已匹配”“未绑定”“被 ban”或“关闭请求”等结果。

`ALL_PROVIDERS` 是父任务，为每个符合条件的 Binding 创建或复用 Provider 子任务。一个 Provider 失败不取消其他子任务。

### 任务状态

```text
PENDING -> RUNNING -> SUCCEEDED
                   -> PARTIALLY_SUCCEEDED
                   -> FAILED
PENDING -> CANCELLED
RUNNING -> CANCEL_REQUESTED -> CANCELLED
```

- `SUCCEEDED`：任务处理完本次允许范围，不代表发现或创建了 NLI 关系；
- `PARTIALLY_SUCCEEDED`：部分 Provider 失败、达到 500 条上限或存在 Continuation；
- `FAILED`：任务在处理任何候选前失败；
- `CANCEL_REQUESTED`：Worker 在安全检查点停止；
- `CANCELLED`：未开始部分停止，已提交关系变化不回滚。

任务字段至少包括：

- Task ID（UUIDv4）；
- Owner Account ID；
- 类型与 Scope；
- Binding ID / Provider ID；
- 父 Task ID；
- 状态；
- `scanned_count`；
- `processed_count`；
- `failed_count`；
- Provider 级分类错误；
- Continuation 引用；
- 创建、开始、完成和更新时间；
- 取消请求时间；
- 幂等响应引用。

任务只允许 Owner Account 查询和取消。结果默认保留 7 天，之后 Task ID 返回已过期语义，但关系状态不受影响。

### 任务创建、幂等与合并

手动创建任务必须携带 `Idempotency-Key`，按 `common.md` 默认保留 24 小时结果。

同时还使用活动任务合并键：

```text
SINGLE:   account_id + binding_id + external_friend_id
PROVIDER: account_id + binding_id
ALL:      account_id
AUTO:     account_id + binding_id
```

相同合并键已经存在 `PENDING / RUNNING / CANCEL_REQUESTED` 任务时，返回现有 Task，不重复排队或读取 Provider。

Idempotency Key 防止相同请求重放；活动任务合并防止不同 Key 对相同范围制造并发任务。任务完成后，新请求仍需满足同步间隔和限流。

### Provider 读取与交互式凭据

具有后台 Refresh Grant 时，Worker 通过 ProviderService 获取本次好友列表。

不具有后台 Refresh Grant 时：

1. 用户在场时完成交互式 Provider 授权；
2. API 请求内通过 ProviderService 读取所需外部好友页；
3. 将规范化结果写入外部投影并形成不含原始 Token 的候选快照；
4. 删除交互式凭据；
5. Worker 只处理该候选快照，不在后台保存或复用 Provider Token。

超过 15 分钟的旧投影不能为了异步便利而作为关系证明。

### 候选处理

每个候选在独立 Pair 事务中处理：

1. 验证 Task、Owner、Binding 和 Provider Scope；
2. 验证候选来自本次读取或 15 分钟内可信投影；
3. 做同 Provider、同 Issuer HMAC 解析；
4. 检查目标账号和 Binding 是否允许 Provider 好友同步；
5. 检查双方 ban；
6. 若已有同方向 pending，仅幂等追加可信来源；
7. 若已有反向 pending，按核心状态机转为 `FRIENDS`；
8. 若满足自动接受条件，直接转为 `FRIENDS`；
9. 否则创建对发起方隐藏目标身份的 Provider pending；
10. 原子写入 Request Source、Outbox 和任务内部结果；
11. 对外只累加非敏感运行统计。

单个候选失败不能回滚其他候选。数据库瞬时错误可以重试 Pair 事务，但 Provider 写入和通知必须保持幂等。

### Request Source

每个 pending 可以有一个或多个不可由客户端伪造的来源记录：

```text
NLI_SEARCH
PROVIDER_SYNC
```

Provider 来源至少记录：

- Request ID；
- Provider ID 与 Issuer；
- 发起方 Binding ID；
- 目标 Subject Lookup HMAC 的受保护引用；
- 关系类型；
- 验证时间；
- Task ID；
- 创建时间。

同一 Request、Provider、Binding 和目标引用使用唯一约束，重复同步不会创建重复来源或重复通知。

### 自动接受

Provider 同步只有同时满足以下条件才能自动接受：

- 双方均为 `ACTIVE` NLI Account；
- 接收方 Account 允许接收好友请求；
- 接收方对应 Binding 为 `ACTIVE`；
- 接收方该 Binding 开启 `auto_sync`；
- 双方均未对另一方设置 ban；
- Adapter 将关系证明为 `MUTUAL_FRIEND`，或方向性 Provider 具有双方各自在 15 分钟内确认的关系证明；
- 当前 Pair 没有与转换冲突的更新。

自动接受不允许清除 ban，也不能仅根据历史投影执行。

如果条件不完整但允许创建申请，则保持普通 Provider pending，由接收方手动决定。

### 分页和 500 条上限

每个 Provider 子任务最多处理 500 个候选。达到上限且 Provider 仍有后续页时：

- Task 进入 `PARTIALLY_SUCCEEDED`；
- Provider 原始 Cursor 不返回客户端；
- 服务端保存与 Owner、Binding、Provider 和快照版本绑定的短期 Continuation；
- 客户端可以用不透明 Continuation 创建后续任务；
- Continuation 默认 24 小时过期且只能消费一次；
- 后续任务仍受活动任务合并、Provider 限流和 ban 规则约束。

### 自动同步

- 每个 Binding 独立启用 `auto_sync`；
- 默认最短调度周期为 6 小时；
- 管理员可以调大周期，但任何自动或手动 Provider 批量同步都不能突破 15 分钟硬下限；
- 调度加入随机抖动，避免整点请求风暴；
- 只有具备 `BACKGROUND_REFRESH` 和有效长期 Grant 的 Binding 才进入后台调度；
- 无后台能力的 Binding 保留手动交互式同步；
- Provider 限流、熔断或临时故障只延后对应 Binding；
- `REAUTH_REQUIRED / DISABLED` Binding 不调度。

### 取消和重试

取消是尽力而为：

- `PENDING` 可以直接取消；
- `RUNNING` 转为 `CANCEL_REQUESTED`；
- Worker 在分页和候选之间检查取消；
- 已提交的 pending、friend、来源和通知不回滚；
- 取消不能清除 ban 或删除用户后来人工操作的关系。

失败任务的重新执行创建新 Task，并受 Idempotency Key、活动任务合并和同步间隔约束。

### 限流与统计隐私

初始约束：

- 同一 Binding 的 Provider 批量同步最小间隔 15 分钟；
- 单任务最多处理 500 个外部好友；
- 同一 External Friend 的单个同步最小间隔 15 分钟；
- 单账号单个同步默认每 15 分钟最多 30 次；
- 自动同步默认最短周期 6 小时；
- 额外应用 Account、IP、Provider 和全局 Worker 容量限制。

用户可见统计只包括：

- `scanned_count`；
- `processed_count`；
- `failed_count`；
- 父任务的 Provider 总数和完成数；
- 经过分类的 Provider 级错误；
- 是否存在 Continuation。

不得返回或推导：

- 匹配 NLI 的数量；
- 未绑定数量；
- 被 ban 数量；
- 关闭好友请求数量；
- 自动接受数量；
- 每个 Subject 的内部处理结果。

单个任务也使用相同响应形状，避免通过统计字段形成特例侧信道。

## 好友聚合读模型

聚合 GET 只读取 NLI 数据库、实例读模型和已有 Provider 投影，不在请求内调用 Provider。刷新外部数据必须显式创建同步任务。

基础响应：

```json
{
  "items": [],
  "next_cursor": null,
  "source_statuses": []
}
```

`items` 使用统一联合类型：

```text
NLI_FRIEND
EXTERNAL_FRIEND
```

### NLI Friend 条目

至少包括：

- `kind = NLI_FRIEND`；
- Account ID；
- 当前公开 username 和 display name；
- `relationship = FRIENDS`；
- Relationship Source 列表；
- 当前查看者可见的 Game Instance 摘要；
- 关系建立和更新时间。

不能返回对方邮箱、Provider Subject、Binding ID、Session、对方设置的 ban 或不可见实例。

### External Friend 条目

至少包括：

- `kind = EXTERNAL_FRIEND`；
- Binding 范围的 External Friend ID；
- Provider ID 和展示名；
- Provider 公开昵称、头像和关系类型；
- `last_confirmed_at`；
- `freshness = FRESH / STALE`；
- 来源可用状态。

External Friend 条目：

- 不包含 Account ID 或 username；
- 不包含 NLI 关系或实例权限；
- 不返回是否存在内部 NLI 匹配；
- 不能直接用于好友可见实例或 Join 权限；
- 只能作为创建异步同步 Task 的 Binding 范围目标。

## 合并和去重

外部投影只有同时满足以下条件才合并到 NLI Friend 条目：

- 当前 Pair 已为 `FRIENDS`；
- 该 Relationship 存在对应 Provider、Issuer 和 Subject 的可信 Provider Source；
- 当前外部投影与该 Source 属于同一查看者 Binding；
- Source 未被撤销或判定为错误关联。

仅仅因为后端可以把外部 Subject 解析到该 NLI Account，不足以合并或向查看者暴露关联。

跨 Provider 规则：

- 不按显示名、头像、UUID 外观或其他启发式信息合并；
- 不因为多个 Provider Subject 内部解析到同一 NLI Account 就自动合并；
- 已是 NLI 好友时，每个 Provider 仍需各自拥有该 Relationship 的可信来源记录才可附加；
- 不满足条件的投影继续显示为独立 External Friend 条目。

## Relationship Source

好友申请转为 `FRIENDS` 时，其 Request Source 转换或复制为 Relationship Source：

```text
NLI_SEARCH
PROVIDER_SYNC
```

来源至少包括：

- Source ID；
- 类型；
- Provider ID 和 Issuer（Provider 来源）；
- 受保护的 Subject 关联引用；
- 首次和最后验证时间；
- `CURRENT / HISTORICAL / UNAVAILABLE` 状态；
- 创建与更新时间。

来源只用于解释关系如何建立和当前外部关联状态，不是好友或联机权限依据。

当 Provider 外部关系解除时：

- NLI Pair 保持 `FRIENDS`；
- 对应来源转为 `HISTORICAL`；
- 不自动删除好友；
- 其他来源不受影响。

Provider 临时不可用时来源转为展示层 `UNAVAILABLE`，不能误标为历史解除。

## Source Status

`source_statuses` 解释本次聚合所使用的数据源：

```text
AVAILABLE
FRESH
STALE
UNAVAILABLE
REAUTH_REQUIRED
DISABLED
```

每个状态项可以包含：

- 来源类型和 Provider ID；
- 当前账号自己的 Binding ID；
- 状态；
- `last_success_at`；
- 数据截止时间；
- 可公开的分类错误；
- 是否可以创建手动同步任务。

NLI 原生来源正常且部分 Provider 失败时仍返回 `200 OK`。只有 NLI 核心好友数据本身不可用时才返回整体服务错误。

`source_statuses` 不能包含 Provider 原始响应、Subject、目标匹配数、ban 数或未绑定数。

## 稳定排序与分页

排序固定为：

1. `NLI_FRIEND`；
2. `EXTERNAL_FRIEND`；
3. 各组内按规范化展示名升序；
4. 最后按 Account ID 或 External Friend ID 升序。

该端点明确采用比 `common.md` 默认弱一致 Cursor 更强的短期查询快照：

- 第一页创建只包含可见条目 ID 和排序键的服务端快照；
- 快照默认保留 10 分钟；
- opaque Cursor 绑定 Account、过滤条件、快照 ID 和下一偏移；
- 翻页顺序使用快照中的排序键，期间改名不会造成当前遍历重复或漏项；
- 条目读取时仍重新执行可见性检查，被删除、被 ban 或进入删除宽限期的账号直接跳过；
- 新增好友或外部投影在新的第一页查询中出现；
- 快照过期、篡改或 Viewer 不匹配返回 `400 Bad Request`；
- `limit` 继续使用公共默认 20、最大 100。

快照只保存排序所需的非秘密引用，不保存 Provider Token 或解密 Subject。

## 好友申请与屏蔽读模型

### Incoming Requests

接收方可以看到：

- Request ID；
- 发起方 NLI 公开身份；
- 到期时间；
- 经过清洗的来源类型；
- Provider 来源的 Provider 展示名和验证时间。

不能返回发起方 Binding ID、Provider Subject 或内部匹配过程。

### Outgoing Requests

- NLI 搜索来源的 pending 可以显示目标公开身份；
- Provider 同步来源的 pending 在接受前不显示目标 NLI 身份，也不进入可枚举的普通 outgoing 列表；
- Task 查询只显示隐私安全的运行状态。

### Own Blocks

用户可以通过独立列表查看和解除自己设置的 ban。列表可以显示被屏蔽账号当前允许公开的 NLI 资料，但不能显示对方是否反向 ban 当前用户。

如果目标处于 `DELETION_PENDING / DELETED`，按账号生命周期隐藏或清理，不通过屏蔽列表泄漏状态。

## 可见实例与代理来源

只有 `NLI_FRIEND` 条目可以包含当前查看者经 Instance ACL 获准看到的实例摘要。实例摘要查询必须具有当前绑定且 ONLINE 的 Source Instance Session；普通未绑定 Account Session 仍可查询好友，但不返回可加入实例。外部 Provider 好友不形成联机身份或第三方联机渠道。完整 ACL 语义由 `game_instance.md` 冻结。

直接实例：

```text
publication_source = DIRECT
owner_account_id = 好友 Account ID
presented_under_account_id = 好友 Account ID
```

代理实例：

```text
publication_source = PROXY
owner_account_id = 实例真实 Owner Account ID
presented_under_account_id = 当前好友 Account ID
```

规则：

- 必须明确区分真实 Owner 与列表展示归属；
- 代理发布不转移实例控制权；
- 不返回代理授权码、Grant Secret 或内部验证材料；
- 客户端不能自行声明 `publication_source` 或 Owner；
- Direct 实例使用 `DIRECT_FRIEND + FRIEND_LIST` ACL Context；
- Proxy 实例使用 `NLI_ACCOUNT + PROXY_FRIEND + FRIEND_LIST` ACL Context；
- ACL 结果不是 ALLOW 时，实例不可见且不能创建 Join Request；
- 授权失效时该代理实例立即从后续查询隐藏；
- 实例字段、ACL 复核和并发语义由 `game_instance.md` 定义。

## Provider 降级查询

- Provider `TEMPORARILY_UNAVAILABLE`：返回缓存投影，标记 `STALE / UNAVAILABLE`；
- Binding `REAUTH_REQUIRED`：保留允许展示的最后投影，标记需要重新验证；
- Binding `DISABLED`：不返回该 Binding 的 External Friend 条目；
- Provider Registry `DISABLED`：可在 30 天投影保留期内展示陈旧资料并标记禁用；
- 投影超过 30 天未确认：删除且不再展示；
- 任何 Provider 降级都不移除 NLI Friend 条目或阻断 NLI 原生列表。

## 安全与并发场景走查

### 同方向重试与反向申请

- A 首次申请 B 创建一个 Request ID；
- A 使用相同或不同 Idempotency Key 重试时仍返回同一 pending；
- 不重复发送通知；
- B 随后主动申请 A，被解释为接受现有申请；
- Pair 原子转为 `FRIENDS`，不存在两条方向相反的 pending。

结果：通过。

### 旧接受请求与新 pending

- A→B 的 Request 1 被 B 拒绝；
- 抑制窗口结束后 A→B 创建 Request 2；
- B 延迟到达的 Request 1 接受操作携带旧 Request ID；
- 后端在 Pair 锁内发现 ID 不匹配并拒绝；
- Request 2 不会被旧操作错误接受。

结果：通过。

### 拒绝后的重复骚扰

- B 普通拒绝 A，不设置永久 ban；
- 24 小时内 A 重复申请得到统一已接收响应；
- 不创建 pending，不通知 B；
- B 仍可主动向 A 发送申请；
- 若 B 选择拒绝并屏蔽，则 ban 持续到 B 主动解除。

结果：通过。

### 接受、删除与屏蔽并发

- Accept、Delete 和 Block 都锁定同一规范化 Pair；
- 每个后执行操作重新读取最新状态；
- 如果 Block 最后提交，最终一定是 `NONE + blocker ban`；
- 已提交事件通过事务 Outbox 去重，不出现 FRIENDS 状态下残留 ban。

结果：通过。

### 删除后 Provider 自动同步

- A 删除 B 并设置 A 的有向 ban；
- 任一方自动同步再次读到外部好友关系；
- 候选处理在 Pair 锁内看到 ban 并静默跳过；
- Task 统计不显示 blocked；
- 自动同步不能携带 `clear_own_ban`。

结果：通过。

### 单个同步枚举目标绑定

- 请求只接受属于当前 Binding 的 opaque External Friend ID；
- 无论目标未绑定、关闭请求、被 ban、达到容量或成功创建 hidden pending，均返回 `202 + task_id`；
- Task 对候选业务结果统一计为 processed，不返回 matched/blocked/unbound；
- 发起方 outgoing 列表不显示 hidden pending 或目标身份；
- 只有接收方同意或明确配置满足自动接受后，关系才向双方可见。

结果：通过。

### 跨 Binding 和跨 Provider ID 重放

- External Friend ID 绑定 Owner Account 与 Binding；
- 在其他 Binding、Account 或 Provider Scope 使用时返回统一不可用语义；
- Subject HMAC 包含 Provider ID 和 Issuer；
- 相同 Subject 字符串不会跨域解析或合并。

结果：通过。

### 自动接受方向性关系

- Adapter 将关系分类为方向性 `FOLLOWING`；
- 只有发起方一侧证明时不能自动接受；
- 双方均有 15 分钟内新鲜证明，且接收方 Account 和 Binding 配置允许时才自动接受；
- `MUTUAL_FRIEND` 可由 Adapter 明确保证互惠语义，避免重复上游调用。

结果：通过。

### 无后台 Grant 的手动同步

- 用户在场时完成交互式授权；
- ProviderService 在请求上下文读取并规范化候选；
- Token 删除后 Worker 只处理候选快照；
- 不把临时 Token 写入 Task、投影或日志；
- 自动调度跳过该 Binding。

结果：通过。

### Provider 宕机时聚合查询

- GET 不发起 Provider 调用；
- NLI Friend 条目正常返回；
- 现有外部投影标记 `STALE / UNAVAILABLE`；
- 响应为 `200 OK` 并包含 `source_statuses`；
- 陈旧投影不能用于创建新同步申请。

结果：通过。

### 外部关系解除

- Provider 后续确认 A 与 B 不再是外部好友；
- 对应 Relationship Source 转为 `HISTORICAL`；
- NLI Pair 仍为 `FRIENDS`；
- 不自动撤销实例可见性或删除好友；
- 后续权限只根据 NLI 关系和实例策略计算。

结果：通过。

### 删除宽限期与永久删除

- B 进入 `DELETION_PENDING` 后从好友、请求、屏蔽和聚合结果隐藏；
- Pair 和来源在 30 天内保留；
- B 恢复后未自然过期的数据重新可见；
- B 永久删除后清理 Pair、投影和 Subject 密文；
- 不留下可用于探测已删除账号的社交占位。

结果：通过。

### 快照分页期间状态变化

- 第一页创建 10 分钟排序快照；
- 翻页期间显示名变化不会重复或漏项；
- 若某好友被删除、屏蔽或进入删除宽限期，读取该快照项时重新鉴权并跳过；
- 新关系只在新的第一页查询出现；
- 过期 Cursor 明确失败，不回退到不稳定分页。

结果：通过。

### 代理实例来源

- 代理实例在好友聚合中携带 `publication_source=PROXY`；
- 同时返回真实 Owner 和 `presented_under_account_id`；
- 不返回授权码或 Grant Secret；
- 代理授权失效后实例从后续查询隐藏，不改变 Pair。

结果：通过。

## Gate C 评审

Gate C 冻结以下边界：

- 规范化单 Pair、`NONE / PENDING / FRIENDS` 与 Request ID；
- 有向 ban、24 小时拒绝抑制和显式恢复；
- Provider Subject 加密、同域 HMAC 和 opaque External Friend ID；
- 统一异步同步 Task、自动接受证明和枚举防护；
- Relationship Source、聚合合并规则和 Provider 部分降级；
- 快照 Cursor、代理实例来源和 Viewer 可见性复核。

数据库字段和索引名留到数据模型阶段；API Path 由 `rest_api.md` 冻结并将在 OpenAPI 中机械化表达。后续阶段不得改变以上领域语义，除非重新打开 Gate C 并记录决策。

## Phase 3 完成条件

- [x] NLI 好友关系状态机冻结
- [x] 有向 ban 与恢复语义冻结
- [x] Provider 投影和同域解析冻结
- [x] 同步任务、自动接受和幂等规则冻结
- [x] 聚合读模型和 Provider 降级冻结
- [x] 隐私、枚举和并发场景通过走查
- [x] Gate C 评审通过
