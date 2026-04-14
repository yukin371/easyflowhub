# 2026-04-11 manager_modules 单一宿主入口收口计划

> status: done
> last_updated: 2026-04-14
> owner: 实施女仆1
> scope: `easyflowhub-app/src/components/manager`, `easyflowhub/docs/roadmap.md`
> related_task: manager_modules 单一宿主入口收口

## 1. 目标

- 保留 `ManagerSidebar -> ManagerExtensionEntries` 作为唯一可操作宿主入口。
- 将 `ExtensionsPanel` 中重复出现的 Manager Entries 降级为只读审计信息，避免形成第二个可操作入口。
- 不把 `manager_modules` 注入 `src/modules` registry 或 builtin sidebar。

## 2. 实施步骤

1. 复核 `ManagerPage.tsx`、`ManagerSidebar.tsx`、`ManagerExtensionEntries.tsx`、`ExtensionsPanel.tsx` 的当前入口与交互边界。
2. 调整 `ExtensionsPanel.tsx`：移除可操作宿主语义，改为审计/清单展示文案，仅保留来源与映射状态说明。
3. 视需要微调 `ManagerExtensionEntries.tsx` 文案，强调其为唯一受控入口。
4. 若页面语义发生变化，同步更新 `docs/roadmap.md` 的 Recent Progress / Validation Debt。

## 3. 检查点

- CP1：侧边栏中的 `ManagerExtensionEntries` 仍可导航到已有面板或查看扩展详情。
- CP2：`ExtensionsPanel` 不再提供可被理解为第二宿主入口的 Manager Entries 交互。
- CP3：代码中未新增 `manager_modules` 到 `src/modules` 或 builtin sidebar 的注入路径。
- CP4：roadmap 文案与当前实现边界一致。

## 4. 验收

- 最低标准：页面只保留单一可操作宿主入口，且扩展总览页仍保持信息完整。
- 一般标准：审计区块能够清楚说明 entry 来源、目标面板映射状态和受控边界。

## 5. 结果

- 已完成 `ExtensionsPanel.tsx` 审计化收口：`Manager Entry Audit` 仅保留来源、映射状态与受控边界说明，不再提供第二个 manager entry 可操作入口。
- 已完成 `ManagerExtensionEntries.tsx` 文案收口，明确侧边栏是唯一受控宿主入口。
- 已同步 `docs/roadmap.md`，记录实现边界与剩余验证债。
- 已补齐 `ManagerExtensionEntries.test.tsx`、`ExtensionsPanel.test.tsx`、`ManagerPage.test.tsx`，将组件级和页面级回归约束固定下来。

## 6. 剩余风险

- `CP2 / Stage B3` 仍需人工回归，确认真实 UI 流程中侧边栏是唯一可操作宿主入口，而 `Extensions` 面板只保留只读审计目录。
- 当前仓库工作区仍存在与本任务无关的 workflow / hooks / 架构图改动，提交时需要拆分范围。
