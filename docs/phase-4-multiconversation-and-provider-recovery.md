# P4 多会话与 Provider 失败恢复补充

> 日期：2026-08-19  
> 范围：多主题并行会话、失败上下文隔离、图片能力防护、Settings 模式交互

## 1. 根因与结论

本轮 400 并非不同会话之间的数据串线。实际数据库记录显示：一个话题的首次请求包含图片且被纯文本模型拒绝，后续请求本身不含图片，但失败 Run 的用户消息仍处于 `committed`，Agent replay 因而反复把旧图片发送给模型。其他话题的请求正常完成，证明 `conversation_id` 隔离有效。

修复后，`failed` 和 `cancelled` Run 在终态事务内把已提交 Item 转为 `abandoned`。记录仍保留用于审计，但 `conversation_items` 只返回 `committed` Item，因此失败图片不会污染下一轮上下文。Migration `202608190004_abandon_terminal_run_items` 对既有数据库执行同一转换。

模型拒绝多模态输入时，错误改为可操作的信息；前端恢复原始草稿和附件，用户移除附件后即可再次发送。新建的 Local compatible 配置默认关闭 Images，避免未知本地文本模型被错误声明为支持图片；已有用户配置保持不变，可在 Settings 中显式调整。

## 2. 多会话生命周期

会话视图现在通过 Vue `KeepAlive` 按 conversation id 缓存。用户在多个主题间切换或短暂进入 Settings 时，各主题分别保留：

- 流式事件订阅和当前 Run id；
- 推理增量、最终输出及工具状态；
- 草稿、所选附件和暂停/取消控制状态。

Rust Runtime 原有 CancellationTree 已按 Run id 隔离，SQLite 唯一约束也是“每个 conversation 最多一个 active Run”，因此不同主题可以并行执行。缓存只解决前端销毁订阅造成的实时状态丢失，不改变后端并发模型。

## 3. Settings 模式边界

Settings 是独立工作模式。打开后会取消未完成的新建/重命名 UI，并对会话列表同时应用：

- 原生按钮 `disabled`，阻止选择、重命名、删除和新建；
- 容器 `inert`，从指针和键盘焦点顺序中移除；
- `aria-disabled`，向辅助技术表达不可交互状态；
- 降低视觉权重并移除 hover 反馈。

左上角 Carrot 入口仍可返回 Workspace，Settings 底部入口维持选中状态，用户不会被困在设置页。

## 4. 数据迁移

Migration：`202608190004_abandon_terminal_run_items`

```sql
UPDATE items
SET status = 'abandoned'
WHERE status = 'committed'
  AND run_id IN (SELECT id FROM runs WHERE status IN ('failed', 'cancelled'));
```

迁移仅改变 replay eligibility，不删除 Item、Run、Event 或错误原因。已在真实数据库的副本上验证，3 条历史失败 Item 均转换为 `abandoned`，原数据库未在验证期间修改。

## 5. 验证结果

- 前端单元测试 10 项通过，新增 Settings 模式锁定和两个主题并行流式回归；
- Rust 定向测试通过，覆盖失败多模态 Run 不污染后续纯文本上下文；
- Provider 错误归一化测试通过；
- 浏览器实际验证两个主题同时生成，来回切换后各自显示正确的输入、推理耗时和最终回答；
- Settings 实际验证 `inert=true`、会话按钮和新建按钮均为 disabled，页面无会话头部残留；
- 完整质量门禁结果以本次提交记录为准。

## 6. 边界

Provider 级 Images 开关仍是用户声明的能力，不自动猜测每个模型的模态。未来若同一 Provider 同时启用文本模型和视觉模型，应将 capabilities 下沉为模型级配置；在此之前，选择图片能力与实际模型不一致时会明确失败并允许无污染重试。
