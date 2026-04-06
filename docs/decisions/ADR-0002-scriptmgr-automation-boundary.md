# ADR-0002 ScriptMgr Automation Boundary

- status: accepted
- date: 2026-04-07

## Context

EasyFlowHub 现在同时存在三类“自动化入口”：

- `scriptmgr-go`：面向脚本发现、执行、MCP、relay、extensions 的 Go sidecar / CLI
- `scripts/`：仓库维护脚本，例如构建和 smoke 入口
- `docs/checklists/`：人工验证、文档同步和 release 前 runbook

随着 `V1-C1`、`V1-C3` 的推进，仓库已经开始出现一个自然问题：

- 是否应该让 `scriptmgr` 继续向上接管常用验证、截图和文档同步动作，变成统一 automation 入口

这个问题不能只停留在临时讨论里，因为它直接影响：

- `scriptmgr-go` 的长期 owner 边界
- 仓库维护脚本是否继续保留在 `scripts/`
- AI 和维护者以后遇到验证、截图、文档同步时应优先进入哪里

同时存在几个约束：

- `scriptmgr-go` 当前的 canonical owner 是脚本运行时、MCP、relay 和 manifest 扩展，不是通用仓库运维
- 截图与真实 UI 验证依赖 DevTools / Playwright / 人工桌面交互，不属于 Go sidecar 的稳定职责
- 文档同步包含判断、取舍和 source-of-truth 收敛，不应伪装成一个“总能自动执行”的运行时命令

## Decision

1. 在 `v1` 阶段，`scriptmgr-go` 不接管仓库级截图、文档同步和通用维护自动化。
2. `scriptmgr-go` 继续专注于脚本运行时、MCP、relay、extensions 与相关 HTTP/CLI 接口。
3. 仓库级验证入口继续由 `scripts/` + `docs/checklists/` 负责，例如：
   - `scripts/run-smoke.ps1`
   - `scripts/check-release-artifacts.ps1`
   - `docs/checklists/scripted-smoke.md`
   - `docs/checklists/windows-package-smoke.md`
4. 只有当某类自动化同时满足下面条件时，才重新评估是否并入 `scriptmgr`：
   - 主要服务于脚本运行时本身，而不是仓库维护流程
   - 可跨仓库复用，而不是 EasyFlowHub 独有 glue code
   - 不要求浏览器会话、桌面窗口或人工判断

## Consequences

- `scriptmgr-go` 的职责不会被“统一入口”口号稀释成通用仓库工具箱。
- `scripts/` 成为仓库维护自动化的明确 owner，避免验证链再次散落。
- 截图、桌面交互、文档同步仍保留在更合适的工具和人工流程中，不强塞进 sidecar。
- 代价是维护者需要接受“不是所有动作都走同一个 CLI”这一现实，入口会分成运行时自动化与仓库自动化两类。

## Alternatives Considered

### 方案 A：让 `scriptmgr` 变成统一 automation 入口

- 不采纳原因：会把 end-user sidecar 与 maintainer-only automation 混在一起，扩大 `scriptmgr-go` 的边界，并增加 shipped binary 的非核心职责。

### 方案 B：继续只靠零散 shell 命令，不建立稳定入口

- 不采纳原因：会让验证链缺少标准命令，不利于 AI、维护者和 CI 复用，也无法支撑 roadmap 中的 release checkpoint。

## Verification

- `scripts/run-smoke.ps1` 能作为单入口执行默认验证链。
- `scripts/check-release-artifacts.ps1` 能作为打包产物预检入口。
- `docs/README.md`、`docs/PROJECT_PROFILE.md`、`docs/roadmap.md` 指向 `scripts/` 和 `docs/checklists/`，而不是把这些动作描述为 `scriptmgr` CLI 的职责。

## Follow-Up

- 如果后续确实出现跨仓库复用的 deterministic automation 能力，再新增 ADR 或 plan 评估是否为 `scriptmgr` 增加独立 automation 模块。
