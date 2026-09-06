# NetherLink v2 WebSocket 通知与实例保活

> 状态：`PHASE_5_COMPLETE`
>
> 依赖：`common.md`、`nli_account.md`、`friendship.md`、`game_instance.md`，Gate C、Gate D 已通过。
>
> 本文档定义 WebSocket 建立鉴权、WS Session、实例在线租约、通知事件、HTTP 状态恢复和多节点路由边界。

## 目标

- 使用 Instance Session 为一个 Game Instance 建立唯一当前 WebSocket；
- 通过客户端标准 Ping 维持 90 秒在线租约；
- 让新连接安全替换旧连接，并阻止旧连接继续续租；
- 将 WebSocket 限定为通知与保活通道；
- 让所有关键业务状态都能通过 HTTP 恢复；
- 支持单节点与多节点部署；
- 定义最小、稳定、可扩展的 Event Envelope。

## 非目标

- 不通过 WebSocket 创建、接受、拒绝或取消业务资源；
- 不把 WebSocket 当作事件永久存储；
- 不在本阶段设计 NAT 穿透、TURN 或游戏流量中继；
- 不保证每条通知恰好一次送达；
- 不用 Ping/Pong 承载业务 Payload。

## 已确认的不变量

- WebSocket 只用于通知和 Instance 在线租约；
- HTTP 资源是权威状态；
- 一个 Instance 同时只有一个当前 WS Session；
- 新连接原子替换旧连接；
- 旧连接不能继续续租；
- 客户端建议每 30 秒发送标准 Ping；
- 当前连接收到有效 Ping 后，租约更新为当前时间加 90 秒；
- 服务端回复标准 Pong，不主动发送 Ping；
- 租约失效立即使 Instance OFFLINE 并取消 PENDING Join Request；
- Instance OFFLINE 后有 10 分钟重连宽限，超时关闭并解绑 Token Family；
- Event ID 用于相关性与客户端去重，不提供无限事件重放保证。

## 设计批次

### 批次 1：连接鉴权与 WS Session（已完成）

- [x] 只有 `ACTIVE_BOUND` Instance Session 可以建立 WebSocket；
- [x] 握手 Token 只通过 `Authorization: Bearer` Header 传输；
- [x] Access Token 只鉴权握手，连接建立后可跨过其到期时间，但续租持续检查 Family/Account/Instance；
- [x] WS Session ID 使用 UUIDv4，在连接就绪消息中返回，不是凭据；
- [x] 新连接原子替换 `current_ws_session_id`，Session ID 本身作为 Fencing Token；
- [x] 旧连接尽力收到 `connection.replaced`，随后用应用关闭码 4001 关闭；
- [x] 每个 Instance 一个当前连接，每个 Account 最多随其 5 个 ACTIVE Instance 拥有 5 个连接。

### 批次 2：Ping/Pong 与实例租约（已完成）

- [x] 握手鉴权并原子取得 current WS Session 时立即 ONLINE；
- [x] 同时设置权威时间 `now + 90s` 的租约并取消 OFFLINE 自动关闭计划；
- [x] 客户端只用标准 WebSocket Ping，服务端标准 Pong 回显 Payload；
- [x] 迟于 `connection_expires_at` 的 Ping 不能复活连接，必须重新握手；
- [x] 当前连接主动或异常关闭时立即 OFFLINE；被替换的旧连接关闭不影响新连接；
- [x] Pong 正常回复，但共享租约写入最多每 10 秒一次；持续滥用关闭连接；
- [x] 多节点过期判断使用共享存储/服务端 UTC 毫秒，单调时钟只用于本地调度；
- [x] OFFLINE 立即取消 PENDING，10 分钟后无新连接才关闭 Instance。

### 批次 3：通知 Envelope 与事件类型（已完成）

- [x] Envelope 包含 UUIDv4 Event ID、type、scope、可选资源引用和创建时间；
- [x] 业务通知只提示变化，不携带完整资源快照；
- [x] 投递为 best-effort，允许丢失、重复和乱序，不使用客户端 ACK；
- [x] Event ID 在事务 Outbox 创建，重试和多实例 Fanout 复用同一 ID；
- [x] 不提供全局或账号 sequence，同 Socket 仅尽力保持发送顺序；
- [x] 账号事件 Fanout 到该账号全部 ONLINE Instance；
- [x] 队列超限时关闭连接并要求 HTTP resync；
- [x] 未知事件或字段被忽略，并按 scope 触发对应 HTTP 刷新；
- [x] 初始连接、账号、好友、Provider、Instance、Join 和任务事件类型已列出。

### 批次 4：HTTP 恢复与多节点（已完成）

- [x] 不提供 Event Replay 或 Last-Event-ID；重连使用 HTTP 完整恢复；
- [x] 每次 `connection.ready` 后必须刷新账号和当前 Instance 状态；
- [x] 长连接期间不强制周期性刷新，收到事件、重新获焦或用户主动刷新时可重新查询；
- [x] LeaseStore 不可用时拒绝新连接和续租，不降级到节点本地造成脑裂；
- [x] 优雅重启使用 1012 并 compare-delete 当前 Session；崩溃连接最多 90 秒后过期；
- [x] Outbox 成功发布到内部路由即完成，不声称客户端收到，完成记录保留 24 小时；
- [x] 使用统一 LeaseStore + EventBus 接口，单节点内存实现，多节点共享实现。

## 连接鉴权

### Principal 与握手

只有 `nli_account` Audience、Token Family 为 `ACTIVE_BOUND` 的 Instance Session 可以连接。

握手必须验证：

- Access Token 当前有效且未撤销；
- Account 为 `ACTIVE`；
- Token Family 为 `ACTIVE_BOUND`；
- Family 绑定的 Instance ID 与请求目标完全一致；
- Instance 为 `ACTIVE` 且 Owner/Family 与 Token 记录一致；
- 账号和实例连接额度未违反既有上限。

普通 Account Session、其他同账号 Instance Session、`nli_guest`、Provider Token 和未来 `nli_admin` 都不能代替目标 Instance Session 建立该连接。

Token 只允许通过 WebSocket HTTP Upgrade 请求的：

```http
Authorization: Bearer <nli_account_access_token>
```

禁止放入 Query String、Cookie、URL Path、`Origin`、自定义可回显 Header 或 `Sec-WebSocket-Protocol`。握手日志必须应用与普通 Authorization Header 相同的脱敏规则。

生产环境只接受 `wss`。服务端不使用 Cookie 鉴权；若 Upgrade 带有 `Origin`，必须按部署配置校验允许来源，Mod 原生连接可以不发送 Origin。握手按 IP、Account 和 Token Family 限流，并限制尚未完成的并发握手，防止连接洪泛。

目标 Instance ID 可以用于路由，但不是凭据。无效 Token、Family 不匹配、Instance 已关闭等鉴权失败不能通过差异化公开消息泄漏其他实例状态。

### Access Token 到期与撤销

Access Token 只证明握手当时可以建立连接。连接成功后绑定的是服务端已验证的 Account、Token Family、Instance 和 WS Session：

- 握手 Token 后续自然到期不会单独关闭连接；
- Refresh 轮换并使旧 Access Token 对新 HTTP 请求失效时，不关闭已有 WebSocket；
- 每次有效 Ping 续租前重新检查 Account、Token Family 和 Instance 状态；
- Family 撤销、compromised、解绑，Account 非 ACTIVE 或 Instance CLOSED 时立即关闭，最迟不得超过下一次租约检查；
- 已建立连接不能提交新 Access Token 在原 Socket 上切换身份；需要更换 Principal 时必须重新握手。

## WS Session 与原子替换

### 易失状态

保活权威状态严格为：

```text
instance_id
current_ws_session_id
connection_expires_at
```

`current_ws_session_id` 使用 UUIDv4。它在连接就绪控制消息中返回，用于客户端诊断和日志相关性，但不是 Bearer 凭据，不能代替连接上下文、Access Token 或 Instance Session。

节点本地可以维护 `ws_session_id -> socket` 映射，多节点路由目录可以维护 Session 所在 Node，但这些是传输索引，不参与 Instance 授权，也不增加心跳业务字段。

不持久化：

- last_seen；
- 心跳计数；
- 客户端 Ping 序号；
- 额外 fencing generation；
- 消息历史；
- Token 原文。

### 建立和替换

连接建立流程：

1. 完成初步握手鉴权；
2. 锁定 Instance 业务记录并重新检查 Account、Family、绑定与 `ACTIVE` 状态；
3. 生成新的 UUIDv4 WS Session ID；
4. 在共享易失存储中对该 Instance 原子替换 `current_ws_session_id` 和租约；
5. 清除 `offline_since`/自动关闭截止并提交业务事务；事务失败时按新 Session ID 补偿删除租约；
6. 记录旧 Session 所在 Node 的传输路由；
7. 先向新 Socket 入队 `connection.ready`，再把它标记为可接收业务 Event；
8. 新连接成为唯一当前连接；
9. 尽力通知并关闭旧连接。

CAS 到 `connection.ready` 之间丢失的通知由 ready 后强制 HTTP 恢复覆盖。业务 Event 不能先于该连接的 ready 消息发送。

WS Session ID 本身是 Fencing Token。任何 Ping、通知投递或关闭处理在改变租约/在线状态前，都必须比较：

```text
connection.ws_session_id == current_ws_session_id
```

不匹配时连接已经被替换：

- 不能刷新 `connection_expires_at`；
- 不能把 Instance 从 OFFLINE 改回 ONLINE；
- 不再接收普通通知；
- 尽力发送 `connection.replaced`；
- 使用应用关闭码 `4001` 关闭。

`connection.replaced` 是尽力而为的连接控制提示。即使 Pub/Sub 或关闭消息丢失，CAS 后的旧连接也已经失去续租资格。

### 连接限制

- 每个 Instance 同时只有一个 current WS Session；
- 每个 Account 最多 5 个 ACTIVE Instance，因此最多 5 个 current Instance WebSocket；
- 重连替换旧连接，不额外占用连接额度；
- 不为同一 Instance 保留主备连接；
- Account Session 和 Guest Session 不拥有通知 WebSocket；
- 账号级业务事件 Fanout 到该账号当前 ONLINE Instance 的连接；没有在线实例时只保留 HTTP 权威状态。

## Ping/Pong 与在线租约

### ONLINE 转换

成功握手并原子写入新的 `current_ws_session_id` 时：

1. 使用权威服务端时间计算 `connection_expires_at = now + 90s`；
2. Instance 立即成为 ONLINE；
3. 清除之前的 `offline_since` 和待执行自动关闭时间；
4. 使好友/ACL 查询可以按最新状态返回 Instance；
5. 新连接不等待首个 Ping。

如果 Instance 已经 CLOSED、Family 已解绑或账号不再 ACTIVE，连接流程必须在取得当前 Session 前失败，不能通过写入租约复活资源。

### 客户端保活

- 客户端推荐每 30 秒发送标准 WebSocket Ping Control Frame；
- 服务端按 RFC 6455 返回标准 Pong，并回显 Ping Payload；Pong 不是续租成功证明；
- Ping Payload 不定义业务结构，最大长度遵循 WebSocket Control Frame 限制；
- 服务端不主动发送 Ping；
- 客户端不得发送 Text/Binary 业务 Data Frame；收到此类帧视为协议违规并可用 `1008 Policy Violation` 关闭；
- Pong 本身不写业务状态；只有来自 current WS Session 的有效 Ping 才有续租资格。

### 续租

收到 Ping 时执行：

1. 协议栈可以立即发送标准 Pong；
2. 从连接上下文取得 WS Session ID、Instance ID 和 Token Family ID；
3. 使用共享权威时间读取当前租约；
4. 确认 `ws_session_id == current_ws_session_id`；
5. 确认当前时间严格早于 `connection_expires_at`；
6. 重新检查 Account ACTIVE、Family ACTIVE_BOUND 且仍绑定该 Instance、Instance ACTIVE；
7. 更新租约为 `now + 90s`。

Pong 只确认协议 Ping 已被接收，不表示第 4～7 步成功。续租失败时服务端随后关闭连接；客户端必须以 `connection.ready`、HTTP 状态和 Socket 是否保持连接判断可用性，不能把 Pong 当授权证明。

如果 Ping 在原租约截止后到达，即使过期 Worker 尚未执行，也不能续租或把 Instance 恢复 ONLINE；连接必须关闭并重新握手生成新 WS Session。

为限制共享存储写入：

- Pong 可以对合法 Ping 正常回复；
- 同一 current WS Session 的租约延长最多每 10 秒实际写入一次；
- 合并写入不能缩短已有租约；
- 正常 30 秒周期不受影响；
- 持续异常高频 Ping 进入连接级限流，超过阈值时以 `1008` 关闭；
- 被合并而未写租约的 Ping 不产生额外业务事件。

### 时钟

- 共享租约存储或统一服务端时间源提供 UTC Unix 毫秒权威时间；
- CAS、Ping 续租和过期判断使用同一时间语义；
- 节点单调时钟只用于调度本地 Timer，不能独立决定跨节点权威过期；
- 时钟异常不得把已过期 Session 延长到过去或复活旧 Session。

### 当前连接关闭

当前 Socket 主动 Close、网络异常或服务端关闭时：

1. 比较关闭连接的 WS Session ID 与 current ID；
2. 只有相等时原子清除 current Session 和租约；
3. 锁定 Instance 业务记录并再次确认 LeaseStore 中没有更新的 current Session；
4. Instance 立即成为 OFFLINE；
5. 记录 `offline_since` 和 10 分钟自动关闭截止时间；
6. 取消全部 PENDING Join Request；
7. 从公开 ACL/好友查询中隐藏；
8. 发送必要的资源变化通知。

被替换的旧连接关闭时 ID 已不相等，因此不能清除新租约、不能使 Instance OFFLINE，也不能取消新连接期间产生的 Request。

### 租约过期

过期 Worker 处理时使用 compare-and-delete：

```text
current_ws_session_id == observed_ws_session_id
AND connection_expires_at == observed_expires_at
AND connection_expires_at <= authoritative_now
```

只有比较成功才触发一次幂等 OFFLINE 处理。若期间发生 Ping 续租或新连接替换，旧过期任务不产生作用。

在线状态由共享租约权威推导；业务数据库保存 Instance 生命周期以及 OFFLINE 后自动关闭所需的 `offline_since`。取消 Join Request 和写 Outbox 可以重试，但每个 Request 只能进入一个终态。

LeaseStore 与业务数据库不是一个事务域。所有 ONLINE/OFFLINE/CLOSED 转换都必须锁定 Instance 记录，并在提交前后按预期 WS Session ID 复核 LeaseStore；CAS 成功但业务事务失败时使用 compare-delete 补偿。后台协调任务负责修复“有有效租约但 offline_since 未清除”或“无有效租约但尚未记录 OFFLINE”的短暂不一致，修复期间对外可用性一律以有效 LeaseStore 租约和 ACTIVE 生命周期的交集为准。

### 10 分钟重连宽限

OFFLINE 后：

- 同一有效 Instance Session 可以重新握手并创建新的 WS Session；
- 重连成功立即 ONLINE，但已经取消的 Join Request 不恢复；
- OFFLINE 期间 Instance 仍占用 Account 实例额度；
- 10 分钟截止 Worker 必须再次确认 Instance ACTIVE、当前无有效 WS Session、`offline_since` 未被清除；
- 条件成立才关闭 Instance、撤销 Invite/Proxy 展示并解绑仍有效 Family；
- 自动关闭与新连接竞争时锁定 Instance 并重新检查租约，不能关闭已经重新 ONLINE 的 Instance。

## 通知 Event Envelope

### 业务事件结构

```json
{
  "event_id": "uuid-v4",
  "type": "join_request.created",
  "scope": "RESOURCE",
  "resource": {
    "type": "join_request",
    "id": "uuid-v4"
  },
  "created_at": 0
}
```

字段规则：

- `event_id`：事务 Outbox 创建的 UUIDv4；
- `type`：小写点分稳定枚举；
- `scope`：`ACCOUNT / INSTANCE / RESOURCE`；
- `resource`：RESOURCE 事件必填，集合级 ACCOUNT/INSTANCE 事件可以为空；
- `created_at`：业务事件写入时的 Unix 毫秒；
- 未知字段必须忽略；
- Envelope 默认最大 1 KiB；
- 不包含完整资源 DTO、Access Token、Secret、Provider Subject、IP、MC Profile 或自由文本。

Event 是“状态可能变化”的提示，不是状态快照或授权凭据。收到事件后，客户端通过对应 HTTP API 重新读取当前权威状态。

### Event ID

- Event ID 在业务事务内随 Outbox 记录生成；
- 同一逻辑事件的 Dispatcher 重试复用同一 ID；
- Fanout 到同账号多个 Instance 时复用同一 ID；
- Event ID 不编码时间、Account、Node 或资源信息；
- Event ID 不提供顺序，也不是 HTTP Cursor；
- 客户端应维护有界的最近 Event ID 去重集合；
- 即使 Event ID 重复，客户端重复执行 GET 恢复也必须安全。

### 投递、重复与顺序

投递语义为 best-effort：

- 事件可能在事务提交后丢失；
- Dispatcher 重试可能导致重复；
- 多节点、不同资源和重连之间可能乱序；
- 不提供全局、Account 或 Instance 严格 Sequence；
- 同一 Socket 的发送队列尽力按入队顺序发送，但客户端不能据此覆盖较新的 HTTP 状态；
- 不设计客户端 ACK，也不允许客户端用 Data Frame 确认业务事件；
- HTTP 读取结果始终优先于事件到达顺序。

### Scope 与恢复提示

```text
ACCOUNT
INSTANCE
RESOURCE
```

- `ACCOUNT`：刷新当前账号的相关集合，例如好友申请、绑定或同步任务；
- `INSTANCE`：刷新当前 Instance、ACL、Invite、Proxy 或 Join Request 集合；
- `RESOURCE`：按 resource type/id 获取单个有权读取的资源；
- 客户端收到未知 type 时仍读取 scope，并执行对应安全刷新；
- 未知 scope 不能被当作授权信息，客户端应执行保守的账号/实例 HTTP 刷新；
- 服务端发送前必须确认该逻辑事件的 Recipient Account/Instance，不因 Payload 为空而跳过鉴权。

### Account 和 Instance Fanout

Account 级事件：

- 在投递时解析该 Account 当前所有 ONLINE Instance；
- 向每个 current WS Session 投递相同 Event ID；
- 发送前再次比较 current WS Session ID；
- 没有在线 Instance 时不排队等待未来连接，只保留 HTTP 权威状态；
- 一个 Instance 发送失败不阻止其他 Instance。

Instance 级事件只投递给目标 Instance 的 current WS Session。Join Request：

- Target 侧 created/updated 事件投递给 Target Instance；
- NLI Requester 侧 updated 事件投递给 Source Instance；
- Guest 没有 WS，通过 HTTP 查询唯一 Request；
- Proxy Grant Issuer 仅接收账号级变化提示，没有 Target Instance 审批能力。

### 连接控制消息

连接控制消息不属于业务 Outbox Event，不使用业务 Event ID：

```text
connection.ready
connection.replaced
connection.resync_required
```

`connection.ready` 至少返回：

- WS Session ID；
- Instance ID；
- `connection_expires_at`；
- 推荐 Ping 间隔 30 秒；
- 租约长度 90 秒。

`connection.replaced` 和 `connection.resync_required` 都是尽力而为提示；Close Code 才是连接终止信号。

### 背压

每个连接使用有界发送队列，初始上限取以下任一先到：

- 256 个 Envelope；
- 1 MiB 未发送数据。

超过上限时：

1. 不无限扩张内存；
2. 不选择性丢弃部分业务事件后继续伪装同步；
3. 尽力发送 `connection.resync_required`；
4. 使用应用关闭码 `4003` 关闭；
5. 客户端重连后执行 HTTP 全量恢复。

### 初始事件类型

连接控制：

```text
connection.ready
connection.replaced
connection.resync_required
```

账号与认证：

```text
account.updated
session.revoked
provider_binding.updated
```

好友与同步：

```text
friend_request.created
friend_request.updated
friendship.updated
friend_sync_task.updated
```

Instance 与授权：

```text
instance.updated
instance.closed
instance.acl_updated
instance.invite_updated
proxy_grant.updated
```

Join：

```text
join_request.created
join_request.updated
```

事件类型不表达完整状态。新增 type 必须保持旧客户端可忽略，并在 HTTP 恢复矩阵中指定对应查询入口。

## HTTP 状态恢复

### 无事件重放

v2 不提供：

- Last-Event-ID；
- WebSocket Event Cursor；
- 离线事件 Inbox；
- 按时间重放通知；
- 永久用户事件日志。

Outbox 是服务端事务投递机制，不是客户端可查询事件源。客户端不能根据 Event ID 推测是否漏掉了中间事件。

### 连接后恢复

每次收到 `connection.ready` 后，客户端必须：

1. 获取当前 Account/Session 摘要；
2. 获取好友、好友申请和必要的 Provider/同步任务状态；
3. 获取当前 Instance、ACL、Invite、Proxy 状态；
4. 获取 Target 和 Requester 侧可见的 Join Request；
5. 用 HTTP 结果替换本地缓存；
6. 之后再把业务事件作为刷新提示。

客户端不能假设“重连成功”表示离线期间没有变化。

本阶段不强制固定周期 HTTP 轮询。连接保持期间：

- 收到事件时按 scope 刷新；
- 应用窗口重新获焦或用户主动刷新时可以重新获取；
- 若某条 Pub/Sub 通知静默丢失，状态可能直到下一次事件、重连或主动刷新才更新；
- 这只影响提示及时性，不影响服务端权威状态或授权检查。

### 恢复矩阵

| Event / Scope | HTTP 权威查询 |
| --- | --- |
| `ACCOUNT` | 当前账号、Session、Provider Binding 和账号设置摘要 |
| `friend_request.*` | Incoming/可见 Outgoing Friend Request 集合 |
| `friendship.updated` | NLI 好友 Pair/好友聚合集合 |
| `friend_sync_task.updated` | 对应 Task 或 Task 集合 |
| `INSTANCE` | 当前 Instance、ACL、Invite、Proxy 摘要 |
| `instance.acl_updated` | 当前 Instance ACL 与 Revision |
| `instance.invite_updated` | Owner 可见 Invite 元数据集合 |
| `proxy_grant.updated` | Issuer Grant 集合或 Owner Instance Proxy 摘要 |
| `join_request.created` | Target Instance 的 PENDING Join Request 集合 |
| `join_request.updated` | 对应 Request；不可见时刷新自身 Request 集合 |
| `session.revoked` | 当前账号 Session 集合；受影响连接随后关闭 |
| 未知 `RESOURCE` | 尝试有权的资源 GET，失败后刷新所属集合 |

具体 HTTP Path 在 Phase 6 冻结，但每种事件必须先有可鉴权的权威查询语义。

Guest 没有 WebSocket，只通过绑定 Guest Token 查询唯一 Join Request。

## Outbox 与投递边界

业务事务原子写入 Outbox：

```text
PENDING -> PUBLISHED
        -> RETRY_WAIT
```

- Dispatcher 发布到内部 EventBus 成功后标记 PUBLISHED；
- PUBLISHED 只表示内部路由接受，不表示任一 Socket 或客户端收到；
- 发布失败使用有界指数退避和抖动重试；
- Event ID 和 Envelope 在重试中保持不变；
- 完成记录保留 24 小时用于排障和去重，之后可以清理；
- 没有在线 Recipient 时可以记录为已路由但无活跃连接，HTTP 状态仍然完整；
- 不因通知失败回滚已经提交的好友、Instance、ACL 或 Join 事务；
- Outbox 不保存 Secret、Token、IP 原文或完整业务 DTO。

## 单节点与多节点抽象

### LeaseStore

LeaseStore 必须支持：

- 按 Instance 原子 compare-and-swap current WS Session；
- 按 Session ID 和 Expires At 条件续租/删除；
- TTL 或等价过期扫描；
- 共享权威时间语义；
- 查询 Instance 当前 Session；
- 查询 Account 当前 ONLINE Instance 用于 Fanout。

### EventBus

EventBus 必须支持：

- 向指定 WS Session/Instance 路由控制和业务提示；
- 向 Account 当前在线 Instances Fanout；
- 跨节点通知旧连接已被替换；
- best-effort Pub/Sub；
- 发送失败与背压指标。

单节点可以使用遵循相同接口的进程内实现。多节点必须使用支持原子 CAS、条件删除、TTL 和跨节点 Pub/Sub 的共享实现；本文不冻结具体 Redis、数据库或消息系统产品。

节点本地 Socket Map 和共享路由目录只是传输索引。所有续租和 OFFLINE 判断仍以 LeaseStore 的 current WS Session 为准。

### 多节点替换

新连接落到 Node B 时：

1. Node B 在 LeaseStore CAS 成为 current；
2. EventBus 通知旧 Session 所在 Node A；
3. Node A 尽力发送 `connection.replaced` 并关闭；
4. 即使第 2 步丢失，Node A 后续 Ping 因 current ID 不匹配而失败；
5. 业务事件只向 CAS 后的 current Session 投递。

### LeaseStore 故障

共享 LeaseStore 不可用时采用 fail-closed：

- 新 WebSocket 握手返回服务暂不可用；
- 现有连接不能续租；
- 不退化为节点本地 current Session；
- 不把旧 expiry 无限延长；
- HTTP/ACL 授权把无法证明 ONLINE 的 Instance 视为不可用；
- 存储恢复后客户端必须重新握手；
- 现有连接按最后成功写入的 90 秒租约过期。

这可能降低可用性，但避免多节点同时取得同一 Instance 的 current 身份。

## 服务重启与部署

### 优雅关闭

节点优雅关闭时：

1. 停止接受新 WebSocket；
2. 对每个本地 Socket 比较 LeaseStore current WS Session；
3. 尽力使用 `1012 Service Restart` 关闭 current 连接；
4. compare-delete 自己仍持有的 current Session；
5. 触发幂等 OFFLINE、PENDING Join Request 取消和 10 分钟宽限；
6. 等待有界时间完成 Outbox/路由排空后退出。

客户端收到 1012 后使用退避和抖动重连。新连接会创建新 WS Session，已经取消的 Join Request 不恢复。

### 非正常崩溃

节点崩溃无法清理 Socket 时：

- LeaseStore 中最后租约最多保留 90 秒；
- 过期 Worker compare-and-delete 后触发 OFFLINE；
- 客户端可立即向其他节点重连并原子替换旧 Session；
- 旧节点恢复后不能复用旧 WS Session ID；
- 如果整个易失 LeaseStore 丢失，所有 Instance 视为 OFFLINE，客户端重新握手。

### 关闭码

初始使用：

```text
1000  Normal Closure
1008  Policy Violation / Ping abuse / Client data frame
1012  Service Restart
4001  Connection Replaced
4002  Authentication or Lease No Longer Valid
4003  Resync Required / Backpressure
```

关闭原因不包含 Token、账号状态、内部 Node、ACL Rule 或其他敏感信息。

## 安全与故障场景走查

### 使用错误 Token Family 握手

- Access Token 有效但 Family 绑定另一个 Instance；
- 握手在创建 WS Session 前拒绝；
- 不关闭或替换目标 Instance 的 current Session；
- 响应不泄漏目标 Instance 是否存在或在线。

结果：通过。

### Token 泄漏到 URL 防护

- 服务端只从 Authorization Header 读取 Bearer Token；
- Query、Cookie 和 Subprotocol 中的 Token 不参与认证；
- Upgrade 请求日志脱敏 Authorization；
- 代理访问日志和关闭原因不包含 Token。

结果：通过。

### Access Token 到期与 Refresh

- 握手后 Access Token 自然到期或 Refresh 轮换旧 Token；
- 已建立 Socket 继续绑定原 Token Family；
- Ping 重新检查 Family/Account/Instance，而非旧 Access Token 字符串；
- 新握手必须使用当前有效 Access Token；
- Family 撤销时当前连接关闭。

结果：通过。

### 两节点同时重连

- Node A 和 Node B 同时尝试成为 current；
- LeaseStore 对 Instance 的 CAS 串行化替换；
- 最后成功 CAS 的 WS Session 是唯一 current；
- 另一个连接的 Ping 因 Session ID 不匹配不能续租；
- 旧连接关闭消息丢失也不影响 Fencing。

结果：通过。

### 被替换连接迟到 Close

- 新连接已经 CAS 替换 current Session；
- 旧 Socket 随后触发 Close Handler；
- Handler compare-delete 发现 WS Session ID 不匹配；
- 不清除新租约、不触发 OFFLINE、不取消新 Request。

结果：通过。

### 租约截止后的迟到 Ping

- Ping 到达时 authoritative now 已不早于 Expires At；
- 即使过期 Worker 尚未运行，也拒绝续租；
- 连接以失效语义关闭；
- 客户端重新握手取得新 WS Session；
- 已触发的 OFFLINE/PENDING 取消不会恢复。

结果：通过。

### 高频 Ping 滥用

- 合法 Pong 可以由协议栈回复；
- 租约写入最多每 10 秒一次；
- 持续高频行为触发连接级限制并以 1008 关闭；
- 攻击者不能通过 Ping 制造无限共享存储写入或业务事件。

结果：通过。

### Ping 续租与过期 Worker 并发

- Worker 持有观察到的 Session ID 和 Expires At；
- Ping 先成功续租时 Expires At 改变，Worker compare-delete 失败；
- Worker 先成功删除时 Ping 不能复活；
- OFFLINE 处理幂等且仅由成功删除者触发。

结果：通过。

### OFFLINE 重连和自动关闭并发

- Instance OFFLINE 后记录 10 分钟截止；
- 重连先成功时清除 offline_since，关闭 Worker 复核失败；
- 关闭 Worker 先提交时 Instance CLOSED，重连握手失败；
- Token Family 不会同时处于解绑和 ONLINE Instance 状态。

结果：通过。

### 业务事务提交但 EventBus 暂时失败

- 业务资源和 Outbox 在同一事务提交；
- Dispatcher 保留同一 Event ID 退避重试；
- 客户端可能暂时无提示，但 HTTP 已返回权威状态；
- 通知失败不回滚业务事务。

结果：通过。

### Account 事件多实例 Fanout

- Account 有三个 ONLINE Instance；
- Dispatcher 向三个 current WS Session 投递同一 Event ID；
- 一个节点失败不阻止其他两个；
- 客户端按 Event ID 去重并各自执行 HTTP 刷新；
- 没有在线 Instance 时不建立离线 WS 队列。

结果：通过。

### 事件重复和乱序

- `join_request.updated` 可能早于重复的 `join_request.created` 到达；
- 客户端不按事件顺序重建状态；
- 两个事件都只触发 HTTP GET；
- 当前 HTTP 终态不会被迟到事件覆盖。

结果：通过。

### 发送队列背压

- 单连接队列超过 256 条或 1 MiB；
- 服务端不继续无限缓存，也不静默丢一部分后假装同步；
- 尽力发送 resync 提示并以 4003 关闭；
- 重连 ready 后客户端执行完整 HTTP 恢复。

结果：通过。

### 未知事件类型

- 旧客户端收到新 type，但认识 ACCOUNT/INSTANCE/RESOURCE scope；
- 忽略未知字段和业务 type；
- 按 scope 执行保守 HTTP 查询；
- 不断开连接，也不把未知 Payload 当授权指令。

结果：通过。

### LeaseStore 故障

- 新连接无法安全 CAS，因此返回暂不可用；
- 现有连接不能延长租约；
- 所有节点不退化到本地 current Session；
- HTTP 把无法证明 ONLINE 的 Instance 视为不可用；
- 存储恢复后客户端重新握手。

结果：通过。

### 优雅重启与进程崩溃

- 优雅重启以 1012 关闭并 compare-delete current Session，立即 OFFLINE；
- 客户端退避重连，建立新 Session；
- 非正常崩溃无法删除租约时，最多 90 秒后过期；
- 旧 Session ID 在进程恢复后不能复用。

结果：通过。

### Guest 尝试建立 WebSocket

- `nli_guest` Token 在 Upgrade 鉴权阶段拒绝；
- 不为 Guest 分配 WS Session 或连接额度；
- Guest 继续通过受限 HTTP 查询唯一 Join Request；
- 不影响目标 Instance 的 current Socket。

结果：通过。

### 通知完全丢失

- 好友、ACL 或 Join 事务已经提交，但客户端没有收到 Event；
- 每次 connection.ready 后客户端完整刷新；
- 长连接中可在下一事件、窗口获焦或用户主动刷新时恢复；
- 服务端所有授权继续使用 HTTP/数据库/LeaseStore 权威状态，不依赖客户端是否收到通知。

结果：通过。

## Phase 5 完成条件

- [x] WebSocket Principal 和握手鉴权冻结
- [x] WS Session 替换与 Fencing 冻结
- [x] Ping/Pong 和实例租约冻结
- [x] Event Envelope 和初始事件类型冻结
- [x] HTTP 恢复矩阵冻结
- [x] 单/多节点与重启行为冻结
- [x] 安全和故障场景通过走查
