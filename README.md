# Carrot

Carrot is a macOS-first desktop LLM client built with Tauri 2, Rust, Vue 3, and TypeScript. The architecture keeps provider calls, credentials, tools, persistence, and synchronization behind Rust-side ports while the WebView remains a typed presentation layer.

Current milestone: P3 durable Agent Runtime completed. Carrot now supports OpenAI Responses and Chat Completions compatible adapters, committed Run/Item/Event history, a bounded ReAct tool loop, versioned plans, execution audit, and SQLite-backed chat snapshots. P4 adds pause/resume, pending input intents, lease takeover, side-effect reconciliation, and recovery UI.

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
- [Phase 2 provider runtime report](docs/phase-2-provider-runtime.md)
- [Phase 2 provider management supplement](docs/phase-2-provider-management.md)
- [Phase 3 durable Agent Runtime report](docs/phase-3-durable-agent-runtime.md)
- [Architecture decision records](docs/adr/)
