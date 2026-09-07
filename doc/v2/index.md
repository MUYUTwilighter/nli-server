# NetherLink v2 实现导航索引

> 状态：`GATE_F_FROZEN + D-329_ROUTING_AMENDMENT / IMPLEMENTATION_NAVIGATION`
>
> 用途：让后续实现者先定位**最小必要上下文**，而不是一次加载全部设计文档。本文只做导航，不复制或覆盖正式规范；当前实现事实先读 [`implementation_progress.md`](implementation_progress.md)，任何行为语义以链接的权威文档和 [`openapi.yaml`](openapi.yaml) 为准。
>
> Pi 自动入口：项目受信任后，[`.pi/APPEND_SYSTEM.md`](../../.pi/APPEND_SYSTEM.md) 会在保留默认系统提示词的前提下追加本项目规则，并要求每回合刷新实现进度。修改该文件后需重启 Pi 或执行 `/reload`；使用 `--no-approve` 或项目未受信任的非交互运行不会加载它。

## 30 秒开始

1. 先只读本文；
2. 读取 [`implementation_progress.md`](implementation_progress.md) 的当前交接，再读取其中链接的当前 Phase 账本；
3. 在“按实现阶段取最小上下文”或“按任务查权威来源”中选择一行；本文下文 `Phase 0–10` 一律指 `implementation_plan.md` 的实现阶段；
4. 只读取该行列出的章节，不要先通读整份大文件；
5. 用 `rg` 定位标题、Operation ID、表名或 ProblemCode，再用带 `offset/limit` 的读取工具取局部；
6. 只有当前切片出现直接依赖时，才追加下一份文档；
7. 实现若要求改变冻结语义，停止编码并显式重开对应 Gate。

默认首轮上下文上限：**本文 + 进度仪表盘 + 当前 Phase 账本 + `implementation_plan.md` 的一个 Phase + 最多三份 prose 规范的相关章节 + 一个目标 OpenAPI Operation/Component 切片**。不要把 `outline.md`、`rest_api.md`、`data_model.md`、`openapi.yaml` 和全部领域文档一起塞入上下文。

## 权威来源怎么选

| 需要回答的问题 | 第一权威来源 | 仅在需要时补充 |
| --- | --- | --- |
| Pi 每轮从哪里进入 | [`.pi/APPEND_SYSTEM.md`](../../.pi/APPEND_SYSTEM.md) | 只负责引导到本索引和进度文件，不作为契约或进度权威 |
| 当前做到哪里、下一动作、阻塞和证据 | [`implementation_progress.md`](implementation_progress.md) + 当前 Phase 账本 | Git/CI/发布产物核实 |
| 阶段顺序、提交边界、部署和验收定义 | [`implementation_plan.md`](implementation_plan.md) 对应 Phase | “已冻结的实施决策”“测试矩阵”“风险与实现阻塞” |
| 产品范围、术语和跨域方向 | [`outline.md`](outline.md) 对应主题 | 不用它替代详细领域规则 |
| ID、JSON、时间、Token、幂等、错误、分页、日志 | [`common.md`](common.md) 对应章节 | REST/OpenAPI 的端点特例 |
| 领域状态机、授权含义和并发场景 | 对应领域文档 | REST 入口、数据模型、OpenAPI |
| HTTP Principal、DTO、状态码、幂等和限流 | [`rest_api.md`](rest_api.md) 对应 API 族 | 对应领域文档与 OpenAPI Operation |
| 表、FK、唯一约束、锁序、TTL、清理、Redis | [`data_model.md`](data_model.md) 对应表/横切章节 | 对应领域状态机 |
| 精确 Path/Method/Schema/Header/Response | [`openapi.yaml`](openapi.yaml) 的单个 Operation/Component；`servers: /v2` 是公共前缀，`paths` 是应用无前缀路由 | `rest_api.md` 的语义解释 |
| 通知 WebSocket 与 Instance ONLINE | [`notifications.md`](notifications.md) | `game_instance.md`、data model 的 WS Lease |
| 信令、NAT、TURN 和客户端恢复 | [`signaling.md`](signaling.md) | Gate F REST/Data/OpenAPI 局部 |

这些文档描述不同层次，不存在可静默选择的“高优先级文档”。如果两份权威文档矛盾，当前切片应停止并重开 Gate，而不是自行猜测。

设计文档状态中的 `PHASE_N` 是历史设计阶段编号（例如 `notifications.md` 为设计 Phase 5、`signaling.md` 为设计 Phase 9）；本文“按实现阶段取最小上下文”的 Phase 0–10 则只对应 `implementation_plan.md`，二者不得混用。

## 全局不变量速查

实现任何切片都不得违反：

- v2 完全舍弃 v1 业务语义和数据，不复用旧 Profile/Presence/Signaling 权威模型；
- 客户端公共 URL 使用 `/v2/{path...}`；反向代理剥离版本首段，v2 应用 Router/Policy/manifest 只使用无版本前缀的 `/{path...}`，不得在应用内重复挂载 `/v2`；
- MC Profile 始终为 `CLIENT_CLAIMED`，不构成 NLI 或 Minecraft 身份证明；
- PostgreSQL/HTTP 是业务权威；LeaseStore/EventBus 只承载可丢失状态；
- 客户端提交的 Account、Role、Source、Traits、ACL、Owner、期限和配额字段都不可信；
- Access Token、Refresh Token、Guest Token、Invite/Proxy Secret 和 TURN Password 必须按各自 Purpose/Audience 使用；
- 关键写入遵循冻结的幂等、锁序、Audit 和 Outbox 故障方向；
- `notifications.md` 定义的通知 WS 只负责通知/保活；`signaling.md` 定义的 Signaling WS 是唯一受限实时业务写入例外；
- SDP、ICE Candidate、网络地址、TURN Secret、Peer Pin 原值不得进入普通业务表、Outbox、Audit Detail、日志或持久队列；
- 浏览器 JavaScript WebSocket 不支持 Phase 9 Signaling；Token 只能在 Upgrade `Authorization` Header；
- 生产 `NLI_RELAY_ENABLED=false`，直到 `B-09-TURN` 真实验收和人工审批通过。

完整依据分别见 [`common.md`](common.md)“已确认的跨模块约束”、各领域文档“已确认的不变量”、[`signaling.md`](signaling.md)“已冻结的输入边界”和 [`implementation_plan.md`](implementation_plan.md)“已冻结的实施决策”。

## 文档清单

| 文档 | 何时读取 | 优先定位的章节 |
| --- | --- | --- |
| [`outline.md`](outline.md) | 不熟悉产品或跨域术语时 | “统一术语”、目标领域、“P2P 信令、NAT 与 TURN” |
| [`common.md`](common.md) | 所有 HTTP/安全/存储切片按需读取 | “ID 与安全凭据”“JSON”“错误响应”“HTTP 鉴权”“幂等”“日志、审计和隐私” |
| [`nli_account.md`](nli_account.md) | Account/Auth/Session/Device/Recovery | “NLI Account”“Session 与 Token Family”“Device Code Flow”“邮箱验证与账号恢复” |
| [`provider.md`](provider.md) | Provider Registry/Login/Binding/Credential | “Provider Adapter”“Provider Login Identity”“Provider Binding”“Credential Manager” |
| [`friendship.md`](friendship.md) | Friend/Block/Projection/Sync/Aggregation | “NLI 好友 Pair”“Provider 外部好友投影”“好友同步任务”“好友聚合读模型” |
| [`game_instance.md`](game_instance.md) | Instance/ACL/Proxy/Invite/Guest/Join | 对应同名章节；Join 实现还读“Acceptance Lease”与“权限矩阵” |
| [`notifications.md`](notifications.md) | Instance WS、ONLINE、通知恢复、多节点提示 | “连接鉴权”“WS Session 与原子替换”“Ping/Pong”“HTTP 状态恢复”“单节点与多节点抽象” |
| [`signaling.md`](signaling.md) | Signaling WS、ICE、TURN、恢复、安全 | 按批次读取；不要默认通读全文 |
| [`rest_api.md`](rest_api.md) | 实现 Handler/Policy/契约测试 | “端点总表”后只读当前 API 族；横切时读 Principal/状态/幂等矩阵 |
| [`data_model.md`](data_model.md) | Migration/Repository/Worker/Redis | 只读当前表组，加“全局约束”“TTL”“清理”“锁顺序”中的必要部分 |
| [`openapi.yaml`](openapi.yaml) | 精确实现或生成 DTO/Test 时 | 只取单个 Path、Operation、参数和引用到的 Component |
| [`implementation_plan.md`](implementation_plan.md) | 每个实现切片开始和验收时 | 当前 Phase；必要时追加实施决策/测试/风险章节 |
| [`.pi/APPEND_SYSTEM.md`](../../.pi/APPEND_SYSTEM.md) | Pi 每回合自动追加；只在维护入口口径时人工读取 | 强制读取顺序、权威边界和进度更新协议 |
| [`implementation_progress.md`](implementation_progress.md) | 每次 Agent 恢复、开始和结束切片时 | “当前交接快照”“Phase 总览”“活跃阻塞与 Gate”；再跟随当前 Phase 账本 |

## 按实现阶段取最小上下文

当前 Phase 不得凭聊天历史推断：先以 [`implementation_progress.md`](implementation_progress.md) 为准，并读取它链接的单个 `progress/phase-N.md`。下面只定义各 Phase 的规范上下文包。

### Phase 0：基线、门禁和发布骨架

当前切片先服从 [`progress/phase-0.md`](progress/phase-0.md) 的更窄最小上下文包；下列内容是 Phase 0 全阶段上下文，不要求在 `P0-01` 一次读完。

必读：

- [`implementation_progress.md`](implementation_progress.md) 当前交接与 [`progress/phase-0.md`](progress/phase-0.md) 当前切片；
- [`implementation_plan.md`](implementation_plan.md)：Phase 0、“Operation Policy Manifest”、“WSL Release Gate”；
- [`common.md`](common.md)：“API 版本”“JSON”“错误响应”；
- [`openapi.yaml`](openapi.yaml)：只检查 `info`、`servers`、Operation inventory 和引用。

无需加载领域状态机、完整数据模型或完整 OpenAPI。

### Phase 1：平台层、Migration Journal 和横切内核

必读：

- `implementation_plan.md`：Phase 1、4.1、4.3～4.5；
- `common.md`：“ID 与安全凭据”至“限流”，以及“日志、审计和隐私”；
- `data_model.md`：“公共 PostgreSQL 约定”“幂等与 Secret Replay”“Transactional Outbox”“安全审计”“Redis / LeaseStore / EventBus”“全局事务与锁顺序”“命名与 Migration 约定”。

只有实现具体表时才追加对应表章节。

### Phase 2：Account、Auth、Device、Session

必读：

- `implementation_plan.md`：Phase 2；
- `nli_account.md`：当前纵切的状态机；
- `rest_api.md`：“Auth、Account、Session 与 Device Code 契约”中的当前端点；
- `data_model.md`：Account、Token Family、Device/邮件凭据中的当前表；
- `openapi.yaml`：当前 Operation 和直接 `$ref`。

按需追加 `common.md` 的 Token、幂等、错误或限流章节；不要读取 Provider/Friend/Instance 文档。

### Phase 3：Provider、Friendship、Sync

Provider 切片：`implementation_plan.md` Phase 3 + `provider.md` 当前章节 + `rest_api.md` Provider API + `data_model.md` Provider 表 + 当前 OpenAPI Operation。

Friend/Sync 切片：`implementation_plan.md` Phase 3 + `friendship.md` 当前章节 + `rest_api.md` Friendship/Sync + `data_model.md` Friendship/Projection/Task + 当前 OpenAPI Operation。

只有 Provider 好友同步这种跨域切片才同时加载 `provider.md` 与 `friendship.md`，并限制到直接相关章节。

### Phase 4：Instance、Notification、ACL、Proxy、Invite、Guest、Join、Report

按纵切拆分：

- Instance/ACL：`game_instance.md`“Game Instance”“Instance ACL” + REST/Data/OpenAPI 对应局部；
- Notification WS：`notifications.md` 当前批次 + `game_instance.md` ONLINE/生命周期局部 + data model 的 Instance WS Lease；
- Proxy/Invite：`game_instance.md` 对应章节 + REST/Data/OpenAPI 对应局部；
- Guest/Join/Report：`game_instance.md`“Guest Session”“Join Request”“权限矩阵” + REST/Data/OpenAPI 对应局部。

不要因为这些能力同属 Phase 4 就一次加载全部五份完整文档。

### Phase 5：Gate E 集成

必读：`implementation_plan.md` Phase 5、`rest_api.md` 横切矩阵、`data_model.md` 的 TTL/清理/锁序/安全场景、`common.md` 的安全与错误约定。OpenAPI 用工具做全量机械检查，不放入语言模型上下文。

只有测试失败定位到某领域时，才加载对应领域场景章节。

### Phase 6：Gate F Signaling Session 与多节点 WS

必读：

- `implementation_plan.md`：Phase 6；
- `signaling.md`：批次 1、批次 2、批次 4A，以及“客户端参考算法”中当前测试涉及的部分；
- `rest_api.md`：“Phase 9 Gate F 扩展：Signaling”；
- `data_model.md`：`signaling_sessions`、`signaling_candidate_receipts`、LeaseStore、锁/保留；
- `openapi.yaml`：5 个 Signaling Operation 和 Client/Server Envelope Component。

仅在比较通知 WS 边界时读取 `notifications.md` 的“已确认的不变量”和“连接鉴权”，不要通读通知场景。

### Phase 7：Relay 持久模型与隐藏 Authorizer

必读：`implementation_plan.md` Phase 7、`signaling.md` 批次 3、`data_model.md` Gate F 的 Relay Grant/Slot/Quota/Permit/清理章节。

按需：`rest_api.md` ICE Server DTO；`openapi.yaml` `issueSignalingIceServers` 与 ICE Components。此阶段 Relay 仍不得生产开启。

### Phase 8：自定义 TURN Adapter 与真实验收

必读：`implementation_plan.md` Phase 8 与“风险与实现阻塞”表的 `B-09-TURN` 行、`signaling.md` 批次 3 的 Authorizer/维护/NAT/配额/安全/验收门槛、`data_model.md` Relay 局部。

只在公共 Credential 行为测试时追加 Gate F REST/OpenAPI；不要加载 Account/Friend 全文，撤销条件用 `rg` 定位对应领域章节。

### Phase 9：原生客户端或 Conformance Harness

必读：`implementation_plan.md` Phase 9、`signaling.md`“客户端参考算法与端到端验收”“客户端错误决策表”及相关 Envelope/ICE 小节、Gate F REST/OpenAPI 局部。

只有测试具体授权失败时才读取 `game_instance.md` 的 Join/Acceptance Lease；浏览器 JS 路径明确不支持。

### Phase 10：Canary、切流和 v1 移除

必读：`implementation_plan.md` Phase 10、4.6～4.8、测试矩阵与风险表。按部署失败点追加 `common.md` 的日志/审计边界或 `data_model.md` 的备份/清理相关章节，不重新加载全部领域设计。

## 按常见任务查权威来源

| 任务 | 最小上下文 |
| --- | --- |
| 新增/实现一个 Handler | `rest_api.md` 当前端点 + `openapi.yaml` 当前无前缀 Operation/DTO + 对应领域状态转换；应用 Router 不挂 `/v2` |
| 实现 Operation Policy | `rest_api.md` Principal/状态/幂等矩阵 + `common.md` 鉴权/错误 + OpenAPI Operation |
| 写一张表的 Migration | `data_model.md` 当前表 + “公共 PostgreSQL 约定” + 当前表涉及的全局 FK/锁/TTL |
| 写 Repository 事务 | 领域并发规则 + `data_model.md` 当前表/全局锁序 + REST 幂等语义 |
| 写清理 Worker | `data_model.md`“TTL 与保留矩阵”“清理与删除编排” + 当前父子表 + 对应领域终态 |
| 写 Audit/Outbox | `common.md`“日志、审计和隐私” + data model Audit/Outbox + implementation plan 4.5 |
| 写通知 WS | `notifications.md` 当前章节 + data model Instance WS Lease + REST 通知映射 |
| 写 Signaling WS | `signaling.md` 批次 1/2/4A + Gate F REST/Data/OpenAPI 局部 |
| 写 TURN Authorizer | `signaling.md` 批次 3 + data model Relay 局部 + implementation plan Phase 7 |
| 写 TURN Adapter | 上一行 + implementation plan Phase 8/B-09-TURN |
| 写客户端恢复 | `signaling.md` 客户端算法/错误表/场景 + Gate F OpenAPI Envelope/ICE |
| 调查授权漏洞 | 当前领域“已确认的不变量/权限矩阵/安全场景” + REST Principal + data model 锁序 |
| 调查敏感数据泄漏 | `common.md` 日志隐私 + `signaling.md` 数据分级 + data model 存储边界 |

## OpenAPI 不整文件加载

`servers: /v2` 表示客户端公共 Base Path；下方 `paths`（如 `/auth/login`）就是应用 Router、Operation Policy 和 mounted manifest 应使用的无版本路由。反向代理负责把公共 `/v2/auth/login` 剥离为应用 `/auth/login`。

先找 Operation：

```bash
rg -n "operationId: <operationId>" doc/v2/openapi.yaml
rg -n "^  /<path>" doc/v2/openapi.yaml
```

再从命中位置读取该 Operation，并仅追踪它直接使用的 `$ref`。Component 可用：

```bash
rg -n "^    <SchemaName>:" doc/v2/openapi.yaml
rg -n "#/components/(schemas|parameters|responses)/<Name>" doc/v2/openapi.yaml
```

全量 81/94、引用和 Schema 正反例应交给脚本/CI，不应通过把 `openapi.yaml` 全文（当前 4468 行）放进模型上下文来验证。

Gate F 5 个 Operation：

```text
createOrAttachSignalingSession
getSignalingSession
closeSignalingSession
openSignalingWebSocket
issueSignalingIceServers
```

## 大文档局部读取方法

列标题，不读正文：

```bash
rg -n '^#{1,3} ' doc/v2/<file>.md
```

按概念同时定位少量权威文件：

```bash
rg -n "<term>" doc/v2/{common,rest_api,data_model}.md
rg -n "<term>" doc/v2/{game_instance,notifications,signaling}.md
```

常用定位词：Operation ID、Path、ProblemCode、表名、状态 enum、`Idempotency-Key`、`Audience`、`ON DELETE`、`expires_at`、`revision`、`Fencing`、Audit Type。

避免只按旧行号长期引用；设计文件变化后行号会漂移，标题、Operation ID、表名和 Schema 名才是稳定导航键。

## 实现切片的上下文交接模板

后续给编码 Agent 的任务应先填这份短模板，而不是附加所有文档：

```text
实现阶段：Phase N
切片：<单一纵切或基础设施任务>
Operation ID / 表 / Worker：<稳定导航键>
必读章节：<最多 3 份文档的精确标题>
按需章节：<触发条件 -> 文档标题>
必须保持的不变量：<只列与该切片直接相关的 3-8 条>
明确非目标：<本切片不得实现或改变什么>
验收：<命令、契约、并发/故障/安全场景>
Gate：<功能开关、发布或生产门槛>
路由视图：<公共 /v2/path → 代理剥离 → 应用 /path>
```

完成当前切片后，必须先按 [`implementation_progress.md`](implementation_progress.md) 的更新协议写入证据、状态和下一动作，再生成下一个上下文包，避免跨 Phase 累积无关上下文。

## 开始编码前检查

- [ ] 已读取 `implementation_progress.md` 和当前 Phase 账本，并核对分支、HEAD、切片状态与下一动作；
- [ ] 已确定 `implementation_plan.md` 中唯一当前 Phase 和切片；
- [ ] 应用路由、Operation Policy 和 mounted manifest 使用无版本 `/{path...}`，公共 `/v2` 只在代理/客户端 URL 测试中出现；
- [ ] 已列出当前 Operation ID、表名、Worker 或协议消息；
- [ ] 已读取领域状态转换，而不只是 OpenAPI Schema；
- [ ] 已读取 Principal、错误、幂等和限流的当前端点规则；
- [ ] 已读取当前数据表的 FK、唯一约束、锁序、TTL 和清理依赖；
- [ ] OpenAPI 只提取当前 Operation 与直接 Components；
- [ ] 已列出负向授权、并发、故障和敏感数据测试；
- [ ] 未引入 v1 业务语义；
- [ ] 未要求静默修改 Gate E/Gate F；
- [ ] 未提前打开 `NLI_API_V2_ENABLED`、`NLI_SIGNALING_ENABLED` 或 `NLI_RELAY_ENABLED`。
