# ADR 0006: MCP Resources

- 状态：P6 延后
- 决策日期：2026-08-20

## 决策

P6 不把 MCP resources 或 subscriptions 注入模型上下文；资源变化通知不触发读取。

## 理由与进入条件

Resources 需要 URI 与 MIME allowlist、内容大小及缓存配额、内容净化、订阅生命周期、来源引用和 prompt-injection 边界。后续须先实现独立资源目录、授权 UI、持久化快照和缓存失效测试。
