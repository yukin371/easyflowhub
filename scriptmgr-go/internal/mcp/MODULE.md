# internal/mcp MODULE

> status: active
> owner: EasyFlowHub script runtime maintainers
> last_verified: 2026-04-07
> verified_against: e524f8522ecf0a15e40af3e6f7627a34e319e81d

## 1. Responsibility

`internal/mcp` 是 `scriptmgr` 的 MCP 服务端协议层，负责 JSON-RPC / MCP request handling、tool schema、动态工具路由和任务通知。

## 2. Owns

This module is the canonical owner of:

- MCP request / response / notification 结构
- `initialize`、`tools/list`、`tools/call` 协议处理
- static tools 与 dynamic `script_*` tool routing
- 参数 schema 与运行时校验
- async task completion notifications

## 3. Must Not Own

This module must not contain:

- 脚本发现核心逻辑
- 脚本执行引擎
- notes repo 的持久化实现
- 外部 MCP client 行为（归 `internal/mcpcli`）

## 4. Entry Points

| Entry point | Role | Notes |
|---|---|---|
| `server.go` | protocol surface | MCP server、tool registration、notifications |
| `router.go` | dynamic tool routing | progressive disclosure、category load / unload |
| `schema.go` | argument validation | tool schema 与参数校验 |
| `transport/*` | transport bridge | stdio 等消息传输实现 |

## 5. Key Dependencies

| Dependency | Why it exists | Risk if changed |
|---|---|---|
| `internal/api` / `model` | 提供脚本与任务数据 | API 返回形状变化会直接影响 tool output |
| `internal/notes` | notes MCP tools | notes sync 行为变化需同步 schema 与 handler |
| `router.go` + `schema.go` | 动态工具与校验配对 | schema / handler 漂移会导致工具表面存在但调用失败 |

## 6. Dependents

Main callers / consumers:

- `cmd/scriptmgr` 的 `mcp` 命令
- Tauri / Claude / Codex 等 MCP 客户端
- `internal/http` 中与 loaded categories 相关的状态展示

## 7. Invariants

These must remain true after changes:

- `tools/list` 默认只暴露 discovery / static tools，脚本工具需经 `load_category`
- tool 参数必须在 handler 前经过 schema 校验
- 无 transport 时 notification 应 graceful degrade，而不是破坏执行路径
- 协议层只做路由和适配，不复制脚本业务逻辑

## 8. Common Pitfalls

- 新增 tool handler 但忘记同步 schema
- 在协议层直接补业务逻辑，绕过 `api` / `executor`
- 忘记考虑 async task 的结果读取与日志读取配套能力

## 9. Reuse Rules

Before adding new code here, check:

- 是否已有 static tool 或 category tool 机制可扩展
- 新需求属于 MCP 服务端还是 `mcpcli` 客户端
- 是否应先扩展 `api` / `notes` 层，再在协议层暴露

## 10. Verification

- Primary verification: `cd scriptmgr-go && go test ./...`
- Secondary verification: `cd scriptmgr-go && go vet ./...`

## 11. Doc Sync Triggers

Update this file when any of these change:

- MCP 协议职责
- dynamic tool loading 规则
- validation invariants
- notes / tasks / notifications 入口
- 常见坑
