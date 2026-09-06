# NetherLink v2 API

> Gate F 已通过。Gate E 的 77 Paths / 89 Operations 保持冻结；Phase 9 仅追加 4 Paths / 5 Operations，形成 81 Paths / 94 Operations 的最终设计基线。P2P 信令、NAT 与 TURN 的权威设计见 `signaling.md`，不得反向改变已冻结授权语义。

NetherLink v2 API（NLI v2 API）将完全抛弃现有实现，新建独立归属 NetherLink 的账号、好友与联机系统。

核心约束：

- NLI 账号认证与 Minecraft 游戏身份认证相互独立；
- 外部 Provider 不可用时，只影响对应来源的扩展能力，不能影响 NLI 自有账号、好友与联机功能；
- 外部 Provider 的好友关系只能作为好友数据来源以及发起 NLI 好友申请的依据，不能直接授予 NLI 联机权限；
- 临时凭据、实例、邀请码和加入请求必须具有过期与撤销机制；
- 客户端提交的 Provider、好友来源和 NLI 权限信息都必须由后端重新鉴别；
- 联机时使用的 MC Profile 由源 Game Instance 自行声明和传递，NLI 只验证 NLI 账号、Session 与实例身份，不为 MC Profile 的真实性背书；本机恶意客户端可以滥用自己的 NLI Auth，且真实 Profile 也不能证明行为可信，因此 Profile 只用于展示和显式弱 ACL 匹配；
- 联机后的恶意行为通过举报通道处理，客户端 MOD 可以进行本地二次校验，但其结果不升级为 NLI 后端身份保证。

## 统一术语

### Provider

Provider 统一表示可向 NLI 提供一种或多种外部能力的第三方账号或认证服务，例如 Microsoft、Mojang 相关服务、LittleSkin 等。

Provider 不代表某一种固定认证协议。不同 Provider 可以分别声明以下能力：

- 验证 Provider 账号控制权；
- 登录 NLI 账号；
- 读取好友；
- 修改好友；
- 读取或加入 Provider 提供的联机实例；
- 修改 Provider 中与好友功能有关的配置。

后端必须根据 Provider 实际声明的能力工作。不支持的操作应明确返回 `UNSUPPORTED`，不能假定能够登录的 Provider 一定能够读取或修改好友。

Provider 暂时不可访问时，只将相关操作标记为暂时失败，不改变绑定状态。只有在 Provider 明确拒绝已有凭据、授权被撤销或绑定身份无法继续验证时，才将对应绑定标记为需要重新验证。

实现层面应建立统一的 Provider 抽象和凭据管理边界。登录、绑定验证、好友读取与修改、配置修改等任何依赖 Provider 的操作都必须通过该边界执行；业务模块不能直接读取、刷新或保存 Provider 凭据。具体接口与存储方式在 Provider 子设计中确定。

### MC 账号 / 游戏账号

MC 账号表示玩家在游戏运行时中使用的角色身份，可以来自 Provider，也可以是离线身份。

Game Instance 发布状态或发送联机请求时，可以携带当前使用的 MC Profile 信息，包括来源、UUID 和用户名。该信息由源实例提供，统一标记为 `CLIENT_CLAIMED`；v2 不接收客户端声明的 MC Profile 头像。NLI 不负责验证它是否为当前玩家真实持有的游戏身份，也不能以它替代 NLI 账号鉴权。

显示名称只能作为可变的展示信息，不能用于唯一识别、绑定或合并 Provider 账号。

### NetherLink Account

NetherLink Account（NLI Account）是 NetherLink 自有账号，使用不可变的 Account ID 标识。邮箱、密码和第三方认证身份都是账号的认证或通知方式，不是账号本身的身份。

### Game Instance

Game Instance 表示一个可以发布状态并接受联机请求的游戏运行时。实例自身就是联机目标，不单独创建其他联机目标资源。

一个已登录 NLI 的实例有且只有一个归属 NLI 账号；归属者同时也是该实例的控制者。归属关系由后端根据 Instance Session 确定，发布实例时客户端不提交 Account ID 等归属信息。只有该实例凭据能够创建、修改和关闭对应实例。

未登录 NLI 的游戏运行时不能发布好友状态，但可以通过邀请码建立短期匿名身份并请求加入其他实例。

## NLI Account 与认证

### 账号基础信息

建议的基础信息包括：

- 不可变 Account ID；
- 账号昵称；
- 已验证邮箱；
- 账号状态；
- 创建与更新时间。

同一个已验证邮箱默认只能关联一个有效 NLI 账号。邮箱用于登录、通知和账号恢复，但不能作为数据库主键，也不能用于跨 Provider 自动合并账号。

暂不提供头像、介绍等 NLI 账号资料。客户端可以在适用场景中显示 Provider 提供的公开游戏头像。

### 注册

允许通过以下方式注册：

- 邮箱与密码；
- Microsoft 等第三方认证；
- 能够提供可靠身份信息的其他 Provider。

使用第三方认证时，后端必须使用 `Provider + Issuer + Subject` 识别认证身份，不能只根据第三方返回的邮箱合并账号。第三方邮箱不可靠或未验证时，需要由 NLI 单独完成邮箱验证。

如果第三方认证同时对应一个 MC 账号，可以向用户询问是否绑定该游戏账号。如果用户不绑定，NLI 只保存用于认证的稳定身份映射，不将其用于好友或联机业务，也不应无必要地保存第三方 Access Token。

### 登录

NLI 支持：

- 邮箱与密码登录；
- 已启用登录用途的第三方 Provider 身份登录；
- 由 NLI API 签发的 Device Code Flow，供客户端 Mod 登录。

网页或账号管理客户端使用 Account Session。Mod 使用 Device Code Flow 获得 Instance Session。两者使用相同的 NLI Account Access Token 与 Refresh Token 格式、相同的普通 API 权限和 `nli_account` Audience；名称只表示会话生命周期和是否绑定 Game Instance，不代表两套业务权限。

每个 NLI Account Token Family 都具有服务端签发的临时 Session ID，用于区分同一账号的并发登录。Session ID 不能代替持久 Account ID，也不能由客户端指定。

Device Code Flow：

1. Mod 向 NLI API 申请 Device Code；
2. 用户在浏览器中打开验证页面并登录 NLI；
3. 用户确认授权本次 Mod 登录；
4. Mod 轮询授权结果；
5. NLI 签发 `nli_account` Audience 的 Access Token 与 Refresh Token。

NLI Account Token 可以调用全部普通 NLI Account API，后端根据账号身份、会话状态、资源归属和业务关系进行授权，不为普通业务端点设置细粒度 Scope。

一组 NLI Account Access Token 与 Refresh Token 同一时刻最多只能创建和管理一个 Game Instance。首次成功创建实例时，后端需要原子地将 Token Family 绑定到该 Instance ID，并将该会话作为 Instance Session 使用；创建请求必须支持幂等键。实例关闭并合法解绑后，会话恢复为 Account Session，同一 Token Family 可以用于后续实例。

Access Token 应短期有效。Refresh Token 需要轮换并检测旧 Token 重用；主动撤销会话或发现超出安全重放窗口的 Refresh Token 重用时，应撤销对应 Token Family。

### 账号恢复

用户可以通过已验证邮箱请求密码重置邮件。重置链接或验证码必须短期有效、只能使用一次，并且请求接口不能暴露邮箱是否已注册。

通过邮件重置密码后，应撤销该账号已有的 Account Session 与 Instance Session，由用户重新登录。仅通过第三方 Provider 注册的用户也可以通过已验证邮箱设置或重置 NLI 密码，避免唯一 Provider 失效后永久失去账号。

### 绑定 Provider 账号

NLI 账号可以绑定 Microsoft 或其他 Provider 账号。绑定必须通过 Provider 实际支持的方式证明账号控制权，不能凭用户名绑定。

同一个 `Provider + Subject` 同时只能绑定一个有效 NLI 账号。绑定、解绑、换绑和重新验证需要记录安全审计信息。

每个绑定至少具有以下状态：

- `ACTIVE`：绑定验证有效，可以使用已启用的 Provider 能力；
- `REAUTH_REQUIRED`：已有凭据被拒绝、授权被撤销或绑定身份无法继续验证，需要用户重新完成绑定验证；
- `DISABLED`：用户主动停用该绑定。

绑定还应记录最后成功验证时间和最近错误，但 Provider 的临时网络故障或服务中断不能直接把绑定改为 `REAUTH_REQUIRED`。

进入 `REAUTH_REQUIRED` 后：

- 暂停依赖该绑定的普通 Provider 登录、好友读取、同步和其他扩展操作，只允许进入专用的重新验证流程；
- 保留原 Binding ID、Provider Subject、配置和既有 NLI 好友关系；
- 提示用户重新完成 Provider 认证；
- 重新验证成功且 Subject 与原绑定一致后恢复为 `ACTIVE`；
- 如果重新认证得到不同 Subject，不得静默替换原绑定，应按换绑流程处理。

每个绑定分别提供用途开关：

- 是否允许用于登录 NLI；
- 是否允许用于读取或同步 Provider 好友；
- 是否允许使用 Provider 的其他扩展能力。

绑定 Provider 账号不表示 NLI 会验证 Game Instance 上报的 MC Profile。绑定仅用于 NLI 登录、Provider 好友和对应 Provider 扩展业务。

如果 Provider 要求用户修改某项配置才能启用好友功能，NLI 只能通过明确、能力受限的 Provider 接口进行操作，并向用户展示具体改动。不得提供任意的 Provider 管理代理接口。

### 账号配置

至少包括：

- 是否允许接收好友请求；
- 各 Provider 是否启用自动好友同步；
- 各绑定的用途开关；
- 有效设备、登录会话和实例会话管理；
- 代理发布授权码管理。

## 好友系统

### NLI 好友关系

好友关系以 NLI Account 为单位。好友申请具有方向，已经接受的好友关系是双向关系。

删除好友时需要保留有方向的 ban 位，用于阻止 Provider 同步或对方申请立即重新创建关系。建议的最小关系信息包括：

- 双方 Account ID；
- 好友状态；
- 待处理申请方向；
- 双方各自设置的 ban 位；
- 申请来源；
- 创建与更新时间。

行为规则：

- A 删除 B 时解除好友关系，并设置 A 方向的 ban；
- Provider 自动同步不能清除 ban；
- A 手动重新添加或同意 B 时，可以提示 A 是否清除自己设置的 ban；
- A 不能清除 B 设置的 ban；
- 批量同步遇到 ban 时直接跳过；
- 单个手动同步遇到自己的 ban 时，可以提示用户解除。

### 查询好友

好友查询以 NLI Account 作为主要集合单位。每个 NLI Account 下可以显示：

- 该账号允许当前用户查看的 Game Instance；
- 每个实例当前使用的 MC 账号公开信息；
- 代理实例及其真实来源 NLI 账号标记；
- 当前好友关系的来源信息。

每个实例拥有独立 ACL；只有调用方具有绑定且 ONLINE 的 Source Instance/Guest Context，并且 ACL 对当前请求者与来源返回 ALLOW 时，实例才进入好友聚合或邀请码结果。普通未绑定 Account Session 仍可查询好友关系，但不返回可加入实例。加入游戏前只提供标记为客户端声明的 MC 用户名、实例描述等允许公开的信息，不提供头像、敏感账号绑定或认证信息。

查询还需要融合各个已启用 Provider 的好友数据。如果某个 Provider 好友没有可用于当前查询的 NLI 好友关系，则显示为独立的外部好友数据，不能凭此外部关系直接获得 NLI 好友联机权限。

未登录 NLI 的客户端可以直接向 Provider 查询好友，但不使用 NLI 好友服务。此时只能通过邀请码进行匿名联机。

### Provider 范围内的身份解析

假设当前游戏账号 A 在 Provider P 中查询到好友 B，NLI 只允许在 Provider P 的范围内对 B 的稳定 Subject 或 UUID 进行内部解析。

普通查询不得返回 B 对应的 NLI Account 信息，也不得提供公开的 `Provider Subject -> NLI Account` 查询接口。

同步时客户端只提交：

- Provider ID；
- A 使用的绑定 ID；
- 后端为该 Binding 签发的 opaque External Friend ID。

Provider 的稳定 Subject 或 UUID 由后端投影加密保存并通过 keyed HMAC 做同域解析，不直接返回给普通客户端。后端负责：

1. 验证绑定属于 A 的 NLI 账号；
2. 验证 Provider 中当前确实存在 A 与 B 的好友关系；
3. 仅在同一 Provider 范围内解析 B 的绑定；
4. 检查对方是否允许接收好友请求；
5. 检查双方关系中的 ban；
6. 创建带有同步来源标记的 NLI 好友申请。

接口响应不能通过不同错误暴露 B 是否注册、是否绑定或是否禁止请求。批量同步只返回统计和任务状态，不返回每个外部好友的 NLI 绑定状态。

### 同步好友

Provider 好友同步的本质是发起带有可信来源标记的 NLI 好友申请，不直接写入好友关系。

支持：

- 同步指定 Provider 好友；
- 同步指定 Provider 的所有好友；
- 同步所有已启用 Provider 的好友；
- 用户主动启用的自动同步。

来源标记必须由后端在验证 Provider 好友关系后生成，不能信任客户端自由提交的来源。

如果接收方为对应 Provider 启用了自动同步，并且后端验证双方确实存在该 Provider 好友关系，则可以自动接受申请；否则保留为普通待处理好友申请。

外部 Provider 好友关系后来解除时，不自动删除已经建立的 NLI 好友关系。所有同步操作必须幂等，批量同步应作为可查询进度的异步任务执行。

### 添加好友

好友申请必须使用可辨识的目标 NLI 用户信息。通过昵称搜索时，昵称只用于定位候选账号，最终操作使用 Account ID。

申请来源至少包括：

- Provider 好友同步；
- NLI 用户搜索。

搜索来源的申请一定需要对方同意。Provider 同步来源只有在满足双方配置和后端关系验证时才能自动接受。

### 删除好友

删除好友会解除双向好友关系，并为发起删除的一方记录有方向的 ban。后续手动恢复关系时，需要由设置该 ban 的用户明确解除。

## 代理发布

NLI 用户可以为指定好友创建代理发布授权码，并自行将授权码分配给该好友。好友发布自己归属并控制的 Game Instance 时直接提供授权码，后端验证后将该实例同时显示在授权用户的好友数据中；不需要先兑换授权码，也不额外生成其他授权层。

代理发布不会转移实例归属或控制权：

- 实例归属者始终是实例控制者；
- 只有实例归属者的 Instance Session 可以管理实例；
- 发布请求不提交实例归属 Account ID，后端从 Instance Session 获取归属账号；
- 后端从授权码获取允许代理展示到的目标账号，客户端不提交目标 Account ID；
- 授权码绑定获授权的 NLI Account，其他账号即使获得授权码也不能使用；
- 被代理展示的实例由后端添加真实来源 NLI 账号标记；
- 实例计入归属者的活跃实例额度；
- 授权码受到签发者的有效授权码数量限制；
- 双方不再是 NLI 好友、授权码被撤销或设置的有限到期时间到达时，代理发布立即失效；重新成为好友不会恢复旧授权；
- 一个授权码同时最多绑定获授权者的一个 ACTIVE Instance，解绑或实例关闭后可以复用于后续实例；
- 一个实例同时最多代理展示到一个目标账号；
- 目标账号的好友通过该有效代理展示路径获得 `PROXY_FRIEND` Identity Trait 和 `FRIEND_LIST` Source，但仍需通过实例状态、ACL、限流与审批检查。

代理发布授权码只需要：

- 使用高熵随机值，后端只保存哈希；
- 绑定签发者与获授权 NLI Account；
- 可由签发者选择有限有效期，也允许保持无到期时间；无到期 Grant 持续占用有效授权额度，直到撤销、好友解除或账号失效；
- 支持签发者主动撤销和轮换 Secret；
- 受到数量限制。

## Game Instance 与联机系统

### Game Instance 发布

发布实例时携带：

- 当前 MC Profile 的来源、UUID、用户名等客户端声明信息；v2 不接收 MC Profile 头像；
- 游戏版本、Loader 和必要的兼容性信息；
- 由 Mod 生成的实例描述；
- 实例有序 ACL 与审批模式；
- 可选的代理发布授权码。

发布请求不携带归属 NLI Account 信息。后端从当前 Instance Session 确定实例归属者，并验证该组 Access Token 与 Refresh Token 只能创建和管理这一个实例。NLI 只验证 NLI 账号与实例会话，不验证客户端声明的 MC Profile 是否真实属于该用户。

后端为实例签发不可猜测的公开 Instance ID。公开 ID 只用于寻址，不作为管理凭据。

实例通过通知 WebSocket 的客户端主动 Ping 维持在线租约，不提供单独的 HTTP 心跳。Instance Session 失效或实例主动关闭时立即关闭；其他断线在 WS Session 租约失效后立即标记为 OFFLINE 并停止查询和加入，同一 Instance Session 可在 10 分钟宽限期内重连，超时后实例自动关闭并解绑 Token Family。

### Game Instance 配置与 ACL

实例不再分别保存隐藏、可加入、允许身份类型和黑名单。发现与请求加入统一由有序 ACL 控制：ACL DENY 或无匹配时，实例对该请求者同时不可见、不可加入。

每条 Access Rule 包含：

- 可选 Identity Types：`NLI_ACCOUNT`、`DIRECT_FRIEND`、`PROXY_FRIEND`、`ANONYMOUS`；
- 可选 Sources：`FRIEND_LIST`、`INVITE_CODE`；
- 可选 typed Subjects：`NLI_ACCOUNT_ID` 或 `MC_USERNAME`，每项可有多个值；
- 必填 Action：`ALLOW` 或 `DENY`；
- 稳定 priority。

除 Action 外，Matcher 项不填或为空表示任意。非空字段之间使用 AND，同字段多个值使用 OR。规则按 priority 顺序首条匹配生效，无匹配默认 DENY。

`NLI_ACCOUNT_ID` 是可靠 NLI 身份 Matcher。`MC_USERNAME` 只匹配 CLIENT_CLAIMED 声明，可用于 ALLOW/DENY，但必须标记为 `WEAK_CLIENT_CLAIMED`，不能声称为可靠白名单或封禁。

所有可见 Instance 都由 NLI 后端上的 Host 发布；当前没有第三方联机渠道或 Provider Friend 联机 Source。没有 NLI Account 的请求者统一视为 `ANONYMOUS`，只能通过 Guest Session 使用邀请码来源。

ACL ALLOW 只表示可以看到实例摘要并创建 Join Request。审批方式由独立 `approval_mode = MANUAL / AUTO` 控制。Instance 生命周期、在线状态、Session、Invite/Proxy 有效性和服务端滥用限制仍是 ACL 不能绕过的硬门槛。

状态与展示相关字段包括当前是否在线、当前游戏状态和实例描述。实例描述由 Mod 控制，可以默认使用经过清理的游戏窗口名；服务端限制长度、移除控制字符，并禁止用于权限判断。

### 实例邀请码

邀请码只提供给实例归属者，由归属者自行分发。有效邀请码建立 `INVITE_CODE` Source，随后仍需由 ACL ALLOW；ACL DENY 时实例不可见、不可加入。邀请码不会向任何用户列表直接公开。

邀请码需要：

- 绑定一个 Instance ID；
- 后端只保存哈希；
- 具有过期时间；
- 支持主动撤销；
- 支持使用次数限制；
- 验证失败限流。

### 匿名加入

匿名用户不能查询或使用 NLI 好友服务。持有有效邀请码后，后端可以为其当前游戏运行时建立短期 Guest Session。Guest Session 具有独立 Guest ID 和 `nli_guest` Audience，与该匿名游戏运行时的生命周期绑定，用完即失效。Guest 只获得短期 Access Token，不签发长期 Refresh Token。

Guest Session 只用于：

- 标识本次加入请求；
- 接收本次请求结果；
- 执行临时封禁和限流；
- 完成本次联机流程。

Guest ID 只是 NLI 服务端签发的临时用户身份，不代表持久 NLI Account。完全匿名用户不存在可靠的永久身份；IP、离线用户名或客户端生成 ID 不能被视为可永久封禁的身份。

未来管理后台使用独立的 `nli_admin` Audience。普通 `nli_account` Token 和 `nli_guest` Token 不能访问管理接口；管理角色和权限不属于当前 v2 用户 API 大纲。

### 联机加入

加入请求直接指向 Instance ID。请求方需要使用自己的 Instance Session，或者通过有效邀请码建立的 Guest Session 发起请求。

目标实例需要获得以下请求者信息：

- 请求者类型：`NLI_ACCOUNT` 或 `ANONYMOUS`；
- 请求者信息：NLI 用户的 Account ID 与公开账号信息，或者匿名用户的短期 Guest ID 与公开信息；
- Source：`FRIEND_LIST` 或 `INVITE_CODE`；
- 服务端推导的 Identity Traits；
- 申请时由请求方 Instance/Guest 提供的 CLIENT_CLAIMED MC Profile 快照，包括来源、UUID 和用户名。

Requester Type、NLI Account 信息、Source 和 Identity Traits 由后端根据 Session、NLI 好友/Proxy 关系或 Invite 验证结果确定，不能信任客户端自由标记。没有 NLI Account 即为 `ANONYMOUS`。

MC Profile 只作为客户端声明展示。NLI 不认证 Profile 归属，也不因为审批期间 Profile 改变而取消请求或改变可靠授权。MC username 如参与 ACL，只属于明确标记的弱 Matcher。恶意行为通过后续举报通道处理；客户端 MOD 可以在联机建立后执行本地二次校验。

后端在创建请求前必须验证实例在线、请求者 Session、Source 证明、Invite/Proxy 不变量、最新 ACL 结果和服务端滥用限制。只有 ACL 首条匹配为 ALLOW 才能返回实例摘要或创建 Request。

`approval_mode=AUTO` 时，后端在同一事务中创建 Join Request、完成全部校验并直接转为 `ACCEPTED`；否则通过通知 WebSocket 提示实例归属者，由其通过 HTTP 接受或拒绝。Request 默认 60 秒过期；ACCEPTED 后由 Request ID 与双方认证 Session 共同形成 60 秒 Acceptance Lease，不签发独立 Bearer Ticket。

接受请求和使用 Lease 时必须重新验证实例、双方 Session、Source 证明和最新 ACL。Request 保存的 CLIENT_CLAIMED Profile 快照只用于展示、弱匹配和举报，不作真实性或一致性鉴权。

MOD 停止接收加入时，可把 ACL 原子替换为 catch-all DENY，并同时取消不再 ALLOW 的 PENDING。API 仍提供幂等批量拒绝。实例关闭或离线时自动取消全部 PENDING。

不维护额外的实例世界版本。ACL Revision 只用于确定访问规则快照与更新，不声称识别客户端是否切换世界。

## WebSocket 通知与实例保活

Phase 5 WebSocket 只用于通知和实例连接保活，不承担关键业务状态写入和联机信令。Phase 9 可定义唯一的独立短期信令 WS 例外，但不得复用本通道或改变 HTTP/PostgreSQL 权威；好友申请、加入请求、同意、拒绝等领域状态修改仍只使用 HTTP API。WebSocket Ping/Pong 只属于连接维护。

### 连接建立

只有目标 Token Family 为 `ACTIVE_BOUND` 的 Instance Session 可以建立 WebSocket。Access Token 只通过 Upgrade 请求的 `Authorization: Bearer` Header 传输；Query、Cookie 和 Subprotocol 不接受 Token。

握手成功后生成客户端可见的 UUIDv4 WS Session ID。它用于相关性和 Fencing，不是凭据。Access Token 自然到期或 Refresh 轮换不会单独关闭已建立连接，但每次续租都重新检查 Account、Token Family 和 Instance 状态。

同一个 Instance 只保留一个 current WS Session。新连接原子替换 current ID；任何 Ping、关闭或通知投递都必须比较自身 Session ID。旧连接尽力收到 `connection.replaced` 并以 4001 关闭，即使关闭消息丢失也不能续租。

保活权威状态只包括：

- 当前 WS Session ID；
- 连接过期时间。

Node 路由只是传输索引。不保存 `last_seen`、心跳次数、额外 Fencing Generation 或业务消息历史。

### 客户端主动保活

保活由客户端主动发起：

1. 握手 CAS 成功立即将 Instance 设为 ONLINE，并设置 `now + 90 秒` 租约；
2. 客户端建议每 30 秒发送标准 WebSocket Ping；
3. 服务端返回标准 Pong，不主动发送 Ping；
4. 客户端不能发送业务 Text/Binary Frame，关键写入仍使用 HTTP；
5. 只有 current WS Session 且当前时间严格早于 Expires At 的 Ping 可以续租；
6. 迟到 Ping 不能复活 Session，客户端必须重新握手；
7. Pong 可以正常回复，但共享租约写入最多每 10 秒合并一次；高频滥用可以 1008 关闭。

当前连接关闭或租约 compare-delete 成功后，Instance 立即 OFFLINE，停止返回并取消 PENDING Join Request。被替换旧连接的 Close 不能影响新连接。同一 Instance Session 可以在 10 分钟宽限期内建立新 WS Session；超时后再次确认无 current Session，才关闭 Instance 并解绑 Token Family。

业务通知使用小型 Envelope：

- UUIDv4 Event ID；
- 小写点分事件类型；
- `ACCOUNT / INSTANCE / RESOURCE` Scope；
- 可选资源类型与 ID；
- 创建时间。

通知只提示变化，不携带完整 DTO、Profile、Token 或 Secret。投递为 best-effort，允许丢失、重复和乱序，不提供 ACK、Sequence、Last-Event-ID 或事件重放。Account 事件 Fanout 到账号所有 ONLINE Instance，并复用同一 Event ID。

HTTP 查询结果始终是权威状态。每次收到 `connection.ready` 后客户端必须完整刷新账号和当前 Instance 相关资源；之后收到事件时按 Scope 查询。长连接不强制周期轮询，静默丢失的提示可在下一事件、重连或主动刷新时恢复。

单节点和多节点使用统一 LeaseStore + EventBus 抽象。多节点 LeaseStore 必须支持 CAS、TTL 和权威时间；共享存储不可用时拒绝新连接和续租，不退化为节点本地 current Session。优雅重启使用 1012 并 compare-delete 自己的 current Session；异常崩溃最多在 90 秒租约后 OFFLINE。

## P2P 信令、NAT 与 TURN

Gate F 冻结的 Phase 9 设计只服务仍满足全部 use-time 授权不变量的 `ACCEPTED` Join Request。每个 Join Request 生命周期内最多创建一个 Signaling Session；Request ID、Signaling Session ID、Grant ID 和 Connection ID 均不是 Bearer Credential。Request 的 REQUESTER 固定为 Offerer，所有实时写入都经独立 Signaling WS 的当前 Route/Fencing 和授权复核。

PostgreSQL 保存会话生命周期、协议 Phase、epoch、最小 Candidate Receipt、Relay Grant/Slot/Permit 与配额权威；LeaseStore/EventBus 只保存可丢失的路由、租约、限流、背压和短期投递状态。SDP、ICE Candidate、网络地址、TURN Secret 与 Peer Pin 原值不得进入普通业务表、Outbox、Audit Detail、日志或持久队列。

Phase 9 的 Signaling WS 是 Phase 5 “WebSocket 不承担业务写入”规则的唯一受限例外。它只支持可设置 Upgrade `Authorization` Header 且主动发送标准 Ping 的原生、桌面或 Mod WebSocket 栈；浏览器 JavaScript WebSocket 不受支持，也不得使用 Query、Fragment、Subprotocol 或首帧 Token 旁路。

基础匿名模式只承诺信任 NLI 信令服务的 `TRANSPORT_ENCRYPTED`；不声称抵抗恶意信令 MITM。更强的 `PEER_VERIFIED_E2E` 留作未来独立协议，必须有可信客户端与外带高熵 Secret，禁止静默降级。

TURN 允许在 60 秒创建窗口内激活新 Allocation，窗口结束后只允许有界维护已激活 Allocation。生产 Relay 默认关闭；自定义 TURN Adapter 必须通过真实 WebRTC/TURN、Runtime Permit、Peer Pin、byte-credit、撤销、配额、节点崩溃和故障测试后才可启用。精确协议、恢复算法、限额与清理顺序以 `signaling.md`、`rest_api.md`、`data_model.md` 和 `openapi.yaml` 为准。

## 初始限制

以下值作为可配置的初始默认值，后续可根据运行指标调整：

| 项目 | 默认值 |
| --- | ---: |
| 每个账户的活跃实例 | 5 |
| 每个 Instance 的 ACL Rule 上限 | 100 |
| 每个 ACL Matcher 字段值上限 | 100 |
| Instance OFFLINE 重连宽限期 | 10 分钟 |
| 每个账户的有效代理发布授权码 | 10 |
| 代理发布授权码有效期 | 可选；允许无到期时间 |
| 每个实例有效邀请码 | 3 |
| 实例邀请码默认有效期 | 24 小时 |
| 实例邀请码最大有效期 | 7 天 |
| 实例邀请码默认成功使用次数 | 1 |
| 实例邀请码最大成功使用次数 | 100 |
| Guest Session 绝对有效期 | 5 分钟 |
| Guest Join 终态结果保留 | 最多 60 秒 |
| Device Code 有效期 | 10 分钟 |
| Device Code 最小轮询间隔 | 5 秒 |
| Access Token 有效期 | 15 分钟 |
| Refresh Token 空闲有效期 | 30 天 |
| Refresh Token 绝对有效期 | 90 天 |
| 客户端 WebSocket Ping 间隔 | 30 秒 |
| WS Session 连接有效期 | 90 秒 |
| 加入请求有效期 | 60 秒 |
| Acceptance Lease 有效期 | 60 秒 |
| 每个 Source Instance 的 outgoing Join Request 上限 | 3 |
| 每个 Target Instance 的 incoming Join Request 上限 | 100 |
| 每个 Provider 的批量同步最小间隔 | 15 分钟 |
| 自动 Provider 好友同步默认最短周期 | 6 小时 |
| 单次 Provider 同步好友数量上限 | 500 |
| 好友申请有效期 | 30 天 |
| 普通拒绝后的同 Pair 静默抑制 | 24 小时 |
| 每账号可见 NLI 搜索 outgoing pending 上限 | 100 |
| 每账号隐藏 Provider-sync outgoing pending 上限 | 500 |
| 每账号 incoming pending 上限 | 500 |
| Provider 外部好友投影新鲜期 | 15 分钟 |
| Provider 外部好友投影最长未确认保留期 | 30 天 |
| 好友同步任务结果保留期 | 7 天 |
| 好友聚合查询快照有效期 | 10 分钟 |

## 正式设计文档

本大纲已由以下冻结文档细化；实现必须遵循其约束：

- NLI Account、认证、会话与恢复；
- Provider 适配器与账号绑定；
- 好友关系、查询与同步；
- Game Instance、邀请码与加入流程；
- HTTP API 与统一错误模型；
- WebSocket 通知协议；
- 联机信令、NAT 穿透与 TURN（Phase 9 / Gate F 已冻结，见 `signaling.md`）。
