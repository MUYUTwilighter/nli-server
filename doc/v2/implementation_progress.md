---
schemaVersion: 1
updatedAt: 2026-09-07T07:02:49Z
implementationStatus: IN_PROGRESS
currentPhase: 0
currentPhaseStatus: IN_PROGRESS
currentSlice: P0-03
currentSliceStatus: READY
expectedBranch: v2/phase-0-contract
lastEvidenceCommit: 49a78b7
worktreeState: CLEAN_AFTER_PROGRESS_COMMIT
baselineCommit: d99ebb4
baselineTag: v2-design-gate-f-routing
baselineTagObject: f8b53d8
baselineTagStatus: PUSHED
openapiSha256: bea05474c9502318407ec6b8a62ba675807ed2d8cb9a268c12b8e47669d521ed
---

# NetherLink v2 实现进度仪表盘

> 状态：`MUTABLE_PROGRESS / PHASE_0_IN_PROGRESS`
>
> 本文件是**实现进度的唯一短摘要**，允许在实现过程中持续更新；它不是 API、领域或数据契约。冻结语义仍以 [`implementation_plan.md`](implementation_plan.md)、[`openapi.yaml`](openapi.yaml) 和各领域规范为准。
>
> 上限：正文保持在约 250 行以内。这里只保留当前交接、Phase 总览、阻塞和最近里程碑；当前 Phase 的任务证据写入对应账本，旧 Phase 通过链接读取，不把历史全部复制回来。

## Agent 恢复协议

项目受信任时，Pi 通过 [`.pi/APPEND_SYSTEM.md`](../../.pi/APPEND_SYSTEM.md) 自动注入入口规则；新的实现 Agent 必须按以下顺序恢复上下文：

1. 读取 [`index.md`](index.md)；
2. 读取本文件，确认 `currentPhase`、`currentSlice`、下一动作和阻塞；
3. 只读取 [`implementation_plan.md`](implementation_plan.md) 中当前 Phase；
4. 读取“当前 Phase 账本”链接；
5. 按账本中的稳定导航键，只加载最多三份规范的相关章节及一个 OpenAPI Operation/Component 切片；
6. 编码前把当前切片从 `READY` 改为 `IN_PROGRESS`；结束前同时更新账本和本仪表盘。

不得为了“恢复记忆”通读全部 `doc/v2/`、恢复已删除的临时设计监督记录，或把聊天摘要当成仓库进度权威。

## 当前交接快照

| 字段 | 当前值 |
| --- | --- |
| 实现状态 | `IN_PROGRESS`：仅推进 Phase 0 clean-slate baseline，不开放业务路由 |
| 当前 Phase | `Phase 0：基线、WSL 发布门禁与可选 CI` |
| 当前 Phase 账本 | [`progress/phase-0.md`](progress/phase-0.md) |
| 当前切片 | `P0-03：建立最小 v2 Cargo target、锁定工具链与 WSL Gate` |
| 切片状态 | `READY` |
| 下一动作 | 按 Phase 0 Task 3 建立不含业务路由的最小 v2 编译骨架，清理旧依赖并固定 toolchain/Gate；本轮不继续实现该切片 |
| 最近已验证 | OpenAPI 81 Paths / 94 Operations / 1422 local `$ref`；无版本 OpenAPI paths；Redocly 有效并保留 8 个已解释 Warning；Markdown 链接和 `git diff --check` 通过 |
| 当前风险 | 保留的 `Cargo.toml` / `Cargo.lock` 暂时指向已删除 target 且含旧依赖；这是 P0-03 的显式前置状态，当前 Cargo build/test 预期不可用，不得误记为回归或通过 |

### 当前切片最小上下文

- [`implementation_plan.md`](implementation_plan.md)：“Phase 0：基线、WSL发布门禁与可选CI”“目标代码结构”；
- [`index.md`](index.md)：“Phase 0：基线、门禁和发布骨架”；
- [`progress/phase-0.md`](progress/phase-0.md)：`P0-03` 行和当前交接；
- `Cargo.toml` / `Cargo.lock`：只检查当前 package metadata、target 声明和待清理依赖；
- [`.pi/APPEND_SYSTEM.md`](../../.pi/APPEND_SYSTEM.md)：入口验收边界。

明确非目标：`P0-03` 不开放业务路由，不实现数据库、认证或信令；本地 `.env` 与 `target/` 不提交或读取其中 Secret/产物。

## Phase 总览

状态只允许：`NOT_STARTED`、`READY`、`IN_PROGRESS`、`BLOCKED`、`COMPLETE`。

| Phase | 状态 | 当前/最后切片 | 账本 | 完成证据摘要 |
| --- | --- | --- | --- | --- |
| 0 基线与门禁 | `IN_PROGRESS` | `P0-03` READY；`P0-01`、`P0-02` COMPLETE | [`progress/phase-0.md`](progress/phase-0.md) | baseline/tag 已推送；v1 工程资产已从 active tree 清除 |
| 1 平台与 Migration Journal | `NOT_STARTED` | — | 激活时创建 `progress/phase-1.md` | — |
| 2 Account/Auth | `NOT_STARTED` | — | 激活时创建 `progress/phase-2.md` | — |
| 3 Provider/Friendship/Sync | `NOT_STARTED` | — | 激活时创建 `progress/phase-3.md` | — |
| 4 Instance/Notification/ACL/Proxy/Invite/Guest/Join/Report | `NOT_STARTED` | — | 激活时创建 `progress/phase-4.md` | — |
| 5 Gate E 集成与运行准备 | `NOT_STARTED` | — | 激活时创建 `progress/phase-5.md` | — |
| 6 Gate F Signaling Session/多节点 WS | `NOT_STARTED` | — | 激活时创建 `progress/phase-6.md` | — |
| 7 Relay 持久模型 | `NOT_STARTED` | — | 激活时创建 `progress/phase-7.md` | — |
| 8 TURN Adapter/真实验收 | `NOT_STARTED` | — | 激活时创建 `progress/phase-8.md` | — |
| 9 客户端/Conformance | `NOT_STARTED` | — | 激活时创建 `progress/phase-9.md` | — |
| 10 Canary/切流/v1 移除 | `NOT_STARTED` | — | 激活时创建 `progress/phase-10.md` | — |

Phase 不得仅因代码已写完就标记 `COMPLETE`；必须满足 `implementation_plan.md` 的完成定义，并在账本记录可复现的命令、输出摘要和 commit/PR。

## 活跃阻塞与 Gate

| ID | 范围 | 状态 | 解除条件 | 影响 |
| --- | --- | --- | --- | --- |
| `B-09-TURN` | 生产 Relay | `BLOCKED` | 真实 TURN Adapter、Runtime Permit、Peer Pin、byte-credit、WebRTC、配额、故障和隐私验收，加人工审批 | 仅阻止生产 `NLI_RELAY_ENABLED=true`；不阻止前置实现 |

生产 `NLI_RELAY_ENABLED` 必须保持 `false`，直到 `B-09-TURN` 解除。

## 最近里程碑

最多保留 12 行；更旧记录留在对应 Phase 账本和 Git 历史。

| 时间（UTC） | 里程碑 | 状态 | 证据 |
| --- | --- | --- | --- |
| 2026-09-07 | P0-01 clean-slate baseline | `COMPLETE` | doc baseline `d99ebb4` 已推送到 `master`；tag `v2-design-gate-f-routing` 已推送；v1 资产清理 commit `49a78b7` 已推送到 Phase 0 分支 |
| 2026-09-07 | 建立 Pi 项目级每轮系统提示词入口 | `COMPLETE` | [`.pi/APPEND_SYSTEM.md`](../../.pi/APPEND_SYSTEM.md) 已包含于 `d99ebb4` 并推送 |
| 2026-09-07 | 有限上下文 Reader Test | `APPROVE` | Fresh reviewer 能回答状态、切片、下一动作、阻塞、最小读取、更新协议和完成证据；3 个 warning 已修正 |
| 2026-09-07 | 创建有限上下文实现进度体系 | `COMPLETE` | 本文件、[`progress/phase-0.md`](progress/phase-0.md)、导航与计划已包含于 `d99ebb4` |
| 2026-09-07 | 核实应用版本 `0.2.0` 已由独立提交完成 | `COMPLETE` | commit `b30b536` 是当前 HEAD 祖先；`Cargo.toml` / `Cargo.lock` 已更新 |
| 2026-09-07 | D-329 代理/应用路由边界文档化并机械验证 | `COMPLETE` | baseline `d99ebb4` / OpenAPI 2.1.1；81/94/1422、Redocly、链接、diff 检查 |
| 2026-08-31 | Gate F 最终设计冻结 | `COMPLETE` | commit `3b4f329`、tag `v2-design-gate-f`；Gate E 77/89，Gate F 81/94 |

## 每次实现切片的更新规则

### 开始时

1. 实时读取工作区、分支和 HEAD；核对 `expectedBranch`，并确认 `lastEvidenceCommit` 是当前 HEAD 的祖先；不一致先更新或报告快照；
2. 在当前 Phase 账本中只选择一个可验证切片；
3. 写明稳定导航键、必读章节、非目标和验收命令；
4. 把切片和 Phase 状态改为 `IN_PROGRESS`，更新 `updatedAt`、`expectedBranch`、`lastEvidenceCommit` 和 `worktreeState`；`lastEvidenceCommit` 表示最近已验证实现/清理证据，不声称等于包含本次进度文字的 commit；
5. 不把尚未运行的测试写成通过。

### 结束时

1. 在 Phase 账本记录变更文件、关键决策、命令、结果、残余风险和 commit/PR；
2. 只有证据齐全才标记切片 `COMPLETE`；否则保持 `IN_PROGRESS` 或 `BLOCKED`；
3. 更新本文件的当前切片、下一动作、Phase 总览、阻塞和最近里程碑；
4. 下一 Agent 所需上下文必须能由“当前交接快照”在五次以内的局部读取中取得；
5. 不记录 Token、Secret、Cookie、SDP、Candidate、地址、TURN Credential、Peer Pin 或未经脱敏的生产信息。

### Phase 切换时

1. 核对当前 Phase 完成定义与全部 Gate 证据；
2. 将 Phase 标记 `COMPLETE`，保留其账本链接；
3. 从当前账本末尾的模板创建下一个 `progress/phase-N.md`；
4. 本文件只切换 `currentPhase/currentSlice` 和短交接，不复制旧 Phase 正文；
5. 若发现需要修改冻结设计，状态改为 `BLOCKED` 并重开对应 Gate，不在进度文件中自行改写契约。

## 进度记录边界

- **`.pi/APPEND_SYSTEM.md`**：Pi 每轮自动注入的项目导航与工作口径；不保存动态状态；
- **本文件**：当前事实、下一动作、Phase 状态和跨 Phase 阻塞；
- **Phase 账本**：切片历史、命令、证据、commit/PR 和当前 Phase 交接；
- **实现计划**：Phase 顺序、任务、验收和完成定义；
- **设计/OpenAPI**：行为契约；
- **Git/CI/发布产物**：最终可核验事实。

进度记录不得复制大段设计正文，也不得以 `COMPLETE`、勾选框或自然语言替代测试输出和版本控制证据。
