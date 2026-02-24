# AgentDock

面向 `codex`、`claude_code`、`opencode` 的本地优先（local-first）编码代理控制平面。

[English](./README.md) | [简体中文](./README.zh-CN.md)

AgentDock 帮助你在同一个桌面工作台中查看并恢复 provider 原生线程，不替代上游 CLI。

## Why AgentDock

- 把多 provider 编码工作流收敛到一个入口，减少在不同 CLI 历史之间来回切换。
- 以 provider 原生会话数据为事实来源，同时维护本地统一索引。
- 保持 TS/Rust 契约语义对齐，确保桌面端、移动端和适配器行为一致。
- 默认本地优先：运行时、SQLite 状态和 CLI 集成都在本机。

## Feature Snapshot

| 能力 | 状态 | 说明 |
| --- | --- | --- |
| Provider 范围（`codex`、`claude_code`、`opencode`） | Now | TS 与 Rust 契约已对齐。 |
| 本地优先桌面运行时（Tauri + React） | Now | Rust host + React/Vite UI。 |
| 统一线程列表 + 恢复 | Now | 三个适配器已接入线程扫描和恢复命令路径。 |
| 桌面执行模式 | Now | terminal-only（内嵌 PTY + 外部终端启动）。 |
| 跨 provider 摘要编排 | Planned | 当前 `ProviderAdapter` 契约未包含此接口。 |
| 移动端远程控制流程 | Planned | Expo 壳层已存在，完整闭环尚未完成。 |

## 当前桌面行为

- 左侧按项目文件夹分组展示线程。
- 线程数据包含 `title` 和可选的 `lastMessagePreview`。
- 左侧条目文本优先使用 `title`；为空时回退到 `lastMessagePreview`。
- Header 标题使用当前选中线程的 `title`。
- 适配器标题策略优先 provider 官方标题，其次回退到用户输入。

## Quick Start

在仓库根目录执行：

```bash
bun install
bun run dev:desktop
```

可选：

```bash
bun run dev:mobile
bun run dev
```

## Prerequisites

- Bun `1.1.27+`
- Rust stable toolchain（见 `rust-toolchain.toml`）
- Tauri v2 与 Expo 所需平台依赖
- 以下 provider CLI 在 `PATH` 可用：
  - `codex`
  - `claude`（对应 `claude_code`）
  - `opencode`

可选环境变量覆盖：

| 变量 | 作用 |
| --- | --- |
| `AGENTDOCK_CODEX_HOME_DIR` | 覆盖 Codex 会话目录根路径。 |
| `AGENTDOCK_CLAUDE_CONFIG_DIR` | 覆盖 Claude 配置目录根路径。 |
| `AGENTDOCK_CLAUDE_BIN` | 覆盖 Claude CLI 二进制名称/路径。 |
| `AGENTDOCK_OPENCODE_DATA_DIR` | 覆盖 OpenCode 数据目录根路径。 |
| `AGENTDOCK_OPENCODE_BIN` | 覆盖 OpenCode CLI 二进制名称/路径。 |

## Development Commands

| 命令 | 用途 |
| --- | --- |
| `bun run dev:desktop` | 启动 Tauri 桌面应用。 |
| `bun run dev:mobile` | 启动 Expo 移动端开发服务。 |
| `bun run dev` | 仅启动桌面 Web UI（Vite）。 |
| `bun run build` | 执行定义了 build 的工作区构建脚本。 |
| `bun run lint` | 执行工作区 lint 检查。 |
| `bun run typecheck` | 执行 TS 检查 + `cargo check --workspace`。 |
| `bun run test` | 执行工作区测试 + `cargo test --workspace`。 |

定向示例：

```bash
bun run --filter @agentdock/contracts test
bun run --filter @agentdock/desktop typecheck
cargo test -p provider-codex -- list_threads_reads_codex_sessions
```

## 契约说明

Provider ID 固定为：

```ts
type ProviderId = "codex" | "claude_code" | "opencode";
```

共享契约文件：
- TS：`packages/contracts/src/provider.ts`
- Rust：`crates/provider-contract/src/lib.rs`

`ProviderAdapter` 当前方法：
- `health_check`
- `list_threads`
- `resume_thread`

## 文档索引

- 当前实现摘要：[`docs/agentdock-current-product-summary.md`](./docs/agentdock-current-product-summary.md)
- 当前 Phase 1 执行规范：[`docs/agentdock-phase1-prd.md`](./docs/agentdock-phase1-prd.md)
- 带勘误的历史规划文档：
  - [`Project-AgentDock.md`](./Project-AgentDock.md)
  - [`PRD-AgentDock-v1.md`](./PRD-AgentDock-v1.md)
  - [`Architecture-AgentDock-v1.md`](./Architecture-AgentDock-v1.md)

## Contributing

1. 创建分支。
2. 实现改动并补测试。
3. 运行：

```bash
bun run typecheck
bun run test
```

4. 使用 Conventional Commits。
5. PR 需包含变更内容、原因、命令/测试与必要截图。

## License

MIT，见 [`LICENSE`](./LICENSE)。
