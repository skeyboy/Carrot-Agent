# ADR 0004: MCP Sampling

- 状态：P6 延后
- 决策日期：2026-08-20

## 决策

P6 不向 MCP Server 暴露模型 sampling。`ClientHandler::create_message` 保持 SDK 默认 `method not found`，客户端能力中不声明 sampling。

## 理由与进入条件

Sampling 会让外部 Server 间接消费模型额度并接触对话上下文。后续启用前必须具备独立的 Provider/模型 allowlist、费用与 token 上限、逐次审批、上下文脱敏、持久化审计和恶意递归测试。
