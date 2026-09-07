# NetherLink v2 项目系统提示词追加

本文件是 Pi 的项目级 `APPEND_SYSTEM.md`：在保留 Pi 默认系统提示词的前提下追加项目入口规则。它只负责启动导航和工作口径，不复制动态进度，也不具有 API、领域或数据契约权威。

## 每轮强制入口

在每个用户回合开始、规划或修改仓库之前：

1. 新会话、任务实质变更或不确定最小上下文时，先读取 `doc/v2/index.md`。
2. 每个回合都读取 `doc/v2/implementation_progress.md`，刷新当前 Phase、当前切片、状态、下一动作、阻塞、分支和 HEAD；不得只依赖对话历史或压缩摘要。
3. 若本轮涉及实现、测试、提交、部署或实现文档，继续读取进度仪表盘指向的单个 `doc/v2/progress/phase-N.md`，以及 `doc/v2/implementation_plan.md` 的当前 Phase；不要读取其他 Phase。
4. 再按 `doc/v2/index.md` 的任务导航，只读取最多三份正式规范的相关章节和一个目标 OpenAPI Operation/Component 切片。
5. 若入口文件彼此矛盾、记录的分支/HEAD 与工作区不符，先停止实现并修正或报告进度状态，不得猜测。

即使上下文压缩、切换模型或恢复旧会话，也必须重新从上述入口恢复当前事实。禁止为了恢复记忆一次性加载全部 `doc/v2/`。

## 统一权威口径

- `doc/v2/index.md`：最小上下文导航；不定义行为契约。
- `doc/v2/implementation_progress.md`：当前进度、下一动作和跨 Phase 阻塞的唯一短摘要；可变，不定义行为契约。
- `doc/v2/progress/phase-N.md`：当前 Phase 的切片、执行证据、commit/PR、局部决策和交接账本。
- `doc/v2/implementation_plan.md`：Phase 0–10 的顺序、任务、Gate、验收和完成定义。
- `doc/v2/openapi.yaml`、`common.md`、`rest_api.md`、`data_model.md` 及领域文档：API、领域、数据与安全契约。
- Git、CI 和发布产物：最终可核验事实。

对话摘要、Agent TODO、分支名称和自然语言“已完成”都不能替代仓库进度文件与可复现证据。进度文件不得反向修改冻结契约；需要改变冻结语义时必须阻塞当前切片并重开对应 Gate。

## 实现切片更新协议

- 开始编码前，核对当前切片、工作区和前置条件，并在仪表盘与 Phase 账本中把实际开始的切片标为 `IN_PROGRESS`。
- 每轮只推进一个可验证切片；不得顺手跨 Phase 或开放未到阶段的路由。
- 结束前同步记录变更文件、实际执行的命令、结果、残余风险、commit/PR 和下一动作。
- 缺少完成定义要求的测试或证据时，只能保持 `IN_PROGRESS` 或 `BLOCKED`，不得标记 `COMPLETE`。
- 仪表盘保持短小；详细历史留在单个 Phase 账本和 Git，不把旧 Phase 正文复制回当前上下文。
- 任何进度文件都不得记录 Token、Secret、Cookie、SDP、ICE Candidate、地址、TURN Credential、Peer Pin 或未经脱敏的生产信息。

## 不得漂移的实现边界

- 客户端公共 API 使用 `/v2/{path...}`；受控反向代理选择版本并剥离首段；应用 Router、Operation Policy 和 mounted route manifest 只使用无版本前缀的 `/{path...}`。
- PostgreSQL/HTTP 是业务权威；LeaseStore/EventBus 只保存可丢失路由、租约、限流和短期投递。
- 通知 WS 只负责保活/通知；Signaling WS 是唯一受限实时业务写入例外。
- MC Profile 始终为 `CLIENT_CLAIMED`，不构成 NLI 或 Minecraft 身份证明。
- 生产 `NLI_RELAY_ENABLED` 在 `B-09-TURN` 真实验收和人工审批前必须保持 `false`。
- 当前状态、切片和 Gate 不在本文件硬编码；每轮以 `doc/v2/implementation_progress.md` 为准。
