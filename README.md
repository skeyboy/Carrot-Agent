# Carrot

Carrot is a macOS-first desktop LLM client built with Tauri 2, Rust, Vue 3, and TypeScript. The architecture keeps provider calls, credentials, tools, persistence, and synchronization behind Rust-side ports while the WebView remains a typed presentation layer.

Current milestone: P3 durable Agent Runtime completed, with three P4 product-resilience slices landed. Carrot now supports OpenAI Responses and Chat Completions compatible adapters, committed Run/Item/Event history, a bounded ReAct tool loop, versioned plans, execution audit, SQLite-backed chat snapshots, transactional pause, same-run resume, durable append input, lease heartbeat/takeover, unknown side-effect isolation, recovery UI, persisted provider reasoning summaries, adaptive themes, and stable long-conversation layouts. Remaining P4 work includes fork/cancel-and-replace consumption, approval, manual side-effect reconciliation, lifecycle fault injection, and macOS packaging hardening.

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
- [Phase 4 chat experience slice](docs/phase-4-chat-experience.md)
- [Phase 4 appearance, reasoning and recovery slice](docs/phase-4-appearance-and-reasoning.md)
- [Architecture decision records](docs/adr/)
