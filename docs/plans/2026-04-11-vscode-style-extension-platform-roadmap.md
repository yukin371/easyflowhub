# 2026-04-11 VSCode 风格扩展平台路线图

> status: draft
> owner: EasyFlowHub maintainers
> related_modules: `scriptmgr-go/internal/extensions`, `scriptmgr-go/internal/relay`, `easyflowhub-app/src/modules`, `easyflowhub-app/src/components/manager`
> related_roadmap_item: `Track E. Extension Enhancement`
> supersedes:

## 1. Problem

- Current problem: 当前仓库已经有 manager builtin module toggle、manifest 扫描、relay preset 导入，但这些还只是“可扩展雏形”，距离 VSCode 风格的统一扩展平台还有明显缺口。
- User / system impact: 新能力一旦继续增长，容易在 manager、relay、script roots、MCP 接入和后续插件 API 上各做一套，形成新的 owner 漂移。
- Why now: `Track E` 已经开始规划贡献聚合、启停、热重载和安装卸载；如果不先把长期平台蓝图写清楚，Phase 1-4 很容易只解决局部需求，后续再返工。

## 2. Goals

- 明确 EasyFlowHub 的“VSCode 风格扩展”应分阶段演进，而不是一次性跳到任意代码插件。
- 把当前已设计的声明式扩展阶段，与后续的 SDK、权限、隔离宿主阶段衔接起来。
- 约束后续实现边界，避免把 manager builtin module、relay import、CLI 临时命令误当成完整插件系统。

## 3. Non-Goals

- 不承诺与 VSCode 扩展 API 兼容。
- 不在 v1.x 直接执行任意第三方代码。
- 不在当前阶段设计 marketplace、远程自动下载、远程自动更新。

## 4. Current State

- `scriptmgr-go/internal/extensions` 当前只负责 root 扫描、`plugin.json` 校验、duplicate id 检测和 contributions 暴露。
- manifest 已支持的 contribution points 包括：`relay_providers`、`relay_routes`、`script_roots`、`mcp_servers`、`manager_modules`。
- manager 当前的 `src/modules` 只是 builtin module registry，不是通用扩展系统。
- relay 已能读取扩展列表，manager Relay 面板已能展示扩展并把 provider / route 贡献导入 `relay.json` 编辑器。
- 当前还没有：`state.json`、effective contributions、enable / disable、热重载、受控 install / uninstall、稳定扩展 SDK、隔离 extension host。

## 5. Proposed Change

- 把扩展平台拆成四层，并按层分阶段推进：

### Layer 1. Registry And State

- owner: `scriptmgr-go/internal/extensions`
- 职责：manifest 扫描、状态持久化、enabled / disabled 计算、effective contribution 聚合、目录监控
- 对应现有设计：`2026-04-08-extension-enhancement-plan.md` 的 Phase 1-4

### Layer 2. Runtime Integration

- owner:
  - relay 相关：`scriptmgr-go/internal/relay`
  - manager 相关：`easyflowhub-app/src/components/manager`
  - frontend module bridge：`easyflowhub-app/src/modules`
- 职责：把 effective contributions 接进 relay live config、manager 面板、script root 发现、MCP server 展示
- 约束：运行时只消费聚合后的只读视图，不直接猜测目录结构或自行扫描 manifest

### Layer 3. Stable Extension API / SDK

- owner: `scriptmgr-go/internal/extensions` + 后续专用 SDK 包
- 职责：定义扩展可依赖的稳定 schema、版本协商、能力声明、权限边界和 manager 嵌入约束
- 目标：让第三方知道“能贡献什么、不能贡献什么、升级如何兼容”

### Layer 4. Isolated Extension Host

- owner: 后续新增独立 extension host 模块
- 职责：当项目需要执行第三方逻辑时，提供与主进程隔离的宿主、生命周期和权限控制
- 约束：只有在声明式扩展平台稳定后才进入实施，不与当前 manifest registry 混做

## 6. Staged Roadmap

### Stage A. Declarative Platform v1（当前主线）

- 对应版本目标：`v1.0.0 -> v1.1.0`
- 交付内容：
  - `ContributionAggregator`
  - `state.json` enable / disable
  - file watching + hot reload
  - managed install / uninstall
  - manager 展示 effective status
- 完成标准：扩展系统从“扫描 manifest”升级为“可安装、可启停、可热重载的声明式平台”

### Stage B. Unified Contribution Consumption

- 目标：不只 relay 使用扩展，`script_roots`、`mcp_servers`、`manager_modules` 也接进统一 runtime
- 建议新增能力：
  - `scriptmgr` discovery 消费扩展 script roots
  - manager 为 `manager_modules` 提供受控入口，而不是把 builtin toggle 当插件系统
  - MCP 管理面板可以显示扩展贡献的 server 定义
- 完成标准：contribution points 不再停留在 schema 存在，而是有真实消费链路

### Stage C. Extension SDK And Capability Model

- 目标：形成对第三方可交付的稳定扩展契约
- 建议新增能力：
  - manifest schema versioning
  - capability / permission 声明
  - manager module mount contract
  - compatibility / deprecation policy
- 完成标准：扩展作者可以基于稳定文档和 schema 开发，而不是猜实现细节

### Stage D. Isolated Extension Host

- 进入条件：
  - Stage A 和 Stage B 已经稳定
  - 出现必须执行第三方逻辑的真实需求
  - 已有权限、签名、升级与故障隔离要求
- 建议新增能力：
  - 独立 host 进程或 sidecar
  - 生命周期：activate / deactivate / crash recovery
  - 权限沙箱和资源配额
  - 主进程与扩展宿主之间的 IPC / RPC 协议
- 完成标准：项目具备真正意义上的“可执行扩展”基础设施

## 7. Boundary Check

- Existing implementation searched: `docs/roadmap.md`, `docs/plans/2026-04-08-extension-enhancement-plan.md`, `docs/plans/2026-04-07-extension-lifecycle-design.md`, `scriptmgr-go/internal/extensions`, `scriptmgr-go/internal/relay`, `easyflowhub-app/src/modules`
- Canonical owner after change: 本文档不改变 owner；仍由 `internal/extensions` 拥有扩展 registry / 生命周期，`src/modules` 仅拥有 manager builtin modules
- New duplication introduced: no
- Architecture doc to sync: `docs/roadmap.md`

## 8. Risks

- “像 VSCode 一样”容易被误解成要立即支持任意代码插件，导致范围失控。
- 如果没有 Stage B，`manager_modules`、`script_roots`、`mcp_servers` 这些 contribution point 会继续停留在 schema 层，平台名义上完整，实际不可用。
- 如果过早进入 Stage D，会在签名、权限、崩溃隔离和升级兼容上把复杂度提前引爆。

## 9. Verification Plan

- 设计评审时检查后续扩展工作是否落在四层之一，而不是临时加平行机制。
- 每完成一个 Stage 或 Phase，都同步 `docs/roadmap.md` 和相关 `MODULE.md`。
- 在实现 Stage A / B 时，要求对应 CLI、HTTP API、manager UI 至少各有一条真实消费链和回归验证。

## 10. Exit Criteria

- 团队对“当前做的是声明式扩展平台，不是完整插件宿主”达成一致。
- `Track E` 的 Phase 1-4 与长期平台路线之间不存在冲突或双源。
- 后续新增扩展点时，可以明确判断其属于 Registry、Runtime、SDK 还是 Host 层。

## 11. Follow-Up

- 将 `Track E` Phase 1 落地为 `ContributionAggregator + EffectiveConfig()`，作为 Stage A 第一实现点。
- 在 Stage A 接近完成时，新增一份 Stage B 的详细实施计划，专门覆盖 `script_roots`、`mcp_servers`、`manager_modules` 的真实接入路径。

## 12. Resolution

Fill after implementation:

- Outcome: `<implemented / abandoned / superseded>`
- Verified by: `<commands / tests / manual checks>`
- Docs updated: `<paths>`
