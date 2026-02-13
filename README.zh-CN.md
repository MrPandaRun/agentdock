# AgentDock

面向 `codex`、`claude_code`、`opencode` 的本地优先（local-first）编码代理控制平面。

[English](./README.md) | [简体中文](./README.zh-CN.md)

AgentDock 帮助你在同一个桌面工作台里查看、恢复、切换不同 provider 的线程，同时不替代上游 CLI。

## Why AgentDock

- 把多 provider 的代理工作流收敛到一个入口，减少在不同历史记录之间来回切换。
- 以 provider 原生会话数据为事实来源，同时维护本地统一索引。
- 提供共享契约层，确保桌面端、移动端和适配器语义一致。
- 默认本地优先：运行时、SQLite 状态和 CLI 集成都在本机。
- 面向真实编码循环：快速重连、定位历史线程、携带上下文继续执行。

## Feature Snapshot

| 能力 | 状态 | 说明 |
| --- | --- | --- |
| Provider 范围（`codex`、`claude_code`、`opencode`） | Now | TS 与 Rust 契约已对齐该范围。 |
| 本地优先桌面运行时（Tauri + React） | Now | Rust Host + React/Vite UI。 |
| 统一线程列表与恢复基础能力 | Now | Provider 适配器已暴露 list/resume 入口。 |
| 跨 provider 上下文切换编排 | In Progress | 摘要 + 回退流属于 Phase 1 目标。 |
| 移动端远程控制工作流 | Planned | Expo 壳层已存在，完整闭环不在 Phase 1。 |

## Quick Start

在仓库根目录执行：

```bash
bun install
```

启动桌面端：

```bash
bun run dev:desktop
```

启动移动端：

```bash
bun run dev:mobile
```

如果只需要桌面 Web UI（不启动 Tauri Host）：

```bash
bun run dev
```

## Prerequisites

- Bun `1.1.27+`（工作区包管理器）。
- Rust stable toolchain（见 `rust-toolchain.toml`）。
- Tauri v2 与 Expo 所需的平台依赖。
- 你要使用的 provider CLI 需要安装并在 `PATH` 可用：
  - `codex`
  - `claude`（对应 `claude_code`）
  - `opencode`

适配器支持的可选环境变量覆盖：

| 变量 | 作用 |
| --- | --- |
| `AGENTDOCK_CODEX_HOME_DIR` | 覆盖 Codex 会话目录根路径。 |
| `AGENTDOCK_CLAUDE_CONFIG_DIR` | 覆盖 Claude 配置目录根路径。 |
| `AGENTDOCK_CLAUDE_BIN` | 覆盖 Claude CLI 二进制名称/路径。 |
| `AGENTDOCK_OPENCODE_DATA_DIR` | 覆盖 OpenCode 数据目录根路径。 |
| `AGENTDOCK_OPENCODE_BIN` | 覆盖 OpenCode CLI 二进制名称/路径。 |

## Development Commands

以下命令均在仓库根目录执行：

| 命令 | 用途 |
| --- | --- |
| `bun run dev:desktop` | 启动 Tauri 桌面应用。 |
| `bun run dev:mobile` | 启动 Expo 移动端开发服务。 |
| `bun run dev` | 仅启动桌面 Web UI（Vite）。 |
| `bun run build` | 执行所有定义了 build 的工作区构建脚本。 |
| `bun run lint` | 执行工作区 lint/type 风格检查。 |
| `bun run typecheck` | 执行 TypeScript 检查 + `cargo check --workspace`。 |
| `bun run test` | 执行工作区测试 + `cargo test --workspace`。 |

定向执行示例：

```bash
bun run --filter @agentdock/contracts test
bun run --filter @agentdock/desktop typecheck
cargo test -p provider-codex -- list_threads_reads_codex_sessions
```

## Monorepo Layout

```text
apps/
  desktop/            Tauri 桌面应用（React UI + Rust host）
  mobile/             Expo 移动端壳层
packages/
  contracts/          共享 TypeScript provider 契约
  config-typescript/  共享 tsconfig 预设
  config-eslint/      共享 ESLint flat config 预设
crates/
  provider-contract/  共享 Rust provider 契约 + trait
  provider-codex/     Codex provider 适配器
  provider-claude/    Claude provider 适配器
  provider-opencode/  OpenCode provider 适配器
  agentdock-core/     SQLite 启动与迁移
docs/
  agentdock-phase1-prd.md
```

## Contracts & Provider Scope

当前 Provider ID 固定为：

```ts
type ProviderId = "codex" | "claude_code" | "opencode";
```

凡是 provider 字段、ID、错误语义变更，都应在同一个改动里同步 TS 与 Rust 契约：

- TypeScript：`packages/contracts/src/provider.ts`
- Rust：`crates/provider-contract/src/lib.rs`

## Roadmap / Current Status

当前项目阶段：**Alpha / Phase 1**。

Phase 1 的目标与验收细节见：

- [`docs/agentdock-phase1-prd.md`](./docs/agentdock-phase1-prd.md)

当前重点：

- Provider 连通性与健康检查。
- 统一线程读取与恢复流程。
- 跨 provider 切换基线与事件记录。
- 本地迁移与契约对齐流程稳定化。

不在 Phase 1 范围内：

- 团队协作与云同步。
- 完整计费系统。
- 移动端远程控制完整生产闭环。

## Contributing

1. Fork 仓库并创建分支。
2. 基于现有契约和代码风格实现改动及测试。
3. 运行基础检查：

```bash
bun run typecheck
bun run test
```

4. 使用 Conventional Commits（示例：`feat(desktop): add provider health panel`）。
5. 提交 PR 时请包含：
   - 变更内容
   - 变更原因
   - 已执行命令/测试
   - 桌面端或移动端 UI 变更截图

## FAQ

### AgentDock 会替代 provider CLI 吗？

不会。AgentDock 负责围绕 provider CLI 做编排与索引，实际会话执行仍由各 provider CLI 完成。

### 目前支持哪些 provider？

`codex`、`claude_code`、`opencode`。

### 数据存在哪里？

AgentDock 默认本地优先。provider 原生会话数据仍由各自 CLI 持有，AgentDock 维护本地 SQLite 索引和元数据。

### 为什么同时有 TypeScript 和 Rust 契约？

桌面/移动侧主要依赖 TypeScript，Host 与适配器主要依赖 Rust。契约双端对齐可保证跨运行时行为一致。

### 移动端现在可用于生产吗？

暂时不是。移动端目前定位为远程控制工作流壳层，完整生产能力不在 Phase 1 范围内。

## License

本项目采用 MIT License，详见 [`LICENSE`](./LICENSE)。
