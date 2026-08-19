# ADR 0001: Application foundation

- Status: Accepted
- Date: 2026-08-18

## Context

Carrot needs a desktop boundary that can safely host LLM provider calls, local tools, persisted execution records, image attachments, and peer synchronization. The first release targets macOS, but domain and infrastructure boundaries must not prevent later Windows and Linux packaging.

## Decision

1. Use Tauri 2 with a Vue 3 and TypeScript WebView. Rust owns provider calls, credentials, tool execution, persistence, and synchronization. Vue owns presentation and short-lived view state.
2. Target macOS first. Platform-specific implementations live behind ports; portable code must not import macOS APIs directly.
3. Use the OpenAI Responses API as the first native provider and send `store: true` in the first release.
4. Support OpenAI-compatible endpoints through Provider adapters. Profiles load from a local TOML configuration file. Secrets are references to OS credential entries, never literal values in the TOML file.
5. Persist complete local execution records even when the remote provider stores responses. Conversation persistence is accessed through a `ConversationStore` port so a network-backed store can be added later.
6. Support file and image attachments. Attachment metadata is a domain type; ingestion and provider encoding are infrastructure responsibilities.
7. Implement initial cross-device synchronization by discovering peers on the local network. Discovery and transport live behind `SyncTransport`; authentication, encryption in transit, conflict handling, and explicit user trust are required before enabling sync.
8. Generate TypeScript commands and DTOs from Rust with Specta/Tauri Specta. Generated bindings are committed and checked in CI.
9. Keep the Tauri capability list minimal. P0 exposes only core capabilities and a health-check command.

## Consequences

- Provider compatibility is explicit rather than assuming all endpoints implement the full OpenAI Responses protocol.
- Remote storage and local storage can coexist; the local record remains the audit and recovery source.
- LAN discovery does not imply automatic trust. Device pairing and encrypted transport are required in the synchronization phase.
- Tauri Specta is currently a release candidate dependency and is pinned exactly. Its upgrade is a deliberate compatibility task.
- Windows and Linux are not validated in P0 CI, but platform-neutral domain ports prevent macOS implementation details from leaking into core logic.

## Deferred decisions

- Exact OS credential implementation;
- LAN discovery protocol and encrypted session protocol;
- attachment size limits and local encryption policy;
- compatibility capability negotiation for custom endpoints;
- network persistence service protocol.
