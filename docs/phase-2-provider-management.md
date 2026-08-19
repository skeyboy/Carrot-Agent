# Phase 2 Provider 管理补充报告

> 状态：实现完成，待最终全量校验
>
> 日期：2026-08-19
>
> 基线提交：`7abf3bf`

## 本次结论

Provider 管理已从工作区侧栏收口到 Settings。侧栏底部只保留 Settings 入口，Providers 作为设置中心内的独立分区。Settings 标题栏提供明确的 Back 入口，返回时保留原会话选择和运行状态；设置期间会话列表保持不可交互。Vue 按职责拆分为 `ProviderSettings`、`ProviderProfileEditor` 和 `ProviderCreateForm`，`App.vue` 只负责加载和协调 IPC 状态。

本地 `providers.toml` 升级为版本 2 的 Provider Catalog，并保存以下状态：

- Provider 名称、类型、协议、自定义 Base URL 和 capability overrides；
- 全局默认 Provider；
- Provider 返回的模型目录、用户启用的模型和默认模型；
- 最近一次模型同步时间；
- `credential_ref`，API Key 本身仍只进入系统凭据存储。

应用启动时仍由 Rust 异步读取 `providers.toml`。旧文件缺少版本、默认 Provider 和模型数组时，会选择首个 Provider 为默认项，并以原 `default_model` 初始化模型选择；首次修改后以版本 2 原子写回。此次没有数据库 Schema 变化，因此没有新增 Diesel migration。

## Provider 操作

Settings 现在支持：

1. 新增、编辑和删除 Provider；
2. 编辑名称、Base URL、`store_responses` 和工具、图片、文件能力；
3. 设置全局默认 Provider，新会话默认使用该 Provider 及其默认模型；
4. 通过 `openai-oxide` 的 Models resource 调用兼容端点 `GET /models`；
5. 从同步目录中启用多个模型，并从启用集合中选择默认模型；
6. 保存、替换和删除 macOS Keychain 中的 API Key。

模型目录和可执行能力是两个概念。Models API 可能返回 embedding、image、audio 等非对话模型，本阶段不根据模型 ID 猜测能力；用户显式决定启用项。接口依据 OpenAI 的 [Models 文档](https://developers.openai.com/api/docs/models) 和 SDK 的 Models resource 实现。

删除 Provider 有两个约束：至少保留一个 Provider；已被现有会话引用的 Provider 不能删除。删除 Profile 不自动删除 Keychain Secret，避免配置文件写入失败和不可回滚的凭据删除形成跨存储不一致；凭据可在删除 Profile 前由用户明确移除。

## 安全与兼容边界

- 远程 Base URL 必须使用 HTTPS；仅 loopback 地址允许 HTTP；
- OpenAI 远程模型同步要求已配置 API Key；loopback compatible 服务允许无 Key 同步，SDK 使用非敏感占位凭据；
- 配置写入先生成临时文件，再替换正式文件；内存状态只在文件写入成功后更新；
- `openai_responses` kind 仍只允许 Responses 协议；
- `chat_completions` compatible Adapter 不在本次范围内，选择该协议进行聊天会继续返回明确配置错误。

## 验证范围

自动化测试覆盖旧配置兼容、版本 2 重载、默认 Provider、模型选择持久化、Provider 编辑/删除和 Vue 设置交互。视觉验收覆盖 `1280x800` 和 `520x800`，确认设置导航、编辑表单、模型列表和删除确认无横向溢出或卡片重叠。

本次没有使用真实 API Key，也没有宣称 OpenAI 实网模型同步或对话验收成功。模型同步 Adapter 只通过编译、类型契约和本地状态测试验证。

## 下一阶段计划：P3

1. 实现 `chat_completions` compatible Adapter，并保持 Provider-neutral 事件协议；
2. 将用户输入、Run、Item 和模型输出在事务中持久化；
3. 建立 durable event sequence、高水位恢复和 UI reducer；
4. 实现 Tool Registry、严格参数校验、执行策略和结果归一化；
5. 完成多轮工具执行循环、最大步数、超时与取消传播；
6. 通过 Fake Provider、wire fixtures 和故障注入验证恢复行为；
7. 最后才使用用户提供的测试凭据进行受控实网验收，不把实网 Secret 写入测试、日志或配置文件。
