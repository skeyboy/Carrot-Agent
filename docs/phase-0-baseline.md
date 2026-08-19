# Phase 0 工程基线报告

> 状态：已完成  
> 日期：2026-08-18

## 阶段目标

建立可持续开发的 Tauri 2 + Vue 3 + TypeScript 工程，验证 Rust/TypeScript 类型安全 IPC，固定架构边界、质量门禁和 macOS 首发策略。本阶段不实现真实 Provider、数据库、附件上传或局域网同步。

## 已确认产品决策

- 首发 macOS，领域层和基础设施端口保持 Windows/Linux 可移植性；
- 必须支持本地 TOML 文件配置 OpenAI-compatible Base URL；
- 首版 OpenAI Responses API 使用 `store: true`；
- 本地仍保存完整执行记录，存储通过 Provider/Repository 端口扩展网络实现；
- 支持附件和图片输入；
- 暂不做云账户同步，跨设备同步先采用局域网发现与点对点连接；
- 首版不要求会话导出和应用层数据库加密，但同步传输必须加密并经设备配对。

## P0 交付物

| 交付物                                    | 状态 | 位置                                           |
| ----------------------------------------- | ---- | ---------------------------------------------- |
| Tauri 2 + Vue 3 + TypeScript 工程         | 完成 | 项目根目录、`src-tauri/`                       |
| Rust 分层模块骨架                         | 完成 | `src-tauri/src/`                               |
| 统一 IPC 错误与健康检查                   | 完成 | `src-tauri/src/error.rs`、`commands/system.rs` |
| Rust 生成 TypeScript 绑定                 | 完成 | `src/bindings.ts`                              |
| Provider 本地配置样例                     | 完成 | `config/providers.example.toml`                |
| 前端基线状态页与单元测试                  | 完成 | `src/App.vue`、`src/App.test.ts`               |
| lint、format、typecheck、test、build 脚本 | 完成 | `package.json`                                 |
| macOS CI                                  | 完成 | `.github/workflows/ci.yml`                     |
| 架构决策记录                              | 完成 | `docs/adr/0001-application-foundation.md`      |
| 完整验证                                  | 完成 | 本报告“验证结果”                               |

## 验收标准

- `npm run bindings` 能从 Rust 命令重新生成 TypeScript API；
- `npm run lint` 无错误和 warning；
- `npm run typecheck` 通过；
- Vue 单元测试通过；
- Rust format、clippy 和 tests 通过；
- 前端 production build 与 macOS Tauri bundle build 通过；
- 应用启动后 Rust 健康检查返回名称、版本、平台和阶段；
- CI 能检测生成绑定漂移。

## 验证结果

以下验证均已通过：

```text
npm run bindings
npm run check
npm run tauri build -- --debug --bundles app
```

结果：

- Prettier、ESLint 和 Vue TypeScript 检查通过；
- Vue/Vitest：1 个测试通过；
- Rust fmt、clippy（warnings as errors）通过；
- Rust：2 个测试通过，包括健康检查和 TypeScript binding 生成；
- Vite production build 通过；
- macOS `Carrot.app` debug bundle 生成成功；
- 桌面 1280×720 和窄视口 390×844 视觉检查通过；
- 两个视口均无横向溢出、越界元素或应用控制台错误。

产物：

```text
src-tauri/target/debug/bundle/macos/Carrot.app
```

默认 `all` bundle 还会尝试生成 DMG；当前环境中 `.app` 成功后，Tauri 的 `bundle_dmg.sh` 失败。DMG、签名、公证属于 P4 发布加固范围，不影响 P0 应用基线验收。

## 阶段结论

P0 验收完成。工程已具备可编译的 macOS Tauri 壳、类型安全 Rust/TypeScript IPC、稳定模块边界、自动化质量门禁和后续阶段所需的 Provider/存储/附件/同步扩展端口。P1 可以在不重构前端或 IPC 基线的情况下开始。

异步持久化、模型转换、迁移规范与 Provider SDK 的补充决策见 [Phase 0 架构补充](phase-0-architecture-supplement.md) 和 [ADR 0002](adr/0002-async-persistence-and-provider-sdk.md)。

## 下一阶段计划：P1 会话与本地持久化

1. 引入 Diesel + `diesel-async` + SQLite、异步连接包装和嵌入式迁移；
2. 实现 conversations、runs、items、run_events、pending_inputs、attachments、provider_profiles 表；
3. 通过 persistence model 与 domain model 的显式转换实现 `ConversationStore` SQLite Adapter；
4. 从应用配置目录加载并校验 Provider TOML，API Key 仍只保存引用；
5. 实现会话 CRUD IPC 和基础会话列表；
6. 覆盖迁移、Repository、配置错误和进程恢复测试。

P1 不接真实 LLM 网络请求；Provider 网络调用和流式聊天留在 P2。
