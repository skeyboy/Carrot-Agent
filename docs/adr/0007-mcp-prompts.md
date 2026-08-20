# ADR 0007: MCP Prompts

- 状态：P6 延后
- 决策日期：2026-08-20

## 决策

P6 不发现或执行 MCP prompts，prompt list 变化通知不进入 Carrot 的工具目录。

## 理由与进入条件

外部 prompt 会改变模型指令边界。后续必须提供明确的来源展示、内容预览、用户主动选择、变量 Schema 校验、版本快照和指令注入测试，且不得静默合并到 system prompt。
