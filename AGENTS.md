# Repository Guidelines

## Product & Architecture Snapshot
- Product is **AgentDock**: a local-first multi-agent control plane.
- MVP provider scope is fixed to `codex` and `claude_code`.
- Desktop runtime is split between `apps/desktop/src` (React + Vite) and `apps/desktop/src-tauri` (Rust host).
- Mobile runtime is `apps/mobile` (Expo + React Native) for remote control workflows.
- Shared provider/thread contracts live in both `packages/contracts` (TypeScript) and `crates/provider-contract` (Rust).
- Core data logic lives in `crates/agentdock-core` with SQLite migrations under `crates/agentdock-core/migrations`.

## Monorepo Layout
- `apps/desktop`: Tauri desktop app with web UI and Rust entrypoint.
- `apps/mobile`: Expo app with mobile shell and assets.
- `packages/contracts`: TS contract source + Vitest tests.
- `packages/config-typescript`: shared tsconfig presets.
- `packages/config-eslint`: shared ESLint flat config preset.
- `crates/provider-contract`: Rust contract types and `ProviderAdapter` trait.
- `crates/provider-codex`: Codex adapter stub.
- `crates/provider-claude`: Claude adapter stub.
- `crates/agentdock-core`: DB bootstrap and migration runner.

## Package Manager & Toolchain
- Use **Bun only** for JS workspace operations.
- Do not add back `pnpm-workspace.yaml` or `pnpm-lock.yaml`.
- Root lockfile is `bun.lockb`.
- Rust toolchain is pinned by `rust-toolchain.toml` (`stable`).

## Contract & Data Rules
- Keep TS and Rust contracts semantically aligned:
  - TS: `packages/contracts/src/provider.ts`
  - Rust: `crates/provider-contract/src/lib.rs`
- If you add/remove/rename provider fields or error codes, update both sides in the same change.
- `ProviderId` values must remain `codex` and `claude_code` unless product scope changes explicitly.
- Adapter crates (`provider-codex`, `provider-claude`) should return standardized `NotImplemented` until real implementations are added.
- SQLite migration policy is **append-only**:
  - Never rewrite an already-applied migration.
  - Add a new migration file and wire it through migration execution.
  - Preserve idempotency (`run_migrations` can run multiple times safely).

## Commands
- `bun install`: install all JS dependencies.
- `bun run dev:desktop`: run Tauri desktop app.
- `bun run dev:mobile`: run Expo dev server.
- `bun run dev`: run desktop web UI (Vite only).
- `bun run build`: run `build` in all workspaces that define it.
- `bun run typecheck`: TS workspace typecheck + `cargo check --workspace`.
- `bun run test`: JS tests + `cargo test --workspace`.
- Focused workspace runs use filters, for example:
  - `bun run --filter @agentdock/contracts test`
  - `bun run --filter @agentdock/desktop typecheck`

## Style & Naming
- Follow `.editorconfig` (UTF-8, LF, final newline, trim trailing whitespace).
- Indentation: 2 spaces for TS/JS/JSON/MD/YAML, 4 spaces for Rust.
- TS strictness is expected; avoid introducing unused symbols and implicit weak typing.
- Naming:
  - React components: PascalCase (`App.tsx`)
  - TS modules: lower-case filenames (`provider.ts`)
  - Tests: `*.test.ts`
  - Rust functions/modules: snake_case; types/enums: PascalCase

## Testing Expectations
- For contract changes, run:
  - `bun run --filter @agentdock/contracts test`
  - `bun run --filter @agentdock/contracts typecheck`
  - `cargo test --workspace --manifest-path ./Cargo.toml`
- For migration changes, ensure rust tests in `crates/agentdock-core/src/db/mod.rs` still pass.
- Before opening PR, run at least:
  - `bun run typecheck`
  - `bun run test`

## Commit & PR Conventions
- Use Conventional Commits (`feat`, `fix`, `chore`, etc.).
- Prefer `type(scope): summary`, example: `feat(desktop): add provider health panel`.
- PR description should include:
  - What changed
  - Why
  - Commands/tests run
  - Screenshots for `apps/desktop` or `apps/mobile` UI changes
