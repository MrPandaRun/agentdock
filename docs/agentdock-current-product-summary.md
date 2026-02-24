# AgentDock Current Product Summary (Implementation-Aligned)

> Updated: 2026-02-24  
> Based on code in: `apps/desktop`, `apps/desktop/src-tauri`, `crates/*`, `packages/contracts`

## 1. Product Positioning

AgentDock is a local-first multi-agent control console for CLI-based coding workflows.

Supported providers:
- `codex`
- `claude_code`
- `opencode`

Core boundary:
- AgentDock does not replace provider-native session engines.
- Session execution and continuation still happen through provider CLIs.

## 2. Current User Value

- Unified historical thread visibility from three providers.
- Folder-grouped navigation and quick thread switching in desktop UI.
- Embedded terminal continuation for selected threads.
- New-thread launch entry per folder/provider from sidebar UI.

## 3. Capability Inventory

### 3.1 Shared Contract Layer

- TS: `packages/contracts/src/provider.ts`
- Rust: `crates/provider-contract/src/lib.rs`

Aligned provider IDs:
- `codex`
- `claude_code`
- `opencode`

Current shared adapter method surface:
- `health_check`
- `list_threads`
- `resume_thread`

### 3.2 Adapter Implementations

- `provider-codex`
  - Reads from `~/.codex/sessions`
  - Uses official title map from `~/.codex/.codex-global-state.json`
  - Resume command path: `codex resume <thread_id>`
- `provider-claude`
  - Reads from `~/.claude/projects`
  - Uses official history display title from `~/.claude/history.jsonl`
  - Resume command path: `claude --resume <thread_id>`
- `provider-opencode`
  - Reads from `~/.local/share/opencode/storage`
  - Prefers session `title`
  - Resume command path: `opencode --session <thread_id>`

All three adapters expose runtime-state reading used by desktop terminal lifecycle decisions.

### 3.3 Desktop UI and Title/Preview Rules

- Left panel: folder-grouped thread list.
- Right panel: embedded terminal (terminal-only mode).

Thread text behavior:
- Backend returns `title` + optional `lastMessagePreview`.
- Sidebar item text (`threadPreview`) uses `title` first.
- If `title` is empty, sidebar falls back to `lastMessagePreview`.
- Header title uses selected thread `title`.

Consistency intent:
- Provider adapters should produce stable, provider-official `title` whenever available.
- Sidebar/header should converge on the same canonical title for normal threads.

### 3.4 Tauri Host Command Surface

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

## 4. Data Layer

`agentdock-core` initializes SQLite and runs append-only migrations on startup.

Current baseline tables include:
- `providers`
- `accounts`
- `configs`
- `mcps`
- `skills`
- `threads`
- `thread_messages`
- `switch_events`
- `remote_devices`
- `remote_sessions`

## 5. Stage Assessment

### Completed

- Three-provider contract alignment (TS/Rust)
- Three-provider thread scanning + resume command path
- Desktop terminal-first continuation flow
- Local DB initialization + migration baseline

### In Progress

- Desktop interaction refinement (window drag/layout details)
- Productized cross-provider switch orchestration strategy

### Not Complete Yet

- End-to-end mobile remote control loop
- Collaboration/cloud capabilities
- Governance/billing systems

## 6. Key Constraints

- Desktop execution flow is terminal-only.
- No current shared API for summary-based cross-provider switch orchestration.
- No in-app message composer/list send flow in current desktop build.
