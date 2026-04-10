# 2026-04-11 Stage B Contribution Consumption Plan

> status: draft
> owner: EasyFlowHub maintainers
> related_modules: `scriptmgr-go/internal/extensions`, `scriptmgr-go/internal/discovery`, `scriptmgr-go/internal/mcpcli`, `easyflowhub-app/src/modules`, `easyflowhub-app/src/components/manager`
> related_roadmap_item: `Track E. Extension Enhancement`
> supersedes:

## 1. Problem

- Current problem: manifest 已声明 `script_roots`、`mcp_servers`、`manager_modules` 三类 contribution point，但当前仓库只有 relay provider / route 有部分真实消费链，其余三类仍停留在 schema 和展示层。
- User / system impact: 平台看起来“支持扩展贡献”，但真正开发时仍需要手改 `roots.json`、MCP 配置和 manager builtin modules，扩展系统无法形成闭环。
- Why now: `2026-04-11-vscode-style-extension-platform-roadmap.md` 已明确 Stage B 要解决“统一消费贡献”；如果不先把这一步设计清楚，Stage A 完成后平台仍会停留在只会扫描和列出扩展的状态。

## 2. Goals

- 让 `script_roots`、`mcp_servers`、`manager_modules` 拥有真实但受控的运行时消费路径。
- 坚持 canonical owner，不把扩展平台做成对现有 owner 的平行替代。
- 明确哪些贡献是“可直接生效的只读 merged view”，哪些贡献只是“描述符”，不能冒充完整插件能力。

## 3. Non-Goals

- 不在 Stage B 执行任意第三方前端或后端代码。
- 不把 `manager_modules` 直接升级为可挂载任意 React bundle 的动态 UI 插件系统。
- 不在 Stage B 引入 marketplace、远程下载、自动更新或 extension host。

## 4. Current State

- `scriptmgr-go/internal/extensions/types.go` 已声明 `script_roots`、`mcp_servers`、`manager_modules`。
- `internal/extensions` 当前只负责 manifest 扫描和校验，不计算 effective contributions。
- `internal/discovery` 当前脚本根目录来源只有：
  - 仓库默认目录自动发现
  - `roots.json` 持久化自定义目录
- `internal/mcpcli` 当前 MCP 服务器配置来源只有本地配置文件，不消费扩展 manifest。
- 前端 `src/modules` 当前只拥有 builtin module registry；`manager_modules` 虽有 schema，但没有真实挂载协议。
- Relay 面板当前能展示扩展 contributions 汇总，但这不代表其他 contribution points 已被消费。

## 5. Proposed Change

- Stage B 建立在 Stage A 之上：`ContributionAggregator`、`state.json`、effective contributions、enable / disable、基础 reload 必须先存在。
- Stage B 只处理“聚合后贡献的消费层”，不重复实现 registry / state。

### 5.1 Shared Rule: Consume Effective View Only

- 所有消费方只读取 `EffectiveContributions`，不自行扫描扩展目录。
- 新增统一原则：
  - static config / persisted config 仍由原 owner 管理
  - extension contributions 作为 read-only overlay 合并到运行时视图
  - 除非显式 import，否则不把扩展贡献写回用户配置文件

### 5.2 Workstream A: `script_roots`

- canonical owner: `scriptmgr-go/internal/discovery`
- 合并模型：

```text
Repo auto roots
  + user persisted roots.json
  + enabled extension script_roots
  = EffectiveScriptRoots
```

- 规则：
  - extension script roots 不写回 `roots.json`
  - disabled 扩展的 script roots 立即从 effective roots 消失
  - duplicate / nested root 继续由 discovery 层去重
  - 发现失败的 extension root 只影响该扩展项，不应让 discovery 全局失败

- API / CLI 变化建议：
  - `GET /api/scripts` 返回 roots 时使用 `EffectiveScriptRoots`
  - `scriptmgr roots` 或现有 roots 输出增加 source 标记：`repo` / `user` / `extension:<id>`
  - manager Scripts 面板后续可显示“来自扩展”的脚本根目录来源

### 5.3 Workstream B: `mcp_servers`

- canonical owner: `scriptmgr-go/internal/mcpcli` 负责外部 MCP server config；`internal/mcp` 继续只管 scriptmgr 自己的 MCP 服务端协议
- 合并模型：

```text
Persisted MCP config
  + enabled extension mcp_servers
  = EffectiveMCPServerCatalog
```

- 规则：
  - extension MCP servers 在 Stage B 先作为 read-only catalog entry，不自动写入 `.scriptmgr/mcp-config.json`
  - 如果与用户自定义 server 同名，persisted config 优先，extension entry 标记冲突
  - manager 需要区分：
    - `persisted`
    - `extension`
    - `conflicted`
  - 后续若需要“导入为本地持久配置”，应显式走 import 动作，而不是静默落盘

- API / CLI 变化建议：
  - 新增 `GET /api/mcp/servers` effective catalog
  - `scriptmgr mcp list-servers` 输出 source 与 enabled state
  - manager MCP 面板新增“扩展贡献的 MCP servers”区块，先展示与状态说明，再评估是否需要 import 按钮

### 5.4 Workstream C: `manager_modules`

- canonical owner:
  - 扩展描述符聚合：`scriptmgr-go/internal/extensions`
  - manager builtin registry：`easyflowhub-app/src/modules`
  - manager 运行时呈现：`easyflowhub-app/src/components/manager`

- 关键边界：
  - 当前 `manager_modules` schema 只有元数据，没有前端代码包、挂载协议、权限模型
  - 所以 Stage B 不把它实现成真正“动态 manager 面板”
  - Stage B 的正确目标是：先把它实现成受控的 manager extension entry，而不是可执行 UI 插件

- v1 建议落地形式：
  - 在 manager 中新增单独的 “Extensions / Contributions” 宿主区块
  - `manager_modules` 先展示为只读 entry/card，包含：
    - name
    - caption
    - icon
    - description
    - source extension
  - entry 可绑定到有限动作之一：
    - 打开扩展详情
    - 跳转到 relay / scripts / mcp 相关已有面板
    - 显示“需要 Stage C/Stage D 才能提供可执行 UI”

- 禁止事项：
  - 不直接把 `manager_modules` 注入 `src/modules` 当作真正 `FeatureModule`
  - 不允许 extension manifest 绕过 `src/modules` owner 直接控制 manager sidebar
  - 不引入从磁盘加载任意前端 bundle 的行为

## 6. Boundary Check

- Existing implementation searched: `scriptmgr-go/internal/extensions/types.go`, `internal/extensions/registry.go`, `internal/discovery/discovery.go`, `internal/api/api.go`, `internal/mcpcli/config.go`, `easyflowhub-app/src/modules`, `easyflowhub-app/src/components/manager/mcp/McpPanel.tsx`, `easyflowhub-app/src/components/manager/relay/RelayPanel.tsx`
- Canonical owner after change:
  - registry / effective contributions: `internal/extensions`
  - script roots consumption: `internal/discovery`
  - external MCP server catalog: `internal/mcpcli`
  - builtin manager modules: `src/modules`
  - manager extension entry host: `src/components/manager`
- New duplication introduced: no
- Architecture doc to sync: `docs/roadmap.md`, related `MODULE.md`

## 7. Risks

- 如果没有 strict owner，`script_roots` 很容易在 registry、discovery、HTTP API 三处各做一次 merge。
- 如果把 `manager_modules` 误做成真正动态 UI，会提前踩进 SDK、bundle 加载、权限和隔离问题。
- `mcp_servers` 如果直接写回本地配置，用户很难区分“自己配置的”与“扩展临时贡献的”来源。

## 8. Verification Plan

- Go tests:
  - effective contributions merge
  - discovery 对 extension roots 的去重与失效处理
  - MCP effective catalog 的冲突规则
- Frontend verification:
  - manager 扩展入口只显示受控 entry，不出现在 builtin sidebar
  - MCP / Scripts 面板可以区分 extension source
- Manual verification:
  - enable / disable 一个扩展后，脚本根、MCP catalog、manager entry 都会同步出现或消失

## 9. Exit Criteria

- `script_roots` 进入真实 discovery 路径，而不需要手工导入到 `roots.json`
- `mcp_servers` 有可读取的 effective catalog，并能在 manager 中看到来源和冲突状态
- `manager_modules` 有受控呈现路径，但未越权升级成任意代码插件
- 三条消费链都明确复用 Stage A 的 effective contributions，而不是重新扫描目录

## 10. Recommended Rollout

### Stage B1

- 落地 `script_roots`
- 原因：收益直接、风险最低、owner 最清晰

### Stage B2

- 落地 `mcp_servers`
- 原因：需要先确认 read-only overlay 与 persisted config 的冲突模型

### Stage B3

- 落地 `manager_modules` host entry
- 原因：最容易被误解为“动态插件”，应在边界最清楚时最后落地

## 11. Follow-Up

- 在 Stage A 实现 `GET /api/extensions/contributions` 后，新增 `EffectiveContributions` 的 TS 类型与前端 API wrapper。
- Stage B1 开工前补 `internal/discovery` 的 `MODULE.md`，把 extension script roots 的消费边界写清。
- Stage B2 开工前补 `internal/mcpcli` 或 manager MCP 模块文档，明确 external MCP catalog 的 source model。

## 12. Resolution

Fill after implementation:

- Outcome: `<implemented / abandoned / superseded>`
- Verified by: `<commands / tests / manual checks>`
- Docs updated: `<paths>`
