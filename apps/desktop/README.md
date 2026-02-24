# AgentDock Desktop

Desktop runtime for AgentDock, built with Tauri (Rust host) + React/Vite (UI).

Current desktop execution model is terminal-only (embedded PTY + terminal launch).

For product overview, provider scope, and contribution guidelines:

- Root README (EN): [`../../README.md`](../../README.md)
- Root README (中文): [`../../README.zh-CN.md`](../../README.zh-CN.md)

## Local Development

Run from the repository root:

```bash
bun run dev:desktop
```

Or run package-scoped commands:

```bash
bun run --filter @agentdock/desktop dev
bun run --filter @agentdock/desktop typecheck
bun run --filter @agentdock/desktop test
```

## Key Paths

- UI source: `src/`
- Tauri host: `src-tauri/`
- Package config: `package.json`

## Notes

- JavaScript workspace operations use Bun.
- Tauri platform dependencies must be installed for your OS.
