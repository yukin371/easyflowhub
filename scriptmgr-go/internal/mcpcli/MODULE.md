# internal/mcpcli MODULE

> status: active
> owner: EasyFlowHub script runtime maintainers
> last_verified: 2026-04-11

## 职责

`internal/mcpcli` 负责外部 MCP server 配置读取、CLI client wrapper，以及 persisted config 与 extension `mcp_servers` overlay 合并后的 effective catalog。

## 相关文档

- 当前版本目标与阶段顺序：`docs/roadmap.md`
- Stage B 真实消费链方案：`docs/plans/2026-04-11-stage-b-contribution-consumption-plan.md`
- MCP 协议层边界：`scriptmgr-go/internal/mcp/MODULE.md`

## 数据流

persisted MCP config + extension effective `mcp_servers` -> effective catalog -> CLI / HTTP / manager MCP panel

## 约定 & 陷阱

- `internal/mcpcli` 只管理外部 MCP server catalog，不拥有 `internal/mcp` 的协议处理。
- extension `mcp_servers` 是 read-only overlay，不自动写回 `.scriptmgr/mcp-config.json`。
- persisted config 优先；同名 extension server 需要保留为 `conflicted` 条目，而不是静默覆盖。
