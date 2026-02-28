# AgentDock

Local-first control plane for coding agents across `codex`, `claude_code`, and `opencode`.

[English](./README.md) | [简体中文](./README.zh-CN.md)

AgentDock helps you inspect and resume agent-native threads from one desktop workspace without replacing upstream CLIs.

## Why AgentDock

- Keep multi-agent coding workflows in one place instead of jumping across CLI history stores.
- Preserve agent-native thread data as the source of truth while maintaining local unified indexing.
- Keep TS and Rust contracts aligned so desktop/mobile surfaces and adapters share consistent semantics.
- Run local-first by default: runtime, SQLite state, and CLI integrations stay on your machine.

## Terminology (Canonical)

- Project: folder-level grouping in the left sidebar.
- Thread: one interaction unit shown in UI.
- Agent: primary execution carrier (`codex` / `claude_code` / `opencode`).
- Model Provider: model vendor used by an agent run (for example OpenAI, Anthropic, OpenRouter).

## Feature Snapshot

| Capability | Status | Notes |
| --- | --- | --- |
| Agent scope (`codex`, `claude_code`, `opencode`) | Now | Reflected in TS and Rust contracts (as Provider IDs). |
| Local-first desktop runtime (Tauri + React) | Now | Rust host + React/Vite UI. |
| Unified thread listing + resume | Now | Three adapters expose thread scan + resume command path. |
| Desktop execution mode | Now | Terminal-only (embedded PTY + terminal launch). |
| Cross-agent summary orchestration | Planned | Not part of current `ProviderAdapter` contract surface. |
| Mobile remote-control workflows | Planned | Expo shell exists; full remote-control loop is not complete. |

## Desktop Behavior (Current)

- Sidebar groups threads by project folder.
- Thread records include `title` and optional `lastMessagePreview`.
- Sidebar item text prefers `title`; if empty, it falls back to `lastMessagePreview`.
- Header title displays selected thread `title`.
- Create New Thread dialog checks per-agent installation status and shows install guidance when a CLI is missing.
- Thread title strategy in adapters prioritizes agent-official titles, then user-input fallback.

## Quick Start

From repository root:

```bash
bun install
bun run dev:desktop
```

Optional:

```bash
bun run dev:mobile
bun run dev
```

## Prerequisites

- Bun `1.1.27+`
- Rust stable toolchain (see `rust-toolchain.toml`)
- Platform dependencies required by Tauri v2 and Expo
- Agent CLIs in `PATH`:
  - `codex`
  - `claude` (for `claude_code`)
  - `opencode`

Optional environment overrides:

| Variable | Purpose |
| --- | --- |
| `AGENTDOCK_CODEX_HOME_DIR` | Override Codex sessions directory root. |
| `AGENTDOCK_CLAUDE_CONFIG_DIR` | Override Claude config directory root. |
| `AGENTDOCK_CLAUDE_BIN` | Override Claude CLI binary name/path. |
| `AGENTDOCK_OPENCODE_DATA_DIR` | Override OpenCode data directory root. |
| `AGENTDOCK_OPENCODE_BIN` | Override OpenCode CLI binary name/path. |

## Development Commands

| Command | Purpose |
| --- | --- |
| `bun run dev:desktop` | Start desktop app with Tauri. |
| `bun run dev:mobile` | Start Expo dev server for mobile. |
| `bun run dev` | Start desktop web UI only (Vite). |
| `bun run build` | Run build scripts in workspaces that define one. |
| `bun run lint` | Run workspace lint checks. |
| `bun run typecheck` | Run TS checks + `cargo check --workspace`. |
| `bun run test` | Run workspace tests + `cargo test --workspace`. |

Focused examples:

```bash
bun run --filter @agentdock/contracts test
bun run --filter @agentdock/desktop typecheck
cargo test -p provider-codex -- list_threads_reads_codex_sessions
```

## Contracts

Provider IDs are fixed to:

```ts
type ProviderId = "codex" | "claude_code" | "opencode";
```

Shared provider contract files:
- TS: `packages/contracts/src/provider.ts`
- Rust: `crates/provider-contract/src/lib.rs`

`ProviderAdapter` currently includes:
- `health_check`
- `list_threads`
- `resume_thread`

## Documentation Map

- Current implementation summary: [`docs/agentdock-current-product-summary.md`](./docs/agentdock-current-product-summary.md)
- Current Phase 1 execution spec: [`docs/agentdock-phase1-prd.md`](./docs/agentdock-phase1-prd.md)
- Historical planning docs with errata:
  - [`Project-AgentDock.md`](./Project-AgentDock.md)
  - [`PRD-AgentDock-v1.md`](./PRD-AgentDock-v1.md)
  - [`Architecture-AgentDock-v1.md`](./Architecture-AgentDock-v1.md)

## Contributing

1. Create a branch.
2. Implement changes with tests.
3. Run:

```bash
bun run typecheck
bun run test
```

4. Use Conventional Commits.
5. Include change summary, reasons, commands/tests, and UI screenshots (if applicable).

## License

MIT. See [`LICENSE`](./LICENSE).
