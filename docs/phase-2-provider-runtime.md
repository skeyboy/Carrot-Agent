# Phase 2 Provider Runtime、设置中心与图片附件报告

> 状态：核心实现已完成
>
> 日期：2026-08-19
>
> 前置提交：`d7f2aae`

## 阶段开始时的进度审计

P0 工程基线和 P1 本地持久化已经完成，Git 工作区干净。可复用基础包括 Provider Profile、`credential_ref`、异步 SQLite、`runs/items/run_events/attachments` 表和类型安全 IPC。阶段开始时尚未实现设置中心、真实凭据存储、Provider Port、模型 SDK、流式事件、取消树和附件导入。

## 本阶段交付

| 能力                                        | 状态 | 主要位置                                                  |
| ------------------------------------------- | ---- | --------------------------------------------------------- |
| 侧栏底部 Settings 入口                      | 完成 | `components/SidebarSettingsNav.vue`                       |
| 左目录、右内容的分区设置中心                | 完成 | `components/settings/`                                    |
| 异步 `settings.toml` 读写和校验             | 完成 | `src-tauri/src/settings/`                                 |
| macOS Keychain 凭据 Adapter                 | 完成 | `src-tauri/src/credentials/mod.rs`                        |
| Provider-neutral 请求/事件端口              | 完成 | `providers/runtime.rs`                                    |
| `openai-oxide` Responses Adapter            | 完成 | `providers/openai_responses.rs`                           |
| 文本增量、工具调用、完成和错误事件          | 完成 | `commands/chat.rs`、`api/chat.ts`                         |
| 应用/Run/请求取消树                         | 完成 | `agent/cancellation.rs`                                   |
| 系统图片选择、规范化、hash 和 SQLite 元数据 | 完成 | `commands/attachment.rs`、`application/service.rs`        |
| 图片 `input_image` data URL                 | 完成 | `application/service.rs`、`providers/openai_responses.rs` |
| 组件化聊天输入和流式消息 UI                 | 完成 | `components/chat/`                                        |

## 设置设计

设置入口固定在会话侧栏最下方，Provider 管理只存在于设置中心。设置页面按 Providers、Runtime、Storage、Sync、About 分区：

- Providers 展示并编辑配置文件、默认 Provider、Base URL、模型目录、启用/默认模型、能力和 Keychain 状态；完整补充见 [Provider 管理报告](phase-2-provider-management.md)；
- Runtime 保存默认策略、请求超时和最大模型步骤；
- Storage 保存图片大小限制并显示数据库、附件和设置文件路径；
- Sync 只显示 P4 计划状态，不提供伪造的可用开关；
- About 显示版本、平台和实施阶段。

Vue 已按局部职责拆分。`App.vue` 只协调数据和页面状态；设置的壳层、五个区域、侧栏入口、会话线程和输入框均是独立组件。

## 凭据与 Provider 边界

`CredentialStore` 是异步端口。`keyring` 的同步 Keychain 调用全部进入 `tokio::task::spawn_blocking`。配置、SQLite、日志和 TypeScript DTO 均不包含 API Key，前端只能读取 `configured: bool`。

`openai-oxide` 固定为 0.16，关闭默认 features，仅启用 `responses`。Adapter 支持：

- Profile 自定义 Base URL；
- `store: true/false` 原样传递，默认 OpenAI Profile 为 `true`；
- 文本和 `input_image`；
- strict Function Schema 和并行工具调用声明；
- `response.output_text.delta`、function call arguments done、completed、failed 和 incomplete；
- `previous_response_id` 的领域字段保留。

实现遵循 OpenAI Responses 的多类型 input、流式事件和 function calling 契约：[Responses API Reference](https://developers.openai.com/api/reference/responses)、[Function calling guide](https://developers.openai.com/api/docs/guides/function-calling)。SDK 是第三方基础设施依赖，不进入 Application、Domain 或 IPC 类型。

## 取消与流式事件

`CancellationTree` 形成应用根 Token 和 Run 子 Token。取消一个 Run 不影响兄弟 Run；应用根取消传播到全部子运行。Provider 在建立 SSE 请求和读取每个事件时均参与 `tokio::select!`，超时由应用服务统一处理。

取消 Rust Future 不等于远端已撤销。P2 的语义是停止本地继续消费并结束 UI；P3 在持久化 Run 状态后增加远端 response ID 取消、终态竞态和恢复协议。

P2 事件是 transient Tauri Event，包含 `runId` 和 `conversationId`。命令启动与首个事件之间的竞态由前端短暂缓冲处理。P3 才将 durable sequence、事件缺口检测和 committed Item reducer 接入已有 `run_events/items` 表。

## 图片附件

文件选择发生在 Rust 端系统对话框，前端不能提交任意路径。导入流程：

1. 按设置限制原始文件大小；
2. 使用 `image` 解码 PNG、JPEG、WebP 或 GIF，并限制为 40 megapixel；
3. 拒绝多帧 GIF，将其余图片重新编码为内部 PNG，并移除 EXIF 和原始容器元数据；
4. 计算 SHA-256，写入应用附件目录；
5. 通过异步 Diesel model 写入 P1 已创建的 `attachments` 表。

本阶段没有修改数据库 Schema，因此没有新增空 migration。后续任何表或列变化仍必须创建新的版本化 migration。

## 验证结果

自动化覆盖：

- 应用设置默认创建、范围校验和原子替换；
- Responses 请求中的图片、工具和 `store: true` 映射；
- Run 取消隔离和应用根取消传播；
- JPEG 导入后规范化为 PNG、SQLite 查询和删除；
- Rust/TypeScript binding 生成；
- Vue 会话创建、设置入口和流式事件渲染。

浏览器可视验证覆盖 1280x800 和 520x800：侧栏底部入口稳定、设置双栏/窄屏横向目录无溢出、Provider 面板可滚动、聊天输入和流式消息布局正常，控制台无 error/warning。

真实 OpenAI 网络请求未执行，因为工作区没有用户 API Key；本报告不把编译和协议映射测试描述为实网成功。Keychain 的真实保存/删除也应由用户在 Tauri 应用内使用自己的测试凭据验收。

## 当前限制

- OpenAI-compatible `chat_completions` Profile 可加载但 P2 聊天执行会明确拒绝，不能静默改用 Responses；
- 当前消息和 Provider 流是会话内显示状态，尚未转换为 durable Run/Item 事务；
- Function Call 已归一化并可显示，但 Tool Registry、Schema 校验、执行、结果回传和多轮循环属于 P3；
- 附件当前统一转 PNG，尚未生成缩略图，也没有远端 Files API 上传策略；
- 流事件尚无 durable sequence 和重连快照。

## 下一阶段计划：P3 工具与 Durable Run

1. 实现异步 `RunStore`，以事务提交用户 Item、Run 状态和单调事件序列；
2. 建立 ChatEvent reducer 和 transient/durable sequence 分层；
3. 实现 Tool Registry、严格 JSON Schema、参数校验和场景工具子集；
4. 实现 Tool Executor 的超时、取消、输出限制、错误归一化和风险策略；
5. 持久化 function call intent、`call_id`、Observation 和 function call output；
6. 完成 Responses 多轮工具循环与 `previous_response_id`/本地重放策略；
7. 增加 Fake Provider、SSE wire fixtures、并行工具和故障注入测试；
8. 将聊天 UI 切换到 durable Run/Item 快照，并增加工具轨迹和 token/耗时视图。
