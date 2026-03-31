# EasyFlowHub AI Agent Guide

> **项目**: EasyFlowHub - 桌面效率工具
> **技术栈**: React 19 + TypeScript 5 + Tauri 2 + Tailwind CSS 4 + Rust
> **最后更新**: 2026-03-31

---

## 项目概述

EasyFlowHub 是基于 Tauri 2 的桌面效率应用，提供快速笔记、待办管理、脚本执行功能。

### 核心功能模块

| 模块 | 状态 | 描述 |
|------|------|------|
| 笔记管理 | ✅ | SQLite 持久化、自动保存、标签、置顶、回收站 |
| 待办管理 | ✅ | 笔记内嵌 checkbox、全局聚合、悬浮卡片、桌面固定 |
| 脚本管理 | ✅ | 本地脚本发现、执行、执行记录时间线 |
| 设置面板 | ✅ | 模块开关、快捷键配置、窗口/编辑器/待办设置 |
| 多窗口 | ✅ | 快速笔记、待办卡片、文件夹小组件 |

---

## 目录结构

```
easyflowhub-app/
├── src/                          # React 前端
│   ├── components/
│   │   ├── manager/              # 管理中心组件
│   │   │   ├── notes/            # 笔记编辑器、Markdown 预览
│   │   │   ├── todos/            # 待办聚合面板
│   │   │   ├── mcp/              # MCP 面板
│   │   │   └── SettingsPanel.tsx # 设置面板
│   │   └── shared/               # 通用组件
│   ├── hooks/                    # 自定义 Hooks
│   ├── lib/
│   │   ├── tauri/                # Tauri IPC 封装
│   │   ├── todoParser.ts         # 待办解析器 (@done:timestamp)
│   │   └── noteParser.ts         # 笔记解析器
│   ├── modules/                  # 功能模块注册系统
│   ├── types/                    # TypeScript 类型
│   └── pages/                    # 页面组件
│       ├── QuickNotePage.tsx     # 快速笔记窗口
│       └── TodoCardPage.tsx      # 悬浮待办卡片
│
├── src-tauri/                    # Tauri 后端 (Rust)
│   ├── src/
│   │   ├── lib.rs                # 主入口、窗口管理、快捷键
│   │   ├── notes.rs              # 笔记 CRUD + 回收站
│   │   ├── settings.rs           # 设置持久化
│   │   ├── scriptmgr.rs          # 脚本管理
│   │   ├── appearance.rs         # 窗口外观
│   │   ├── widget.rs             # 桌面小组件
│   │   ├── modules.rs            # 模块配置
│   │   └── mcp_server.rs         # MCP 服务
│   └── tauri.conf.json
```

---

## 多窗口架构

| 窗口 label | 页面 | 描述 |
|-----------|------|------|
| `manager` | ManagerPage | 管理中心 (关闭时隐藏而非退出) |
| `quick-note-{n}` | QuickNotePage | 快速笔记悬浮窗 |
| `todo-card-{n}` | TodoCardPage | 待办卡片悬浮窗 |
| `folder-widget-{n}` | FolderWidgetPage | 文件夹小组件 |

---

## IPC 命令

### 笔记 (`notes.rs`)
`list_notes`, `get_note`, `save_note`, `create_note`, `delete_note`, `search_notes`, `save_image`, `toggle_pin_note`, `trash_note`, `trash_notes_batch`, `list_trash`, `restore_note`, `restore_notes_batch`, `permanent_delete_note`, `empty_trash`

### 设置 (`settings.rs`)
`get_settings`, `update_settings`

### 窗口 (`lib.rs`)
`toggle_always_on_top`, `hide_window`, `close_window`, `create_note_window`, `create_todo_card_window`, `toggle_todo_card_pin`, `show_manager_window`, `hide_manager_window`, `close_all_note_windows`, `toggle_note_windows_visibility`

### 外观 (`appearance.rs`)
`set_always_on_top`, `get_window_state`, `set_window_opacity`

---

## 待办系统数据格式

笔记中的 checkbox 自动解析为待办：

```markdown
- [ ] 待处理任务
- [x] 已完成 @done:2026-03-31T12:00:00
```

- `@done:ISO时间戳` 在勾选时自动附加
- 取消勾选时自动移除
- 已完成项在保留期内显示删除线（默认24小时，可在设置中调整）

---

## 开发规范

### 命名约定

| 类型 | 规范 | 示例 |
|------|------|------|
| 组件 | PascalCase | `NoteEditor.tsx` |
| Hook | camelCase + use 前缀 | `useAutoSave.ts` |
| IPC 命令 | snake_case | `list_notes`, `run_script` |
| CSS 变量 | `--manager-*` | `--manager-accent` |

### 模块系统

功能模块通过 `FeatureModule` 接口注册：

```typescript
interface FeatureModule {
  id: string;
  name: string;
  icon: string;
  caption: string;
  defaultEnabled: boolean;
  component: React.ComponentType;
}
```

新模块在 `src/modules/builtin/` 中定义，自动注册到侧边栏。

---

## 常用命令

```bash
cd easyflowhub-app
bun install           # 安装依赖
bun tauri dev         # 开发模式
bun tauri build       # 生产构建
npx tsc --noEmit      # TypeScript 类型检查
cd src-tauri && cargo check  # Rust 检查
```

---

## 快捷键

| 快捷键 | 功能 |
|--------|------|
| Ctrl+Alt+N | 新建快速笔记 |
| Ctrl+Alt+M | 显示/隐藏管理窗口 |
| Ctrl+Alt+D | 关闭所有快速笔记 |
| Ctrl+Alt+H | 隐藏/显示所有快速笔记 |

---

## 注意事项

1. **修改代码前先读取** - 理解现有代码再修改
2. **保持类型安全** - TS/Rust 类型同步
3. **IPC 封装** - 新命令需在 `lib/tauri/` 中封装
4. **Tailwind v4** - 不需要 tailwind.config.js
5. **模块化** - 新功能通过模块系统注册
