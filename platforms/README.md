# Platform shells

This directory owns native IDE entry projects, platform-only packaging assets, and local build adapters. Shared application code remains in `src/`, while the Tauri Rust shell, commands, migrations, and cross-platform dependencies remain in `src-tauri/`.

```text
platforms/
├── macos/      # Xcode entry, entitlements and macOS build adapter
├── windows/    # Reserved Windows IDE and packaging entry
└── linux/      # Reserved Linux packaging and desktop integration entry
```

Tauri desktop does not generate a separate native project for every operating system. The canonical desktop executable is the Cargo package in `src-tauri`; platform-specific Tauri configuration files must remain beside `tauri.conf.json` so the CLI discovers and merges them automatically. Files here may depend on `src-tauri`, but shared runtime code must not depend on an IDE project in this directory.

Mobile targets follow Tauri's generated-project convention under `src-tauri/gen/apple` and `src-tauri/gen/android`. Those directories are intentionally separate from the macOS desktop wrapper here.
