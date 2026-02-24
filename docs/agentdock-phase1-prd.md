# Product Requirements Document: AgentDock Phase 1 (Execution Baseline)

**Version**: 1.1  
**Date**: 2026-02-24  
**Owner**: Product / Engineering

---

## 1. Executive Summary

This document is the current execution baseline for Phase 1 delivery.

Phase 1 in code currently targets:
- Agent scope (Provider IDs): `codex`, `claude_code`, `opencode`
- Desktop-first local runtime (Tauri host + React UI)
- Terminal-only thread continuation flow
- Unified thread retrieval and resume command generation via provider adapters

AgentDock does not replace agent-native CLIs. Agent-native thread stores remain the source of truth.

### 1.1 Canonical Terminology (UI/Product)

- Project: folder-level grouping in the left sidebar.
- Thread: one interaction unit shown in UI.
- Agent: primary execution carrier (`codex` / `claude_code` / `opencode`).
- Model Provider: model vendor used by an agent run (for example OpenAI, Anthropic, OpenRouter).

---

## 2. Scope

### 2.1 In Scope (Current)

- Agent health checks and thread retrieval for all three agents.
- Unified thread listing in desktop app, grouped by project folder.
- Resume command path through shared provider adapter contracts.
- Runtime-state querying per agent for terminal lifecycle handling.
- Local SQLite initialization and append-only migration policy.

### 2.2 Out of Scope (Current)

- Full mobile remote-control loop.
- Team collaboration and cloud sync.
- Billing and policy engine.
- Productized cross-agent context-summary orchestration API.

---

## 3. Product Behavior Baseline

### 3.1 Desktop Interaction Model

- Sidebar: folder-grouped thread list.
- Main panel: embedded terminal session.
- No in-app message composer/list send flow.

### 3.2 Thread Display Rules

- Thread payload includes `title` and optional `lastMessagePreview`.
- Sidebar item text prefers `title`; fallback is `lastMessagePreview`.
- Header uses selected thread `title`.

### 3.3 Thread Title Source Priority (By Agent)

- Codex: official title map from `~/.codex/.codex-global-state.json`, then fallback.
- Claude: official display title signal in `~/.claude/history.jsonl`, then fallback.
- OpenCode: session `title`, then fallback.

---

## 4. Contract Requirements

### 4.1 Provider IDs

Must remain:
- `codex`
- `claude_code`
- `opencode`

### 4.2 Shared Contract Methods (Current)

Both TS and Rust contract layers must align on:
- `health_check`
- `list_threads`
- `resume_thread`

Current contract files:
- TS: `packages/contracts/src/provider.ts`
- Rust: `crates/provider-contract/src/lib.rs`

### 4.3 Non-Goals for Current Contract

- No switch-summary method in the current shared trait/interface.
- No UI-send message API in current desktop command surface.

---

## 5. Functional Requirements

## FR-01 Provider Adapter Execution

- Adapters must read provider-native thread metadata.
- Adapters must generate resume command guidance for terminal continuation.
- Adapter errors must map to shared error codes.

Acceptance:
- Thread listing succeeds for all three agents when local data exists.
- Resume command payload is generated for selected thread.

## FR-02 Thread Aggregation in Desktop Host

- Tauri host must aggregate thread overviews from Claude, Codex, and OpenCode adapters.
- Aggregated list must be sorted by latest activity.

Acceptance:
- `list_threads` returns combined and sorted entries.

## FR-03 Runtime State Query

- Tauri host must expose runtime-state query commands per agent.

Acceptance:
- `get_claude_thread_runtime_state`, `get_codex_thread_runtime_state`, `get_opencode_thread_runtime_state` are available and callable.

## FR-04 Terminal Session Lifecycle

- Embedded terminal supports start/write/resize/close lifecycle.
- New thread launch and existing thread resume both use terminal command path.

Acceptance:
- User can continue selected thread in embedded terminal.
- Terminal session resize and closure work without app restart.

## FR-05 Data and Migration Baseline

- App startup initializes SQLite and runs migrations safely.
- Migration policy remains append-only and idempotent.

Acceptance:
- Repeated startup does not reapply completed migrations.

---

## 6. Non-Functional Requirements

### 6.1 Reliability

- Critical flows return typed/structured errors across adapter boundaries.
- Listing failure from one agent adapter must be surfaced clearly.

### 6.2 Security

- Credentials are referenced, not stored as plaintext in SQLite.
- External CLI invocations follow least-privilege defaults.

### 6.3 Performance

- Thread listing should remain responsive for daily local usage.
- Terminal session reuse should avoid unnecessary churn when switching selected threads.

---

## 7. Current Public Command Surface (Desktop Host)

From `apps/desktop/src-tauri/src/commands.rs`:

- `list_threads`
- `get_claude_thread_runtime_state`
- `get_codex_thread_runtime_state`
- `get_opencode_thread_runtime_state`
- `open_thread_in_terminal`
- `open_new_thread_in_terminal`
- `start_embedded_terminal`
- `start_new_embedded_terminal`
- `write_embedded_terminal_input`
- `resize_embedded_terminal`
- `close_embedded_terminal`

---

## 8. Verification Checklist

- `bun run typecheck`
- `bun run test`
- Contract parity check between TS and Rust provider files
- Manual desktop validation:
  - list threads from three providers
  - select/resume thread in embedded terminal
  - create new thread launch from sidebar folder menu

---

## 9. Definition of Done (Execution Baseline)

Phase 1 execution baseline is complete when:

- Three-provider thread listing and resume flow is stable in desktop app.
- Terminal-only execution model is documented and consistent with code.
- Shared contracts and docs align on Agent IDs (Provider IDs) and current method surface.
- Migration and startup behavior remain stable and repeatable.
