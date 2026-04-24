# 2026-04-24 v1.0.0 任务看板

> status: active
> owner: EasyFlowHub maintainers
> related_prd: `docs/PRD-v1.0.0.md`
> related_delivery_plan: `docs/plans/2026-04-24-v1-delivery-plan.md`
> related_roadmap_item: `v1.0.0 / Phase 1-4`

## 使用方式

- 每张任务卡都应有一个主 owner，不建议跨 TS / Rust / Go 混成单卡实现。
- `Status` 只使用：`todo`、`in_progress`、`blocked`、`done`。
- 开始前先确认影响模块、依赖任务、验证方式、要同步的文档。
- 完成后把结果回填到本文件，并同步 `roadmap.md` 的 Recent Progress 或 Validation Debt。

## 看板概览

| ID | Title | Phase | Priority | Status | Depends On |
|---|---|---|---|---|---|
| T1 | Quick note 关闭行为回归 | P1 | P0 | `in_progress` | - |
| T2 | Quick note 图片与旧笔记兼容回归 | P1 | P0 | `in_progress` | - |
| T3 | Todo 闭环主路径回归 | P1 | P0 | `in_progress` | - |
| T4 | Windows 打包产物行为复验 | P1 | P0 | `in_progress` | T1 |
| T5 | Notes manager 一致性 smoke | P1 | P0 | `in_progress` | - |
| T6 | 标准样例脚本固定 | P2 | P1 | `todo` | T4 |
| T7 | 脚本执行记录链路复验 | P2 | P1 | `todo` | T6 |
| T8 | 模块启停持久化回归 | P2 | P1 | `todo` | - |
| T9 | 快捷键与自启动设置回归 | P2 | P1 | `todo` | T4 |
| T10 | Relay 真实上游回归 | P3 | P1 | `todo` | T4 |
| T11 | Relay 密钥配置收口 | P3 | P1 | `todo` | T10 |
| T12 | MCP 只读 catalog 回归 | P3 | P2 | `todo` | T10 |
| T13 | Extension contribution 视图收口 | P3 | P1 | `todo` | T10 |
| T14 | v1.1.0 范围冻结 | P3 | P1 | `todo` | T13 |
| T15 | Final smoke run | P4 | P0 | `todo` | T1,T2,T3,T4,T5,T6,T7,T8,T9,T10,T11,T12,T13,T14 |
| T16 | 文档最终同步 | P4 | P0 | `todo` | T15 |
| T17 | 剩余风险审查 | P4 | P0 | `todo` | T15 |
| T18 | 发版交接摘要 | P4 | P1 | `todo` | T16,T17 |

## 任务卡

### T1. Quick note 关闭行为回归

- Status: `in_progress`
- Owner scope: `easyflowhub-app/src/pages`, `easyflowhub-app/src-tauri/src`
- Goal: 确认 `Esc`、`Alt+F4`、工具按钮都只关闭当前 quick note 窗口。
- Progress:
  - 已新增 `QuickNotePage.test.tsx`，覆盖 `Escape` 关闭与 `destroy()` 失败后回退 `close_window`。
  - 仍缺 `Alt+F4` 和打包产物下的人工复验。
- Deliverables:
  - 关闭路径行为说明
  - 必要代码修复或回归记录
  - 打包产物下的复验结果
- Verification:
  - 人工验证三种关闭路径
  - 如涉及逻辑修复，补最小测试或说明无法自动化原因
- Docs to sync:
  - `docs/roadmap.md`

### T2. Quick note 图片与旧笔记兼容回归

- Status: `in_progress`
- Owner scope: `easyflowhub-app/src/pages`, `easyflowhub-app/src-tauri/src/notes.rs`
- Goal: 确认图片插入、删除、预览、大图展示与旧笔记数据兼容。
- Progress:
  - 已新增 `useEditorImageInsertion.test.tsx`，覆盖粘贴图片后的保存与 `onImageSaved` 元数据回传。
  - 已扩展 `imageAssets.test.ts`，覆盖 legacy full asset URL 提取。
  - 已新增 `QuickNotePage.test.tsx`，覆盖 legacy 图片加载后的移除行为。
  - 根据 `2026-04-24` 人工验收反馈，已补 quick note 图片双击放大预览，并新增对应页面级回归。
  - 仍缺打包态人工回归与安装器路径复验。
- Deliverables:
  - 回归用例清单
  - 问题修复或通过记录
  - 兼容性结论
- Verification:
  - 人工图片流转验证
  - 旧笔记样本重开验证
- Docs to sync:
  - `docs/roadmap.md`

### T3. Todo 闭环主路径回归

- Status: `in_progress`
- Owner scope: `easyflowhub-app/src/pages/TodoCardPage.tsx`, `easyflowhub-app/src/components/manager/todos`
- Goal: 完成“创建 -> 聚合 -> 勾选/编辑 -> 回跳”的完整验证。
- Progress:
  - 已新增 `TodoCardPage.test.tsx`，覆盖勾选待办后的保存内容、打开原笔记时的 manager 导航，以及创建待办箱并保存新任务。
  - 已新增 `TodoPanel.test.tsx`，覆盖 manager 侧勾选待办、来源跳转与悬浮卡片入口。
  - 剩余主要是 manager + todo card 的人工闭环回归。
- Deliverables:
  - 主路径回归记录
  - 如存在断点，明确是 UI、解析还是导航问题
- Verification:
  - 人工端到端 smoke
  - 必要时补现有前端测试
- Docs to sync:
  - `docs/roadmap.md`

### T4. Windows 打包产物行为复验

- Status: `in_progress`
- Owner scope: `easyflowhub-app/src-tauri/src`, `scripts`, `docs/checklists`
- Goal: 确认托盘、自启动、多窗口、关闭/隐藏行为在打包产物中正常。
- Progress:
  - 已细化 `windows-package-smoke.md` 的 quick note / todo 检查项，补入图片重开、legacy asset、移除图片一致性和“只关闭当前窗口”要求。
  - 已执行 `check-release-artifacts.ps1`，确认 NSIS / MSI / portable 产物存在。
  - 已新增 `windows-package-smoke-2026-04-24.md` 记录本轮预检结果。
  - 已根据人工验收问题完成一轮修复并重新构建 `EasyFlowHub.exe` / NSIS / MSI 新产物。
  - 仍缺修复后的真实打包产物人工复验。
- Deliverables:
  - package smoke 结果
  - 发现的问题与阻塞清单
- Verification:
  - `docs/checklists/windows-package-smoke.md`
  - 必要时 `scripts/check-release-artifacts.ps1`
- Docs to sync:
  - `docs/roadmap.md`
  - `docs/checklists/windows-package-smoke.md`

### T5. Notes manager 一致性 smoke

- Status: `in_progress`
- Owner scope: `easyflowhub-app/src/components/manager`, `easyflowhub-app/src-tauri/src/notes.rs`
- Goal: 完成 CRUD、删除、恢复、重开窗口数据一致性回归。
- Progress:
  - 已新增 `NotesPanel.test.tsx`，覆盖 `MANAGER_OPEN_NOTE_EVENT` 打开指定笔记，以及关闭编辑器时 `flushSave` 当前草稿。
  - 已根据人工验收反馈补 manager 含图笔记回归：打开时正文与附件图片分离展示，关闭保存时重新合并回原始 markdown 存储格式，并补测试固定。
  - 已新增 `TrashPanel.test.tsx`，覆盖单条恢复、批量恢复和清空回收站。
  - 删除、恢复、重开窗口一致性仍需人工 smoke 关闭。
- Deliverables:
  - 一致性回归结论
  - 是否存在升级或重开后异常
- Verification:
  - manager 人工 smoke
- Docs to sync:
  - `docs/roadmap.md`

### T6. 标准样例脚本固定

- Status: `todo`
- Owner scope: `scriptmgr-go/internal`, `scripts`
- Goal: 固定一个发版级 scripts 样例与最短验证步骤。
- Deliverables:
  - 样例脚本选择结果
  - 执行步骤与预期结果
- Verification:
  - `scripts/run-smoke.ps1`
  - 样例脚本手工执行
- Docs to sync:
  - `docs/checklists/scripted-smoke.md`
  - `docs/roadmap.md`

### T7. 脚本执行记录链路复验

- Status: `todo`
- Owner scope: `easyflowhub-app/src/components/manager/scripts`, `easyflowhub-app/src/components/manager/tasks`, `scriptmgr-go/internal`
- Goal: 确认列表、执行、历史/时间线视图可闭环。
- Deliverables:
  - 执行链路回归记录
  - 如失败，明确是 discover、run、record 还是 UI 展示问题
- Verification:
  - manager 人工 smoke
  - 必要时 Go 测试
- Docs to sync:
  - `docs/roadmap.md`

### T8. 模块启停持久化回归

- Status: `todo`
- Owner scope: `easyflowhub-app/src/modules`, `easyflowhub-app/src/components/manager/SettingsPanel.tsx`
- Goal: 确认模块开关持久化、核心模块保护和重开后状态恢复。
- Deliverables:
  - 模块开关回归结果
  - 如有问题，明确 registry / settings / UI owner
- Verification:
  - 现有前端测试
  - manager 人工 smoke
- Docs to sync:
  - `docs/roadmap.md`

### T9. 快捷键与自启动设置回归

- Status: `todo`
- Owner scope: `easyflowhub-app/src-tauri/src/settings.rs`, `easyflowhub-app/src/components/manager/SettingsPanel.tsx`
- Goal: 确认关键设置修改后持久化，并在重开或系统重启场景下行为一致。
- Progress:
  - 依赖 `T4` 的真实 Windows 打包态人工复验；当前仅完成产物预检。
- Deliverables:
  - 设置回归记录
  - 已知限制或平台依赖说明
- Verification:
  - manager 人工 smoke
  - Windows 行为复验
- Docs to sync:
  - `docs/roadmap.md`

### T10. Relay 真实上游回归

- Status: `todo`
- Owner scope: `scriptmgr-go/internal/relay`
- Goal: 覆盖 stream、`429`、`5xx`、timeout、鉴权差异。
- Deliverables:
  - 测试覆盖结果
  - 失败模式说明
- Verification:
  - `go test ./...` 或 relay 相关包测试
- Docs to sync:
  - `docs/roadmap.md`

### T11. Relay 密钥配置收口

- Status: `todo`
- Owner scope: `scriptmgr-go/internal/relay`, `easyflowhub-app/src/components/manager/relay`
- Goal: 收敛到 `api_key_env` 优先路径，减少明文配置依赖。
- Deliverables:
  - 配置策略结论
  - manager 提示或文档同步
- Verification:
  - relay 配置回归
  - manager 人工验证
- Docs to sync:
  - `README.md`
  - `docs/roadmap.md`

### T12. MCP 只读 catalog 回归

- Status: `todo`
- Owner scope: `scriptmgr-go/internal/mcpcli`, `easyflowhub-app/src/components/manager/mcp`
- Goal: 确认 persisted / extension / conflicted 三类条目展示稳定。
- Deliverables:
  - catalog 展示回归记录
  - 冲突态表现结论
- Verification:
  - manager MCP 面板人工验证
  - 必要时 Go 测试
- Docs to sync:
  - `docs/roadmap.md`

### T13. Extension contribution 视图收口

- Status: `todo`
- Owner scope: `scriptmgr-go/internal/extensions`, `easyflowhub-app/src/components/manager/extensions`
- Goal: 收口聚合视图、只读审计、导入入口和边界文案。
- Deliverables:
  - 视图回归记录
  - 受控边界文案确认
- Verification:
  - manager 人工验证
  - 现有前端测试
- Docs to sync:
  - `docs/roadmap.md`
  - 相关 plan

### T14. v1.1.0 范围冻结

- Status: `todo`
- Owner scope: `docs`
- Goal: 明确安装/卸载、热重载、签名验证等进入 `v1.1.0`。
- Deliverables:
  - `v1.1.0` 承接项清单
  - 与 `v1.0.0` 边界的一致口径
- Verification:
  - 文档 review
- Docs to sync:
  - `docs/PRD-v1.0.0.md`
  - `docs/roadmap.md`

### T15. Final smoke run

- Status: `todo`
- Owner scope: `scripts`, `docs/checklists`
- Goal: 执行最终默认 smoke 与 package smoke。
- Deliverables:
  - smoke 结果
  - 阻塞项列表
- Verification:
  - `scripts/run-smoke.ps1`
  - `docs/checklists/windows-package-smoke.md`
- Docs to sync:
  - `docs/roadmap.md`

### T16. 文档最终同步

- Status: `todo`
- Owner scope: `README.md`, `docs/*`
- Goal: 保证 PRD、roadmap、README、checklist、相关 plan 口径一致。
- Deliverables:
  - 文档同步完成
  - 对外入口无冲突
- Verification:
  - docs review
- Docs to sync:
  - `README.md`
  - `docs/README.md`
  - `docs/roadmap.md`
  - 必要 plan / MODULE

### T17. 剩余风险审查

- Status: `todo`
- Owner scope: `docs`
- Goal: 形成剩余风险、可接受限制和发版后首批跟进项。
- Deliverables:
  - 风险列表
  - `v1.1.0` 首批 backlog
- Verification:
  - 文档 review
- Docs to sync:
  - `docs/roadmap.md`

### T18. 发版交接摘要

- Status: `todo`
- Owner scope: `docs`
- Goal: 形成对内交接摘要或 release note 草稿。
- Deliverables:
  - 交付结果摘要
  - 已验证项 / 未验证项 / 已知限制
- Verification:
  - review
- Docs to sync:
  - 新增 release summary 或在现有文档中补总结

## 建议泳道

### 立即开始

- T1
- T2
- T4

### 可并行推进

- T3
- T5
- T8

### 在 `CP1` 接近关闭后启动

- T6
- T7
- T9
- T10
- T13

### 发版收口

- T15
- T16
- T17
- T18

## 完成定义

- 任务状态从 `todo` 改为 `done` 前，必须写明验证方式。
- 如果任务没有完成但可以阶段性停止，状态改为 `blocked`，并写出 blocker。
- 所有 `P0` 任务完成前，不应宣布 `v1.0.0` ready。
