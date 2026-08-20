# ADR 0005: MCP Elicitation

- 状态：P6 延后
- 决策日期：2026-08-20

## 决策

P6 不接收 MCP Server 的表单或 URL elicitation。处理器保持 SDK 的自动拒绝行为，也不声明该能力。

## 理由与进入条件

Elicitation 需要可信来源展示、字段级敏感信息标识、URL 跳转策略、暂停/恢复持久化、超时与取消语义。只有这些能力具备专用 UI 和钓鱼/Schema/恢复测试后才可进入实施。
