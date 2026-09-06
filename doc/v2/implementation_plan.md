# NetherLink v2 实现计划

> 状态：`IMPLEMENTATION_PLAN_FROZEN / IMPLEMENTATION_NOT_STARTED`
>
> 设计基线：Git commit `3b4f329`，本地 annotated tag `v2-design-gate-f`
>
> 契约基线：OpenAPI 3.1.1，Gate E `77 Paths / 89 Operations`，Gate F 总计 `81 Paths / 94 Operations`
>
> 应用版本：`0.2.0`
>
> 初始生产目标：`hangzhou-traffic`，WSL 本地构建、手动上传、原生 systemd 运行，不使用容器
>
> 本文不是新 API 契约。若实现需要改变 `3b4f329:doc/v2/` 的冻结语义，必须显式重开对应 Gate，不能通过普通实现 PR 静默修改。

## 1. 目标

- 完全舍弃 v1 业务实现，以冻结的 `doc/v2/` 为唯一产品和协议基线；
- 建立 PostgreSQL 业务权威、独立易失 LeaseStore/EventBus、封闭 HTTP/WS 契约和可恢复 Worker；
- 分阶段实现 Gate E 的 89 个 Operation 与 Gate F 的 5 个 Operation；
- 在真实多节点、WebRTC/TURN、撤销、配额、崩溃和隐私验收完成前保持生产 Relay 关闭；
- 每个阶段都有可自动执行的验收、独立开关、提交边界和失败恢复策略。

## 2. 非目标

- 不迁移 v1 Profile、Presence、好友、实例、信令或 TURN 业务数据；
- 不兼容 v1 API、Redis Key、数据库表语义或客户端协议；
- 不把 MC Profile 升级为 NLI/Minecraft 身份证明；
- 不在服务端持久保存 SDP、ICE Candidate、网络地址、TURN Password 或 Peer Pin 原值；
- 不用普通 coturn shared-secret 配置替代冻结的 Relay Authorizer；
- 不在本阶段升级 `hangzhou-traffic` 硬件，也不引入Kubernetes、Docker或其他容器运行时；未来迁移到新服务器时沿用v2备份恢复和绿地部署流程。

## 3. 权威输入与适用范围

- `doc/v2/openapi.yaml`：公共 HTTP/WS Upgrade 表面和 DTO；
- `doc/v2/common.md`：公共格式、安全和错误约定；
- 各领域文档：账号、Provider、好友、实例、通知和信令领域语义；
- `rest_api.md`：端点授权、状态转换、幂等和错误语义；
- `data_model.md`：持久化、事务、锁序、保留和清理；
- 本实现计划：实现顺序和交付方式，不覆盖上述契约。

这些文档应已一致；若实现中发现交叉文档矛盾，必须停止该切片并重开设计Gate，不能自行按“优先级”选择其中一份。现有 `src/`、`migrations/`、`tests/` 和 `deploy/` 只作为 v1 反例或通用基础设施候选，不具有 v2 语义权威。

## 4. 已冻结的实施决策

### 4.1 数据库与 Migration

采用**全新 v2 PostgreSQL Database**，不对已有 v1 Database 做原地升级：

- v2 使用独立 `DATABASE_URL`、数据库权限和 SQLx `_sqlx_migrations` journal；
- v2 migrator 只嵌入 `migrations/v2/`，例如 `sqlx::migrate!("migrations/v2")`；
- 现有三份 v1 migration 不进入 v2 journal，也不复制 v1 数据；
- 部署前置检查拒绝连接含 v1 业务表但没有 v2 marker 的数据库，防止误写旧库；
- Migration 由独立 `nli-migrate` 命令执行，API 进程启动时不自动执行 DDL；
- Migration 只前向推进。Expand、受控回填、read/write switch、contract cleanup 分开提交和发布；
- 每个 Migration 必须在空数据库、重复执行检查、checksum/journal 检查和最小权限应用账号下验证；
- 切流前演练数据库备份恢复。不可逆切流后使用 forward recovery，不声称可以恢复 v1 写入。

### 4.2 v1 隔离

- 新代码落在 `src/platform/` 与 `src/v2/`；v2 模块不得引用 v1 DTO、Repository、Redis 方法或 Signaling Connection Registry；
- 可审计后重写或移植：Bearer 语法解析、随机令牌、Secret Debug 遮蔽、数据库/Redis健康检查、Request ID、Timeout、Trace、Metrics、优雅停机；
- 禁止复用：`src/api/signaling.rs`、`src/model/signaling.rs`、`src/signaling.rs`、`src/api/turn.rs`、`src/db/friends.rs`、`src/redis.rs` 的 `nli:*` 数据模型，以及 v1 Minecraft Principal/Presence 权威语义；
- v1 删除是切流观测期后的独立提交，不与 Relay 实现或生产切流绑定。

### 4.3 运行边界

- `AppState` 只装配 application ports；Handler 不直接操作 SQLx/Redis；
- PostgreSQL 是领域生命周期、协议 Phase、幂等、Audit、Outbox、配额和最小 Receipt 的权威；
- LeaseStore/EventBus 使用独立 Redis 部署和 `nli:v2:{environment}:*` 前缀；敏感易失实例整体禁 RDB/AOF、Swap、通用备份和跨环境复制，不能假装按 Namespace 禁持久化；
- EventBus 只负责提示，不参与授权或 Signaling Payload 投递许可；
- 生产配置默认 `NLI_API_V2_ENABLED=false`、`NLI_SIGNALING_ENABLED=false`、`NLI_RELAY_ENABLED=false`；
- `NLI_RELAY_ENABLED` 只有实现验收 Gate 和人工发布审批同时满足后才能为 true。

### 4.4 Operation Policy Manifest

建立 checked-in `src/v2/http/operation_policy.rs`，逐 Operation 固定：

- method/path/operationId；
- Principal、Audience、Purpose 和 recent-auth 要求；
- 是否要求 `Idempotency-Key`；
- 是否允许 Secret Replay；
- Body limit；
- 公开状态码和 ProblemCode；
- Audit policy、Outbox event、Rate limit 和功能开关。

Manifest 从冻结 OpenAPI/REST 生成或由测试核对。不得把以下策略一刀切：

- 只有契约指定的非天然幂等写入要求 Idempotency-Key；
- 只有返回 Token/Secret 且文档允许 Replay 的操作保存加密 Replay；
- GET、天然幂等 DELETE 和普通 PUT 不得被额外要求 Idempotency-Key；
- 所有 94 个 Operation 都必须有 Principal 负向测试。

### 4.5 Outbox 与 Audit 故障方向

- 领域写入要求 Outbox 时，**Outbox insert 与领域状态在同一 PostgreSQL 事务**；insert 失败必须回滚领域事务；
- 提交后的 Dispatcher/EventBus 失败不回滚业务，由 Outbox 有界重试并最终标记 ABANDONED；
- Audit 使用逐事件 policy：
  - `REQUIRE_BEFORE_SUCCESS`：账号/会话创建、Grant/Secret 签发等在必要 Audit 未成功前不得公布成功；
  - `REVOCATION_WINS`：显式撤销、关闭、ban、额度阻断和安全终止必须先失效授权，不得因 Audit 写入故障维持旧授权；使用保存点隔离 Audit 错误，并由权威终态行驱动有界补写，最迟 60 秒告警；
- Audit 失败不得被吞掉；指标、告警和补写丢弃事件使用固定类型，不记录敏感 Payload。

### 4.6 发布与回滚

- v2在`hangzhou-traffic`绿地部署，使用全新PostgreSQL Database、一个v2专用无持久Redis实例和独立v2服务进程；不与v1共享逻辑Database/Schema、Redis进程/数据、Credential、文件或写模型；
- v1账号、好友、实例、邀请码、Join、信令和TURN数据均不导入、不映射、不继承；v2用户需要建立新的账号与关系；
- 生产 Gate E/F 路由按文档要求成组开放，不能把缺失路由伪装为94项已完成；
- 切流采用DNS/反向代理指向通过容量和故障验收的新环境；观测期保留上一个已验证v2二进制和v2数据库恢复点；
- v2接受第一笔生产写入前可以撤销切流；接受生产写入后不得回到v1而造成双写或丢弃v2事实，只能关闭功能、回退到兼容当前v2 Schema的前一v2版本或forward-fix；
- 无论v1当前是否也位于`hangzhou-traffic`，切流时v1都进入只读/维护状态且不成为v2故障转移；Phase 0实机清点其主机和依赖。若同机，v2可为节省资源共用PostgreSQL server process，但必须使用独立Database/owner/application role/备份；v2 Redis必须是独立无持久实例；
- v1只读服务最长保留到v2全量切流后14天，随后停止进程；v1历史仅按既定离线保留策略保存，不导入v2；
- v1代码、测试和部署样例只在v2全量观测期通过后删除；
- v2生产数据库每日执行一次加密custom-format逻辑备份，并在每次Migration前额外创建恢复点；备份必须传输到异机存储，保留14份每日和8份每周副本，每月至少完成一次隔离环境真实恢复演练。初始RPO目标24小时、RTO目标4小时，实测不满足时阻断Traffic Gate。

### 4.7 `hangzhou-traffic` 原生部署与容量边界

- 初始生产拓扑在 `hangzhou-traffic` 单机原生运行Nginx、一个v2 API/Worker进程、PostgreSQL和一个v2专用Redis进程；不使用容器，不继承v1数据；
- 为节省内存，默认由同一`nli-api-v2`进程运行HTTP/WS和有界后台Worker；Migration使用短期命令，不常驻第二个Worker进程；
- v2 Redis整实例都视为可丢失易失存储，关闭RDB/AOF，禁止Swap、通用备份和跨环境复制。不得与需要持久化的其他Redis用途共用实例；
- PostgreSQL使用全新v2 Database和独立owner/migration/application角色，可与服务同机并在资源受限时共用现有PostgreSQL server process，但不得使用v1 Database、Schema、owner或application role；
- systemd必须设置Restart、启动/停止超时、文件描述符上限、最小权限和资源限制；Worker使用有界批次，不能因清理/Outbox任务饿死认证、撤销、Lease续租或健康检查；
- 新环境必须监测CPU、RSS、连接数、PostgreSQL连接池/锁等待、Redis延迟、WS数量、网络吞吐和磁盘，并为认证/撤销/Lease保留容量；
- 当前单机没有TURN双故障域且网络压力已高，生产`NLI_RELAY_ENABLED`固定为false；只允许STUN/direct分支及RELAY明确不可用响应。未来至少两个TURN故障域通过Phase 8后才能重开生产Relay决策；
- 切流前执行HTTP、WS和Signaling混合负载；容量不足时拒绝新低优先级工作，不允许绕过授权或退化为节点本地权威。

### 4.8 WSL构建、手动上传与systemd发布

- WSL是受控Release Builder；提交`rust-toolchain.toml`并使用`Cargo.lock`和`cargo build --locked --release`。构建前执行格式、Clippy、单元、契约和本地原生依赖集成测试；
- 首次部署前记录远端`uname -m`、发行版和glibc版本。构建target必须匹配；优先使用经依赖验证的静态musl目标，否则WSL的glibc不得新于远端；
- WSL Gate必须在WSL原生Linux文件系统中的checkout运行，不从`/mnt/c`、`/mnt/d`等DrvFS路径构建；提交`.gitattributes`确保`*.sh`与systemd模板使用LF；
- Release产物包含二进制和`release-manifest.json`；Manifest记录Git commit、版本、Rust target/toolchain、`Cargo.lock` SHA-256、OpenAPI基线摘要、Redocly版本、测试执行数、时间戳和二进制SHA-256。先上传到`/opt/netherlink-v2/releases/<commit>/`临时文件并逐项远端校验；
- `current`切换使用同一文件系统内`ln -s <new-release> current.next && mv -T current.next current`，禁止使用非原子的`ln -sfn`；
- 配置位于`/etc/netherlink-v2/nli.env`，root拥有、服务组只读、权限`0640`；Secret不得出现在命令行、unit文件、release目录或日志；
- Migration由部署者使用独立migration凭据手动执行并核对journal；API systemd服务使用无DDL权限的application凭据；
- `nli-api-v2.service`使用专用非登录用户，至少启用`NoNewPrivileges`、`PrivateTmp`、`ProtectSystem=strict`、`ProtectHome=true`、空`CapabilityBoundingSet`、`UMask=0077`、`Restart=on-failure`、`LimitCORE=0`和受控`LimitNOFILE`；v2不写PrivateTmp之外的本地业务状态，不使用磁盘spool；日志只写journald；
- unit使用`Wants/After=network-online.target`，并在Phase 0按实机unit名称加入PostgreSQL和`nli-v2` Redis的`After=`；具体指令需在目标发行版执行`systemd-analyze verify`和`systemd-analyze security`验证兼容性；
- 发布顺序固定为：本地Gate→构建Manifest/SHA-256→上传临时路径→远端校验Manifest→创建并验证可读的异机v2数据库恢复点→Migration→原子切换软链接→`systemctl restart`→readiness/metrics/日志检查；任何contract阶段Migration必须先在隔离环境实际恢复该恢复点，失败则不得切换；
- 发布失败时只可切回兼容当前Schema的前一v2 release并重启；Migration不做down，必要时从本次恢复点恢复v2数据库后再forward-fix；
- `deploy/systemd/`和`scripts/release-wsl.sh`只保存无Secret模板/步骤。部署脚本在切换前必须校验`release-manifest.json`存在且内容匹配；不得把手动操作理解为可以跳过清单、校验和或验收记录。

## 5. 目标代码结构

```text
src/
  platform/
    config.rs
    clock.rs
    secrets.rs
    observability.rs
    shutdown.rs
  v2/
    domain/
    application/
    ports/
    adapters/
      postgres/
      lease_store/
      event_bus/
      provider/
    http/
      router.rs
      middleware.rs
      extractors.rs
      operation_policy.rs
    ws/
      notifications.rs
      signaling.rs
    workers/
    relay/
    state.rs
  bin/
    nli-migrate.rs
migrations/v2/
tests/v2/
scripts/
```

领域模块按 `account/provider/friendship/instance/join/signaling/relay` 分区；共享 Domain 不得依赖 Axum、SQLx、Redis 或 Provider SDK。

## 6. 分阶段交付计划

### Phase 0：基线、WSL发布门禁与可选CI

**目标**：先建立“什么算实现完成”的机械证明，不改变生产行为。

**任务**：

1. 将实现准备提交和`v2-design-gate-f`推送到`origin`；从包含全部准备决策的最新`master`创建`v2/phase-0-contract`分支/独立worktree并记录`git rev-parse`，契约漂移仍比较`3b4f329`；
2. 以独立提交确认`Cargo.toml`/`Cargo.lock`中的应用版本`0.2.0`，不与功能提交混合；
3. 提交`rust-toolchain.toml`并增加WSL Gate脚本；使用WSL原生PostgreSQL和v2专用无持久Redis运行依赖测试，不使用容器；可选CI先执行不依赖服务的格式、Clippy、单元和契约检查；
4. 增加 OpenAPI lint、全部本地 `$ref`、81/94、operationId 唯一和 JSON Schema 正反例检查；
5. 以 `3b4f329:doc/v2/openapi.yaml` 或其固定 SHA-256 做 drift 基线；普通 PR 不允许更新基线；
6. 建立两类清单：
   - `frozen_inventory` 始终为94项；
   - `mounted_route_manifest` 只包含当前真实装配路由，并按阶段允许子集校验；
7. 建立依赖集成测试脚本，禁止以全部 `#[ignore]` 的旧测试作为发布证据；
8. 实机清点`hangzhou-traffic`的架构、发行版/glibc、systemd与依赖unit名称、v1是否同机、磁盘/内存/网络基线；不读取或提交现有Secret；
9. 建立`release-manifest.json`生成/远端校验、Migration前异机备份和原子软链接切换脚本的无Secret骨架。

**验收**：

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
npm ci --ignore-scripts
npx --no-install redocly lint doc/v2/openapi.yaml
```

Phase 0提交锁定Redocly版本的`package.json`/`package-lock.json`，禁止Gate联网浮动选择版本。WSL Gate必须验证PostgreSQL/Redis服务测试实际执行且非ignored，并输出执行数和Release Manifest。CI不是必备发布依赖；一旦配置，CI失败同样阻断合并，但不能替代WSL依赖验收。

**完成定义**：WSL Gate可重复运行；冻结inventory为94，mounted manifest可为空但不能伪报已实现。

**回滚**：仅工具和测试，可独立 revert。

### Phase 1：平台层、全新 Migration Journal 与横切内核

**目标**：建立不含业务路由的 v2 运行骨架。

**Migration 批次**：

1. v2 marker、extensions、受控 Registry 和 Key purpose；
2. Account/Email/Username/Credential/Token Family与Generation；
3. Idempotency Record、Secret Replay、Audit、Outbox；
4. Worker checkpoint/claim所需结构。

**任务**：

- 实现独立 `nli-migrate`，移除 v2 API 启动时自动迁移；
- 建立 Clock、SecretStore、UnitOfWork、IdempotencyStore、AuditSink、OutboxStore、LeaseStore、EventBus ports；
- 实现完整 RFC9457 Problem、Request ID、Retry-After/WWW-Authenticate、closed DTO和端点级 Body Limit；
- 配置 Node ID、Boot ID、Region、环境 Namespace、Key version、Worker lease与三个默认关闭开关；
- Redis 原子 primitive：replace、renew-if-current、delete-if-current、read-current；
- 建立 Secret/Audit/Outbox 序列化前 Redaction 和禁止日志字段测试。

**验收**：

- 空数据库 migration、checksum、重复部署、最小权限启动；
- 错误连接 v1 数据库必须失败；
- Idempotency 同Key同摘要、同Key异摘要、PROCESSING接管和Secret tombstone测试；
- Outbox insert失败回滚领域fixture；Dispatcher失败不回滚已提交fixture；
- `REQUIRE_BEFORE_SUCCESS` 与 `REVOCATION_WINS` 故障注入测试；
- v2专用无持久Redis配置启动检查与CAS/TTL/重启测试。

**完成定义**：v2进程仅有内部 health/readiness/metrics，不装配任何冻结业务 Operation。

**开关/恢复**：`NLI_API_V2_ENABLED=false`；Schema只expand。

### Phase 2：Account、Auth、Device 与 Session（30 Operations）

**范围**：Auth 13 + Device Authorization 5 + Accounts 9 + Sessions 3。

**顺序**：

1. 注册、Email验证、登录和Refresh；
2. 密码恢复/修改、近期认证；
3. Account读取/修改/删除恢复；
4. Session列表/撤销；
5. Device Code创建、浏览器确认、轮询和消费。

**每个纵切必须同时完成**：Domain状态机、Repository、Handler、Operation Policy、幂等/Audit/Outbox/限流、契约测试和删除编排扩展。

**关键验收**：

- username大小写、Email隐藏、Provider Subject不参与本阶段身份合并；
- Refresh轮换、60秒安全重放、旧Token重用导致Family compromised；
- Device并发确认/轮询只有一个消费赢家；
- 全部30项 Principal/Audience/recent-auth/未知字段/Problem负向矩阵；
- Account删除只失效已部署依赖；未部署下游路由保持关闭。

**完成定义**：mounted manifest 精确包含本阶段30项，未实现路由不开放。

**恢复**：关闭 Auth 路由组；数据库不逆迁移。

### Phase 3：Provider、Friendship 与 Sync（30 Operations）

**范围**：Provider Login 7 + Providers 2 + Provider Bindings 6 + Friendship 11 + Friend Sync 4。

**任务**：

- 建立 Provider port、Login Identity与业务Binding分离、版本化AEAD Credential Manager；
- 实现 purpose-aware OAuth Flow Cookie/State和固定Callback；
- 实现单行Friend Pair、有向ban、Request/Submission、Provider投影、同步Task/Continuation；
- 只按v2 port重新实现Minecraft Adapter，不复制v1好友图或MC Principal逻辑；
- 扩展Account删除、Provider解绑、ban和Friend变化的撤销编排。

**关键验收**：Provider Subject抢占、用途混淆、Binding revision、Credential reauth、反向请求接受、ban阻止自动同步、Cursor快照、Singleflight、上游故障隔离、Outbox恢复。

**完成定义**：本阶段30项契约通过；累计60项 mounted。

**恢复**：Provider逐个禁用；NLI原生Account/Friend不依赖Provider可用性。

### Phase 4：Instance、Notification、ACL、Proxy、Invite、Guest、Join 与 Report（29 Operations）

**范围**：Instances 6 + ACL 2 + Proxy Grants 6 + Invites 5 + Guest 1 + Join 8 + Reports 1。

**任务**：

- 实现Token Family单Instance绑定、Instance生命周期和Phase 5通知WS；
- LeaseStore只保存current WS/ONLINE租约，业务通知只作hint；
- 实现有序ACL首条匹配、revision、Proxy、Invite/Resolution/Reservation；
- 实现Guest 5分钟单流程、Join Request/AUTO或MANUAL审批、Acceptance Lease 60秒；
- 所有Source、Identity Traits和CLIENT_CLAIMED Profile由服务端推导或受控复制；
- 扩展Account/Family/Instance/Friend/Proxy/Invite/Guest的撤销和删除orchestrator。

**关键验收**：

- 双请求绑定Instance只有一个赢家；旧WS Close不能删除新Lease；Redis丢失令Instance OFFLINE；
- ACL更新与Join Accept锁序竞态；Invite轮换/超卖/消费修复；
- Guest猜测、错误Session、第三方UUID统一404；
- AUTO Join同事务、Report越权和敏感字段不可序列化；
- 全29项Principal负向矩阵以及通知丢失后的HTTP恢复。

**完成定义**：Gate E 89项全部真实装配，route manifest与Gate E inventory零差集；经过独立预生产验收后才允许成组开放Gate E。

**恢复**：按路由组关闭；Lease compare-delete只删除自身fence。

### Phase 5：Gate E 集成与运行准备

**目标**：在进入Signaling前证明HTTP/PostgreSQL基础完整。

**任务/验收**：

- 89 Operation Schemathesis/契约套件；
- 全局锁序并发测试和Deadlock有界重试；
- 所有Idempotency/Secret/Audit/Outbox policy覆盖率报告；
- Outbox Dispatcher、TTL/Sweeper、删除编排重复执行和故障恢复；
- 依赖故障、限流、404防枚举、Header和Body Limit矩阵；
- 日志/数据库/Outbox/Audit扫描确认无明文Token、Credential、Profile禁区或网络敏感原值；
- 备份恢复、Worker停机、EventBus完全丢失演练。

**完成定义**：Gate E实现验收通过；Gate F所有路由仍关闭。

### Phase 6：Gate F Signaling Session 与多节点WS（Relay仍关闭）

**数据库**：`signaling_sessions`、`signaling_candidate_receipts`及索引/清理。

**5个Operation必须同时存在**：

- create-or-attach；
- get；
- delete；
- Signaling WS Upgrade；
- ICE Servers。

Relay关闭时ICE Operation仍按冻结契约响应：有效策略为ALL时可返回明确STUN-only `UNAVAILABLE` union；RELAY或服务端强制Relay返回 `503 TURN_UNAVAILABLE`。不得缺路由。

**Route port必须提供单一原子操作族**：

- session-scoped Route Document和`route_revision`；
- 双方Slot均含`node_id/boot_id/connection_id/expires_at`；
- replace、renew、compare-delete、check-both共用同一序列化点；
- PG use-time前置重验；CAS后复核和失败compare-delete补偿；
- 入队前单次check-both；最长250ms单消息投递许可；
- 写Socket前再次检查接收fence与截止；
- 节点RPC相互认证且加密；EventBus不得参与投递授权。

**协议任务**：Phase/epoch、Offer/Answer、最多2次Restart、Candidate Receipt、Delivery ACK、Result/Error、8帧在途、背压、关闭码、敏感易失缓存与恢复。

**验收必须启动至少两个真实服务进程**：多节点协议验收先在WSL/受控非生产环境运行同版本双实例；跨真实网络和故障域验收使用未来临时双机环境。`hangzhou-traffic`单机生产拓扑不替代这些Gate。

- 两Role连接替换和旧节点延迟帧；
- check-both与replace竞争；
- PG提交/Route CAS/入队/Socket写各崩溃点；
- Redis重启、RPC失败、ACK丢失、乱序/重复；
- 64 Candidate、3 epoch、Frame/队列/速率上限；
- 授权撤销与最多250ms已批准尾部；
- SDP/Candidate不进入PG、Outbox、Audit、普通日志、磁盘spool或Crash Dump。

**完成定义**：Gate F五项在预生产原子开放并可完成直连；`NLI_RELAY_ENABLED=false`。

**恢复**：关闭Signaling组，现存Session fail closed；不恢复v1。

### Phase 7：Relay持久模型与隐藏Authorizer

**数据库顺序**：Relay Grant → Quota Bucket → Allocation Slot → Runtime Permit；所有FK、partial unique和索引按冻结模型实现。

**任务**：

- Grant/Role/Principal/Policy绑定；
- `LIVE_GAUGE/FIXED_WINDOW/LEDGER`配额桶；
- Prepare/Activate、同five-tuple单live Slot、node/boot/fence；
- 5秒续签、最长15秒Permit、2秒时钟误差；
- Peer IP/endpoint Pin、byte-credit、nonce/sequence/revision；
- Secret Replay Tombstone、Orphan Reaper和撤销Worker；
- mTLS内部Authorizer API，不暴露NLI Bearer给TURN协议。

**清理状态机验收**：

1. Permit达到安全截止并删除；
2. Slot关闭/删除并释放并发Quota；
3. Grant删除；
4. Candidate Receipt删除；
5. Signaling Session删除。

任一步失败必须保留父行、告警并有界重试；FK错误不是成功。Account/Family/Guest删除必须先撤销Grant并等待/驱动上述链。缺失撤销上下文只能fail closed。

**完成定义**：Authorizer在Mock Adapter下通过并发与故障注入；公共Relay仍关闭，Mock不算生产验收。

### Phase 8：自定义TURN Adapter与真实验收

**任务**：实现或集成支持内部Authorizer的TURN Adapter；现有普通coturn模板不得作为v2生产方案。

**真实验收矩阵**：

- 标准WebRTC栈：UDP/TCP/TLS、IPv4/IPv6；
- PENDING重传合并、Activate提交后才返回success；
- 同five-tuple单live Slot；6并发/24累计；2GiB；8Mbit/s与burst；
- 60秒新建窗口、窗口外仅维护、1小时绝对截止；
- 5秒续期/15秒Permit、分区停止、boot fencing；
- Peer Pin、CreatePermission/ChannelBind、IPv4-mapped IPv6和metadata/管理网段denylist；
- 撤销、Secret Store丢失、PG不可达、Adapter/节点崩溃、Orphan Reaper；
- 未使用Allocation标准`Refresh(LIFETIME=0)`；
- 日志、备份、Swap、Crash Dump和抓包边界检查。

**完成定义**：形成可复现验收报告；Relay开关仍保持false，等待发布审批。

### Phase 9：原生客户端/黑盒Conformance Harness

客户端仓库未确定前，本阶段是显式外部阻塞；不得在服务端仓库伪造“客户端已实现”。至少交付原生/桌面/Mod参考客户端或等效黑盒Harness：

- 可设置Upgrade `Authorization` Header并主动发送标准Ping；
- 机械执行`signaling.md`冻结的12步算法和28个场景；
- create-or-attach→GET→ICE policy→最新WS generation；
- SDP→Candidate→ice_end；ACK只表示收到，不删除唯一副本；
- ACK超时先GET且不抢占健康自身连接；
- RELAY必须`iceTransportPolicy=relay`并检查SDP/Trickle两条出站路径；
- RELAY失败禁止静默直连；
- 未使用TURN Allocation尽力Refresh(0)；
- 浏览器JavaScript WebSocket路径必须明确拒绝或标记不支持。

**完成定义**：跨真实网络、双节点服务和真实Adapter在临时双机非生产环境重复通过；结果必须在未来独立TURN环境复现，`hangzhou-traffic`当前不承载生产Relay。

### Phase 10：新服务器Canary、全量切流与v1移除

拆成四个独立发布Gate：

1. **Hidden Relay Gate**：代码和内部Adapter部署但Relay关闭；
2. **Client Gate**：参考客户端与真实验收通过；
3. **Traffic Gate**：在新服务器完成独立PostgreSQL/Redis备份恢复、容量和混合负载演练后，内部canary→小比例新客户端→全量DNS/代理切流并完成观测期；
4. **Removal Gate**：最后删除v1代码、旧migration装配、旧测试和coturn/nginx v1模板，并按保留策略下线旧服务器。

**切流前**：明确公告v2为空数据新系统、账号和关系需重新建立；记录DNS TTL、代理配置、新环境容量基线、前一兼容v2二进制和v2恢复点。旧服务器保持独立，不复制v1业务数据到新环境。

**切流边界**：旧服务器先进入只读/维护状态，再把流量导向v2；在v2接受第一笔生产写入前可撤销切流。此后禁止重新开放v1写入或把v1作为故障转移，以免形成两个不一致的业务权威。

**切流后恢复**：优先关闭Relay、Signaling或受影响路由组；回退到兼容当前v2 Schema的前一v2版本；必要时恢复v2数据库并forward-fix。

**最终完成定义**：94项契约、Worker、多节点、Relay和客户端证据齐全；v1删除提交独立可审计；生产Relay仅在人工审批后开启。

## 7. 测试矩阵

| 层级 | 必须证明 |
|---|---|
| Domain单元 | closed enum/DTO、UnixMillis、状态转换、ACL、Phase/epoch、摘要和Redaction |
| PostgreSQL | 空库Migration、FK/unique/check/index、锁序、并发唯一赢家、TTL和清理链 |
| LeaseStore | 原子CAS、Route Document、双角色check-both、TTL、重启、分区和旧fence |
| HTTP | 当前阶段mounted manifest、状态/Header/Body、Problem、幂等、404防枚举 |
| Principal | 94项Audience/Purpose/Family/Instance/Guest/recent-auth/错误绑定负向矩阵 |
| WS | Ping/Lease、replace、epoch、ACK、背压、跨节点、恢复、撤销尾部 |
| Worker | Outbox恢复、Audit补写、有界批次、SKIP LOCKED、幂等删除、父子顺序 |
| Security | 明文扫描、日志注入、Secret轮换/缺Key、SSRF/egress、备份/Swap/Dump |
| Relay | 真实WebRTC/TURN、Permit、Pin、Quota、崩溃和故障；Mock不算生产证据 |
| Client | 12步算法、28场景、RELAY不降级、Refresh(0)、浏览器WS不支持 |

每个依赖测试使用独立Database或Schema、唯一Redis Prefix；并发/多节点测试不得只用单进程Mock。

## 8. WSL Release Gate、可选CI与契约漂移门禁

每个PR由可选CI执行无外部依赖的静态检查；每个合并/Release Candidate必须附带一次本地WSL Gate记录，至少包括：

1. `cargo fmt --check`、Clippy `-D warnings`、unit/integration compile；
2. WSL实际启动原生PostgreSQL和v2专用无持久Redis并运行非ignored集成测试；
3. OpenAPI 3.1、全部local ref、固定81/94、唯一lowerCamelCase operationId；
4. 当前mounted route与阶段允许Operation集合双向差集；
5. 对`3b4f329:doc/v2/openapi.yaml`执行breaking drift检查；
6. JSON Schema/AJV正反例和已装配Operation的黑盒契约测试；
7. Migration空库/checksum/最小权限检查；
8. Secret/SDP/ICE/IP禁区静态与运行产物扫描。

冻结文档变化应直接使WSL Gate和可选CI失败，并要求显式Gate重开记录，而不是自动接受新基线。手工上传不允许绕过Release Gate记录。

## 9. 分支与提交策略

- 契约漂移比较基线保持在`3b4f329`和tag `v2-design-gate-f`，并将基线提交及annotated tag推送到`origin`；
- 实现分支从包含全部实现准备决策的最新`master`创建，不从`3b4f329`直接创建；第一条分支为`v2/phase-0-contract`，后续使用`v2/phase-1-platform`等短分支；
- 每个提交只含一种职责：Migration、Domain/port、Adapter、HTTP/WS、Worker、测试、部署或开关；
- v2应用版本固定为`0.2.0`，Cargo版本号以独立的实现准备提交纳入，不与功能修改混合；
- 每阶段先合并不可见基础，再合并默认关闭的路由，最后单独提交开关；
- Schema的expand/backfill/switch/contract分开；
- v1删除最后单独提交；
- 不重写或amend设计基线提交。

## 10. 首批可交付切片

首批实现只覆盖Phase 0和Phase 1，不开放任何业务路由：

1. 从最新实现准备提交建立独立worktree/分支、WSL Gate脚本和可选静态CI；
2. 固定OpenAPI inventory、阶段mounted manifest和drift门禁；
3. v2目录、Problem/Principal/Operation Policy骨架；
4. 独立v2 Database marker与`nli-migrate`；
5. Idempotency/Audit/Outbox/Secret ports及故障方向测试；
6. v2专用无持久Redis部署检查和基础CAS；
7. 所有功能开关默认false。

该切片结束后再批准Phase 2注册→验证→登录→Refresh纵切。

## 11. 风险与实现阻塞

| ID | 风险/阻塞 | 解除条件 |
|---|---|---|
| I-DB-01 | 误用旧Database或SQLx journal | 独立v2 DB、marker、专用migrator和错误库拒绝测试通过 |
| I-BUILD-01 | WSL手工构建可能遗漏测试或产物不可在远端运行 | 锁toolchain/lockfile、原生依赖Gate、target/glibc预检、SHA-256和Release metadata通过 |
| I-CONTRACT-01 | 94项范围被“路由占位”伪装完成 | frozen inventory与mounted manifest分离并按阶段零差集 |
| I-AUDIT-01 | 撤销因Audit失败而回滚 | 两类Audit policy和故障注入通过 |
| I-ROUTE-01 | 跨PG/Redis/节点竞态 | 两真实进程、原子check-both、250ms许可和写前fence测试通过 |
| B-09-TURN | 自定义Adapter与真实兼容性未知 | Phase 8全部真实验收；之前生产Relay保持false |
| I-CLIENT-01 | 客户端仓库/技术栈未确定 | 指定原生/Mod交付仓库并实现12步/28场景Harness |
| I-CUTOVER-01 | v1无数据继承且v2首笔生产写入后不可回到v1 | 空数据重启公告、旧服只读边界、v2备份恢复和forward-recovery演练通过 |
| I-CAPACITY-01 | `hangzhou-traffic`单机资源/网络压力挤压认证、撤销或Lease | systemd资源边界、Worker有界批次、容量预算和HTTP/WS/Signaling混合负载通过；Relay保持关闭 |
| I-AVAIL-01 | `hangzhou-traffic`单机宕机导致v2整体不可用 | 接受单机SPOF；异机备份、24h RPO/4h RTO和异机重建Runbook经恢复演练通过 |
| I-SUPPLY-01 | Rust 1.94与新增依赖供应链 | WSL/可选CI锁定toolchain和Redocly，依赖审计和许可证检查通过 |

## 12. 当前证据与未完成事项

制定计划时已执行：

- `cargo test --all-targets`：46个现有单元测试通过，11个依赖型旧集成测试仍为ignored；这些只证明v1基线可构建，不证明v2；
- `cargo fmt --all -- --check`：通过；
- `cargo clippy --all-targets --all-features -- -D warnings`：通过；
- Gate F文档/OpenAPI机械校验已在设计基线前通过。

尚未执行任何v2 Migration、Rust API、LeaseStore、TURN Adapter或客户端实现测试。实现只能按阶段完成定义逐项更新证据。
