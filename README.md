# AgentDock

Local-first control plane for coding agents across `codex`, `claude_code`, and `opencode`.

[English](./README.md) | [简体中文](./README.zh-CN.md)

AgentDock helps you inspect, resume, and switch agent threads from one desktop workspace without replacing the upstream CLIs.

## Why AgentDock

- Keep multi-provider agent workflows in one place instead of jumping between separate histories.
- Preserve provider-native session data as source of truth while maintaining a local unified index.
- Provide a shared contract layer so desktop, mobile, and adapters stay semantically aligned.
- Build local-first by default: your runtime, SQLite state, and CLI integrations stay on your machine.
- Optimize for real coding loops: reconnect quickly, find old threads, and continue execution with context.

## Feature Snapshot

| Capability | Status | Notes |
| --- | --- | --- |
| Provider scope (`codex`, `claude_code`, `opencode`) | Now | Reflected in TS and Rust contracts. |
| Local-first desktop runtime (Tauri + React) | Now | Host runtime in Rust, UI in React/Vite. |
| Unified thread listing and resume foundations | Now | Provider adapters expose list/resume entry points. |
| Cross-provider switch context orchestration | In Progress | Summary + fallback flow is part of Phase 1 goals. |
| Mobile remote-control workflows | Planned | Expo shell exists; full loop is not Phase 1 scope. |

## Quick Start

From the repository root:

```bash
bun install
```

Run desktop:

```bash
bun run dev:desktop
```

Run mobile:

```bash
bun run dev:mobile
```

If you only need the desktop web UI (without Tauri host):

```bash
bun run dev
```

## Prerequisites

- Bun `1.1.27+` (workspace package manager).
- Rust stable toolchain (see `rust-toolchain.toml`).
- Platform dependencies required by Tauri v2 and Expo.
- Provider CLIs installed and reachable in `PATH` for the providers you want to use:
  - `codex`
  - `claude` (for `claude_code`)
  - `opencode`

Optional environment overrides used by adapters:

| Variable | Purpose |
| --- | --- |
| `AGENTDOCK_CODEX_HOME_DIR` | Override Codex sessions directory root. |
| `AGENTDOCK_CLAUDE_CONFIG_DIR` | Override Claude config directory root. |
| `AGENTDOCK_CLAUDE_BIN` | Override Claude CLI binary name/path. |
| `AGENTDOCK_OPENCODE_DATA_DIR` | Override OpenCode data directory root. |
| `AGENTDOCK_OPENCODE_BIN` | Override OpenCode CLI binary name/path. |

## Development Commands

Run these from the repository root:

| Command | Purpose |
| --- | --- |
| `bun run dev:desktop` | Start the desktop app with Tauri. |
| `bun run dev:mobile` | Start the Expo dev server for mobile. |
| `bun run dev` | Start desktop web UI only (Vite). |
| `bun run build` | Run build scripts in all workspaces that define one. |
| `bun run lint` | Run workspace lint/type-style checks. |
| `bun run typecheck` | Run TypeScript checks and `cargo check --workspace`. |
| `bun run test` | Run workspace tests and `cargo test --workspace`. |

Focused examples:

```bash
bun run --filter @agentdock/contracts test
bun run --filter @agentdock/desktop typecheck
cargo test -p provider-codex -- list_threads_reads_codex_sessions
```

## Monorepo Layout

```text
apps/
  desktop/            Tauri desktop app (React UI + Rust host)
  mobile/             Expo mobile shell
packages/
  contracts/          Shared TypeScript provider contracts
  config-typescript/  Shared tsconfig presets
  config-eslint/      Shared ESLint flat config preset
crates/
  provider-contract/  Shared Rust provider contract + trait
  provider-codex/     Codex provider adapter
  provider-claude/    Claude provider adapter
  provider-opencode/  OpenCode provider adapter
  agentdock-core/     SQLite bootstrap and migrations
docs/
  agentdock-phase1-prd.md
```

## Contracts & Provider Scope

Provider IDs are currently fixed to:

```ts
type ProviderId = "codex" | "claude_code" | "opencode";
```

Keep TypeScript and Rust contracts aligned in the same change whenever provider fields, IDs, or error semantics change:

- TypeScript: `packages/contracts/src/provider.ts`
- Rust: `crates/provider-contract/src/lib.rs`

## Roadmap / Current Status

Current project phase: **Alpha / Phase 1**.

Phase 1 intent and acceptance details are documented in:

- [`docs/agentdock-phase1-prd.md`](./docs/agentdock-phase1-prd.md)

Current focus:

- Provider connectivity and health checks.
- Unified thread retrieval and resume flows.
- Cross-provider switching baseline with event logging.
- Stable local migration and contract alignment workflows.

Out of Phase 1 scope:

- Team collaboration and cloud sync.
- Full billing systems.
- Full mobile remote-control production loop.

## Contributing

1. Fork and create a branch for your change.
2. Implement code and tests with the existing contracts/style.
3. Run baseline checks:

```bash
bun run typecheck
bun run test
```

4. Use Conventional Commit messages (for example: `feat(desktop): add provider health panel`).
5. Open a PR with:
   - What changed
   - Why it changed
   - Commands/tests you ran
   - Screenshots for desktop/mobile UI updates

## FAQ

### Does AgentDock replace provider CLIs?

No. AgentDock orchestrates and indexes workflows around provider CLIs; execution still happens through each provider's CLI.

### Which providers are supported right now?

`codex`, `claude_code`, and `opencode`.

### Where is data stored?

AgentDock is local-first. Provider-native session data remains with each CLI, while AgentDock maintains local SQLite-backed indexing and metadata.

### Why are there both TypeScript and Rust contracts?

Desktop/mobile surfaces rely on TypeScript, while the host and adapters use Rust. Contract parity keeps behavior consistent across both runtimes.

### Is mobile production-ready?

Not yet. Mobile support is currently positioned as a remote-control workflow shell, with full production scope outside Phase 1.

## License

Licensed under the MIT License. See [`LICENSE`](./LICENSE).
