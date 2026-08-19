# ADR 0003: Durable run runtime invariants

- Status: Accepted
- Date: 2026-08-19

## Context

Carrot combines direct answers, ReAct, Plan-and-Execute and Reflection while supporting tool side effects, process recovery, pause/resume and messages submitted during an active run. Treating these as independent loops or recovering from a mutable step index would produce ambiguous execution state and unsafe tool retries.

## Decision

1. Carrot has one durable Run Runtime. ReAct is the step explorer, Plan organizes complex work, and Reflection is a bounded quality gate. Product strategies (`fast`, `auto`, `quality`) select a composition of these policies.
2. `AgentOrchestrator` is the sole logical writer that advances a Run. Concurrent requests use an expected version and a runtime lease to prevent duplicate workers.
3. Lifecycle status and execution phase are separate domain values. Pause is a protocol from `pause_requested` to a verified safe point and then `paused`.
4. User input, mode selection, plan revisions, tool execution intent, tool observations and terminal state transitions are committed before the Runtime acknowledges or continues. Token chunks, heartbeats and derived snapshots may be coalesced.
5. The durable truth consists of normalized records plus append-only `run_events`. Snapshots are rebuildable caches identified by an event high-water mark.
6. Input submitted during a run enters a durable inbox with one of `append`, `fork` or `cancel_and_replace`. It is acknowledged only after commit.
7. Tool recovery depends on declared cancellation, idempotency and reconciliation capabilities. An `executing` record after a crash means the result is unknown; non-reconcilable side effects require user resolution.
8. Dropping a Future is not treated as cancellation of external work. Every Future used in `tokio::select!` must have an explicit cancellation-safety review. Blocking work has separate completion semantics.
9. Raw hidden chain-of-thought is neither persisted nor exposed. The Runtime stores replayable input/output items, tool traces, state events, deterministic validation results and concise decision summaries.
10. Panic hooks are diagnostic only. Recovery correctness comes from committed state, event sequence, leases and tool reconciliation.
11. P1 creates the durable schema foundation for `runs`, `items`, `run_events` and `pending_inputs`, including stable IDs and sequence constraints. P3/P4 add orchestration behavior through later migrations without repurposing conversation fields.

## Consequences

- Conversation Provider and model values are defaults for new Runs, not execution history. Every Run records its own immutable Provider profile snapshot and model.
- Exactly-once execution is not promised for arbitrary external side effects.
- Frontend durable events originate only from committed `run_events`; transient token events use a separate sequence.
- Fault-injection tests are required around dispatch, observation commit, pause races and lease takeover.
- P1 carries additional schema tables before their full application behavior is exposed, preventing unstable identifiers and destructive schema changes later.

## References

- [Agent Runtime modes and resilience design](../agent-runtime-modes-and-resilience.md)
- [Tokio `select!` cancellation safety](https://docs.rs/tokio/latest/tokio/macro.select.html#cancellation-safety)
- [Rust `std::panic::set_hook`](https://doc.rust-lang.org/std/panic/fn.set_hook.html)
- [Tauri plugins](https://v2.tauri.app/plugin/)
