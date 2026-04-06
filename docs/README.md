# EasyFlowHub Docs

## Read Order

对 AI 或新维护者，推荐按下面顺序阅读：

1. [AGENTS.md](../AGENTS.md)
2. [PROJECT_PROFILE.md](./PROJECT_PROFILE.md)
3. [roadmap.md](./roadmap.md)
4. [ARCHITECTURE_GUARDRAILS.md](./ARCHITECTURE_GUARDRAILS.md)
5. `docs/checklists/review-checklist.md`（涉及共享 owner、跨层 import、review 时必读）
6. 最近一次相关的 `MODULE.md`
7. 相关 `docs/decisions/ADR-*.md`
8. 相关 `docs/plans/YYYY-MM-DD-*.md`

## Document Types

| Path | Purpose | Maintenance rule |
|---|---|---|
| `docs/PROJECT_PROFILE.md` | 项目画像、验证命令、入口与 owner 概览 | 只在技术栈、验证命令或拓扑变化时更新 |
| `docs/roadmap.md` | 当前版本目标、active tracks、验证债务 | 只保留 active 内容，不保留长历史 |
| `docs/ARCHITECTURE_GUARDRAILS.md` | 架构边界、依赖方向、canonical owners | 边界变化时必须更新 |
| `docs/checklists/*.md` | 实施前 / review 时的执行清单 | 规则或协作方式变化时更新 |
| `*/MODULE.md` | 模块职责、owner、不变量、常见坑 | 只写模块级高价值信息 |
| `docs/plans/*.md` | 临时设计 / 实施方案 | 完成后要归档、收敛或 supersede |
| `docs/decisions/ADR-*.md` | 长期有效的边界决策 | 不记录普通实现步骤 |
| `docs/templates/*` | 通用模板与可复制手册 | 作为其他仓库或新模块的启动骨架 |

## Current Source Of Truth

- 仓库画像: [PROJECT_PROFILE.md](./PROJECT_PROFILE.md)
- 当前目标与验证债: [roadmap.md](./roadmap.md)
- 架构边界: [ARCHITECTURE_GUARDRAILS.md](./ARCHITECTURE_GUARDRAILS.md)
- 实施 / review 清单: [checklists/review-checklist.md](./checklists/review-checklist.md)
- Windows 打包 smoke: [checklists/windows-package-smoke.md](./checklists/windows-package-smoke.md)
- 通用模板入口: [templates/AI_REPO_BOOTSTRAP_PLAYBOOK.md](./templates/AI_REPO_BOOTSTRAP_PLAYBOOK.md)

## Doc Hygiene Rules

- 文档只写源码和工具不容易直接推导的信息
- 一项长期边界只保留一个权威文档入口
- 已过期大文档不要继续修补，优先缩成指针文档或归档
- 每次非平凡改动至少同步 `roadmap`、相关 `MODULE.md`、`ADR`、`plan` 中的一项
