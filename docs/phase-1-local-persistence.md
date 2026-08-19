# Phase 1 本地持久化与会话工作区报告

> 状态：已完成
>
> 日期：2026-08-19
>
> 基线提交：`714c81a`

## 阶段目标

在不接入真实 LLM 网络请求的前提下，建立异步 SQLite 持久化、版本化 migration、Provider 本地配置、会话 CRUD 与基础工作区，并为后续可恢复 Run Runtime 固定稳定的数据标识和约束。

## 已交付

| 交付物                                        | 状态 | 位置                                                     |
| --------------------------------------------- | ---- | -------------------------------------------------------- |
| Diesel 2.3 + `diesel-async` 0.9 SQLite 执行层 | 完成 | `src-tauri/src/persistence/`                             |
| deadpool 异步连接池与 Tokio blocking 隔离     | 完成 | `persistence/database.rs`                                |
| 嵌入式、版本化 migration                      | 完成 | `src-tauri/migrations/202608190001_initial_persistence/` |
| 会话领域模型与 persistence model 转换         | 完成 | `domain/conversation.rs`、`persistence/models.rs`        |
| 异步 `ConversationStore` SQLite Adapter       | 完成 | `persistence/repository.rs`                              |
| Provider TOML 异步加载、创建、校验和 reload   | 完成 | `providers/config.rs`                                    |
| 类型安全会话与 Provider IPC                   | 完成 | `commands/conversation.rs`、`commands/provider.rs`       |
| 会话创建、选择、重命名、删除工作区            | 完成 | `src/App.vue`、`src/api/workspace.ts`                    |
| Rust 生成的 TypeScript bindings               | 完成 | `src/bindings.ts`                                        |

## 数据库设计结论

首版 migration 创建：

- `conversations`：标题、新 Run 的默认 Provider/Model、乐观版本；
- `runs`：实际 Provider/Model 快照、状态、阶段、lease 和事件高水位；
- `items`：规范化、可重放上下文；
- `run_events`：Run 内严格唯一的 durable sequence；
- `pending_inputs`：`append`、`fork`、`cancel_and_replace` 持久化 inbox 基础；
- `attachments`：附件元数据和应用内相对路径；
- `provider_profiles`：未来 Run 配置快照的非敏感记录。

Conversation 上的 Provider/Model 明确是默认值；真实执行配置属于 Run 快照。数据库限制同一 Conversation 只能有一个 queued/running/pause-requested/waiting-for-approval Run，并限制 `(run_id, seq)` 唯一。

SQLite 每个连接启用：

```text
foreign_keys = ON
journal_mode = WAL
busy_timeout = 5000ms
```

SQLite 仍是同步引擎。`SyncConnectionWrapper<SqliteConnection>` 将 Diesel 操作放入 Tokio blocking pool，Application/Command 只看到异步接口。

## 模型边界

```text
Tauri request DTO
  -> validated application input
  -> domain model
  -> Diesel insert/query/change model
  -> SQLite
```

返回路径执行反向转换。Diesel row 不进入 Application 或 IPC。为避免 JavaScript 精度丢失，领域层 `i64` 时间戳在 IPC 中编码为十进制字符串，版本转换为经过范围校验的 `i32`。

更新和删除使用 `expected_version`。版本不匹配返回 `CONFLICT`，前端不能静默覆盖更新后的记录。

## Provider 配置

应用首次启动时在 Tauri `app_config_dir/providers.toml` 创建配置模板，后续从该文件 reload。配置校验包括：

- 拒绝未知字段和重复 ID；
- Provider ID 仅允许小写 ASCII、数字、`-`、`_`；
- 拒绝空 label、model 和 credential reference；
- 远端 URL 必须 HTTPS，仅 loopback/localhost 允许 HTTP；
- Base URL 不允许 query 或 fragment；
- OpenAI Responses 类型必须使用 Responses protocol；
- API Key 字段属于未知字段，会被拒绝。

首版依旧只保存 `credential_ref`，不保存 API Key。

## Runtime 韧性落地边界

P1 已落地：稳定 Run/Item ID 容器、状态约束、active Run 唯一约束、事件序列唯一约束、lease 字段、旧 runtime 非终态查询以及 durable inbox 表。

P1 未落地：Agent Orchestrator、模型流、工具执行、RunStore 事务推进、CancellationToken、pause/resume、pending input 消费和崩溃自动恢复。这些按 ADR 0003 在 P2-P4 实现，不能因数据库表已存在而宣称功能可用。

## 验证结果

自动化验证覆盖：

- migration 空库初始化与重复运行；
- WAL 与核心表创建；
- SQLite 事务回滚；
- active Run 唯一约束；
- Run event sequence 唯一约束；
- 旧 runtime 非终态 Run 恢复扫描查询；
- 会话 create/list/get/update/delete；
- stale version 冲突；
- Provider 样例、文件初始化、重复 ID、远程 HTTP、未知字段；
- Application Service 初始化；
- TypeScript binding 生成；
- Vue 会话创建工作流；
- 1120×760 与 390×844 的创建/重命名、布局、溢出和控制台检查。

真实 Tauri 进程已验证在 macOS 应用目录创建数据库和配置文件。数据库报告 migration `202608190001`，journal mode 为 WAL。

## 阶段结论

P1 完成。Carrot 已拥有可迁移的本地数据基础、异步 Repository、严格 Provider 配置和可操作的会话工作区。P2 可以在不让 OpenAI SDK 类型或流式临时状态污染数据库/IPC 的情况下接入凭证、Provider 与附件。

## 下一阶段计划：P2 Provider、流与附件

1. 接入 macOS Keychain credential Adapter，配置仍只引用凭证 ID；
2. 锁定并契约测试 `openai-oxide`，关闭 default features，首批启用 `responses`；
3. 定义 Provider-neutral request、output item、usage、error 和 stream event 模型；
4. 实现 OpenAI Responses 与 OpenAI-compatible capability profile；
5. 实现 Run 级 `CancellationToken`，逐个记录 Future cancellation safety；
6. 将 transient token sequence 与 committed Item/event sequence 分离；
7. 实现 PNG/JPEG/WebP/非动图 GIF 导入、校验、hash 和图片输入；
8. 使用 Fake Provider 和 wire fixtures 验证普通响应、SSE、图片、取消、错误与 `store: true`；
9. 更新聊天 UI 并输出 P2 阶段报告。
