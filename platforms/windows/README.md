# Windows shell placeholder

Windows packaging is not implemented in the current macOS-first milestone. A future Windows entry may place Visual Studio helpers, signing metadata and installer assets here while continuing to build the shared `src-tauri` Cargo package. Tauri-specific overrides must use `src-tauri/tauri.windows.conf.json` so the CLI can merge them automatically.
