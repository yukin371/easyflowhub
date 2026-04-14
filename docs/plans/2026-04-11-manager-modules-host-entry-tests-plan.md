# 2026-04-11 实施女仆2：manager_modules 单一宿主入口补测试计划

> status: done
> last_updated: 2026-04-14

## 背景
- 目标子仓库：easyflowhub
- 目标文件：`easyflowhub/easyflowhub-app/src/components/manager/ManagerExtensionEntries.test.tsx`、`easyflowhub/easyflowhub-app/src/components/manager/extensions/ExtensionsPanel.test.tsx`、`easyflowhub/easyflowhub-app/src/components/manager/ManagerPage.test.tsx`
- 目标：为 manager_modules 单一宿主入口收口行为补齐测试，避免被当作 builtin workspace 模块来源。

## 计划
1. 读取组件、侧边栏、现有测试与 modules/MODULE.md，确认约定与现有断言模式。
2. 调整/补充 ManagerExtensionEntries 相关测试，覆盖 builtin 命中、extensions 回退、详情按钮保留、非 builtin 来源四类场景。
3. 运行目标测试验证。
4. 汇总覆盖点、测试执行情况与剩余风险。

## 检查点
- 检查点 A：测试名称与断言能直接映射四个最低覆盖要求。
- 检查点 B：测试不引入对 manager_modules 变成 builtin source 的错误假设。
- 检查点 C：测试可在本地稳定运行。

## 结果
- 已完成 `ManagerExtensionEntries.test.tsx` 的四类核心覆盖：builtin 命中、`extensions` 回退、详情按钮事件、非 builtin 来源保护。
- 已补 `ExtensionsPanel.test.tsx`，覆盖只读审计目录语义、详情深链高亮、空态、`extensionsApi.contributions()` 失败降级、`extensionsApi.list()` 失败回退。
- 已补 `ManagerPage.test.tsx`，覆盖侧边栏 extension entry 触发页面级切换到 `extensions` 面板，并派发 `MANAGER_OPEN_EXTENSION_EVENT` 的联动路径。
- 已通过 `bun run vitest run src/components/manager/ManagerPage.test.tsx src/components/manager/extensions/ExtensionsPanel.test.tsx src/components/manager/ManagerExtensionEntries.test.tsx`。
- 已通过 `bunx tsc --noEmit`。

## 剩余风险
- 当前仍需人工回归 `CP2 / Stage B3`，确认真实 UI 流程中 `ManagerSidebar -> ManagerExtensionEntries` 是唯一可操作宿主入口。
- 当前工作区存在与本计划无关的 workflow / hooks / 架构图改动，提交时需要拆分范围，避免混提。

## 建议提交范围
- `docs/roadmap.md`
- `docs/plans/2026-04-11-manager-modules-single-host-entry-plan.md`
- `docs/plans/2026-04-11-manager-modules-host-entry-tests-plan.md`
- `easyflowhub-app/src/components/manager/ManagerExtensionEntries.tsx`
- `easyflowhub-app/src/components/manager/ManagerExtensionEntries.test.tsx`
- `easyflowhub-app/src/components/manager/extensions/ExtensionsPanel.tsx`
- `easyflowhub-app/src/components/manager/extensions/ExtensionsPanel.test.tsx`
- `easyflowhub-app/src/components/manager/ManagerPage.test.tsx`

## 建议排除项
- `easyflowhub-app/src/components/manager/SettingsPanel.tsx`
- `.github/workflows/release.yml`
- `.github/workflows/test.yml`
- `.githooks/commit-msg`
- `docs/ARCHITECTURE.drawio`
- `docs/ARCHITECTURE.svg`
- `.claude/`
- `.serena/`
- `easyflowhub-app/.serena/`

## 最短人工回归
1. 打开 manager，确认侧边栏 `ManagerExtensionEntries` 仍显示 extension entry，且可见“查看扩展详情”按钮。
2. 点击某个未命中 builtin 的 extension entry，确认页面切到 builtin `Extensions` 面板，而不是出现新的 sidebar workspace 模块。
3. 在 `Extensions` 面板确认 `Manager Entry Audit` 区块只展示来源、映射状态和受控边界说明，不出现“打开面板”或“查看扩展详情”类第二入口按钮。
4. 点击侧边栏中的“查看扩展详情”，确认页面切到 `Extensions` 面板并高亮对应扩展卡片。
5. 刷新 manager 页面后重复第 2-4 步，确认行为不依赖首次加载时序。
