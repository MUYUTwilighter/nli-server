---
schemaVersion: 1
phase: 0
phaseStatus: IN_PROGRESS
currentSlice: P0-01
currentSliceStatus: IN_PROGRESS
updatedAt: 2026-09-07T06:57:24Z
---

# Phase 0 进度账本：基线、WSL 发布门禁与可选 CI

> 本账本只记录 [`implementation_plan.md`](../implementation_plan.md) Phase 0 的执行事实与证据，不定义新契约。当前摘要见 [`implementation_progress.md`](../implementation_progress.md)。

## Phase 完成定义

以实施计划 Phase 0 为权威。简要判定：WSL Gate 可重复运行；冻结 inventory 为 94；mounted manifest 不伪报实现；应用只接受无版本路由；代理黑盒测试证明公共 `/v2` 的选择、剥离及绕过拒绝。

## 当前交接

| 字段 | 当前值 |
| --- | --- |
| 当前切片 | `P0-01` |
| 状态 | `IN_PROGRESS`：用户已授权删除全部 v1 工程资产并完整整理/push Git |
| 已完成 | Gate F 核心基线存在；独立版本提交 `b30b536` 已把 Cargo 版本设为 `0.2.0`；D-329、进度体系和 Pi 入口已在工作区完成并验证 |
| 下一动作 | 形成并推送 `master` doc-only baseline 和 `v2-design-gate-f-routing`；在 Phase 0 分支独立删除 `src/`、`tests/`、旧 `migrations/`、`deploy/`、`config/terms`、`doc/v1/`、`.env.example`，提交并推送 |
| 阻塞 | 无用户授权阻塞；push 前仍须确认远端未前进并完成机械验证 |
| 非目标 | 不编写 v2 业务代码；不移动旧 `v2-design-gate-f`；不删除本地 `.env` 或 `target/`；不提交 Secret/构建产物 |

## 工作项

状态只允许：`NOT_STARTED`、`READY`、`IN_PROGRESS`、`BLOCKED`、`COMPLETE`。

| ID | 实施计划映射 | 任务 | 状态 | 前置条件 | 证据 |
| --- | --- | --- | --- | --- | --- |
| `P0-01` | Task 1、5 + D-330 | 让 D-329/进度入口进入 `master` 并创建/推送 routing tag；在 Phase 0 分支删除全部 v1 工程资产，形成 clean-slate baseline 并记录 SHA | `IN_PROGRESS` | push 前远端无竞态；删除范围已获用户确认 | 已核实旧 `v2-design-gate-f` 仍指向 `3b4f329`；待记录新 commit/tag/cleanup commit/OpenAPI SHA-256 |
| `P0-02` | Task 2 | 独立确认 Cargo 应用版本 `0.2.0` | `COMPLETE` | — | commit `b30b536`；已核实为当前 HEAD 祖先，`Cargo.toml` / `Cargo.lock` 为 `0.2.0` |
| `P0-03` | Task 3、7 | 固定 Rust toolchain，建立 WSL Gate 和真实依赖测试入口 | `NOT_STARTED` | `P0-01` | 待记录脚本、命令、执行数 |
| `P0-04` | Task 4、5 | 固定 Redocly/npm lock，并建立 OpenAPI lint、81/94、local ref、operationId、Schema 和 drift 门禁 | `NOT_STARTED` | `P0-01` | 待记录锁定版本、测试与基线 digest |
| `P0-05` | Task 6 | 建立 frozen inventory、阶段 mounted manifest 与代理黑盒清单 | `NOT_STARTED` | `P0-04` | 待记录清单和双向差集 |
| `P0-06` | Task 8 | 无 Secret 地清点 `hangzhou-traffic` 环境和容量基线 | `NOT_STARTED` | 授权访问目标环境 | 待记录脱敏结果位置 |
| `P0-07` | Task 9 | 建立 Release Manifest、备份和原子切换脚本骨架 | `NOT_STARTED` | `P0-03` | 待记录脚本和 dry-run |
| `P0-08` | Phase 验收 | 执行完整 Phase 0 Gate 并确认完成定义 | `NOT_STARTED` | `P0-02`～`P0-07` | 待记录全部命令与输出摘要 |

## P0-01 最小上下文包

稳定导航键：`D-329`、`x-routing-boundary`、`servers: /v2`、`v2-design-gate-f-routing`。

必读：

1. [`implementation_plan.md`](../implementation_plan.md)：“3.1 设计冻结迁移记录”“4.3 运行边界”“Phase 0”；
2. [`common.md`](../common.md)：“API 版本”；
3. [`openapi.yaml`](../openapi.yaml)：`info.x-routing-boundary` 与 `servers`。

验收：

- 正式 commit 包含 D-329 文档、`.pi/APPEND_SYSTEM.md`、实现导航和进度体系；
- `v2-design-gate-f` 仍指向 `3b4f329`；
- 新 annotated tag `v2-design-gate-f-routing` 精确指向正式文档 commit；
- 记录 `git rev-parse`、tag object、`doc/v2/openapi.yaml` SHA-256；
- 81 Paths / 94 Operations、1422 local `$ref`、lowerCamelCase operationId、Redocly、Markdown 链接和 `git diff --check` 通过；
- 不包含实现代码或 Secret。

## 执行证据

只记录实际执行结果；失败也必须保留摘要。大输出保存到版本化测试报告或 CI Artifact，本表只放路径/摘要。

| 时间（UTC） | 切片 | 命令/检查 | 结果 | 证据位置或摘要 |
| --- | --- | --- | --- | --- |
| 2026-09-07 | `P0-01` 准备 | OpenAPI YAML/路径/operationId/local ref 路由断言 | `PASS` | 81 Paths、94 Operations、1422 local `$ref`；OpenAPI paths 无 `/v1`/`/v2` 前缀 |
| 2026-09-07 | `P0-01` 准备 | Redocly 2.51.2 lint | `PASS_WITH_WARNINGS` | API description valid；8 个冻结且已解释 Warning |
| 2026-09-07 | `P0-01` 准备 | Markdown relative links、已删除临时文件的悬空引用、`git diff --check` | `PASS` | 链接有效、无悬空引用、无 whitespace error |
| 2026-09-07 | 进度体系 | Fresh limited-context Reader Test | `APPROVE` | 能回答当前状态/切片/下一动作/阻塞/最小读取/更新协议/完成证据；3 个 warning 已修正 |
| 2026-09-07 | Pi 入口 | `.pi/APPEND_SYSTEM.md` 路径、默认提示词追加语义和导航链接检查 | `PASS` | 使用官方项目级追加文件；不替换 Pi 默认系统提示词；需项目 trust + restart/`/reload` |

## 实现局部决策

这里只能记录不改变冻结契约的实现选择。改变 API、领域、数据或安全语义时必须停止并重开 Gate。

| ID | 切片 | 决策 | 理由 | 状态 |
| --- | --- | --- | --- | --- |
| `I-P0-001` | `P0-01` | 提前删除 active tree 中全部 v1 工程资产；已部署 v1 的运行/下线仍留到 Phase 10 | 用户明确选择 clean-slate，并授权提交、tag 与 push；避免后续 Agent 误复用旧实现 | `APPROVED` |

## 残余风险

- 工作区尚未形成可引用的正式 doc-only baseline commit；仪表盘 `lastEvidenceCommit` 仍只是清理前证据。
- `v2-design-gate-f-routing` 尚未创建或推送；push 前必须 fetch 并验证 `origin/master` 未前进。
- clean-slate 删除后 `Cargo.toml` / `Cargo.lock` 暂时保留，但显式 target 指向将不存在；P0-03 必须先建立最小 v2 编译骨架并清理依赖，期间不得把 Cargo 失败误记为回归。
- Phase 0 的工具链、CI/WSL Gate、manifest 和真实依赖验证均未实现。
- `B-09-TURN` 仍阻止生产 Relay 开启，但不阻止 Phase 0。

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
