# P4 主题与推理摘要切片

> 完成日期：2026-08-19
>
> 范围：自适应主题、消息边界、Provider 推理摘要、折叠交互

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

## 6. 后续 P4 计划

1. 将 `pause_requested` 作为独立事务边界，并实现 same-run resume。
2. 接入 durable pending input inbox 的 append、fork、cancel-and-replace。
3. 启动时扫描过期 lease，按工具能力执行 takeover 与 reconcile。
4. 加入高风险审批、未知副作用决策 UI 和幂等键。
5. 完成休眠/唤醒、强制终止、竞态故障注入和 macOS 打包加固。
