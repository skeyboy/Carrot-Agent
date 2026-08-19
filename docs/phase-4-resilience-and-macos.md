# P4 韧性、审批与 macOS 加固收口报告

> 完成日期：2026-08-19
>
> 范围：fork/cancel-and-replace、附件 inbox、高风险审批、人工 reconcile、业务幂等键、生命周期故障恢复与 macOS bundle 加固

## 1. 阶段结论

P4 的本地 Agent 韧性闭环已经完成。运行中输入统一先进入 SQLite inbox；`append` 在原 Run 安全点消费，`fork` 挂起父 Run 并创建子 Run，子 Run 结束后父分支重新出现在恢复入口并可继续；`cancel_and_replace` 取消并 supersede 父 Run。文本和图片使用同一个持久化 `ProviderMessage`，应用退出后仍可从恢复横条继续未消费的分支输入。

非只读工具在执行副作用前必须进入 `waiting_for_approval`。审批与 `run_id + call_id + arguments_hash` 绑定；批准后通过 same-run resume 执行 prepared 工具，拒绝则提交结构化 ToolOutput 后让模型继续。`AgentTool::business_idempotency_key` 可从订单号、资源 ID 等领域参数生成跨 Run 稳定键；未提供时 Runtime 使用当前 Run/Call/参数 hash 的 execution fallback。最终键随执行上下文传给 Adapter，并受数据库唯一索引保护。

进程在外部副作用执行中被强制终止时，lease takeover 将 Run 隔离为 `recovery_required`。用户必须选择 Mark succeeded、Mark failed 或 Abandon；前两种决定提交 Observation 后才允许恢复，不会盲目重放工具。

## 2. 数据库与事务边界

Migration `202608190003_p4_resilience` 新增：

- `pending_inputs.child_run_id`，记录 inbox 被哪个子 Run 原子消费；
- `tool_executions.idempotency_key`、`reconciliation_status`、`reconciliation_note`；
- `(tool_name, idempotency_key)` 唯一索引，拒绝同一业务操作的重复落点；
- `tool_approvals`，保存参数 hash、决定和时间戳。

创建分支 Run、更新父 Run、消费 pending input、提交用户 Item 和事件均在同一事务完成。审批先提交 prepared tool 和审批事件，批准后才允许 `mark_tool_executing`。人工 reconcile 同时更新 ToolExecution、提交 ToolOutput、写事件并转换 Run 状态。

## 3. UI 与 IPC

会话 Composer 在运行中提供 Add、Fork、Replace 三种输入意图，附件按钮保持可用。Vue 继续按职责拆分：

- `ChatComposer.vue`：输入、附件和 intent；
- `ToolApprovalBanner.vue`：参数预览与批准/拒绝；
- `RunRecoveryBanner.vue`：same-run resume、人工 reconcile 和放弃；
- `ConversationThread.vue`：事件归并和命令编排。

新增 typed IPC：`chat_branch`、`chat_tool_approval`、`chat_tool_recovery`；`chat_input` 返回完整 pending input，Snapshot 返回审批、幂等键和 reconcile 状态。

## 4. 休眠、退出与强制终止

Tauri `ExitRequested` 和 `Resumed` 会先为内存中的活跃 Run 事务提交 `pause_requested`，再向 Run cancellation tree 发送 pause。正常到达安全点时提交 `paused`；若退出发生得更快，下一实例在 lease 到期后把 `pause_requested` 恢复为 `paused`。

无法由进程内 hook 捕获的 SIGKILL、断电和系统强杀仍依赖 30 秒 lease。安全/可取消工具写入中断 Observation；已进入执行态的 external side effect 或 dangerous 工具进入人工 reconcile。生命周期 hook 是降低恢复延迟的补充，不替代 lease 和事务正确性。

## 5. macOS 加固

- 使用 `tauri.macos.conf.json` 隔离首发平台配置，主配置继续保留跨平台 bundle 能力；
- 最低系统版本设为 macOS 12，启用 hardened runtime；
- 显式配置最小空 Entitlements，后续新增系统能力必须经过权限审查；
- WebView 启用 CSP，仅允许本地资源、Tauri IPC 与 data/blob 图片；
- 增加 `nosniff` 响应头、DeveloperTool bundle category 和产品说明；
- 产物目标为 `.app` 与 `.dmg`。

开发环境生成 `.app` 后使用本机 ad-hoc identity、runtime options 和项目 Entitlements 重新签名，`codesign --verify --deep --strict` 通过，可验证 hardened-runtime 资源封装链。该签名没有 TeamIdentifier，不能分发。Developer ID 签名、Apple 公证和 stapling 需要发布证书及 CI 密钥，不能在源码测试中伪造，作为发布流水线门禁执行。

## 6. 验证矩阵

Rust 自动化覆盖：

- fork 消费包含图片的 inbox 并保留 suspended 父 Run；
- cancel-and-replace 精确消费 inbox、取消父 Run 并 supersede 旧 Item；
- dangerous 工具审批持久化，只有匹配批准的执行可恢复；
- 外部副作用 lease 过期进入 recovery_required，人工 Mark succeeded 后可恢复；
- pause_requested 后强制终止，lease takeover 恢复为 paused；
- cancellation tree 能枚举活跃 Run 供生命周期检查点使用。

前端验证覆盖既有 7 项会话/主题/恢复测试、TypeScript 类型检查和生产构建。macOS bundle 通过 Tauri CLI 构建，生成 22 MiB arm64 `.app`；Bundle ID 为 `com.carrot.desktop`、最低系统为 macOS 12，本机 ad-hoc hardened-runtime 签名通过 strict 校验。Developer ID 与公证未执行。

## 7. P4 后续边界

P4 功能阶段收口。进入后续阶段前仍需按发布环境完成 Developer ID 签名、公证与安装机验收；网络存储、局域网同步冲突协议、可查询外部副作用 Adapter 和更细粒度策略审批不纳入本阶段。
