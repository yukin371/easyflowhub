# 2026-04-25 Claude Codex Relay Switch Design

> status: implemented
> owner: EasyFlowHub maintainers
> related_modules: `scriptmgr-go/internal/cli`, `scriptmgr-go/internal/hostconfig`, `scriptmgr-go/internal/relay`
> related_roadmap_item: `v1.1.0 relay host configuration convergence`

## Problem

- 当前仓库已经有 `scriptmgr relay serve`，但它只覆盖 OpenAI 兼容转发，不负责 Claude / Codex 宿主配置切换。
- 用户想要的是类似 `cc-switch` 的“快速切换宿主到不同 AI 网关/中转”的能力，而不是在仓库里复制一套新的代理实现。
- 如果直接把 Claude 强接到现有 OpenAI relay，会造成协议边界错误。

## Decision

- 保持 `scriptmgr-go/internal/relay` 继续只做 OpenAI 兼容 relay。
- 新增 `scriptmgr-go/internal/hostconfig`，专门负责 Claude / Codex 宿主配置的读写、备份和原子切换。
- 在 `scriptmgr relay` 下新增 `hosts` 子命令，而不是引入新的顶层命令。

## Scope

新增命令：

```bash
scriptmgr relay hosts status [--json]
scriptmgr relay hosts switch --host claude --claude-base-url <url>
scriptmgr relay hosts switch --host codex --codex-base-url <url> [--codex-provider relay]
scriptmgr relay hosts switch --claude-base-url <anthropic-url> --codex-base-url <openai-url>
```

## Boundaries

- Claude:
  - 只修改 `~/.claude/settings.json`
  - 当前只管理 `env.ANTHROPIC_BASE_URL`
- Codex:
  - 只修改 `~/.codex/config.toml`
  - 当前只管理 `model_provider` 以及对应 provider section 的 `base_url` / `wire_api` / `requires_openai_auth`
- relay:
  - 不新增 Anthropic 协议兼容
  - 不在 `internal/relay` 中吸收宿主配置写入职责

## Why Not Full cc-switch

- 当前仓库已有 relay owner，继续复制本地 proxy / health monitor / failover 编排，会和 `internal/relay` 重叠。
- 最缺的是“宿主配置编排”，不是第二套代理服务。
- 先把配置切换做稳，比仓促复制 `cc-switch` 更符合当前 v1/v1.1 边界。

## Verification

- `cd scriptmgr-go && go test ./...`
- 手动 smoke:
  - `scriptmgr relay hosts status --json`
  - `scriptmgr relay hosts switch --host codex --codex-base-url http://127.0.0.1:8787/v1`
  - `scriptmgr relay hosts switch --host claude --claude-base-url https://gateway.example.com/anthropic`

## Follow-Up

- 后续可继续补：
  - dry-run / diff 展示
  - 更多 Claude env 字段切换
  - 统一的 host preset 概念
  - manager 侧只读展示当前 Claude / Codex 指向状态
