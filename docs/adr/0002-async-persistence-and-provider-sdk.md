# ADR 0002: Async persistence and provider SDK boundaries

- Status: Accepted
- Date: 2026-08-18

## Context

Carrot needs asynchronous application services without blocking Tauri's Tokio runtime. The local database is SQLite through Diesel, while future storage may be remote. Provider implementations must support OpenAI and custom OpenAI-compatible endpoints without coupling the domain to a third-party SDK or assuming that an OpenAI-shaped endpoint is a native Gemini endpoint.

## Decision

1. Rust application ports use `async_trait`; Tokio is the common asynchronous runtime. Commands and application services await ports and never perform blocking database or HTTP work directly.
2. P1 uses Diesel, `diesel-async` and SQLite. SQLite remains a synchronous engine: `diesel-async`'s `SyncConnectionWrapper<SqliteConnection>` runs Diesel operations through Tokio's blocking pool. Repository methods expose async interfaces, but the architecture does not claim native asynchronous SQLite I/O.
3. All layer crossings use explicit model conversion:
   - Tauri request/response DTOs convert to and from domain models;
   - Diesel query/insert/update models convert to and from domain models with `From` or fallible `TryFrom` implementations;
   - provider SDK request/response/event types convert to and from provider-neutral domain models.
4. Diesel row types and Provider SDK types cannot appear in domain ports, application service signatures, or generated TypeScript bindings.
5. Every schema change is a versioned Diesel migration. P1 embeds migrations in the application and runs pending migrations before repositories become available. Runtime code must not create or alter tables ad hoc.
6. Migrations are forward-only after release. A migration is tested against an empty database and the previous supported schema. Startup migration failure prevents writes and returns a stable application error; it is never silently ignored.
7. P2 adopts `openai-oxide` for the native OpenAI Responses adapter and tested OpenAI-compatible profiles, behind a Carrot-owned `LlmProvider` port. It will be added with default features disabled and only the required API features enabled.
8. `openai-oxide` is not the native Gemini abstraction. Gemini through an OpenAI-compatible gateway may use the compatible adapter when contract tests pass. Direct Gemini API support requires a separate `GeminiProvider` adapter and a later SDK evaluation.
9. Custom Base URL profiles declare protocol and capability data. A compatible endpoint is not assumed to support Responses, streaming tool calls, image inputs, Files API, or remote response storage merely because it accepts OpenAI-style authentication.

## Initial dependency plan

| Phase         | Dependency                                                | Purpose                                                         |
| ------------- | --------------------------------------------------------- | --------------------------------------------------------------- |
| P0 supplement | `async-trait 0.1`, `tokio 1`                              | Async ports and shared runtime contract                         |
| P1            | `diesel 2.3`, `diesel-async 0.9`, `diesel_migrations 2.3` | SQLite repositories, blocking isolation and embedded migrations |
| P2            | `openai-oxide 0.15` candidate                             | OpenAI Responses and compatible-provider adapter                |

Versions listed for P1/P2 are candidates observed on 2026-08-18. They are added and locked only when that phase begins, after a compatibility check with the repository toolchain.

## Consequences

- UI and application code have a consistently asynchronous contract while SQLite blocking work stays off Tokio worker threads.
- Local SQLite and a future network store can implement the same domain port.
- Database and SDK upgrades are contained in infrastructure mappings instead of propagating through the application.
- P1 must include migration and conversion tests, not only CRUD tests.
- P2 must include provider capability and wire-contract tests for each supported compatible endpoint profile.
- Native Gemini support remains possible without forcing Gemini semantics through an OpenAI-only SDK.

## References

- [diesel-async SQLite support and features](https://docs.rs/diesel-async/0.9.2/diesel_async/)
- [diesel-async migration harness](https://docs.rs/diesel-async/0.9.2/diesel_async/migrations/index.html)
- [openai-oxide repository](https://github.com/fortunto2/openai-oxide)
- [Gemini OpenAI compatibility](https://ai.google.dev/gemini-api/docs/openai)
- [rust-genai multi-provider SDK](https://github.com/jeremychone/rust-genai)
- [OpenAI function calling guide](https://developers.openai.com/api/docs/guides/function-calling)
- [OpenAI image and vision guide](https://developers.openai.com/api/docs/guides/images-vision)
