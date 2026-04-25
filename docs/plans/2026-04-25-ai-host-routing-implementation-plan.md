# 2026-04-25 AI Host Routing And Relay Implementation Plan

> status: active
> owner: EasyFlowHub maintainers
> paired_prd: `docs/PRD-v1.1.0-ai-host-routing.md`
> implementation_under_review: `scriptmgr-go/internal/hostconfig`, `scriptmgr-go/internal/cli/relay.go`, `scriptmgr-go/internal/relay`

## Goal

把“Claude / Codex 宿主配置切换”和“relay / 负载均衡运行时”一起收敛成可验收交付项，并用它审视当前实现是否符合正式需求。

## Scope

- 审视现有 `relay hosts` CLI 是否满足 host config 主路径。
- 审视现有 `internal/relay` 是否满足 relay runtime 主路径。
- 明确哪些能力已经实现、哪些只是后续增强。
- 给出保留 / 补强 / 回退判断标准。

## Work Packages

### WP1. Contract Review

- 检查现有 host config 命令是否覆盖：
  - `status`
  - 只切 Claude
  - 只切 Codex
  - 同时切两者
- 检查 JSON 输出是否足够做自动化消费。
- 检查错误信息是否能区分参数缺失、解析失败、写入失败。

### WP1-B. Relay Runtime Review

- 检查现有 relay 是否覆盖：
  - OpenAI 兼容入口
  - provider config
  - route selection
  - weighted round robin
  - `429` / `5xx` / timeout 最小 failover
- 检查健康状态、配置快照、管理入口是否与当前 README/manager 描述一致。

### WP2. Safety Review

- 检查写入前是否有备份。
- 检查是否使用原子替换。
- 检查配置不存在时的新建策略是否明确。
- 检查复杂已有配置是否会被错误覆盖。

### WP3. Boundary Review

- 检查宿主配置写入是否集中在单一 owner。
- 检查 `internal/relay` 是否仍然只拥有 OpenAI 兼容 relay。
- 检查文档是否清楚写明“不把 Claude 协议硬塞进现有 relay”。
- 检查 CLI 是否只做入口与展示，不偷放 relay 选路逻辑。

### WP4. Gap Closure

- 如果 `status` 输出不足，补字段。
- 如果 `switch` 缺 dry-run / diff，仅记录为后续，不在本轮冒然扩 scope。
- 如果 relay 缺真实上游回归证据，优先补测试和 smoke 记录，而不是继续加新协议。
- 如果测试未覆盖关键风险，先补测试，再讨论 UI。

## Acceptance Checklist

- [x] `scriptmgr relay hosts status --json` 输出满足 PRD 的最小字段要求
- [x] 只切 Claude 的主路径有明确命令示例和 smoke
- [x] 只切 Codex 的主路径有明确命令示例和 smoke
- [x] 备份和原子写入有测试或稳定验证证据
- [x] relay runtime 的最小能力范围已和 PRD 对齐
- [x] `429` / `5xx` / timeout 的最小 failover 有测试或 smoke 证据
- [x] roadmap 已明确这是一项“宿主配置编排”能力，而不是“统一协议中转”
- [x] 现有实现与 PRD 的偏差已列出

## Current Assessment

基于当前实现的初步判断：

- 已满足：
  - 有统一 CLI 入口
  - 有 Claude / Codex 状态查看
  - 有分宿主切换
  - 有备份和原子写入
  - relay 已有 OpenAI 兼容入口与基础负载能力
- 仍未满足或未正式承诺：
  - preset
  - manager UI
  - 更强的熔断恢复、配额调度
  - 非 OpenAI 协议适配
  - 完整 `cc-switch` 式运行时编排

## Evidence From Current Code

### Host config evidence

- `scriptmgr-go/internal/hostconfig`
  - 已负责 Claude / Codex 配置读取、备份、原子写入
  - 当前最小支持面：
    - Claude: `~/.claude/settings.json` 的 `ANTHROPIC_BASE_URL`
    - Codex: `~/.codex/config.toml` 的 `model_provider` 与 provider `base_url`
- `scriptmgr-go/internal/cli/relay.go`
  - 已暴露 `scriptmgr relay hosts status|switch`
  - `switch` 已支持 `--dry-run` 与 `--json`

### Relay runtime evidence

- `scriptmgr-go/internal/relay/server.go`
  - 已暴露：
    - `GET /health`
    - `GET /api/relay/config`
    - `PUT /api/relay/config`
    - `GET /api/extensions`
    - `GET /api/extensions/contributions`
    - `/v1/*`
    - `/relay/v1/*`
- `scriptmgr-go/internal/relay/service.go`
  - 已实现：
    - route match by path/model
    - provider filtering by enabled + model pattern
    - weighted round robin（基于 weight 展开 + route cursor）
    - provider health 状态记忆
    - `429` / `5xx` 自动重试
    - provider timeout 后切到下一 upstream
- `scriptmgr-go/internal/relay/service_test.go`
  - 已有自动化证据覆盖：
    - upstream failure fallback
    - stream response passthrough
    - `429` retry + provider specific auth
    - explicit `5xx` retry
    - `api_key_env` 解析
    - missing env fallback
    - timeout retry
    - extension relay contributions merge / consume

## Known Gaps Against PRD

- 已有 weighted round robin，但还缺一段文档化说明，明确它是当前唯一正式承诺的负载策略。
- 目前 health 语义是“失败计数阈值下的最小健康状态”，还不是完整熔断恢复模型；PRD 已限制这点，但 README 和后续文档还需要继续避免过度承诺。
- Claude / Codex host switch 当前已经实现，但还没有 dry-run / diff，所以更适合作为维护者工具，而不是立即上升为复杂产品化体验。

## Verified In This Round

- `cd scriptmgr-go && go test ./internal/relay/...`
  - passed
- 新增显式 `5xx` failover 自动化样例：
  - `TestProxyRetriesOn5xx`
- `cd scriptmgr-go && go test ./internal/hostconfig ./internal/relay/...`
  - passed
- `cd scriptmgr-go && go test ./internal/hostconfig ./internal/cli ./internal/relay/...`
  - passed
- `go run .\cmd\scriptmgr relay hosts status --json`
  - passed
  - 已确认当前输出包含：
    - Claude / Codex 配置路径
    - base URL
    - Claude model / Codex active model
    - Codex provider
    - 宿主配置存在状态
- `go run .\cmd\scriptmgr relay hosts switch --dry-run --json --host codex --codex-base-url http://127.0.0.1:8787/v1 --codex-provider relay`
  - passed
  - 已确认 preview 输出包含结构化 diff + before / after，不落盘
- `go run .\cmd\scriptmgr relay hosts switch --dry-run --host claude --claude-base-url https://gateway.example.com/anthropic`
  - passed
  - 已确认文本预览优先显示关键字段变化，而不是整段文件内容

## Decision Gate

本轮结束后必须给出三选一结论：

1. 保留当前最小实现并继续补强
2. 保留部分能力，调整命令合同
3. 回退当前实现，仅保留 PRD 和设计文档

当前结论：`1. 保留当前最小实现并继续补强`

理由：

- host config 与 relay runtime 都已有可工作的最小实现，不是纯设计占位。
- 核心边界已经收口：宿主配置编排归 `internal/hostconfig`，relay 运行时归 `internal/relay`。
- 关键证据已具备：host status、备份/原子写入测试、`429` / `5xx` / timeout failover。
- 剩余缺口属于增强项，而不是必须回退当前实现的硬伤。

建议下一阶段只补：

1. host preset 概念
2. README / manager 侧对当前 host target 的只读展示
3. 是否需要在 `--json` 中进一步压缩完整 before / after 内容
