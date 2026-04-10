# manager mcp MODULE

> status: active
> owner: EasyFlowHub frontend maintainers
> last_verified: 2026-04-11

## 职责

`src/components/manager/mcp` 负责展示 scriptmgr MCP 服务状态、动态类别管理，以及 persisted MCP config 与 extension `mcp_servers` 的只读 effective catalog。

## 数据流

manager panel -> `src/lib/api/scriptmgr` -> `/api/mcp/categories` + `/api/mcp/servers` -> 渲染类别开关和 external MCP server catalog

## 约定 & 陷阱

- 此模块只做运营和可视化，不直接改写 `.scriptmgr/mcp-config.json`。
- extension `mcp_servers` 在当前阶段只应显示为只读 overlay；冲突项必须显式标记，不能静默覆盖 persisted config。
- `internal/mcp` 与 `internal/mcpcli` 是两条 owner：前者是协议层，后者是外部 server catalog，前端不要混淆。
