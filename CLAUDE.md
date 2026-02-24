# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

AgentDock is a local-first multi-agent control plane.

Current provider scope is fixed to:
- `codex`
- `claude_code`
- `opencode`

Current stack:
- Desktop: Tauri (`apps/desktop/src-tauri`) + React/Vite (`apps/desktop/src`)
- Mobile: Expo React Native (`apps/mobile`)
- Shared TS contracts: `packages/contracts`
- Rust workspace crates: `crates/*`
- SQLite core + migrations: `crates/agentdock-core`

## Monorepo Structure

- `apps/desktop`: desktop UI + Tauri host
- `apps/mobile`: mobile remote shell
- `packages/contracts`: provider/thread TypeScript contracts + Vitest tests
- `packages/config-typescript`: shared TS config presets
- `packages/config-eslint`: shared ESLint flat config
- `crates/provider-contract`: Rust contract types and `ProviderAdapter` trait
- `crates/provider-codex`: Codex adapter implementation
- `crates/provider-claude`: Claude adapter implementation
- `crates/provider-opencode`: OpenCode adapter implementation
- `crates/agentdock-core`: DB bootstrap + migration runner

## Package Manager & Toolchain

- JS workspace manager is **Bun only**.
- Root lockfile is `bun.lockb`.
- Do not introduce `pnpm-workspace.yaml` or `pnpm-lock.yaml`.
- Rust toolchain is controlled by `rust-toolchain.toml` (currently `stable`).

## Commands

### Development
- `bun run dev:desktop` - Run Tauri desktop app (Vite + Rust)
- `bun run dev` - Run desktop web UI only (Vite)
- `bun run dev:mobile` - Run Expo dev server for mobile app

### Build & Verify
- `bun run build` - Run `build` script across all workspaces
- `bun run typecheck` - TypeScript typecheck across JS workspaces + `cargo check --workspace`
- `bun run test` - JS tests (Vitest) + `cargo test --workspace`
- `bun run lint` - Run `lint` script across all workspaces

### Workspace-Specific Runs
Use `bun run --filter <workspace>` for focused operations:
- `bun run --filter @agentdock/contracts test`
- `bun run --filter @agentdock/contracts typecheck`
- `bun run --filter @agentdock/desktop typecheck`
- `bun run --filter @agentdock/mobile typecheck`

### Rust-Specific
- `bun run rust:check` - `cargo check --workspace --manifest-path ./Cargo.toml`
- `bun run rust:test` - `cargo test --workspace --manifest-path ./Cargo.toml`

## Architecture Notes

### Contract Synchronization
TypeScript and Rust contracts must stay semantically aligned. If modifying provider contracts, update both:
- `packages/contracts/src/provider.ts` (TypeScript)
- `crates/provider-contract/src/lib.rs` (Rust)

`ProviderId` values are fixed to `"codex"`, `"claude_code"`, and `"opencode"` unless product scope changes.

### Provider Adapter Pattern
The `ProviderAdapter` trait (`crates/provider-contract/src/lib.rs`) currently includes:
- `health_check()`
- `list_threads()`
- `resume_thread()`

Do not document or depend on removed switch-summary trait methods unless they are reintroduced in code.

### Desktop Runtime Boundary (Current)
Desktop is terminal-first and terminal-only for thread execution flow:
- Left panel: folder-grouped thread list
- Right panel: embedded terminal session
- No in-app message composer/message list flow

Current Tauri command surface includes:
- `list_threads`
- `get_claude_thread_runtime_state`
- `get_codex_thread_runtime_state`
- `get_opencode_thread_runtime_state`
- terminal launch + embedded terminal commands (`open_*`, `start_*`, `write_*`, `resize_*`, `close_*`)

## Database

- SQLite managed via `crates/agentdock-core/src/db/mod.rs`
- Migrations stored in `crates/agentdock-core/migrations/*.sql`
- Migration policy is **append-only** - never modify applied migrations
- Migration runner is idempotent (`run_migrations` can run multiple times safely)
- Desktop app initializes DB on startup in app data dir (`agentdock.db`) via `agentdock_core::db::init_db`

## Agent Change Rules

- If you change provider contract fields or error codes, update TS and Rust contract files in the same change.
- Adapters are real integrations; unsupported branches should return standardized provider errors.
- For schema changes, add a new migration file and ensure existing migration tests still pass.
- Prefer making verification part of every change:
  - `bun run typecheck`
  - `bun run test`

## Style & Conventions

- Follow `.editorconfig` (UTF-8, LF, final newline, trim trailing whitespace)
- Indentation: 2 spaces for TS/JS/JSON/MD/YAML, 4 spaces for Rust
- React components: PascalCase filenames (`App.tsx`)
- TS modules: lower-case filenames (`provider.ts`)
- Tests: `*.test.ts` for Vitest, `#[test]` for Rust
- Rust functions/modules: snake_case; types/enums: PascalCase
