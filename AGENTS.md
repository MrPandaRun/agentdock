# Repository Guidelines

## Product & Architecture Snapshot
- Product is **AgentDock**: a local-first multi-agent control plane.
- MVP provider scope is fixed to `codex`, `claude_code`, and `opencode`.
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
- `crates/provider-codex`: Codex adapter implementation.
- `crates/provider-claude`: Claude adapter implementation.
- `crates/provider-opencode`: OpenCode adapter implementation.
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
- `ProviderId` values must remain `codex`, `claude_code`, and `opencode` unless product scope changes explicitly.
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
- `bun run lint`: ESLint check across workspaces.
- `bun run typecheck`: TS workspace typecheck + `cargo check --workspace`.
- `bun run test`: JS tests + `cargo test --workspace`.
- Focused workspace runs use filters, for example:
  - `bun run --filter @agentdock/contracts test`
  - `bun run --filter @agentdock/desktop typecheck`

### Single Test Commands
- **TypeScript/Vitest**: `bun run --filter <package> vitest run -t "test name"` or `vitest run path/to/file.test.ts`
- **Rust/Cargo**: `cargo test -p <crate_name> -- <test_name_substring>`
- Example: `cargo test -p provider-codex -- list_threads_reads_codex_sessions`

## Style & Naming

### General Formatting
- Follow `.editorconfig` (UTF-8, LF, final newline, trim trailing whitespace).
- No Prettier configured—rely on `.editorconfig` and IDE formatting.
- Indentation: 2 spaces for TS/JS/JSON/MD/YAML, 4 spaces for Rust.

### TypeScript/TSX
- **Import Order**: Group imports separated by blank lines:
  1. External libraries (third-party packages)
  2. Internal aliases using `@/` (e.g., `@/lib/utils`, `@/components/ui/button`)
  3. Local relative imports (same/nearby directory)
- Use `import type { ... }` for type-only imports.
- **Types**: `interface` for object shapes and props; `type` for unions, aliases, or mapped types.
- **Components**: PascalCase files, props interface named `ComponentProps`, default export for main components.
- **Error Handling**: `try/catch` with `unknown` error type, convert to string via `instanceof Error`.
- Test files: `*.test.ts` in `tests/` directories, use Vitest (`describe`, `test`, `expect`).
- Type checking strict mode enabled: `noUnusedLocals`, `noUnusedParameters`, `noFallthroughCasesInSwitch`.
- Never use `as any`, `@ts-ignore`, or `@ts-expect-error`.

### Rust
- **Naming**: `snake_case` for functions/modules, `PascalCase` for types/enums.
- **Error Handling**: Use `thiserror` crate for custom errors, define `pub type CrateResult<T> = Result<T, CrateError>`.
- **Serialization**: Annotate with `#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]` and `#[serde(rename_all = "snake_case")]` for TS interop.
- **Testing**: Use `#[cfg(test)]` modules with `#[test]` attributes in source files.
- **Traits**: Define core abstractions (e.g., `ProviderAdapter`) for multiple implementations.

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
