# P4 Markdown 会话体验与 Xcode 调试补充

> 日期：2026-08-19  
> 范围：推理过程、消息复制、Markdown 预览、紧凑会话头部、macOS Xcode 本地调试

## 1. 本轮结论

本轮完成会话阅读与 macOS 开发体验收口。AI 推理在流式阶段持续展开，完成后自动折叠，耗时按钮同时承担状态摘要和展开入口；用户消息与 AI 最终回答均提供独立复制操作。消息正文和推理正文统一通过 Markdown 组件渲染，会话头部压缩为 52px 单行栏，为消息区释放纵向空间。

仓库新增共享 Xcode 工程，当前位于 `platforms/macos/Carrot.xcodeproj`。Xcode 的 Carrot Scheme 通过 Legacy Build Target 运行统一构建脚本，先生成 Vue 前端资源，再编译 Rust/Tauri 二进制；Debug 的 Run 动作由 LLDB 直接启动 `src-tauri/target/debug/carrot`，无需另开 Vite 服务。

## 2. 前端组件边界

| 组件                      | 职责                                 |
| ------------------------- | ------------------------------------ |
| `MarkdownContent.vue`     | 将受控 Markdown 转为消息 DOM         |
| `MessageActions.vue`      | 用户消息和 AI 回答共用的复制反馈     |
| `ReasoningDisclosure.vue` | 推理流状态、耗时、自动折叠与手动展开 |
| `ConversationMessage.vue` | 按角色组合正文、推理、图标和操作     |

Markdown 使用 `markdown-it`，启用换行和常用块级语法。原始 HTML 被禁用，链接只接受 `http`、`https` 和 `mailto` 协议，生成的外链附带 `noopener noreferrer`。该边界不承担服务端内容净化；未来若开放 HTML、内嵌媒体或插件，必须重新进行安全评审。

## 3. Xcode 工作流

首次使用前仍需安装 Node.js、Rust 和 npm 依赖。打开 `platforms/macos/Carrot.xcodeproj` 并选择共享的 Carrot Scheme：

- `Command+B`：执行前端类型检查与生产资源构建，再运行 `cargo build`；
- `Command+R`：构建后由 LLDB 启动 debug 二进制；
- Release/Profile：改用 `cargo build --release` 和 release 二进制；
- Clean：仅清理 `src-tauri` 对应的 Cargo target。

构建脚本通过临时 `TAURI_CONFIG` 将前端资源嵌入 debug 二进制，不修改日常 `npm run tauri dev` 使用的 Vite 开发配置。Xcode 工程不替代 Tauri bundle、Developer ID 签名和 Apple 公证流程。

## 4. 验证结果

- `npm run lint`：通过；
- `npm run test:unit`：8 项通过，覆盖推理折叠、双方复制、Markdown 预览和危险 HTML/链接隔离；
- `xcodebuild -project platforms/macos/Carrot.xcodeproj -list`：识别 Carrot Target 和共享 Scheme；
- `xcodebuild -project platforms/macos/Carrot.xcodeproj -scheme Carrot -configuration Debug build CODE_SIGNING_ALLOWED=NO`：通过；
- 浏览器 1280×720 验收：头部 52px、消息区 593px、输入区 75px，三区无重叠；流式推理展开，结束后折叠，耗时入口可重新展开；
- `npm audit --omit=dev`：生产依赖无已知漏洞。npm 仍报告既有开发依赖漏洞，未使用强制升级破坏当前工具链。

## 5. 后续边界

本轮没有改变 P4 Runtime、数据库 schema 或 Provider 协议，因此不需要数据库 migration。后续可在不改消息组件接口的情况下增加代码高亮、引用来源和单个代码块复制。Xcode 的正式 Archive 仍需接入持有证书的签名、公证与安装机验收环境。
