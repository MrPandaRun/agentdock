# D-20260211-agent-control-plane-prd-v1

## Executive Summary

做一个本地优先（local-first）的多 Agent 控制台，统一管理 `Codex`、`Claude Code`、`Gemini`、`OpenCode` 的账号、配置、MCP、skills，并提供类似 Codex Desktop 的线程（thread）会话中心与手机远程控制入口。

产品目标是把“多工具切换和状态分散”的成本降到最低：用户不再反复改配置、找历史会话、重复安装 MCP/skills，而是在一个控制台内完成连接、切换、延续和复用；离开电脑时也能用手机查看进度并执行关键操作。

## Product Positioning

- 产品类型：开发者 AI Agent 控制平面（Agent Control Plane）
- 核心价值：`一次配置，多端复用；统一线程，快速切换`
- 竞品参考：`cc switch`（切换能力）+ `Codex Desktop`（线程管理体验）
- 差异化：
  - 不只切模型/供应商，而是管理完整工作上下文（账号 + 配置 + MCP + skills + thread）
  - 本地优先，适合个人开发者和小团队先跑通流程
  - 面向“AI coding workflow”而非通用聊天

## Target Users

1. 重度 AI 编程用户：同时使用 2-4 个代码 Agent
2. 技术内容创作者：需要保存并复用大量 thread 上下文
3. 小团队 Tech Lead：希望统一团队工具栈与工作方式

## Problem Statement

**当前痛点**

- 账号和 key 分散在不同 CLI/桌面应用
- 配置格式不一致（模型、温度、路径、权限、环境变量）
- MCP 和 skills 需要重复安装、逐个维护版本
- thread 会话跨工具割裂，难检索、难续写、难交接

**目标状态**

- 5 分钟内完成多 Agent 初始化
- 15 秒内从一个 Agent 切到另一个 Agent 并带上上下文
- 线程统一检索与恢复，减少“重新解释问题”的重复成本

## MVP Scope (V1)

### In Scope

1. Provider 连接与账号管理（先支持 `Codex` + `Claude Code`）
2. 配置中心（模型、默认参数、工作目录、环境变量模板）
3. MCP Registry（安装、启停、权限范围、版本）
4. Skills Center（导入、启停、版本状态、兼容性提示）
5. Thread Center（会话列表、标签、搜索、恢复、跳转）
6. 一键切换入口（从当前 thread 启动到另一个 provider）
7. 手机远程控制（React Native）：查看 thread 状态、触发预设操作、查看最近执行结果

### Out of Scope

1. 团队协作与云同步
2. Mobile Web / PWA 远程端
3. 复杂计费系统
4. 非开发场景的通用聊天 UI

## Core Modules

### M01 Accounts & Profiles

- 多账户（工作/个人）与 provider 绑定
- 凭据状态检查（有效/过期/缺失）
- 本地加密存储（系统 Keychain）

### M02 Config Control Center

- provider 配置模板与覆盖层（global/project/profile）
- 可视化差异比较（diff）
- 快速导入导出

### M03 MCP Registry

- MCP server 列表、启动参数、权限策略
- 按 thread 或按 profile 启用
- 版本升级提示与回滚

### M04 Skills Hub

- skills 安装源管理（本地路径/Git）
- 启停状态与版本一致性检查
- provider 兼容矩阵（哪些技能在哪些 provider 可用）

### M05 Thread Center

- 统一线程索引（provider、时间、项目、标签、状态）
- 全文检索（标题/消息/标签）
- Thread 快照（关键提示词、依赖、执行目录）

### M06 Switcher

- 从 thread 直接切换 provider
- 自动注入最小上下文摘要（最近目标、关键约束、未完成任务）
- 切换失败时提供 fallback（只带 prompt，不带环境）

### M07 Mobile Remote (React Native)

- 手机端访问控制台（React Native App，iOS/Android）
- 远程查看：thread 列表、运行状态、最近日志摘要
- 远程操作：暂停/继续、重试上一步、切换 provider（受权限策略约束）
- 安全配对：二维码绑定 + 短时令牌 + 可撤销设备授权
- 连接范围：V1 支持局域网与公网远程访问

## Information Architecture

1. Dashboard
2. Threads
3. Providers
4. MCP
5. Skills
6. Remote
7. Settings

## Technical Architecture (建议)

- 桌面壳：`Tauri`（性能和系统权限更友好）
- 前端：`React + TypeScript + Zustand + TanStack Query`
- 手机端：`React Native + Expo + TypeScript`
- 后端服务层：`Rust`（进程管理、文件系统监听、密钥桥接）
- 通信：`WebSocket`（桌面端与手机端的实时状态同步）
- 存储：
  - `SQLite`：结构化配置和线程索引
  - 文件索引：thread 原始日志映射
  - 系统 Keychain：敏感凭据
- 远程连接策略（MVP）：
  - 同局域网直连优先
  - 提供安全 relay（公网可用）
  - relay 仅转发加密通道，不存储业务数据
- 适配器层：
  - `provider-adapter-codex`
  - `provider-adapter-claude-code`
  - `provider-adapter-gemini`（V1.1）
  - `provider-adapter-opencode`（V1.1）

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

## Security & Reliability Baseline

- API key 不落明文：只存 keychain 引用
- thread 导入前做脱敏规则（可配置）
- 所有外部命令执行加超时和日志
- provider 适配器契约测试（防止上游格式变化导致解析崩溃）
- 远程控制默认最小权限：先只开放“查看 + 安全操作白名单”
- 手机端授权支持单设备吊销与全量失效（紧急登出）

## Delivery Roadmap (12 Weeks)

1. Week 1-2：需求冻结 + 架构与数据模型落地 + 低保真原型
2. Week 3-4：Provider 连接（Codex/Claude）+ 账号与配置中心
3. Week 5-6：Thread Center（索引、搜索、恢复）
4. Week 7-8：MCP Registry + Skills Hub
5. Week 9-10：Switcher + RN 手机端只读看板 + 远程安全配对
6. Week 11：RN 手机端白名单操作 + 稳定性修复 + 可观测性
7. Week 12：内测发布（20-50 位种子用户）

## Success Metrics (First 60 Days)

1. 首次完成双 provider 连接时间 <= 10 分钟
2. 线程恢复成功率 >= 95%
3. 切换操作中位耗时 <= 8 秒
4. 周活跃用户（WAU）留存 >= 35%
5. 人均每周切换次数 >= 12
6. 手机远程控制周使用渗透率 >= 25%
7. 手机端远程操作成功率 >= 98%

## Naming Options

| Name             | 类型   | 含义                     | 子域名候选            |
| ---------------- | ------ | ------------------------ | --------------------- |
| AgentDock        | 主推   | Agent 的“停靠与调度港口” | `dock.mrpanda.run`    |
| ThreadDock       | 主推   | 强调线程会话中心         | `threads.mrpanda.run` |
| SwitchForge      | 备选   | 强调切换与工作流锻造     | `switch.mrpanda.run`  |
| ContextPort      | 备选   | 强调上下文转运和复用     | `context.mrpanda.run` |
| AgentSwitchboard | 描述型 | 直观表达“总控台”         | `control.mrpanda.run` |
| ModelBridge      | 备选   | 强调模型/Agent 间桥接    | `bridge.mrpanda.run`  |

## Recommended Naming

- 英文产品名：`AgentDock`
- 中文名：`代理坞`
- Slogan：`One dock for all coding agents.`
- 推荐主入口：`dock.mrpanda.run`

理由：

1. 不局限“switch”，覆盖管理和沉淀能力
2. 读音和语义都稳定，适合做品牌延展（Dock OS / Dock Cloud）
3. 与 thread 中心概念自然兼容（thread docking）

## Domain & URL Strategy

1. 统一主域：`mrpanda.run`
2. 产品入口：`dock.mrpanda.run`
3. 管理后台（可选）：`control.mrpanda.run`
4. 文档站（可选）：`docs.mrpanda.run`
5. API（可选）：`api.mrpanda.run`

说明：

- MVP 阶段建议先只启用一个子域：`dock.mrpanda.run`。
- 手机远程支持公网入口：`remote.mrpanda.run`（可与 App 配对和会话转发）。
- 其他能力优先使用应用内路由（如 `/threads`, `/providers`, `/skills`），避免过早拆分。

## MVP Feature Priority

1. P0：多 provider 连接、账号、配置模板、线程中心
2. P1：MCP 管理、skills 管理、一键切换、RN 手机端只读看板
3. P2：RN 手机端可执行操作、跨 provider thread 摘要自动生成、批量迁移工具

## Decisions Locked (2026-02-11)

1. `Yes`：V1 强制本地优先，不做云备份。
2. `Yes`：Thread 数据允许跨 provider 复用（保留权限与审计）。
3. `Yes`：V1 先只支持 2 个 provider（`Codex` + `Claude Code`）。
4. `Yes`：开源核心适配器层。
5. `Yes`：V1 提供跨网络远程访问（局域网直连 + 安全 relay）。
