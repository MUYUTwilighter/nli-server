# NetherLink v2 Game Instance 与加入流程

> 状态：`PHASE_4_COMPLETE / GATE_D_REPASSED`
>
> 依赖：`outline.md`、`common.md`、`nli_account.md`、`friendship.md`，Gate A、Gate C 已通过。
>
> 本文档定义 Game Instance、Instance Session、代理发布授权、邀请码、Guest Session、Join Request 和联机授权边界。

## 目标

- 让 NLI Account 的 Instance Session 创建并独占管理一个 Game Instance；
- 明确实例资源生命周期、在线租约、可见性和可加入状态；
- 明确 MC Profile 只是客户端声明的展示与弱 ACL 匹配信息，不作为 NLI 身份证明；
- 支持好友列表、代理展示和邀请码路径；
- 为匿名用户建立最小权限、短生命周期 Guest Session；
- 定义 Join Request 的创建、审批、过期、取消和二次鉴权；
- 确保客户端不能伪造 Account ID、ACL Identity Trait、Source、代理来源或权限结论。

## 非目标

- WebSocket 具体帧格式、事件 Envelope 与多节点路由由 `notifications.md` 定义；
- 不使用 WebSocket 执行关键状态写入；
- 不验证客户端声明的 Minecraft 账号所有权；
- 不传输实际游戏连接地址或联机信令，除非后续独立设计明确加入；
- 不设计管理后台权限。

## 已确认的不变量

- Instance ID 使用 UUIDv4，只用于寻址，不是管理凭据；
- 创建实例时后端从 Token Family 推导 Owner Account，不接收客户端 Owner ID；
- 一个 Token Family 同时最多绑定并管理一个 Instance；
- 每账号最多 5 个绑定实例；
- 只有创建并绑定实例的 Instance Session 可以管理该实例；
- 实例关闭后 Token Family 可以解绑并恢复为 Account Session；
- MC Profile 的来源、UUID 和用户名均标记为客户端声明；v2 不接收客户端声明头像；
- Instance ACL 统一决定请求者是否可以发现并请求加入；DENY 或无匹配同时意味着不可见、不可加入；
- 邀请码和代理授权码都是高熵不透明 Secret，后端只保存哈希；
- Guest 使用 `nli_guest` Audience，不签发 Refresh Token；
- Join Request 的请求者类型、ACL Identity Traits 和 Source 由后端推导；
- 接受 Join Request 时必须重新检查实例、请求、来源证明和最新 ACL。

## 设计批次

### 批次 1：Game Instance 与 Instance Session（已完成）

- [x] 生命周期 `ACTIVE / CLOSED` 与在线状态 `ONLINE / OFFLINE` 分离；
- [x] WS 租约失效后立即 OFFLINE，10 分钟内可重连，超时自动关闭并解绑；
- [x] 访问控制统一为有序 `Matcher + ALLOW/DENY` ACL；
- [x] ACL 同时控制实例发现与 Join Request 资格，不再区分 hidden、joinable、allowed types 和黑名单；
- [x] Instance Session 可以更新现有 Profile、版本、描述和配置，不创建世界版本；
- [x] MC Profile source/UUID/username 有明确限制且始终为客户端声明；
- [x] v2 不接收客户端声明的 MC Profile 头像；
- [x] 描述和兼容字段有固定限制，不接受任意 Metadata JSON；
- [x] 实例创建、关闭、额度和 Token Family 绑定在事务内处理。

### 批次 2：代理发布授权（已完成）

- [x] Grant Secret 为高熵不透明值，只保存哈希，创建或轮换时仅显示一次；
- [x] Grant 可以不设到期时间，也可由签发者选择有限有效期；
- [x] 一个 Grant 同时最多绑定 Grantee 的一个 ACTIVE Instance，关闭/解绑后可以复用；
- [x] 一个 Instance 同时最多代理展示到一个 Presented-Under Account；
- [x] Instance 创建或更新时均可绑定、切换或解除代理；
- [x] 有效代理路径把 Presented-Under Account 的好友归类为 FRIEND，但不绕过其他检查；
- [x] 签发者与 Grantee 解除好友时 Grant 终止失效，重新加好友不会恢复；
- [x] 轮换 Secret 不影响已绑定实例，撤销 Grant 才终止现有展示；
- [x] 仅签发者可列表、轮换和撤销；Grantee 只在绑定后看到非秘密来源摘要。

### 批次 3：邀请码与 Guest Session（已完成）

- [x] 邀请码 Secret 仅在创建/轮换时显示一次，列表只返回元数据；
- [x] 每个邀请码必须设置 1–100 次成功使用上限，默认 1；
- [x] Join Request 创建时原子预留次数，ACCEPTED 时消费，其他终态释放；
- [x] 每实例最多 3 个有效邀请码，默认 24 小时，最大 7 天；
- [x] Guest Session 绝对有效期 5 分钟，Join 终态后最多保留 60 秒读取结果；
- [x] Guest 绑定一个 Instance、一个 Invite 且最多创建一个 Join Request；
- [x] 匿名公开资料只包含 Guest ID 和 CLIENT_CLAIMED MC Profile；
- [x] Guest ID 仅封禁到 Session 结束，IP 只用于最长 24 小时的服务端临时控制；
- [x] MC UUID/username 黑名单只作为不可靠的客户端期望过滤。

### 批次 4：Join Request（已完成）

- [x] 加入来源为 `FRIEND_LIST / INVITE_CODE`，身份类型独立为 Direct Friend、Proxy Friend、NLI Account 或 Anonymous；
- [x] Join Request 保存申请时 MC Profile 快照，但 Profile 变化不取消请求、不影响授权；
- [x] ACCEPTED 形成绑定双方 Session 的 60 秒 Acceptance Lease，不签发独立 Ticket；
- [x] 同 requester session + target 只保留一个 PENDING；源实例最多 3 个 outgoing，目标最多 100 个 incoming；
- [x] 最新 ACL、来源证明、Invite/Proxy、好友关系或在线状态失效时取消受影响 pending；
- [x] `approval_mode=AUTO` 在同一事务中创建并直接接受 Request；
- [x] 拒绝原因只允许固定公开枚举；
- [x] NLI 请求终态可查询 24 小时，Guest 仅 60 秒，最小安全审计保留 30 天。

## Game Instance

### 状态模型

资源生命周期：

```text
ACTIVE -> CLOSED
```

在线状态独立派生：

```text
OFFLINE <-> ONLINE
```

- `ACTIVE`：实例资源仍由一个有效 Token Family 绑定；
- `CLOSED`：不可逆终态，不能重新上线、查询或加入；
- `ONLINE`：当前认证 WS Session 的租约有效；
- `OFFLINE`：没有有效 WS 租约，不可公开发现或加入，但可能仍在重连宽限期内。

实例创建后为 `ACTIVE + OFFLINE`，直到 Phase 5 定义的认证 WebSocket 建立并形成有效租约。

WS 租约失效时：

1. 立即转为 `OFFLINE`；
2. 从所有好友、代理和公开查询中隐藏；
3. 拒绝新的 Join Request，并取消全部 PENDING；
4. 允许同一 Instance Session 在 10 分钟内重新建立租约并恢复 `ONLINE`；
5. 连续 OFFLINE 满 10 分钟后自动转为 `CLOSED` 并解绑仍有效的 Token Family。

Token Family 被撤销、标记 compromised、账号禁用/删除或实例被显式关闭时，不等待宽限期，立即关闭实例。

### 字段范围

- Instance ID（UUIDv4）；
- Owner Account ID；
- Owner Token Family ID；
- Owner Session ID；
- 生命周期状态；
- 在线状态、当前 WS Session 引用和租约截止时间；
- OFFLINE 开始和自动关闭时间；
- 当前 MC Profile；
- 游戏和 Loader 兼容信息；
- 实例描述；
- 有序 ACL Rules；
- `approval_mode = MANUAL / AUTO`；
- 展示用游戏状态；
- 创建、更新和关闭时间。

Owner、Token Family、Session 和在线租约均由服务端确定，客户端不能提交或覆盖。

### 创建与 Token Family 绑定

创建实例必须使用 `nli_account` Access Token 和当前未绑定的有效 Token Family，并携带 Idempotency Key。

事务流程：

1. 锁定 Token Family 和 Account 的活跃实例额度；
2. 确认 Family 为 `ACTIVE`，未绑定其他实例；
3. 确认 Account 为 `ACTIVE` 且当前绑定实例少于 5；
4. 校验并清洗实例字段；
5. 创建 UUIDv4 Instance ID；
6. 将 Token Family 原子转为 `ACTIVE_BOUND` 并绑定 Instance ID；
7. 创建 `ACTIVE + OFFLINE` 实例；
8. 提交后允许建立实例 WebSocket。

同一 Idempotency Key 重试返回原实例。Family 已绑定时使用不同 Key 创建实例返回冲突，不自动关闭旧实例。数据库约束必须保证一个 Family 最多绑定一个 ACTIVE Instance。

### 管理权限

只有与实例绑定的同一 Token Family 可以更新或关闭实例。Owner Account 的其他 Account Session 或 Instance Session 即使属于同一账号，也不能管理该实例。

Instance ID 只用于寻址。知道 Instance ID 不提供更新、关闭、审批 Join Request 或查看邀请码的权限。

### 更新

绑定的 Instance Session 可以更新：

- MC Profile；
- 游戏、Loader 和 Mod 版本；
- 描述；
- ACL Rules；
- approval mode；
- 展示用游戏状态。

更新不创建新的 Instance 或“世界版本”。所有旧 pending Join Request 在接受时根据最新配置和状态重新鉴权。

更新不能改变 Owner、Token Family、Instance ID、生命周期状态或服务端租约字段。

### 关闭

关闭是幂等操作：

1. 锁定 Instance 与 Token Family；
2. 将 ACTIVE Instance 转为 CLOSED；
3. 清除在线租约并使当前 WS Session 失去续租资格；
4. 取消所有 pending Join Request；
5. 撤销实例邀请码并移除代理展示；
6. 如果 Token Family 仍有效且仍绑定该 Instance，则原子解绑并恢复 `ACTIVE` Account Session；
7. 通过 Outbox 发出关闭通知。

自动关闭执行相同流程。已 CLOSED 实例不能用重连、Refresh 或重复创建请求恢复。

## MC Profile 与实例字段

### MC Profile

```text
verification = CLIENT_CLAIMED
source: 1–32 ASCII
uuid: optional canonical UUID
username: 1–64 Unicode code points
```

规则：

- Unicode username 规范化为 NFC；
- 禁止 C0/C1 控制字符、NUL、双向控制字符和首尾空白；
- source 只允许可打印 ASCII 标识符，不因此推导或验证 Provider Binding；
- UUID 即使格式合法也只表示客户端声明；
- v2 不接收 MC Profile 头像 URL、二进制头像或任意附加 JSON；
- 所有公开响应必须保留 `verification = CLIENT_CLAIMED`；
- NLI 后端只执行长度、编码和危险控制字符等结构清洗，不验证 Profile 是否真实属于请求者；
- MC Profile 不能用于 NLI 鉴权、Owner 推导、好友判断或可靠持久账号封禁；
- MC username 仅在 Owner 明确配置时参与标记为 `WEAK_CLIENT_CLAIMED` 的 ACL Matcher。

### 兼容字段和描述

- `description`：0–128 Unicode code points，NFC，移除控制字符和首尾空白；
- `game_version`：1–64 可打印 ASCII；
- `loader_id`：可选，1–64 可打印 ASCII；
- `loader_version`：Loader 存在时必填，1–64 可打印 ASCII；
- `mod_version`：1–64 可打印 ASCII；
- 不接受任意 Metadata JSON；
- 展示字段需要进行上下文相关转义，不能包含 HTML 或终端控制序列；
- 描述、游戏状态和版本字段只用于展示与兼容提示，不参与身份鉴权。

### Profile 信任、举报与本地校验

NLI 无法从根本机制上保证任一联机客户端的 MC Profile：恶意客户端可以控制本机 Mod、调用本机 NLI Auth 能力或在联机建立后改变游戏行为。即使某个 Profile 被证明归属真实，也不能证明该客户端不会实施恶意操作。

因此：

- NLI 后端不尝试认证联机双方的 MC Profile；
- 服务端只向对端转发带 `CLIENT_CLAIMED` 标记的声明快照；
- Join 审批、ACL 和 Acceptance Lease 不能把 MC Profile 当作可靠身份；
- Profile 在 Request 生命周期中变化不会取消 Request，也不会影响可靠授权结论；
- 客户端 MOD 可以在联机建立后使用游戏协议、服务器能力或其他本地机制进行二次校验；
- 本地校验结果属于客户端判断，不能反向升级为 NLI Account 身份证明。

后续举报通道应允许提交：

- Reporter 的认证身份；
- Instance ID 和 Join Request ID；
- 被举报方已知的 NLI Account ID 或短期 Guest ID；
- 当时展示的 CLIENT_CLAIMED Profile 快照；
- 分类原因、时间和由用户主动提供的证据。

举报记录必须明确区分 NLI 已验证标识与客户端声明字段，不能仅凭 MC username/UUID 自动处罚一个 NLI Account。举报 API、证据保留和人工处置在独立 Moderation 设计中冻结。

展示用 `game_state` 使用受限枚举：

```text
IN_WORLD
BUSY
UNKNOWN
```

它由客户端声明，仅用于 UI，不能推导 ACL 结果或审批方式。

## Instance Access Control List

Instance 不再分别保存 `hidden`、`joinable`、`allowed_joiner_types` 或黑名单。发现和请求加入统一由有序 ACL 决定；不允许加入的请求者也不能发现实例。

`approval_mode = MANUAL / AUTO` 与 ACL 分离：ACL 决定是否有资格发现并创建 Join Request，approval mode 决定合格请求需要 Owner 审批还是原子自动接受。

### Access Rule

每条 Access Rule 包含：

```text
rule_id: UUIDv4
priority: integer
matcher:
  identity_types: optional list
  sources: optional list
  subjects: optional typed list
action: ALLOW | DENY
```

除 `action` 外，其他 Matcher 项不填或使用空列表都表示“任意”：

- identity types 为空：任意身份类型；
- sources 为空：任意有效来源；
- subjects 为空：任意用户或 MC 名称；
- 三项全部为空：匹配所有请求者和来源。

非空字段之间使用 AND；同一字段内多个值使用 OR。`action` 必填，不能默认为 ALLOW。

### Identity Type

初始身份 Trait：

```text
NLI_ACCOUNT
DIRECT_FRIEND
PROXY_FRIEND
ANONYMOUS
```

- 有 NLI Account Session 的请求者一定具有 `NLI_ACCOUNT`；
- 与真实 Instance Owner 是 NLI 好友时同时具有 `DIRECT_FRIEND`；
- 与当前 Presented-Under Account 是 NLI 好友且 Proxy Grant 有效时同时具有 `PROXY_FRIEND`；
- 没有 NLI Account、通过 Guest Session 请求时具有 `ANONYMOUS`；
- 一个 NLI 请求者可以同时具有多个 Trait；Matcher 命中其中任一个指定值即可；
- 不存在 Provider Friend、第三方联机渠道或客户端自报身份类型；所有可见 Instance 都由 NLI 后端上的 Host 发布。

### Source

初始来源：

```text
FRIEND_LIST
INVITE_CODE
```

- `FRIEND_LIST`：只有存在有效 Direct Friend 或 Proxy Friend 关系时由后端建立；
- `INVITE_CODE`：只有 Invite Secret、Generation、有效期和次数检查通过后由后端建立；
- 不提供 `PUBLIC_ACCOUNT` 来源或公开实例目录；
- 来源由后端根据实际入口推导，客户端不能声明；
- 未来增加来源时必须新增服务端已知枚举和验证器，不能接受任意字符串。

### Subject Matcher

subjects 使用带命名空间的值：

```text
NLI_ACCOUNT_ID:<uuid>
MC_USERNAME:<normalized_name>
```

- `NLI_ACCOUNT_ID` 与认证 Account ID 精确匹配，是 NLI 范围内可靠 Matcher；
- `MC_USERNAME` 对 Request/Guest 中 CLIENT_CLAIMED username 做 NFC 与 Unicode Case Folding 后匹配；
- 同一个 subjects 列表中的多个 typed value 使用 OR；
- NLI ID 和 MC username 不自动互相转换，也不按显示名称猜测；
- Anonymous 没有 NLI Account ID，但可以匹配其 CLIENT_CLAIMED MC username。

MC username 可用于 ALLOW 或 DENY，但必须标记 `WEAK_CLIENT_CLAIMED`：

- 请求者可以任意伪造或更换该值；
- ALLOW 不能被描述为白名单、安全身份验证或封禁绕过保护；
- DENY 也只能作为便利过滤，不能视为可靠长期 ban；
- MOD 的高级 ACL UI 必须显著提示该风险；
- NLI 后端只按声明值执行确定性字符串匹配，不尝试证明 Minecraft 账号所有权。

### 求值顺序

ACL 按 `priority` 从小到大求值，第一条匹配规则的 Action 生效：

```text
for rule in rules.order_by(priority):
    if matcher.matches(context):
        return rule.action
return DENY
```

- 同一 Instance 内 priority 必须唯一；更新 API 拒绝重复 priority；
- 无匹配默认 DENY；
- ACL 为空等价于 DENY ALL；
- Owner 的绑定 Instance Session 始终可以管理自己的 Instance，不经过面向加入者的 ACL；
- Owner 不能通过管理绕过“不能向自身创建 Join Request”。

### 前置硬门槛

ACL 只在以下硬门槛通过后求值：

- Instance 为 `ACTIVE + ONLINE`；
- 请求者的 NLI Source Instance Session 或 Guest Session 有效；匿名首次交换 Invite 时可以使用经过 Secret 验证和 Profile 结构清洗的临时 Context，只有 ACL ALLOW 后才创建 Guest；
- Source 已由对应 NLI 好友/Proxy Grant 或 Invite 验证器建立；
- Proxy Grant、Invite Reservation 和账号状态满足各自不变量；
- 请求未触发服务端滥用限制。

ACL 的 ALLOW 不能复活 CLOSED/OFFLINE Instance、伪造 Source、绕过 Session 鉴权或扩大 Guest Token 范围。

### ALLOW 与 DENY

- `ALLOW`：请求者可以经当前 Source 获取 Instance 摘要并创建 Join Request；
- `DENY` 或无匹配：实例对该请求者不可见，也不能创建 Join Request；
- 同一次读取和创建请求必须使用同一套 ACL Context 计算；
- Instance ID 即使已知，也不能绕过 DENY；
- Invite 验证成功但 ACL DENY 时使用统一不可用响应，不能泄漏具体匹配规则；
- ACL Rule、命中的 priority 和内部拒绝原因只对 Owner 的绑定 Instance Session 可见。

### 默认 ACL

新 Instance 默认使用以下等价规则，approval mode 默认为 MANUAL：

```text
100: identity in [DIRECT_FRIEND, PROXY_FRIEND]
     AND source = FRIEND_LIST
     -> ALLOW

200: source = INVITE_CODE
     -> ALLOW

1000: <all matcher fields empty>
      -> DENY
```

MOD 可以提供人性化预设，但提交到后端的始终是完整有序 ACL。

### ACL 更新与请求失效

- ACL 作为完整列表原子替换，拒绝部分更新造成的短暂开放窗口；
- 每次替换增加 `acl_revision`；
- 后端重评所有 PENDING，最新 ACL 为 DENY 的请求转为 CANCELLED；
- Acceptance Lease 使用时再次按该 Request 的身份、来源和 MC Profile 快照评估最新 ACL；
- 从 DENY 改为 ALLOW 不会自动恢复旧 Request，必须重新创建；
- 更新 ACL 和取消失效 PENDING 使用同一事务及 Outbox；
- Rule 数量默认最多 100，每个 Matcher 列表最多 100 个值。

### 数量与并发

- 每个 Account 最多 5 个 `ACTIVE` 且绑定 Token Family 的 Instance；
- OFFLINE 重连宽限期内仍计入额度；
- CLOSED 不计入额度；
- 创建、显式关闭、自动关闭和 Session 撤销都必须锁定 Account 额度与 Token Family；
- 并发创建超过额度时只允许满足约束的事务提交；
- 不自动踢出或关闭旧实例。

## 代理发布授权

### 语义和角色

A 为好友 B 创建代理发布 Grant：

- A 是 `issuer_account_id`，也是实例代理展示的 `presented_under_account_id`；
- B 是 `grantee_account_id`，只能用自己的 Instance Session 绑定该 Grant；
- B 的 Game Instance 仍归 B 所有并由 B 控制；
- A 不能更新、关闭或审批 B 的实例；
- A 的好友可以从 A 的好友条目发现该代理实例，并通过有效代理路径参与 FRIEND 身份判断。

代理发布既不转移 Owner，也不让签发者获得实例管理权限。

### Grant 字段与状态

字段范围：

- Grant ID（UUIDv4）；
- Issuer Account ID；
- Grantee Account ID；
- Secret Hash；
- Secret Generation；
- 状态；
- 可选到期时间；
- 当前绑定 Instance ID；
- 撤销时间和原因；
- 创建、更新时间与最后成功绑定时间。

状态：

```text
ACTIVE -> REVOKED
```

以下条件让 Grant 立即不可用：

- 签发者主动撤销；
- Issuer 与 Grantee 不再为 NLI 好友；
- 任一账号不再为 ACTIVE；
- 设置了有限到期时间且已经过期；
- Grant 被安全管理员封存；
- 数据完整性校验失败。

好友解除触发终止性撤销，双方后来重新成为好友也不能恢复旧 Grant。

### Secret 与有效期

- Secret 使用至少 256 bit 随机熵；
- 创建和轮换响应中只显示一次；
- 服务端只保存带 Pepper 的密码学哈希和 Generation；
- 普通列表、日志、审计、实例读模型和 Join Request 均不返回 Secret；
- Secret 可以轮换，旧 Generation 立即不能用于新的绑定；
- 轮换不解除已经绑定的实例，Grant ID 和现有代理展示保持不变；
- 需要终止现有展示时必须撤销 Grant 或解绑实例。

`expires_at` 可选：

- 未设置时 Grant 可以持续有效，直到撤销、好友解除或账号失效；
- 设置时必须晚于当前时间，并受服务端允许的日期范围校验；
- 有限到期时间到达后立即停止现有代理展示；
- 不存在强制默认到期时间或“永久 Grant 自动续期”任务。

由于允许无到期 Grant，它会持续计入签发者最多 10 个 ACTIVE Grant 的额度，直到显式或自动撤销。

### 创建、列表和分发

创建 Grant：

1. Issuer 使用 `nli_account` Session 并完成近期重新认证；
2. 请求指定 Grantee Account ID，不接受 username 作为最终写入目标；
3. 后端锁定双方 Pair，确认双方均 ACTIVE 且为 `FRIENDS`；
4. 确认 Issuer 少于 10 个 ACTIVE Grant；
5. 创建 Grant ID、Secret 和 Hash；
6. 原子写入审计与通知 Outbox；
7. 仅在本次成功响应返回原始 Secret。

Issuer 可以列出 Grant ID、Grantee 公开身份、状态、可选到期时间、当前是否绑定实例和最后使用时间。列表不能再次读取 Secret。

Grantee 不能通过 API 枚举别人签发给自己的未使用 Grant；Issuer 自行通过安全渠道分发 Secret。成功绑定后，Grantee 只能在自己的 Instance 详情中看到 Issuer 的公开身份、Grant ID 和代理状态，不看到 Secret Hash 或其他 Grant。

### 绑定、复用与切换

Instance 创建或更新时可以提交一个 Proxy Grant Secret。

后端：

1. 从当前 Instance Session 推导 Owner/Grantee；
2. 哈希 Secret 并定位 Grant，使用常量时间比较；
3. 锁定 Grant、Issuer-Grantee Pair 和 Instance；
4. 重新检查 Grant ACTIVE、可选到期时间、双方 ACTIVE 且仍为 FRIENDS；
5. 确认 `grantee_account_id` 等于实例 Owner；
6. 确认 Grant 未绑定其他 ACTIVE Instance；
7. 确认 Instance 未绑定其他 Proxy Grant；
8. 原子建立 Grant 与 Instance 的关联；
9. 写入审计和读模型 Outbox。

相同 Grant 重试绑定同一 Instance 幂等成功。Grant 已绑定另一个 ACTIVE Instance，或 Instance 已绑定另一个 Grant 时返回冲突。

实例关闭或 Owner 主动解除代理后，Grant 清空当前 Instance 引用并可用于 Grantee 的后续 Instance。切换代理目标必须提交新 Grant Secret，并在单个事务中解除旧关联、验证新 Grant、建立新关联；失败时保留旧关联。

### 撤销和并发

只有 Issuer 可以轮换 Secret 或撤销 Grant，并要求近期重新认证。Grantee 可以解除自己 Instance 的代理关联，但不能撤销 Issuer 的整个 Grant。

撤销流程：

- 锁定 Grant 和当前 Instance 关联；
- 转为 REVOKED 并清除当前绑定；
- 后续所有列表和 Join 鉴权立即忽略该代理路径；
- 异步清理读模型延迟不能延长权限；
- 通知当前 Instance Owner；
- 上游缓存或旧 Secret 不能恢复 Grant。

绑定与撤销并发时，以 Grant 行锁顺序为准；如果绑定先提交，随后撤销仍会立即终止展示；如果撤销先提交，绑定失败。

### 代理展示和加入授权

C 从好友 A 的条目看到 B 的代理 Instance 时，后端保存并返回：

```text
publication_source = PROXY
owner_account_id = B
presented_under_account_id = A
```

C 通过该展示路径请求加入时归类为 `FRIEND`，但必须同时满足：

- Grant 当前有效且绑定该 Instance；
- A 与 B 仍为 NLI 好友；
- C 与 A 仍为 NLI 好友；
- A–B 与 C–A 的 NLI 好友关系仍有效；
- C 的 ACL Context 未命中更高优先级 DENY；
- Instance 为 ACTIVE、ONLINE；
- 以 `identity_types=[NLI_ACCOUNT, PROXY_FRIEND]`、`source=FRIEND_LIST` 和 C 的声明 Profile 组成 ACL Context，首条匹配规则为 ALLOW；
- 请求未超过限流和 pending 上限。

代理路径只建立 `PROXY_FRIEND + FRIEND_LIST` ACL Context，不绕过实例状态、ACL、限流或审批。Join Request 必须向 B 的 Instance Session 发送，A 没有审批权。

客户端不能提交或覆盖 `owner_account_id`、`presented_under_account_id` 或 `publication_source`；后端从当前有效 Grant 推导。

## 实例邀请码

### 字段和状态

字段范围：

- Invite ID（UUIDv4）；
- Instance ID；
- Secret Hash；
- Secret Generation；
- 状态；
- 到期时间；
- `max_uses`；
- `consumed_uses`；
- `reserved_uses`；
- 创建、轮换、撤销和最后成功使用时间。

状态：

```text
ACTIVE -> REVOKED
ACTIVE -> EXPIRED
ACTIVE -> EXHAUSTED
```

- 每个 ACTIVE Instance 最多有 3 个尚未终止的邀请码；
- 默认有效期 24 小时，最大有效期 7 天；
- `max_uses` 必须为 1–100，默认 1；
- `consumed_uses + reserved_uses` 不能超过 `max_uses`；
- 实例关闭时邀请码全部终止；OFFLINE 重连宽限期内不自动撤销，但不能创建 Join Request。

### Secret 管理

- Secret 使用至少 256 bit 随机熵；
- 创建和轮换响应只显示一次；
- 服务端只保存带 Pepper 的密码学 Hash 和 Generation；
- Owner 的绑定 Instance Session 可以创建、列出元数据、轮换和撤销；
- Owner 其他 Session 和 Proxy Grant Issuer 不能管理邀请码；
- 列表不返回 Secret 或 Hash；
- 丢失 Secret 时只能轮换，不能找回；
- 轮换使旧 Generation 不能建立新的 Guest Session 或 Join Request。

轮换不取消已经创建并持有次数 Reservation 的 pending Join Request；撤销则取消所有关联 pending Request 并释放 Reservation。

### 使用次数预留

邀请码路径创建 Join Request 时，在一个事务内：

1. 锁定 Invite；
2. 重新检查状态、到期、Generation 和 Instance；
3. 确认 `consumed_uses + reserved_uses < max_uses`；
4. 创建与 Join Request 一一对应的 Reservation；
5. `reserved_uses += 1`。

终态处理：

- `ACCEPTED`：`reserved_uses -= 1`，`consumed_uses += 1`；
- `REJECTED / CANCELLED / EXPIRED`：`reserved_uses -= 1`，不增加 consumed；
- 自动接受在同一事务中预留并立即消费；
- 达到上限后状态转为 `EXHAUSTED`；
- Reservation 最长不能超过 Join Request 的 60 秒有效期；
- 清理任务必须修复已经过期但未释放的 Reservation。

由数据库约束和 Invite 行锁防止并发超卖。拒绝或超时不会永久消耗邀请码次数，避免恶意请求耗尽。

### NLI Account 使用邀请码

已登录 NLI Account 必须使用自己的绑定 Instance Session 发起加入。邀请码：

- 建立服务端验证的 `INVITE_CODE` Source，并按请求者实际 NLI 关系附加 Identity Trait；
- 不创建 Guest Session；
- 不伪造 Direct/Proxy Friend Trait；
- 不绕过 ACL、ONLINE、Session、Proxy/Invite 不变量或限流；
- 原始 Secret 不转发给目标 Instance。

## Guest Session

### 创建

匿名客户端使用有效 Invite Secret 建立 Guest Session。服务端：

1. 对 Invite Secret 做限流和常量时间 Hash 验证；
2. 验证 Invite ACTIVE、未过期、未耗尽；
3. 验证目标 Instance ACTIVE、ONLINE，并以 `identity_types=[ANONYMOUS]`、`source=INVITE_CODE` 和声明 MC username 求值 ACL 为 ALLOW；
4. 只对 CLIENT_CLAIMED MC Profile 做结构清洗，不验证真实性；
5. 创建 UUIDv4 Guest ID；
6. 签发至少 256 bit 随机熵的不透明 `nli_guest` Access Token；
7. 只保存 Token Hash、Guest 元数据和 Invite Generation；
8. 不在此步骤预留 Invite 次数，Reservation 在 Join Request 创建时产生。

无效、过期、撤销、耗尽、实例不可用或不允许匿名使用统一公开失败语义，不能帮助枚举 Invite 状态。

### 范围与字段

Guest Session 绑定：

- 一个 Guest ID；
- 一个 Instance ID；
- 一个 Invite ID 和 Secret Generation；
- 一个 CLIENT_CLAIMED MC Profile；
- 最多一个 Join Request ID；
- 创建、绝对过期和终态后过期时间；
- 服务端滥用控制所需的短期 IP 摘要。

Guest Token 只能：

- 创建一次绑定目标的 Join Request；
- 查询该 Request 的结果；
- 在仍为 PENDING 时取消该 Request；
- 通过 HTTP 查询该 Request 的结果；Guest 不拥有通知 WebSocket。

不能访问账号、好友、Provider、其他实例、邀请码管理、代理 Grant 或普通实例管理 API。

### 生命周期

- Guest Session 绝对有效期为 5 分钟；
- 最多关联一个 Join Request；
- Join Request 进入终态后，Guest Session 最晚在 60 秒后失效；
- 实际过期时间取绝对过期与终态后 60 秒中的较早者；
- Invite 撤销或 Instance 关闭可以立即终止 Guest；Guest 被封禁后降为仅可在保留窗口读取已有关联 Request 结果；
- 失效后 Token、Guest ID 和 Request 不能用于创建新 Session 或新请求；
- Guest 不签发 Refresh Token，也不能升级或合并为 NLI Account Session。

### 匿名公开信息

目标 Instance 只能收到：

- `requester_type = ANONYMOUS`；
- Guest ID；
- CLIENT_CLAIMED MC Profile 的 source、UUID 和 username；
- 明确的 `verification = CLIENT_CLAIMED`；
- Source `INVITE_CODE`。

不返回 IP、Invite Secret/Hash、Guest Token、稳定设备指纹、头像或推断的 NLI Account。

### 匿名封禁和限流

匿名身份没有可靠永久标识：

- Guest ID ban 只在对应 Guest Session 生命周期内有效；
- Owner 可拒绝并封禁当前 Guest，阻止创建/取消请求或使用 Acceptance Lease，但仍允许在终态后 60 秒内读取固定结果；
- IP 仅由服务端用于速率限制和最长 24 小时临时封禁，Owner 不能查看或直接管理 IP；
- IP 摘要使用轮换密钥计算，原始 IP 不进入普通日志或业务读模型；
- MC username 只能通过标记为 `WEAK_CLIENT_CLAIMED` 的 ACL Rule 匹配，不宣称可靠封禁；MC UUID 当前不作为 ACL Subject。

初始限制：

- 同一 IP 邀请码失败验证每 15 分钟最多 10 次；
- 同一 IP 成功创建 Guest Session 每 15 分钟最多 20 次；
- 同一 Invite 创建 Guest Session 每 15 分钟最多 100 次；
- 每个 Instance 同时最多 100 个未过期 Guest Session；
- Guest 仍受 Join Request 的 Instance/IP/pending 限制；
- 超限响应不得区分 Invite 不存在、实例不允许匿名或次数耗尽。

## Join Request

### 状态机

```text
PENDING -> ACCEPTED
PENDING -> REJECTED
PENDING -> CANCELLED
PENDING -> EXPIRED
```

Request 创建后 60 秒过期。所有终态不可逆，不允许从终态恢复为 PENDING，也不复用 Request ID。

Auto Approval 仍创建完整 Request 记录，但在同一事务中从初始状态直接提交为 `ACCEPTED`，不暴露可并发审批的 PENDING 窗口。

### 字段范围

- Join Request ID（UUIDv4）；
- Target Instance ID；
- Requester Type；
- Requester Account ID 与 Source Instance/Session ID，或 Guest ID/Session ID；
- Source；
- 服务端推导的 Identity Traits；
- 可选 Presented-Under Account ID；
- 可选 Invite ID、Generation 和 Reservation ID；
- 仅用于展示、ACL 弱匹配和举报的 CLIENT_CLAIMED MC Profile 快照；
- 状态；
- 创建与过期时间；
- 决策方式 `MANUAL / AUTO`；
- 决策者 Session ID；
- 固定公开拒绝原因；
- Acceptance Lease 截止时间；
- 终态时间。

Account ID、Guest ID、Session ID、Source、Identity Traits、Presented-Under Account、Invite 引用和 Requester Type 均由后端根据鉴权上下文与实际来源填写，客户端不能覆盖。

### 请求者类型和来源实例

```text
NLI_ACCOUNT
ANONYMOUS
```

NLI Account 请求者：

- 必须使用绑定自己 Source Instance 的 Instance Session；
- Source Instance 必须 `ACTIVE + ONLINE`；
- Source 与 Target 不能是同一 Instance；
- Account 必须 ACTIVE；
- MC Profile 从 Source Instance 当前记录复制，不接受 Join 请求临时提交另一份。

Anonymous 请求者：

- 必须使用绑定 Target 和 Invite 的有效 Guest Session；
- MC Profile 来自 Guest Session 的不可变快照；
- Guest 最多创建一个 Join Request。

### Source、Identity Traits 与 ACL Context

Join Source：

```text
FRIEND_LIST
INVITE_CODE
```

服务端推导：

- `FRIEND_LIST`：NLI 请求者通过有效 Direct Friend 或 Proxy Friend 列表发现 Instance；
- `INVITE_CODE`：NLI 请求者先验证 Invite Secret 并使用绑定当前 Instance Session 的短期 Invite Resolution，或 Anonymous 使用由 Invite 建立的 Guest Session；
- 不存在第三方联机 Source，也不允许客户端提交任意 Source；
- 没有 NLI Account 的请求者统一作为 `ANONYMOUS`，必须使用 Guest Session。

Identity Traits 根据当前 NLI 状态计算：

- NLI 请求者具有 `NLI_ACCOUNT`；
- Requester 与真实 Owner 为好友时附加 `DIRECT_FRIEND`；
- Requester 与有效 Proxy Presenter 为好友时附加 `PROXY_FRIEND`；
- Guest 只具有 `ANONYMOUS`。

客户端不能指定 Source 或 Trait。Invite Source 不抹去请求者已有的 NLI Trait，也不会为非好友伪造 Friend Trait。

ACL Context 包含：

- Identity Trait 集合；
- 一个经过验证的 Source；
- 可选可靠 NLI Account ID；
- 当前请求方 Instance/Guest 提供的可选 CLIENT_CLAIMED MC username。

NLI 请求者只有使用绑定且 ONLINE 的 Source Instance Session 时才具有联机发现 Context；普通未绑定 Account Session 可以管理账号和好友，但不返回可加入 Instance 摘要。Anonymous 必须先通过 Invite 建立 Guest Context。

### 创建前授权

所有请求必须：

1. 验证 Target Instance 为 `ACTIVE + ONLINE`；
2. 验证 Requester Account/Source Instance 或 Guest Session；
3. 验证 FRIEND_LIST 的 Direct/Proxy 关系，或 INVITE_CODE 的 Secret/Generation/次数；
4. 从服务端状态建立 Identity Traits 和 Source；
5. 使用当前 ACL Revision 求值；
6. 只有首条匹配 Action 为 ALLOW 时才返回 Instance 摘要或创建 Request；
7. 对 Invite 路径在创建事务中生成次数 Reservation；
8. 应用请求者、Invite、Instance 和 IP 等服务端滥用限制。

DENY、无匹配、无效 Source、无权查看、Instance 不在线和 Invite 不可用均使用统一不可用语义。响应不能暴露命中的 Rule、priority、Invite 剩余次数或内部 NLI 关系。

### 去重和数量限制

活动唯一键：

```text
requester_session_id + target_instance_id + status=PENDING
```

- 相同 Requester Session 向同一 Target 重复创建时返回现有 PENDING；
- NLI Source Instance 同时最多 3 个 outgoing PENDING；
- Guest 由生命周期限制为最多 1 个；
- Target Instance 同时最多 100 个 incoming PENDING；
- Auto Approval 不占用 pending 配额，但受请求速率和 Invite 次数约束；
- 超限使用统一限流/不可用响应，不能暴露 Target 当前 pending 数；
- 写操作必须携带 Idempotency Key，并用数据库唯一约束处理并发重复。

### MC Profile 快照

Join Request 保存创建时的 CLIENT_CLAIMED MC Profile 快照：

- 快照只用于向 Target 展示、执行该 Request 的弱 MC_USERNAME Matcher 和后续举报；
- 快照不附带真实性保证；
- Source Instance 后续更换 Profile 不取消 Request，也不改变已经保存的快照；
- Manual Accept、Auto Approval 和 Acceptance Lease 都不能要求当前 Profile 与快照一致；
- Guest Profile 在 Session 内保持原声明，但仍不是可靠身份；
- 联机建立后双方 MOD 可以独立执行本地二次校验。

如果客户端在联机阶段展示另一 Profile，NLI 可以在举报上下文中保留前后声明差异，但不能据此自动把某个 MC Profile 绑定到 NLI Account。

### 手动接受与拒绝

只有 Target Instance 绑定的同一 Instance Session 可以接受、拒绝或批量拒绝。Owner 的其他 Session、Proxy Presenter 和请求者均无审批权限。

接受事务必须锁定 Request、Target、Requester Session，以及需要时的 Invite/Proxy Grant，并重新验证：

- Request 仍为 PENDING 且未过期；
- 操作者 Session 仍绑定 Target；
- Target 仍 `ACTIVE + ONLINE`；
- Source Instance 或 Guest Session 仍有效；
- Request 的 Source 证明仍有效；
- Direct/Proxy Friend Trait 所需的好友关系和 Proxy Grant 仍有效；
- 使用 Request 保存的 MC Profile 快照和最新可靠身份状态重新构造 ACL Context；
- 最新 ACL 的首条匹配 Action 仍为 ALLOW；
- Invite Reservation 仍属于该 Request。

全部通过后：

- Request 转为 ACCEPTED；
- Invite Reservation 原子消费；
- 创建 60 秒 Acceptance Lease；
- 写入审计与通知 Outbox。

拒绝原因只允许固定公开枚举：

```text
DECLINED
BUSY
NOT_ACCEPTING
```

具体 ACL Rule、内部策略和限流不会作为公开拒绝原因返回。Owner 可以通过插入高优先级 DENY Rule 拒绝特定 NLI Account 或弱匹配 MC username；Guest Session 的临时封禁仍是独立的服务端滥用动作。

### Auto Approval

`approval_mode=AUTO` 时仍执行与手动接受相同的完整检查，并在单个事务中：

1. 创建 Join Request；
2. 创建 Invite Reservation（如适用）；
3. 立即转为 ACCEPTED；
4. 消费 Reservation；
5. 创建 60 秒 Acceptance Lease；
6. 写入审计与双方通知 Outbox。

任一检查失败则整体回滚，不留下 PENDING 或次数 Reservation。

### Acceptance Lease

ACCEPTED 不签发独立 Join Ticket。授权由以下组合构成：

```text
Join Request ID
+ Requester Instance Session 或 Guest Session
+ Target Instance Session
+ 60 秒 Acceptance Lease
```

Request ID 本身不是凭据。后续联机信令如果实现，必须重新验证：

- 请求双方提交的 Session 与 Request 记录匹配；
- Acceptance Lease 未过期；
- Target 仍 ACTIVE、ONLINE；
- Session 未撤销；
- Request 仍为 ACCEPTED；
- Source 和 Direct/Proxy Friend Trait 所需关系仍有效；
- 使用 Request 保存的 CLIENT_CLAIMED Profile 快照重新求值最新 ACL，结果仍为 ALLOW。

Invite Reservation 在 ACCEPTED 时已经消费，因此 Invite 因本次消费转为 EXHAUSTED 不会使当前 Lease 自我失效。Owner 如需终止已经接受但尚未使用的 Invite Lease，应关闭 Instance，或原子替换 ACL 使该 Request Context 命中 DENY。

Acceptance Lease 过期后 Request 历史状态仍为 ACCEPTED，但不能再次用于联机。是否允许 Lease 一次消费及具体信令留给后续独立设计，不能因此把 Request ID 当 Bearer Secret。

### 取消、过期与配置失效

Requester 可以用原 Requester Session 取消自己的 PENDING。其他 Session 不能代为取消。

以下事件立即取消受影响 PENDING：

- Target 转为 OFFLINE 或 CLOSED；
- ACL 替换后该 Request Context 的首条匹配结果变为 DENY 或无匹配；
- Invite 撤销、过期或可用次数因其他已接受请求而耗尽且本 Request 没有有效 Reservation；
- Proxy Grant 撤销或 Proxy Friend 来源失效；
- Direct Friend 来源因好友解除失效；
- Requester Source Instance 离线、关闭或 Session 撤销；
- Guest Session 失效或被封禁；
- 任一相关 Account 不再 ACTIVE。

MC Profile 改变不取消 Request；ACL 继续使用 Request 保存的声明快照。后台到期任务和任何 Request 读写路径都应惰性把超过 60 秒的 PENDING 转为 EXPIRED 并释放 Invite Reservation。

### 批量拒绝

Target Instance Session 可以幂等拒绝当前全部 PENDING，并使用固定原因 `NOT_ACCEPTING`。

将 ACL 原子替换为 catch-all DENY 可以作为“停止接收并隐藏实例”操作，并在同一事务中：

- 增加 ACL Revision；
- 批量终止所有不再 ALLOW 的 PENDING；
- 释放对应 Invite Reservation；
- 写入通知 Outbox。

批量拒绝不影响已经 ACCEPTED 的历史记录，但 Acceptance Lease 使用时会按最新 ACL 复核，因此 catch-all DENY 会立即使未使用 Lease 失效。

### 并发终态

Accept、Reject、Cancel、Expire 和配置失效都锁定同一 Request：

- 只有第一个合法转换提交；
- 相同命令和 Idempotency Key 重试返回原结果；
- 不同终态命令看到已结束状态时返回稳定冲突或当前资源；
- Invite Reservation 只能消费或释放一次；
- 通知通过事务 Outbox 去重；
- Target 离线/关闭与 Accept 并发时，Accept 必须同时锁定并复核 Target，不能在离线后产生有效 Lease。

### 查询、通知与保留

- Target Instance Session 可查询该 Instance 的 pending 和最近终态；
- NLI Requester Instance Session 可查询自己的 Request；
- Guest 只能查询绑定的唯一 Request，并在终态后最多保留 60 秒；
- 通知可能丢失，所有结果必须可通过上述 HTTP 查询恢复；
- NLI Account Request 终态业务记录可查询 24 小时；
- 最小安全审计保留 30 天，只含 ID、Source、Identity Traits、分类结果和时间，不含 Secret、Token、IP 原文或 Provider Subject；
- 业务记录到期后 Request ID 不可再用于读取公开资料或授权。

## 权限矩阵

| 操作 | Owner 的绑定 Instance Session | Requester Instance Session | Guest Session | Owner 其他 Session | Proxy Issuer | 其他用户 |
| --- | --- | --- | --- | --- | --- | --- |
| 更新/关闭 Instance | 允许 | 仅自己的 Source Instance | 禁止 | 禁止 | 禁止 | 禁止 |
| 管理 Invite | 允许 | 禁止 | 禁止 | 禁止 | 禁止 | 禁止 |
| 绑定/解除 Proxy Grant | 允许（作为 Instance Owner） | 仅自己的 Instance | 禁止 | 禁止 | 仅管理自己签发的 Grant | 禁止 |
| 创建 Join Request | 不向自身创建 | 允许 | 允许一次 | 禁止 | 不能代替请求者 | 禁止 |
| 查询 Join Request | Target 侧允许 | 自己的请求允许 | 唯一请求允许 | 禁止 | 禁止 | 禁止 |
| Accept/Reject/批量拒绝 | 允许 | 禁止 | 禁止 | 禁止 | 禁止 | 禁止 |
| 取消请求 | 禁止 | 自己的 PENDING | 自己的 PENDING | 禁止 | 禁止 | 禁止 |
| 使用 Acceptance Lease | Target 侧参与 | Requester 侧参与 | Guest 侧参与 | 禁止 | 禁止 | 禁止 |

## 安全与并发场景走查

### 同一 Token Family 并发创建 Instance

- 两个请求使用不同 Idempotency Key 并发创建；
- 事务锁定同一 Token Family；
- 第一个创建并绑定成功；
- 第二个看到 `ACTIVE_BOUND` 后返回冲突；
- 不会创建两个 ACTIVE Instance 或突破账号 5 个额度。

结果：通过。

### WS 断线、重连和自动关闭

- 租约失效时 Instance 立即 OFFLINE、隐藏并取消 PENDING；
- 旧 WS Session 不能续租；
- 同一 Instance Session 在 10 分钟内可以建立新租约；
- 超时关闭和重连竞争时锁定 Instance，只有先提交的合法转换生效；
- CLOSED 后任何迟到重连均不能恢复 Instance。

结果：通过。

### Instance 关闭与 Token Family Refresh 并发

- Refresh 不改变 Family 绑定关系；
- Close 锁定 Instance 与 Family 并原子解绑；
- Close 前签发的新 Access Token 仍只能按服务端最新 Family 状态授权；
- Instance ID 和旧 WS Session 均不能作为管理凭据；
- 已关闭实例不会因 Refresh 恢复。

结果：通过。

### ACL DENY 与直接 ID 探测

- 请求者的 ACL Context 命中 DENY 或无匹配时，实例同时不可见、不可申请加入；
- 仅知道 UUIDv4 Instance ID 不能绕过 ACL；
- FRIEND_LIST 必须先有服务端验证的 Direct/Proxy Friend Trait；
- INVITE_CODE 必须先验证 Secret，再求值相同 ACL；
- DENY、无效 Invite、OFFLINE 和不存在使用统一不可用语义。

结果：通过。

### 空 Matcher 与首条匹配

- Rule 10 只填写特定 `NLI_ACCOUNT_ID` 并 ALLOW，其他 Matcher 为空，表示该账号从任意有效 Source 均匹配；
- Rule 20 只填写 `source=INVITE_CODE` 并 ALLOW，表示任意身份持有效邀请码均匹配；
- 最后的全空 Matcher DENY 匹配所有剩余 Context；
- 非空字段之间 AND、字段内多个值 OR；
- priority 最小的首条匹配生效，不再额外执行后续 DENY/ALLOW；
- ACL 为空时默认 DENY。

结果：通过。

### 没有 NLI Account 与第三方关系

- 没有有效 NLI Account Session 的请求者只能建立 `ANONYMOUS` Guest；
- Anonymous 只能通过验证后的 `INVITE_CODE` Source；
- Provider Friend 或外部好友投影不会生成联机 Trait 或 Source；
- 所有 Instance 都由 NLI 后端上的 Owner Instance Session 发布；
- 客户端不能自报 DIRECT_FRIEND、PROXY_FRIEND 或第三方来源。

结果：通过。

### 伪造 MC Profile

- 客户端可以提交格式合法但虚假的 source、UUID 和 username；
- 服务端始终保存并返回 `CLIENT_CLAIMED`；
- Owner 可把 MC username 用于显式标记为 WEAK_CLIENT_CLAIMED 的 ALLOW/DENY Matcher；
- 它不能改变 Account ID、Identity Trait、Source、Owner 或 Session；
- v2 不接受头像或任意 Metadata JSON；
- 恶意行为通过举报通道和可选的联机后客户端本地二次校验处理，而不是伪造后端 Profile 可信保证。

结果：通过。

### 弱 MC username ALLOW

- Owner 可以显式配置 `MC_USERNAME` ALLOW Rule；
- Rule 和求值解释均标记 `WEAK_CLIENT_CLAIMED`；
- 攻击者可以声明相同 username 并命中，因此 NLI 不把结果描述为身份白名单；
- 可靠 Account ID、Session、Source 和 Proxy/Invite 证明仍由后端确定；
- MOD UI 必须提示风险，联机后可进行本地二次校验。

结果：通过。

### Proxy Secret 泄漏给第三方

- Grant 绑定固定 Grantee Account；
- 第三方即使获得 Secret，也无法用自己的 Instance Session 绑定；
- 失败响应不暴露 Issuer、Grantee 或 Grant 状态；
- Issuer 可以轮换 Secret，现有合法绑定不受影响。

结果：通过。

### Proxy 绑定与撤销并发

- 两条路径都锁定 Grant 和 Instance；
- 撤销先提交时绑定失败；
- 绑定先提交时随后撤销立即移除展示并使 Proxy Join 路径失效；
- 异步读模型延迟不能延长授权。

结果：通过。

### 无到期 Proxy Grant 与好友解除

- Grant 可以无 expires_at 并长期占用 Issuer 额度；
- Issuer 与 Grantee 解除好友时授权检查立即失败；
- 后台任务把 Grant 终止为 REVOKED；
- 双方重新成为好友后旧 Secret 和 Grant 均不恢复；
- 必须创建新 Grant。

结果：通过。

### Invite 次数并发预留

- 剩余一次时两个 Join Request 并发；
- Invite 行锁和计数约束只允许一个 Reservation；
- 被拒绝、取消或过期时释放并允许后续请求；
- ACCEPTED 时只消费一次；
- 自动接受在同一事务预留并消费。

结果：通过。

### Invite Secret 轮换与已有请求

- 轮换后旧 Secret 和尚未创建请求的旧 Generation Guest 不能发起新请求；
- 已有 PENDING 持有独立 Reservation，可以继续审批；
- 撤销 Invite 则取消这些 PENDING 并释放 Reservation；
- 列表和日志均不能找回新旧 Secret。

结果：通过。

### Guest Token 跨实例重放

- Guest Token 绑定 Instance、Invite、Generation 和唯一 Join Request；
- 对其他 Instance、第二个 Request 或普通 API 使用时拒绝；
- 终态后最多 60 秒只能读取原结果；
- 绝对 5 分钟后不可恢复或刷新。

结果：通过。

### Anonymous 重复骚扰

- Guest ID ban 阻止当前 Session；
- 新建 Guest 仍需有效 Invite，并受到 IP、Invite、Instance 和 pending 限制；
- 服务端可按短期 IP 摘要封禁最长 24 小时；
- Owner 看不到 IP，也不能把伪造 MC UUID 当作永久身份。

结果：通过。

### 手动审批期间 Profile 变化

- Owner 看到 Request 创建时的 CLIENT_CLAIMED 快照；
- Source Instance 后续改变 Profile 不取消旧 PENDING；
- Accept 与 Lease 不比较当前 Profile 和请求快照是否一致；
- ACL 对该 Request 的 MC_USERNAME 匹配始终使用保存的快照；
- MOD 可在建立联机后执行本地二次校验，声明差异可进入举报证据。

结果：通过。

### Auto Approval 与 Invite 消费

- `approval_mode=AUTO` 执行完整 Session、Source、Identity Trait、最新 ACL、实例状态和次数检查；
- Request 创建、Reservation、ACCEPTED 和 60 秒 Lease 原子提交；
- 中途失败整体回滚，不留下次数占用或 PENDING；
- 原始 Invite 不进入通知或 Request 读模型。

结果：通过。

### Accept 与 Target 离线并发

- 两者锁定同一 Target 和 Request；
- 离线先提交则 Request 被取消，Accept 失败；
- Accept 先提交可以形成 Lease，但 Target 随后离线会使 Lease 的使用时复核失败；
- 不会在 OFFLINE Instance 上产生可用加入授权。

结果：通过。

### Source 或 ACL 在审批前失效

- Direct Friend 解除、Proxy Grant 撤销、Invite 撤销或 ACL 更新为 DENY 会取消受影响 PENDING；
- 若异步取消尚未执行，Accept 事务仍会重新验证 Source 与最新 ACL；
- Profile 变化和非相关描述变更不取消请求。

结果：通过。

### Acceptance Lease 重放

- Request ID 本身不是 Bearer Secret；
- 必须同时提供记录匹配的双方认证 Session；
- Lease 仅 60 秒；
- Target/Source OFFLINE、Session 撤销、Source 失效或最新 ACL 变为 DENY 会使使用失败；
- Lease 过期后历史 ACCEPTED 状态不能再次授权。

结果：通过。

### 通知丢失

- WebSocket 只发送提示，不承担状态写入；
- Target 和 Requester 均可用各自绑定 Session 通过 HTTP 查询；
- Guest 在终态后 60 秒内可以查询；
- PENDING、终态和 Lease 的权威状态均在 HTTP 读模型中。

结果：通过。

## Gate D 重新评审

重新打开后，Gate D 冻结以下修订边界：

- `ACTIVE / CLOSED` 生命周期、`ONLINE / OFFLINE` 租约和 10 分钟重连；
- 同一 Token Family 独占管理、每账号 5 个额度和 UUIDv4 Instance ID；
- 有序 Matcher+Action ACL 同时决定发现和请求资格，无匹配默认 DENY；
- Matcher 空项表示任意，字段间 AND、字段内 OR；
- Identity Traits 仅来自 NLI Account、Direct Friend、Proxy Friend 或 Anonymous；
- Sources 仅为服务端验证的 FRIEND_LIST 或 INVITE_CODE，不存在第三方联机来源；
- typed NLI Account ID 与 WEAK_CLIENT_CLAIMED MC username Matcher；
- approval mode 与 ACL 分离，AUTO 仍执行完整 ACL 和硬门槛；
- MC Profile 只用于声明展示、弱匹配和举报，变化不取消 Request 或改变可靠授权；
- Proxy Grant 的单实例/单目标、可选无到期、终止撤销和 PROXY_FRIEND Trait；
- Invite 的 3 个额度、24 小时默认/7 天最大、1–100 次 Reservation；
- Guest 的单流程、5 分钟绝对期限、最小公开资料和临时封禁；
- Join Request 状态机、60 秒 Request/Lease、原子 Auto Approval 和 ACL 二次求值；
- 权限矩阵、结果保留、举报边界与通知丢失恢复。

数据库表名和索引名留到数据模型阶段；API Path、举报入口和 WebSocket Event Envelope 分别由 `rest_api.md`、`notifications.md` 冻结。后续设计不得改变上述领域授权语义，除非重新打开 Gate D 并记录决策。

## Phase 4 完成条件

- [x] Instance 生命周期和 Session 绑定冻结
- [x] ACL、Source、Identity Trait 和 Profile 声明边界冻结
- [x] 代理发布授权生命周期冻结
- [x] 邀请码、Guest Session 和匿名限制冻结
- [x] Join Request 状态机、ACL 二次求值和 Acceptance Lease 冻结
- [x] 权限矩阵及修订后安全并发场景通过走查
- [x] Gate D 重新评审通过
