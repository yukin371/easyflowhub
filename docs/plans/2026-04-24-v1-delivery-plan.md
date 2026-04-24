# 2026-04-24 v1.0.0 交付分阶段实施计划

> status: active
> owner: EasyFlowHub maintainers
> related_modules: `easyflowhub-app/src`, `easyflowhub-app/src-tauri/src`, `easyflowhub-app/src/modules`, `scriptmgr-go/internal`
> related_roadmap_item: `v1.0.0 / Phase 1-4`
> related_prd: `docs/PRD-v1.0.0.md`
> execution_board: `docs/plans/2026-04-24-v1-task-board.md`
> supersedes:

## 1. Problem

- 当前已经有较多功能与专题设计，但缺少一份直接对应 `v1.0.0` 的交付拆解文档。
- `PRD` 已定义版本目标与边界，但如果没有执行层拆分，团队仍会回到“按局部热点推进”的方式。
- 现有 `roadmap` 更适合记录 active 状态，不适合承载过细的阶段任务与验收编排。

## 2. Goals

- 把 `v1.0.0` 拆成可执行的 4 个交付阶段。
- 为每个阶段定义任务包、owner 边界、进入条件、退出条件和验收方式。
- 明确哪些内容必须在 `v1.0.0` 完成，哪些要明确延后到 `v1.1.0`。

## 3. Non-Goals

- 不替代专题技术方案文档，例如 relay、extension、manager 模块化细节设计。
- 不在本计划里展开到单个函数、单个测试文件级别实现说明。
- 不把 `v1.1.0+` 的未来能力重新拉回 `v1.0.0` 范围。

## 4. Current State

- `docs/PRD-v1.0.0.md` 已定义产品定位、范围、阶段和发布判定。
- `docs/roadmap.md` 已收敛为当前执行状态、checkpoint、验证债和 next queue。
- 代码中已有 quick note、notes manager、todo、scripts、relay、MCP、extensions、manager modules 等基础能力。
- 当前主要风险不是“完全没做”，而是“已有能力缺少统一发版收口和回归出口”。

## 5. Delivery Strategy

### 5.1 总体原则

| 原则 | 说明 |
|---|---|
| 核心体验优先 | Quick note、notes、todo、Windows 行为先收口，再谈高级能力 |
| 主路径优先 | 每个能力先固定一条可重复验证的主路径，而不是追求所有分支完整 |
| 受控交付 | relay / MCP / extensions 只做 `v1.0.0` 所需最小闭环，不扩平台承诺 |
| 文档同步 | PRD 定边界，roadmap 记状态，plan 管执行，README 对外同步 |

### 5.2 建议阶段顺序

| 阶段 | 建议时间盒 | 主目标 | 是否可并行 |
|---|---|---|---|
| Phase 1 | 1 周 | 关闭核心体验阻塞项 | 部分可并行 |
| Phase 2 | 1 周 | 固定 scripts / manager / settings 主路径 | 可与 Phase 1 后半段并行 |
| Phase 3 | 1 周 | 高级能力受控纳入与边界锁定 | 可并行，但不得阻塞 Phase 1/2 |
| Phase 4 | 0.5-1 周 | 发版治理、文档对齐、最终验收 | 收尾阶段为主 |

## 6. Phase Breakdown

### Phase 1. Core Experience Stability

#### 目标

- 让用户最直接感知的桌面主路径达到“日常可用”。

#### 任务包

| ID | 任务包 | 主 owner scope | 主要工作 |
|---|---|---|---|
| P1-A | Quick note close-path regression | `easyflowhub-app/src/pages`, `easyflowhub-app/src-tauri/src` | 收口 `Esc`、`Alt+F4`、工具按钮关闭行为 |
| P1-B | Quick note image + legacy note validation | `easyflowhub-app/src/pages`, `easyflowhub-app/src-tauri/src/notes.rs` | 图片插入、删除、展示、旧笔记兼容回归 |
| P1-C | Todo closed loop regression | `easyflowhub-app/src/pages/TodoCardPage.tsx`, `easyflowhub-app/src/components/manager/todos` | 创建、聚合、勾选、编辑、回跳 |
| P1-D | Windows desktop behavior | `easyflowhub-app/src-tauri/src` | autostart、tray、多窗口、关闭/隐藏行为在打包产物中复验 |
| P1-E | Notes manager consistency smoke | `easyflowhub-app/src/components/manager`, `easyflowhub-app/src-tauri/src/notes.rs` | CRUD、删除、恢复、重开一致性回归 |

#### 进入条件

- PRD 和 roadmap 范围已冻结到当前版本口径。
- Windows package smoke 与 scripted smoke 入口可用。

#### 退出条件

- `CP1` 全部条件满足。
- quick note、notes manager、todo 的人工 smoke 结果已记录。
- 打包产物下的 Windows 关键行为已复验。

#### 验收方式

- 人工 smoke 为主。
- 必要时补前端回归测试，但测试不是此阶段唯一出口。

### Phase 2. Automation Workspace Baseline

#### 目标

- 让 scripts、tasks、manager、settings 成为可重复验证的工作台主路径。

#### 任务包

| ID | 任务包 | 主 owner scope | 主要工作 |
|---|---|---|---|
| P2-A | Standard script sample | `scriptmgr-go/internal`, `scripts` | 固定发版级样例脚本与验证步骤 |
| P2-B | Script execution record validation | `easyflowhub-app/src/components/manager/scripts`, `tasks`, `scriptmgr-go/internal` | 列表、执行、记录、时间线复验 |
| P2-C | Module toggle persistence | `easyflowhub-app/src/modules`, `easyflowhub-app/src/components/manager/SettingsPanel.tsx` | 模块启停、核心模块保护、重开后状态验证 |
| P2-D | Shortcut / autostart settings validation | `easyflowhub-app/src-tauri/src/settings.rs`, `easyflowhub-app/src/components/manager/SettingsPanel.tsx` | 关键设置持久化与可回归验证 |

#### 进入条件

- Phase 1 的核心 UI 主路径已基本稳定。
- manager 模块入口和 settings 面板行为没有新增结构性变更。

#### 退出条件

- 至少一条 scripts 主路径可由团队重复执行。
- manager/settings 的关键配置修改后能重开验证。
- scripts / tasks / settings 的最短验收路径写入 checklist 或本计划结果区。

#### 验收方式

- `scripts/run-smoke.ps1`
- manager 人工 smoke
- 必要的前端测试 / 类型检查

### Phase 3. Controlled Integrations

#### 目标

- 让 relay / MCP / extensions 在 `v1.0.0` 中成为受控增强，而不是不封顶工程。

#### 任务包

| ID | 任务包 | 主 owner scope | 主要工作 |
|---|---|---|---|
| P3-A | Relay upstream regression | `scriptmgr-go/internal/relay` | stream、`429`、`5xx`、timeout、鉴权差异回归 |
| P3-B | Relay config hygiene | `scriptmgr-go/internal/relay`, `easyflowhub-app/src/components/manager/relay` | `api_key_env` 路径收口，减少明文配置依赖 |
| P3-C | MCP manager read-only validation | `scriptmgr-go/internal/mcpcli`, `easyflowhub-app/src/components/manager/mcp` | effective catalog、冲突态、只读展示回归 |
| P3-D | Extension contribution consumption closure | `scriptmgr-go/internal/extensions`, `easyflowhub-app/src/components/manager/extensions` | 聚合视图、只读审计、导入入口与边界文案收口 |
| P3-E | Scope freeze for v1.1.0 | `docs` | 把 install/uninstall、hot reload、signature 等明确排到下一版本 |

#### 进入条件

- Phase 1 不再存在明显发布阻塞项。
- 团队对 `v1.0.0` 不做完整扩展平台已有明确共识。

#### 退出条件

- `CP2` 条件满足。
- 高级能力边界已通过 PRD / roadmap / README 对齐。
- `v1.1.0` 承接项已被明确写出。

#### 验收方式

- Go 自动化测试
- manager 人工异常场景验证
- 文档边界一致性 review

### Phase 4. Release Governance And Final Sign-Off

#### 目标

- 完成版本收尾，确保交付口径一致、验证可追溯、文档可接手。

#### 任务包

| ID | 任务包 | 主 owner scope | 主要工作 |
|---|---|---|---|
| P4-A | Final smoke run | `scripts`, `docs/checklists` | 执行默认 scripted smoke 与 package smoke |
| P4-B | Docs sync | `README.md`, `docs/*` | README、PRD、roadmap、相关 MODULE/plan 同步 |
| P4-C | Residual risk review | `docs` | 列出未验证项、可接受限制、发版后优先跟进项 |
| P4-D | Version handoff summary | `docs` | 形成一份交付结论或 release note 草稿 |

#### 进入条件

- Phase 1-3 的退出条件已基本满足。
- 不再引入大范围结构性变更。

#### 退出条件

- `CP3` 关闭。
- smoke 结果、剩余风险、下一版本候选项已回填文档。
- 仓库入口文档不存在互相冲突的版本口径。

#### 验收方式

- checklist review
- docs review
- 最终 smoke 结果回填

## 7. Ownership Boundary

| 领域 | Canonical owner | 本计划要求 |
|---|---|---|
| Quick note / notes persistence | `easyflowhub-app/src-tauri/src/notes.rs` | 不在 TS / Go 侧复制持久化 owner |
| Manager module registration | `easyflowhub-app/src/modules` | 新增面板或开关逻辑仍从 registry 进入 |
| Relay routing / failover | `scriptmgr-go/internal/relay` | manager 只做配置与展示，不复制选路逻辑 |
| Extension manifest aggregation | `scriptmgr-go/internal/extensions` | `v1.0.0` 继续坚持声明式 manifest 边界 |
| MCP external server catalog | `scriptmgr-go/internal/mcpcli` | manager 继续只读消费 effective catalog |

## 8. Suggested Execution Order

1. 先关闭 `P1-A`、`P1-B`、`P1-D`，因为它们最直接决定 `CP1`。
2. 再并行推进 `P1-C`、`P1-E` 与 `P2-A`、`P2-C`。
3. 当 `CP1` 接近关闭时，开始集中推进 `P3-A` 到 `P3-E`。
4. 所有功能边界冻结后，再进入 `P4-A` 到 `P4-D` 的发版收口。

## 9. Risks

- 团队可能在 Phase 3 中继续把 extensions 拉成平台工程，挤压核心发布时间。
- 如果只验证开发态，不验证打包产物，Phase 1 会出现假关闭。
- scripts / settings 若没有标准样例和记录方式，Phase 2 仍会停留在“功能存在但无法验收”。
- 文档如果不同步，PRD、roadmap、README 会重新分叉成多套口径。

## 10. Verification Matrix

| 场景 | 自动化 | 人工 | 必要性 |
|---|---|---|---|
| Quick note / notes / todo | 部分已有 | 必须 | 高 |
| Windows tray / autostart / package behavior | 少量脚本辅助 | 必须 | 高 |
| Scripts / tasks baseline | `scripts/run-smoke.ps1` + Go tests | 必须 | 高 |
| Relay regression | Go tests | 建议补人工异常验证 | 中高 |
| Docs / scope alignment | 无 | 必须 | 高 |

## 11. Exit Criteria Summary

- `CP1`、`CP2`、`CP3` 有明确关闭记录。
- `v1.0.0` 范围内每个核心域至少有一条标准验收主路径。
- `v1.1.0` 承接项已经被写清，不再隐性混入 `v1.0.0`。
- 仓库入口文档、执行文档和专题计划之间不再出现范围冲突。

## 12. Resolution

Fill after implementation:

- Outcome: `planned`
- Verified by: `doc review + roadmap sync`
- Docs updated: `docs/PRD-v1.0.0.md`, `docs/roadmap.md`, `docs/README.md`, `docs/plans/2026-04-24-v1-delivery-plan.md`
