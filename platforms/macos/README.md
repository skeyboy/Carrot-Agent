# macOS shell

`platforms/macos` contains the macOS-native development and packaging boundary for Carrot:

- `Carrot.xcodeproj`: shared Xcode entry and LLDB launch scheme;
- `Config/Carrot.xcconfig`: Xcode-only build settings;
- `Entitlements.plist`: hardened-runtime entitlements consumed by the Tauri bundler;
- `scripts/xcode-build.sh`: adapter from Xcode actions to the shared npm and Cargo builds.

The executable is still produced from `src-tauri`; the Xcode project is a native IDE wrapper, not a second application implementation.

## Local debugging

Open `platforms/macos/Carrot.xcodeproj` and select the shared Carrot scheme:

- `Command+B` builds the Vue frontend and the Rust debug or release binary;
- `Command+R` launches `src-tauri/target/debug/carrot` under LLDB;
- Profile uses `src-tauri/target/release/carrot`;
- Clean delegates to `cargo clean` for the shared Tauri target directory.

Node.js, npm, Rust and Cargo must be available in Xcode's environment. JavaScript dependencies remain in the root `package.json`; Rust and platform-conditional native dependencies remain in `src-tauri/Cargo.toml`, so versions are not duplicated between shells.

## Tauri boundary

`src-tauri/tauri.macos.conf.json` stays beside the main Tauri configuration because the CLI discovers that filename automatically. It contains the macOS bundle settings and references this directory's `Entitlements.plist`. Release signing, notarization and stapling remain CI responsibilities.
