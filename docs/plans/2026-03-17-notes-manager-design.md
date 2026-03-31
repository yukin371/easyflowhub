# DeskFlow 笔记管理功能设计

> 创建日期: 2026-03-17

## 概述

DeskFlow 笔记系统分为两个核心页面：

- **NotesPage（快速笔记）**：轻量灵感捕捉工具，类似便利贴，关闭自动保存
- **ManagerPage（管理中心）**：功能完整的管理中心，包含笔记管理、脚本管理、回收站、设置

## 页面定位

### 快速笔记 (NotesPage)
- 无需手动保存，关闭即自动持久化
- 多标签支持
- 纯粹的灵感捕捉工具

### 管理中心 (ManagerPage)
- 整理 + 轻量编辑
- 版本管理（简单撤销栈）
- 回收站（定时清理）
- 标签（扁平标签 + 颜色）
- 导出（TXT / Markdown）
- 更强功能需导出到其他软件

## 数据模型

### Note 扩展字段

```typescript
interface Note {
  id: string;
  title: string;
  content: string;
  created_at: string;
  updated_at: string;
  is_pinned: boolean;
  // 新增字段
  deleted_at: string | null;  // 回收站：删除时间，null 表示未删除
}

interface Tag {
  id: string;
  name: string;
  color: string;  // 十六进制颜色，如 "#FF5733"
}

interface NoteVersion {
  id: string;
  note_id: string;
  content: string;
  title: string;
  created_at: string;
}

interface AppConfig {
  notes: {
    trashRetentionDays: number;    // 回收站保留天数，默认 14，范围 7-30
    versionStackSize: number;      // 版本栈大小，默认 20，范围 10-50
  };
  quickNote: {
    opacity: number;               // 透明度，默认 0.8，范围 0.3-1.0
    width: number;                 // 宽度，默认 400，范围 300-800
    height: number;                // 高度，默认 500，范围 300-1000
  };
  scripts: {
    rootPath: string;              // 脚本根目录
  };
  appearance: Record<string, unknown>;  // 预留扩展
}
```

### SQLite 表结构

```sql
-- 扩展 notes 表
ALTER TABLE notes ADD COLUMN deleted_at TEXT DEFAULT NULL;

-- 标签表
CREATE TABLE IF NOT EXISTS tags (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    color TEXT NOT NULL DEFAULT '#6B7280'
);

-- 笔记-标签关联表
CREATE TABLE IF NOT EXISTS note_tags (
    note_id TEXT NOT NULL,
    tag_id TEXT NOT NULL,
    PRIMARY KEY (note_id, tag_id),
    FOREIGN KEY (note_id) REFERENCES notes(id) ON DELETE CASCADE,
    FOREIGN KEY (tag_id) REFERENCES tags(id) ON DELETE CASCADE
);

-- 版本快照表
CREATE TABLE IF NOT EXISTS note_versions (
    id TEXT PRIMARY KEY,
    note_id TEXT NOT NULL,
    title TEXT NOT NULL,
    content TEXT NOT NULL,
    created_at TEXT NOT NULL,
    FOREIGN KEY (note_id) REFERENCES notes(id) ON DELETE CASCADE
);

CREATE INDEX idx_note_versions_note_id ON note_versions(note_id);

-- 配置表
CREATE TABLE IF NOT EXISTS config (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
```

## 后端 API 设计

### 标签管理
```rust
pub fn list_tags() -> Vec<Tag>
pub fn create_tag(name: String, color: String) -> Tag
pub fn update_tag(id: String, name: String, color: String) -> Tag
pub fn delete_tag(id: String) -> bool
pub fn assign_tags(note_id: String, tag_ids: Vec<String>) -> Note
pub fn remove_tag_from_note(note_id: String, tag_id: String) -> Note
```

### 回收站
```rust
pub fn trash_note(id: String) -> Note
pub fn restore_note(id: String) -> Note
pub fn list_trash() -> Vec<Note>
pub fn permanently_delete(id: String) -> String
pub fn empty_trash() -> usize
pub fn cleanup_expired_trash() -> usize
```

### 版本管理
```rust
pub fn save_version(note_id: String) -> NoteVersion
pub fn list_versions(note_id: String) -> Vec<NoteVersion>
pub fn restore_version(note_id: String, version_id: String) -> Note
pub fn cleanup_old_versions() -> usize
```

### 配置
```rust
pub fn get_config() -> AppConfig
pub fn update_config(config: AppConfig) -> AppConfig
```

### 导出
```rust
pub fn export_note(id: String, format: String) -> ExportResult
pub fn export_notes(ids: Vec<String>, format: String) -> ExportResult
```

## 前端架构

### 文件结构

```
src/
├── types/
│   ├── note.ts          # 扩展 Note 类型
│   ├── tag.ts           # 标签类型
│   ├── version.ts       # 版本类型
│   └── config.ts        # 配置类型
│
├── lib/tauri/
│   ├── notes.ts         # 扩展导出和回收站 API
│   ├── tags.ts          # 标签 API
│   ├── versions.ts      # 版本 API
│   └── config.ts        # 配置 API
│
├── hooks/
│   ├── useNotes.ts      # 快速笔记专用
│   ├── useTags.ts       # 标签管理
│   └── useConfig.ts     # 配置管理
│
├── components/
│   ├── NotesPage.tsx         # 快速笔记
│   ├── NoteEditor.tsx        # 编辑器组件
│   ├── TabBar.tsx            # 标签栏
│   │
│   └── manager/              # 管理中心模块
│       ├── ManagerPage.tsx        # 主页面
│       ├── ManagerSidebar.tsx     # 侧边栏导航
│       ├── NotesPanel.tsx         # 笔记管理
│       ├── ScriptsPanel.tsx       # 脚本管理
│       ├── TrashPanel.tsx         # 回收站
│       ├── SettingsPanel.tsx      # 设置
│       ├── TagPicker.tsx          # 标签选择器
│       ├── TagBadge.tsx           # 标签徽章
│       ├── VersionHistory.tsx     # 版本历史
│       └── ExportDialog.tsx       # 导出对话框
```

### ManagerPage 布局

```
┌─────────────────────────────────────────┐
│              ManagerPage                 │
├──────────┬──────────────────────────────┤
│ Sidebar  │         Content Panel        │
│ ──────── │                              │
│ 📝 笔记   │   NotesPanel                 │
│ ⚡ 脚本   │   ScriptsPanel               │
│ 🗑️ 回收站 │   TrashPanel                 │
│ ⚙️ 设置   │   SettingsPanel              │
└──────────┴──────────────────────────────┘
```

### NotesPanel 布局

```
┌─────────────────────────────────────────────────────────┐
│  NotesPanel                                             │
├─────────────────────────────────────────────────────────┤
│  工具栏                                                  │
│  [列表|网格|时间线]  [🔍 搜索框...]  [排序 ▼]  [+ 新建]   │
├──────────────┬──────────────────────────────────────────┤
│  标签筛选     │  笔记列表                                │
│  ─────────── │  ────────────────────────────────────   │
│  ☐ 全部       │  笔记卡片...                             │
│  ☐ 工作       │                                          │
│  ☐ 学习       │                                          │
│              │                                          │
├──────────────┴──────────────────────────────────────────┤
│  底部操作栏（选中时显示）                                 │
│  已选择 N 项                    [删除] [导出] [加标签]   │
└─────────────────────────────────────────────────────────┘
```

### TrashPanel 布局

```
┌─────────────────────────────────────────────────────────┐
│  TrashPanel                                             │
├─────────────────────────────────────────────────────────┤
│  工具栏                                                  │
│  回收站 (N)  [🔍 搜索...]              [清空回收站]      │
├────────────────────┬────────────────────────────────────┤
│  笔记列表           │  预览面板                          │
│  ────────────────  │  ──────────────────────────────── │
│  笔记卡片...        │  标题 / 内容 / 标签 / 时间信息     │
│                    │  [恢复] [永久删除]                  │
└────────────────────┴────────────────────────────────────┘
```

### SettingsPanel 配置项

- **笔记设置**
  - 回收站保留天数（7-30 天）
  - 版本历史保留数量（10-50 步）
- **快速笔记窗口**
  - 默认透明度（30%-100%）
  - 默认窗口大小（宽/高）
- **脚本设置**
  - 脚本根目录

## 实现计划

### 阶段一：数据层基础
- [ ] 扩展 SQLite 表结构
- [ ] 扩展 Note 类型
- [ ] 实现标签 CRUD API
- [ ] 实现回收站 API
- [ ] 实现版本快照 API
- [ ] 实现配置 API

### 阶段二：ManagerPage 框架
- [ ] 创建 ManagerPage 主页面
- [ ] 实现 ManagerSidebar 侧边栏导航
- [ ] 实现路由切换逻辑

### 阶段三：NotesPanel 功能
- [ ] 连接真实数据
- [ ] 实现标签筛选
- [ ] 实现编辑面板
- [ ] 实现版本历史
- [ ] 实现批量导出

### 阶段四：其他面板
- [ ] TrashPanel 回收站
- [ ] SettingsPanel 配置
- [ ] ScriptsPanel 复用

### 阶段五：完善
- [ ] 定时清理任务
- [ ] 快速笔记窗口配置生效
- [ ] 测试和优化

## 当前进度

| 模块 | 状态 | 说明 |
|------|------|------|
| 快速笔记 | ✅ 已完成 | 多标签、自动保存、关闭持久化 |
| ManagerPage 框架 | 📋 待实现 | 侧边栏 + 面板切换 |
| NotesPanel | 📋 待实现 | 连接真实数据、标签筛选 |
| TrashPanel | 🔜 后续 | 回收站功能 |
| SettingsPanel | 🔜 后续 | 配置管理 |
| 版本历史 | 🔜 后续 | 撤销栈 |
| 批量导出 | 🔜 后续 | TXT/MD 导出 |
