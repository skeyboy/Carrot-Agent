# P4 会话体验与运行控制切片

> 完成日期：2026-08-19
>
> 范围：会话布局、停止/暂停后编辑替换、回复复制

## 1. 阶段结论

本切片修复长会话和长侧栏的固定布局，并将运行中输入的停止、暂停、编辑、重发串成完整闭环。会话顶部不再重复展示 Provider/模型信息；已完成回复提供左下角复制操作。

这里的暂停语义是“在 cancellation-safe 边界停止当前 worker，持久化为 paused，并允许编辑后用新 Run 原子替换旧输入”。它不是从旧 Run 指令位置继续执行的 same-run resume。

## 2. 落地内容

- 应用根布局锁定到 `100dvh`，页面本身不滚动；侧栏会话列表和消息列表分别独立滚动。
- Settings 固定在侧栏底部；Composer 固定在会话底部，消息列表边界终止于 Composer 上方。
- 消息追加、工具记录和流式文本更新后自动滚动到最新内容。
- 取消树分别记录 cancel 与 pause 意图；Runtime 在安全取消点提交对应终态和事件。
- stop/pause 后恢复原文本和附件；重发时通过 `replaces_run_id` 校验旧 Run，并在数据库事务内 supersede 旧消息。
- 旧 Run 必须属于当前会话且处于 paused/cancelled，避免跨会话或已完成内容被修改。
- 已完成 assistant 消息显示 Copy 按钮；优先使用 Clipboard API，并为受限 WebView 提供 selection copy 回退。
- Vue 会话逻辑继续拆分为 Composer、运行状态、消息动作等独立组件。

## 3. 验证结果

- Vue 单元测试覆盖 stop 恢复、pause 恢复、编辑重发、旧消息移除和复制调用。
- Rust 集成测试覆盖暂停提交及替换输入的数据库原子性。
- 浏览器实测 19 个会话时列表独立滚动，Settings 完整可见；页面 `scrollHeight` 等于 viewport 高度。
- 浏览器实测长会话时消息列表 `scrollHeight > clientHeight`，Composer 底边等于 viewport 底边，消息列表与 Composer 不重叠，并自动滚动到末尾。

## 4. 后续 P4 计划

1. 持久化 `pause_requested` 和 pending input inbox，补齐运行中 append/fork/cancel-and-replace。
2. 实现 same-run resume、lease takeover 和应用重启后的 recovery UI。
3. 按工具幂等、查询、取消能力实现外部副作用 reconcile。
4. 补充休眠/唤醒、强制终止、pause 与工具完成竞态故障注入。
5. 完成审批策略、安全加固和 macOS 打包验收。
