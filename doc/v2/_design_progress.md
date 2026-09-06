# NetherLink v2 详细设计进度表（临时）

> **文件性质：临时监督文件。** 用于跟踪 `doc/v2/outline.md` 后续细化进度、设计关卡、阻塞项和决策记录。
>
> 当 v2 HTTP Contract Freeze 完成、详细设计转入正式实现计划后，可以归档或删除本文件。正式设计结论必须写入对应的模块文档，不能只保留在本文件中。

## 状态标记

- `[ ]`：尚未开始
- `[-]`：进行中
- `[x]`：已完成并通过对应检查
- `[!]`：被阻塞，必须在“阻塞项”中说明原因
- `[D]`：明确延期，不属于当前阶段交付范围

## 进度维护规则

每次推进设计时都应更新：

1. 当前阶段状态；
2. 对应文档路径；
3. 已确认的关键决策；
4. 尚未解决的问题；
5. 完成检查及其证据；
6. 下一个可执行步骤；
7. 下方“进度记录”中的一条简短记录。

约束：

- 本文件只记录进度和摘要，详细设计必须写入正式模块文档；
- 未通过当前 Gate 时，不开始依赖该 Gate 的后续设计；
- 总大纲发生变化时，先更新 `outline.md`，再同步本文件和受影响子文档；
- OpenAPI 冻结前，不创建最终数据库迁移；
- 信令设计不得反向改变已经冻结的账号、好友和实例授权语义；如果确实需要改变，必须重新打开对应 Gate。

## 当前状态

- 总体状态：`DETAIL_DESIGN_COMPLETE / GATE_F_FROZEN / IMPLEMENTATION_PLANNED_NOT_STARTED`
- 当前阶段：实现准备 — 基线已冻结、实现计划已完成，尚未开始编码
- 当前契约基线：Git commit `3b4f329` / annotated tag `v2-design-gate-f`；包含 `doc/v2/outline.md`、`common.md`、`nli_account.md`、`provider.md`、`friendship.md`、`game_instance.md`、`notifications.md`、`signaling.md`、`rest_api.md`、`data_model.md`、`openapi.yaml`
- 当前工作文档：`doc/v2/implementation_plan.md`（非契约文档）
- 下一个可执行步骤：仅在用户明确授权后执行 `implementation_plan.md` 的 Phase 0–1 首批切片；先建 CI/契约门禁和独立 v2 Database/Migrator，不开放业务路由
- 用户执行边界：实现计划已制定但实现未开始；不得自动创建 Migration、修改 Rust API、实现 TURN Adapter 或客户端

## 总体依赖关系

```text
outline.md
    |
    v
common.md
    |
    v
nli_account.md
    |
    v
provider.md
    |
    v
friendship.md
    |
    v
game_instance.md
    |
    v
notifications.md
    |
    +----------------+
    |                |
    v                v
rest_api.md      data_model.md
    |                |
    +-------+--------+
            v
       openapi.yaml        (Gate E)
            |
            v
       signaling.md
            |
            v
 rest_api.md + data_model.md + openapi.yaml
                  (Gate F extension)
            |
            v
 implementation_plan.md (non-contract)
```

说明：

- `rest_api.md` 与 `data_model.md` 都依赖领域模型冻结；两者可以交替校验，但 API 不应暴露数据库内部结构；
- `openapi.yaml` 只能从已经评审的 REST 契约生成；
- `signaling.md` 依赖已经冻结的 Instance、Guest 和 Join Request 语义；
- Gate F 将 `signaling.md` 的 5 个入口及其最小持久模型回写 `rest_api.md`、`data_model.md` 与 `openapi.yaml`，不改变 Gate E 的 89 个 Operation。

---

## 基线：v2 总大纲

状态：`COMPLETE`

- [x] NLI Account 与 MC Profile 身份边界
- [x] Provider 基础抽象与绑定失效状态
- [x] 好友关系、Provider 同步与有向 ban
- [x] 代理发布授权码
- [x] Game Instance 是唯一联机目标
- [x] 匿名 Guest 与邀请码加入
- [x] Join Request 基本语义
- [x] WebSocket 通知与客户端主动保活
- [x] 初始数量和时间限制

正式文档：`doc/v2/outline.md`

---

## Phase 0：公共约定

状态：`COMPLETE`

目标文档：`doc/v2/common.md`

### 设计任务

- [x] 定义 ID 类型和不透明 ID 使用原则
- [x] 定义时间戳、时区、TTL 和过期判断规则
- [x] 定义 JSON 字段命名规则
- [x] 定义 HTTP 鉴权头与 Token Audience
- [x] 定义幂等键规则和适用写操作
- [x] 定义分页、排序和游标规则
- [x] 定义统一错误响应结构
- [x] 定义限流响应结构
- [x] 定义日志、审计与隐私字段规则
- [x] 冻结“HTTP 为权威状态，WebSocket 仅通知和保活”的公共约束
- [x] 定义服务端推导字段不得由客户端覆盖的规则

### 完成标准

- [x] 后续模块不需要重复定义鉴权、时间、错误和分页格式
- [x] 公共约定不依赖具体数据库表
- [x] 至少用账号查询、好友申请、实例发布三个示例验证约定可用

### 备注

- 当前只需冻结统一结构，不必提前枚举所有业务错误码。

---

## Phase 1：NLI Account 与认证

状态：`COMPLETE / GATE_A_PASSED`

目标文档：`doc/v2/nli_account.md`

依赖：Phase 0

### 设计任务

- [x] Account ID、邮箱、昵称和账号状态
- [x] 昵称搜索与公开账号定位规则
- [x] 邮箱验证流程
- [x] 邮箱密码注册和登录
- [x] 第三方注册后的 NLI 密码设置
- [x] 密码修改与重置邮件
- [x] 账号停用、删除与恢复
- [x] Account Session
- [x] Instance Session
- [x] NLI Device Code Flow
- [x] Access Token 与 Refresh Token
- [x] Refresh Token 轮换和重用检测
- [x] 一组 Instance Token 绑定一个 Game Instance
- [x] 会话撤销与密码重置后的全量撤销
- [x] Device Code 状态机
- [x] Token Family 状态机
- [x] 认证与恢复接口的限流和防枚举规则

### 完成标准

- [x] 任意 NLI 请求都能确定 Principal、Audience 和权限范围
- [x] Account Session、Instance Session、Guest Session 不混用
- [x] Provider 完全不可用时仍能通过已验证邮箱恢复账号
- [x] Device Code、Token Family 和密码重置均具有明确过期和原子消费规则
- [x] 使用典型成功、拒绝、过期、重放场景完成走查

### Gate A：身份模型冻结

- [x] Account ID 语义冻结
- [x] Account/Instance Session 边界冻结
- [x] Token 生命周期冻结
- [x] Device Code 流程冻结
- [x] 邮箱恢复流程冻结
- [x] Gate A 评审通过

---

## Phase 2：Provider 抽象与凭据管理

状态：`COMPLETE / GATE_B_PASSED`

目标文档：`doc/v2/provider.md`

依赖：Gate A

### 设计任务

- [x] Provider Capability 表示
- [x] Provider Adapter 统一接口
- [x] Credential Manager 统一边界
- [x] Provider Binding 数据与用途开关
- [x] `ACTIVE / REAUTH_REQUIRED / DISABLED` 状态机
- [x] 临时错误、需要重验证和不支持错误分类
- [x] Provider 凭据加密存储和最小暴露原则
- [x] Access Token 刷新及 Refresh Grant 管理
- [x] 重新验证、解绑和换绑流程
- [x] Provider 回调的 state/CSRF 防护
- [x] 自动好友同步的后台凭据要求
- [x] 重试、退避与熔断策略
- [x] Provider 服务不可用时的业务降级
- [x] Mock Provider 测试接口

### 完成标准

- [x] 业务模块不能直接读取、刷新或保存 Provider 凭据
- [x] Mock Provider 可以覆盖登录、绑定、重验证和好友读取
- [x] Provider 故障不会阻断 NLI 原生登录、好友或联机
- [x] 每类 Provider 错误都有确定的状态影响
- [x] 不支持后台授权的 Provider 仍能使用客户端主动操作

### Gate B：外部系统边界冻结

- [x] Adapter 接口冻结
- [x] Credential Manager 边界冻结
- [x] Binding 状态机冻结
- [x] Provider 错误分类冻结
- [x] Gate B 评审通过

---

## Phase 3：好友关系与聚合

状态：`COMPLETE / GATE_C_PASSED`

目标文档：`doc/v2/friendship.md`

依赖：Gate A、Gate B

### 设计任务

- [x] NLI 好友申请状态机
- [x] 双向好友关系
- [x] 有向 ban 及解除规则
- [x] 重复申请和反向申请
- [x] 添加、接受、拒绝、删除和恢复
- [x] 并发好友操作
- [x] Provider 外部好友投影
- [x] Provider 范围内身份解析
- [x] 未绑定外部好友表示
- [x] 单个好友同步
- [x] Provider 批量同步
- [x] 全 Provider 同步
- [x] 自动同步
- [x] 同步任务状态和进度
- [x] 好友聚合读模型
- [x] 代理实例的来源标记
- [x] Provider 失效时的查询降级
- [x] 防止通过同步接口枚举 NLI 绑定状态

### 完成标准

- [x] 任意两个 NLI Account 之间只有一个确定关系状态
- [x] 同步请求重复执行不会创建重复申请
- [x] ban 不会被自动同步清除
- [x] Provider 数据不可用时仍能查询 NLI 原生好友
- [x] 聚合结果能够解释每个条目的来源

### Gate C：社交图冻结

- [x] 好友关系状态机冻结
- [x] ban 语义冻结
- [x] 聚合读模型冻结
- [x] Provider 同步流程冻结
- [x] 隐私和枚举防护冻结
- [x] Gate C 评审通过

---

## Phase 4：Game Instance 与加入流程

状态：`COMPLETE / GATE_D_REPASSED`

目标文档：`doc/v2/game_instance.md`

依赖：Gate A、Gate C

### 设计任务

#### Game Instance

- [x] 实例创建、更新和关闭
- [x] Instance Session 绑定
- [x] 公开 Instance ID
- [x] MC Profile 客户端声明边界
- [x] 实例描述与字段限制
- [x] 有序 ACL、统一可见/可加入语义和 Approval Mode
- [x] 实例数量限制

#### 代理发布

- [x] 授权码生成和分配
- [x] 签发者与获授权用户绑定
- [x] 发布时直接使用授权码
- [x] 到期、撤销和好友解除后的失效
- [x] 客户端来源标记

#### 邀请码与 Guest

- [x] 实例邀请码生成、使用和撤销
- [x] 邀请码有效期和次数限制
- [x] Guest Session
- [x] 匿名用户公开信息
- [x] 临时封禁与限流

#### Join Request

- [x] `PENDING / ACCEPTED / REJECTED / CANCELLED / EXPIRED` 状态机
- [x] 请求者类型：NLI Account / Anonymous
- [x] 请求者公开信息
- [x] 请求方 MC Profile
- [x] Source：Friend List / Invite Code；Identity Traits：NLI Account / Direct Friend / Proxy Friend / Anonymous
- [x] 手动审批和自动同意
- [x] 实例退出时批量拒绝
- [x] 实例关闭或过期时取消请求
- [x] 接受请求时的二次鉴权

### 完成标准

- [x] 每个 Source 与 Identity Trait 都能说明数据来源、验证责任和目标可见信息
- [x] Account ID、请求者类型、Source 和 Identity Traits 由后端推导
- [x] MC Profile 始终标记为客户端声明
- [x] 实例和 Join Request 生命周期没有未定义转换
- [x] 完成权限矩阵和典型加入场景走查

### Gate D：联机授权模型冻结

- [x] Instance 生命周期冻结
- [x] 统一 ACL、可见性和邀请码语义重新冻结
- [x] 代理发布在 ACL 下的来源与身份语义重新冻结
- [x] Guest 在 ACL 下的来源与身份语义重新冻结
- [x] Join Request 与 Profile 展示语义重新冻结
- [x] Gate D 重新评审通过

---

## Phase 5：WebSocket 通知与实例保活

状态：`COMPLETE`

目标文档：`doc/v2/notifications.md`

依赖：Gate C、Gate D

### 设计任务

- [x] WebSocket 建立鉴权
- [x] WS Session ID
- [x] 新连接原子替换旧连接
- [x] 客户端主动 Ping
- [x] 服务端标准 Pong
- [x] 连接过期处理
- [x] 实例下线和 Join Request 取消
- [x] 通知事件 Envelope
- [x] Event ID 语义
- [x] 通知丢失后的 HTTP 恢复
- [x] 多实例连接限制
- [x] 服务端重启行为
- [x] 单节点和多节点路由边界
- [x] 列出初始事件类型

### 完成标准

- [x] 丢失任意通知都能通过 HTTP 恢复
- [x] WebSocket 不承担关键业务状态写入
- [x] 保活仅保存当前 WS Session ID 与连接过期时间
- [x] 旧连接不能为实例续期
- [x] 重连、过期和主动关闭场景完成走查

---

## Phase 6：REST API 契约

状态：`COMPLETE`

目标文档：`doc/v2/rest_api.md`

依赖：Gate A、Gate B、Gate C、Gate D、Phase 5

### 设计任务

- [x] `/auth/*`
- [x] `/accounts/*`
- [x] `/providers/*`
- [x] `/friends/*`
- [x] `/friend-sync/*`
- [x] `/instances/*`
- [x] `/instance-invites/*`
- [x] `/join-requests/*`
- [x] 为每个端点定义 Principal 和权限
- [x] 定义 Request/Response DTO
- [x] 定义幂等和并发语义
- [x] 定义 HTTP 状态码和业务错误码
- [x] 定义限流
- [x] 定义产生的通知事件

### 完成标准

- [x] API 不依赖数据库内部结构进行解释
- [x] 客户端不需要猜测状态转换
- [x] 所有写操作都有重试和冲突语义
- [x] 服务端推导字段不出现在可伪造的请求位置

---

## Phase 7：数据模型

状态：`COMPLETE`

目标文档：`doc/v2/data_model.md`

依赖：领域 Gate 全部通过，REST API 核心语义完成

### 设计任务

#### PostgreSQL 权威业务状态

- [x] NLI Account、认证器与 Token Family
- [x] Device、邮件凭据和短期加密重放
- [x] Provider Login Identity、Binding 与加密凭据
- [x] 好友关系、申请、ban、Projection 和 Source
- [x] Provider 同步任务
- [x] Game Instance、ACL、Proxy Grant 与 Invite
- [x] Guest Session、Join Request、Reservation 与 Report
- [x] Idempotency、审计与 Outbox

#### Redis / 易失存储

- [x] WS Session ID、Lease 与 Node 路由
- [x] EventBus Pub/Sub
- [x] 限流 Bucket 和一次性 Nonce
- [x] Browser Transaction、Singleflight 与可丢失缓存

#### 约束

- [x] 主键与唯一约束
- [x] 外键和删除行为
- [x] 索引
- [x] TTL 与清理规则
- [x] 原子操作和事务边界
- [x] 并发更新
- [x] 原始 Token 和授权码哈希存储

### 完成标准

- [x] 每个领域不变量由约束、事务或原子操作保证
- [x] 所有易失资源都有 TTL
- [x] 原始 Token、Provider 凭据和授权码不会出现在日志或明文业务表中
- [x] 数据模型不会反向污染 API 契约

---

## Phase 8：OpenAPI 契约冻结

状态：`PHASE_8_COMPLETE`

目标文件：`doc/v2/openapi.yaml`

依赖：Phase 6、Phase 7

### 设计任务

- [x] 生成全部 HTTP 路径和 Schema
- [x] 统一安全方案
- [x] 统一错误 Envelope
- [x] 检查枚举完整性
- [x] 检查分页和幂等参数
- [x] 添加主要流程示例
- [x] 检查 Provider 凭据和内部映射泄漏
- [x] 使用新上下文进行客户端可实现性阅读测试

### Gate E：v2 HTTP Contract Freeze

- [x] REST API 文档通过评审
- [x] 数据模型通过评审
- [x] OpenAPI 校验通过
- [x] 主要流程示例完整
- [x] 安全与隐私检查通过
- [x] Gate E 评审通过

> Gate E 通过后，才创建正式数据库迁移、Rust API 类型和客户端 SDK。

---

## Phase 9：P2P 信令、NAT 与 TURN

状态：`COMPLETE / GATE_F_FROZEN`

目标文档：`doc/v2/signaling.md`

依赖：Gate D、Gate E

### 设计任务

- [x] Join Request 接受后的短期连接授权
- [x] Offer / Answer / ICE 状态机
- [x] 信令消息 Envelope、ACK、重发和兼容规则
- [x] TURN 临时凭据与独立 Relay 授权设计（生产 Adapter 待实现验收）
- [x] 信令会话绝对过期、不重建与授权撤销边界
- [x] 断线和重连行为
- [x] 多节点信令路由
- [x] 消息大小、速率、队列和背压限制
- [x] SDP / ICE 隐私与日志规则

### 完成标准

- [x] 信令只服务已批准且 use-time 仍获授权的 Join Request
- [x] 信令设计不改变账号、好友和实例授权语义
- [x] 所有信令会话和 TURN 凭据均受冻结的短期或绝对期限约束
- [x] Gate F 保持 Gate E 的 77 Paths / 89 Operations 不变，只追加至 81 Paths / 94 Operations
- [x] OpenAPI、REST、数据模型和信令协议完成一致性与机械复核

### 后续实现入口（暂停）

以下仅定义恢复工作时的顺序，不代表已经开始实现：

1. PostgreSQL Migration、约束、索引、事务锁序及到期/撤销清理任务；
2. Rust REST DTO、封闭 ProblemCode、5 个 Gate F Operation 和 Signaling WS v1 Envelope；
3. LeaseStore/EventBus 的原子 Route Document、双角色检查、Fencing、限流和背压；
4. 自定义 TURN Adapter、Runtime Permit、Peer Pin 与 byte-credit；生产 Relay 默认关闭，直到通过 `B-09-TURN`；
5. 原生/桌面/Mod 客户端参考算法及真实 WebRTC/TURN、多节点、撤销、崩溃和隐私验收。

任何实现若需改变冻结契约，必须显式重开对应 Gate；不得以实现便利静默修改。

---

## 全流程场景检查

以下场景应随相关阶段逐步标记完成：

- [ ] 纯邮箱用户登录网页并通过 Device Code 登录 Mod
- [ ] 纯 Provider 注册用户通过邮箱重置密码恢复账号
- [ ] Provider 临时宕机但 NLI 原生功能正常
- [ ] Provider 授权失效并进入 `REAUTH_REQUIRED`
- [ ] Provider 重新验证成功
- [ ] Provider 好友同步为 NLI 好友申请
- [ ] 对方开启自动同步并自动接受
- [ ] 有向 ban 阻止自动同步
- [ ] 用户重新添加时主动解除自己的 ban
- [ ] 好友使用代理发布授权码发布实例
- [ ] 代理授权过期后实例停止代理展示
- [ ] NLI 好友通过好友列表加入
- [ ] NLI 非好友通过邀请码加入
- [ ] 匿名用户通过邀请码加入
- [ ] 目标实例收到请求者类型、公开信息、CLIENT_CLAIMED MC Profile、Identity Traits 和 Source
- [ ] 实例退出世界并批量拒绝旧请求
- [ ] 客户端崩溃后 WS Session 到期，实例自动下线
- [ ] WebSocket 通知丢失后通过 HTTP 恢复状态

---

## 阻塞项

| ID | 阶段 | 问题 | 负责人 | 状态 | 解除条件 |
| --- | --- | --- | --- | --- | --- |
| B-09-TURN | Phase 9 / 实现 | 独立 Relay 设计复核修正完成；自定义 Adapter 兼容性未验证 | 主设计 / 实现 | DESIGN_RESOLVED / IMPLEMENTATION_GATE | 设计可进入后续阶段；通过真实 WebRTC/TURN 验收后才可启用生产 Relay |

## 决策记录

正式结论必须同步进入对应设计文档。此表只用于索引。

| ID | 日期 | 阶段 | 决策摘要 | 正式文档位置 | 影响范围 |
| --- | --- | --- | --- | --- | --- |
| D-001 | — | Baseline | Game Instance 是唯一联机目标，不单独建模 Room | `outline.md` | Instance / Join |
| D-002 | — | Baseline | 实例归属者同时是控制者 | `outline.md` | Auth / Instance |
| D-003 | — | Baseline | NLI 不验证客户端声明的 MC Profile | `outline.md` | Instance / Join |
| D-004 | — | Baseline | WebSocket 由客户端主动 Ping，保活只记录 WS Session ID 与过期时间 | `outline.md` | Notifications |
| D-005 | — | Phase 0 | 持久业务资源使用 UUIDv4，安全凭据使用独立高熵不透明值 | `common.md` | All modules |
| D-006 | — | Phase 0 | HTTP JSON 使用 snake_case，枚举默认使用 UPPER_SNAKE_CASE | `common.md` | HTTP / Client |
| D-007 | — | Phase 0 | JSON 时间戳与时长使用 Unix 毫秒整数 | `common.md` | HTTP / Storage |
| D-008 | — | Phase 0 | API 使用 /v2 路径版本 | `common.md` | HTTP routing |
| D-009 | — | Phase 0 | 错误使用 RFC 9457 Problem Details 并扩展 code/request_id | `common.md` | Error handling |
| D-010 | — | Phase 0 | 成功响应直接返回资源，列表使用 items/next_cursor | `common.md` | HTTP / Client |
| D-011 | — | Phase 0 | ~~Account、Instance、Guest 使用统一 Audience 并通过 Scope 隔离~~（已由 D-028 取代） | `common.md` | Auth / All modules |
| D-012 | — | Phase 0 | 服务端拒绝未知请求字段，客户端忽略未知响应字段 | `common.md` | HTTP / Client |
| D-013 | — | Phase 0 | 使用 POST 命令与 PUT 完整替换，不定义通用 PATCH | `common.md` | HTTP writes |
| D-014 | — | Phase 0 | Request ID 始终由服务端生成 UUIDv4 | `common.md` | HTTP / Logging |
| D-015 | — | Phase 0 | 400 处理语法与类型错误，422 处理语义校验，409 处理状态冲突 | `common.md` | Error handling |
| D-016 | — | Phase 0 | 仅文档标记的非天然幂等写操作强制 Idempotency-Key，默认保留 24 小时 | `common.md` | HTTP writes |
| D-017 | — | Phase 0 | Refresh Token 轮换在短窗口内幂等返回同一结果 | `common.md` | Auth |
| D-018 | — | Phase 0 | 集合统一使用不透明 Cursor，默认 20、最大 100，采用弱一致 Keyset 语义 | `common.md` | Queries / Client |
| D-019 | — | Phase 0 | 写操作原子检查当前状态，冲突返回 409，不要求全局 ETag | `common.md` | HTTP writes |
| D-020 | — | Phase 0 | 创建返回 201 + 可用时的 Location；可确认权限时重复 DELETE 返回 204 | `common.md` | HTTP writes |
| D-021 | — | Phase 0 | 429 返回 Retry-After 与 retry_after_ms，不公开内部剩余额度 | `common.md` | Rate limiting |
| D-022 | — | Phase 0 | 普通应用日志不记录请求或响应 Body | `common.md` | Observability |
| D-023 | — | Phase 0 | IP、邮箱和 MC Profile 不进入普通日志；调查所需标识只进入最小化安全审计 | `common.md` | Privacy / Security |
| D-024 | — | Phase 0 | 安全审计与普通应用日志分离并采用追加式记录 | `common.md` | Audit |
| D-025 | — | Phase 0 | 字段错误使用 Problem Details errors 数组和 JSON Pointer | `common.md` | Validation errors |
| D-026 | — | Phase 0 | 服务端错误文本固定英文，客户端按稳定 code 本地化 | `common.md` | Client / Errors |
| D-027 | — | Phase 0 | Problem type 使用 urn:netherlink:problem:<code> | `common.md` | Errors |
| D-028 | — | Phase 0 修订 | NLI Account Session 与 Instance Session 共用 nli_account Audience 和普通 API 权限；Guest 使用 nli_guest；未来管理后台保留 nli_admin | `common.md`、`outline.md` | Auth / Guest / Admin |
| D-029 | — | Phase 0 修订 | 每个 NLI Token Family 具有临时 Session ID，最多绑定一个 Game Instance；普通用户 API 不使用细粒度 Scope | `common.md`、`outline.md` | Auth / Instance |
| D-030 | — | Phase 0 修订 | Guest ID 与匿名游戏运行时生命周期绑定，只签发 nli_guest 短期 Access Token，不签发长期 Refresh Token | `common.md`、`outline.md` | Guest / Join |
| D-031 | — | Phase 1 | 账号使用唯一小写 username 定位和非唯一 Unicode display_name 展示；密码登录仅接受邮箱 | `nli_account.md` | Account / Client |
| D-032 | — | Phase 1 | 每个有效账号必须具有已验证邮箱；邮箱整体大小写不敏感且不执行 Provider 别名合并 | `nli_account.md` | Account / Recovery |
| D-033 | — | Phase 1 | 注册先创建 24 小时 PENDING_EMAIL 账号；可信 Provider 的 verified email 可满足验证 | `nli_account.md` | Registration / Provider |
| D-034 | — | Phase 1 | 密码为 10–128 Unicode 字符且不要求组合规则；使用可升级参数的 Argon2id | `nli_account.md` | Password auth |
| D-035 | — | Phase 1 | username 改名冷却 30 天，旧名为原账号保留 90 天 | `nli_account.md` | Account discovery |
| D-036 | — | Phase 1 | 账号状态为 PENDING_EMAIL/ACTIVE/DISABLED/DELETION_PENDING/DELETED | `nli_account.md` | Account lifecycle |
| D-037 | — | Phase 1 | 主动删除进入 30 天宽限期并立即撤销会话、隐藏账号 | `nli_account.md` | Deletion / Recovery |
| D-038 | — | Phase 1 | Access/Refresh Token 使用不透明随机值；Access 15 分钟，Refresh 空闲 30 天、绝对 90 天 | `nli_account.md` | Token lifecycle |
| D-039 | — | Phase 1 | 每账号最多 10 个 Token Family、5 个并发绑定实例；达到上限拒绝新会话 | `nli_account.md` | Session limits |
| D-040 | — | Phase 1 | Token Family 同时只绑定一个实例，但实例关闭后可解绑并复用于后续实例 | `nli_account.md`、`common.md`、`outline.md` | Session / Instance |
| D-041 | — | Phase 1 | Refresh 结果可重放 60 秒；超窗旧 Refresh 重用只撤销对应 Family 并通知用户 | `nli_account.md` | Token security |
| D-042 | — | Phase 1 | Refresh 后旧 Access Token 立即失效，只有成功 Refresh 更新 30 天空闲期 | `nli_account.md` | Token lifecycle |
| D-043 | — | Phase 1 | Session ID 对账号本人可见；支持单独撤销和保留当前会话撤销其他全部 | `nli_account.md` | Session management |
| D-044 | — | Phase 1 | Session 列表返回清理后的 client_name 和服务端时间/绑定状态，不返回 IP | `nli_account.md` | Session / Privacy |
| D-045 | — | Phase 1 | Device Code Flow 采用 RFC 8628 兼容字段和轮询错误，10 分钟有效、初始 5 秒轮询 | `nli_account.md` | Device auth |
| D-046 | — | Phase 1 | User Code 使用 8 位 Crockford Base32，浏览器必须显示 client_name 并显式批准 | `nli_account.md` | Device auth / UX |
| D-047 | — | Phase 1 | Token Family 在 Mod 首次成功消费时创建，消费时重新检查 Session 上限 | `nli_account.md` | Device auth / Session |
| D-048 | — | Phase 1 | Device Code 消费结果允许 60 秒重放同一 Token Pair，过快轮询使用 slow_down | `nli_account.md` | Device auth / Retry |
| D-049 | — | Phase 1 | 邮箱验证和密码重置使用 30 分钟高熵一次性链接；重发使旧链接失效 | `nli_account.md` | Email / Recovery |
| D-050 | — | Phase 1 | 修改密码保留当前 Session 并撤销其他全部；密码重置撤销全部 Session | `nli_account.md` | Password / Session |
| D-051 | — | Phase 1 | 邮箱变更要求近期重新认证、验证新邮箱并通知旧邮箱 | `nli_account.md` | Email security |
| D-052 | — | Phase 1 | 删除恢复要求原邮箱链接并设置新密码；DISABLED 只能人工恢复 | `nli_account.md` | Account recovery |
| D-053 | — | Phase 1 | 邮件发送最小间隔 60 秒，同邮箱每小时 5 封、同 IP 每小时 20 封 | `nli_account.md` | Abuse controls |
| D-054 | — | Phase 1 | Gate A 通过：账号、Session、Token、Device Code 和恢复语义冻结 | `nli_account.md` | Phase gate |
| D-055 | — | Phase 2 | Provider 只能由管理员注册启用；未注册站点只能由 Mod 本地使用 | `provider.md` | Provider registry / Security |
| D-056 | — | Phase 2 | Provider ID 使用稳定小写 slug，作为 UUIDv4 公共资源规则的配置标识例外 | `provider.md` | Provider registry |
| D-057 | — | Phase 2 | 有效能力是 Provider 声明、Binding 用途开关和 Credential Grant 的交集 | `provider.md` | Capability / Authorization |
| D-058 | — | Phase 2 | Provider Adapter 使用编译期强类型异步实现和运行时配置实例 | `provider.md` | Adapter / Deployment |
| D-059 | — | Phase 2 | 业务模块不能向 Adapter 传递任意 URL/Method/Header/JSON | `provider.md` | Security boundary |
| D-060 | — | Phase 2 | 一个 NLI Account 对同一 Provider 最多一个有效业务 Binding | `provider.md` | Binding |
| D-061 | — | Phase 2 | Provider Login Identity 与业务 Binding 分离，以 Provider+Issuer+Subject 关联 | `provider.md` | Login / Binding |
| D-062 | — | Phase 2 | 业务模块统一调用 ProviderService，由其内部协调 Registry、Credential Manager 和 Adapter | `provider.md` | Provider boundary |
| D-063 | — | Phase 2 | 长期凭据使用版本化主密钥和每记录独立 AEAD Nonce 加密 | `provider.md` | Credential security |
| D-064 | — | Phase 2 | Refresh Grant 加密持久化，Access Token 默认仅进程内短期缓存 | `provider.md` | Credential storage |
| D-065 | — | Phase 2 | Provider 刷新采用按 Binding 的本地 Singleflight 和多节点短租约锁 | `provider.md` | Refresh concurrency |
| D-066 | — | Phase 2 | Access Token 按需惰性刷新，不周期性刷新全部 Binding | `provider.md` | Refresh policy |
| D-067 | — | Phase 2 | 无长期 Refresh Grant 的 Provider 仅支持用户在场操作，不保存密码或要求客户端上传 Token | `provider.md` | Background capability |
| D-068 | — | Phase 2 | 解绑先使本地凭据失效，上游撤销尽力执行且失败不恢复本地状态 | `provider.md` | Revocation |
| D-069 | — | Phase 2 | Registry 状态为 ENABLED/NO_NEW_BINDINGS/DISABLED，禁用不批量改写 Binding 状态 | `provider.md` | Provider lifecycle |
| D-070 | — | Phase 2 | Binding 状态为 ACTIVE/REAUTH_REQUIRED/DISABLED；用户禁用保留加密 Grant | `provider.md` | Binding lifecycle |
| D-071 | — | Phase 2 | 仅读取和明确幂等 Provider 操作最多自动重试 2 次，写入未知结果不重放 | `provider.md` | Retry safety |
| D-072 | — | Phase 2 | 熔断按 Provider+操作类别隔离 | `provider.md` | Resilience |
| D-073 | — | Phase 2 | 重验证必须保持同一 Subject；不一致返回 SUBJECT_MISMATCH 并要求显式换绑 | `provider.md` | Identity security |
| D-074 | — | Phase 2 | 解绑移除业务凭据/投影但保留 NLI 好友和独立 Login Identity | `provider.md` | Unbind semantics |
| D-075 | — | Phase 2 | Provider 浏览器事务使用一次性 state、支持时 PKCE、精确 Redirect 和 10 分钟 TTL | `provider.md` | OAuth security |
| D-076 | — | Phase 2 | Provider 永久停用保留 Provider ID/历史 Binding，不自动删除 NLI 好友或静默迁移 Subject | `provider.md` | Provider retirement |
| D-077 | — | Phase 2 | Mock Provider 必须经过真实 ProviderService/Credential Manager 边界并支持能力/错误/延迟注入 | `provider.md` | Testing |
| D-078 | — | Phase 2 | ProviderService 区分认证事务入口和 Binding 业务操作入口，登录不要求业务 Binding | `provider.md` | Authentication / Provider boundary |
| D-079 | — | Phase 2 | Gate B 通过：Registry、Adapter、Credential Manager、Binding、错误与重验证边界冻结 | `provider.md` | Phase gate |
| D-080 | — | Phase 3 | 每个无序 Account Pair 使用规范化单行和唯一约束 | `friendship.md` | Friendship model |
| D-081 | — | Phase 3 | 同方向重复申请幂等；反向申请视为接受并转为 FRIENDS | `friendship.md` | Request state |
| D-082 | — | Phase 3 | 普通拒绝不设 ban，拒绝并屏蔽才设置接收方有向 ban | `friendship.md` | Ban semantics |
| D-083 | — | Phase 3 | 好友申请 30 天过期；旧 Request ID 不能作用于新 pending | `friendship.md` | Request lifecycle |
| D-084 | — | Phase 3 | 有向 ban 阻止对方申请和双方自动同步，仅设置者可显式清除 | `friendship.md` | Ban / Recovery |
| D-085 | — | Phase 3 | 删除宽限期内社交图保留但隐藏，恢复后恢复可见，永久删除后清理 | `friendship.md` | Account lifecycle |
| D-086 | — | Phase 3 | Pair 写入使用唯一约束、行锁、Request ID 复核和事务 Outbox | `friendship.md` | Concurrency |
| D-087 | — | Phase 3 | 外部投影按 Binding+规范化 Subject 唯一，Subject 加密保存并使用 keyed HMAC 同域索引 | `friendship.md` | Projection / Privacy |
| D-088 | — | Phase 3 | 外部投影 15 分钟内 FRESH，之后 STALE，30 天未确认删除 | `friendship.md` | Projection freshness |
| D-089 | — | Phase 3 | 客户端使用 Binding 范围 opaque External Friend ID，不接触原始 Subject/HMAC | `friendship.md` | External identity privacy |
| D-090 | — | Phase 3 | 只有本次读取或 15 分钟内可信缓存可证明 Provider 好友关系 | `friendship.md` | Sync proof |
| D-091 | — | Phase 3 | Provider 同步 pending 接受前不向发起方显示目标 NLI 身份 | `friendship.md` | Anti-enumeration |
| D-092 | — | Phase 3 | 单个、批量、全 Provider 和自动同步统一为异步 Task，单个也只返回 opaque Task ID | `friendship.md` | Sync / Privacy |
| D-093 | — | Phase 3 | 相同账号与同步 Scope 的活动任务合并；Idempotency Key 仍保留 24 小时 | `friendship.md` | Task deduplication |
| D-094 | — | Phase 3 | 自动同步默认最短周期 6 小时，Provider 批量同步硬下限 15 分钟 | `friendship.md` | Scheduling / Rate limit |
| D-095 | — | Phase 3 | Mutual 关系一侧新鲜证明可自动接受；方向性关系要求双方新鲜证明 | `friendship.md` | Auto accept proof |
| D-096 | — | Phase 3 | 自动接受要求接收方 Account 接收开关和对应 Binding auto_sync 同时开启 | `friendship.md` | Consent |
| D-097 | — | Phase 3 | 每 Provider 子任务最多处理 500 条，超出返回服务端不透明 Continuation | `friendship.md` | Task bounds |
| D-098 | — | Phase 3 | 取消同步不回滚已提交关系变化 | `friendship.md` | Cancellation |
| D-099 | — | Phase 3 | 同步任务仅公开运行统计，不返回 matched/blocked/unbound/auto-accepted 数量 | `friendship.md` | Enumeration defense |
| D-100 | — | Phase 3 | 外部投影仅在已有 NLI 好友且具备对应 Relationship Source 时合并 | `friendship.md` | Aggregation / Privacy |
| D-101 | — | Phase 3 | 聚合 GET 不实时调用 Provider，只读原生数据和投影 | `friendship.md` | Query boundary |
| D-102 | — | Phase 3 | 部分 Provider 故障返回 200 和 source_statuses，不阻断 NLI 好友 | `friendship.md` | Partial degradation |
| D-103 | — | Phase 3 | 聚合列表 NLI 优先并按规范化展示名+稳定 ID 排序，使用 10 分钟查询快照 Cursor | `friendship.md` | Pagination |
| D-104 | — | Phase 3 | 用户只可查看和管理自己设置的 ban | `friendship.md` | Ban privacy |
| D-105 | — | Phase 3 | 接受后保留 Relationship Source；外部关系解除只将来源标记历史 | `friendship.md` | Provenance |
| D-106 | — | Phase 3 | 代理实例显式返回 PROXY、真实 owner_account_id 和 presented_under_account_id | `friendship.md` | Proxy attribution |
| D-107 | — | Phase 3 | 不按显示信息或内部解析自动合并跨 Provider 外部条目 | `friendship.md` | Identity privacy |
| D-108 | — | Phase 3 | 普通拒绝后相同申请方 24 小时内的新申请静默抑制 | `friendship.md` | Anti-harassment |
| D-109 | — | Phase 3 | 可见搜索 outgoing 上限 100；隐藏 Provider outgoing 和 incoming 分别上限 500 | `friendship.md` | Abuse prevention / Privacy |
| D-110 | — | Phase 3 | Gate C 通过：Pair、ban、Provider 投影、同步任务、聚合和枚举边界冻结 | `friendship.md` | Phase gate |
| D-111 | — | Phase 4 | Instance 生命周期 ACTIVE/CLOSED 与 ONLINE/OFFLINE 租约状态分离 | `game_instance.md` | Instance lifecycle |
| D-112 | — | Phase 4 | WS 租约失效立即 OFFLINE，10 分钟内同 Session 可重连，超时自动关闭并解绑 | `game_instance.md` | Liveness / Cleanup |
| D-113 | — | Phase 4 | 请求者归入 FRIEND/NON_FRIEND_ACCOUNT/ANONYMOUS 互斥类别集合 | `game_instance.md` | Join authorization |
| D-114 | — | Phase 4 | hidden 实例不进入任何列表，只能通过有效邀请码直接寻址 | `game_instance.md` | Visibility |
| D-115 | — | Phase 4 | Instance Session 可更新现有 Profile/版本/描述/配置，不创建世界版本 | `game_instance.md` | Instance mutation |
| D-116 | — | Phase 4 | MC Profile source/UUID/username 始终 CLIENT_CLAIMED；v2 不接收客户端 MC 头像 | `game_instance.md` | MC identity boundary |
| D-117 | — | Phase 4 | description 0–128 Unicode，兼容字段 1–64 ASCII，不接受任意 Metadata JSON | `game_instance.md` | Field constraints |
| D-118 | — | Phase 4 | 只有绑定实例的同一 Token Family 可管理实例；其他同账号 Session 无权管理 | `game_instance.md` | Instance ownership |
| D-119 | — | Phase 4 | Instance 默认非隐藏、不可加入、关闭自动同意，仅允许 FRIEND 类别 | `game_instance.md` | Secure defaults |
| D-120 | — | Phase 4 | Proxy Grant 同时最多绑定 Grantee 的一个 ACTIVE Instance；关闭/解绑后可复用 | `game_instance.md` | Proxy scope |
| D-121 | — | Phase 4 | 一个 Instance 同时最多代理展示到一个 Presented-Under Account | `game_instance.md` | Proxy attribution |
| D-122 | — | Phase 4 | Owner Instance Session 可在创建或更新时绑定、切换或解除代理 | `game_instance.md` | Proxy lifecycle |
| D-123 | — | Phase 4 | Presented-Under Account 的好友经有效代理路径归类 FRIEND，但不绕过其他检查 | `game_instance.md` | Join authorization |
| D-124 | — | Phase 4 | Issuer/Grantee 好友解除使 Grant 终止失效，重新加好友不恢复 | `game_instance.md` | Revocation |
| D-125 | — | Phase 4 | Secret 轮换只阻止新绑定，不中断现有 Instance；撤销才终止展示 | `game_instance.md` | Secret rotation |
| D-126 | — | Phase 4 | 仅 Issuer 可列表/轮换/撤销 Grant；Grantee 绑定后只见非秘密摘要 | `game_instance.md` | Grant visibility |
| D-127 | — | Phase 4 | Proxy Grant 允许不设到期时间，也可选择有限到期；无到期 Grant 持续占额度 | `game_instance.md` | Grant expiry |
| D-128 | — | Phase 4 | Invite Secret 仅创建/轮换显示一次；Owner 列表只返回元数据 | `game_instance.md` | Invite secret |
| D-129 | — | Phase 4 | Invite 使用上限必填 1–100、默认 1；创建 Request 时预留，ACCEPTED 消费，其他终态释放 | `game_instance.md` | Invite usage |
| D-130 | — | Phase 4 | 每实例最多 3 个 Invite，默认 24 小时、最大 7 天 | `game_instance.md` | Invite limits |
| D-131 | — | Phase 4 | Guest Session 绝对 5 分钟，Join 终态后最多再保留 60 秒 | `game_instance.md` | Guest lifetime |
| D-132 | — | Phase 4 | Guest 绑定一个 Instance/Invite 且最多一个 Join Request | `game_instance.md` | Guest scope |
| D-133 | — | Phase 4 | 匿名公开资料仅含 Guest ID 与 CLIENT_CLAIMED MC Profile，不含独立昵称/头像 | `game_instance.md` | Guest privacy |
| D-134 | — | Phase 4 | Guest ID ban 仅持续至 Session 结束；IP 只用于最长 24 小时服务端临时控制 | `game_instance.md` | Anonymous abuse control |
| D-135 | — | Phase 4 | Join 路径为 DIRECT_FRIEND/PROXY_FRIEND/PUBLIC_ACCOUNT/INVITE_CODE，身份类别独立推导 | `game_instance.md` | Join path |
| D-136 | — | Phase 4 | 手动审批期间 Source MC Profile 变化会取消旧 Request 并要求重建 | `game_instance.md` | Approval consistency |
| D-137 | — | Phase 4 | ACCEPTED 使用 Request ID+双方 Session 的 60 秒 Acceptance Lease，不签发独立 Ticket | `game_instance.md` | Join authorization |
| D-138 | — | Phase 4 | 同 requester session+target 单 PENDING；源实例 outgoing 3、目标 incoming 100 | `game_instance.md` | Pending limits |
| D-139 | — | Phase 4 | joinable/身份类别/Invite/Proxy/好友关系/在线状态失效时取消受影响 pending | `game_instance.md` | Invalidation |
| D-140 | — | Phase 4 | auto_accept 在同一事务中创建并直接 ACCEPTED，不暴露 PENDING 窗口 | `game_instance.md` | Auto accept |
| D-141 | — | Phase 4 | Join 拒绝原因仅允许 DECLINED/BUSY/NOT_ACCEPTING 固定枚举 | `game_instance.md` | Response safety |
| D-142 | — | Phase 4 | NLI Join 终态可查 24 小时、Guest 60 秒、最小安全审计 30 天 | `game_instance.md` | Retention |
| D-143 | — | Phase 4 | Instance 黑名单区分可靠 Account、CLIENT_CLAIMED Profile 和短期 Guest ID | `game_instance.md` | Blacklist boundary |
| D-144 | — | Phase 4 | Guest 被封禁后仅可在保留窗口读取固定结果，不能创建/取消请求或使用 Lease | `game_instance.md` | Guest result recovery |
| D-145 | — | Phase 4 | Gate D 通过：Instance、Proxy、Invite、Guest、Join 状态机与授权边界冻结 | `game_instance.md` | Phase gate |
| D-146 | — | Phase 4 修订 | 重新打开 Gate D：用统一 ACL 替代 hidden/joinable/身份类别/黑名单分散控制，并弱化 Profile 语义 | `game_instance.md` | Gate reopened |
| D-147 | — | Phase 4 修订 | ACL 按 priority 首条匹配生效，无匹配默认 DENY | `game_instance.md` | ACL evaluation |
| D-148 | — | Phase 4 修订 | Matcher 除 Action 外空项表示任意；字段间 AND，同字段多值 OR | `game_instance.md` | ACL matcher |
| D-149 | — | Phase 4 修订 | Identity Traits 仅为 NLI_ACCOUNT/DIRECT_FRIEND/PROXY_FRIEND/ANONYMOUS；无 NLI 即 Anonymous | `game_instance.md` | Identity model |
| D-150 | — | Phase 4 修订 | Source 仅为服务端验证的 FRIEND_LIST/INVITE_CODE；无 PUBLIC_ACCOUNT 或第三方联机来源 | `game_instance.md` | Source model |
| D-151 | — | Phase 4 修订 | Subject 使用 typed NLI_ACCOUNT_ID 与 MC_USERNAME，禁止自动命名空间推断 | `game_instance.md` | ACL subjects |
| D-152 | — | Phase 4 修订 | MC_USERNAME 可用于 ALLOW/DENY，但必须标记 WEAK_CLIENT_CLAIMED 并提示可伪造 | `game_instance.md` | Weak matcher |
| D-153 | — | Phase 4 修订 | ACL ALLOW 同时允许发现与创建 Request；DENY/无匹配同时不可见不可加入 | `game_instance.md` | Unified access |
| D-154 | — | Phase 4 修订 | approval_mode MANUAL/AUTO 与 ACL 分离，AUTO 仍执行完整 ACL | `game_instance.md` | Approval |
| D-155 | — | Phase 4 修订 | Join 保存 CLIENT_CLAIMED Profile 快照；Profile 变化不取消 Request 或影响可靠授权 | `game_instance.md` | Profile semantics |
| D-156 | — | Phase 4 修订 | 恶意行为通过举报通道处理，MOD 可联机后本地二次校验但不升级后端身份保证 | `game_instance.md` | Abuse response |
| D-157 | — | Phase 4 修订 | Gate D 重新通过：ACL、Source、Identity Traits、Profile 展示和原 Join 生命周期冻结 | `game_instance.md` | Phase gate |
| D-158 | — | Phase 5 | 只有 ACTIVE_BOUND Instance Session 可建立 WebSocket；Guest/Account Session 仅用 HTTP | `notifications.md` | WS principal |
| D-159 | — | Phase 5 | 握手 Token 只允许 Authorization Bearer Header，禁止 Query/Cookie/Subprotocol | `notifications.md` | Token transport |
| D-160 | — | Phase 5 | 连接可跨 Access Token 自然到期和 Refresh 轮换，但续租持续检查 Family/Account/Instance | `notifications.md` | Connection auth |
| D-161 | — | Phase 5 | WS Session ID 为客户端可见 UUIDv4，仅作相关性与 Fencing，不是凭据 | `notifications.md` | WS session |
| D-162 | — | Phase 5 | 新连接原子替换 current_ws_session_id；每次 Ping 比较自身 Session ID | `notifications.md` | Fencing |
| D-163 | — | Phase 5 | 旧连接尽力收到 connection.replaced 并以应用关闭码 4001 关闭 | `notifications.md` | Replacement |
| D-164 | — | Phase 5 | 每 Instance 一个 current WS；每 Account 最多随 5 个 ACTIVE Instance 拥有 5 个连接 | `notifications.md` | Connection limit |
| D-165 | — | Phase 5 | 握手 CAS 成功立即 ONLINE 并设置 90 秒租约，不等待首个 Ping | `notifications.md` | Online transition |
| D-166 | — | Phase 5 | 迟于 connection_expires_at 的 Ping 不能复活旧连接，必须重新握手 | `notifications.md` | Lease expiry |
| D-167 | — | Phase 5 | 客户端只发送标准 WS Ping；服务端标准 Pong，禁止客户端业务 Data Frame | `notifications.md` | Ping protocol |
| D-168 | — | Phase 5 | 当前连接关闭立即 OFFLINE；被替换旧连接关闭不得影响新 Session | `notifications.md` | Close semantics |
| D-169 | — | Phase 5 | Pong 可回复但租约共享写入最多每 10 秒一次，高频滥用以 1008 关闭 | `notifications.md` | Write coalescing |
| D-170 | — | Phase 5 | 多节点租约使用共享权威 UTC 毫秒，节点单调时钟只作本地调度 | `notifications.md` | Clock semantics |
| D-171 | — | Phase 5 | OFFLINE 立即取消 PENDING；10 分钟无新 Session 才关闭 Instance 并解绑 | `notifications.md` | Grace period |
| D-172 | — | Phase 5 | 业务通知只携带资源变化提示，不嵌入完整 DTO 或 MC Profile | `notifications.md` | Event payload |
| D-173 | — | Phase 5 | 投递为 best-effort，允许丢失/重复/乱序，不设计客户端 ACK | `notifications.md` | Delivery semantics |
| D-174 | — | Phase 5 | Event ID 为事务 Outbox UUIDv4，Dispatcher 重试和多实例 Fanout 复用 | `notifications.md` | Event identity |
| D-175 | — | Phase 5 | 不提供全局/账号 sequence，同 Socket 仅尽力按发送顺序 | `notifications.md` | Ordering |
| D-176 | — | Phase 5 | Account 事件 Fanout 到账号全部 ONLINE Instance，使用相同 Event ID | `notifications.md` | Fanout |
| D-177 | — | Phase 5 | Event Envelope 显式包含 ACCOUNT/INSTANCE/RESOURCE Scope | `notifications.md` | Recovery hint |
| D-178 | — | Phase 5 | 发送队列超过 256 条或 1 MiB 时以 4003 关闭并要求 HTTP resync | `notifications.md` | Backpressure |
| D-179 | — | Phase 5 | 未知事件/字段可忽略，并按 Scope 执行保守 HTTP 刷新 | `notifications.md` | Forward compatibility |
| D-180 | — | Phase 5 | 不提供 Event Replay/Last-Event-ID；每次 connection.ready 后执行完整 HTTP 恢复 | `notifications.md` | Recovery |
| D-181 | — | Phase 5 | 长连接不强制周期刷新；事件静默丢失可在下一事件、重连或主动刷新恢复 | `notifications.md` | Client refresh |
| D-182 | — | Phase 5 | LeaseStore 故障时 fail-closed，不退化到节点本地造成 current Session 脑裂 | `notifications.md` | Shared state failure |
| D-183 | — | Phase 5 | 优雅重启使用 1012 并 compare-delete current Session；崩溃最多等待 90 秒租约 | `notifications.md` | Restart |
| D-184 | — | Phase 5 | Outbox 发布到内部路由即 PUBLISHED，不声称客户端收到；完成记录保留 24 小时 | `notifications.md` | Outbox semantics |
| D-185 | — | Phase 5 | 使用 LeaseStore+EventBus 抽象；单节点内存实现，多节点共享 CAS/TTL/PubSub 实现 | `notifications.md` | Deployment boundary |
| D-186 | — | Phase 5 收尾 | 生产环境只接受 WSS；Origin 按部署允许列表检查；握手按 IP/Account/Family 限流 | `notifications.md` | Transport security |
| D-187 | — | Phase 5 收尾 | LeaseStore 与业务库跨事务使用 Instance 锁、Session ID 复核、补偿删除和协调任务收敛 | `notifications.md` | Cross-store consistency |
| D-188 | — | Phase 5 收尾 | connection.ready 必须先于该连接的业务 Event；CAS 到 ready 的缺口由强制 HTTP 恢复覆盖 | `notifications.md` | Startup ordering |
| D-189 | — | Phase 5 收尾 | Phase 5 完成：鉴权、租约、事件、恢复、多节点和故障场景冻结 | `notifications.md` | Phase completion |
| D-190 | — | Phase 6 | Auth 使用常见动作 Path，业务资源保持 REST 风格 | `rest_api.md` | Path style |
| D-191 | — | Phase 6 | 邮箱注册/验证不签发 Session，激活后显式登录 | `rest_api.md` | Registration |
| D-192 | — | Phase 6 | 邮件 Secret 由前端从链接取出后通过 JSON Body POST；成功结果重放 60 秒 | `rest_api.md` | Email credentials |
| D-193 | — | Phase 6 | Token 响应含 Token Pair、Session 摘要和绝对过期时间 | `rest_api.md` | Token DTO |
| D-194 | — | Phase 6 | Device Code Mod 端点使用 RFC 8628 Form/错误；浏览器批准使用普通 JSON API | `rest_api.md` | Device API |
| D-195 | — | Phase 6 | Provider Login 与业务 Binding 分离；回调使用短期 HttpOnly Cookie 后 POST complete | `rest_api.md` | Provider auth |
| D-196 | — | Phase 6 | 未关联 Provider LOGIN 不自动注册，只有显式 REGISTER 才创建账号 | `rest_api.md` | Provider registration |
| D-197 | — | Phase 6 | 当前 Family 维护 5 分钟 reauthenticated_at；邮件 reauth 同时绑定 Secret 与 Access Token | `rest_api.md` | Recent auth |
| D-198 | — | Phase 6 | 登录要求 Idempotency-Key，Token 结果仅加密重放 60 秒 | `rest_api.md` | Login retry |
| D-199 | — | Phase 6 | 当前 Session 可直接 DELETE；revoke-others 由服务端保留当前 Family | `rest_api.md` | Session API |
| D-200 | — | Phase 6 | Account 精确 username/ID 查询仅显示 ACTIVE 且无任一方向 ban 的最小公开资料 | `rest_api.md` | Account discovery |
| D-201 | — | Phase 6 | PUBLIC Registry 隐藏 DISABLED；既有 Binding 用户仍可读取其 Provider 降级状态 | `rest_api.md` | Registry visibility |
| D-202 | — | Phase 6 | Binding 授权使用 HttpOnly Browser Cookie + Completion，CREATE/REAUTH/REPLACE 明确分离 | `rest_api.md` | Binding auth |
| D-203 | — | Phase 6 | Binding 用 PUT + revision 完整替换用途开关 | `rest_api.md` | Binding update |
| D-204 | — | Phase 6 | 好友提交统一返回 24 小时 Submission Receipt，不直接暴露 Pending 是否创建 | `rest_api.md`、`friendship.md` | Enumeration defense |
| D-205 | — | Phase 6 | Friend Request 命令返回可重放的当前关系/请求结果摘要 | `rest_api.md` | Request commands |
| D-206 | — | Phase 6 | `/friends` 保持 NLI_ACCOUNT/EXTERNAL_PROVIDER 判别联合聚合列表 | `rest_api.md` | Friends read model |
| D-207 | — | Phase 6 | Friend Sync 创建使用 mode=ALL/ONE 严格判别联合并统一返回 202 Task | `rest_api.md` | Sync API |
| D-208 | — | Phase 6 | Instance 普通配置与 ACL 分别 PUT 并各自使用 Revision | `rest_api.md` | Instance update |
| D-209 | — | Phase 6 | Instance 创建可选在同一事务绑定 Proxy Grant Secret | `rest_api.md` | Instance creation |
| D-210 | — | Phase 6 | `/instances` 只列 Owner 自有资源；好友和 Invite 使用各自 Source 入口 | `rest_api.md` | Instance discovery |
| D-211 | — | Phase 6 | ACL PUT 使用 Body acl_revision+Idempotency-Key；已有 Rule 回传 ID，新 Rule 服务端生成 | `rest_api.md` | ACL API |
| D-212 | — | Phase 6 | Proxy/Invite Secret 创建与轮换可加密重放 60 秒 | `rest_api.md` | Secret retry |
| D-213 | — | Phase 6 | NLI Invite 解析签发 60 秒、绑定当前 Instance Session 的 Resolution ID | `rest_api.md` | Invite resolution |
| D-214 | — | Phase 6 | Instance WebSocket Upgrade Path 固定为 `/v2/instances/{instance_id}/ws` | `rest_api.md` | WS path |
| D-215 | — | Phase 6 | Join Body 只含 Target ID 与可选 Invite Resolution；Source/Traits/Profile 均由服务端推导 | `rest_api.md` | Join input |
| D-216 | — | Phase 6 | MANUAL/AUTO 创建统一 201 返回 PENDING/ACCEPTED Request DTO | `rest_api.md` | Join response |
| D-217 | — | Phase 6 | Acceptance Lease 无独立 Validation API；Phase 9 真正使用时按 Request ID 重验 | `rest_api.md` | Lease API |
| D-218 | — | Phase 6 | Requester 列表仅限当前 Source Instance，Account 其他 Session 不可读取 | `rest_api.md` | Join reads |
| D-219 | — | Phase 6 | Guest 创建返回 Access Token/Guest/Instance 摘要并加密重放 60 秒 | `rest_api.md` | Guest API |
| D-220 | — | Phase 6 | Report 必须引用调用者参与的 Join Request，目标与 Profile 快照由服务端推导 | `rest_api.md` | Report context |
| D-221 | — | Phase 6 | Report 只返回 RECEIVED Receipt，不提供用户侧审核状态或处罚结果 | `rest_api.md` | Report privacy |
| D-222 | — | Phase 6 收敛 | Principal 矩阵区分未绑定 Account、绑定 Instance、Guest 与专用流程凭据 | `rest_api.md` | Authorization matrix |
| D-223 | — | Phase 6 收敛 | 返回 Secret 的操作统一只加密重放 60 秒，普通幂等摘要保留 24 小时 | `rest_api.md` | Secret idempotency |
| D-224 | — | Phase 6 收敛 | Instance/ACL/Binding/Login Identity 使用 Revision；其他状态机用锁和唯一约束 | `rest_api.md` | Concurrency |
| D-225 | — | Phase 6 收敛 | 普通 Body 64 KiB、ACL 256 KiB、Report 8 KiB，并显式使用 413 | `rest_api.md` | Payload limits |
| D-226 | — | Phase 6 收敛 | 每个业务 HTTP 事务映射到 Phase 5 Event Type/Scope/Recipient；Report 无用户通知 | `rest_api.md` | Notification map |
| D-227 | — | Phase 6 收敛 | Provider Login Identity 提供独立列表、启停、删除与 LINK 流程，不影响业务 Binding | `rest_api.md` | Login identity API |
| D-228 | — | Phase 6 收尾 | Phase 6 完成：89 个 Path/Method、Principal/DTO/错误/幂等/限流/通知映射与场景冻结 | `rest_api.md` | Phase completion |
| D-229 | — | Phase 7 | PostgreSQL 是全部业务事实唯一权威；Redis 仅保存可丢失易失状态 | `data_model.md` | Storage authority |
| D-230 | — | Phase 7 | Device/Guest/Join/Invite Resolution 虽短期仍入 PostgreSQL以支持事务与 HTTP 恢复 | `data_model.md` | Durable short-lived state |
| D-231 | — | Phase 7 | 数据库时间使用 timestamptz，API Repository 转换 Unix 毫秒 | `data_model.md` | Time type |
| D-232 | — | Phase 7 | 枚举使用 text/varchar + CHECK，不使用 PostgreSQL ENUM | `data_model.md` | Enum storage |
| D-233 | — | Phase 7 | JSONB 只保存版本化非授权快照，关键字段全部列化/规范化 | `data_model.md` | JSONB boundary |
| D-234 | — | Phase 7 | 使用领域终态+保留清理，不采用全局 soft-delete；不依赖 PostgreSQL RLS | `data_model.md` | Deletion/auth boundary |
| D-235 | — | Phase 7 | AEAD 密文分列保存版本/算法/AAD；高熵 Secret 使用带 Pepper 的 keyed digest | `data_model.md` | Cryptographic storage |
| D-236 | — | Phase 7 | username/email 使用独立占用注册表统一保证当前、历史和待验证值唯一 | `data_model.md` | Identity reservations |
| D-237 | — | Phase 7 | Token Family 与 token_generations 分表，ROTATED Refresh 保留用于重用检测 | `data_model.md` | Token rotation |
| D-238 | — | Phase 7 | Account 行锁+COUNT 保证 10 Family/5 Bound 上限，不维护计数器 | `data_model.md` | Account limits |
| D-239 | — | Phase 7 | Provider Login/Binding 共享 provider_principals，Subject AEAD+digest 跨用途唯一归属 | `data_model.md` | Provider identity |
| D-240 | — | Phase 7 | Provider Credential 版本化；旧版立即 RETIRED 并在 24 小时后清理 | `data_model.md` | Credential versions |
| D-241 | — | Phase 7 | 外键默认 RESTRICT；只有无独立审计价值的纯从属子表 CASCADE | `data_model.md` | FK policy |
| D-242 | — | Phase 7 | Friendship 使用单 Pair 行保存关系、双向 ban 与抑制；Request 作为独立历史行 | `data_model.md` | Friendship locking |
| D-243 | — | Phase 7 | Request Source 与 Relationship Source 分表，接受时事务内复制/转换 | `data_model.md` | Relationship provenance |
| D-244 | — | Phase 7 | External Friend Projection 使用随机 Binding-scoped ID、Subject AEAD+digest，不保存 matched Account | `data_model.md` | Provider projection |
| D-245 | — | Phase 7 | 无后台 Grant 的同步候选进入 PostgreSQL并在 15 分钟后清理 | `data_model.md` | Sync candidates |
| D-246 | — | Phase 7 | Provider 子任务结果和 Continuation 分表，禁止保存敏感匹配分类统计 | `data_model.md` | Sync task results |
| D-247 | — | Phase 6/7 修订 | Friend Sync mode=ALL 可选 Binding：有值 PROVIDER_FULL，无值 ALL_PROVIDERS；AUTO 仅内部 | `rest_api.md`, `data_model.md` | Sync API scope |
| D-248 | — | Phase 7 | Instance ONLINE 仅由 ACTIVE 生命周期与 LeaseStore 有效 current Lease 交集派生 | `data_model.md` | Instance presence |
| D-249 | — | Phase 7 | Instance 配置使用固定列；config_revision 与 acl_revision 独立 CAS | `data_model.md` | Instance storage |
| D-250 | — | Phase 7 | ACL 使用 Rule+Identity/Source/Subject 规范化子表，空子表表示任意 | `data_model.md` | ACL normalization |
| D-251 | — | Phase 7 | Proxy Grant/Instance 通过历史关联表绑定，以 ACTIVE partial unique 保证双侧一对一 | `data_model.md` | Proxy binding |
| D-252 | — | Phase 7 | Invite 使用事务计数+一 Request 一 Reservation，临时 EXHAUSTED 由容量动态派生 | `data_model.md` | Invite capacity |
| D-253 | — | Phase 7 | Join Identity Traits 与 Source Proof 分表保存服务端推导快照，审批仍重新推导 | `data_model.md` | Join evidence |
| D-254 | — | Phase 7 | Report 默认保留 180 天后按处置/Legal Hold 匿名化或清理，安全审计至少 30 天 | `data_model.md` | Report retention |
| D-255 | — | Phase 7 | 普通幂等记录在 PostgreSQL 与业务事务原子提交，非秘密结果保留 24 小时 | `data_model.md` | Idempotency storage |
| D-256 | — | Phase 7 | Token/Secret 成功结果使用独立 AEAD Replay 60 秒，过窗保留幂等 tombstone | `data_model.md` | Secret replay |
| D-257 | — | Phase 7 | Outbox Event/Delivery 分表，同 Event ID 多接收者复用 | `data_model.md` | Outbox fanout |
| D-258 | — | Phase 7 | Outbox 指数退避最多 24 小时，ABANDONED/PUBLISHED 后再保留 7 天 | `data_model.md` | Outbox retention |
| D-259 | — | Phase 7 | 安全审计 append-only 分权；高风险默认 180 天、普通业务 30 天 | `data_model.md` | Audit retention |
| D-260 | — | Phase 7 | 主库不做审计 Hash Chain；需要时导出独立 WORM/SIEM | `data_model.md` | Audit integrity |
| D-261 | — | Phase 7 | Redis 限流采用原子 GCRA/Token Bucket，Key 只含用途隔离身份 Digest | `data_model.md` | Rate store |
| D-262 | — | Phase 7 | Redis 故障按风险降级：Lease/Secret/Provider Flow fail-closed，低风险读继续 | `data_model.md` | Redis failure |
| D-263 | — | Phase 7 | 默认保留：DELETED Account 90 天、CLOSED Instance/终态 Friend/Grant/Invite 30 天、终态 Auth 24 小时 | `data_model.md` | Retention policy |
| D-264 | — | Phase 7 | 所有 FK/分页/TTL 查询建立用途索引；禁止把 now() 放入 partial index predicate | `data_model.md` | Index policy |
| D-265 | — | Phase 7 | 外键默认 RESTRICT，使用复合 FK 固化 Family/Owner/Pair/Provider/Invite 上下文一致性 | `data_model.md` | Referential integrity |
| D-266 | — | Phase 7 | 全局锁顺序从 Idempotency→Account→Family→Pair/Grant→Instance→Invite→Request/Reservation | `data_model.md` | Lock ordering |
| D-267 | — | Phase 7 收敛 | 终态资源解除可授权 Family FK并保留不可授权 Session ID Snapshot，支持 Token 历史按期清理 | `data_model.md` | Historical references |
| D-268 | — | Phase 7 收敛 | `/friends` 10 分钟服务端排序快照存 Redis AEAD，普通 Keyset Cursor 自包含 MAC/AEAD | `data_model.md` | Cursor storage |
| D-269 | — | Phase 7 收敛 | 邮件凭据与 email_delivery_jobs 同事务创建，Worker 不在业务事务内调用邮件 Provider | `data_model.md` | Email delivery |
| D-270 | — | Phase 7 收敛 | Provider Browser Completion 使用 Redis Claim Lease + PostgreSQL Idempotency fencing，避免消费后丢结果 | `data_model.md` | Browser flow recovery |
| D-271 | — | Phase 7 收敛 | 完成 12 类安全/并发场景与 11 组 REST API 到权威模型映射复核 | `data_model.md` | Data model review |
| D-272 | — | Phase 7 收尾 | Phase 7 完成：PostgreSQL/Redis 边界、42 组表/注册表、约束/索引/TTL/锁/清理与 API 映射冻结 | `data_model.md` | Phase completion |
| D-273 | — | Phase 8 | OpenAPI 使用 3.1.1 与 JSON Schema 2020-12 | `openapi.yaml` | Specification version |
| D-274 | — | Phase 8 | Server 固定相对 `/v2`，OpenAPI Paths 省略 `/v2` 前缀 | `openapi.yaml` | Server/path split |
| D-275 | — | Phase 8 | operationId 使用 lowerCamelCase；DTO closed object；nullable 使用 type union | `openapi.yaml` | Schema style |
| D-276 | — | Phase 8 | WebSocket 保留 GET/101，并用 x-websocket-events 描述通知边界 | `openapi.yaml` | WebSocket representation |
| D-277 | — | Phase 8 | RFC 9457 使用 Problem 基类和封闭 ProblemCode enum | `openapi.yaml` | Error schema |
| D-278 | — | Phase 8 | 89 个 Operation 均提供请求/响应示例，另补主要流程串联 | `openapi.yaml` | Examples policy |
| D-279 | — | Phase 8 | Auth/Binding Provider Flow 使用两个用途隔离的 __Secure Cookie 与最小 Path | `openapi.yaml` | Browser cookies |
| D-280 | — | Phase 8 | Provider Callback 显式声明 state/code/error/error_description 且 code/error 互斥 | `openapi.yaml` | Callback query |
| D-281 | — | Phase 8 | Auth/Account/Session/Device/Provider Login 的前 37 个 Operation 已与 REST 路径逐项覆盖 | `openapi.yaml` | OpenAPI batch 1 |
| D-282 | — | Phase 8 | Provider Binding/Friendship/Block/Friend Sync 已扩展至前 60 个 Operation；外部好友与同步统计只使用隐私安全 DTO | `openapi.yaml` | OpenAPI batch 2 |
| D-283 | — | Phase 8 | Provider Binding 的 effective_capabilities 统一使用 Registry Capability 名 `READ_FRIENDS`，不引入 `FRIEND_READ` 别名 | `rest_api.md`; `openapi.yaml` | Capability naming |
| D-284 | — | Phase 8 | Instance/ACL/Proxy/Invite/Guest/Join/Report 完成后，OpenAPI 已精确覆盖全部 89 个 REST Operation | `openapi.yaml` | OpenAPI batch 3 |
| D-285 | — | Phase 8 | NLI 实体 ID 保持 UUIDv4；CLIENT_CLAIMED Minecraft UUID 使用任意版本 canonical UUID；Guest 请求不接受服务端推导的 verification | `openapi.yaml` | Claimed profile schema |
| D-286 | — | Phase 8 | ACL 新 Rule 必须省略 rule_id，由服务端生成；显式 null 不作为创建语义 | `openapi.yaml` | ACL request schema |
| D-287 | — | Phase 8 | 每个 Operation 显式声明 security 与 x-principal；Provider LOGIN/REGISTER 和 LINK/REAUTH 使用 purpose-aware 安全扩展 | `openapi.yaml` | Security model |
| D-288 | — | Phase 8 | 全部 HTTP 响应统一 X-Request-ID；401 增加 WWW-Authenticate；429/503 保持 Retry-After | `openapi.yaml` | Common headers/errors |
| D-289 | — | Phase 8 | WebSocket 使用结构化 x-websocket-events/lease/close-codes，冻结无 ACK/重放/顺序及 HTTP 恢复语义 | `openapi.yaml`; `notifications.md` | WebSocket extensions |
| D-290 | — | Phase 8 | OpenAPI 顶层冻结 10 组跨 Operation 流程示例，覆盖账号、Device、Provider、好友、实例、Invite、Guest、Proxy 和 Report | `openapi.yaml` | Flow examples |
| D-291 | — | Phase 8 | Redocly 正式验证通过；仅保留 License 未冻结、303/101 非 2XX 和扩展引用未计数这 5 个预期 Warning | `openapi.yaml` | OpenAPI validation |
| D-292 | — | Phase 8 | Device Authorization approve/deny 显式绑定执行 lookup 的当前 Account，防止仅凭 Authorization UUID 跨账号操作 | `openapi.yaml`; `nli_account.md` | Device IDOR boundary |
| D-293 | — | Phase 8 | Friend Submission 隐私分支不声明 404；目标不存在、ban、抑制和隐藏容量统一 202 Receipt | `openapi.yaml`; `rest_api.md` | Enumeration resistance |
| D-294 | — | Phase 8 | InstanceRead/JoinRequestVisible 保持冻结 Wire DTO，以 closed oneOf 和字段存在规则选择视图，不新增 discriminator 字段 | `openapi.yaml` | Client union selection |
| D-295 | — | Phase 8 | Gate E 安全、隐私、客户端可实现性和机械校验通过；HTTP Contract 状态切换为 GATE_E_FROZEN | `openapi.yaml`; `_design_progress.md` | Gate E |
| D-296 | — | Phase 9 | 建立 `signaling.md` 入口；信令只服务仍授权的 ACCEPTED Join Request，独立于 Phase 5 通知 WS，且不得反向改变 Gate E 契约 | `signaling.md`; `outline.md` | Phase 9 boundary |
| D-297 | — | Phase 9 | 任一精确绑定参与方可用单一 create-or-attach 入口建立会话；角色由 Request 绑定推导，Offerer 固定为 REQUESTER | `signaling.md` | Entry / roles |
| D-298 | — | Phase 9 | 每个 Join Request 终身最多一个 Signaling Session；Acceptance Lease 不一次消费但不允许同 Request 重建，绝对期限不得延长 | `signaling.md` | Cardinality / lease |
| D-299 | — | Phase 9 | 创建、attach、握手和所有被接受的信令操作按当前 Session、双方在线、Source、关系/Proxy 与最新 ACL 原子重验；已消费 Invite 只验 Reservation 完整性，不因 EXHAUSTED 自撤销 | `signaling.md` | Authorization |
| D-300 | — | Phase 9 | PostgreSQL 权威保存最小会话生命周期；LeaseStore 只保存路由、参与方 Fencing、短租约和背压，易失存储不可使会话复活 | `signaling.md` | Storage authority |
| D-301 | — | Phase 9 | 每个会话角色最多一个 UUIDv4 connection_id；共享 LeaseStore CAS replace/renew/compare-delete，旧节点和旧连接不能推进会话，Store 故障 fail closed | `signaling.md` | Fencing / multi-node |
| D-302 | — | Phase 9 | 全局锁顺序在 Invite Reservation 后追加 Signaling Session 叶节点；PG 撤销同事务终止，ONLINE 丢失通过 use-time 复核和 Worker 收敛 | `signaling.md` | Concurrency / revoke |
| D-303 | — | Phase 9 | 创建强制公共 24 小时 Idempotency；第三方与错误绑定统一 404，授权丢失、状态冲突、期限过期和依赖不可用使用封闭且不泄漏分支的错误集合 | `signaling.md` | Retry / errors |
| D-304 | — | Phase 9 | Guest 只以 nli_guest 进入独立信令 WS，绝对过期截断会话/Fencing；Request、Session、Signaling ID 和 connection_id 均非 Bearer Credential | `signaling.md` | Guest / credential boundary |
| D-305 | — | Phase 9 | 聚焦复核后将 TARGET 锚定 nli_account/ACTIVE_BOUND owner family，登记独立信令 WS 为通知 WS 禁令的唯一例外，并补齐处理中幂等、锁序、DELETE 204、Fencing deadline 和 Event/Audit Registry 边界 | `signaling.md`; `common.md`; `outline.md` | Batch 1 review closure |
| D-306 | — | Phase 9 | 服务端只权威保存 WAITING_FOR_OFFER/WAITING_FOR_ANSWER/EXCHANGING_CANDIDATES 与 epoch；CONNECTED/ICE 成败只在客户端本地，不接受连接成功声明 | `signaling.md` | Protocol state |
| D-307 | — | Phase 9 | 协议 v1 使用按方向 closed union；客户端不提交 sender_role/时间/授权字段，服务端帧增加推导 Role、时间和 reply_to | `signaling.md` | Wire envelope |
| D-308 | — | Phase 9 | 初始 Offer 固定 epoch 1；Answer 后 REQUESTER 可显式 ICE Restart，最多 epoch 3；相同原始 SDP 由 Session-scoped keyed digest 幂等，不做文本规范化 | `signaling.md` | Offer / Answer / restart |
| D-309 | — | Phase 9 | Frame 64 KiB、SDP 48 KiB、Candidate 2048 B、每角色每 epoch 64 条、最多两个 Restart；Candidate 不解析地址归属 | `signaling.md` | Protocol hard limits |
| D-310 | — | Phase 9 | Result 只确认服务端提交，delivery_ack 只确认对端收到；消息允许重复乱序且无服务端持久 Replay，客户端按 message_id/epoch 缓冲、ACK 和保留本地 Payload 重发 | `signaling.md` | Retry / recovery |
| D-311 | — | Phase 9 | ice_end 按角色/epoch 记录且不代表已连接；Signaling Session 终态保留 24 小时，敏感易失 Payload 立即删除 | `signaling.md` | ICE completion / retention |
| D-312 | — | Phase 9 | 用户选择独立 Relay 授权：60 秒内建立 Allocation，窗口外仅维护既有 Allocation；需要可信 TURN 授权/管理扩展，长期时间戳 Credential 不能独自保证此边界 | `signaling.md` | Relay authorization direction |
| D-313 | — | Phase 9 | 独立 Grant 最长 1 小时；首次激活限 60 秒内，维护使用最长 15 秒运行许可、5 秒续期、2 秒时钟误差提前截止；自然 Guest/Signaling 到期与显式撤销分离 | `signaling.md` | Relay lifecycle |
| D-314 | — | Phase 9 | PG Grant/Slot 与配额权威，Prepare/Activate 去重、node/boot/fence、permit nonce/sequence/revision、Peer endpoint Pin 和预扣 byte-credit 防重放/越权；Orphan Reaper 安全回收 | `signaling.md` | Relay concurrency |
| D-315 | — | Phase 9 | 每 Grant 6 同时/24 累计 ACTIVE Allocation、2 GiB 总额；并行/短交错 UDP/TCP/TLS 收集；批次 3 复核问题修正后设计冻结，真实 Adapter 验收仍为生产开关门槛 | `signaling.md` | Relay review / deployment |
| D-316 | — | Phase 9 | Signaling Route 使用含双方 Slot 和 route_revision 的单一原子 Document；5 秒 Ping/20 秒 Lease，check-both 与 replace 共用序列化点，投递许可最长 250ms且写 Socket 前再验当前 fence | `signaling.md` | Multi-node routing |
| D-317 | — | Phase 9 | Candidate 使用最小 PG Receipt：发送 Role/epoch/message 唯一、HMAC-SHA-256 digest，不保存地址；Receipt 判定先于 ice_end，新消息才消耗每 epoch 64 条额度 | `signaling.md` | Candidate retry |
| D-318 | — | Phase 9 | 信令限制为 64KiB Frame、128帧/512KiB Socket 队列、20 data frame/s、独立 control 预留与 4401–4407 关闭码；批次 4A 评审修正后设计冻结 | `signaling.md` | Backpressure / close |
| D-319 | — | Phase 9 | 基础模式仅承诺 `TRANSPORT_ENCRYPTED` 且信任 NLI 信令；不声称抵抗恶意信令 MITM。未来 `PEER_VERIFIED_E2E` 需独立协议并禁止静默降级 | `signaling.md` | Anonymous / E2E boundary |
| D-320 | — | Phase 9 | SDP/ICE/地址/Secret 禁止进入普通日志、持久队列和 Dump；break-glass 限 30 分钟且窗口结束后 24 小时删除；固定 Audit Schema 保留 30 天 | `signaling.md` | Privacy / audit |
| D-321 | — | Phase 9 | 撤销和安全关闭不能被 Audit 故障阻塞；Session/Grant 创建与签发需先成功审计，聚合补写限 60 秒；TURN egress 使用规范化地址 denylist 与固定 Peer Pin | `signaling.md` | Audit failure / SSRF |
| D-322 | — | Phase 9 | 客户端以 HTTP GET + 最新 WS generation + 本地 WebRTC 三层状态恢复；原 Payload 保留到 epoch替换/终态/截止，ACK不表示应用，ACK超时先 GET且不抢占健康自身连接 | `signaling.md` | Client recovery |
| D-323 | — | Phase 9 | v2 信令只支持可设置 Authorization Upgrade Header和标准 Ping的原生/Mod WS 栈；浏览器 JS 不受支持，禁止 Query/Subprotocol/首帧 Secret旁路；#106 经修正后冻结 | `signaling.md` | Client transport / review |
| D-324 | — | Phase 9 | Gate F 在 Gate E 77 Paths/89 Operations 上只追加4 Paths/5 Operations，总计81/94；REST、数据模型和OpenAPI同步冻结，既有89项语义不变 | `rest_api.md`; `data_model.md`; `openapi.yaml` | Gate F surface |
| D-325 | — | Phase 9 | OpenAPI 2.1.0以JSON Schema 2020-12约束Signaling/ICE closed DTO和WS类型—Payload分支；ProblemCode增加Signaling/TURN封闭错误 | `openapi.yaml` | Machine contract |
| D-326 | — | Phase 9 | Gate F机械验收：YAML/全部本地引用/81 Paths/94唯一lowerCamel operationId/新示例/ICE与Envelope正反例通过；Redocly有效且仅8个已解释Warning | `openapi.yaml` | Gate F validation |
| D-327 | — | Phase 9 | Phase 9 与全套详细设计在 Gate F 后暂停；恢复时按 Migration/清理 → Rust REST/WS → LeaseStore/EventBus → 默认关闭的 TURN Adapter → 客户端及真实网络验收推进，契约变更须显式重开 Gate | `_design_progress.md`; `outline.md` | Implementation entry / pause |
| D-328 | — | Gate F final review | 不改变81/94表面的前提下收窄5个Signaling Operation错误状态；明确不可关联坏帧直接关闭、TURN同five-tuple单live Slot、Quota Bucket模式/window/GC及Permit→Slot→Grant→Receipt→Session清理顺序 | `signaling.md`; `rest_api.md`; `data_model.md`; `openapi.yaml` | Final consistency hardening |
| D-329 | — | Implementation preparation | Gate F契约以commit `3b4f329`和annotated tag `v2-design-gate-f`独立冻结；提交只含12个`doc/v2`设计文件，既有Cargo 0.2.0工作区修改未混入 | Git history | Baseline provenance |
| D-330 | — | Implementation preparation | v2使用全新独立PostgreSQL Database与`migrations/v2` journal，不迁移v1数据；按CI/平台→30 Identity→30 Provider/Friend→29 Runtime/Join→Gate E→5 Signaling→隐藏Relay→客户端验收→切流/删除v1推进 | `implementation_plan.md` | Implementation sequence |
| D-331 | — | Implementation preparation | v2绿地部署到新服务器且不继承任何v1业务数据；新旧环境不共享DB/Redis/Secret/备份，旧服切流时只读且不作v2故障转移；API/PG/易失Redis/TURN按角色和网络压力隔离 | `implementation_plan.md` | Infrastructure / cutover |
| D-332 | — | Implementation preparation | v2应用版本固定为`0.2.0`；初始部署目标固定为`hangzhou-traffic`单机原生systemd，WSL锁版本构建后手工校验上传，不使用容器；为节省资源API与有界Worker同进程、v2使用一个整实例无持久Redis，当前单机生产Relay固定关闭 | `implementation_plan.md`; `Cargo.toml`; `Cargo.lock` | Version / deployment |
| D-333 | — | Implementation preparation review | 手工发布在每次Migration前创建异机恢复点，contract Migration必须先真实恢复；v2无本地业务状态/磁盘spool，systemd使用依赖排序与硬化；每日/每周异机备份、24h RPO/4h RTO、原子软链接、锁定Redocly、Release Manifest和临时双机验收写入门禁 | `implementation_plan.md` | Deployment hardening |


## 进度记录

按时间倒序追加，每条只记录本次推进结果和下一步。

| 日期 | 阶段 | 完成内容 | 遗留问题 | 下一步 |
| --- | --- | --- | --- | --- |
| — | 实现准备复核 | 独立Reviewer确认81/94、PG权威、易失Redis、Relay关闭和分支策略无冲突；修正Migration前恢复点、systemd本地写/依赖、v1同机最坏假设、稳态异机备份、软链接原子性、Redocly锁定、WSL/CRLF、Release Manifest、单机SPOF和临时双机验收 | `hangzhou-traffic`实机架构/systemd/PostgreSQL unit名称仍须Phase 0探测；当前单机不满足生产Relay Gate | 单独提交0.2.0与计划，推送基线，建立Phase 0分支 |
| — | 实现准备 | 用户确认v2即`0.2.0`，初始部署使用`hangzhou-traffic`；计划改为WSL原生依赖Gate/锁版本Release构建/SHA-256手工上传/原子软链接/systemd最小权限服务，全程不使用容器；单机资源约束下API与Worker同进程、v2 Redis整实例无持久化、Relay固定关闭 | 尚需实机确认架构、发行版/glibc、systemd版本和容量；这些列为Phase 0部署预检 | 完成独立复核后，单独提交版本与计划更新，再建立Phase 0分支 |
| — | 实现准备 | 根据用户确认补充新服务器绿地部署：v1数据完全不继承，新旧DB/Redis/Secret/备份隔离，旧服只读下线，v2首笔生产写入后只允许v2回退/forward-fix；新增角色拓扑、网络容量、混合负载和TURN故障域门槛 | 新服务器供应商、规格和客户端迁移公告尚待实施阶段确定 | 实现仍未开始；Phase 0先建立容量/契约验收骨架 |
| — | 实现准备 | 创建仅含12个v2设计文件的基线提交`3b4f329`和tag `v2-design-gate-f`；完成`implementation_plan.md`，吸收Reviewer对migration journal、Outbox/Audit故障方向、Route线性化、Relay清理、客户端验收和切流策略的全部阻塞修正 | 当前Cargo 0.2.0修改仍未提交且不属于设计基线；客户端仓库、自定义TURN Adapter和真实基础设施仍是后续实现输入 | 保持IMPLEMENTATION_PLANNED_NOT_STARTED；待明确授权后只执行Phase 0–1 |
| — | Gate F 最终审阅 | 三路只读审阅因上下文上限未形成正式输出；主审从持久轨迹提取并逐项核实，修正Quota窗口歧义、five-tuple双Slot、Relay清理/FK顺序、无message_id错误关联及OpenAPI额外403/413/422；YAML/1422本地引用/81 Paths/94 Operations/状态码集合/AJV正反例/Markdown链接/diff检查通过，Redocly有效且仍为8个已解释Warning | 独立Reviewer运行稳定性不足；真实TURN/WebRTC与数据库约束仍属于实现验收，不在本轮执行 | 最终审阅无设计 blocker；保持IMPLEMENTATION_PAUSED |
| — | Phase 9 / Gate F | #108完成：总体状态、D-324–D-327、正式基线、Phase 9完成标准和后续实现入口已同步；全套详细设计与Gate F冻结 | Redocly保留8个预期Warning；真实TURN/WebRTC、多节点、撤销、配额、崩溃和隐私验证属于实现Gate | 按用户要求暂停；仅在明确授权后建立新的实现计划 |
| — | Phase 9 | #106 Reviewer 恢复后发现 ACK reply_to 无合法位置及 Restart epoch 通则冲突；修正后 Oracle 条件批准，再补 4405 退避、8 帧在途定义、ACK 超时先 GET、TURN Refresh(0)；12 步算法/错误表/28 场景冻结 | 真实客户端/WebRTC/TURN 故障测试仍属实现 Gate；#107 未开始 | 同步 REST/Data/OpenAPI 并执行 Gate F |
| — | Phase 9 | #106 草案补齐 12 步单一客户端恢复算法、11 类错误决策与 24 个跨 HTTP/WS/ICE/TURN/隐私端到端场景；自检修正 Delivery ACK 与 RELAY opaque SDP 边界 | 客户端和协议一致性双评审运行中，尚未冻结 | 收取评审，修正后进入 #107 |
| — | Phase 9 | #105 独立安全复核无 blocker；修正撤销不受 Audit 故障阻塞、break-glass 权限/事件/删除锚点、明确 TURN denylist、60 秒 Audit 补写和 Digest Key 轮换；#105 设计冻结 | 运行时 Redaction/egress/部署检查未执行；未来 PEER_VERIFIED_E2E 未纳入本版 | 开始 #106 场景与客户端评审 |
| — | Phase 9 | #104 复核完成并修正 ice_end 后 Receipt-first 重投、原子双角色 Route check、写 Socket 前 fence、依赖故障续租、20 秒 Lease、Receipt sender/HMAC、Close 优先和 oldest-first；批次 4A 冻结 | #105 隐私/E2E 草案未评审，#106 客户端走查未开始 | 开始 #105，Phase 9 结束后暂停 |
| — | Phase 9 | #104 采用截止即清理的最小 Candidate Receipt，解决同 ID 重试重复计数及 ice_end 后重发；明确跨存储投递排序和最长 250ms 已授权尾部、缓存/队列独立额度；#105 增加基础 E2E 信任边界与日志/备份检查草案 | #104 聚焦复核运行；#105 尚待评审；Phase 9 未结束，不进入实现 | 完成当前规划后按用户要求暂停 |
| — | Phase 9 | 独立 Relay 评审返回，修正 PENDING Allocate 去重及成功响应时机、孤儿 Reaper、24 次累计额度、并行传输、时钟/续期/Secret Replay/TLS/byte-credit 说明；23 个场景，#103 设计冻结 | 尚无真实 TURN Adapter 测试，不宣称 RFC/客户端兼容已验证；#104 为草案 | 完成 #104 多节点信令/恢复/背压复核 |
| — | Phase 9 | #103 独立复核运行期间推进 #104 草案：5 秒 Ping/15 秒租约、RPC 投递授权边界、重连/ACK 保留、队列与速率候选值、独立 4401–4407 关闭码和清理要求 | 均为 REVIEW_PENDING；Candidate 去重丢失后的重试可能耗尽硬额度，需 Gate F 前确认恢复语义；Relay 独立复核仍未返回 | 修正 #103，继续复核 #104，不提前冻结 |
| — | Phase 9 | Relay 本地复核补齐许可 nonce/revision/sequence、延迟响应绝对截止、额度幂等、Peer IP/endpoint 有界 Pin、鉴权删除、独立撤销重验和 Guest 清理边界；累计 20 个场景，结构/diff 检查通过 | 原独立评审空输出，已恢复同一会话并补充修正上下文；未收到可验收结论 | 保持 #103 待复核，不宣称冻结或真实兼容性通过 |
| — | Phase 9 | 按 D-312 重写独立 Relay 授权详细稿：窗口内 Prepare/Activate、窗口外仅维护、PG Grant/Slot/额度、node/boot/fence、15 秒运行许可、Peer Pin、撤销矩阵、多 Allocation 和 17 个场景；移除普通时间戳授权草案 | REVIEW_PENDING；1 小时/15 秒等参数需聚焦复核，真实 TURN Adapter 验收未运行 | 收取独立复核，修正后再完成 #103 |
| — | Phase 9 | 冻结批次 2：三阶段 PG 状态机、epoch/显式 Restart、方向化 v1 Envelope、Result/ACK/Error、keyed digest 幂等、Trickle ICE、硬上限、乱序重发、终态读取与 12 个场景 | TURN、精确速率/队列/关闭码及多节点投递仍待后续批次；两个初始 Reviewer 因验收/上下文问题失败但 Oracle 完整产物已人工复核吸收 | 开始批次 3 |
| — | Phase 9 | 批次 1 聚焦复核完成并修正 3 个 blocker、4 个 warning：有效 Audience/Target Family、独立 WS 唯一例外、处理中幂等、通知 Registry、锁序、DELETE 204、绝对 Fencing deadline；机械检查通过 | Offer/Answer/ICE 协议 Phase、消息幂等、Fencing 精确 TTL/关闭码尚待后续批次 | 开始批次 2 |
| — | Phase 9 | 冻结批次 1：四个独立入口、双方角色、单 Request 单会话、Lease 非一次消费、原子授权重验、锁顺序、共享 Fencing、Guest、幂等/错误与 12 个并发故障场景 | 聚焦复核发现 Audience/Target Family、WS 例外和处理中幂等等可修正项 | 执行聚焦复核修正 |
| — | Phase 8 | 开始将冻结的 89 个 REST Path/Method、DTO、Principal、错误、幂等和分页机械化为 OpenAPI | OpenAPI 表达约定待确认 | 建立 `openapi.yaml` 组件骨架 |
| — | Phase 9 | 创建 `signaling.md` Phase 9 骨架，固化 Gate E 输入边界、范围/非目标、权威/易失方向、状态机与协议待决项、安全/客户端清单和四个设计批次；同步 `outline.md` | 尚未冻结具体信令传输、状态机或 TURN 策略 | 开始 Phase 9 批次 1 |
| — | Phase 8 | Gate E 聚焦复核无确认阻塞；修正 Device Authorization Owner 边界、Friend Submission 404 枚举、可选 reject body、Provider completion 401/403、Problem detail 与 union 选择说明；Redocly 和 89 Operation 全量机械审计复验通过 | 三个初始 Reviewer 均上下文超限，Security/Client/Contract 以禁止继续读文件的恢复轮次产出；Contract CLEAN，Client 无硬阻塞 | Gate E 通过，执行 #99 并进入 Phase 9 |
| — | Phase 8 | 增加 10 组主要流程串联示例；修正 Device Code 示例长度和 Friend Sync discriminator；Redocly 验证有效，89 Operation/181 本地引用/端点覆盖/请求响应示例/Flow operationId 全量检查通过 | 5 个预期 lint Warning 已记录；安全与客户端复核尚待 #98 | 开始批次 #98 |
| — | Phase 8 | 统一全部 89 Operation 的 Security/x-principal、purpose-aware Provider Auth、RFC 9457/401/429/503、36 个强制幂等端点、7 个分页端点、全响应 X-Request-ID 与结构化 WebSocket 扩展；全量机械审计通过 | 正式 OpenAPI Validator 与主要流程串联示例留待 #97 | 开始批次 #97 |
| — | Phase 8 | 完成 Instance/ACL/Proxy/Invite/Guest/Join/Report 共 29 个 Operation 与严格 DTO；累计 89/89；修正 Accept 路径、Guest 推导字段、canonical MC UUID、ACL 新 Rule ID 和天然幂等 DELETE；YAML、引用、operationId、示例 Schema 与 REST 精确覆盖检查通过 | Writer 多次在验收报告阶段失败，父 Agent 已复核并修正产物 | 开始批次 #96 |
| — | Phase 8 | 完成 Provider Registry/Binding、好友聚合/申请/Block、Friend Sync 共 23 个 Operation 与严格 DTO；累计 60/89，路径、内部引用、operationId、示例与 REST 精确覆盖检查通过 | 两个独立 Reviewer 均因上下文超限未产出；Gate E 前需重新执行聚焦复核 | 开始批次 #95 |
| — | Phase 8 | 完成 Auth/Device/Provider Login/Account/Session 共 37 个 Operation 与严格 DTO；Provider Callback/Cookie、RFC 8628、Recent Auth、Token Pair 和全部示例已表达；路径/$ref/operationId 校验通过 | Provider Binding/Friendship 待机械化 | 开始批次 #94 |
| — | Phase 8 | 已机械化注册/验证/登录/Refresh/密码恢复/删除恢复与 RFC Device 共 15 个 Operation；每项含请求/响应示例，YAML/$ref/operationId 检查通过 | Provider Login、Account、Session 尚待完成 | 继续批次 #93 |
| — | Phase 8 | 建立 OpenAPI 3.1.1 骨架、相对 /v2 Server、Tag/Security/Parameter/Header、RFC 9457/OAuth Device Error 和公共 Response Components；冻结 Schema/示例/Cookie 约定 | 89 个 Paths 与领域 DTO 待机械化 | 开始 Auth/Account/Device/Provider Login 批次 |
| — | Phase 7 | 完成最终结构检查：表/注册表、Fence、决策编号、REST 89 端点映射、事件/字段命名与 diff check 通过；状态切换 READY_FOR_PHASE_8 | 独立 Reviewer 因上下文超限未产出，已完成本地结构复核 | Phase 7 完成，进入 Phase 8 OpenAPI |
| — | Phase 7 | 完成 REST 模型映射、服务端推导字段、注册/Refresh/Provider/Friend/Instance/ACL/Invite/Guest/Join/删除/Outbox 并发与隐私复核；补齐 Cursor、Email Job 与 Browser Completion 恢复 | 独立 Reviewer 因上下文超限未产出，已完成本地结构复核 | 执行最终检查并进入 Phase 8 |
| — | Phase 7 | 冻结复合 FK/唯一约束、查询与 TTL 索引、全领域保留矩阵、清理编排、READ COMMITTED 行锁顺序和 Migration 策略 | 场景/API 映射复核待完成 | 进行 Phase 7 最终复核 |
| — | Phase 7 | 完成 Idempotency/Secret Replay、Outbox Event/Delivery、安全审计与 Redis Lease/EventBus/GCRA/Provider Flow/Lock/Cache 模型 | 全局约束、清理和竞态复核待完成 | 开始 Phase 7 收敛 |
| — | Phase 7 | 完成 Instance 固定配置、Lease 派生在线、规范化 ACL、Proxy Binding、Invite/Resolution、Guest、Join/Traits/Proof/Reservation 与 Report 表 | Audit/Outbox/Idempotency/Redis 待设计 | 开始批次 4 |
| — | Phase 7 | 完成 Friendship Pair/Request/双向 ban、Request/Relationship Source、Provider Projection、Task/Provider Result/Candidate/Continuation/Schedule 表 | Instance/Join 系列待设计 | 开始批次 3 |
| — | Phase 7 | 完成 Account/Usernames/Emails/Password、Token Family/Generation、Device、Email Credential、Provider Principal/Login/Binding/Credential 表与约束索引 | Friendship/Projection/Task 待设计 | 开始批次 2 |
| — | Phase 7 | 冻结 PostgreSQL 单一业务权威、Redis 易失边界、UUID/timestamptz/text+CHECK、JSONB 限制、领域终态清理、应用鉴权和版本化 AEAD/keyed digest | 领域表待设计 | 开始 Account/Auth/Provider 批次 |
| — | Phase 7 | 开始从领域与 REST 契约推导 PostgreSQL 持久模型、Redis 易失模型、约束、索引、TTL 和事务边界 | 权威存储分界待确认 | 建立 `data_model.md` 并完成存储边界批次 |
| — | Phase 6 | 完成注册/Refresh/Provider/Device/Friend/Instance/ACL/Invite/Guest/Join/Report 安全并发场景和十条客户端流程复核；Path 无重复、事件映射完整 | 无 | Phase 6 完成，进入 Phase 7 数据模型 |
| — | Phase 6 | 完成跨端点收敛：Principal/服务端推导字段矩阵、状态码、Secret 幂等、Revision/锁、Body/分页/限流和通知映射；补充 Provider Login Identity 管理 | 场景与客户端实现复核待完成 | 开始 Phase 6 收尾 |
| — | Phase 6 | 完成批次 4：Guest Token/60 秒重放、服务端 Join Source、统一 Request DTO、Source/Target 查询、无独立 Lease 检查和 Request-bound Report Receipt | 跨端点矩阵与通知映射待统一 | 开始 REST 契约横向收敛 |
| — | Phase 6 | 完成批次 3：Instance 创建/Owner 读取、配置与 ACL Revision、WS Path、Proxy Secret 生命周期、Invite 管理和 Session-bound Resolution | Guest/Join/Lease/Report API 待确认 | 开始批次 4 |
| — | Phase 6 | 完成批次 2：Registry/Binding DTO、浏览器授权完成、用途 Revision、好友 Submission Receipt、聚合列表、Request 命令、Block 与 ONE/ALL 同步 Task | Instance/ACL/Proxy/Invite API 待确认 | 开始批次 3 |
| — | Phase 6 | 完成批次 1：Auth 动作 Path、显式登录、Token DTO/60 秒重放、RFC Device、Provider 浏览器完成、5 分钟 reauth、Session 撤销与 Account 精确定位 | Provider/Friendship API 待确认 | 开始批次 2 |
| — | Phase 6 | 开始冻结 HTTP Path、Principal、DTO、状态码、幂等/并发、限流和通知映射 | 各领域端点待确认 | 建立 `rest_api.md` 并完成认证/账号批次 |
| — | Phase 5 | 完成安全与故障场景走查，补充 WSS/Origin/握手限流、Pong 非续租证明、跨存储补偿协调和 ready 首帧规则；独立 reviewer 未返回输出，已完成本地一致性复核 | 无 | Phase 5 完成，进入 Phase 6 REST API 契约 |
| — | Phase 5 | 完成批次 4：无 Event Replay、连接后全量 HTTP 恢复、恢复矩阵、Outbox 路由语义、LeaseStore/EventBus、多节点 fail-closed 和 1012 重启 | 安全场景走查待完成 | 开始 Phase 5 收尾 |
| — | Phase 5 | 完成批次 3：1 KiB 提示 Envelope、Outbox UUID、best-effort 无 ACK、无顺序保证、Scope 恢复、全在线实例 Fanout、4003 背压和初始事件表 | HTTP 恢复与多节点待确认 | 开始批次 4 |
| — | Phase 5 | 完成批次 2：握手即 ONLINE、标准 Ping/Pong、90 秒严格租约、10 秒写合并、关闭立即 OFFLINE、compare-delete 过期和 10 分钟自动关闭 | Event Envelope 与恢复待确认 | 开始批次 3 |
| — | Phase 5 | 完成批次 1：Instance-only Principal、Authorization Header、连接跨 Token 到期、UUIDv4 Session Fencing、4001 替换和 1/5 连接限制 | Ping/Pong、事件和恢复待确认 | 开始批次 2 |
| — | Phase 5 | 开始 WebSocket 鉴权、实例保活、通知 Envelope、HTTP 恢复和多节点路由细化 | 连接范围与事件语义待确认 | 建立 `notifications.md` 并完成批次 1 |
| — | Phase 4 修订 | 完成有序 ACL、空 Matcher wildcard、NLI/Direct/Proxy/Anonymous Traits、Friend/Invite Sources、弱 MC Matcher、Profile 纯展示及举报边界；Gate D 重新通过 | 无 | 进入 Phase 5 |
| — | Phase 4 修订 | 根据反馈重新打开 Gate D：MC Profile 仅展示，访问控制改为 Matcher+Action ACL | ACL 求值、身份/来源枚举和 MC 名称 allow 风险待确认 | 修订 ACL 与 Profile 边界 |
| — | Phase 4 | 完成安全与并发场景、权限矩阵和通知丢失恢复走查；Gate D 通过 | 无 | 进入 Phase 5 WebSocket 通知与保活设计 |
| — | Phase 4 | 完成批次 4：四类 Join 路径、Profile 一致性、60 秒 Session-bound Lease、3/100 pending、失效取消、原子 auto-accept 和结果保留 | 安全场景与 Gate D 待完成 | 开始 Gate D 走查 |
| — | Phase 4 | 完成批次 3：Invite 单次显示、1–100 次预留消费、3 个/24h/7d、Guest 5 分钟单流程、最小公开资料和临时匿名控制 | Join Request 待确认 | 开始批次 4 |
| — | Phase 4 | 完成批次 2：单实例复用 Grant、单代理目标、更新时绑定、代理 FRIEND 路径、好友解除终止、Secret 轮换和可选无到期 | 邀请码、Guest 与 Join Request 待确认 | 开始批次 3 |
| — | Phase 4 | 完成批次 1：生命周期/租约分离、10 分钟重连、互斥身份类别、hidden 邀请寻址、字段限制、无 MC 头像和同 Family 管理 | 代理授权、邀请码、Guest 与 Join Request 待确认 | 开始批次 2 |
| — | Phase 4 | 开始 Game Instance、代理发布、邀请码、Guest 和 Join Request 细化 | 实例生命周期与加入授权待确认 | 建立 `game_instance.md` 并完成批次 1 |
| — | Phase 3 | 完成安全与并发场景走查，补充 24 小时拒绝抑制和分离 pending 上限；Gate C 通过 | 无 | 进入 Phase 4 Game Instance 设计 |
| — | Phase 3 | 完成批次 4：来源约束合并、无上游 GET、200 部分降级、快照 Cursor、自有 ban 列表、来源历史和代理 Owner 标记 | 安全场景与 Gate C 待完成 | 开始 Gate C 走查 |
| — | Phase 3 | 完成批次 3：统一异步同步任务、活动 Scope 合并、6 小时自动同步、关系感知自动接受、500 条 Continuation 和隐私统计 | 聚合读模型待确认 | 开始批次 4 |
| — | Phase 3 | 完成批次 2：Binding 范围投影、加密 Subject/HMAC 索引、15 分钟新鲜度、opaque ID、同域解析和 pending 隐藏 | 同步任务和聚合读模型待确认 | 开始批次 3 |
| — | Phase 3 | 完成批次 1：规范化 Pair、30 天 pending、反向申请接受、有向 ban、显式恢复、删除宽限保留及并发事务 | Provider 投影、同步和聚合待确认 | 开始批次 2 |
| — | Phase 3 | 开始好友关系、Provider 投影、同步任务与聚合读模型细化 | 好友状态机、同步和隐私边界待确认 | 建立 `friendship.md` 并完成批次 1 |
| — | Phase 2 | 完成批次 4：Provider 降级/永久停用、Mock Provider、刷新竞争、Subject mismatch、解绑离线和故障场景走查；Gate B 通过 | 无 | 进入 Phase 3 好友设计 |
| — | Phase 2 | 完成批次 3：Registry/Binding 状态、错误分类、安全重试、分操作熔断、同 Subject 重验证、显式换绑和回调防护 | Mock 与故障场景待完成 | 开始批次 4 |
| — | Phase 2 | 完成批次 2：ProviderService 边界、版本化 AEAD、Refresh 持久/Access 短缓存、单飞刷新、交互式降级和本地优先撤销 | Binding 错误与重验证待确认 | 开始批次 3 |
| — | Phase 2 | 完成批次 1：管理员 Registry、slug ID、Capability 交集、强类型编译期 Adapter、单 Provider 单 Binding、登录身份分离 | Credential Manager、错误与重验证待确认 | 开始批次 2 |
| — | Phase 2 | 开始 Provider 抽象与凭据管理细化 | Provider 注册、能力、Adapter 和 Binding 边界待确认 | 建立 `provider.md` 并完成批次 1 |
| — | Phase 1 | 完成批次 4：一次性邮件凭据、密码/邮箱变更、删除恢复、人工 DISABLED 恢复、限流与安全场景走查；Gate A 通过 | 无 | 进入 Phase 2 Provider 设计 |
| — | Phase 1 | 完成批次 3：RFC 8628 Device Flow、8 位 User Code、显式批准、消费时创建 Session、60 秒结果重放 | 邮箱验证、密码恢复和账号生命周期细节待确认 | 开始批次 4 |
| — | Phase 1 | 完成批次 2：不透明 Token、15m/30d/90d、10/5 会话限制、实例可解绑复用、60 秒 Refresh 重放和会话管理 | Device Code 和恢复细节待确认 | 开始批次 3 |
| — | Phase 1 | 完成批次 1：username/display_name、必需验证邮箱、PENDING_EMAIL、Argon2id、状态机和 30 天删除恢复 | Session、Token、Device Code 和恢复细节待确认 | 开始批次 2 |
| — | Phase 1 | 开始 NLI Account 与认证细化，准备账号、认证器、会话与恢复决策 | 账号公开定位、注册激活和密码策略待确认 | 建立 `nli_account.md` 并完成批次 1 |
| — | Phase 0 修订 | 收敛 Token 模型：NLI Account/Instance 共用 nli_account Audience；Guest 使用 nli_guest；保留未来 nli_admin；移除普通 API Scope | Guest 具体生命周期留待 Game Instance 子设计 | 进入 Phase 1 时冻结 Token Claims 与 Session 状态 |
| — | Phase 0 | 完成批次 4：Retry-After、最小化日志、独立安全审计、字段错误数组、英文错误文本和稳定 Problem URN；完成三个公共场景走查 | 无 | Phase 0 完成，进入 Phase 1 |
| — | Phase 0 | 完成批次 3：选择性强制 Idempotency-Key、Refresh 短期重放、Cursor 20/100、弱一致 Keyset、原子 409、201/Location 与幂等 DELETE | 限流、日志和审计待确认 | 开始批次 4 |
| — | Phase 0 | 完成批次 2：Bearer + Scope、严格请求字段、POST 命令/PUT 替换、服务端 Request ID、400/422/409 错误映射 | 幂等、分页、限流和日志待确认 | 开始批次 3 |
| — | Phase 0 | 完成批次 1：UUIDv4、snake_case、Unix 毫秒、/v2、Problem Details、直接资源响应；创建 `common.md` 骨架 | 鉴权、HTTP 行为、幂等、分页、限流和日志待确认 | 开始批次 2 |
| — | Phase 0 | 开始公共约定规划，准备基础格式与 API 行为决策 | 等待分批确认公共约定 | 建立并细化 `common.md` |
| — | Planning | 创建详细设计路线图和监督标记 | 无 | 创建 `common.md` |

## 下一步

```text
Phase 8 / openapi.yaml
```

Phase 7 已完成。下一步把 `rest_api.md` 的 89 个 Path/Method、Principal、DTO、RFC 9457 Problem、幂等 Header、分页和状态码机械化为 OpenAPI；OpenAPI 不得重新解释领域或暴露 `data_model.md` 内部字段。
