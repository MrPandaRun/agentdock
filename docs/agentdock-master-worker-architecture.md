# AgentDock Master-Worker Orchestration Architecture (Proposal)

> Updated: 2026-02-24  
> Status: **Proposal document, not current implemented API surface**.

## Important Scope Note

This document describes a future orchestration direction.
It should not be used as a source of truth for current command/contract availability.

Current implementation source of truth:
- `packages/contracts/src/provider.ts`
- `crates/provider-contract/src/lib.rs`
- `apps/desktop/src-tauri/src/commands.rs`

## 1. Architecture Philosophy

AgentDock can evolve toward a filesystem-driven Master-Worker Agent pattern:

- Local workspace acts as durable context.
- A master agent decomposes and dispatches work.
- Worker agents execute focused tasks.

## 2. Directory-Based Control Plane (`.agentdock`)

Potential layout:

```text
.agentdock/
  ├── agent.md
  ├── soul.md
  ├── skills/
  │   ├── code_review.py
  │   └── manage_threads.sh
  └── state/
      ├── active_workers.json
      └── history/
```

## 3. Proposed Scheduling Flows

### 3.1 Spawn New Worker Thread (Proposed)

Example tool payload:

```json
{
  "tool": "spawn_thread",
  "args": {
    "task_name": "frontend_settings_impl",
    "provider": "codex",
    "prompt": "Create a UserSettings page and tests.",
    "working_dir": "./src/settings"
  }
}
```

### 3.2 Resume Existing Worker Thread (Proposed)

Example tool payload:

```json
{
  "tool": "resume_thread",
  "args": {
    "thread_id": "thread-xyz-789",
    "additional_prompt": "Fix missing db_mock and rerun tests."
  }
}
```

## 4. Compatibility with Current Baseline

This proposal is compatible with current foundations, but not fully implemented:

1. Shared contract alignment exists for three providers (`codex`, `claude_code`, `opencode`).
2. SQLite already exists as a central state foundation.
3. Desktop terminal runtime can be used as execution substrate.

## 5. Non-Implemented Parts

The following are proposal-only in this document:

- `spawn_thread` / orchestrator tool surface
- Parent-child thread orchestration lifecycle APIs
- Full master-worker scheduling runtime in AgentDock host

Use `docs/agentdock-current-product-summary.md` for current behavior.
