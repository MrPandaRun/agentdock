# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

AgentDock is a local-first multi-agent control plane. V1 provider scope is fixed to `codex` and `claude_code`.

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
- `crates/provider-codex`: Codex adapter stub
- `crates/provider-claude`: Claude adapter stub
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
- `bun run lint` - Run `lint` script across all workspaces (typecheck only currently)

### Workspace-Specific Runs
Use `bun run --filter <workspace>` for focused operations:
- `bun run --filter @agentdock/contracts test` - Run contract package tests
- `bun run --filter @agentdock/contracts typecheck` - Typecheck contract package
- `bun run --filter @agentdock/desktop typecheck` - Typecheck desktop app only
- `bun run --filter @agentdock/mobile typecheck` - Typecheck mobile app only

### Rust-Specific
- `bun run rust:check` - `cargo check --workspace --manifest-path ./Cargo.toml`
- `bun run rust:test` - `cargo test --workspace --manifest-path ./Cargo.toml`

## Architecture

### Monorepo Structure
- **apps/desktop** - Tauri desktop app: `apps/desktop/src` (React/Vite UI) + `apps/desktop/src-tauri` (Rust host)
- **apps/mobile** - Expo React Native app for remote control workflows
- **packages/contracts** - TypeScript contracts shared between frontend and backend
- **packages/config-typescript** - Shared tsconfig presets
- **packages/config-eslint** - Shared ESLint flat config presets
- **crates/provider-contract** - Rust `ProviderAdapter` trait and shared types
- **crates/provider-codex** - Codex provider adapter (returns `NotImplemented` for now)
- **crates/provider-claude** - Claude Code provider adapter (returns `NotImplemented` for now)
- **crates/agentdock-core** - Core domain logic with SQLite bootstrap and migrations

### Provider Adapter Pattern
The `ProviderAdapter` trait (`crates/provider-contract/src/lib.rs`) defines the interface for all AI coding agent providers. Implementations in `provider-codex` and `provider-claude` currently return `NotImplemented` errors. The trait includes:
- `health_check()` - Provider health validation
- `list_threads()` - Enumerate threads, optionally filtered by project path
- `resume_thread()` - Resume a thread with optional context
- `summarize_switch_context()` - Generate context summary for thread switching

### Contract Synchronization
TypeScript and Rust contracts must stay semantically aligned. If modifying provider contracts, update both:
- `packages/contracts/src/provider.ts` (TypeScript)
- `crates/provider-contract/src/lib.rs` (Rust)

Provider ID values are fixed to `"codex"` and `"claude_code"` unless product scope changes.

### Database
- SQLite managed via `crates/agentdock-core/src/db/mod.rs`
- Migrations stored in `crates/agentdock-core/migrations/*.sql`
- Migration policy is **append-only** - never modify applied migrations
- Migration runner is idempotent (`run_migrations` can run multiple times safely)
- Desktop app initializes DB on startup in app data dir (`agentdock.db`) via `agentdock_core::db::init_db`

## Agent Change Rules

- If you change provider contract fields or error codes, update TS and Rust contract files in the same change.
- Keep adapter stubs (`provider-codex`, `provider-claude`) returning standardized `NotImplemented` unless implementing real provider integration.
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
