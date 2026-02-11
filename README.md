# AgentDock

AgentDock is a local-first control plane for coding agents (`Codex`, `Claude Code` in V1).

## Monorepo Layout

- `apps/desktop`: Tauri + React + TypeScript desktop app
- `apps/mobile`: Expo + React Native + TypeScript mobile app
- `packages/contracts`: shared TypeScript contracts
- `packages/config-typescript`: shared TypeScript config presets
- `packages/config-eslint`: shared ESLint config presets
- `crates/provider-contract`: Rust provider adapter contract
- `crates/provider-codex`: Codex provider adapter stub
- `crates/provider-claude`: Claude provider adapter stub
- `crates/agentdock-core`: core Rust domain and SQLite migration runner

## Quick Start

```bash
bun install
bun run dev:desktop
```

```bash
bun run dev:mobile
```

## Verification

```bash
bun run --filter @agentdock/contracts typecheck
cargo test --workspace --manifest-path ./Cargo.toml
```
