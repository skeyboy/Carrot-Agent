# P6 macOS MCP 扩展阶段报告

> 完成日期：2026-08-20  
> SDK：官方 `rmcp 3.1.x`  
> 平台范围：macOS 产品验收，共享领域与协议层保持跨平台

## 交付结果

- 配置升级为 v2，并兼容迁移 P5 v1；stdio 与 Streamable HTTP 使用同一 Server/Catalog/策略模型。
- 高风险本地 Server 使用 macOS `sandbox-exec` 进程隔离、最小环境、目录写入 allowlist 和可选网络权限；解释器及依赖保持只读可访问，非 macOS 缺少 Adapter 时拒绝启用高风险工具。
- 工具策略逐项保存 `risk`、`idempotent`、`reconcile` 和启用状态。非只读调用沿用 durable approval；取消、超时或协议断开导致的副作用未知进入 `recovery_required`，不自动重放。
- Streamable HTTP 只允许 HTTPS 或 loopback HTTP，禁止 URL credential/query/fragment及自动重定向，限制连接、请求、SSE 单事件、结果和 Catalog 大小。
- Bearer 与 OAuth token 只进入系统 Keychain。OAuth 使用官方 SDK 的 discovery、PKCE/state 和授权码交换；回调严格绑定 loopback redirect origin/path。
- `tools/list_changed` 生成新的 Server/Catalog revision；刷新失败进入 degraded，并执行 1/2/4 秒有限重连。HTTP session 失效不自动重新初始化或重放工具调用。
- 管理 UI 支持 transport、认证、目录/网络权限、工具风险、幂等/reconcile、手动刷新和 OAuth 完成流程。
- 管理 UI 内置 `Carrot system MCP` 条目：受控本地高风险工具、远程 HTTP、安全认证、动态目录/重连默认开启并可持久化关闭；结果未知恢复和治理能力强制开启；高级能力保持关闭并锁定。

## 高级能力边界

Sampling、elicitation、resources、prompts、Tasks 和 MRTR 均保持关闭，并分别记录在 ADR 0004-0009。它们不属于 P6 完成条件。

## 验证

发布门禁使用 `npm run check`；MCP 定向验证使用 `cargo test --manifest-path src-tauri/Cargo.toml mcp`。仓库内 fixture 不依赖公网或第三方 MCP Server。

## 保留风险

- Developer ID 签名、公证、Intel/Apple Silicon 双架构与真实第三方 OAuth 服务仍属于发布环境验收。
- Windows/Linux 只保留接口与默认拒绝能力；后续必须实现各自的 `ProcessIsolation` 和 credential Adapter 才能开放高风险本地工具。
