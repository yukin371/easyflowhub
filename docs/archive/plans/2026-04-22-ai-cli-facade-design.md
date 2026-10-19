# 2026-04-22 AI CLI Facade Design

> **状态：deferred / superseded（2026-10-16）**
> 
> 本方案对应早期 v0.3 的 AI CLI First 方向，当前项目执行主线已转向 EasyFlowHub v1.0/v1.1 桌面效率工具与受控 relay / host routing。本文保留用于历史追溯，不作为当前 source of truth；如需重启该方向，应先重新评审并同步 `docs/roadmap.md` 与 PRD。

> status: draft
> owner: EasyFlowHub maintainers
> related_modules: `scriptmgr-go/internal/cli`, `scriptmgr-go/internal/api`, `scriptmgr-go/internal/mcp`, `scriptmgr-go/internal/model`
> related_roadmap_item: `v0.3 AI access simplification`
> supersedes: `2026-03-19-mcp-cli-wrapper-design.md`（仅对 AI 主接入层部分）

## 1. Problem

- Current problem: 当前 AI 接入 `scriptmgr` 主要依赖 MCP 协议层，存在 `initialize -> tools/list -> tools/call` 多轮往返、tool catalog 暴露面偏大、schema/notification 额外 token 消耗高的问题。
- User / system impact: 在 AI 高频场景下，协议开销高于真正业务调用，导致 token 成本偏高、接入说明复杂、能力发现面过宽，也不利于把高频 AI 能力收敛成稳定命令契约。
- Why now: 0.2 版本被定义为内部核心闭环验证版，0.3 正适合在不动产品 owner 的前提下，把 AI 主接入层从 MCP 切到 CLI First，为后续内部版本建立更轻量的 AI 调用面。

## 2. Goals

- 为 AI / 自动化新增统一入口：`scriptmgr ai ...`。
- 用稳定 JSON CLI 契约替代 MCP 作为 AI 主接入层，减少 token 开销与能力面暴露。
- 保持现有业务 owner 不变，不在 facade 中复制 discovery / executor / relay / notes 逻辑。
- 允许 MCP 继续作为兼容层存在，但冻结新增 AI 主路径能力。
- 为后续 smoke、回归、文档和 AI prompt 固定一套稳定命令面。

## 3. Non-Goals

- 不在 0.3 删除 `internal/mcp` 或 `internal/mcpcli`。
- 不改 manager 面板、扩展 manifest 中 `mcp_servers`、产品内部 owner 的职责分配。
- 不把现有所有 CLI 命令全部重写成 AI 子命令；只覆盖 AI 高频调用面。
- 不在 facade 层引入新的脚本执行、任务运行或 relay 路由逻辑。

## 4. Current State

- 现有 CLI 已具备基础 JSON 输出能力：`list`、`describe`、`run`、`history`、`tasks` 等命令可输出结构化 JSON，入口位于 `scriptmgr-go/internal/cli/cli.go:19`。
- 现有业务能力 owner 已明确：脚本发现/执行归 `internal/api`、`internal/discovery`、`internal/executor`；relay 归 `internal/relay`；MCP 服务端协议归 `internal/mcp`。
- `internal/mcp` 当前负责 MCP JSON-RPC、动态 tool 路由、schema 校验与 notifications，不适合继续承担“AI 最小命令面”职责。
- 仓库已存在一份旧设计 `docs/plans/2026-03-19-mcp-cli-wrapper-design.md`，但其目标是“用 CLI 包装外部 MCP server”，不是“用 CLI 替代 AI 对 scriptmgr 的主接入层”。

## 5. Proposed Change

### 5.1 Architecture

新增 AI facade 层：

```text
AI / automation
  -> scriptmgr ai <domain> <action> --json
  -> internal/cli/ai.go (facade)
  -> existing owner modules
     - scripts/tasks -> internal/api + discovery/executor/runtime
     - notes -> internal/notes / existing API owner
     - relay -> internal/relay
  -> stable JSON response
```

分层约束：

- `scriptmgr ai` 只负责命令收口、输入校验、输出契约和错误模型统一。
- 业务逻辑仍复用现有 owner，不在 facade 中平行实现。
- `internal/mcp` 保持兼容，但新 AI 主路径默认不再优先加 MCP tool。

### 5.2 Command Surface

0.3 首批 AI 命令只覆盖高频能力，推荐如下：

```bash
scriptmgr ai capabilities --json
scriptmgr ai scripts list --json [--search <term>]
scriptmgr ai scripts describe <script-id> --json
scriptmgr ai scripts run <script-id> --json [--async] [--capture-output] [-- <args...>]
scriptmgr ai tasks list --json [--status <status>] [--limit <n>]
scriptmgr ai tasks output <task-id> --json
scriptmgr ai tasks stop <task-id> --json
```

可选第二批（仅在 AI 高频确实需要时再加）：

```bash
scriptmgr ai notes list --json
scriptmgr ai notes read <note-id> --json
scriptmgr ai relay health --json
```

约束：

- 不把 `favorites`、`roots`、`sessions`、`extensions`、`mcp` 等低频或 owner 边界较重的命令直接纳入 0.3 首批。
- 不要求 `scriptmgr ai` 与现有人类 CLI 结构完全一致；AI facade 以“稳定、短、可解析”为第一原则。

### 5.3 Response Contract

所有 AI facade 命令统一输出包裹结构，而不是直接透传现有零散 JSON：

成功：

```json
{
  "ok": true,
  "data": {},
  "meta": {
    "command": "scriptmgr ai scripts list",
    "generated_at": "2026-04-22T00:00:00Z",
    "version": "0.3"
  }
}
```

失败：

```json
{
  "ok": false,
  "error": {
    "code": "SCRIPT_NOT_FOUND",
    "message": "script foo not found"
  },
  "meta": {
    "command": "scriptmgr ai scripts describe",
    "generated_at": "2026-04-22T00:00:00Z",
    "version": "0.3"
  }
}
```

统一约定：

- AI facade 的 stdout 只输出最终 JSON；stderr 只保留真正诊断信息。
- 退出码必须稳定：成功为 0，业务失败为非 0。
- `data` 中允许复用现有 model 结构，但最外层 envelope 必须统一。
- 现有 `generated_at` 可下沉到 `meta.generated_at`，避免重复字段漂移。

### 5.4 Capability Discovery

新增：

```bash
scriptmgr ai capabilities --json
```

用途：

- 给 AI 一个最小能力清单，而不是暴露完整 MCP tool catalog。
- 返回域、动作、必需参数、是否支持 async、建议消费方式。
- 文档与 prompt 只需要引用少量固定命令，不再解释 `tools/list` / `tools/call`。

推荐返回形状：

```json
{
  "ok": true,
  "data": {
    "capabilities": [
      {
        "domain": "scripts",
        "actions": ["list", "describe", "run"],
        "async": ["run"]
      },
      {
        "domain": "tasks",
        "actions": ["list", "output", "stop"]
      }
    ]
  },
  "meta": {
    "command": "scriptmgr ai capabilities",
    "generated_at": "2026-04-22T00:00:00Z",
    "version": "0.3"
  }
}
```

### 5.5 Async Task Model

`scriptmgr ai scripts run --async --json` 不再依赖 MCP notification；统一走“启动后查询”模型：

1. `run --async` 返回 task/session id、状态、输出路径。
2. AI 通过 `scriptmgr ai tasks list/output` 主动查询。
3. 如需停止任务，使用 `scriptmgr ai tasks stop`。

约束：

- 0.3 不为 AI facade 设计新的 push notification 机制。
- 现有 runtime / executor 的任务记录与输出文件继续作为事实来源。
- facade 只负责调用 owner、拼装统一 envelope，不参与发现、校验、业务决策。

## 6. Boundary Check

- Existing implementation searched: `scriptmgr-go/internal/cli/cli.go`, `scriptmgr-go/internal/api/api.go`, `scriptmgr-go/internal/model/types.go`, `scriptmgr-go/internal/mcp/MODULE.md`, `docs/plans/2026-03-19-mcp-cli-wrapper-design.md`, `docs/plans/2026-04-07-general-fullstack-ai-workflow-design.md`
- Canonical owner after change:
  - AI facade / JSON contract: `scriptmgr-go/internal/cli`
  - script discovery / execution: `internal/api`, `internal/discovery`, `internal/executor`
  - task runtime state: existing executor/runtime owner
  - MCP compatibility: `internal/mcp`
- New duplication introduced: no（前提是 facade 只编排、不复制业务逻辑）
- Architecture doc to sync: `docs/roadmap.md`（在 Candidate Next Version 或 Track 新增条目）、`scriptmgr-go/internal/cli/MODULE.md`（若不存在则新增，明确 facade 边界）

## 7. Risks

- 如果把 AI facade 做成大量细碎子命令，最后只会把 MCP tool catalog 换壳搬到 CLI，token 收益会被稀释。
- 如果直接透传现有 CLI 的各种 JSON 形状，AI facade 会失去“稳定契约”价值，后续仍然难以维护 prompt 和自动化。
- 如果在 facade 中补参数校验、输出拼装时顺手加入业务逻辑，可能造成 owner 漂移。
- MCP 与 AI CLI 并存期间，文档若不收口，团队容易继续默认走 MCP 主路径。

## 8. Verification Plan

- Go tests:
  - `scriptmgr ai capabilities --json` 输出结构固定
  - `scripts list/describe/run` envelope 与错误模型固定
  - `tasks list/output/stop` envelope 与退出码固定
  - async run -> tasks output 主链路可回归
- CLI smoke:
  - `scriptmgr ai scripts list --json`
  - `scriptmgr ai scripts run <sample> --json`
  - `scriptmgr ai scripts run <sample> --async --json`
  - `scriptmgr ai tasks list --json`
  - `scriptmgr ai tasks output <task-id> --json`
- Doc verification:
  - README / roadmap / AI 使用说明改为 CLI First，不再把 MCP 作为默认入口

## 9. Exit Criteria

- AI 高频主路径已可仅通过 `scriptmgr ai` 覆盖 scripts + tasks 场景。
- AI 使用说明可以在不提 `tools/list` / `tools/call` 的前提下完成接入。
- `scriptmgr ai` 的 envelope、错误码和 async 查询模型已稳定。
- MCP 仍可兼容存在，但新 AI 能力默认不再优先加到 `internal/mcp`。

## 10. Recommended Rollout

### Phase A — Contract First

- 新增 `scriptmgr ai` 顶层命令与统一 envelope helpers
- 落地 `capabilities`, `scripts list`, `scripts describe`
- 先固定输出形状和错误模型

### Phase B — Execution And Tasks

- 接入 `scripts run`
- 接入 `tasks list`, `tasks output`, `tasks stop`
- 固定 async run 查询闭环

### Phase C — Docs And Default Path Switch

- 更新 README / roadmap / AI 使用说明
- 将 CLI First 写成默认 AI 接入方式
- 把 MCP 标记为兼容层/legacy path

### Phase D — Optional Expansion

- 评估是否为 notes / relay 提供 AI facade 命令
- 仅在 AI 高频需要时继续扩展，避免过早膨胀命令面

## 11. Follow-Up

- 若 Phase A/B 落地顺利，再补一份实施计划文档，拆成命令契约、测试、文档切换三个子任务。
- 检查 `internal/cli` 是否需要新增 `MODULE.md`，明确 AI facade 与人类 CLI 的边界。
- 在 0.3 收尾时决定 MCP 是继续兼容保留，还是进入下一轮清退评估。

## 12. Resolution

Fill after implementation:

- Outcome: `<implemented / abandoned / superseded>`
- Verified by: `<commands / tests / manual checks>`
- Docs updated: `<paths>`
