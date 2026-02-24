# Project AgentDock

> Historical planning snapshot created on 2026-02-11.
> The errata below is the authoritative interpretation for current implementation.

## Errata (2026-02-24)

1. Provider scope in code is now `codex`, `claude_code`, and `opencode` (not 2 providers).
2. Desktop runtime is terminal-only for execution flow; no in-app message composer/list path.
3. Current shared `ProviderAdapter` contract includes only `health_check`, `list_threads`, and `resume_thread`.
4. Chinese archive docs moved to local ignored workspace `local-docs/`.

## Historical Project Summary

- Product Name: `AgentDock` (Chinese name: `代理坞`)
- Entry: `dock.mrpanda.run`
- Positioning: multi-agent control plane (accounts, configs, MCP, skills, threads, React Native remote)
- Core Promise: `configure once, reuse everywhere; unified threads, fast switching`

## Historical Locked Decisions (2026-02-11)

1. `Yes`: V1 is local-first only, no cloud backup.
2. `Yes`: Thread reuse across providers is allowed (with permissions and audit).
3. `Yes`: V1 supports only 2 providers: `Codex` + `Claude Code`.
4. `Yes`: Open-source the core adapter layer.
5. `Yes`: Mobile remote supports cross-network access (LAN direct + secure relay).

## Scope Snapshot (MVP)

- P0: Provider connection, account/config center, Thread Center
- P1: MCP Registry, Skills Hub, Switcher, RN mobile read-only
- P2: RN mobile allowlisted operations, cross-provider summary auto-generation

## Timeline Snapshot

1. Week 1-2: freeze requirements, data model, low-fidelity prototype
2. Week 3-4: provider integration + account/config
3. Week 5-6: Thread Center
4. Week 7-8: MCP + Skills
5. Week 9-10: Switcher + RN mobile read-only + secure pairing
6. Week 11: RN mobile allowlisted operations + stability fixes
7. Week 12: internal beta release

## Document Map

- Main PRD: [PRD-AgentDock-v1.md](PRD-AgentDock-v1.md)
- Current implementation summary: [docs/agentdock-current-product-summary.md](docs/agentdock-current-product-summary.md)
- Local Chinese archive (git-ignored): `local-docs/`

## Next Actions

1. Produce V1 information architecture wireframes (Dashboard/Threads/Providers/MCP/Skills/Remote).
2. Define provider adapter contracts (I/O, error codes, timeout strategy).
3. Design RN mobile cross-network pairing flow (QR + short-lived token + revocation + relay security policy).
