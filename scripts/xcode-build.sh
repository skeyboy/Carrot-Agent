#!/bin/bash

set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CARGO_MANIFEST="$PROJECT_ROOT/src-tauri/Cargo.toml"

cd "$PROJECT_ROOT"

if [[ "${ACTION:-build}" == "clean" ]]; then
  cargo clean --manifest-path "$CARGO_MANIFEST"
  exit 0
fi

npm run build

# Xcode launches the debug binary directly, so embed the freshly built frontend
# instead of pointing the WebView at Vite's development URL.
export TAURI_CONFIG='{"build":{"devUrl":null,"frontendDist":"../dist"}}'

if [[ "${CONFIGURATION:-Debug}" == "Release" ]]; then
  cargo build --release --manifest-path "$CARGO_MANIFEST"
else
  cargo build --manifest-path "$CARGO_MANIFEST"
fi
