# AgentDock 最新产品文档（基于当前实现）

> 更新日期：2026-02-18  
> 依据代码：`apps/desktop`、`apps/desktop/src-tauri`、`crates/*`、`packages/contracts`

## 1. 产品定位

AgentDock 是一个 **local-first 多 Agent 控制台**，用于统一管理和续接不同 CLI Agent 会话。  
当前支持的 Provider：

- `claude_code`
- `codex`
- `opencode`

核心边界：

- AgentDock 不替代各家 CLI 会话引擎；
- 会话执行与恢复仍由 provider CLI 完成；
- AgentDock 负责统一索引、展示、切换和本地运行体验。

## 2. 当前用户价值（已落地）

- 在一个桌面端中统一查看三类 provider 的历史线程。
- 按项目文件夹聚合线程并快速切换会话。
- 在应用内嵌终端中直接恢复并继续会话（默认模式）。
- 支持按 provider 新建线程入口（同项目路径下）。
- 支持线程切换时的后台会话保活策略（尽量不打断进行中的 agent）。

## 3. 当前能力清单

### 3.1 Provider 与契约层

- TS 契约：`packages/contracts/src/provider.ts`
  - `ProviderId = "codex" | "claude_code" | "opencode"`
  - health / list / resume / summarize_switch_context 契约完整
- Rust 契约：`crates/provider-contract/src/lib.rs`
  - 与 TS 语义对齐，含错误码与 `ProviderAdapter` trait

### 3.2 Adapter 实现（Rust）

- `provider-claude`
  - 读取 `~/.claude/projects` 线程与消息
  - 支持 `claude --resume` 恢复
  - 支持 `send_message`（`--print --output-format json`）
  - 支持 runtime state（用于判断 agent 是否仍在回答）
- `provider-codex`
  - 读取 `~/.codex/sessions` 线程与消息
  - 支持 `codex resume <thread_id>` 恢复
  - 支持 runtime state
- `provider-opencode`
  - 读取 `~/.local/share/opencode/storage` 会话数据
  - 支持 `opencode --session <thread_id>` 恢复
  - 支持 runtime state

### 3.3 Desktop（React + Tauri）

- 左侧：按 folder 分组线程列表，显示最新消息预览和时间。
- 右侧模式切换：
  - `Terminal`（默认）：应用内 PTY 终端，支持会话恢复/新会话启动。
  - `UI`（alpha）：消息流展示 + 输入框；当前仅 Claude 支持 UI 发送。
- 线程新建流程：
  - 左侧 folder 行内菜单可选 provider（Claude/Codex/OpenCode）
  - 触发终端启动后进行“新线程发现与绑定”
- 主题能力：
  - Light / Dark / System
  - 终端色板随主题切换

### 3.4 终端体验（重点）

- 真 PTY（`portable_pty`）+ xterm 渲染。
- 动态尺寸同步（前端 resize -> Tauri `resize_embedded_terminal`）。
- 会话级缓存和复用（线程切换时尽量复用现有 session）。
- 后台清理策略：
  - 若会话无用户输入且 runtime state 判断 agent 未在回答，则可清理；
  - 若 agent 正在回答，尽量保留后台运行。
- 输入体验增强：
  - `Shift+Enter` 换行
  - 复制粘贴快捷键兼容
  - dictation/IME 兼容性处理（xterm helper textarea 调整）
- 工具栏：
  - Refresh 重建当前终端会话
  - Provider Quick Guide（快捷键、模式、故障排查）

## 4. Tauri Host 能力（当前公开命令）

主要命令（`apps/desktop/src-tauri/src/commands.rs`）：

- 线程与消息：`list_threads`、`get_thread_messages`
- runtime state：
  - `get_claude_thread_runtime_state`
  - `get_codex_thread_runtime_state`
  - `get_opencode_thread_runtime_state`
- Claude UI 发送：`send_claude_message`
- 外部终端拉起：`open_thread_in_terminal`、`open_new_thread_in_terminal`
- 内嵌终端：`start_*` / `write_*` / `resize_*` / `close_*`

## 5. 数据层（SQLite）

`agentdock-core` 已在应用启动时自动初始化 DB 并执行迁移。

已落地的初始表：

- `providers`
- `accounts`
- `configs`
- `mcps`
- `skills`
- `threads`
- `thread_messages`
- `switch_events`
- `remote_devices`
- `remote_sessions`

迁移策略：append-only + 幂等执行（`run_migrations` 可重复运行）。

## 6. 移动端状态

`apps/mobile` 当前为 Expo 壳层，已复用 contracts 中 provider 类型；  
远程控制完整流程尚未闭环，仍属于后续阶段。

## 7. 当前约束与已知边界

- UI 发消息仅支持 `claude_code`，`codex/opencode` 当前通过 Terminal 继续会话。
- 产品主路径是“CLI 会话恢复与继续”，而非重建一套独立会话引擎。
- 窗口拖拽/标题区目前为持续打磨中的桌面交互项（已采用 overlay + 虚拟拖拽区方案）。

## 8. 阶段判断（按当前实现）

### 已完成（相对 Phase 1 核心）

- 三 provider 契约对齐（TS/Rust）
- 三 provider 线程扫描、消息读取、恢复命令打通
- 桌面端统一线程中心与终端会话主流程打通
- 本地 DB 初始化与迁移基线落地

### 进行中

- UI 模式从“只读 + Claude 可发送”向多 provider 能力收敛
- 桌面交互细节（窗口拖拽与布局一致性）持续优化

### 未完成（后续）

- 移动端远程控制闭环
- 更完整的跨 provider switch 编排产品化（策略/可视化/回退体验）
- 团队协作、云同步、权限与计费体系

## 9. 下一阶段需求（能力大块）

### 9.1 接入层（Multi-Channel Ingress）

- 统一接入入口：`Desktop App`、`Mobile App`、`Telegram`、`WhatsApp`、`微信`（服务号/企业微信）。
- 不同入口共享同一任务与会话模型，避免各渠道各自实现一套逻辑。
- 支持双向消息：渠道发起任务、AgentDock 回推状态与结果。
- 核心目标是“多入口接入，同一套编排内核”。

### 9.2 全局编排层（Coordinator）

- 提供跨 provider 的任务编排与调度能力（即时 + 定时）。
- 支持把任务分配给 `claude_code`、`codex`、`opencode`，并跟踪执行状态。
- 支持多 agent 并行与不中断切换，保障进行中任务不被误杀。
- 提供统一结果汇总视图（按任务 / 按项目文件夹）。

### 9.3 配置管理层（Accounts / Skills / MCP）

- 统一管理不同 provider 的账号状态、skills、mcp 配置。
- 支持按全局、按文件夹生效，并保留审计轨迹。
- 保障配置改动可控：默认影响新会话，不强制中断运行中会话。

### 9.4 模板层（Workspace Templates）

- 为文件夹绑定模板，快速注入该项目的 skills、mcp、预置 prompt。
- 支持内置模板与自定义模板并存，满足不同工作流场景。
- 模板目标是“快速复制工作能力”，不是复制会话历史。

### 9.5 观测与治理层（Ops & Guardrails）

- 提供任务状态、失败原因、执行日志、重试情况的统一观测。
- 提供权限和安全边界（谁可触发、谁可终止、哪些渠道可用）。
- 为后续团队协作与云端能力预留治理基础。

## 10. 建议推进顺序（高层）

1. 先做 `9.3` + `9.4`，把配置与模板底座打稳。
2. 再做 `9.2`，把全局编排与定时能力打通。
3. 然后做 `9.1`，先接 `App` 与 `Telegram`，再扩展到 `WhatsApp` 和 `微信`。
4. 最后补齐 `9.5`，形成可运营、可治理的完整闭环。
