# Project AgentDock

## Project Summary

- Product Name: `AgentDock`（中文：`代理坞`）
- Entry: `dock.mrpanda.run`
- Positioning: 多 Agent 控制平面（账号、配置、MCP、skills、thread、RN 手机远程）
- Core Promise: `一次配置，多端复用；统一线程，快速切换`

## Locked Decisions (2026-02-11)

1. `Yes` V1 仅本地优先，不做云备份。
2. `Yes` Thread 允许跨 provider 复用（需权限与审计）。
3. `Yes` V1 只支持 2 个 provider：`Codex` + `Claude Code`。
4. `Yes` 开源核心适配器层。
5. `Yes` 手机远程支持跨网络访问（局域网直连 + 安全 relay）。

## Scope Snapshot (MVP)

- P0：Provider 连接、账号/配置中心、Thread Center
- P1：MCP Registry、Skills Hub、Switcher、RN 手机端只读
- P2：RN 手机端白名单操作、跨 provider 摘要自动生成

## Timeline Snapshot

1. Week 1-2：需求冻结、数据模型、低保真原型
2. Week 3-4：Provider + 账号配置
3. Week 5-6：Thread Center
4. Week 7-8：MCP + Skills
5. Week 9-10：Switcher + RN 手机只读 + 安全配对
6. Week 11：RN 手机白名单操作 + 稳定性修复
7. Week 12：内测发布

## Document Map

- 主 PRD：[[02-Projects/agentdock/PRD-AgentDock-v1]]
- 原始归档：[[07-Docs/D-20260211-agent-control-plane-prd-v1]]

## Next Actions

1. 产出 V1 信息架构线框（Dashboard/Threads/Providers/MCP/Skills/Remote）
2. 定义 provider adapter 契约（输入输出、错误码、超时策略）
3. 设计 RN 手机端跨网络配对流程（二维码 + 短时 token + 吊销 + relay 安全策略）
