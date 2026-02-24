# AgentDock V1 Architecture Design

> Historical architecture document (V1 planning baseline).
> The errata section below is authoritative for current implementation as of 2026-02-24.

## Historical + Errata (2026-02-24)

| Topic | Historical Text | Current Implementation Truth |
| --- | --- | --- |
| Provider scope | Two providers (`codex`, `claude_code`) | Current code/contract scope: `codex`, `claude_code`, `opencode`. |
| Adapter maturity | Dual-provider contract + adapter stubs | Adapters are implemented in `provider-codex`, `provider-claude`, and `provider-opencode`. |
| Contract methods | Includes `summarizeSwitchContext` alignment | Current shared contract/trait includes `health_check`, `list_threads`, `resume_thread` only. |
| Switch flow | Uses `summarize_switch_context` runtime call | This is a historical target flow, not a current contract API. |
| Desktop interaction model | Broader thread-management assumptions | Current execution UI is terminal-only for thread continuation. |

## 1. Document Goal

This document describes an implementable architecture for AgentDock V1, covering the current repository baseline and near-term implementation boundaries, for engineering, testing, and follow-up extension work (V1.1).

V1 goals:

- Local-first, with no cloud sync dependency as the primary path
- Support three providers: `codex`, `claude_code`, `opencode`
- Unified management of accounts, configs, MCP, skills, and threads, plus mobile remote viewing/action entry

---

## 2. Current Technical Baseline

- Desktop: Tauri + React + TypeScript
  - UI: `apps/desktop/src`
  - Host: `apps/desktop/src-tauri`
- Mobile: Expo + React Native + TypeScript
  - `apps/mobile`
- Shared TS contracts:
  - `packages/contracts`
- Rust workspace:
  - `crates/provider-contract`
  - `crates/provider-codex`
  - `crates/provider-claude`
  - `crates/provider-opencode`
  - `crates/agentdock-core`
- Data storage: SQLite (migrations under `crates/agentdock-core/migrations`)
- Package manager: Bun workspace (`bun.lockb`)

---

## 3. Architectural Principles

- Local-first: core state is readable and recoverable locally
- Contract-first: TS and Rust share semantically aligned provider contracts
- Adapter isolation: provider-specific differences are converged into the adapter layer
- Incremental extensibility: keep remote/relay extension points in V1 without introducing early cloud-state complexity
- Secure by default: no plaintext credentials; remote control starts from least privilege

---

## 4. System Context Diagram

```mermaid
flowchart LR
    U["Developer (Desktop)"] --> D["AgentDock Desktop App"]
    M["Developer (Mobile)"] --> A["AgentDock Mobile App"]
    D --> C1["Codex CLI / API"]
    D --> C2["Claude Code CLI / API"]
    D --> C3["OpenCode CLI / API"]
    D --> K["OS Keychain"]
    D --> FS["Local Filesystem (Thread logs / configs)"]
    D --> DB["SQLite (local index/config)"]
    A <-->|"LAN / Internet (secure channel)"| D
    A <-->|"Optional secure relay (V1 public network path)"| R["Relay (stateless forwarding)"]
    R <-->|Encrypted tunnel| D
```

---

## 5. Container and Module Layering

```mermaid
flowchart TB
    subgraph Desktop["apps/desktop"]
      UI["React UI (Dashboard/Threads/Providers/MCP/Skills/Remote)"]
      IPC["Tauri IPC Boundary"]
      UI --> IPC
    end

    subgraph Host["apps/desktop/src-tauri"]
      CMD["Tauri Commands"]
      CORE["agentdock-core (db/migration/domain)"]
      PC["provider-contract"]
      COD["provider-codex"]
      CLA["provider-claude"]
      OPC["provider-opencode"]
      CMD --> CORE
      CMD --> COD
      CMD --> CLA
      CMD --> OPC
      COD -.implements.-> PC
      CLA -.implements.-> PC
      OPC -.implements.-> PC
    end

    subgraph Shared["packages/contracts"]
      TS_CONTRACT["TS Provider/Thread Contracts"]
    end

    subgraph Mobile["apps/mobile"]
      RN["React Native App"]
    end

    UI --> TS_CONTRACT
    RN --> TS_CONTRACT
    IPC --> CMD
```

---

## 6. Core Runtime Flows

### 6.1 Desktop Startup and Database Initialization

Current implementation executes on desktop host startup:

- Resolve app data directory
- Create `agentdock.db`
- Run `init_db` + `run_migrations`

```mermaid
sequenceDiagram
    participant App as Desktop App
    participant Host as Tauri Host (Rust)
    participant Core as agentdock-core::db
    participant DB as SQLite

    App->>Host: launch
    Host->>Core: init_db(path/to/agentdock.db)
    Core->>DB: open connection
    Core->>DB: PRAGMA foreign_keys=ON
    Core->>DB: ensure schema_migrations exists
    Core->>DB: apply 0001_init.sql if not applied
    DB-->>Core: success
    Core-->>Host: ready
    Host-->>App: app ready
```

### 6.2 Cross-Provider Thread Switch (Historical Target Flow, Not Current Contract API)

```mermaid
sequenceDiagram
    participant UI as Desktop UI
    participant Host as Tauri Host
    participant Src as Source Provider Adapter
    participant Dst as Target Provider Adapter
    participant DB as SQLite

    UI->>Host: switchThread(threadId, targetProvider)
    Host->>Src: summarize_switch_context(threadId)
    Src-->>Host: SwitchContextSummary
    Host->>Dst: resume_thread(contextSummary)
    alt success
      Dst-->>Host: ResumeThreadResult(resumed=true)
      Host->>DB: insert switch_events(result=success)
      Host-->>UI: switched
    else fail
      Dst-->>Host: ProviderError
      Host->>DB: insert switch_events(result=failed)
      Host-->>UI: fallback (prompt-only)
    end
```

### 6.3 Mobile Remote View/Action (V1 Design Path)

```mermaid
sequenceDiagram
    participant Mobile as Mobile App
    participant Desktop as Desktop Runtime
    participant DB as SQLite
    participant Relay as Optional Relay

    Mobile->>Desktop: pair/auth (LAN preferred)
    alt public network
      Mobile->>Relay: connect
      Relay->>Desktop: encrypted forwarding
    end
    Mobile->>Desktop: query thread status
    Desktop->>DB: read thread index/state
    DB-->>Desktop: data
    Desktop-->>Mobile: status/log summary
```

---

## 7. Data Model (V1)

The following entities are based on migration `0001_init.sql`.

```mermaid
erDiagram
    providers ||--o{ accounts : has
    providers ||--o{ threads : owns
    providers ||--o{ configs : scopes
    accounts ||--o{ configs : owns
    accounts ||--o{ threads : owns
    threads ||--o{ thread_messages : contains
    threads ||--o{ switch_events : source
    providers ||--o{ switch_events : target
    remote_devices ||--o{ remote_sessions : starts
    threads ||--o{ remote_sessions : references

    providers {
      text id PK
      text name
      text status
      text last_checked_at
    }
    accounts {
      text id PK
      text provider_id FK
      text profile_name
      text credential_ref
      text created_at
    }
    configs {
      text id PK
      text scope
      text provider_id FK
      text account_id FK
      text payload_json
      text updated_at
    }
    mcps {
      text id PK
      text name
      text command
      text args_json
      text scope
      int enabled
      text version
    }
    skills {
      text id PK
      text name
      text source
      text version
      int enabled
      text compatibility_json
    }
    threads {
      text id PK
      text provider_id FK
      text account_id FK
      text project_path
      text title
      text tags_json
      text last_active_at
    }
    thread_messages {
      text id PK
      text thread_id FK
      text role
      text content
      text created_at
    }
    switch_events {
      text id PK
      text from_thread_id FK
      text to_provider_id FK
      text result
      text created_at
    }
    remote_devices {
      text id PK
      text device_name
      text paired_at
      text last_seen_at
      text revoked_at
    }
    remote_sessions {
      text id PK
      text device_id FK
      text thread_id FK
      text action
      text result
      text created_at
    }
```

---

## 8. Deployment and Network Topology (V1)

```mermaid
flowchart LR
    subgraph LocalMachine["Developer Local Machine"]
      Desktop["Tauri Desktop App"]
      SQLite["SQLite"]
      Keychain["OS Keychain"]
      Desktop --> SQLite
      Desktop --> Keychain
    end

    subgraph MobileNet["Mobile Device"]
      Mobile["Expo RN App"]
    end

    Mobile <-->|"LAN preferred"| Desktop
    Mobile <-->|"Public Internet (optional)"| Relay["Secure Relay (no business data persistence)"]
    Relay <-->|Encrypted forward| Desktop
```

---

## 9. Contract and Boundary Design

### 9.1 Provider Contracts (Dual-Side Sync)

- TS: `packages/contracts/src/provider.ts`
- Rust: `crates/provider-contract/src/lib.rs`

Sync constraints:

- Provider IDs: `codex`, `claude_code`, `opencode`
- Consistent error-code semantics (for example `not_implemented`)
- `healthCheck/listThreads/resumeThread` aligned with Rust traits

### 9.2 Adapter Boundaries

- `provider-codex`, `provider-claude`, and `provider-opencode` are implementation layers depending on `provider-contract`
- UI and business layer must not depend directly on provider-specific protocol details
- New providers should be added as new crates implementing the trait, instead of changing upper-layer workflows

---

## 10. Security and Reliability Design

- Credentials are stored as keychain references, not plaintext in SQLite
- Foreign key constraints enabled by default in DB (`PRAGMA foreign_keys=ON`)
- Migration execution is reentrant to avoid schema issues on repeated startup
- Remote control starts with least privilege; write actions require explicit allowlist policy
- Provider call timeout/error/fallback (prompt-only) should follow a unified strategy (gradually completed in V1)

---

## 11. Observability Recommendations (V1 -> V1.1)

- Local log levels: `INFO/WARN/ERROR`, with module tags (provider/db/remote/switch)
- Key metrics:
  - provider health-check success rate
  - thread resume success rate
  - switch latency (P50/P95)
  - remote action success rate
- Switch and remote actions should persist to event tables (already available: `switch_events`, `remote_sessions`)

---

## 12. Evolution Plan

### V1 (Current)

- Connect desktop startup flow with local DB initialization
- Solidify three-provider contracts and adapter implementations
- Establish unified data model and migration mechanism

### V1.1

- Explore additional providers beyond current three-provider baseline
- Improve switch-context strategy and failure fallback UX
- Strengthen remote-control secure pairing and permission model

### V2 (Direction)

- Team collaboration and optional cloud sync
- More complete audit, policy engine, and plugin ecosystem

---

## 13. Acceptance Checklist

- `bun run typecheck` passes
- `bun run test` passes
- Desktop app auto-creates and migrates local SQLite on startup
- TS and Rust provider contracts remain semantically aligned
- New migrations follow append-only policy (no edits to historical migrations)
