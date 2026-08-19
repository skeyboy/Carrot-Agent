# Tauri 多平台壳工程布局

> 日期：2026-08-19
> 当前落地：macOS；预留：Windows、Linux

## 1. 设计结论

Carrot 采用“一个共享 Tauri Rust 壳 + 多个平台入口包装层”，不为每个桌面系统复制业务 Runtime：

```text
Carrot/
├── src/                         # Vue/TypeScript 共享界面
├── src-tauri/                   # Tauri/Cargo 共享桌面壳与领域实现
│   ├── tauri.conf.json
│   ├── tauri.macos.conf.json    # Tauri 自动发现的 macOS 薄入口
│   ├── Cargo.toml
│   └── src/
└── platforms/
    ├── macos/                   # Xcode、xcconfig、Entitlements、构建适配器
    ├── windows/                 # Windows 原生入口预留
    └── linux/                   # Linux 原生入口预留
```

`src-tauri` 是可执行程序的唯一实现位置。`platforms/<os>` 只放 IDE 工程、签名权限、打包资源和调用共享构建的脚本，依赖方向固定为 platform shell -> shared Tauri shell。

## 2. 与 Tauri 约定的关系

Tauri 会从 `src-tauri` 自动读取 `tauri.macos.conf.json`、`tauri.windows.conf.json` 和 `tauri.linux.conf.json`，并与主配置合并，因此这些薄配置不能迁离 `src-tauri`。macOS 配置可引用 `platforms/macos/Entitlements.plist`，将平台资产集中管理。

Tauri 的 `src-tauri/gen/apple` 和 `src-tauri/gen/android` 用于 iOS/Android 生成工程。macOS 桌面 Xcode 包装层不放入 `gen/apple`，避免与未来 iOS 工程发生所有权冲突。

参考：

- [Tauri platform-specific configuration](https://v2.tauri.app/reference/config/#platform-specific-configuration)
- [Tauri macOS application bundle](https://v2.tauri.app/distribute/macos-application-bundle/)
- [Tauri mobile icon/generated project paths](https://v2.tauri.app/develop/icons/)

## 3. macOS 当前入口

`platforms/macos/Carrot.xcodeproj` 是共享 Cargo 程序的 Xcode/LLDB 入口：

- Xcode 构建调用 `platforms/macos/scripts/xcode-build.sh`；
- 脚本从目录位置解析仓库根目录，不依赖调用者当前工作目录；
- 前端依赖继续由根 `package.json` 和锁文件管理；
- Rust 及 native crate 依赖继续由 `src-tauri/Cargo.toml` 和 `Cargo.lock` 管理；
- `Config/Carrot.xcconfig` 只保存 Xcode 平台设置，不复制 Cargo/npm 版本；
- Debug/Release 分别启动共享 target 下的对应二进制。

这种安排确保 Xcode、命令行 `npm run tauri dev` 和 CI 使用同一依赖图与产物，不产生第二套 macOS 实现。

## 4. 后续平台接入规则

Windows 接入时新增 `src-tauri/tauri.windows.conf.json`，并在 `platforms/windows` 放置 Visual Studio 辅助入口、签名和安装器资产。Linux 接入时新增 `src-tauri/tauri.linux.conf.json`，并在 `platforms/linux` 放置 desktop entry、包元数据和发行版脚本。

平台专属 Rust crate 依赖使用 Cargo 的 `target.'cfg(target_os = "...")'.dependencies`，平台专属权限使用 Tauri capabilities target filter。不得从共享 domain/application 层导入 Xcode、Visual Studio 或发行版脚本。

## 5. 验收

- 根目录不再散落 Xcode 工程或 macOS 专属构建脚本；
- `xcodebuild -project platforms/macos/Carrot.xcodeproj -list` 能识别 Carrot Scheme；
- Xcode Debug 构建能从新目录解析 Vue 与 Cargo 工程；
- Tauri macOS bundle 能从平台目录读取 Entitlements；
- Windows/Linux 目录明确标记为未实现，不提供伪入口。
