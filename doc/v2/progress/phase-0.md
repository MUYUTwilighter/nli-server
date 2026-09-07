---
schemaVersion: 1
phase: 0
phaseStatus: IN_PROGRESS
currentSlice: P0-04
currentSliceStatus: READY
updatedAt: 2026-09-07T07:43:41Z
---

# Phase 0 进度账本：基线、WSL 发布门禁与可选 CI

> 本账本只记录 [`implementation_plan.md`](../implementation_plan.md) Phase 0 的执行事实与证据，不定义新契约。当前摘要见 [`implementation_progress.md`](../implementation_progress.md)。

## Phase 完成定义

以实施计划 Phase 0 为权威。简要判定：WSL Gate 可重复运行；冻结 inventory 为 94；mounted manifest 不伪报实现；应用只接受无版本路由；代理黑盒测试证明公共 `/v2` 的选择、剥离及绕过拒绝。

## 当前交接

| 字段 | 当前值 |
| --- | --- |
| 当前切片 | `P0-04` |
| 状态 | `READY` |
| 已完成 | `P0-01` clean-slate基线、`P0-02`版本0.2.0、`P0-03`最小Cargo/锁定Rust 1.94/WSL原生依赖Gate均完成；18项跨Phase E2E清单已迁入实现计划 |
| 下一动作 | 固定Redocly/npm lock并建立OpenAPI lint、81/94、1422 local `$ref`、operationId、Schema与routing baseline drift门禁 |
| 阻塞 | 无 |
| 非目标 | 不开放业务路由，不实现数据库/认证/信令；不读取或提交本地 `.env`、`target/` |

## 工作项

状态只允许：`NOT_STARTED`、`READY`、`IN_PROGRESS`、`BLOCKED`、`COMPLETE`。

| ID | 实施计划映射 | 任务 | 状态 | 前置条件 | 证据 |
| --- | --- | --- | --- | --- | --- |
| `P0-01` | Task 1、5 + D-330 | 让 D-329/进度入口进入 `master` 并创建/推送 routing tag；在 Phase 0 分支删除全部 v1 工程资产，形成 clean-slate baseline并记录 SHA | `COMPLETE` | — | doc `d99ebb4`；tag object `f8b53d8` → `d99ebb4`；OpenAPI SHA-256 `bea05474c9502318407ec6b8a62ba675807ed2d8cb9a268c12b8e47669d521ed`；cleanup `49a78b7`；均已推送 |
| `P0-02` | Task 2 | 独立确认 Cargo 应用版本 `0.2.0` | `COMPLETE` | — | commit `b30b536`；已核实为当前 HEAD 祖先，`Cargo.toml` / `Cargo.lock` 为 `0.2.0` |
| `P0-03` | Task 3、7 | 建立最小 v2 Cargo target并清理旧依赖；固定 Rust toolchain，建立 WSL Gate 和真实依赖测试入口 | `COMPLETE` | `P0-01` COMPLETE | `2dc44bd` + `0dfaa42`；零运行时依赖，dev-only sqlx/redis/tokio；Windows 2项bootstrap、WSL 2项非ignored依赖测试通过 |
| `P0-04` | Task 4、5 | 固定 Redocly/npm lock，并建立 OpenAPI lint、81/94、local ref、operationId、Schema 和 drift 门禁 | `READY` | `P0-01` | 待记录锁定版本、测试与基线 digest |
| `P0-05` | Task 6 | 建立 frozen inventory、阶段 mounted manifest 与代理黑盒清单 | `NOT_STARTED` | `P0-04` | 待记录清单和双向差集 |
| `P0-06` | Task 8 | 无 Secret 地清点 `hangzhou-traffic` 环境和容量基线 | `NOT_STARTED` | 授权访问目标环境 | 待记录脱敏结果位置 |
| `P0-07` | Task 9 | 建立 Release Manifest、备份和原子切换脚本骨架 | `NOT_STARTED` | `P0-03` | 待记录脚本和 dry-run |
| `P0-08` | Phase 验收 | 执行完整 Phase 0 Gate 并确认完成定义 | `NOT_STARTED` | `P0-02`～`P0-07` | 待记录全部命令与输出摘要 |

## P0-03 最小上下文包

稳定导航键：`Phase 0 Task 3`、`目标代码结构`、`rust-toolchain.toml`、`Cargo.toml`、`WSL Gate`。

必读：

1. [`implementation_plan.md`](../implementation_plan.md)：“4.2 v1 隔离”“Phase 0”“目标代码结构”；
2. [`index.md`](../index.md)：“Phase 0：基线、门禁和发布骨架”；
3. 项目根 `Cargo.toml` / `Cargo.lock` 和 [`.pi/APPEND_SYSTEM.md`](../../../.pi/APPEND_SYSTEM.md)。

验收：

- 只建立最小 `src/platform/` / `src/v2/` 编译骨架，不挂载任何业务路由；
- `Cargo.toml` 不再引用已删除 target，旧实现专用依赖被移除或有当前 Phase 依据；
- toolchain/lock 固定，格式、Clippy、单元和契约检查可重复；
- WSL Gate 明确真实 PostgreSQL/v2 专用无持久 Redis 测试不得全部 ignored；
- Pi trust + restart/`/reload` 的入口检查可复现；
- 不读取、提交或记录 `.env`、Secret 与 `target/` 产物。

## 执行证据

只记录实际执行结果；失败也必须保留摘要。大输出保存到版本化测试报告或 CI Artifact，本表只放路径/摘要。

| 时间（UTC） | 切片 | 命令/检查 | 结果 | 证据位置或摘要 |
| --- | --- | --- | --- | --- |
| 2026-09-07 | `P0-01` 准备 | OpenAPI YAML/路径/operationId/local ref 路由断言 | `PASS` | 81 Paths、94 Operations、1422 local `$ref`；OpenAPI paths 无 `/v1`/`/v2` 前缀 |
| 2026-09-07 | `P0-01` 准备 | Redocly 2.51.2 lint | `PASS_WITH_WARNINGS` | API description valid；8 个冻结且已解释 Warning |
| 2026-09-07 | `P0-01` 准备 | Markdown relative links、已删除临时文件的悬空引用、`git diff --check` | `PASS` | 链接有效、无悬空引用、无 whitespace error |
| 2026-09-07 | 进度体系 | Fresh limited-context Reader Test | `APPROVE` | 能回答当前状态/切片/下一动作/阻塞/最小读取/更新协议/完成证据；3 个 warning 已修正 |
| 2026-09-07 | Pi 入口 | `.pi/APPEND_SYSTEM.md` 路径、默认提示词追加语义和导航链接检查 | `PASS` | 使用官方项目级追加文件；不替换 Pi 默认系统提示词；需项目 trust + restart/`/reload` |
| 2026-09-07 | `P0-01` | doc-only baseline commit/tag/push | `PASS` | `origin/master=d99ebb4`；tag object `f8b53d8` peel 到 `d99ebb4`；OpenAPI SHA-256 见工作项 |
| 2026-09-07 | `P0-01` | clean-slate v1 资产删除与 push | `PASS` | commit `49a78b7`：59 files、9876 deletions；`origin/v2/phase-0-contract=49a78b7`；本地 `.env` / `target/` 保留且未提交 |
| 2026-09-07 | `P0-03` | Windows Rust 1.94：`cargo fmt/build/clippy/test --locked` | `PASS` | 1项lib单元测试+1项binary fail-closed测试；0 failed/ignored；Clippy `-D warnings` |
| 2026-09-07 | `P0-03` | WSL原生Git checkout：`scripts/wsl-gate.sh` | `PASS` | Rust 1.94.0；PostgreSQL 14 smoke与临时Redis 6 smoke共2项，2 passed/0 failed/0 ignored；Redis RDB/AOF断言通过 |
| 2026-09-07 | `P0-03` | 从DrvFS执行WSL Gate | `EXPECTED_REJECT` | `/mnt/d`被拒绝，exit 1；要求WSL原生Linux文件系统checkout |
| 2026-09-07 | `P0-03` | 非法`NLI_TEST_POSTGRES_SERVICE` | `EXPECTED_REJECT` | `postgresql;id`在sudo前被白名单拒绝，exit 1 |
| 2026-09-07 | `P0-03` | 初次手工依赖测试 | `FAIL_THEN_FIXED` | WSL调用用户与peer role不一致导致PG失败；准备脚本改为输出显式WSL role URL并验证DB owner，最终Gate通过 |
| 2026-09-07 | `P0-03` | 独立只读review | `PASS_AFTER_FIXES` | 无P0；关闭执行数硬编码、sudo service name白名单和进度欠账3个P1 |
| 2026-09-07 | 跨Phase验收迁移 | E2E-01～E2E-18数量/状态检查 | `PASS` | commit `5e265c2`；18项均在`implementation_plan.md`且保持`NOT_STARTED` |

## 实现局部决策

这里只能记录不改变冻结契约的实现选择。改变 API、领域、数据或安全语义时必须停止并重开 Gate。

| ID | 切片 | 决策 | 理由 | 状态 |
| --- | --- | --- | --- | --- |
| `I-P0-001` | `P0-01` | 提前删除 active tree 中全部 v1 工程资产；已部署 v1 的运行/下线仍留到 Phase 10 | 用户明确选择 clean-slate，并授权提交、tag 与 push；避免后续 Agent 误复用旧实现 | `APPROVED` |
| `I-P0-002` | `P0-03` | Phase 0保持零运行时依赖；sqlx/redis/tokio仅作为dev依赖支持显式feature服务冒烟测试 | 恢复可构建但不提前引入API/领域栈；`--locked`固定解析结果 | `APPROVED` |
| `I-P0-003` | `P0-03` | bootstrap binary恒失败退出且不监听端口；`required-features`依赖测试只由WSL Gate启用 | fail closed且避免普通无服务测试伪失败；Gate解析并强制至少2 passed/0 ignored | `APPROVED` |

## 残余风险

- P0-04～P0-07的OpenAPI、manifest、代理、实机和发布门禁尚未实现；不能把P0-03 Gate当作完整Release Gate。
- Phase 0 bootstrap binary故意失败退出；P0-07 systemd模板必须注明当前binary禁止部署，否则`Restart=on-failure`会形成crash loop。
- WSL smoke使用PostgreSQL 14和独占临时Redis 6；生产版本、架构和systemd unit仍由P0-06实机清点确认。
- sqlx传递依赖包含dotenvy；当前仅测试代码使用，Phase 1升为运行时依赖时必须确保生产配置只来自受控systemd环境文件。
- `.idea/`和空`AGENT.md`是既有跟踪资产，不属于P0-03；未跟踪空`config/`不会进入Git。
- `B-09-TURN`仍阻止生产Relay开启，但不阻止P0-04。

## Phase 账本模板

激活下一个 Phase 时复制以下结构到 `phase-N.md`，然后删除模板说明：

```markdown
---
schemaVersion: 1
phase: N
phaseStatus: IN_PROGRESS
currentSlice: PN-01
currentSliceStatus: IN_PROGRESS
updatedAt: <UTC ISO-8601>
---

# Phase N 进度账本：<名称>

## Phase 完成定义
## 当前交接
## 工作项
## 当前切片最小上下文包
## 执行证据
## 实现局部决策
## 残余风险
```
