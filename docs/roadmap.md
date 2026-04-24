# EasyFlowHub Roadmap

## v1.0.0

### Source Of Truth

- 正式需求基线：[PRD-v1.0.0.md](./PRD-v1.0.0.md)
- 项目画像：[PROJECT_PROFILE.md](./PROJECT_PROFILE.md)
- 架构边界：[ARCHITECTURE_GUARDRAILS.md](./ARCHITECTURE_GUARDRAILS.md)
- 交付拆解计划：[plans/2026-04-24-v1-delivery-plan.md](./plans/2026-04-24-v1-delivery-plan.md)
- 任务看板：[plans/2026-04-24-v1-task-board.md](./plans/2026-04-24-v1-task-board.md)

### Current Goal

- 先完成 quick note、notes manager、todo、scripts、manager/settings 的正式发布闭环。
- 把 Windows 桌面关键路径和打包产物行为收敛到可验证、可交接、可回归。
- 以受控方式纳入 relay / MCP / extensions，不让高级能力反向阻塞 v1 核心发布。

### Phase Plan

| Phase | Status | Goal | Key scope | Gate |
|---|---|---|---|---|
| Phase 1. Experience Stability | `[in_progress]` | 稳定最核心的桌面体验 | quick note、notes manager、todo、窗口关闭、自启动、托盘 | `CP1` |
| Phase 2. Automation Baseline | `[in_progress]` | 收口 scripts / tasks / settings 的可验证主路径 | 脚本发现、执行、记录、模块启停、关键设置持久化 | `v1 baseline` |
| Phase 3. Controlled Advanced Capabilities | `[in_progress]` | 让 relay / MCP / extensions 以受控范围进入 v1 | relay 回归、MCP catalog、extension contribution aggregation | `CP2` |
| Phase 4. Release Governance | `[in_progress]` | 完成发版前验证和文档同步 | smoke、checklist、CI、README / docs 对齐 | `CP3` |

### Active Tracks

#### Track A. Core Experience

- owner scope: `easyflowhub-app/src`, `easyflowhub-app/src-tauri/src`
- 重点：收尾 quick note、notes manager、todo、Windows 多窗口与关闭行为。
- 当前任务：
  - `V1-A1` `[in_progress]` quick note Markdown / 图片 / 历史兼容回归
  - `V1-A2` `[in_progress]` todo 聚合、回跳、端到端闭环回归
  - `V1-A3` `[in_progress]` autostart、托盘、关闭/隐藏行为在打包产物中复验

#### Track B. Automation Workspace

- owner scope: `easyflowhub-app/src/components/manager`, `easyflowhub-app/src/modules`, `scriptmgr-go/internal`
- 重点：让 scripts、tasks、settings 不只是“有入口”，而是“有标准验证主路径”。
- 当前任务：
  - `V1-B1` `[in_progress]` 固定标准脚本样例，验证列表、执行、记录/时间线
  - `V1-B2` `[in_progress]` manager 模块启停、快捷键、自启动设置持久化回归
  - `V1-B3` `[planned]` 汇总 scripts / manager 的发布前手工 smoke 结果

#### Track C. Controlled Integrations

- owner scope: `scriptmgr-go/internal/relay`, `scriptmgr-go/internal/extensions`, `scriptmgr-go/internal/mcpcli`, `easyflowhub-app/src/components/manager/{relay,mcp,extensions}`
- 重点：把 relay / MCP / extensions 保持在“受控增强”范围，不膨胀成平台级阻塞项。
- 当前任务：
  - `V1-C1` `[in_progress]` relay 真实上游回归：stream、`429`、`5xx`、timeout、鉴权差异
  - `V1-C2` `[in_progress]` provider 密钥从明文 JSON 继续收敛到环境变量或独立密钥层
  - `V1-C3` `[in_progress]` extensions contribution aggregation 的前端真实消费与审计视图收口
  - `V1-C4` `[planned]` 明确 extensions Phase 2+ 留到 `v1.1.0`，不作为 `v1.0.0` 阻塞项

#### Track D. Release Governance

- owner scope: `docs`, `.github/workflows`, `scripts`
- 重点：让 PRD、roadmap、README、checklist 与当前交付面一致。
- 当前任务：
  - `V1-D1` `[done]` Windows package smoke checklist 与 scripted smoke 已建立
  - `V1-D2` `[in_progress]` README / PRD / roadmap 同步到正式 v1 范围
  - `V1-D3` `[in_progress]` 每轮 checkpoint 后回填验证结果与剩余风险

### Release Checkpoints

#### CP1. Experience Stability Gate `[active]`

- quick note 在 `Esc`、`Alt+F4`、工具按钮关闭下都只关闭当前窗口。
- quick note 图片插入、删除、大图展示与旧笔记兼容完成回归。
- todo 从“笔记创建 -> 聚合展示 -> 勾选/编辑 -> 回跳”没有明显断裂。
- autostart、托盘、多窗口行为在 Windows 打包产物下验证通过。

#### CP2. Controlled Integration Gate `[active]`

- relay 至少完成一轮真实上游回归，覆盖 stream、`429`、`5xx`、timeout。
- manager Relay 面板在服务未启动、运行时未启动、非法 JSON 下有可理解反馈。
- provider API key 不继续依赖推荐的明文 JSON 配置方式。
- extensions / MCP 在 v1 范围内保持“只读聚合或受限导入”，不升级为完整平台承诺。

#### CP3. Release Governance Gate `[queued]`

- 至少一条标准验证链可被团队重复执行。
- roadmap、PRD、README、相关 `MODULE.md` 与当前代码 owner 不冲突。
- 打包 smoke 与默认 scripted smoke 的结果已回填。

### Validation Debt

- quick note 的 `Esc` / `Alt+F4` / 工具按钮关闭回归仍需在打包产物中复验。
- 最新人工验收已暴露并修复一轮图片放大、撤销、manager 图片展示问题，但仍缺修复后的 Windows 二次复验。
- notes manager 仍缺一次发布前 CRUD / 回收 / 恢复 / 重开窗口一致性回归。
- scripts 仍缺发版级标准样例脚本与记录链路复验。
- manager/settings 仍缺模块启停、快捷键、自启动的发布前人工验证。
- relay 仍缺真实上游场景回归与密钥层收口结论。
- Windows 产物目录当前同时出现 `DeskFlow_*` 与 `EasyFlowHub_*` 命名安装包，发版前需要确认正式交付物命名。

### Next Queue

1. 先关闭 `CP1`，因为它仍是最直接的发布阻塞项。
2. 固定 scripts / tasks / settings 的标准验证主路径，完成 `Phase 2` 收口。
3. 把 relay / MCP / extensions 的 v1 边界写死，避免在发版前继续膨胀范围。
4. 在每轮 checkpoint 后同步 README、PRD、roadmap 与 smoke 结果。

### Recent Progress

- `2026-04-24` 已基于上述修复重新完成一轮 Tauri Windows 打包，新的 `EasyFlowHub.exe`、NSIS、MSI 产物已生成，并再次通过 `scripts/check-release-artifacts.ps1` 预检；当前进入“修复后待人工复验”状态。
- `2026-04-24` 已根据人工验收反馈修复一轮图片与编辑器回归：`useHistory` 历史入栈缺陷已修复并补 `useHistory.test.tsx`；quick note 图片缩略图与 manager markdown 预览已支持双击放大；`NotesPanel` / `NoteEditor` 已改为分离正文与附件图片，避免 manager 打开含图笔记时展示原始 `asset:` markdown。
- `2026-04-24` 已继续修正 manager 预览排版细节：图片不再默认整块居中，改为与正文绕排，解决图文混排阅读断裂问题。
- `2026-04-24` 已继续修正 manager 编辑器撤销链路：正文编辑接入显式历史栈，`useShortcutEngine` 开始处理 `Ctrl+Z / Ctrl+Shift+Z`，避免删除或移动图片 markdown 时撤销失效。
- `2026-04-24` 已把人工验收发现与修复状态回填到 `docs/checklists/windows-package-smoke-2026-04-24.md`，当前仍待用户基于修复后的产物完成第二轮 Windows smoke。
- `2026-04-24` 已执行 Windows 产物预检并通过：`check-release-artifacts.ps1` 确认 NSIS / MSI / portable 产物存在，同时新增 `docs/checklists/windows-package-smoke-2026-04-24.md` 记录本轮 package smoke 预检结果与待完成人工项。
- `2026-04-24` 已把 manager 侧 todo / trash 主路径也补进自动化：新增 `TodoPanel.test.tsx` 覆盖勾选、来源跳转与悬浮卡片入口，新增 `TrashPanel.test.tsx` 覆盖单条恢复、批量恢复与清空回收站。
- `2026-04-24` 已为 todo / notes manager 补最小页面级回归：新增 `TodoCardPage.test.tsx` 覆盖待办勾选保存、打开原笔记跳转和待办箱创建；新增 `NotesPanel.test.tsx` 覆盖 `MANAGER_OPEN_NOTE_EVENT` 打开指定笔记与关闭编辑器时 `flushSave` 草稿。
- `2026-04-24` 已为 quick note 补最小自动化回归：新增 `QuickNotePage.test.tsx` 覆盖 `Escape` 关闭与 `destroy()` 失败回退 `close_window`，新增 `useEditorImageInsertion.test.tsx` 覆盖图片粘贴保存链，并扩展 `imageAssets.test.ts` 覆盖 legacy asset URL 提取与 quick note 图片移除相关约束。
- `2026-04-24` 已新增 [plans/2026-04-24-v1-task-board.md](./plans/2026-04-24-v1-task-board.md)，把 `P1-A ~ P4-D` 进一步拆成可领取任务卡、依赖关系和完成定义。
- `2026-04-24` 已新增 [plans/2026-04-24-v1-delivery-plan.md](./plans/2026-04-24-v1-delivery-plan.md)，把 `v1.0.0` 拆成 4 个交付阶段、任务包、owner 边界和验收口径。
- `2026-04-24` 新增正式 [PRD-v1.0.0.md](./PRD-v1.0.0.md)，并将 roadmap 收敛为执行状态与阶段推进视图。
- `2026-04-14` 已继续推进 manager 模块化：`SettingsPanel` 改为消费共享 `useToggleableModules()` hook，并补 `SettingsPanel.test.tsx` 固定模块渲染和开关行为。
- `2026-04-14` 已为 `manager_modules` 单一宿主入口补齐前端自动化回归，包括 `ManagerExtensionEntries.test.tsx`、`ExtensionsPanel.test.tsx`、`ManagerPage.test.tsx`。
- `2026-04-11` 已新增 builtin `Extensions` 面板，并把 extension `manager_modules` 收口为只读 entry host，不再直接注入 `src/modules` registry。
- `2026-04-11` 已推进 extension contribution aggregation 主链路：后端暴露 `/api/extensions/contributions`，并新增 effective catalog / overlay 相关消费基础。
- `2026-04-07` 已完成 Windows package smoke、scripted smoke、CI toolchain 对齐，以及 docs / checklist 的第一轮治理收口。

## Candidate Next Version

### v1.1.0

- extensions `Phase 2-4`：启停状态持久化、热重载、安装 / 卸载。
- relay 密钥管理与配置体验进一步收口，减少 raw JSON 依赖。
- MCP 外部 server 生命周期管理增强。
- 在不破坏桌面单机场景定位的前提下，再评估更强搜索或扩展生态能力。

## Maintenance Rules

- PRD 负责版本需求边界，roadmap 只保留 active 执行内容。
- 深设计细节放入 `docs/plans` 或 `docs/decisions`，不要继续堆进 roadmap。
- 完成的历史条目及时压缩，只保留仍影响执行的近期进展。
- 每轮 checkpoint 结束后，回填验证结果、剩余风险和 next queue。
