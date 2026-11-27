# Plans Index

方案目录。方案不是长期 source of truth —— 长期边界看 [../decisions](../decisions/README.md)，当前优先级看 [../roadmap.md](../roadmap.md)。

## 生命周期

状态由**位置**表达，文件头写一行状态行——`> 状态：` 后面跟反引号包住的 token——两者必须一致。`scripts/check-plan-lifecycle.mjs` 判这件事，[ARCHITECTURE_GUARDRAILS §9](../ARCHITECTURE_GUARDRAILS.md) 记录它在哪跑。

| 目录 | token | 含义 | 进来之后 |
|---|---|---|---|
| `proposals/` | `proposed` | 想清楚了、没排期 | 可改；被采纳时移入 `active/` |
| `active/` | `active` | 当前的实施或验收依据 | 可改 |
| `completed/` | `completed` | 目标已交付 | 只改勘误 |
| `rejected/` | `rejected` | 评估后不做 | 冻结 |
| `archive/` | `archived` / `superseded` | 历史背景、被取代、deferred | 冻结，只作追溯 |

归档动作就是**移动**，不是加一句"已完成"：

```bash
git mv docs/plans/active/2026-11-25-xxx.md docs/plans/completed/2026-11-25-xxx.md
# 再把头部的 token 改成 completed
node scripts/check-plan-lifecycle.mjs   # 漏改的链接、写错的状态、没进索引的文件会逐个点名
```

规则（都由门禁判，不是约定）：

1. `docs/plans/` 根目录只允许 `README.md`；其余文件都在五个目录之一。**位置就是状态**，根目录留一个散文件等于没有状态。
2. 头部前 20 行必须有且只有一行状态行（`> 状态：` + 反引号包住的 token），token 取自上面的表。
3. `proposals/`、`active/`、`completed/`、`rejected/` 的文件名必须是 `YYYY-MM-DD-slug.md`；`archive/` 保留历史文件名（含两个 superseded 的 `scriptmgr-*.md`）。
4. 每个方案文件都必须在下面的索引里出现一次——没有被索引的方案等于没人知道它存在。
5. `superseded` 必须在头部写明取代者（`> superseded_by:` 或 `> 取代者：`），且至少有一个指向真实存在的文件。
6. 全文任何位置提到 `docs/plans/...` 路径都必须能解析（脚本注释、JSON 配置同样算），移动文件时一起改。

为什么用移动而不是在文件里标状态：文件里的状态可以永远不改，而 `active/` 会一直变长，最后没人说得清哪份还是依据。移动会让 diff 变难看，这正是它的价值——归档是一次需要解释的动作。ADR-0001 要求文档只保留当前状态，这条是它在 `docs/plans` 的落地。

## 索引

### Proposals（7）

- [2026-03-19-mcp-cli-wrapper-design.md](./proposals/2026-03-19-mcp-cli-wrapper-design.md)：MCP CLI wrapper（待实现；AI 主接入层的方向已被 relay 部分覆盖）
- [2026-03-23-phase3-scripts-panel-design.md](./proposals/2026-03-23-phase3-scripts-panel-design.md)：脚本面板 Phase 3（设计完成，未开工）
- [2026-04-07-extension-lifecycle-design.md](./proposals/2026-04-07-extension-lifecycle-design.md)：扩展安装 / 启停 / 签名 / 目录边界（未实施）
- [2026-04-07-general-fullstack-ai-workflow-design.md](./proposals/2026-04-07-general-fullstack-ai-workflow-design.md)：通用全栈 AI 工作流（已设计，待落地）
- [2026-04-08-extension-enhancement-plan.md](./proposals/2026-04-08-extension-enhancement-plan.md)：Track E 扩展增强主线（designed，未开工）
- [2026-04-11-stage-b-contribution-consumption-plan.md](./proposals/2026-04-11-stage-b-contribution-consumption-plan.md)：Stage B 贡献消费（draft）
- [2026-04-11-vscode-style-extension-platform-roadmap.md](./proposals/2026-04-11-vscode-style-extension-platform-roadmap.md)：VSCode 风格扩展平台长期蓝图（draft）

### Active（8）

- [2026-04-07-api-relay-and-extension-phase1.md](./active/2026-04-07-api-relay-and-extension-phase1.md)：relay 一期已在 `scriptmgr-go/internal/relay` 落地；extension 部分仍是这条线的依据
- [2026-04-24-v1-delivery-plan.md](./active/2026-04-24-v1-delivery-plan.md)：`v1.0.0` 四阶段交付拆解、任务包与验收口径
- [2026-04-24-v1-task-board.md](./active/2026-04-24-v1-task-board.md)：`v1.0.0` 任务卡看板（`todo` / `in_progress` / `blocked` / `done`）
- [2026-04-25-ai-host-routing-implementation-plan.md](./active/2026-04-25-ai-host-routing-implementation-plan.md)：AI host routing 实施计划
- [2026-11-24-native-runtime-redesign.md](./active/2026-11-24-native-runtime-redesign.md)：WebView → Windows 原生单进程的重构总设计（ADR-0009，G0 已过，当前 Phase 1）
- [2026-11-25-c2-ime-acceptance.md](./active/2026-11-25-c2-ime-acceptance.md)：C2 三种输入法人机验收清单（**待执行**，Phase 1 第一周硬性）
- [2026-11-25-native-quicknote-parity.md](./active/2026-11-25-native-quicknote-parity.md)：快速笔记 T0 parity 28 条 A 级判据 + 6 条允许差异（G1 的判据对象）
- [2026-11-25-phase1-quicknote-shell.md](./active/2026-11-25-phase1-quicknote-shell.md)：Phase 1 快速笔记壳：窗口与几何、键位、多窗口、透明度、数据层、交互层（1a–2l 已落地）

### Completed（20）

- [2026-03-15-mcp-implementation-plan.md](./completed/2026-03-15-mcp-implementation-plan.md)：ScriptMgr MCP 实施计划（MCP server 已落地 `scriptmgr-go/internal/mcpcli`）
- [2026-03-15-mcp-integration-design.md](./completed/2026-03-15-mcp-integration-design.md)：MCP 集成设计（已验证）
- [2026-03-17-ctrl-scroll-opacity-design.md](./completed/2026-03-17-ctrl-scroll-opacity-design.md)：Ctrl+滚轮调整透明度
- [2026-03-19-notes-mcp-sync-design.md](./completed/2026-03-19-notes-mcp-sync-design.md)：notes MCP 同步（文件仓库模式）
- [2026-04-11-manager-modules-host-entry-tests-plan.md](./completed/2026-04-11-manager-modules-host-entry-tests-plan.md)：manager 模块宿主入口测试方案
- [2026-04-11-manager-modules-single-host-entry-plan.md](./completed/2026-04-11-manager-modules-single-host-entry-plan.md)：manager 模块单宿主入口
- [2026-04-25-claude-codex-relay-switch-design.md](./completed/2026-04-25-claude-codex-relay-switch-design.md)：Claude / Codex relay 切换
- [2026-10-16-v0.2.0-product-delivery-plan.md](./completed/2026-10-16-v0.2.0-product-delivery-plan.md)：v0.2.0 产品交付计划（2026-10-21 发布）
- [2026-10-18-v0.2.0-checkpoint-summary.md](./completed/2026-10-18-v0.2.0-checkpoint-summary.md)：v0.2.0 阶段 checkpoint 与验收证据
- [2026-10-19-architecture-and-file-size-review.md](./completed/2026-10-19-architecture-and-file-size-review.md)：架构与文件规模审查（阶段 A–D 全部完成）
- [2026-11-24-window-lifecycle-and-latency-contract.md](./completed/2026-11-24-window-lifecycle-and-latency-contract.md)：P0.5 窗口生命周期 + 计时契约（静置 600 → 262 MB）
- [2026-11-25-g0-review.md](./completed/2026-11-25-g0-review.md)：G0 评审（有条件通过，退出条件 C1–C5）
- [2026-11-25-native-architecture-gate-spike.md](./completed/2026-11-25-native-architecture-gate-spike.md)：S4 架构门禁（23 项自检 + 8 次变异）
- [2026-11-25-native-shell-spike.md](./completed/2026-11-25-native-shell-spike.md)：S1 原生壳（热唤起 p95 48.9 ms）
- [2026-11-25-notes-db-compat-spike.md](./completed/2026-11-25-notes-db-compat-spike.md)：S3 数据兼容（39 项门禁检查）
- [2026-11-25-richedit-island-spike.md](./completed/2026-11-25-richedit-island-spike.md)：S2 文本岛（键入 p95 4.95 ms）
- [2026-11-26-notes-db-shared-migrator.md](./completed/2026-11-26-notes-db-shared-migrator.md)：`notes.db` 统一迁移器（批 1–3；批 4 `user_version` 未做）
- [2026-11-26-store-schema-version.md](./completed/2026-11-26-store-schema-version.md)：schema 版本号算出来而不是盖上去（六级梯子，24 项门禁）
- [2026-11-27-c1-font-not-private.md](./completed/2026-11-27-c1-font-not-private.md)：C1 字库映射（Commit 130.1 → 91.5 MB，一个字没少）
- [2026-11-27-c2-launcher-productized.md](./completed/2026-11-27-c2-launcher-productized.md)：launcher 产品化（藏到屏外的预创建窗口，p95 2.4–3.2 ms，25 项门禁）

### Rejected（1）

- [2026-03-18-appearance-framework-impl.md](./rejected/2026-03-18-appearance-framework-impl.md)：外观框架实施计划（评估后废弃，保留作为不可行路径的记录）

### Archive（10）

- [2026-03-17-appearance-framework-design.md](./archive/2026-03-17-appearance-framework-design.md)：外观框架设计（最终未按此实施）
- [2026-03-17-manager-page-redesign.md](./archive/2026-03-17-manager-page-redesign.md)：DeskFlow 管理中心重构（设计阶段记录）
- [2026-03-17-notes-manager-design.md](./archive/2026-03-17-notes-manager-design.md)：DeskFlow 笔记管理设计
- [2026-03-23-modular-architecture-design.md](./archive/2026-03-23-modular-architecture-design.md)：早期 `Deskflow` 模块化设计（superseded）
- [2026-03-25-scripts-market-v2-plan.md](./archive/2026-03-25-scripts-market-v2-plan.md)：脚本市场 v2 规划（从未开工）
- [2026-04-22-ai-cli-facade-design.md](./archive/2026-04-22-ai-cli-facade-design.md)：v0.3 AI CLI facade 设计（deferred / superseded）
- [2026-04-22-ai-cli-facade-impl-plan.md](./archive/2026-04-22-ai-cli-facade-impl-plan.md)：v0.3 AI CLI facade 实施计划（未执行）
- [UI_REQUIREMENTS.md](./archive/UI_REQUIREMENTS.md)：Tauri 版 UI 需求材料（不是方案；原生侧看 parity 清单）
- [scriptmgr-roadmap.md](./archive/scriptmgr-roadmap.md)：旧 ScriptMgr 路线（superseded 为 `docs/roadmap.md`）
- [scriptmgr-v1.0-roadmap.md](./archive/scriptmgr-v1.0-roadmap.md)：旧 ScriptMgr v1.0 路线（superseded）

## Fast Path

### Windows 原生运行时（native runtime track）

1. [../roadmap.md](../roadmap.md) 的 `Native Runtime Track`
2. [2026-11-24-native-runtime-redesign.md](./active/2026-11-24-native-runtime-redesign.md)（总设计）
3. [2026-11-25-g0-review.md](./completed/2026-11-25-g0-review.md)（G0 已批准：锁定 egui/eframe 0.31，退出条件 C1–C5）
4. [2026-11-25-c2-ime-acceptance.md](./active/2026-11-25-c2-ime-acceptance.md)（**待执行**：C2 三种输入法人机验收）
5. [2026-11-25-native-quicknote-parity.md](./active/2026-11-25-native-quicknote-parity.md)（**已批准**：T0 parity 判据）
6. [2026-11-25-phase1-quicknote-shell.md](./active/2026-11-25-phase1-quicknote-shell.md)（**进行中**：Phase 1 第一批）
7. [2026-11-26-notes-db-shared-migrator.md](./completed/2026-11-26-notes-db-shared-migrator.md)（`notes.db` 迁移器现状）
8. S1–S4 的 spike 记录：[shell](./completed/2026-11-25-native-shell-spike.md) / [richedit](./completed/2026-11-25-richedit-island-spike.md) / [notes-db](./completed/2026-11-25-notes-db-compat-spike.md) / [架构门禁](./completed/2026-11-25-native-architecture-gate-spike.md)
9. [../decisions/ADR-0009-windows-native-runtime.md](../decisions/ADR-0009-windows-native-runtime.md)

### 扩展平台 / VSCode 风格模块化

1. [../roadmap.md](../roadmap.md)
2. [2026-04-11-vscode-style-extension-platform-roadmap.md](./proposals/2026-04-11-vscode-style-extension-platform-roadmap.md)：长期平台蓝图，回答"最终要扩到哪一层"
3. [2026-04-08-extension-enhancement-plan.md](./proposals/2026-04-08-extension-enhancement-plan.md)：当前 Track E 主线，回答"下一阶段先做什么"
4. [2026-04-07-extension-lifecycle-design.md](./proposals/2026-04-07-extension-lifecycle-design.md)：安装、启停、签名、目录边界，回答"什么不能乱做"
5. [../../scriptmgr-go/internal/extensions/MODULE.md](../../scriptmgr-go/internal/extensions/MODULE.md)
6. [../../easyflowhub-app/src/modules/MODULE.md](../../easyflowhub-app/src/modules/MODULE.md)

### Relay / Host Routing

1. [../roadmap.md](../roadmap.md)
2. [2026-04-07-api-relay-and-extension-phase1.md](./active/2026-04-07-api-relay-and-extension-phase1.md)
3. [2026-04-25-ai-host-routing-implementation-plan.md](./active/2026-04-25-ai-host-routing-implementation-plan.md)
4. [../../scriptmgr-go/internal/relay/MODULE.md](../../scriptmgr-go/internal/relay/MODULE.md)

### Release Planning

1. [../PRD-v0.2.0.md](../PRD-v0.2.0.md)
2. [2026-10-16-v0.2.0-product-delivery-plan.md](./completed/2026-10-16-v0.2.0-product-delivery-plan.md)
3. [../roadmap.md](../roadmap.md)
4. [../PROJECT_PROFILE.md](../PROJECT_PROFILE.md)
5. [../PRD-v1.0.0.md](../PRD-v1.0.0.md)（既有发布治理基线）
6. 最近一次相关的专题 plan

## 维护规则

- 新方案从 `proposals/` 开始；开始实施时移入 `active/`；交付后移入 `completed/`。
- 被别的方案取代时移入 `archive/` 并把 token 改成 `superseded`，头部写 `> superseded_by:` 指向新方案。
- `archive/` 冻结：不再更新内容，只改勘误。
- 深设计细节留在本目录，不要回写进 `roadmap.md`；长期边界进 `../decisions`。
