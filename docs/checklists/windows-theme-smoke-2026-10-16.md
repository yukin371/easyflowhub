# Windows 毛玻璃主题实机验收记录（2026-10-16）

## 环境

- 运行方式：`bun run tauri dev`
- 前端地址：`http://localhost:1420/`
- 原生进程：`easyflowhub.exe`（Tauri debug）
- 平台：Windows

## 已验证

- [x] Tauri debug 进程成功启动，原生 EasyFlowHub 窗口可被 Windows UI Automation 发现。
- [x] 原生 Manager 窗口能加载真实本地笔记数据、组件入口和设置入口。
- [x] Manager 深色主题：冷色背景、深色玻璃面板、文字和控件可读。
- [x] Manager 浅色主题：设置页点击“浅色”后实时切换，浅色玻璃层次、边框和文字可读。
- [x] 主题状态写入 `data-theme` / `color-scheme`，Launcher 前端预览同步为浅色。
- [x] Launcher 搜索 Snow Shot 动作。
- [x] Launcher 收藏动作，并在“收藏”筛选中显示。
- [ ] Snow Shot MCP 已在「设置 → 系统」开启时，从 Launcher 点击动作可直接进入可见截图界面，而不只启动主窗口。
- [ ] Snow Shot MCP 未开启时显示设置指引且不跳转安装页；D 盘注册安装可被正常发现。
- [x] Launcher 毛玻璃窗口、搜索框、筛选标签、动作卡片和键帽布局正常。
- [x] 暗色 Manager 内容区的 `bg-white/*` 半透明残留已发现并修复，修复后重新截图确认内容区恢复深色玻璃。

## 限制与待补项

- [ ] Quick Note / Todo 在普通浏览器预览中会停留在加载态，因为它们依赖 Tauri IPC 和本地窗口标签；需要在独立原生子窗口中继续验收。
- [x] Launcher 在普通浏览器预览中执行 Snow Shot 会提示 `invoke` 不存在，这是浏览器环境限制；该预览不代表原生 MCP 调用结果。
- [ ] 实机日志显示 `Ctrl+Alt+H` 已被其他进程占用；现有代码已记录失败，后续应在设置页重新录入或改用鼠标中键验证。
- [ ] 原生窗口后续交互在本轮被物理 Escape 中断，未继续执行破坏性或状态变更操作。
- [ ] Launcher 使用全局快捷键 / 中键唤起后，不点击窗口即可直接输入并用 Esc 关闭；重复获得原生焦点时搜索状态保留。
- [ ] Launcher 浅色 / 深色实机窗口约为 `564×320`，圆角玻璃面板铺满窗口主体，只留窄透明边缘；点击该边缘不会误关，关闭按钮、Esc 与重复触发可收起。
- [ ] Launcher 标题栏拖动后窗口保持显示，按钮仍可点击，关闭请求只隐藏窗口。

## 结论

主题页面的深色 / 浅色视觉切换和 Launcher 核心交互已通过 Windows 实机预览与原生 Manager 首屏检查。Quick Note、Todo 原生子窗口、Snow Shot MCP 截图调用（需先在 Snow Shot 中开启集成）和快捷键冲突恢复仍需下一轮不被中断的 Windows smoke。
