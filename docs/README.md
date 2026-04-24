# EasyFlowHub Docs

## Read Order

对 AI 或新维护者，推荐按下面顺序阅读：

1. [AGENTS.md](../AGENTS.md)
2. [PROJECT_PROFILE.md](./PROJECT_PROFILE.md)
3. [PRD-v1.0.0.md](./PRD-v1.0.0.md)
4. [roadmap.md](./roadmap.md)
5. [ARCHITECTURE_GUARDRAILS.md](./ARCHITECTURE_GUARDRAILS.md)
6. `docs/checklists/review-checklist.md`（涉及共享 owner、跨层 import、review 时必读）
7. 最近一次相关的 `MODULE.md`
8. 相关 `docs/decisions/ADR-*.md`
9. 相关 `docs/plans/YYYY-MM-DD-*.md`

## Fast Paths

如果目标是“快速进入开发”，不要先扫完整个 `docs/plans` 目录，按任务类型走最短路径：

### 扩展平台 / VSCode 风格扩展

1. [roadmap.md](./roadmap.md)
2. [plans/2026-04-11-vscode-style-extension-platform-roadmap.md](./plans/2026-04-11-vscode-style-extension-platform-roadmap.md)
3. [plans/2026-04-08-extension-enhancement-plan.md](./plans/2026-04-08-extension-enhancement-plan.md)
4. [plans/2026-04-07-extension-lifecycle-design.md](./plans/2026-04-07-extension-lifecycle-design.md)
5. 相关 `MODULE.md`

### Manager 模块化 / 面板接入

1. [roadmap.md](./roadmap.md)
2. [../easyflowhub-app/src/modules/MODULE.md](../easyflowhub-app/src/modules/MODULE.md)
3. [plans/2026-04-11-vscode-style-extension-platform-roadmap.md](./plans/2026-04-11-vscode-style-extension-platform-roadmap.md)
4. 最近一次相关的 `manager` 子模块文档

### 打包 / 本地验收

1. [roadmap.md](./roadmap.md)
2. [checklists/windows-package-smoke.md](./checklists/windows-package-smoke.md)
3. [checklists/scripted-smoke.md](./checklists/scripted-smoke.md)

### 版本规划 / 发版收口

1. [PRD-v1.0.0.md](./PRD-v1.0.0.md)
2. [roadmap.md](./roadmap.md)
3. [PROJECT_PROFILE.md](./PROJECT_PROFILE.md)
4. [ARCHITECTURE_GUARDRAILS.md](./ARCHITECTURE_GUARDRAILS.md)
5. 最近一次相关的 `docs/plans/*.md`

更细的 plans 索引见 [plans/README.md](./plans/README.md)。

## Document Types

| Path | Purpose | Maintenance rule |
|---|---|---|
| `docs/PROJECT_PROFILE.md` | 项目画像、验证命令、入口与 owner 概览 | 只在技术栈、验证命令或拓扑变化时更新 |
| `docs/PRD-v1.0.0.md` | `v1.0.0` 正式需求基线、范围和开发阶段设计 | 需求边界或版本策略变化时更新 |
| `docs/roadmap.md` | 当前版本执行状态、active tracks、验证债务与 next queue | 只保留 active 内容，不保留长历史 |
| `docs/ARCHITECTURE_GUARDRAILS.md` | 架构边界、依赖方向、canonical owners | 边界变化时必须更新 |
| `docs/checklists/*.md` | 实施前 / review 时的执行清单 | 规则或协作方式变化时更新 |
| `*/MODULE.md` | 模块职责、owner、不变量、常见坑 | 只写模块级高价值信息 |
| `docs/plans/*.md` | 临时设计 / 实施方案 | 完成后要归档、收敛或 supersede |
| `docs/decisions/ADR-*.md` | 长期有效的边界决策 | 不记录普通实现步骤 |
| `docs/templates/*` | 通用模板与可复制手册 | 作为其他仓库或新模块的启动骨架 |

## Current Source Of Truth

- 仓库画像: [PROJECT_PROFILE.md](./PROJECT_PROFILE.md)
- 版本需求基线: [PRD-v1.0.0.md](./PRD-v1.0.0.md)
- 当前目标与执行状态: [roadmap.md](./roadmap.md)
- 架构边界: [ARCHITECTURE_GUARDRAILS.md](./ARCHITECTURE_GUARDRAILS.md)
- 实施 / review 清单: [checklists/review-checklist.md](./checklists/review-checklist.md)
- Scripted smoke: [checklists/scripted-smoke.md](./checklists/scripted-smoke.md)
- Windows 打包 smoke: [checklists/windows-package-smoke.md](./checklists/windows-package-smoke.md)
- plans 快速索引: [plans/README.md](./plans/README.md)
- 通用模板入口: [templates/AI_REPO_BOOTSTRAP_PLAYBOOK.md](./templates/AI_REPO_BOOTSTRAP_PLAYBOOK.md)

## Known Historical Plans

- [2026-03-23-modular-architecture-design.md](./plans/2026-03-23-modular-architecture-design.md)
  - 这是早期 `Deskflow` 命名阶段的模块化设计，现已降级为历史指针文档，不再作为 active source of truth
  - manager 模块入口以 `easyflowhub-app/src/modules/MODULE.md` 为准
  - 扩展平台演进以 `2026-04-11-vscode-style-extension-platform-roadmap.md` 与 `2026-04-08-extension-enhancement-plan.md` 为准

## Doc Hygiene Rules

- 文档只写源码和工具不容易直接推导的信息
- 一项长期边界只保留一个权威文档入口
- `PRD` 负责版本需求边界，`roadmap` 负责当前执行与验证状态
- 已过期大文档不要继续修补，优先缩成指针文档或归档
- 每次非平凡改动至少同步 `roadmap`、相关 `MODULE.md`、`ADR`、`plan` 中的一项
