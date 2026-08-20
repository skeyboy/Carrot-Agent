# P5 macOS 本地 MCP 阶段报告

> 完成日期：2026-08-20
>
> 状态：已完成

## 交付结果

P5 已将 MCP 作为外部工具源接入现有 Agent Runtime，没有引入第二套执行引擎。首轮产品范围仅承诺 macOS 本地 stdio；共享领域类型、Catalog、配置和持久化模型不依赖 macOS 私有 API，为 Windows/Linux 后续 Adapter 保留边界。

- 官方 Rust SDK `rmcp 3.1.x`，仅启用 Client 与 child-process transport；
- `mcp-servers.toml` v1 配置、原子保存、严格校验、启动自动连接、休眠关闭与恢复重连；
- initialize、分页工具发现、稳定 provider alias、输入/输出 JSON Schema 校验及 256 KiB 结果上限；
- built-in 与已授权 MCP 工具组成动态 Registry，只有 `read_only_tools` allowlist 中的工具暴露给模型；
- Run 固化 Tool Catalog，工具执行固化来源 Server、远端名称、Schema hash、定义和策略；恢复时拒绝身份、定义或策略已变化的工具；
- Tauri typed IPC 与 Vue 设置页支持 Server CRUD、连接/断开、状态/错误展示和逐工具授权；
- 单个 Server 启动失败只进入 `failed`，不阻止 Carrot 启动及内置工具使用。

## 安全边界

Server executable 与 working directory 必须是绝对路径。子进程清空继承环境，仅恢复 `PATH`，stdout 专用于协议，stderr 持续排空且不写入应用日志。MCP annotations 只用于界面提示；将工具加入 `read_only_tools` 是用户明确作出的本地策略决定。

P5 不开放 MCP 写入、脚本、通用 shell、Streamable HTTP、OAuth、sampling、elicitation、resources、prompts 或 Tasks。上述能力进入 P6，且不得复用 P5 的只读自动执行策略。

## 验证

- `cargo test ... discovers_and_calls_a_stdio_server`：仓库内 Node fixture 完成 initialize、`tools/list`、`tools/call`、结构化输出和 shutdown；
- `npm run test:unit`：覆盖管理页加载、工具授权、连接与断开；
- `npm run check`：格式、ESLint、TypeScript、Vue build、Rust clippy 与全量测试；
- 应用内浏览器在 1280×720 和 520×760 验证 MCP 列表与新增表单，无页面级横向溢出或控件重叠。

## P6 接续项

P6 增加受控写入/脚本、macOS 进程隔离 Adapter、结果未知恢复、Streamable HTTP/OAuth、动态 list-changed 更新和更完整的 crash/timeout/flood 故障注入。Developer ID 签名、公证和 release bundle 实机矩阵继续作为发布环境门禁。
