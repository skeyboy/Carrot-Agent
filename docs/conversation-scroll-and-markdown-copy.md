# 会话滚动接管、流式批处理与代码复制

> 日期：2026-08-19

## 交互结论

流式响应默认跟随最新内容，但用户滚离底部后，浏览意图优先于自动滚动。会话列表以距离底部 24px 为容差：超过容差即暂停自动跟随，并在输入区正上方显示悬浮入口；用户点击入口或自行滚回底部后，立即恢复跟随并隐藏入口。

悬浮入口同时承担状态反馈：Run 存在或正在创建时显示 `In progress`，Run 结束后显示 `Complete`。状态变化不夺取滚动位置，确保用户阅读历史内容时不会被流式更新打断。

## 组件边界

- `ConversationThread.vue`：管理滚动容器、是否跟随最新内容以及 Run 状态；
- `ThreadScrollIndicator.vue`：只负责状态展示和回到底部命令；
- `MessageActions.vue`：只保留消息级 `Copy`，复制完整消息；
- `MarkdownContent.vue`：按 MarkdownIt 顶层 block token 拆分内容并保持原顺序渲染；
- `CodeBlockCopyButton.vue`：只位于 fenced/indented code block 右上角，复制所属代码块原始源码；
- `renderBatcher.ts`：以 50ms 上限批量提交流式 delta，终态事件到达时立即 flush。

复制条件复用 MarkdownIt token 和源码行映射，不用正则或语言关键词猜测代码。只有 `fence` 与 `code_block` 提供复制；标题、列表、引用、表格、强调、链接和行内代码均正常渲染但不显示代码复制入口。

Provider 高频 delta 不直接逐条修改 Vue 状态。首个 delta 建立 50ms 批次窗口，同一窗口内的文本和推理 delta 合并为一次响应式更新、一次 Markdown 解析和一次滚动。`completed`、`paused`、工具调用等顺序边界会同步 flush，`failed`、`cancelled` 和 Snapshot 重建会丢弃未提交的 stale delta。消息列表关闭 CSS scroll anchoring，避免浏览器锚定与应用自动跟随同时修改位置。

## 验收场景

1. 流式响应时用户上划，后续 token 不改变当前阅读位置；
2. 悬浮入口显示 `In progress`，Run 完成后原位置显示 `Complete`；
3. 点击入口回到底部、隐藏入口，并恢复后续自动跟随；
4. 用户自行滚到底部也自动隐藏入口和恢复跟随；
5. 普通内容仅显示 `Copy`，复制完整消息；
6. 标题、列表、引用、强调和行内代码不显示代码复制按钮；
7. 每个 fenced/indented code block 在自身右上角显示按钮，并只复制所属 block 源码；
8. 高频 delta 在 50ms 内合并，终态前不丢失尾部文本，取消后不提交 stale delta。
