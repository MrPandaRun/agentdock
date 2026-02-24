# D-20260211-agent-control-plane-prd-v1

> Historical planning document (2026-02-11).
> The errata section below is authoritative for current implementation as of 2026-02-24.

## Historical + Errata (2026-02-24)

| Topic | Historical Text | Current Implementation Truth |
| --- | --- | --- |
| Provider scope | V1 starts with 2 providers (`Codex`, `Claude Code`) | Current code/contract scope is 3 providers: `codex`, `claude_code`, `opencode`. |
| OpenCode status | `opencode` planned for V1.1 | `provider-opencode` is already implemented in code and included in desktop thread list/runtime-state flow. |
| Desktop execution UX | Implies full thread center + switching UX as active scope | Current desktop is terminal-only for execution (`EmbeddedTerminal` + terminal launch commands). |
| Switch orchestration API | Assumes full cross-provider summary orchestration path | Current shared contract exposes `health_check`, `list_threads`, `resume_thread` only; no `summarize_switch_context` method. |
| Adapter maturity | Early-stage adapter planning | Adapters are real implementations (`provider-codex`, `provider-claude`, `provider-opencode`) with runtime-state support exposed by Tauri host commands. |

## Executive Summary

Build a local-first multi-agent console that unifies management of accounts, configurations, MCP, and skills for `Codex`, `Claude Code`, `Gemini`, and `OpenCode`, and provides a Codex Desktop-like thread center plus a mobile remote-control entry.

The product goal is to minimize the cost of "tool switching and fragmented state": users should no longer repeatedly edit configuration, search historical sessions, or reinstall MCP/skills. Instead, they can connect, switch, continue, and reuse everything in one control console; when away from desktop, they can still check progress and perform key operations from mobile.

## Product Positioning

- Product type: developer AI Agent Control Plane
- Core value: `configure once, reuse everywhere; unified threads, fast switching`
- Competitive references: `cc switch` (switching capability) + `Codex Desktop` (thread management UX)
- Differentiation:
  - Not only model/vendor switching, but full working context management (account + config + MCP + skills + thread)
  - Local-first, suitable for individuals and small teams to run workflows first
  - Built for AI coding workflows, not generic chat

## Target Users

1. Heavy AI coding users: use 2-4 coding agents in parallel
2. Technical content creators: need to preserve and reuse large thread context
3. Small-team tech leads: want standardized team tooling and workflows

## Problem Statement

**Current pain points**

- Accounts and keys are scattered across different CLIs/desktop apps
- Configuration formats are inconsistent (model, temperature, path, permissions, env)
- MCP and skills need repeated installation and per-tool version maintenance
- Threads are fragmented across tools, hard to search, resume, and hand over

**Target state**

- Multi-agent initialization within 5 minutes
- Switch from one agent to another with context in 15 seconds
- Unified thread search/resume to reduce repeated problem re-explanation

## MVP Scope (V1)

### In Scope

1. Provider connection and account management (initially `Codex` + `Claude Code`)
2. Configuration center (model, default params, working directory, env templates)
3. MCP Registry (install, start/stop, permission scope, version)
4. Skills Center (import, start/stop, version status, compatibility hints)
5. Thread Center (list, tags, search, resume, jump)
6. One-click switch entry (start from current thread on another provider)
7. Mobile remote control (React Native): view thread status, trigger preset actions, view recent execution summaries

### Out of Scope

1. Team collaboration and cloud sync
2. Mobile Web / PWA remote endpoint
3. Complex billing system
4. Generic chat UI for non-development scenarios

## Core Modules

### M01 Accounts and Profiles

- Multi-account (work/personal) and provider binding
- Credential state checks (valid/expired/missing)
- Local encrypted storage (system keychain)

### M02 Config Control Center

- Provider config templates and override layers (global/project/profile)
- Visual diff comparison
- Fast import/export

### M03 MCP Registry

- MCP server list, startup parameters, permission policies
- Enable by thread or by profile
- Version upgrade hints and rollback

### M04 Skills Hub

- Skills source management (local path/Git)
- Start/stop state and version consistency checks
- Provider compatibility matrix (which skill works with which provider)

### M05 Thread Center

- Unified thread index (provider/time/project/tag/status)
- Full-text search (title/message/tag)
- Thread snapshots (key prompts, dependencies, execution directory)

### M06 Switcher

- Switch provider directly from a thread
- Auto-inject minimal context summary (latest objective, key constraints, pending tasks)
- Fallback on switch failure (prompt only, no environment)

### M07 Mobile Remote (React Native)

- Mobile console access (RN app on iOS/Android)
- Remote visibility: thread list, runtime state, recent log summary
- Remote actions: pause/continue, retry previous step, switch provider (constrained by policy)
- Secure pairing: QR binding + short-lived tokens + revocable device authorization
- Connectivity scope: V1 supports LAN and public network access

## Information Architecture

1. Dashboard
2. Threads
3. Providers
4. MCP
5. Skills
6. Remote
7. Settings

## Technical Architecture (Recommended)

- Desktop shell: `Tauri` (better performance and system permission integration)
- Frontend: `React + TypeScript + Zustand + TanStack Query`
- Mobile: `React Native + Expo + TypeScript`
- Backend runtime layer: `Rust` (process management, filesystem watchers, keychain bridge)
- Communication: `WebSocket` (real-time state sync between desktop and mobile)
- Storage:
  - `SQLite`: structured configs and thread index
  - file index: thread raw log mapping
  - system keychain: sensitive credentials
- Remote connectivity strategy (MVP):
  - prefer LAN direct connection
  - provide secure relay for public-network availability
  - relay forwards encrypted channels only and stores no business data
- Adapter layer:
  - `provider-adapter-codex`
  - `provider-adapter-claude-code`
  - `provider-adapter-gemini` (V1.1)
  - `provider-adapter-opencode` (V1.1)

## Data Model (MVP)

1. `providers(id, name, status, last_checked_at)`
2. `accounts(id, provider_id, profile_name, credential_ref, created_at)`
3. `configs(id, scope, provider_id, account_id, payload_json, updated_at)`
4. `mcps(id, name, command, args_json, scope, enabled, version)`
5. `skills(id, name, source, version, enabled, compatibility_json)`
6. `threads(id, provider_id, account_id, project_path, title, tags_json, last_active_at)`
7. `thread_messages(id, thread_id, role, content, created_at)`
8. `switch_events(id, from_thread_id, to_provider_id, result, created_at)`
9. `remote_devices(id, device_name, paired_at, last_seen_at, revoked_at)`
10. `remote_sessions(id, device_id, thread_id, action, result, created_at)`

## Security and Reliability Baseline

- API keys are never stored in plaintext, only keychain references
- Apply configurable redaction rules before thread import
- Add timeout and logging to all external command execution
- Contract tests for provider adapters (prevent parser breakage due to upstream format changes)
- Remote control starts with least privilege: view + safe allowlisted operations only
- Mobile authorization supports single-device revoke and global invalidate (emergency logout)

## Delivery Roadmap (12 Weeks)

1. Week 1-2: requirement freeze + architecture/data model + low-fidelity prototype
2. Week 3-4: provider connections (Codex/Claude) + account and config center
3. Week 5-6: Thread Center (index/search/resume)
4. Week 7-8: MCP Registry + Skills Hub
5. Week 9-10: Switcher + RN mobile read-only board + secure remote pairing
6. Week 11: RN mobile allowlisted operations + stability fixes + observability
7. Week 12: internal beta release (20-50 seed users)

## Success Metrics (First 60 Days)

1. First-time dual-provider connection <= 10 minutes
2. Thread resume success rate >= 95%
3. Median switching latency <= 8 seconds
4. WAU retention >= 35%
5. Average switches per user per week >= 12
6. Weekly mobile remote usage penetration >= 25%
7. Mobile remote action success rate >= 98%

## Naming Options

| Name | Type | Meaning | Candidate subdomain |
| --- | --- | --- | --- |
| AgentDock | Primary | Dock and scheduling hub for agents | `dock.mrpanda.run` |
| ThreadDock | Primary | Emphasizes thread session center | `threads.mrpanda.run` |
| SwitchForge | Backup | Emphasizes switching and workflow shaping | `switch.mrpanda.run` |
| ContextPort | Backup | Emphasizes context transfer and reuse | `context.mrpanda.run` |
| AgentSwitchboard | Descriptive | Directly describes the control console | `control.mrpanda.run` |
| ModelBridge | Backup | Emphasizes bridging between models/agents | `bridge.mrpanda.run` |

## Recommended Naming

- English product name: `AgentDock`
- Chinese name: `代理坞`
- Slogan: `One dock for all coding agents.`
- Recommended entry: `dock.mrpanda.run`

Reasons:

1. Not limited to "switching"; also covers management and accumulation.
2. Stable pronunciation and semantics, suitable for brand extension (Dock OS / Dock Cloud).
3. Naturally compatible with the thread center concept (thread docking).

## Domain and URL Strategy

1. Primary domain: `mrpanda.run`
2. Product entry: `dock.mrpanda.run`
3. Admin console (optional): `control.mrpanda.run`
4. Docs site (optional): `docs.mrpanda.run`
5. API (optional): `api.mrpanda.run`

Notes:

- In MVP stage, recommend enabling only one subdomain first: `dock.mrpanda.run`.
- For mobile remote public entry, use: `remote.mrpanda.run` (for app pairing and session forwarding).
- Prefer in-app routes for other capabilities (`/threads`, `/providers`, `/skills`) to avoid premature domain split.

## MVP Feature Priority

1. P0: multi-provider connection, accounts, config templates, thread center
2. P1: MCP management, skills management, one-click switch, RN mobile read-only board
3. P2: RN mobile executable operations, auto-generated cross-provider thread summary, bulk migration tools

## Decisions Locked (2026-02-11)

1. `Yes`: V1 is strictly local-first, no cloud backup.
2. `Yes`: Thread data can be reused across providers (with permissions and audit).
3. `Yes`: V1 supports only 2 providers first (`Codex` + `Claude Code`).
4. `Yes`: Open-source the core adapter layer.
5. `Yes`: V1 supports cross-network remote access (LAN direct + secure relay).
