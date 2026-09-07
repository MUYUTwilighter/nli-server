# NetherLink v2 P2P 信令、NAT 与 TURN

> 状态：`PHASE_9_COMPLETE / GATE_F_FROZEN + D-329_ROUTING_AMENDMENT`
>
> 依赖：Gate D、Gate E（均已通过）
>
> 本文只设计已批准 Join Request 之后的短期连接协商。不得反向改变 `game_instance.md`、`rest_api.md` 或 `openapi.yaml` 已冻结的身份、ACL、Session、Join Request 与 Acceptance Lease 语义。

## 目标

- 只为仍满足全部不变量的 `ACCEPTED` Join Request 建立短期信令会话；
- 冻结 Offer、Answer、ICE Candidate 与结束/失败状态机；
- 冻结 NAT 穿透策略和短期 TURN 凭据；
- 支持多节点路由、断线恢复、限流和过期收敛；
- 保护 SDP、ICE、网络地址、TURN 凭据和客户端声明 Profile；
- 让客户端在通知丢失、重复、乱序和节点故障后恢复到确定状态。

## 已冻结的输入边界

### 身份与授权

- NLI Account 身份与 Minecraft 身份完全独立；
- MC Profile 始终为 `CLIENT_CLAIMED`，只用于展示、弱 Matcher 和举报上下文；
- MOD 可以在连接建立后本地二次校验，但不得升级 NLI 后端身份保证；
- 无 NLI Account 的请求者只能使用绑定单个目标与单个 Join 流程的短期 Guest Session；
- Provider Login Identity、Provider Binding 和 Provider 好友均不形成信令身份或第三方 Join Source；
- 只有 NLI Host 发布的 Instance 可以成为信令 Target。

### Join 与 Acceptance Lease

- 信令入口只接受已存在的 Join Request ID，不重新创建或隐式批准 Join Request；
- `ACCEPTED` Request 只暴露 60 秒 `lease_expires_at`，不签发独立 Bearer Ticket；
- 真正创建或继续信令会话时，必须原子重验调用方当前 Session、双方 Instance/Guest、Target ONLINE、Source 证明、最新 ACL、Invite/Proxy 状态和未过期 Acceptance Lease；
- 单独的 Lease Validation API 继续禁止，避免检查与使用之间的 TOCTOU；
- ACL、Friendship、Invite、Proxy、Session、Instance 或 Lease 失效必须使后续信令操作失败并收敛既有会话；
- 信令层不得延长 Acceptance Lease，也不得把 Request ID 当作 Bearer Secret。

### HTTP 与现有 WebSocket

- PostgreSQL/HTTP 状态继续是业务权威；
- Phase 5 的 `/instances/{instance_id}/ws` 只负责实例保活和通知，不承载 SDP、Answer 或 ICE Candidate；
- 关键授权和信令会话创建使用明确的 HTTP 状态转换；
- Phase 9 的客户端公共 `/v2/signaling-sessions/{signaling_session_id}/ws`（应用内 `/signaling-sessions/{signaling_session_id}/ws`）是 `common.md`“WebSocket 不承担关键业务状态写入”规则的唯一书面例外，只允许承载本文冻结的短期信令消息、限流和生命周期；Phase 5 通知/保活 WS 的边界保持不变；
- 通知仍允许丢失、重复和乱序；客户端恢复不能依赖通知重放。

## 范围

### 本阶段设计

1. 信令会话创建与双方角色；
2. Offer / Answer / ICE 状态机；
3. 信令消息 Envelope、版本和相关 ID；
4. Trickle ICE、candidate 完成和 ICE restart；
5. STUN/TURN 选择与短期 TURN Credential；
6. 信令会话 TTL、关闭、撤销和垃圾回收；
7. 断线、重连、重复提交和幂等；
8. 多节点路由、节点故障和易失消息边界；
9. 消息大小、频率、队列与背压；
10. SDP/ICE/TURN/网络地址的隐私、日志和审计规则；
11. 客户端恢复流程和错误模型；
12. Phase 9 API/协议示例与验证场景。

### 非目标

- 不重新设计账号认证、Provider、好友、ACL、Invite、Guest 或 Join Request；
- 不把 Minecraft Token、Profile、UUID 或 username 变成 NLI 身份凭据；
- 不设计公共房间、全局实例目录、Provider 联机渠道或任意第三方 Host；
- 不保证 P2P 一定直连；TURN Relay 是受策略约束的正常回退；
- 不在服务端解密或代理游戏应用数据；
- 不把信令消息、SDP 或 ICE Candidate 保存为长期业务历史；
- 不在本阶段定义具体游戏协议或 MOD 本地二次校验协议。

## 批次 1 冻结：授权入口、角色与 Fencing

### 权威状态与存储边界

PostgreSQL 是以下信息的唯一业务权威：

- `signaling_session_id`、`join_request_id` 和一对一关联；
- 创建方角色、固定 Offerer、会话生命周期状态和绝对截止时间；
- 关闭时间和不含敏感细节的终止分类；
- 创建/关闭命令的 Idempotency、Audit 与 Outbox。

Redis/LeaseStore/EventBus 只能保存可丢失的节点路由、每参与方连接 Fencing、短租约、Singleflight、限流、背压和临时投递。Redis 中不存在会话不等于业务会话不存在；PostgreSQL 中不存在或不再 `ACTIVE` 时，Redis 不能使其复活。任何客户端可见的业务状态转换必须先提交 PostgreSQL。

SDP、ICE Candidate、网络地址和 TURN Secret 仍只允许进入内存或后续批次明确规定的短期易失存储，不进入普通业务表、Idempotency Response、Outbox Payload、Audit Detail 或普通日志。旧的 `src/model/signaling.rs`、`src/signaling.rs` 和 Redis Presence 模型属于 v1，v2 不得复用其“接受前建会话、按 Profile/Presence 识别参与方、节点内存即权威”的语义。

### 公开入口

批次 1 冻结以下独立入口；它们是 Gate E 之后的新增扩展，不改变既有 89 个 HTTP Operation。表中是客户端公共 URL；反向代理剥离 `/v2` 后，应用内统一挂载对应的无版本前缀路由：

| Method / Path | Principal | 语义 |
|---|---|---|
| `POST /v2/join-requests/{request_id}/signaling-sessions` | 与 Request 精确绑定的 `INSTANCE` 或 `GUEST` | create-or-attach；Body 为 closed empty object；强制 `Idempotency-Key` |
| `GET /v2/signaling-sessions/{signaling_session_id}` | 该会话任一精确绑定参与方 | 读取最小权威状态，用于通知丢失和重连恢复 |
| `DELETE /v2/signaling-sessions/{signaling_session_id}` | 该会话任一精确绑定参与方 | 天然幂等地关闭整个会话；不能只踢掉对端 |
| `GET /v2/signaling-sessions/{signaling_session_id}/ws` | 该会话任一精确绑定参与方 | 独立信令 WebSocket Upgrade；不能通过 Phase 5 通知 WS 进入 |

`POST` 允许任一方先调用。服务端只能从 Join Request 保存的 Session 绑定推导角色，客户端不得提交角色：

- `REQUESTER`：当前 `nli_account` Access Token 的 Token Family 精确匹配 Request 保存的 `requester_token_family_id`，或当前 `nli_guest` Token 精确匹配 `guest_session_id`；
- `TARGET`：当前 `nli_account` Access Token 的 Token Family 为 `ACTIVE_BOUND`，其绑定 Instance 精确等于 Request 的 `target_instance_id`，并等于该 Target 当前 `owner_token_family_id`；MANUAL Request 的 Decider 只提供一致性证据，AUTO Request 不要求存在 Decider；
- 固定 `offerer=REQUESTER`；谁先创建会话都不改变 Offerer；
- Account Session、同账号的其他 Token Family、重建后的 Instance Session、旧 Guest、Request ID 和 signaling session ID 都不能代替上述绑定。

AUTO 与 MANUAL 产生的 `ACCEPTED` Request 使用完全相同的入口。既有 `join_request.updated` 通知可以在 Request 转为 `ACCEPTED` 时唤醒双方，不新增 Phase 5 Event Type。通知只是提示；任一方都必须通过上述 HTTP 入口 create-or-attach 并恢复权威状态。

### 会话基数与 Acceptance Lease

每个 Join Request 在整个生命周期内最多创建一个逻辑 Signaling Session，数据库使用 `UNIQUE(join_request_id)` 兜底，而不是只约束活动状态：

- 首次合法调用创建并返回 `201 Created`；
- 另一方或同一方使用新 Key 调用时，在完整重验后 attach 既有会话并返回 `200 OK`；
- 原 `Idempotency-Key` 重放必须返回首次保存的相同 HTTP/业务结果，即使会话后来终止；该 DTO 是首次响应快照，客户端必须用 `GET` 获取当前状态；
- 并发首次创建先按全局顺序锁行；唯一约束冲突的事务回读胜者、重新鉴权后返回该同一会话，不得产生第二个 ID 或重复 Outbox；
- `CLOSED`、`EXPIRED` 或后续协议失败后不允许在同一 Request 下重新创建；客户端可重试读取/attach，但需要新协商时必须重新发起 Join Request。

Acceptance Lease **不在创建时一次消费**。它是双方 attach、重连和整个协商共享的绝对授权窗口，但不允许产生第二个逻辑会话：

```text
signaling_expires_at = min(acceptance_lease_expires_at,
                           guest_absolute_expires_at if Guest else +infinity)
```

`signaling_expires_at` 不得续期，也不得晚于数据库权威时钟的 Lease 截止。因为 Invite Reservation 已在 Request `ACCEPTED` 时消费，Invite 后续变为 `EXHAUSTED`、过期或轮换不使该 Lease 自我失效；使用时仍校验该 Request 的已消费 Reservation 完整性，但不重新要求 Invite Secret。Owner 终止此授权仍使用已冻结的 Instance 关闭或 ACL DENY。Proxy Grant、Direct/Proxy Friendship 和其他尚未消费的 Source 证明则必须保持有效。

### 原子授权重验

创建、attach、WebSocket 握手/重连，以及每个会推进状态或向对端投递数据的操作，都必须在确认调用方当前凭据后重验：

1. Join Request 存在且调用方是按保存绑定推导出的参与方；
2. Request 仍为 `ACCEPTED`，Acceptance Lease 严格满足 `database_now() < lease_expires_at`；
3. 调用方 Token Audience 正确，绑定 Token Family/Guest 仍有效，Account（如有）仍 `ACTIVE`；
4. Requester Source Instance 或 Guest 仍有效；Instance Source 必须仍 `ACTIVE + ONLINE`；
5. Target 仍 `ACTIVE + ONLINE`，调用方为 TARGET 时其当前 `ACTIVE_BOUND` Token Family 仍等于 Target 的 `owner_token_family_id`；
6. FRIEND_LIST 的 Direct/Proxy Friendship、Proxy Grant 和 Source Proof 仍有效；INVITE_CODE 路径按上节只验证已消费 Reservation 完整性；
7. 使用 Request 保存的 `CLIENT_CLAIMED` Profile 快照、当前可靠 Identity Traits 和最新 ACL Revision 重新求值，首条匹配仍为 ALLOW；
8. Signaling Session 仍为 `ACTIVE`、未过截止时间，且实时操作携带的连接仍通过当前 Fencing。

不能提供“先验证再使用”的独立接口。后续批次可以为纯读取定义缓存，但任何被接受并投递的 Offer、Answer、ICE、ICE Restart 或 TURN Credential 请求都不能弱化以上 use-time 复核。MC Profile 当前值不得替换 Request 快照。

### 事务与锁顺序

在 `data_model.md` 的全局顺序末尾增加 Signaling Session 叶节点，不重排 Gate E 已冻结顺序：

```text
Idempotency Record
-> Account（UUID 升序）
-> Token Family（UUID 升序）
-> Friend Pair（low/high 升序）
-> Proxy Grant（UUID 升序）
-> Game Instance（UUID 升序）
-> Instance Invite
-> Guest Session
-> Join Request
-> Invite Reservation
-> Signaling Session
-> Audit / Outbox append
```

实现先无锁读取 Request 以发现依赖，再按全局顺序加锁，最后重读 Request、Session 绑定、ONLINE Lease、Source 与 ACL Revision。任何路径都不能先锁 Signaling Session 再反向锁 Join Request 或 Instance。

PostgreSQL 权威撤销（Account/Family/Guest 撤销、Instance 关闭、ACL/关系/Proxy 变更）在同一领域事务中按该顺序终止受影响的活动 Signaling Session并写 Outbox。ONLINE 是 LeaseStore 派生状态，不能伪装成跨存储数据库事务：Lease 到期/丢失由协调 Worker 收敛 PostgreSQL，会话在收敛前也因每次 use-time ONLINE 复核而 fail closed。Acceptance Lease 到期由惰性访问和 Sweeper 幂等转为 `EXPIRED`。

### Participant Connection Fencing

每个 `(signaling_session_id, role)` 最多一个当前实时连接。每次成功握手生成新的 UUIDv4 `connection_id`，LeaseStore 使用批次 4 冻结的 session-scoped Route Document：

```text
signaling_session_id
-> route_revision
 + REQUESTER { connection_id, node_id, boot_id, expires_at }
 + TARGET    { connection_id, node_id, boot_id, expires_at }
```

- 新连接原子替换本 Role Slot并递增 `route_revision`，取得所有权并使旧连接失效；`connection_id` 只是 Fencing Token，不是 Bearer Credential；
- 每次续租、转发、状态推进和 Close 都必须比较当前 `connection_id`；旧节点的延迟帧和旧连接的 Close 不得修改新连接；
- 只有持有当前值的节点可以 CAS 续租；失去所有权必须关闭本地 Socket；
- 握手在 PostgreSQL 完整重验后执行 LeaseStore CAS，并在成功后再次确认权威会话/绑定；后置确认失败时 compare-delete 自己写入的连接值；
- LeaseStore 不可用时禁止新握手和所有状态改变/投递；既有节点不能退化为仅凭本地内存继续授权；EventBus 不可用只触发事务 Outbox 重试，不阻断已具备安全路由的握手；
- 节点路由错误最多造成延迟或断线，不能绕过 PostgreSQL 状态前置条件或产生双 Offer/Answer 提交；
- Fencing TTL、Ping、跨节点转发和关闭码的精确数值在批次 4 冻结；必须满足 `connection_expires_at = min(now + fencing_ttl, signaling_expires_at)`。

Guest 使用同一独立信令 WS，凭 `nli_guest` Access Token 建立 `REQUESTER` 连接；它不能连接 Phase 5 Instance WS。Guest Token 无 Refresh，绝对过期立即截断 Fencing TTL；过期后不能靠仍打开的 Socket继续发送，必须使用新 Invite/Guest/Join 流程。

### 幂等、公开 DTO 与错误

`POST` 使用公共 24 小时 Idempotency 规则，作用域为认证 Principal + Method + 规范化路由 + Key。相同 Key/相同请求返回首次结果；相同 Key/不同 Request 摘要返回 `409 IDEMPOTENCY_KEY_REUSED`；处理中返回公共可重试冲突。`DELETE` 天然幂等；Offer/Answer/ICE 的消息级幂等留给批次 2。

create/attach 的公开响应只包含：

```json
{
  "signaling_session_id": "uuid-v4",
  "join_request_id": "uuid-v4",
  "role": "REQUESTER",
  "offerer": "REQUESTER",
  "status": "ACTIVE",
  "channel": "/v2/signaling-sessions/{signaling_session_id}/ws",
  "created_at": 1730000000000,
  "expires_at": 1730000060000
}
```

`GET` 使用同一 DTO并可返回终态；DTO 不包含对端 Session/Account/Profile、ACL/Source 命中、Invite/Reservation、节点路由、connection ID、SDP、ICE、IP 或 TURN 数据。批次 2 另加协议 Phase 时只能增加响应字段，不改变本节角色和生命周期含义。

公开错误集合固定为：

| HTTP / code | 公开条件 |
|---|---|
| `400 INVALID_REQUEST` / 公共 Key 错误 | Body、Header 或 UUID 格式错误 |
| `401 UNAUTHORIZED` | Token 缺失、无效、过期、撤销或 Audience 错误；带 `WWW-Authenticate` |
| `404 JOIN_REQUEST_NOT_FOUND` | Request 不存在/不可见、调用方不是精确绑定参与方或使用错误 Session；这些分支正文一致 |
| `404 SIGNALING_SESSION_NOT_FOUND` | 会话不存在/不可见或调用方不是其精确绑定参与方；正文一致 |
| `409 SIGNALING_AUTHORIZATION_LOST` | 已确认参与方后，Target/Source/Account/关系/Proxy/ACL/绑定任一不再允许；不区分具体分支 |
| `409 SIGNALING_SESSION_STATE_CONFLICT` | 会话已终止、消息不适用于当前状态或同一 Request 禁止重建 |
| `409 IDEMPOTENCY_KEY_REUSED` | 同 Key 配合不同请求摘要 |
| `409 IDEMPOTENCY_IN_PROGRESS` | 同 Key/同请求仍在执行；响应带 `Retry-After` 或 `retry_after_ms`，客户端只能重试同一请求 |
| `410 SIGNALING_WINDOW_EXPIRED` | 已确认参与方的 Acceptance Lease、Guest 绝对期限或会话绝对期限已过；必须新建 Join Request |
| `429 RATE_LIMITED` | 超过 IP/Principal/Family/Guest/Request 多维限制；带 `Retry-After`，不暴露配额值 |
| `503 SIGNALING_UNAVAILABLE` | PostgreSQL、LeaseStore 或必要路由不可安全使用；带 `Retry-After` |

先验证 Token，再验证参与方绑定，之后才能返回可操作的 409/410；第三方猜测 Request/Session UUID 只能得到不可区分的 404。错误不得暴露对端是否在线、哪条 ACL、好友/Proxy/Invite 状态、Guest 精确剩余时间、节点或网络地址。`SIGNALING_SESSION_STATE_CONFLICT` 只用于 create/attach 和实时消息；能够从保留行确认参与方的 `DELETE` 对已经 `CLOSED/EXPIRED` 的会话仍返回 `204 No Content`，第三方 DELETE 仍返回固定 404。

创建、关闭、到期、授权撤销、连接替换和拒绝旧 Fencing 记录最小安全 Audit（资源 ID、角色、结果分类、时间和 Request ID）；不得记录消息 Payload。新增 Audit Type 由受控 Migration 注册，应用不能运行时创建任意类型。创建/attach 受 IP + Principal/Family/Guest + Join Request 多维限流，精确阈值在批次 4 与消息限流一并冻结。

### 批次 1 并发与故障走查

1. 双方同时首次 `POST`：Request 行锁与 `UNIQUE(join_request_id)` 只产生一个 Session；双方分别得到服务端推导的正确角色。
2. 创建提交后 Target Lease 立即丢失：下一次发送因 ONLINE 重验失败；协调 Worker 将会话终止，旧节点不能继续投递。
3. 操作恰好跨越 `lease_expires_at`：数据库 `now()` 和严格 `<` 决定过期；失败事务不投递半条消息。
4. 节点 A 分区、客户端连到节点 B：B 的 CAS 替换 Fencing；A 无法续租，A 的延迟帧与 Close 均因旧 `connection_id` 被拒绝。
5. LeaseStore 整体不可用：所有新握手和实时状态改变返回/收敛为暂时不可用，不回退到节点内存授权。
6. Guest Token 在打开的 Socket 中途过期：会话到期且连接失去有效 Fencing；Guest 不能用 Session ID 重连，必须重新走 Invite/Join。
7. ACL 在协商中改为 catch-all DENY：替换事务终止受影响 Session；即使终止事件丢失，下一帧的最新 ACL 重验仍拒绝。
8. Proxy Grant 或 Friendship 在协商中撤销：与 ACL 相同 fail closed；Invite 已消费则不因自身 EXHAUSTED 错误自撤销。
9. Sweeper 与最后一个合法操作竞争：双方按 Request → Signaling Session 顺序锁定；只有一个状态前置条件提交，重复 Sweeper 幂等。
10. 第三方猜测 UUID：Token 有效但不是保存的参与方时只得到固定 404，不能区分 Request/Session 是否存在或对端是否在线。
11. 旧 Token Family 与新 Instance Session 属于同一 Account：因不是 Request 保存的精确绑定而拒绝，不能接管原会话。
12. 通知丢失：双方可用已知 Join Request 调用 create-or-attach，或用已知 Session ID `GET`；无需通知 Replay。

批次 1 结论：没有 Lease 一次消费，没有同一 Request 下的 Session 重建，没有节点本地授权；暂时性路由故障只能重试，授权/期限/终态失败需要新 Join Request。

## 批次 2 冻结：Offer、Answer 与 Trickle ICE

### 权威状态与协议 Phase

PostgreSQL 继续分别保存生命周期和协议进度：

```text
status: ACTIVE | CLOSED | EXPIRED
protocol_phase: WAITING_FOR_OFFER | WAITING_FOR_ANSWER | EXCHANGING_CANDIDATES
epoch: 0..3
requester_ice_end_epoch: 0..3
target_ice_end_epoch: 0..3
```

- 新会话为 `ACTIVE + WAITING_FOR_OFFER + epoch=0`；
- `protocol_phase`、`epoch`、当前 Offer/Answer 的版本化 keyed digest、每角色 Candidate 计数、`ice_end` epoch、终止分类和时间是 PostgreSQL 权威；
- keyed digest 对客户端提交的 SDP **原始 UTF-8 字节**计算，不解析、不修剪、不规范化；它只用于安全重试判等，不能用于跨 Session 关联；
- SDP、Candidate、投递 ACK 和待投递 Payload 是短期易失状态；消息去重缓存同样可丢失，但批次 4 对 Candidate 使用截止即清理的最小 PG Receipt 防止重试重复计数。易失数据丢失允许重发和重复投递，不回滚 PostgreSQL Phase；
- `CONNECTING`、`CONNECTED` 和具体 ICE 成败只属于客户端本地观测。服务端不能验证 P2P 是否建立，因此不接受“已连接”声明，也不把它写成权威状态；
- 协商失败由参与方 `close`，授权失效由服务端 `CLOSED`，时间窗结束为 `EXPIRED`；不另设含义重叠的 `FAILED` 生命周期。

`candidate_exchange_complete` 是 `requester_ice_end_epoch == epoch && target_ice_end_epoch == epoch` 的只读派生值，只表示双方声明当前 epoch 不再发送 Candidate，不表示 P2P 已连接。

### 消息方向与 Envelope

信令 WS 使用协议版本 `1`。Access Token 仍只在 Upgrade 的 `Authorization` Header 中传递；首帧必须由服务端发送 `ready`，客户端不得在收到 `ready` 前发送业务帧。v2 信令客户端必须使用能够设置 Upgrade Header并主动发送标准 Ping 的原生/桌面/Mod WebSocket 栈；浏览器原生 JavaScript `WebSocket` API 不满足此能力，明确不受本版支持。不得为兼容浏览器把 Token 放入 URL Query、Fragment、`Sec-WebSocket-Protocol` 或首个未认证业务帧。未来浏览器支持需要独立的一次性握手授权设计并重新通过 Gate F。

客户端请求是 closed union，公共字段为：

```json
{
  "version": 1,
  "signaling_session_id": "uuid-v4",
  "message_id": "uuid-v4",
  "type": "offer",
  "epoch": 1,
  "payload": {}
}
```

- `signaling_session_id` 必须等于 Upgrade Path；它只是相关 ID；
- `message_id` 由客户端生成 UUIDv4，并在同一逻辑消息重发时保持不变；
- 顶层 `reply_to` 只在 `delivery_ack` 分支必填且为所确认服务端数据帧的 UUIDv4 `message_id`；其他客户端分支禁止该字段。`delivery_ack.payload` 仍是 closed empty object，关联 ID不塞进 Payload；
- 客户端不提交 `sender_role`、`created_at`、`expires_at` 或授权字段；Role 来自当前 Fencing 连接；
- `epoch` 对 `offer/answer/ice_restart/ice_candidate/ice_end/delivery_ack` 必填：初始 Offer 固定为 1；合法 `ice_restart` 固定为 PostgreSQL `current+1<=3`；Answer/Candidate/ice_end 必须等于 current；delivery_ack 必须等于被确认帧且仍为 current。`close` 不带 epoch；
- 未知请求字段、未知 `type`、版本非 1、类型错误或不符合对应 Payload Schema 的帧不投递。

服务端发送的 closed union 统一包含 `version`、`signaling_session_id`、服务端生成或保留的 `message_id`、`type`、`created_at` 和 `expires_at`。转发数据帧额外包含服务端推导的 `sender_role` 与 `epoch`；`result/error/delivery_ack` 分支在顶层必含 UUIDv4 `reply_to` 关联客户端 `message_id`，其他分支禁止该字段。时间只由服务端生成，不用于替代 Phase、epoch 或授权判断。

客户端到服务端类型：

```text
offer | answer | ice_candidate | ice_end | ice_restart | delivery_ack | close
```

服务端到客户端类型：

```text
ready | offer | answer | ice_candidate | ice_end | ice_restart
| result | delivery_ack | resend_required | error | session_closed
```

Payload 不得携带 Access/Refresh Token、Invite/Grant Secret、TURN Secret、Provider 凭据、MC Profile、内部 ACL/Source 命中、任意网络授权声明或自由文本错误。所有错误和关闭原因使用封闭枚举。

### Payload 与硬上限

| 类型 | Payload | 硬上限 |
|---|---|---|
| 完整 UTF-8 JSON Frame | — | 64 KiB；超限不解析 Payload，不回显内容 |
| `offer` / `answer` | `{ "sdp": string }` | SDP UTF-8 48 KiB；禁止 NUL；保留 CR/LF 和原始字节语义 |
| `ice_restart` | `{ "sdp": string }` | 与 Offer 相同；仅 REQUESTER，且只在已有 Answer 后合法 |
| `ice_candidate` | RTCIceCandidateInit 的 closed 子集：`candidate`、nullable `sdp_mid`、nullable `sdp_m_line_index`、nullable `username_fragment` | `candidate` 2048 B；其余字符串各 256 B；index `0..65535` |
| `ice_end` | closed empty object | 每角色每 epoch 一次权威效果 |
| `delivery_ack` | closed empty object；顶层必须有 `reply_to` 和被确认帧的 `epoch` | 只确认收到转发帧，不声明 SDP 已应用或 P2P 已连接 |
| `close` | `{ "reason": "USER_CLOSED" | "CLIENT_ERROR" | "NEGOTIATION_FAILED" }` | 无自由文本 |

每角色每 epoch 最多接受 64 个 Candidate；整个 Session 最多 3 个 epoch，即最多 1 个初始 Offer 和 2 次 ICE Restart。不同 epoch 分别计数。精确每秒速率、队列和带宽阈值在批次 4 冻结，但不能提高这里的协议硬上限。

服务端不从 Candidate 字符串提取、验证或记录公网/内网 IP 归属；只做 UTF-8、长度、NUL、字段类型和计数检查。候选是否能被 WebRTC 栈使用由接收客户端决定。

### 合法状态转换

每个将被接受的帧先验证当前 connection Fencing并执行批次 1 的完整 use-time 授权重验，再按 Request → Signaling Session 锁序转换 PostgreSQL，提交后才发送 `result` 和尝试投递。

| 当前 Phase | 消息与角色 | 前置条件 | 权威效果 |
|---|---|---|---|
| `WAITING_FOR_OFFER` | `offer` / REQUESTER | `epoch=1` | 保存 Offer keyed digest，epoch=1，清零本 epoch 计数/结束标志，转 `WAITING_FOR_ANSWER` |
| `WAITING_FOR_ANSWER` | 相同 `offer` / REQUESTER | epoch 与原始字节 digest 均相同 | 幂等成功，不改变 epoch/Phase，重新尝试投递 |
| `WAITING_FOR_ANSWER` | `answer` / TARGET | epoch=current，当前 epoch 尚无 Answer | 保存 Answer keyed digest，转 `EXCHANGING_CANDIDATES` |
| `EXCHANGING_CANDIDATES` | 相同 `offer`、`answer` 或已提交的 `ice_restart` / 原发送方 | epoch 与对应原始字节 digest 均相同 | 幂等成功，不改变 Phase，重新尝试投递 |
| `EXCHANGING_CANDIDATES` | `ice_restart` / REQUESTER | `epoch=current+1<=3` | 保存新 Offer digest，清除旧 Answer digest，初始化新 epoch 计数/结束标志，转 `WAITING_FOR_ANSWER` |
| `WAITING_FOR_ANSWER` | 相同 `ice_restart` / REQUESTER | epoch/digest 等于已经提交的新 Offer | 幂等成功并重投，不再次增加 epoch |
| `WAITING_FOR_ANSWER` | 新 `ice_candidate` / REQUESTER | epoch=current，REQUESTER 尚未 `ice_end`，Receipt 不存在 | 插入 Receipt、增加权威计数并投递，不改变 Phase |
| `EXCHANGING_CANDIDATES` | 新 `ice_candidate` / 任一方 | epoch=current，发送方尚未 `ice_end`，Receipt 不存在 | 插入 Receipt、增加权威计数并投递，不改变 Phase |
| 发送方已可发 Candidate 的 Phase | `ice_end` / 同一方 | epoch=current | 设置该角色 `ice_end_epoch`；重复为幂等成功 |
| 任意 `ACTIVE` | `close` / 任一方 | — | 转 `CLOSED`，记录 `CLOSED_BY_REQUESTER/TARGET` 与时间，删除易失 Payload |
| 任意终态 | `close` / 已确认参与方 | — | 幂等成功，不改首次终止分类 |

Candidate 入口的判定顺序固定为：先按 `(session_id, sender_role, epoch, message_id)` 查 Receipt；存在且 HMAC digest 相同则返回 `DUPLICATE` 并可重新投递，不增加计数，也不应用“尚未 ice_end”前置；存在但 digest 不同则 `PROTOCOL_VIOLATION`；Receipt 不存在时才检查 Phase、epoch、ice_end 和 64 条额度并插入。TARGET 在 Answer 提交前不能发送 Candidate 或 `ice_end`。不同内容的重复初始 Offer/Answer、用 `offer` 代替 `ice_restart`、TARGET 发 Offer/Restart、除合法 REQUESTER `ice_restart(current+1<=3)` 外的旧/未来 epoch、超过 epoch/Candidate 上限、`ice_end` 后新增 Candidate，均返回稳定 in-band error且不投递。Answer 与 Close、Restart 与 Close、双方 `ice_end` 并发都由 Signaling Session 行锁串行；事务失败不留下 Payload 投递。

### 消息幂等、顺序与 ACK

- `(signaling_session_id, role, message_id)` 在易失存储中去重到 `signaling_expires_at`；同 ID/同摘要返回原 `result`，同 ID/不同摘要是 `PROTOCOL_VIOLATION`；
- 去重缓存丢失后，Offer/Answer/Restart 仍由 PostgreSQL keyed digest 实现语义幂等，`ice_end/close` 由状态前置条件幂等；Candidate 由批次 4 最小 Receipt 防止重复计数，仍可能重复投递，因此接收方必须按 `(sender_role, epoch, message_id)` 去重。已经接受的 Candidate 可在发送方 ice_end 后以原 ID/内容重发，但不能新增 Candidate；
- `result` 只表示服务端已提交/接受或识别为重复，`status` 为 `ACCEPTED | DUPLICATE`；它不保证对端已收到；
- 对端收到可重发数据帧后发送 `delivery_ack`，服务端只在当前连接/Fencing 下转发给原发送方；ACK 是易失提示，不进入 PostgreSQL；
- 发送方在收到对端 ACK 前保留本地 Offer、Answer、Restart 和 Candidate，并使用相同 `message_id` 重发；服务端可以重复投递，不能保证全局顺序或 exactly-once；
- WebSocket 单连接内保持接收顺序，但跨节点转发、重连和重投允许乱序。接收方按 epoch 缓冲 Candidate，直到获得对应 Offer/Answer；旧 epoch 丢弃，未来 epoch 暂存上限一个且总量仍受 64 条约束；
- `ice_end` 只结束发送方当前 epoch 的新 Candidate；它可以先于部分已投递 Candidate 到达，对端必须等待自己的有界重排/重试窗口，不能把接收顺序当作权威；
- `delivery_ack` 丢失只导致安全重发，不改变协议 Phase。

### 握手、重连与易失数据丢失

成功握手和 Fencing CAS 后，服务端先发送：

```json
{
  "version": 1,
  "signaling_session_id": "uuid-v4",
  "message_id": "uuid-v4",
  "type": "ready",
  "created_at": 1730000000000,
  "expires_at": 1730000060000,
  "payload": {
    "role": "REQUESTER",
    "offerer": "REQUESTER",
    "status": "ACTIVE",
    "protocol_phase": "WAITING_FOR_ANSWER",
    "epoch": 1,
    "candidate_exchange_complete": false
  }
}
```

- 终态会话改发 `session_closed` 后关闭；
- 易失待投递 Payload 存在时可以重投；不存在时向应负责重发自己数据的当前连接发送 `resend_required`，其 closed Payload 固定为 `{ "epoch": 1, "required_from_self": ["OFFER"|"ANSWER"|"ICE_CANDIDATES"|"ICE_END"] }`。数组去重且按 SDP→Candidates→ICE_END 顺序，只请求接收者自己曾提交且当前 epoch仍合法的数据；不包含 digest、SDP、Candidate、对端缺失项或对端状态。没有当前连接时不为该 Role持久排队，Role下次 ready重新计算；
- 在 `WAITING_FOR_ANSWER`，REQUESTER 必须能重发当前 Offer/Restart，TARGET 必须等待收到后再生成 Answer；
- 在 `EXCHANGING_CANDIDATES`，REQUESTER 与 TARGET 都必须能以原 `message_id` 重发自己当前 epoch 的 SDP/Candidate；客户端丢失自身协商上下文时不能从服务端恢复敏感 Payload，只能关闭并重新发起 Join Request；
- 客户端超时后先 `GET` 权威状态并重连，再按 `ready/resend_required` 重发；禁止根据最后一条本地通知盲目创建第二会话；
- 节点崩溃发生在 PostgreSQL 提交与投递之间时，发送方因未收到对端 ACK而重发；digest/state 幂等保证不会双重推进。

### In-band Result、Error 与兼容性

`result`、`error` 和 `delivery_ack` 必须包含 `reply_to`；因此只有已解析出语法有效 UUIDv4 客户端 `message_id` 的拒绝才发送 `error` Envelope。无法解析 JSON/Envelope、无法取得合法 `message_id` 或重组体已超限时，不伪造关联 ID，直接按 1002/1009 尽力关闭；普通日志仍只记录固定分类。`error` 只包含稳定 `code`、固定英文 `detail` 和可选 `retry_after_ms`，不得回显 Payload。初始封闭 code：

```text
INVALID_FRAME
UNSUPPORTED_VERSION
UNKNOWN_MESSAGE_TYPE
PAYLOAD_TOO_LARGE
RATE_LIMITED
ROLE_NOT_ALLOWED
PHASE_CONFLICT
STALE_EPOCH
EPOCH_LIMIT_REACHED
CANDIDATE_LIMIT_REACHED
SIGNALING_AUTHORIZATION_LOST
SIGNALING_WINDOW_EXPIRED
SESSION_CLOSED
SIGNALING_UNAVAILABLE
PROTOCOL_VIOLATION
BACKPRESSURE
```

单个格式/Phase/旧 epoch 错误只拒绝该帧；版本不支持、伪造 Session ID、同 `message_id` 不同摘要、持续超限或无法维持 Fencing 属于连接级错误，发送尽力而为的 `error` 后使用批次 4 的专用信令关闭码。该关闭码段不得复用 Phase 5 的 4001–4003。

请求 DTO 拒绝未知字段。协议 v1 客户端忽略已知响应类型中的未知字段；收到未知服务端 `type` 或更高 `version` 时不执行 Payload，关闭信令连接并通过 HTTP `GET` 恢复。新增可选响应字段可保持 v1；新增类型、改变必填字段或语义必须提升协议版本。未知客户端类型永不透传。

### 终态读取与保留

`GET` 在参与方凭据仍有效时返回批次 1 DTO，并增加：

```text
protocol_phase
epoch
candidate_exchange_complete
terminal_classification (nullable)
terminal_at (nullable)
```

终态分类只允许 `CLOSED_BY_REQUESTER | CLOSED_BY_TARGET | CLOSED_BY_AUTHORIZATION | CLOSED_BY_POLICY | EXPIRED_BY_WINDOW`，不公开具体 ACL、Session、关系、网络或错误分支。Signaling Session 在终态后保留 24 小时，然后硬删除；Guest 访问仍受其更短的 Token 绝对期限限制。易失 Payload 在终态立即删除，版本化 keyed digest 随会话行删除，最小 Audit 按安全保留策略独立保存。

### 批次 2 协议与故障走查

1. 同 `message_id` Offer 重发：返回原结果、epoch 不变并重新尝试投递；同 ID 不同内容关闭连接。
2. 新 `message_id` 但 SDP 原始字节相同：keyed digest 判为重复，不增加 epoch；CR/LF 或空白变化视为不同内容并因 Phase 冲突拒绝。
3. 初始 Offer 提交后节点崩溃：Requester 未获对端 ACK，重连后重发；PG digest 使其幂等并恢复投递。
4. Answer 与 ICE Restart 竞争：Restart 只有进入 `EXCHANGING_CANDIDATES` 后合法；行锁决定 Answer 先提交，随后 Restart 建立新 epoch。
5. Restart 后旧 Answer/Candidate/`ice_end` 到达：以旧 epoch 拒绝，不投递。
6. Candidate 先于对应 SDP 到达接收端：接收端按 epoch 有界缓存；没有 SDP 时不能交给 WebRTC。
7. `ice_end` 先于先前 Candidate 到达：它只表示发送方不再生成新 Candidate，接收端等待有界重排窗口。
8. 双方同时 Close：第一次提交确定终止分类，第二次幂等成功；Join Request 历史仍为 ACCEPTED。
9. Close 与 Candidate 竞争：Session 行锁和提交后投递保证终态后 Candidate 不会被新接受；已在网络中的重复由客户端丢弃。
10. 易失 Store 全丢：PG Phase/epoch/digest/计数不回退；双方按 `ready/resend_required` 重发本地 Payload，允许重复、不产生双状态转换。
11. Guest 在协商中到期：下一帧转 `EXPIRED_BY_WINDOW`，删除 Payload并终止 Fencing；必须新 Invite/Join。
12. 客户端报告本地 `CONNECTED`：协议无该输入类型；NLI 不据此产生真实性或长期状态。

## 批次 3 冻结：STUN、TURN 与独立 Relay 授权

### 已选择的授权方向：独立 Relay 授权

用户已选择独立 Relay 授权，而不是普通 1 小时可重复 Allocate 的 TURN REST 凭据：

- 60 秒窗口内经完整 Join/Session/ACL 重验授予 Relay 使用权；窗口外不再允许凭原 Join 创建新的 Allocation；
- 已在窗口内建立的 Allocation 可以在独立、有限的数据面期限内维护；维护不等于重新批准 Join、不开放信令或 Guest Refresh；
- TURN 认证密码只证明对 Allocation 的控制权，不能单独决定新建或续期权限；授权器必须区分首次 Allocate、UDP 重传的原事务、既有 Allocation Refresh、Permission/Channel 维护；
- 独立授权必须由可信 TURN 授权/管理扩展实施，普通时间戳 HMAC Credential 不足以实现；
- 独立 Grant 初始最长 1 小时、不续签；一个角色最多 6 个同时存活的 Allocation，以支持地址族/传输尝试。下文定义维护和撤销的独立规则。

> 状态：`DESIGN_FROZEN`。独立复核产出的 Allocate 重传/响应、孤儿 Slot 清理、累计配额及兼容性边界问题已由主设计修正。此状态只代表设计冻结，不代表真实 TURN 实现通过测试。必须使用支持本节内部授权协议的 TURN Adapter；普通 coturn 配置不足以实现，生产启用仍受下方实现验收门槛约束。

### 标准与生命周期结论

v2 基线遵循 [RFC 8445 ICE](https://www.rfc-editor.org/rfc/rfc8445)、[RFC 8489 STUN](https://www.rfc-editor.org/rfc/rfc8489)、[RFC 8656 TURN](https://www.rfc-editor.org/rfc/rfc8656) 以及 [RFC 7064](https://www.rfc-editor.org/rfc/rfc7064)/[RFC 7065](https://www.rfc-editor.org/rfc/rfc7065) URI。`turn:...?transport=tcp` 表示客户端到 TURN Server 的 TCP 传输，不等于 RFC 6062 TCP Relay Allocation；本阶段 WebRTC Data Channel 仍使用标准 ICE/TURN UDP Relay Candidate，不为原始 Minecraft TCP 定义 TURN-TCP。

TURN Credential 用于认证 Allocate、Refresh、CreatePermission 和 ChannelBind 等控制请求；Allocation、Permission 和 Channel Binding 各有独立期限。RFC 8656 要求长期 Allocation 在到期前 Refresh，Permission 固定 300 秒。TURN REST 时间戳到期后既有 Allocation 的认证缓存/Refresh 行为取决于实现，不能仅凭 RFC 断言必然立即断流或可无限续期；必须针对选定版本验证。

三个期限互相独立：

- `credential_issue_deadline = signaling_expires_at`：只有 Signaling Session 仍 `ACTIVE` 且 Acceptance Lease 未过期时才能签发或安全重放 Secret；
- `allocation_create_deadline = signaling_expires_at`：新 Allocation 必须在该时间之前完成授权提交与本地激活；窗口外只有原 Allocation 的维护，不允许重建、迁移或换 5-tuple；
- `relay_grant_expires_at = issued_at + 3600s`：已激活 Allocation 的独立绝对上限，不因首次 Allocate、Refresh 或节点重连向后移动；
- Credential 不延长 Acceptance Lease、不允许继续发送信令或创建第二个 Signaling Session。TURN Adapter 按操作类型实施授权，禁止缓存一次认证成功后无条件放行所有操作；
- 到达 1 小时绝对期限后不续签、不滚动延长。仍需 Relay 的客户端必须取得新的 Join Request、Acceptance Lease 和 Signaling Session；直连 P2P 不受 TURN Grant 期限影响。

TURN Allocation 使用 RFC 默认 600 秒 LIFETIME 和标准 Refresh/Permission/ChannelBind 流程；独立授权器另有不可延长的绝对截止和最长 15 秒运行许可。绝对截止是管理面撤销，不把任意小于标准默认值的 LIFETIME 裁剪误称为 RFC 要求。TURN Adapter 逐包检查本地许可和绝对截止；到期丢弃数据并删除 Allocation，即使最近一次标准 Refresh 返回的 LIFETIME 尚未结束。客户端必须被告知 1 小时硬上限。

### ICE Server 获取入口

新增 Gate F 扩展入口，生产环境仅允许 HTTPS（TLS 1.2+，优先 TLS 1.3），公布的 API Origin 启用 HSTS；不得在明文 HTTP 接收 Bearer 或返回 Password，也不通过重定向转发带凭据的请求。开发明文只允许显式启用的本机回环测试，不构成部署基线。

```http
POST /v2/signaling-sessions/{signaling_session_id}/ice-servers
Idempotency-Key: <opaque>
Authorization: Bearer <bound token>
Cache-Control: no-store

{
  "requested_policy": "ALL"
}
```

`requested_policy` 只允许：

- `ALL`：收集 host、server-reflexive 和 relay Candidate，由 ICE 优先级选择；
- `RELAY`：只使用 relay Candidate，用于客户端隐私偏好或受限网络。

服务端可以因部署/隐私策略把 `ALL` 提升为 `RELAY`，不得把客户端请求的 `RELAY` 静默降级为 `ALL`。Body 不接受 Region、TURN Host、用户名、TTL、配额或角色；Region 和 Role 均由服务端选择/推导。

两方必须分别调用并获得不同 Credential。调用执行批次 1 的完整 use-time 重验，另外要求 Session 为 `ACTIVE`、当前 epoch 未超过 3、Region 有安全容量。Guest 使用其唯一绑定上下文；同账号、同 IP 或同 Signaling ID 不能替其他角色取 Credential。

可用 Relay 的响应为 closed DTO：

```json
{
  "ice_transport_policy": "ALL",
  "relay_status": "AVAILABLE",
  "region": "ap-east-1",
  "ice_servers": [
    {
      "urls": ["stun:stun.example.net:3478", "stuns:stun.example.net:5349"]
    },
    {
      "urls": [
        "turn:turn.example.net:3478?transport=udp",
        "turn:turn.example.net:3478?transport=tcp",
        "turns:turn.example.net:5349?transport=tcp"
      ],
      "username": "opaque-short-lived-username",
      "credential": "opaque-secret",
      "credential_type": "PASSWORD"
    }
  ],
  "credential_issue_deadline": 1730000060000,
  "allocation_create_deadline": 1730000060000,
  "relay_grant_expires_at": 1730003600000
}
```

成功返回 `200 OK`。`relay_status=UNAVAILABLE` 时 `ice_servers` 仅含 STUN，`allocation_create_deadline` 与 `relay_grant_expires_at` 显式为 null，不创建 Grant；以后恢复 Relay 必须在窗口内用新 Key 请求。有效策略为 `RELAY` 时响应不包含 STUN，客户端还必须设置 ICE transport policy=relay，过滤 SDP 内嵌及 Trickle 的非 relay Candidate，不能仅过滤单独 Candidate 帧。

响应必须带 `Cache-Control: no-store`、`Pragma: no-cache`、`Referrer-Policy: no-referrer`。STUN URL 不含 Credential；TURN Password、完整 Username 和响应 Body 不进入 Idempotency 普通明文、Outbox、Audit Detail、错误或日志。

`ALL` 且 Relay 临时不可用时可以返回显式 `relay_status=UNAVAILABLE` 的 STUN-only union，客户端可尝试直连并展示“Relay 不可用”的降级状态。`RELAY` 或服务端强制 Relay 时禁止该降级，返回 `503 TURN_UNAVAILABLE`。生产部署必须至少配置两个故障域的 TURN Endpoint；单节点开发环境可以只配置一个并明确标记非生产。

### Credential、Secret Replay 与 Region

每个 `(signaling_session_id, role)` 最多一个逻辑 TURN Grant。首次命令创建 Grant；同 Key/同请求在重新鉴权且未过签发窗口时返回相同 Secret；不同 Key 在签发窗口内 create-or-get 同一 Grant，不轮换 Secret或扩大期限。策略与已保存的请求策略不同返回 `TURN_GRANT_STATE_CONFLICT`；不能通过新 Key 改成更宽松策略。Secret Replay 只以版本化 AEAD 保留至 `credential_issue_deadline`；到达该期限时删除 Replay 密文并保留不可重放 Tombstone。窗口外即使原 Key 匹配也返回 `410 SIGNALING_WINDOW_EXPIRED`，不重签。这个截止只管 HTTP 签发/重放；TURN Password 的验证材料可在 `relay_grant_expires_at` 之前用于原 Allocation 维护，但每次操作还必须通过独立授权。泄漏/丢失后不存在延长 Lease 的恢复入口。

客户端使用标准 TURN long-term authentication 的 Username/Password，不引入浏览器必须理解的新 STUN 属性。Username 为随机不透明标识，Password 为独立 CSPRNG 256-bit Secret。支持时协商 SHA-256 Message Integrity；兼容 KDF 不改变独立授权检查。

服务端不采用时间戳 HMAC 作为授权替代品。短期 Replay Password AEAD 到期销毁后，TURN 维护只保留所需的 realm-bound Message Integrity 验证材料；这些材料具有等同凭据的敏感性，只在专用易失 Secret Store/Adapter 内存保留至 Grant 截止，不进入普通业务表或日志。Secret Store 丢失不得重新生成不同 Password 延续原 Grant，只能终止受影响 Relay。

`grant_id` 为随机 UUIDv4且不承载授权。Region 从服务端维护的健康容量、调用 IP 的粗粒度地理位置和数据驻留策略选择；不把精确位置写入 DTO。一个 Grant 可以在所选 Region 的两个故障域内尝试多个 Allocation，但额度跨节点共享。客户端不能获得任意第三方 TURN URL。

### Relay Authorizer 与 TURN Adapter 内部契约

此接口是部署内部 mTLS 服务，不是公共 REST API，也不接受 NLI Access Token 作为 TURN 协议密码。

PostgreSQL 保存 `relay_grants`（Session/角色/参与方绑定、策略、创建截止、绝对截止、ACTIVE/REVOKED/EXPIRED、revision、配额）及 `relay_allocation_slots`（随机 allocation_id、grant_id、node_id、boot_id、fence、PENDING/ACTIVE/CLOSED、激活时间、额度预留）。不保存原始 5-tuple、Peer 地址、SDP、Password。原始路由、Peer 集合、鉴权材料只存在易失 Store。锁序为既有领域锁 → Signaling Session → 配额维度行（由数据库时间确定完整 type/key/window 三元组后排序）→ Relay Grant → Allocation Slot → Runtime Permit；维护先无锁发现依赖，再按同序重读，不能从 Permit/Slot 反锁领域行。并发/存量桶不得因固定时间窗口或TTL归零，精确桶模式见 `data_model.md`。

| 内部操作 | 必须实施的检查/效果 |
|---|---|
| `prepare_allocate` | Adapter 已完成 STUN Message Integrity；Authorizer 按批次 1 完整复核当前 Session/Lease/Source/ACL，原子预留配额，创建 PENDING Slot；发出绑定 node/boot/fence 的单次激活许可，最长 2 秒且不越过创建截止 |
| `activate_allocation` | Adapter 先创建不转发的本地 Socket；授权器复核未撤销、许可/截止和配额后把 Slot 转 ACTIVE；提交后返回运行许可，Adapter 再验本地截止才开放转发并发送 Allocate success；失败不得公布 relayed address，compare-delete 本地 Socket并释放预留 |
| `renew_runtime_permit` | 只匹配 ACTIVE Slot 的原 node/boot/fence，复核独立 Grant、撤销和配额；每 5 秒申请，许可最长 15 秒且不超过绝对截止；不创建 Slot、不移动 5-tuple、不依赖已过期 Signaling Lease |
| `close_allocation` | 匹配原 fence，幂等关闭 Slot、销毁本地状态；延迟 Close 不作用于新 Slot |
| `revoke_grant` | 领域事务置 REVOKED、revision 增加并写最小 Outbox；推送尽力快速停止，下一次许可续期必拒绝 |

运行许可必须包含 `permit_id`、`request_nonce`、`grant_id`、`allocation_id`、`node_id`、`boot_id`、`fence`、`grant_revision`、递增 `permit_sequence`、`issued_at`、`not_after` 和有限 byte-credit 标识。许可通过内部认证通道取得，只能在绑定的进程/Slot 使用；不暴露给客户端。Adapter 不接受旧序号覆盖新状态，不接受已观察撤销 revision 之前的许可。

`not_after` 在 Authorizer 持有 Grant 行锁并作最后授权决定时计算，不以响应发送或接收时刻计算。事务提交/网络延迟消耗许可期限；Adapter 不能在迟收到许可时重新启动完整 15 秒计时。续期请求的本地起始单调时间也限制最晚停止时刻；同一 nonce 重试只能重取原许可和原额度，不能增加期限或 byte-credit。首次激活另外必须在 `allocation_create_deadline` 之前，即使运行许可尚未到期也不能晚激活。

普通 UDP Allocate 重传首先按原 STUN transaction、原 5-tuple 和 node/boot 查找同一事务。PENDING 时合并到原 Prepare/Activate 操作，不另建 Slot、不再次预留，亦不提前返回成功地址；ACTIVE 且本地 Allocation 存在时返回原成功结果。同一 Grant/node/boot/five-tuple 即使使用新的 transaction ID也不得建立第二个 live Slot；Adapter 按所选 TURN 标准返回既有 Allocation 的合法结果或 Allocation Mismatch，Authorizer 的 partial unique 只作禁止双计费的最终兜底。其他 Grant 已占用同一 Adapter live five-tuple 时必须在 Prepare 前拒绝。失败转 CLOSED 时返回标准 TURN 错误或因节点死亡超时，由客户端启动新尝试；不能把不存在的地址当成功重放。重传不会再次计费，也不会延长任何期限。原 Slot/Socket 已消失，哪怕 transaction ID 相同也不是重放成功；必须按新建处理，创建窗口外拒绝。TCP 重连、新源端口、NAT rebinding、备用节点接管及 ICE 新 Allocation 都是新建，不能冒充 Refresh。相同调用的内部幂等记录与 Slot 绑定；提交不确定先查询原操作，不重复分配。

窗口内允许 CreatePermission/ChannelBind 建立 Peer 集合；必须完成当前路径的 Peer 安装。创建截止后只允许对已存在于该 Allocation 的 Peer IP 集合续 Permission、对同一 Peer endpoint 维护 Channel；不允许增加新 Peer 或改变端口，从而不能把旧 Allocation 变成新的代理出口。集合和 endpoint Pin 在 Allocation 剩余生命内保留于易失状态，即使标准 Permission Timer 暂时失效；丢失 Pin 状态则终止，不从后续请求猜测恢复。Refresh(0)/删除只在通过原 Allocation 身份验证后始终允许，不能无鉴权销毁他人 Allocation；发送数据必须同时满足标准 TURN Permission、Pin 和运行许可。

CreatePermission 只确定 IP，不自动批准该 IP 的任意端口。窗口内，成功的 ChannelBind 或原客户端绑定传输上的 Send indication 可把其目标 endpoint 登记到有界 Pin 集合；来自 Peer 的入站包不能新增 Pin。每 Allocation 最多 32 个 Peer IP、64 个 endpoint；IPv4-mapped IPv6 先规范化再检查出口和计数。窗口结束时冻结集合，未出现过的 endpoint 一律拒绝。只安装 Permission 却没有及时确定目标端口的路径可能失败，客户端必须在窗口内完成该步骤；不能以此为由放宽截止后建连。

TURN 出口在 IPv4/IPv6 统一禁止 loopback、private、link-local、multicast、未指定地址、云 metadata 与管理网段；RELAY 双方互通需要显式允许本服务的公网 Relay 端口段，不允许通过域名/DNS绕过 IP egress 检查。地址检查是 TURN 出口安全策略，不把客户端 Candidate 声明升级为身份认证。

### 维护、撤销与故障边界

- Signaling Lease 到期、Guest 5 分钟自然到期、Access Token 自然到期/正常 Refresh 轮换、通知 WS 临时 OFFLINE，不单独撤销已激活的独立 Relay；否则匿名长时 Relay 无法成立。它们仍禁止新的信令/HTTP操作；不存在 Guest Refresh 或新 Principal。
- Signaling `DELETE/close`、Instance 显式 CLOSED、Account 非 ACTIVE、Token Family 显式撤销、Guest ban、ACL 对保存 Context 变为 DENY、依赖 Friendship/Proxy 被撤销、Grant 超额或到期，撤销相关 Grant。评估继续使用原 Request Profile 快照，不要求 Profile 当前值一致。
- Signaling 自然 EXPIRED 不等同主动 Close；Grant 引用必要的最小参与方/Source 上下文以支持独立撤销，不能靠 Session.status=ACTIVE 判断维护许可。Instance OFFLINE 宽限结束后按冻结领域规则实际转 CLOSED 时仍撤销 Grant；并不承诺无限期离线维持 Relay。
- Guest 清理必须区分自然到期与显式 ban。Grant 存活期间保留最小 guest_id、参与方绑定、Request Context 和显式撤销标记，不保留可继续用于 HTTP 的 Guest Token。清理 Worker 不得先删依赖或把“记录缺失”解释为授权有效；上下文无法验证时停止续许可。
- 续运行许可在领域依赖锁和 Grant 锁下重新检查独立撤销条件；领域变更事务与续许可串行。关联撤销 Outbox 可丢失，但不能凭仅缓存的 Grant.status 忽略已提交的 ACL/Family/Instance 变化。
- 撤销推送丢失或网络分区时，已有运行许可最多继续 15 秒；新授权请求立即拒绝。此为明确的最大授权滞后，不声称跨网络瞬时停止。已在网络中的包无法回收，直连 P2P 不受 TURN 管理面控制。
- Authorizer/Adapter 必须维持已监测的最大相对时钟不确定度 2 秒；超过即拒绝新建/续许可。Adapter 按 `not_after - 2s` 提前截止，同时受本次请求开始的单调时刻 + 15 秒限制，取更早者；绝对 Grant 截止也扣除该误差。回拨、暂停恢复或无法安全换算时 fail closed。许可 revision/sequence 校验与 Grant 行锁共同防止撤销后续期。
- PostgreSQL/Authorizer/Secret Store 不可达：不能签发新的运行许可；一次续期 RPC 失败时，Adapter 可在原许可内按 250ms 起、最高 1 秒带抖动退避重试，不立即销毁仍有效连接，也不延长原 not_after。原许可或 byte-credit 耗尽即停止数据面，即使标准 TURN LIFETIME 尚长也不能继续转发。
- Adapter 重启使 boot_id 改变，旧 Allocation 不复活；窗口内可重新申请，窗口外只能新 Join。没有跨节点透明 Allocation 迁移承诺。
- ACTIVE Slot 配额不能只因 Redis TTL 消失而归零；PG 预留在旧运行许可安全到期和清理确认后才释放。带宽字节额度由 Authorizer 原子预扣为有限 byte-credit，节点实际发送消耗；未确认余额宁可损失额度，不能重复发放。没有额度的包不转发。

运行许可与 byte-credit 的发放在同一授权事务记录。响应包含唯一 `credit_id`、所归属 allocation_id/fence、预扣字节数；不是可反复重置的“余额”。Adapter 对 credit_id 只加载一次，双向实际转发的 Relay Payload 字节均扣减（不计内部 RPC/重复计量报告），续许可提交单调累计消耗水位。重复 nonce/响应不能再次装载额度，报告回退拒绝；未用额度只在经验证的安全关闭后返还，崩溃未确认部分不返还。

Orphan Reaper 每 5 秒扫描有界批次。PG Slot 必须保存已经发出的最大 `permit_not_after` 和 PENDING 激活许可截止。Reaper 先无锁发现依赖、按既有锁序重读：无成功激活的 PENDING 超过激活截止 + 5 秒转 CLOSED；ACTIVE 超过最后许可 `permit_not_after + 5s` 且没有更新许可时转 CLOSED并使 fence 失效。任意更晚续期被拒绝，不以节点心跳活跃为理由复活。并发续许可与 Reaper 锁同一 Slot，只有一方可提交。节点失联时健康数据库下最迟在最后许可决定后约 25 秒回收并发 Slot 额度，而不是等 1 小时；数据库不可用期间不承诺回收时延，但禁止新分配。数据面最迟在原许可截止停止，回收额度不会与旧节点有效许可重叠；累计次数和未确认 byte-credit 不随回收重置。

一个小时是本版独立授权的显式产品上限，不是 TURN 标准限制；改变它需要重新评审 Relay 配额/滥用设计，不得滚动续签原 Grant。

### NAT 与传输策略

- `ALL` 同时收集 host、STUN server-reflexive 和 TURN relay Candidate，不先做可枚举的“NAT 类型 API”；
- UDP/TCP/TLS 收集在剩余窗口内并行或短交错启动，不能等待 UDP 黑洞完整超时后才串行开始 TCP/TLS。优先级偏好 `turn/UDP 3478`，同时准备 `turn/TCP 3478` 与 `turns/TCP 5349`；部署可额外在 443 提供 `turns`，但必须是有效证书域名。最终选择由 ICE 完成，未选中 Allocation 应主动删除；
- 支持 IPv4/IPv6，客户端在可用平台使用 mDNS host Candidate，减少局域网地址直接暴露；
- `RELAY` 不发送 host/server-reflexive Candidate 给对端，但 TURN 和 NLI 仍能看到必要网络元数据，因此不是匿名网络；
- STUN 失败不等于授权失败；ICE 继续尝试其他 Endpoint。只有所有允许的 Relay Endpoint 都不可用时才产生 TURN 降级/错误；
- Client-to-TURN 的 TCP/TLS 只是 UDP 被封锁时的传输回退，不把应用层原始 TCP 直接交给 TURN。

### 初始配额与滥用边界

硬配额按最先命中者拒绝，响应不公开其他用户或 Region 的当前占用：

| 维度 | 初始上限 |
|---|---|
| 每 Session/Role 逻辑 Grant | 1 |
| 每 Grant 同时 / 累计创建 Allocation | 6 / 24；同时数包含 PENDING 预留，累计仅计首次进入 ACTIVE 的 Slot；PENDING→CLOSED 不计累计，ACTIVE 删除不返还累计 |
| 每 Signaling Session 总同时 Allocation | 12 |
| 每 Guest 同时 Allocation | 6 |
| 每 Requester Source Instance 活动 Grant | 3 |
| 每 Target Instance 活动 Grant | 32（计入双方角色 Grant） |
| 每 Account 活动 Grant | 20（按 Credential 所属方计） |
| 每规范化源 IP 同时 Allocation | 48 |
| 每 Allocation 合计持续带宽 | 8 Mbit/s，允许 10 秒 16 Mbit/s burst |
| 每 Grant 总 Relay 流量 | 2 GiB 或 1 小时，先到者终止 |
| Credential Endpoint | 每 Session/Role 3 次/分钟；每 Account 30 次/分钟；每 Guest/IP 10 次/分钟 |

TURN 集群还必须有 Region 级总 Allocation、带宽和出口费用上限；达到软阈值停止新 Grant，既有 Allocation 优先；达到安全硬阈值时按明确策略终止最新/滥用 Allocation并记录审计，不能随机影响未超个人配额的用户。硬额度使用 PostgreSQL 原子预留和有限 byte-credit；易失 Store 只作速率计数与路由。额度不可安全判定时拒绝新 Grant/Allocation，不退化为节点本地无限额度。

### 错误、撤销与隐私

| HTTP / code | 语义 |
|---|---|
| `404 SIGNALING_SESSION_NOT_FOUND` | 不存在、不可见、错误绑定，正文一致 |
| `409 SIGNALING_AUTHORIZATION_LOST` | 已确认参与方后，ACL/Source/Instance/Session 不再允许；不细分 |
| `409 TURN_GRANT_STATE_CONFLICT` | Grant 已终止或请求策略与既有 Grant 冲突 |
| `410 SIGNALING_WINDOW_EXPIRED` | 已超过签发窗口；必须新 Join |
| `429 TURN_QUOTA_EXCEEDED` | 个人/Session/IP 配额或速率；带 `Retry-After`，不暴露数值来源 |
| `503 TURN_UNAVAILABLE` | 请求的 Relay 策略无健康容量；带 `Retry-After` |

撤销采用上述独立 Grant + 最长 15 秒运行许可，而不是普通 TURN REST 最坏 1 小时的尽力撤销。公共 HTTP 错误复用批次 1 的认证、结构与幂等错误，表中仅为 TURN 业务补充。内部 Authorizer 不向客户端暴露数据库约束、节点或具体撤销原因；STUN 协议错误由 Adapter 映射为标准错误，HTTP Problem 不直接塞入 STUN。

普通应用日志和 HTTP Access Log 必须在序列化前遮蔽 `credential`、完整 TURN Username、SDP、Candidate、Peer IP 和 Relay Address。安全审计只记录 grant_id、session_id、role、region、签发/拒绝分类、粗粒度字节桶和时间；TURN 原始 Payload/Password 日志禁止开启；确有运维需求的最小网络元数据诊断须显式启用、隔离存储和受限访问，最长 24 小时，不进入通用日志平台。聚合容量指标不得带 Account、Guest、完整 IP 或 Username。

### 批次 3 场景走查

1. 双方并发取 Credential：各自 Role Key 只产生一个 Grant和一个不同 Secret，不互相覆盖。
2. Guest 猜 Target 的 Grant ID：没有绑定 Guest Token和 Signaling 上下文只得到固定 404，Grant ID 不能认证 TURN API。
3. HTTP 响应丢失：同 Idempotency Key 在签发窗口内解密返回相同 Secret，不产生第二个 Username。
4. Lease 在签发事务中到期：数据库严格 `<` 复核使整个签发回滚，不留下可用 Secret。
5. Credential 已签发后 Signaling Lease 到期：不能再取 Secret或发信令；已建立 Relay 最多继续到 1 小时 Grant 截止，不构成 Lease 延长。
6. TURN/UDP 被封：窗口内并行/短交错的 TCP/TLS 尝试仍可建立，不等待 UDP 超时；过窗全部失败则新 Join，不切换到未授权第三方 Relay。
7. Region Relay 全部故障：`ALL` 得到显式 STUN-only 降级，`RELAY` 得到 503；客户端不会误以为隐私模式仍生效。
8. Allocation Refresh 接近 1 小时：即使协议返回标准 LIFETIME，Adapter 仍在 Grant 绝对截止停止逐包转发，不能借 Refresh越界。
9. Authorizer/Secret Store 丢失：拒绝新 Grant/Allocation/续许可；现有 Relay 在已有最长 15 秒运行许可结束时停止，PG 配额不因 Redis 清空而释放。
10. 凭据出现在异常/日志字段：结构化 Redaction 在序列化前移除；安全 Audit 只保留 Grant ID 与分类。
11. Relay 数据达到 Grant 合计 2 GiB：撤销该 Grant 的全部 Allocation，不影响其他 Role 的 Grant；Allocation 带宽超限先按桶限速，持续滥用按策略撤销。客户端不得删除旧 Slot 再重建以重置总字节额度。
12. Signaling 主动 Close/ACL 后续撤销：信令立即 fail closed，TURN 最迟在已有运行许可到期停止；Signaling/Guest 自然到期只停止新授权，不撤销独立维护。
13. 截止后用旧 Credential 在另一节点或新 5-tuple Allocate：拒绝，即使 Password 正确且 Grant 尚未满 1 小时。
14. 窗口内 Prepare、窗口后 Activate：拒绝激活，销毁未转发 Socket并释放预留。
15. 截止后旧事务重传：仅原 Slot/Socket 存在时重放原结果；节点已重启不得恢复。
16. 截止后向新 Peer CreatePermission：拒绝；原 Peer 续 Permission 可继续，丢失 Peer Pin 则 fail closed。
17. 节点崩溃后剩余 byte-credit 未确认：不重复返还并发放；只能少用，不能超过 Grant 总额。
18. 授权成功响应延迟 20 秒后到达：原 not_after 已过，Adapter 拒绝；重试不能重启许可计时或再次领取原额度。
19. 收到撤销推送后又收到旧续期响应：按 revision/sequence 拒绝，不能恢复已关闭数据面。
20. Guest 自然清理与 Relay 续许可竞争：最小撤销上下文保留；自然到期不等于 ban，缺失上下文则 fail closed。
21. Prepare 后客户端在 500ms 重传 Allocate：命中 PENDING 原事务，不重复预留，只有激活提交后发送 success。
22. Adapter 崩溃且不回 Close：健康 Reaper 在最后许可结束 + 宽限后关闭 Slot并释放并发额度；旧 boot/fence 永不复活。
23. 6 路初始候选加两次 Restart 后发生窗口内网络重试：24 次累计允许有限额外尝试；未激活项不消耗累计，但受并发/请求速率限制。

### 实现验收门槛

Gate F 可冻结设计，但生产 Relay 功能默认关闭，直到选定 TURN Adapter 通过真实 WebRTC 栈测试：跨 60 秒与 Guest 到期持续维护、窗口外新建拒绝、多传输/IPv4/IPv6 候选、旧事务重传、截止并发激活、Peer Pin、15 秒分区停止、1 小时强制停止、配额并发/计费及 Secret Store 丢失。只存在配置样例、RFC 引用或 Mock 成功不能宣称兼容性已验证。

## 批次 4A 冻结：多节点信令、断线与背压

> 状态：`DESIGN_FROZEN`。独立复核的两个协议阻塞项和六个澄清项均已修正。本节只细化信令 WS，不把独立 Relay Allocation 的寿命绑定到信令 Socket；Relay 撤销仍由批次 3 的独立 Authorizer 处理。

### 连接租约与 Token

- 每角色一个 current connection，客户端每 5 秒发送标准 WS Ping，服务端 Pong；信令 `connection_expires_at = min(shared_now + 20s, signaling_expires_at)`。任意业务帧不代替 Ping，服务端不得通过本地定时器无条件续租。固定 20 秒 Lease 在最后一次成功续租后提供三个后续 Ping 机会，但移动网络黑洞仍可能断线；客户端必须按 4407 恢复，不能依赖 Socket 永久在线。
- 续租必须 compare-current `connection_id + node_id + boot_id`，同时重验 Account/Family/Guest/Instance 与 Signaling Session。旧 Socket 的 Ping、Pong、Close 不得续租或删除新连接。
- 新握手必须使用当前有效的 Access Token。已经通过握手的 NLI Socket 按绑定 Family 维持认证；Access 自然到期或同 Family 正常 Refresh 不单独中断，但撤销 Family、解绑 Instance、Guest 绝对到期立即阻止继续操作。不得在 WS 帧里更新 Token，变更 Family 必须新 Join。
- 信令 Ping 不能续期 Phase 5 Instance Lease；即使信令 WS 活跃，Source/Target 的通知 WS Lease 丢失仍导致信令 use-time 授权失败。
- 所有节点使用 LeaseStore 原子时间判定连接期限；转换为本地单调计时器时扣除网络延迟与时钟误差。Store/PG 瞬时不可达时当前续租失败并停止接受新的业务/投递，但不冒充显式撤销或立即发送 4403；Socket 可保留到既有 connection_expires_at 以等待后续 Ping 重试，期限到达后以 4407/物理关闭结束。显式撤销立即失去业务权限并使用 4403。

### 路由和投递的线性化边界

LeaseStore 使用一个 session-scoped Route Document，而不是两个可独立读取的 Key：`session_id -> route_revision + REQUESTER slot + TARGET slot`；每个 Slot 含 node_id、boot_id、connection_id、expires_at。任一连接 replace/renew/compare-delete 都原子修改/检查对应 Slot并递增或核对 route_revision；不建立可持久重放 SDP 的消息队列。节点间使用相互认证的加密 RPC，调用方不是客户端，传输内容只在内存或短期易失缓存中存在。

1. 入口先做 Frame/大小/限流检查，再按批次 1 授权和锁序检查当前发送方连接。
2. Offer/Answer 等权威转换提交 PostgreSQL 后才返回 `result`；事务内不进行慢速网络发送。提交不确定时客户端按同一 ID/epoch/digest 重试，不自行递增 epoch。
3. 节点间传递有界内部 Envelope：session_id、sender role、发送连接 fence、接收连接 fence、epoch、原 message_id、绝对过期、Payload。接收节点不能信任客户端提供的这些字段。
4. 接收节点入队前开启独立短授权事务，按全局锁序锁定当前领域依赖和 Session，确认 epoch/授权/期限；持锁对单一 Route Document 执行一次不可拆分的原子 check-both，同时比较 route_revision 及两个角色的 node/boot/connection/有效期，并与 Connection replace 共享同一存储序列化点。禁止按角色分两次 GET/CAS；任何 Slot 或 revision 在操作中变化都使整体失败。检查通过才提交一次投递许可，事务失败不得入队。该短协调沿用 Phase 5 跨存储例外，不在锁内发送 Payload。许可只用于此消息/此接收 Socket的一次即时入队，最长 250ms并受 Session/连接截止限制；实际写入 Socket 前必须再次原子比较接收方 `(route_revision, connection_id, boot_id)` 当前性并检查期限，任一变化即丢弃而非写旧 Socket。超时重新授权而非延长许可。断线期间没有当前接收连接时不伪造投递 ACK。
5. 该 check-both 是本次投递授权与连接替换的排序点；领域撤销由数据库依赖锁排序。排序点后仍可能发生撤销/连接替换，存在最长 250ms 的已授权本地入队尾部，已经进入 OS/网络的字节更无法回收。客户端按当前 Socket、Session、epoch 和终态丢弃旧数据；不能声称撤销后对端绝不会收到旧包。承诺是撤销后不再批准新的状态转换或新的投递授权，不保证排空已经批准的网络数据。
6. 目标节点拒绝旧 route 时，入口最多重新查一次路由并尝试一次；仍失败则停止，靠客户端重连/重发。不允许 RPC 节点之间无限相互转发。

通知 EventBus 不参与数据投递授权。EventBus 停止只影响提示；LeaseStore/权威数据库/必要的节点 RPC 不可用时信令 fail closed。TURN 数据面则遵循其独立短运行许可，不能把它混同为信令投递缓存。

### 易失缓存、重发与去重

- 服务端最多缓存当前 epoch 的双方 SDP及有界 Candidate，缓存截止不晚于 `signaling_expires_at`；新 epoch 或终态立即清除旧 Payload。不得把 Payload 写入 PostgreSQL、持久队列、磁盘 spool 或 Crash Dump。
- `result` 不是 Peer ACK；对端 ACK 也只表示收到，不表示已执行 `setRemoteDescription/addIceCandidate`。客户端在当前 epoch 内保留可恢复的本地 SDP/Candidate，不能收到 ACK 就删除唯一副本，否则对端重连丢失上下文时无法恢复。
- Peer ACK 缺失时采用 500ms、1s、2s、之后每 2s 的带抖动重发，按最旧未 ACK 帧优先，受共同限流与绝对截止约束。每角色最多 8 个**在途未 ACK** 数据帧；已 ACK但为 `resend_required` 保留的本地副本不占该窗口。窗口满只阻塞新数据，不阻塞 ACK/error/close/Ping，避免双向互等和旧帧饥饿。
- 重连先取得 `ready` 快照，恢复顺序为本角色当前 SDP → Candidate → ice_end。未来 epoch Candidate 最多缓存一个 epoch、64 条或 128 KiB，先到者为限，最多等待 2 秒；仍无对应 SDP 时丢弃本地缓冲并请求/等待安全重发，不把它交给 WebRTC。
- 相同 message_id 的去重表必须保留 Payload 摘要与原始关联信息，不能把不同 Sender 或 Session 的同 ID 当作同一消息。ACK 只允许相反角色确认当前 epoch 曾接收的数据 ID，不能 ACK result/ACK 本身。
- 为避免重试消耗 64 条新 Candidate 额度，采用 PostgreSQL 最小 `signaling_candidate_receipts`，唯一键 `(session_id, sender_role, epoch, message_id)`；`sender_role` 始终是消耗配额的发送方。仅保存版本化 Session-scoped HMAC-SHA-256 Payload digest及接受时间，不保存 Candidate/地址。首次接受在同一 Session 锁下插入 Receipt并增加计数；同 ID/同 digest 重试不增加计数，仍须完整授权/Fencing 重验才能再次投递；同 ID/不同 digest 为 PROTOCOL_VIOLATION。新 ID 即使内容相同也算新 Candidate，因此仍受上限约束。
- Receipt 保留到 Session 终态或绝对窗口结束，不随 connection replace 或易失缓存丢失删除；新 epoch 后旧 Receipt 不再允许投递，但仍可检测旧 ID 重用。每 Session 最多 384 条（2 角色 × 3 epoch × 64）；到期逻辑上立即不可授权，Worker 每秒有界删除，物理积压告警不延长有效期。Key 材料只在专用 Secret Store保留；丢失则安全终止，不绕过判等。
- 这是对批次 2“所有消息去重集合易失”的受限细化：仅 Candidate 最小 Receipt 持久到协商截止，其他易失去重规则不变，敏感 Payload 仍不持久化。恢复只保证可从客户端副本重发，不保证客户端丢失 Payload 后可恢复。

### 背压和速率（候选初始值）

| 对象 | 候选上限/处理 |
|---|---|
| 每连接完整重组消息 | 64 KiB；拒绝二进制业务消息，禁用 permessage-deflate，限制碎片总量，不能仅限单片 |
| 每 Socket 出站队列 | 128 帧或 512 KiB，先到为限；其中 8 帧/8 KiB 留给 control；Close 绝对优先，其次 error、ACK/Pong，业务帧不得占用预留 |
| 每 Session 共享易失 Payload 缓存 | 512 KiB；逻辑缓存条目以 session/role/epoch/message 唯一定位。节点本地队列独立按上述队列上限计数，不声称包含网络瞬态副本的全球 512 KiB RSS 上限 |
| 每连接数据帧 | 20/s、burst 40，含重发；另限 128 KiB/s、burst 256 KiB |
| 每连接 ACK/control | 40/s、burst 80；标准 Ping 至多 2/s |
| 每 Session/Role 握手 | 6/分钟、burst 2；另受 Account/Guest/IP 聚合限制 |
| 每 Account / Guest / IP 握手 | 30 / 6 / 60 每分钟；代理出口仅信任已配置反向代理的 IP 信息 |

超限拒绝当前帧并返回固定 code 与 retry_after_ms；连续 3 次在 10 秒内违反同一限流规则则关闭连接，不主动关闭逻辑 Session。出站队列已满时不通过丢掉随机 SDP/Close 继续假装正常；停止业务入队，尝试使用控制预留发送 BACKPRESSURE 后关闭 Socket。关闭帧发不出去也必须物理关闭并 compare-delete 自己的 fence。

### 信令 WS 关闭码（独立于 Phase 5）

| Code | 含义 | 客户端动作 |
|---|---|---|
| 1000 | 客户端正常关闭或 Session 主动终止 | GET 确认，不盲目重连终态 |
| 1002 | 非法 WS 协议/不支持的信令版本 | 不自动重试相同协议 |
| 1009 | 重组 Frame 过大 | 修正输入；不原样重试 |
| 4401 | 当前连接被替换 | 旧连接停止；不自动抢回以形成替换风暴 |
| 4402 | 协商窗口结束 | 新 Join |
| 4403 | 授权丢失 | GET 当前可见状态；不得仅重发 Secret |
| 4404 | Session 已终止 | GET，若需重连则新 Join |
| 4405 | 持续限流/协议策略违规 | 服务端可写 Socket 时先发 `RATE_LIMITED` error及 retry_after_ms；否则默认带抖动退避 500ms–2s并受窗口截止约束。同 Session/Role 连续 3 次 4405 后停止自动重连并要求修正客户端 |
| 4406 | 背压 | 在仍有效窗口内 GET/重连/重发 |
| 4407 | 节点重启、依赖故障、连接租约超时 | 带抖动退避 250ms 起，最高 2s，且不超过窗口 |

未经 Upgrade 的失败仍使用 HTTP Problem。Close reason 最多固定安全分类，不带 IP、Username、Secret、SQL 或对端隐藏状态。客户端无法获知关闭码时同样先 GET，不能把异常断线解释为 Host 拒绝。

### 清理与恢复测试要求

连接 compare-delete 只针对自身 fence；Session EXPIRED/CLOSED 后清理双方 route、缓存和去重状态。数据库/Store 可用时 Worker 每秒扫描有界到期批次，实际操作不能等待 Worker 才检查期限。PG Session 最小终态保留遵循批次 2 的 24 小时；Relay Grant 及 Slot 在尚有运行许可/字节额度时不随 Session 缓存清理，直到其独立期限与安全回收结束。

必须覆盖：断线后新连接抢占、旧 Close 晚到、提交后节点崩溃、RPC 重复/乱序、单原子 check-both 中间发生 replace 必须整体失败、写 Socket 前 current fence 二次检查、route 变更重试上限、接收队列耗尽且 Close 优先、延迟 ACK、双方待 ACK 窗口满且 oldest-first、Ping 依赖瞬时故障只让旧 Lease 自然跑完、Ping 不续 Instance Lease、ice_end 后同 ID/同 digest Candidate 返回 DUPLICATE 可重投、易失去重丢失但 Candidate Receipt 不重复扣额度、Receipt Key 丢失安全终止、Session 到期而 Relay 独立维护、过期清理与最后一次投递竞争。

## 隐私、日志、审计与滥用（#105，`DESIGN_FROZEN`）

> 独立安全复核结论为无阻塞项；撤销/Audit 故障方向、break-glass 权限与事件、删除锚点、TURN 禁止网段、补写时限和 Digest Key 生命周期已按全部 Warning 修正。运行时 Redaction、TURN egress 与部署检查仍须实现验收，不因文档冻结视为已测试。

### 匿名并不等于不可追踪或已验证身份

Guest 只证明持有受限的服务凭据并获得本次 Join 授权，不证明现实身份或 Minecraft 身份。MC Profile 始终 CLIENT_CLAIMED；握手成功、DTLS 加密和 TURN Message Integrity 都不能提升这一保证。

基础数据面要求使用正确校验对端 SDP fingerprint 的 WebRTC DTLS/SCTP Data Channel；服务端不终止游戏数据的 DTLS 会话。直连与 TURN 路径都不能使用未加密的原始游戏数据替代该通道。TURN/TLS 只保护客户端到 Relay 的一段链路，不单独构成双方端到端加密。

基础模式信任 NLI 提供的信令完整性。完全控制信令的攻击者可能替换双方 fingerprint并建立两个连接，故产品只能标记为 `TRANSPORT_ENCRYPTED`，不能标记为“服务器不可见的已认证 E2E”。此前讨论的外带 E2E Secret/Noise 是未来可选 `PEER_VERIFIED_E2E` 方向，尚未冻结具体协议、Key 生命周期、降级防护或实现，不写成已提供保证，也不能在客户端静默从它降级到基础模式。URL fragment 虽通常不随 HTTP 请求发送，仍可被页面 JavaScript/浏览器扩展读取；若网页代码由攻击者控制，它不能充当独立可信渠道。抗恶意信令需要可信客户端和独立认证的指纹/公钥或高熵外带 Secret，而非把低熵邀请码直接用作 PSK。

### 数据分级与禁止流向

| 数据 | 允许位置 | 明确禁止 |
|---|---|---|
| SDP、ICE、Peer/Relay 网络地址 | 客户端内存、受限服务内存及短期易失缓存 | 业务表、普通日志、Outbox、持久消息队列、自动错误上报附件 |
| TURN Password、验证材料 | 客户端内存、限时 AEAD Replay、专用 Secret Store/Adapter | URL、日志、指标 Label、trace baggage、Audit Detail |
| session-scoped keyed digest/Receipt | 最小 PG 状态，按协议截止/终态清理 | 公开响应、跨 Session 相关分析、普通调试日志 |
| 授权/拒绝/撤销审计 | 最小 ID、角色、时间、固定结果分类 | 原始 Payload、自由文本网络错误、完整 Username |

拒绝解析的原始帧也属敏感数据，不能因为它“无效”就记录。反向代理、负载均衡、RPC tracing、APM、Crash Dump、Redis 持久化/复制及备份都属于检查范围：承载敏感易失数据的 Store 禁用 RDB/AOF和磁盘副本，不能与需要持久化的业务缓存混用；诊断默认不采样 Body。Keyed digest 并非匿名化，依然按敏感关联数据控制访问。

### 日志、Trace、崩溃与保留

- HTTP/WSS/RPC 在进入通用 Access Log、Trace、APM 和 Error Reporter 前执行结构化字段级 Redaction；禁止先完整序列化再用正则清洗。Query、Header、Cookie、WS Frame、内部 RPC Body 默认不采集。
- Request ID、Signaling Session ID、Grant ID、Role、Region 和固定错误分类可以进入安全事件；它们仍是关联标识，普通业务指标不得把高基数 ID 用作 Label。
- 普通日志只允许固定 Operation 名、HTTP/WS 状态、耗时、字节桶和粗粒度结果，不含 SDP/ICE、TURN Username/Password、Access Token、Invite、Peer/Relay 地址、MC Profile 或原始上游错误。
- Crash Dump/Core Dump 默认关闭；确需启用时使用隔离节点、最小采样、加密和受限访问，并维护敏感 Buffer 区域清单，由 Dump Filter 对注册区域执行可测试的掩码/排除。信令/TURN Worker 不允许把 Heap Dump 自动上传第三方。
- 承载敏感 Payload 的 Redis Namespace 禁用 RDB、AOF、磁盘 Swap、跨环境复制和通用备份；内存回收覆盖 Buffer 引用，但不承诺语言运行时立即物理擦除，故节点/进程访问仍按 Secret 等级隔离。
- 最小网络诊断必须由持有 `security.breakglass` 的指定安全操作员显式开启，限定 Grant/Session、最长 30 分钟采集窗口；操作员身份和审批者进入安全 Audit。单人值班时仍须第二名具有该权限的人员批准，不能自批。数据仅进入隔离存储，原始地址最迟在采集窗口结束后 24 小时删除。普通支持人员不能开启或下载。
- Signaling Session/Receipt 终态保留 24 小时；易失 Payload 在 epoch 替换、终态或绝对期限时立即逻辑删除；TURN 验证材料与 Peer Pin 最迟在 Grant/Allocation 回收后删除；安全 Audit 保留 30 天。清理失败告警但不延长授权。

### 安全审计事件注册表

以下类型由受控 Migration 注册，均使用固定 Schema；`metadata` 只能包含表列中明确允许的枚举/整数，不接受自由 JSON Payload：

| Event Type | 最小字段 | 禁止字段 |
|---|---|---|
| `signaling.session_created` | actor role、session/request ID、expires_at | SDP、Profile、Peer ID |
| `signaling.session_closed` | actor/service、session ID、固定 terminal classification | 自由关闭文本、ACL Rule |
| `signaling.authorization_lost` | session ID、固定 domain class、时间 | 关系详情、在线状态、Token Claim |
| `signaling.connection_replaced` | session ID、role、旧/新 connection ID digest、node class | 原始 IP、Header |
| `signaling.protocol_rejected` | session ID、role、固定 code、计数桶 | Frame/Payload、Candidate |
| `relay.grant_issued` | grant/session ID、role、region、policy、期限/额度 | Username、Password、地址 |
| `relay.grant_revoked` / `relay.grant_expired` | grant ID、固定 reason class、时间、粗粒度用量桶 | Peer、Permission、Secret |
| `relay.allocation_abuse` | grant/allocation ID、region、固定 quota/egress class | 数据包、目标地址原值 |
| `diagnostic.breakglass_enabled` | session/grant ID、operator ID、approver ID、窗口开始/结束、固定 result class | 自由文本、SDP/Candidate、原始地址 |
| `diagnostic.breakglass_download` | session/grant ID、operator ID、时间、固定 result class | 下载内容、原始地址 |
| `diagnostic.breakglass_purged` | session/grant ID、service/operator ID、时间、固定 result class | 被删内容、原始地址 |
| `audit.backfill_dropped` | source scope digest、事件类型、时间桶、固定 cause/result class | 原始待写事件、Payload、地址 |

成功 Candidate、普通 ACK/Ping/Pong、每次运行许可续期不逐条写 Audit，避免形成精细通信时间线；只写聚合计数或异常安全事件。重复协议错误按 `(session, role, code, 1min bucket)` 聚合，严重重放/SSRF/Secret 使用异常可以单独记录。Audit 写入失败时：

- 会话创建和 TURN Grant 签发在 Audit 提交前不向客户端公布成功/Secret，失败则不创建或不签发；
- 显式撤销先在 Authorizer/Relay 立即使 Grant 无效，不得因 Audit 故障拒绝或回滚撤销；事件进入有界内存补写并在 60 秒内重试，超时触发安全告警，但旧授权仍保持无效；
- `signaling.session_closed` Audit 失败不阻塞安全方向的关闭；事件按同一 60 秒有界规则补写；
- 高风险滥用先阻断，再审计；普通重复 Frame 的聚合 Audit 可有界补写，但 60 秒仍失败即关闭来源连接并记录待补的 `audit.backfill_dropped` 安全事件。缓存满同样关闭来源连接，不能无审计继续。

### 元数据和滥用边界

- ALL 模式可能向对端暴露真实/局域网地址；RELAY 降低对端可见性，但 NLI、TURN、网络提供方仍能观察连接时间与流量。mDNS 不构成完整匿名保护。
- Grant ID、Role、Node ID 是标识不是凭据；TURN Password 泄漏仍可能在授权边界内滥用，独立 Authorizer、Peer Pin和配额限制影响范围，不能声称 Secret 不可转借。
- HTTP/WSS/RPC 按 Secret 字段拒绝记录，而不是只靠正则事后清洗；结构化错误使用固定英文文案，拒绝引用客户端 SDP/Candidate 内容。
- IP 限流 Key 使用版本化 keyed digest并按短窗口滚动；所有 Digest Key 仅存在专用 Secret Store，包含 `key_version`，按密钥策略轮换。旧版本只保留至其覆盖的限流窗口或 Receipt/Session 绝对截止，随后销毁；疑似泄漏立即轮换并安全终止无法验证的旧 Session，禁止降级成未 keyed hash。安全审计仅在确有调查必要时保存规范化/截断网络标识，不能保存可逆“加密 IP”作为常规捷径。完整地址只允许进入上述 break-glass 诊断。
- TURN Permission/Peer Pin 创建及任何 Peer 地址变化时，先规范化已解析 A/AAAA 并拒绝：`0.0.0.0/8`、`10.0.0.0/8`、`100.64.0.0/10`、`127.0.0.0/8`、`169.254.0.0/16`（含 `169.254.169.254`）、`172.16.0.0/12`、`192.0.0.0/24`、`192.168.0.0/16`、`198.18.0.0/15`、`224.0.0.0/4`、`240.0.0.0/4`、`::/128`、`::1/128`、`fc00::/7`、`fe80::/10` 和 `ff00::/8`；IPv4-mapped IPv6 必须先还原为 IPv4 并应用上述 IPv4 denylist，不得按 IPv6 绕过。数据转发阶段只使用已 Pin 的规范化 IP，不重新解析 DNS；每包仍核对 Pin/fence/许可。部署另维护云 Metadata 与本地域名/IP denylist，默认拒绝。协议错误不得返回目标可达性差异。
- 同一 Guest/Family/Account/Instance/IP/Region 的速率、并发、累计字节和失败模式联合检测；自动动作只允许限流、关闭连接、撤销 Grant、临时 Guest/IP 风险桶，不自动把 CLIENT_CLAIMED Profile 归责给 Account。
- 风险封禁和 Appeal/人工调查不改变好友、ACL 或 Account 身份真值；误报恢复不能复活过期 Lease、旧 Session、旧 Grant 或旧 connection fence。
- 验收包含恶意 JSON、控制字符、压缩/碎片绕过、日志注入、跨角色 ACK、TURN SSRF、端点扫描、重放、分区、账单放大、Audit 故障和清理积压；仅完成文档检查不能声称运行时防护已测试。

### #105 安全场景

1. 无效 Frame 含 Token/换行：若有合法 message_id，Error和所有日志只出现固定 INVALID_FRAME；若无法取得合法 message_id，仅以1002/1009关闭并记录固定分类，不保存原文或注入伪日志行。
2. 客户端在 SDP/Candidate 填入内网/metadata 地址：信令可作为不可信字符串转发，但 TURN egress 不向禁止网段创建 Permission/Pin或转发数据。
3. RELAY 模式的本地 WebRTC 栈可能产生 host/srflx Candidate：可信客户端必须设置 `iceTransportPolicy=relay`，并在 SDP 和 Trickle 两条出站路径做防泄漏断言；服务端因 SDP opaque 不能声称完整识别内嵌 Candidate。恶意客户端泄漏自己的地址不提升其身份，也不能绕过 TURN egress/授权。
4. Redis 被配置持久化：部署检查失败，生产信令节点不启动，而不是把 SDP/ICE 写入 RDB/AOF。
5. Audit Store 不可用时创建 Session/Grant：高风险操作 fail closed；不会先发 Secret 后补 Audit。
6. 普通 Ping 风暴：只做限流/聚合，不生成逐帧 30 天审计造成二次 DoS。
7. TURN Username 出现在上游错误：Adapter 映射固定分类并在进入 Trace 前删除敏感值。
8. 用户要求“匿名”：UI 明确展示对端/NLI/TURN 可见的元数据和 `TRANSPORT_ENCRYPTED` 保证，不声称 Account/Minecraft 已验证。
9. 恶意信令服务替换 fingerprint：基础模式无法抵抗，文档/客户端不得显示 `PEER_VERIFIED_E2E`；未来模式必须独立评审且禁止静默降级。
10. Guest 被举报：Report 只能使用已冻结的 Join/Session 上下文，不根据 Candidate IP 自动绑定 Minecraft 或 NLI Account。
11. Break-glass 诊断超时：30 分钟后自动停止采集，最迟在该采集窗口结束后 24 小时删除原始地址；启用、下载和删除分别写固定安全审计。
12. 清理 Worker 积压：逻辑期限仍即时 fail closed；告警不会把过期 Receipt/Grant重新变成授权依据。

## 客户端参考算法与端到端验收（#106，`DESIGN_FROZEN`）

> 客户端评审发现的 `delivery_ack.reply_to` 与 `ice_restart(current+1)` 两个协议阻塞项已修正；一致性复核在四个措辞修正后批准冻结。该冻结证明契约可实现，不代表真实 WebRTC/TURN/网络故障测试已执行。

### 单一恢复算法

客户端必须把 HTTP 权威状态、当前 WS Fencing 和本地 WebRTC 状态分开，不从其中一个推断另外两个：

1. 收到 `ACCEPTED` 提示或轮询结果后，以稳定 Idempotency Key 调用 create-or-attach；HTTP 结果不确定只重发同一请求。`201/200` 都进入同一流程，不能把 200 当作另一个 Session。
2. 对已知 Session 先 `GET`。404 表示当前 Principal 不可见/不存在，不能探测对端；410、终态或同 Request 禁止重建时，需要新 Join Request。可重试 429/503/`IDEMPOTENCY_IN_PROGRESS` 时遵守 Retry-After 和绝对截止。
3. 在窗口内按策略获取 ICE Servers。`RELAY` 的 503 不得退化直连；成功时必须设置 `iceTransportPolicy=relay`，并对 SDP 内嵌与 Trickle 两条出站路径执行非 relay Candidate 防泄漏断言，断言失败则停止并显示策略错误，不能静默发送。服务端把 SDP 视为 opaque，客户端不能依赖服务端代为过滤。`ALL + UNAVAILABLE` 可在明确 UI 状态下只尝试 STUN。TURN Secret 只放进当前 WebRTC 配置内存，不写磁盘或遥测。
4. 使用满足上述 Header/Ping 要求的原生 WebSocket 栈，以当前 Access/Guest Token Upgrade 独立 WS；浏览器 JavaScript 客户端在本版必须停止并显示“不支持的信令传输”，不得用 Query/Subprotocol降级。等待 `ready` 后核对 Session、Role、Phase、epoch 和期限。新连接会替换本 Role 旧连接；客户端只让最新成功 `ready` 对应的 generation 处理入站帧，旧 generation 的回调、Timer、ACK 和 Close 全部失效。当前 generation 每 5 秒主动发送标准 WS Ping（至多 2/s），Pong只表示链路响应；业务帧不能代替 Ping，Pong/续租失败或20秒 Lease到期按4407恢复。
5. REQUESTER 在 `WAITING_FOR_OFFER` 产生 epoch 1 Offer；TARGET 只在收到并成功设置当前 Offer 后产生 Answer。Delivery ACK 只在 Frame 已通过校验并进入本地有界处理队列后发送，仍不表示 WebRTC API 应用成功；WebRTC 调用失败不得声称 CONNECTED，应发送 `close(CLIENT_ERROR|NEGOTIATION_FAILED)` 或在仍可恢复时保留上下文重试。
6. 每个可重发数据帧生成一个 UUIDv4 message_id，并在原逻辑消息整个生命周期保持原始 UTF-8 Payload 字节不变。先保存到当前进程的受保护内存，再发送；`result` 只记录服务端接受，Peer `delivery_ack` 只记录一次送达。副本必须保留到该 epoch 被新 Restart取代、Session 终态或绝对期限（先到者），不能因 ACK 或 WebRTC 本地 connected 提前删除，以支持对端重连的 `resend_required`。
7. 收到 Offer/Answer/Restart 时先按 `(sender_role, epoch, message_id)` 去重，再按 epoch 放入有界本地处理队列并发送 `delivery_ack`，随后串行调用 WebRTC。Candidate 在对应 Remote Description 成功前有界缓存；`ice_end` 不清空尚未应用的同 epoch Candidate。重复帧重新 ACK但不重复调用 WebRTC；后续应用失败是独立的本地错误/Close，不追溯改变 ACK 语义。
8. 断线、4406/4407或通知丢失执行：停止旧 generation发送 → `GET` → 若 ACTIVE 且未过期则带抖动重连 → 等 `ready/resend_required` → 以原 ID/原字节按 SDP、Candidate、ice_end 顺序重发。仅 ACK 超时时先 `GET`；若当前 Socket仍为最新 generation且可用，则在原连接按退避重发，不为一次 ACK 丢失抢占自身连接；只有 Socket失效/关闭才重连。不得自行递增 epoch或创建第二会话。
9. 只有 REQUESTER 在已有 Answer 后、WebRTC 本地 ICE restart 确已生成新 Offer时发送 `ice_restart(epoch+1)`；最多到 epoch 3。旧 epoch回调全部丢弃，不能 ACK成当前数据。
10. WebRTC `connected/completed/disconnected/failed` 只改变本地 UI/重试策略。短暂 disconnected 不写服务端；失败可在期限和 epoch额度内 Restart，否则 Close并发起全新 Join。直连成功不自动撤销已有 Relay Grant；客户端应通过标准 TURN `Refresh(LIFETIME=0)` 尽力删除未使用 Allocation以释放并发额度，这不是新的 NLI REST API，失败仍由 Grant/Reaper有界回收。
11. 进程崩溃或本地敏感上下文丢失后，服务端不能回放 SDP/Candidate。若 Session 尚可见，客户端应幂等关闭；随后走新 Join Request。不得生成新 message_id 猜测旧 SDP/Candidate以续接。
12. 结束时先尽力发送 WS `close` 或 HTTP DELETE，再关闭 WebRTC/TURN；但本地资源清理不等待网络成功。主动 Close 会触发 Relay 撤销，自然 Signaling/Guest 到期则遵循批次 3 已冻结的独立 Relay维护边界。

### 客户端错误决策表

| 观察 | 可否原 Session 恢复 | 必须动作 |
|---|---|---|
| HTTP 429/503、WS 4406/4407，且 GET 为 ACTIVE/未过期 | 是 | Retry-After/抖动退避，重连并用原 ID 重发 |
| 4401 旧连接被替换 | 仅最新 generation | 旧连接永久停止；用户明确触发或没有更新 generation 时才建立一次新连接，避免抢占风暴 |
| 401、Family/Guest 撤销、4403 | 否 | 不用 Session ID/旧 Secret 重试；按账号或 Invite 流程重新授权 |
| 410、4402、`EXPIRED_BY_WINDOW` | 否 | 新 Join Request；Guest 需要新 Invite/Guest |
| 4404、GET 终态、同 Request STATE_CONFLICT | 否 | 不 attach/recreate；需要联机则新 Join Request |
| STALE_EPOCH | 可能 | GET/ready 对齐；丢弃旧回调；仅 REQUESTER 在已有 Answer 后可显式发送 `ice_restart(current+1<=3)`，该合法未来 epoch 不属于 STALE_EPOCH |
| PHASE_CONFLICT、ROLE_NOT_ALLOWED、PROTOCOL_VIOLATION | 不自动 | 停止相关发送，核对本地状态；协议违例关闭连接 |
| CANDIDATE_LIMIT_REACHED / EPOCH_LIMIT_REACHED | 否或仅等待现有 ICE | 不绕过上限；现有 ICE 失败则 Close + 新 Join |
| RELAY policy 下 TURN_UNAVAILABLE | 否（本次隐私策略） | 明确失败；禁止 host/srflx 降级 |
| `result=ACCEPTED|DUPLICATE` 但无 Peer ACK | 是 | 按最旧优先和既定退避使用原 ID/原字节重发 |
| Client Context Lost | 否 | 幂等关闭可见旧 Session，创建新 Join；不请求服务端返回 Payload |

### #106 端到端场景矩阵

1. Target 先 create、Requester 后 attach：两者得到同一 Session，Requester仍为唯一 Offerer。
2. Create 响应丢失并跨节点重试：相同 Key返回首次快照；客户端随后 GET，不误把快照状态当当前状态。
3. 双方同时重连：每个 Role 独立替换自身 Slot，Route revision 串行，任一旧 Socket均不能投递。
4. Requester Offer提交成功但 `result` 和投递都丢失：原 ID/字节重发返回 DUPLICATE，PG Phase不重复推进。
5. Target 收到重复 Offer：只调用一次 setRemoteDescription，每次重复均可 ACK；Answer保持同一 ID/字节重发。
6. Candidate、ice_end、Candidate重投乱序：新 Candidate 在 ice_end 后拒绝；Receipt存在的同 ID/同 digest重投为 DUPLICATE且可再次交付。
7. Candidate 在远端 SDP 前到达：通过校验并进入有界缓存即可 ACK；SDP应用失败则不调用 addIceCandidate，随后按本地失败规则 Close，ACK 不被解释为应用成功。
8. Peer ACK 在发送方断线时到达：不持久化；发送方重连后可安全重发，接收方去重并重新 ACK。
9. 双方待 ACK窗口都满：ACK/control预留继续流动，oldest-first释放窗口，不形成互锁。
10. Backpressure Close帧无法排队：物理关闭并 compare-delete自身 fence；逻辑 Session仍由 GET恢复。
11. Store瞬时故障一轮 Ping：不接受业务但 Socket活到原20秒 Lease；恢复后仅 current fence可续租。
12. 20秒 Lease到期和新节点握手竞争：Route CAS只留下新 generation；旧节点延迟 Close无效。
13. ACL/Friend/Proxy撤销与250ms投递许可竞争：撤销后不批准新许可；已批准网络尾部由当前 generation/epoch过滤。
14. Guest绝对到期恰逢 Answer提交：数据库时钟和锁序只允许提交或过期其中一个结果，无半条投递。
15. ICE epoch 1失败并 Restart到2：旧 epoch Candidate/ACK全部丢弃；新 Offer由 Requester发送。
16. epoch 3仍失败：不允许第四个 epoch；关闭并新 Join，不在同 Request重建 Session。
17. UDP黑洞但TCP/TLS可用：窗口内并行尝试可激活；客户端不等UDP完整超时再开始回退。
18. RELAY Grant已激活后Signaling自然到期：不再发信令/新建Allocation，原Allocation可在独立截止内维护。
19. 用户主动关闭Signaling：授权器撤销全部绑定Grant；最长15秒许可尾部后停止Relay。
20. 进程崩溃且易失Payload丢失：Receipt/PG状态不泄漏Payload；客户端关闭旧Session并新Join。
21. 通知、EventBus全丢：双方仅凭HTTP create-or-attach/GET和已知ID完成恢复，不依赖通知Replay。
22. Audit Store在撤销时故障：Grant先失效，补写失败不复活；Session/本地资源仍安全关闭。
23. 未知v2服务端消息到v1客户端：不执行Payload，关闭WS并GET；服务端不得把未知客户端消息透传。
24. UI显示匿名会话：协商页只把 `TRANSPORT_ENCRYPTED` 显示为模式保证；只有 WebRTC 本地 DTLS/Data Channel成功后才显示“连接已加密”，不显示已验证Account/MC身份或 `PEER_VERIFIED_E2E`。
25. 浏览器原生 JavaScript WebSocket 尝试加入：客户端在发 Secret 前明确报不支持；不会把 Bearer 放入 Query/Subprotocol，也不会建立首帧认证旁路。
26. 收到 `resend_required`：接收者只按 `required_from_self` 重发自己的当前 epoch原 ID/字节；不会猜测或代发对端 Payload。
27. 当前 WS generation 维持 5 秒 Ping：业务流量持续但 Ping停止时 Lease仍到期；Pong/续租恢复只允许 current fence继续。
28. RELAY 模式发现 SDP或Trickle出现非 relay Candidate：客户端在发送前失败并清理，不依赖 opaque 服务端过滤，也不静默降级 ALL。

## 安全与隐私检查清单

- [x] 未批准或 Lease 过期的 Request 无法创建信令会话
- [x] Request ID、Session ID 和 signaling_session_id 均不能单独作为 Bearer Credential
- [x] 旧 Token Family、旧 Guest、旧 WS/信令连接无法推进当前会话
- [x] ACL/Source/Proxy 失效能及时阻止后续操作；已消费 Invite 的非自撤销边界明确
- [x] SDP、ICE、IP、TURN Secret 的禁止流向、Redaction 与受限诊断规则已冻结（运行时待验收）
- [x] 错误响应不泄漏对端地址、在线状态或隐藏授权分支
- [x] Candidate 和 SDP 的大小、数量、字符、频率、队列和缓存规则已冻结
- [x] 多节点路由不可用时 fail closed，不退化为无 Fencing 的节点本地授权
- [x] TURN Credential 配合独立 Authorizer 最小权限、有限期限和配额（实现待验收）
- [x] Guest 只能访问其绑定的唯一 Join/信令上下文

## 客户端可实现性检查清单

- [x] 双方能确定角色和第一条合法消息
- [x] 重复 HTTP 或实时消息不会创建第二个逻辑会话或重复推进 Phase
- [x] 消息丢失或乱序后有确定的重连、ACK、重发、epoch 缓冲动作
- [x] ICE Candidate 可分批发送并以每角色 `ice_end` 明确结束
- [x] ICE Restart 使用原 Session/Fencing 和新 epoch，最多两次且不延长期限
- [x] 对端离线、Target 重连、节点不可用和 TURN 失败有稳定错误与恢复动作
- [x] 原生客户端明确实现 5 秒标准 Ping / 20 秒 Lease；浏览器 JS 传输明确不受支持且无 Secret 降级旁路
- [x] 客户端能区分重试、禁止的同 Request 重建和必须重新发起 Join Request
- [x] 未知消息类型和未来字段具有前向兼容规则

## Phase 9 设计批次

### 批次 1：授权入口与角色（`FROZEN`）

- [x] create-or-attach、读取、关闭和独立 WS 入口；
- [x] Join Request/Lease 原子重验与无一次消费语义；
- [x] 服务端推导 Requester/Target，固定 Requester Offerer；
- [x] 精确 Session/Guest 绑定与 Participant Connection Fencing；
- [x] 单 Request 单逻辑会话、幂等、锁顺序、并发与封闭错误集合；
- [x] PostgreSQL 权威、LeaseStore 路由及失效收敛边界。

### 批次 2：Offer / Answer / ICE（`FROZEN`）

- [x] PostgreSQL 权威 Phase/epoch 与客户端本地 CONNECTED 边界；
- [x] 按方向封闭的 v1 Envelope、Result/Error/ACK；
- [x] Trickle ICE、每角色 `ice_end` 与 Candidate 硬上限；
- [x] 原始字节 keyed digest、消息去重、乱序和确定重发；
- [x] 最多两个新 epoch 的显式 ICE Restart；
- [x] 终态最小读取和 24 小时保留；
- [x] 精确每秒速率、队列和关闭码数值明确留给批次 4。

### 批次 3：TURN 与 NAT（`DESIGN_FROZEN`）

- [x] STUN/TURN 配置发现与不可降级的 RELAY 策略；
- [x] Credential 签发/重放窗口与独立维护期限；
- [x] Prepare/Activate、短运行许可、Peer Pin、撤销与孤儿清理；
- [x] 多 Allocation、并发/累计配额及 byte-credit；
- [x] 23 个场景、独立复核修正与生产 Adapter 验收门槛。

### 批次 4：多节点、恢复与隐私（`FROZEN`）

- [x] 5 秒 Ping / 20 秒连接 Lease、session-scoped 原子 Route Document 与 Fencing；
- [x] 节点 RPC、250ms 投递许可、故障/断线/重连；
- [x] Candidate 最小 Receipt、ACK/重发与有界乱序；
- [x] Frame、队列、缓存、速率、关闭码、TTL 与清理；
- [x] 日志、审计、匿名 E2E 边界和滥用防护复核；
- [x] 12 步客户端算法、错误决策表、28 个端到端场景与双重阅读评审。

## Phase 9 完成标准

- [x] 信令只服务仍被授权的已批准 Join Request
- [x] 信令设计不改变 Gate E 冻结的账号、好友、实例和 Join 语义
- [x] 所有信令会话、消息路由和 TURN Credential 均短期有效
- [x] PostgreSQL 权威与易失路由边界明确
- [x] 多节点故障与旧连接竞争具有 Fencing
- [x] 安全、隐私、限流和客户端恢复场景通过设计评审（运行时验收仍待实现）
- [x] Gate F 将 4 Paths / 5 Operations 同步至 REST、数据模型和 OpenAPI，81 Paths / 94 Operations 冻结
