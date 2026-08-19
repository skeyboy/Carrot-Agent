# P4 主题、推理摘要与运行恢复切片

> 完成日期：2026-08-19
>
> 范围：自适应主题、消息边界、Provider 推理摘要、暂停/恢复、持久化 inbox、lease takeover 与恢复 UI

## 1. 执行前基线验证

上一阶段提交为 `213da0c fix: stabilize chat controls and scrolling`。执行前重新检查 Git 状态、重新运行 Specta TypeScript binding 导出，并确认 `chat_pause` 与 `replaces_run_id` 由 Rust 源正确生成且工作区无差异。P3 的 Run/Item/Event、Snapshot 和 `reasoning_summary` Item 类型可直接承载本切片。

## 2. 阶段结论

Settings 新增 Appearance 区域，支持 System、Light、Dark，并将选择保存到现有 `settings.toml`。旧配置没有 `theme` 字段时按 System 加载。主题由根节点统一解析，System 会监听 macOS appearance 变化，当前界面和后续组件可复用同一组语义色变量。

会话消息改为独立圆角条目。用户内容在左、用户图标在右；assistant/tool 图标保持在左。推理中显示 Provider 返回的实时摘要，完成后默认折叠，以推理耗时为入口展开或收起，最终回答始终保持可见。

## 3. 推理数据契约

OpenAI Responses API 提供 reasoning summary text delta 事件。Carrot 对原生 OpenAI Responses Profile 请求 `reasoning.summary = auto`，将 SDK 的 `response.reasoning_summary_text.delta` 映射为 `ReasoningDelta`。参考：[OpenAI Responses API Reference](https://developers.openai.com/api/reference/ruby/resources/beta/subresources/responses)。

Runtime 对单次模型请求聚合摘要和耗时，并在最终消息前提交：

- `items.kind = reasoning_summary`；
- `content_json = { summary, durationMs }`；
- `run_events.kind = reasoning_summary_committed`；
- 实时 delta 不写数据库；
- Snapshot 将同一 Run 的摘要附着到最后一条 assistant 结果；
- 不保存或展示隐藏 chain-of-thought，不从普通文本猜测推理。

Chat Completions compatible Adapter 当前 SDK 没有结构化 reasoning delta 字段，因此保持不发送、不解析。后续只能通过显式 Provider capability 和协议契约扩展。

## 4. 版本与迁移

本切片不新增数据库 migration。初始 migration 已允许 `items.kind = reasoning_summary`，新增表或列没有必要。主题属于版本化本地设置文件，通过 serde 字段默认值兼容旧版本；Rust 测试覆盖缺少 `theme` 的旧配置加载。

## 5. 验证结果

- Rust 测试覆盖 Responses reasoning 参数映射、摘要 Item 持久化、暂停后替换和旧设置兼容。
- Vue 测试覆盖三态主题保存、推理实时展开、完成后折叠/展开、用户图标顺序和复制。
- 浏览器验证 System 解析为当前 macOS 黑夜主题，Light/Dark 可独立切换。
- 浏览器验证深色 Settings 与会话页无文字重叠，Header、Sidebar、Composer 和消息条目使用一致主题表面。
- 浏览器验证用户/assistant 条目均为 7px 圆角，用户 SVG 为条目最后一个子元素，消息列表不覆盖 Composer。

## 6. 运行恢复切片结论

本切片将暂停从内存控制信号提升为持久化协议：`chat_pause` 必须先在单个 SQLite 事务中提交 `run_pause_requested`，把状态改为 `pause_requested`，之后才取消 Provider/Tool Future。工具 Observation 提交会保留 `pause_requested`，Runtime 到达安全点后再提交 `paused`。这样即使进程在点击暂停后崩溃，启动恢复也能完成暂停，而不会把它误判为普通中断。

`chat_resume` 通过版本检查和 lease claim 将 `paused/interrupted` 原子改回 `running`，继续使用原 `run_id`、Provider snapshot、模型和已提交 Items。重复 Resume 会冲突；存在未知外部副作用的 `recovery_required` Run 不允许 Resume。暂停后仍可显式选择 Edit，此时才取消旧 Run、supersede 旧输入并创建替换 Run。

运行中输入通过既有 `pending_inputs` 表落盘。首版 UI 暴露 `append`，后端契约同时保留 `fork`、`cancel_and_replace` intent；后二者只可靠排队，后续编排策略实现前不伪装为已消费。`append` 在模型安全点事务转换为用户 Item，并在 Run 完成事务中再次检查，避免输入与完成事件竞态导致丢失。

每个 active Run 每 10 秒续租，lease 为 30 秒。启动及恢复页加载会扫描属于旧实例且已过期的 lease：无未知副作用时进入 `paused/interrupted`；`executing` 的 `external_side_effect/dangerous` 工具进入 `recovery_required`，禁止自动重放。不会仅凭 owner 不同抢占未过期 lease，否则双进程会产生两个 worker；启动时遗留的 Run 由 UI 每 5 秒复查，收到正常流事件后立即停止复查。

## 7. 恢复 UI

恢复 UI 独立为 `RunRecoveryBanner.vue`：

- `paused/interrupted`：提供 Resume、Stop；Paused 额外提供 Edit；
- `recovery_required`：展示副作用未知原因，只允许 Abandon，不提供盲重试；
- 横条占用独立网格行，不覆盖第一条消息；桌面和窄屏下操作区可换行；
- Composer 在运行中保持可输入，提交成功后才显示追加消息；
- same-run Resume 保留原用户消息和已提交轨迹，不创建新 Run。

## 8. 版本、测试与验证

本切片复用了 P1 已迁移的 `pending_inputs`、`runs.runtime_instance_id`、`lease_expires_at_ms` 和 P3 的 `tool_executions.risk/retryable`，没有新增表或列，因此不创建空 migration。领域模型、Diesel Row 和 IPC DTO 均从现有版本化 Schema 转换；后续新增 reconcile/approval 字段必须单独 migration。

验证结果：

- Rust 新增 pause/request、same-run claim、重复 Resume 冲突、append exactly-once 消费和未知外部副作用恢复测试；
- 完整 Rust 测试 28 个通过，1 个默认 ignored；该 ignored 的 OpenAI-compatible 流式测试已使用 `http://127.0.0.1:11434/v1` 的 `phi4-mini:latest` 单独通过；
- Vue 7 个测试通过，覆盖恢复横条、same-run Resume、显式 Edit 和原有会话控制；
- 浏览器验证深色桌面下 Pause -> Resume，恢复横条、第一条消息和底部 Composer 不重叠；视觉检查发现并修复了横条绝对定位遮挡首条消息的问题。

## 9. 后续进展

上述 fork/cancel-and-replace、附件 inbox、高风险审批、人工 reconcile、业务幂等键、生命周期故障注入和 macOS bundle 加固已经在 [P4 韧性、审批与 macOS 加固收口报告](phase-4-resilience-and-macos.md) 完成。Developer ID 签名与 Apple 公证保留为持有发布证书的 CI 门禁。
