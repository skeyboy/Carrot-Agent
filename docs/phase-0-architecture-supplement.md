# Phase 0 架构补充：异步、持久化与 Provider SDK

> 状态：已完成  
> 日期：2026-08-18  
> 适用范围：P0 基线补充，不包含 P1/P2 功能实现

## 本次结论

1. Rust 统一采用 Tokio 运行时，跨层异步端口采用 `async_trait`。`ConversationStore` 已从占位接口升级为基于领域模型的异步 CRUD 契约。
2. P1 数据库采用 Diesel + SQLite + `diesel-async`。SQLite 底层仍是同步操作，通过 `SyncConnectionWrapper` 和 Tokio blocking pool 隔离；不在 Tauri command 或 Tokio worker 中直接执行同步 Diesel 查询。
3. IPC、数据库和 Provider 各自拥有边界模型，并通过显式 `From`/`TryFrom` 转换连接。Diesel row、SDK DTO 和 wire event 不得进入领域层。
4. 所有数据库版本变化必须通过 Diesel migration。迁移在 Repository 初始化前执行，覆盖空库升级和上一支持版本升级测试。
5. `openai-oxide` 可作为 P2 的 OpenAI Responses/OpenAI-compatible SDK 候选。当前版本支持自定义 Base URL、Responses、流式响应和工具调用；它是第三方 OpenAI API SDK，不是原生 Gemini SDK。
6. Gemini 通过 OpenAI-compatible 网关时可复用 compatible adapter，但必须通过能力和协议契约测试。未来直连 Gemini API 时使用独立 Adapter，并重新评估原生 SDK 或多 Provider SDK。

## 本次代码变更

- 直接依赖 `async-trait 0.1` 与 `tokio 1`；
- 新增 `Conversation`、`NewConversation`、`ConversationChanges` 领域模型；
- 新增异步 `ConversationStore` CRUD 契约与稳定的存储错误边界；
- 新增 ADR 0002，固定 P1/P2 技术边界。

## 明确不在本次实现的内容

- Diesel、`diesel-async`、连接池和 SQLite schema；
- migrations 目录与启动迁移执行器；
- Repository 实现及会话 CRUD IPC；
- `openai-oxide` 依赖、真实网络请求、SSE 和 function calling；
- Gemini 原生 API Adapter。

这些内容不属于 P0 基线，提前加入会产生未被用例验证的依赖和错误抽象。

## P1 执行计划

1. 锁定 Diesel 2.3、`diesel-async` 0.9 和 `diesel_migrations` 2.3 的兼容版本与 feature 集；
2. 建立 `migrations/<timestamp>_<name>/up.sql` 和 `down.sql`，创建首版核心表，包括 `run_events` 与 `pending_inputs` 的恢复基础；
3. 实现数据库启动、WAL、foreign keys、busy timeout、连接池和嵌入式迁移；
4. 创建 persistence models，并为所有 domain/persistence 转换添加测试；
5. 实现 SQLite `ConversationStore`，覆盖空库、升级、CRUD、并发和损坏数据测试；
6. 实现 Provider TOML 的配置模型、领域模型转换、校验和 reload；
7. 实现类型安全会话 CRUD IPC 和基础界面；
8. 运行完整质量门禁并输出 P1 阶段报告。

## P2 依赖分析入口

P2 开始时再加入 `openai-oxide`，建议配置为：关闭 default features，首批只启用 `responses`，根据附件实现再决定 `files`/其他 feature。验收前必须验证：

- 自定义 Base URL 和额外 header；
- `store: true`、`previous_response_id` 与完整 output item 保留；
- SSE 分片、取消、超时、重试和错误映射；
- 并行 function calls、参数增量拼接和 `call_id` 关联；
- 文本、图片和文件输入；
- compatible endpoint 的能力降级和不支持错误；
- SDK DTO 不越过 Provider Adapter 边界。

若 `openai-oxide` 在这些契约上不满足要求，替换 SDK 只影响 Provider Adapter，不改变 Agent Runtime 与领域模型。

### SDK 候选对比

| 方案                   | 优势                                                                       | 限制                                                     | 阶段结论                                                        |
| ---------------------- | -------------------------------------------------------------------------- | -------------------------------------------------------- | --------------------------------------------------------------- |
| `openai-oxide 0.15`    | OpenAI Responses 覆盖较完整；支持 Base URL、流式和工具调用；feature 可裁剪 | 第三方 0.x SDK；不提供原生 Gemini 语义                   | P2 OpenAI/compatible 首选候选，锁定版本并做 wire contract tests |
| `genai 0.6`            | Rust 多 Provider 抽象，包含 OpenAI Responses、Gemini 原生、图片和工具      | 统一抽象可能无法暴露 Carrot 需要的全部厂商细节；仍是 0.x | 原生 Gemini 阶段对照评估，不在 P0/P1 引入                       |
| 自建 `reqwest` Adapter | 协议与事件控制最完整，依赖最少                                             | SSE、重试、错误和 API 演进维护成本最高                   | SDK 契约不满足时的兜底方案                                      |

Google 官方提供 Gemini 的 OpenAI compatibility endpoint，因此基础文本、Chat Completions 和部分工具场景可以由 compatible Adapter 接入。Google 同时建议：若不受 OpenAI SDK 限制，或需要 Gemini File API、Grounding 等高级能力，应调用原生 Gemini API。Carrot 据此不承诺一个 OpenAI-compatible Profile 等价于完整 Gemini 支持。

参考：

- [Gemini OpenAI compatibility](https://ai.google.dev/gemini-api/docs/openai)
- [Gemini partner and library integrations](https://ai.google.dev/gemini-api/docs/partner-integration)
- [rust-genai](https://github.com/jeremychone/rust-genai)
