# Dock Agent Issue 7 Implementation Plan

> Source: GitHub Issue #7, "[RFC]: Introduce Dock Agent as primary orchestration layer".
> Status: long-running implementation plan.
> Updated: 2026-06-03.

## Scope

Dock Agent becomes AgentDock's primary orchestration layer while the MVP provider set remains fixed to:

- `codex`
- `claude_code`
- `opencode`

Dock Agent is not a fourth provider. It coordinates existing provider threads, schedules recurring work, accepts one chat connector, mediates restricted remote commands, and records auditable decisions.

## Requirements

1. Cross-provider orchestration can create, resume, and track tasks against existing provider threads.
2. Scheduler supports hour-scale and week-scale recurring checks.
3. One chat connector can receive commands and enqueue Dock Agent tasks.
4. Remote control supports a restricted command surface with explicit policy and audit records.
5. Audit logs are durable and queryable for orchestration, chat, schedule, and remote command events.
6. TypeScript and Rust contracts stay semantically aligned.
7. SQLite migrations remain append-only and idempotent.
8. Desktop and mobile surfaces expose only the necessary entry points for the MVP workflow.
9. Existing thread listing, thread resume, and provider switching remain stable.

## Current Baseline

- Provider adapters already exist for `codex`, `claude_code`, and `opencode`.
- Shared provider/thread contracts exist in TypeScript and Rust.
- SQLite migration execution is centralized in `crates/agentdock-core/src/db/mod.rs`.
- Desktop has thread list, resume, terminal launch, MCP, skill, and provider health surfaces.
- Mobile currently shows a minimal remote shell with the three MVP providers.
- Some Sophon proposal and integration code exists, but Dock Agent must not depend on Sophon as a provider.

## Phase 1: Foundation

- Add Dock Agent contract types for tasks, schedules, chat connectors, remote commands, and audit logs.
- Add SQLite tables for durable Dock Agent state.
- Keep provider references nullable and tied to existing provider/thread concepts.
- Verify migrations are idempotent.
- Document that Dock Agent is an orchestration layer, not a provider.

## Phase 2: Core Runtime

- Add an `agentdock-core::dock_agent` module that owns task persistence and state transitions.
- Implement allowed state transitions:
  - `queued` to `running`
  - `running` to `succeeded`
  - `running` to `failed`
  - `queued` or `running` to `canceled`
- Emit audit logs for every task transition.
- Add focused Rust tests for persistence and transition validation.

## Phase 3: Scheduler

- Implement a scheduler service that scans enabled schedules by `next_run_at`.
- Support `hourly`, `weekly`, and `cron` cadence values.
- Enqueue due `scheduled_check` tasks without executing provider work inline.
- Update `last_run_at` and compute the next run atomically.
- Add tests for repeat runs, disabled schedules, and idempotent due scans.

## Phase 4: Chat Connector

- Implement one `webhook` chat connector.
- Validate connector secrets outside plain config JSON.
- Convert inbound chat payloads into `chat_command` tasks.
- Emit audit logs for accepted and rejected inbound commands.
- Add desktop setup and test command affordances.

## Phase 5: Restricted Remote Commands

- Define a command allowlist policy format in `policy_json`.
- Reject commands that do not match policy before execution.
- Require an actor and remote device context where available.
- Store command args, working directory, result, and outcome.
- Emit audit logs for request, approval, rejection, execution, success, and failure.

## Phase 6: Desktop And Mobile Entry Points

- Desktop: add a Dock Agent queue/status panel with schedule, connector, remote command, and audit sections.
- Mobile: add remote workflow entry points for queue inspection and restricted command requests.
- Preserve existing provider thread list, resume, and terminal behavior.
- Keep provider selector limited to `codex`, `claude_code`, and `opencode` for MVP provider workflows.

## Phase 7: Provider Boundary Cleanup

- Remove Sophon from shared `ProviderId` contracts and MVP provider arrays.
- Isolate any remaining Sophon proposal or CLI code from the provider contract path.
- Update desktop code paths that currently treat Sophon as a provider.
- Re-run contract, desktop typecheck, and Rust workspace tests.

## Verification Gates

- `bun run --filter @agentdock/contracts test`
- `bun run --filter @agentdock/contracts typecheck`
- `cargo test -p agentdock-core db`
- `cargo test --workspace --manifest-path ./Cargo.toml`
- `bun run typecheck`
- `bun run test`

## Risks

- Sophon references are currently broad in desktop and Rust host code; provider boundary cleanup should be isolated from scheduler/runtime work.
- Remote command execution needs conservative defaults and a deny-by-default policy.
- Chat connector secrets must not be stored directly in connector config JSON.
- Scheduler retries need careful idempotency to avoid duplicate provider work.
