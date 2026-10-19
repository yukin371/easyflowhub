# modules MODULE

> status: active
> owner: EasyFlowHub frontend maintainers
> last_verified: 2026-10-19

## 职责

`src/modules` 是 manager 功能模块的 canonical owner，负责内置模块定义、模块注册、启停配置持久化，以及前端消费 registry 的统一接口。

## 相关文档

- 当前版本目标与 active tracks：`docs/roadmap.md`
- manager / extension 长期演进路线：`docs/plans/2026-04-11-vscode-style-extension-platform-roadmap.md`
- Stage B 统一接入方案：`docs/plans/2026-04-11-stage-b-contribution-consumption-plan.md`
- 当前 manager builtin 模块边界以本文件为准，不以旧 `Deskflow` 模块化设计为准

## 数据流

`builtin/*` 定义模块元数据 + 面板组件 -> `registerBuiltinModules()` 注册到 `moduleRegistry` -> `moduleRegistry.loadConfig()` 合并本地启停配置 -> manager / settings 通过 `hooks.ts` 消费当前模块列表

## 约定 & 陷阱

- 新 manager 面板必须先在这里注册，再进入 `ManagerSidebar`，不要直接在 manager 壳层硬编码侧边栏项。
- Inbox 是 `notes` 模块里的工作区子视图（“收集箱 / 笔记”），不单独注册成侧栏模块；独立工具面板仍按 registry 约定注册。
- 文件检索是独立 `files` builtin 模块，业务和文件系统边界归 FilesPanel / native file_search owner；不得绕过 registry 在 Manager 壳层硬编码入口。
- `moduleRegistry` 负责模块启停和配置读写；UI 不应复制订阅或配置初始化逻辑。
- builtin module 仅描述元数据和面板组件绑定，不承载业务状态。
- `isCore` 模块不可在设置页关闭；当前核心模块为 `notes` 与 `settings`。
- `settings` 在侧边栏固定在底部（`ManagerSidebar` 单独渲染，不进 Workspace 滚动列表），图标为 `⚙`；新增模块不得抢占该位置，也不要改回列表项。
- 执行记录不再是独立模块：`tasks` 已从 builtin 列表移除，作为「脚本」页的运行记录 tab 呈现。旧 `modules.json` 中的 `tasks` 条目会被 registry 忽略，不需要迁移。
- 未来若扩展到插件式模块，先扩展 `FeatureModule` / registry 边界，不要让 builtin toggle 冒充完整插件系统。
- 当前 Stage B3 的 `manager_modules` 仍不进入 `FeatureModule` / registry；它们只允许在 manager 壳层作为只读 extension entry host 出现，并映射到已有面板。
- 内置 `extensions` 面板属于受控宿主和观测入口，本身是 builtin module，但第三方 `manager_modules` 仍不能借此冒充真正动态插件。
