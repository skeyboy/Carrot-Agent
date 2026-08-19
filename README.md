# Carrot

Carrot is a macOS-first desktop LLM client built with Tauri 2, Rust, Vue 3, and TypeScript. The architecture keeps provider calls, credentials, tools, persistence, and synchronization behind Rust-side ports while the WebView remains a typed presentation layer.

Current milestone: P1 local persistence and conversation workspace completed; P2 Provider integration is next.

## Requirements

- Node.js 22 or newer
- Rust stable
- macOS development tools for the initial target

The module boundaries remain portable to Windows and Linux; platform packaging is enabled after the macOS path is stable.

## Commands

```bash
npm install
npm run bindings
npm run dev
npm run tauri dev
npm run check
```

`npm run bindings` regenerates [src/bindings.ts](src/bindings.ts) from Rust commands and DTOs. The generated file is committed so frontend checks do not depend on starting the desktop runtime.

## Documentation

- [System design and implementation plan](docs/llm-client-design-plan.md)
- [Agent modes and resilient session runtime](docs/agent-runtime-modes-and-resilience.md)
- [Phase 0 baseline report](docs/phase-0-baseline.md)
- [Phase 1 local persistence report](docs/phase-1-local-persistence.md)
- [Architecture decision records](docs/adr/)
