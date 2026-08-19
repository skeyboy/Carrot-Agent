# P3 Durable Agent Runtime 阶段报告

> 完成日期：2026-08-19
>
> 状态：已完成

## 1. 阶段结论

P3 已将 P2 的单次 Provider 流请求升级为可持久化的统一 Agent Runtime。Chat Completions compatible Adapter、Run/Item/Event、工具 write-ahead 记录、ReAct 工具循环、Plan revision、有界 Reflection、运行预算和 Snapshot 恢复均已接入应用主路径。

本阶段使用 `http://127.0.0.1:11434/v1` 和 `phi4-mini:latest` 完成真实 OpenAI-compatible 流式适配验证，不需要或使用真实 OpenAI API Key。该本地模型服务能完成普通流式对话，但在本次验证中没有返回原生 `tool_calls`，而是生成了文本 JSON。因此真实工具闭环使用 Fake Provider 协议测试验证；Carrot 不会把文本中的 JSON 猜测为可执行工具调用。

## 2. 落地范围

### Provider Adapter

- `openai-oxide` 启用 `chat` feature；
- 新增 Chat Completions Adapter，支持自定义 Base URL、`store`、图片 data URL、严格 Function Schema 和流式 tool-call arguments 拼接；
- Responses 与 Chat Completions 共用 Carrot 自有的 `ProviderInputItem`，支持 message、tool call 和 tool output 重放；
- 只有 Provider 返回结构化 tool call 时才进入工具执行管线。

### Durable Runtime

- `AgentRuntime` 统一执行 Provider 请求、ReAct、工具 Observation 回传和终止判断；
- `max_model_steps` 是模型调用硬预算，取消和请求超时形成显式终止；
- `fast` 跳过 Plan，`auto` 使用持久化 Plan + ReAct，`quality` 额外写入一次有界 Reflection 决策摘要；
- 模型流式 delta 是 transient event；最终消息、工具调用和 Observation 是 committed Item；
- Provider 远端状态不是恢复真相源，每轮请求均可由本地 committed Item 重建。

### 数据与事务

Migration `202608190002_p3_agent_runtime` 新增：

| 表                | 用途                                        |
| ----------------- | ------------------------------------------- |
| `tool_executions` | 工具参数 hash、风险、状态、输出和可重试属性 |
| `plans`           | Run 的版本化 Plan                           |
| `plan_steps`      | Plan revision 内的有序步骤                  |
| `run_snapshots`   | 事件高水位对应的运行状态缓存                |

关键事务不变量：

1. 用户 Item 与 Run 启动事件同时提交；
2. tool call Item、`prepared` 执行意图和事件同时提交；
3. tool output Item、工具终态和 Observation 事件同时提交；
4. Run 状态、单调事件序号、lease 和 materialized snapshot 同时更新；
5. Plan revision 递增时，上一 active revision 在同一事务内变为 `superseded`。

当前 lease 为短期运行所有权记录，每次 durable transition 刷新。旧进程扫描、抢占和 reconcile 属于 P4。

### 工具系统

工具注册必须声明风险、幂等、取消和 reconcile 能力。首批只读工具为 `calculator` 和 `get_current_time`。两者使用严格 JSON Schema，并在 Rust 工具边界再次校验参数。未知工具直接拒绝。写操作、外部副作用和人工审批仍保持关闭，进入 P4。

### IPC 与 Vue

- 新增 `chat_snapshot` typed command；
- DTO 将 SQLite 的 `i64` 序号和毫秒时间转换为字符串，避免 JavaScript 精度丢失；
- Vue 在订阅实时事件后加载 Snapshot，并在 completed/failed/cancelled 后再次对账；
- `ConversationThread` 保持协调职责，运行状态拆为独立 `AgentRunStatus` 组件；
- 应用或会话重新加载后，消息和工具轨迹从 SQLite 恢复，不再依赖组件内存。

## 3. 验证结果

```text
cargo test agent::runtime::tests::persists_react_tool_loop_with_contiguous_events
  PASS: 两轮模型请求、calculator、Observation、最终答案
  PASS: 4 个 committed Item、1 个 succeeded tool execution
  PASS: durable event seq 从 1 连续递增
  PASS: UI 只收到一次最终 Completed

cargo test providers::openai_chat::tests::streams_from_local_openai_compatible_service -- --ignored
  PASS: http://127.0.0.1:11434/v1
  PASS: phi4-mini:latest 返回 LOCAL_ADAPTER_OK

npm run typecheck
  PASS
```

最终提交前还执行完整 `npm run check`，结果以 Git 提交时的工作树为准。

## 4. 明确边界

P3 不包含真实 OpenAI API Key 的公网验收、高风险工具审批、pause/resume、旧 runtime 扫描与 lease 抢占、未知副作用 reconcile、durable pending input UI、休眠/唤醒自动恢复和局域网同步。这些内容进入 P4。当前若进程在工具外部副作用期间退出，不允许据此宣称 exactly-once。

## 5. 关键文件

- `src-tauri/src/agent/runtime.rs`
- `src-tauri/src/providers/openai_chat.rs`
- `src-tauri/src/persistence/run_repository.rs`
- `src-tauri/src/tools/mod.rs`
- `src-tauri/migrations/202608190002_p3_agent_runtime/`
- `src/components/chat/ConversationThread.vue`
- `src/components/chat/AgentRunStatus.vue`

## 6. 下一阶段

P4 按以下顺序执行：

1. pause requested -> safe point -> paused/resume；
2. durable pending input 的 append/fork/cancel-and-replace；
3. 启动时旧 runtime 扫描、lease 接管和工具 reconcile；
4. 高风险审批、幂等键和未知副作用恢复 UI；
5. app lifecycle、休眠/唤醒和故障注入矩阵。
