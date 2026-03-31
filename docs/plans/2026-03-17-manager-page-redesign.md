# DeskFlow 管理中心重构设计

**日期**: 2026-03-17
**状态**: 设计阶段
**优先级**: P0

---

## 概述

重构 DeskFlow 管理中心，采用禅风格极简设计，实现高效的空间利用和可靠的自动保存机制。

### 核心目标

1. **空间高效**：单面板路由模式，编辑时全屏可用
2. **禅风格极简**：无保存/关闭按钮，自动保存静默处理
3. **数据安全**：localStorage 备份 + 异常关闭保护
4. **灵活查询**：4宫格/列表/时间线视图，标签 AND/OR 切换
5. **可扩展**：多面板架构，快捷键可配置

---

## 一、整体架构

### 单面板 + 路由导航模式

```
┌──────────────────────────────────────┐
│  ← 返回    笔记管理        🔍 🏷️ ⋯  │  <- 顶部工具栏
├──────────────────────────────────────┤
│  🏷️ 已选: 工作 灵感                   │  <- 标签筛选条
├──────────────────────────────────────┤
│  🔲列表 📄网格 📅时间线               │  <- 视图切换
│                                       │
│  ┌────┐┌────┐┌────┐┌────┐           │  <- 4宫格视图
│  │📌  ││📝  ││💡  ││📋  │           │
│  │标题1││标题2││标题3││标题4│           │
│  │3分钟││1小时││昨天││2天前│           │
│  └────┘└────┘└────┘└────┘           │
│                                       │
└──────────────────────────────────────┘
         ↓ 点击笔记
┌──────────────────────────────────────┐
│  ← 返回    标题: [____________]      │  <- 顶部工具栏
├──────────────────────────────────────┤
│                                       │
│   正文编辑区 (全屏可用)                │
│   (退出自动保存)                       │
│                                       │
├══════════════════════════════════════┤
│ 💡 #工作 #灵感  [编辑标签]    已保存  │  <- 底部状态栏
└──────────────────────────────────────┘
```

### 禅风格原则

- **无保存按钮**：内容变化时自动防抖保存
- **无关闭按钮**：点击返回、ESC 键自动退出
- **无干扰提示**：保存状态用状态栏小字显示
- **全屏专注**：编辑时内容占满可用空间
- **标签在底部**：状态栏显示当前标签，支持快速编辑

---

## 二、组件架构

### 组件树

```
ManagerPage/
├── ManagerSidebar          # 侧边导航栏
│   ├── NavItem (笔记)
│   ├── NavItem (脚本)
│   ├── NavItem (桌面组件)
│   ├── NavItem (MCP管理)
│   └── NavItem (设置)
│
├── ContentArea             # 内容区域（路由切换）
│   │
│   ├── NotesPanel/         # 笔记面板
│   │   ├── ViewToggle      # 视图切换
│   │   ├── TagFilterBar    # 标签筛选条
│   │   ├── SearchBar       # 搜索框
│   │   └── NoteList/
│   │       ├── ListView
│   │       ├── GridView    # 4宫格
│   │       └── TimelineView
│   │
│   ├── NoteEditor/         # 笔记编辑器（全屏）
│   │   ├── TopBar          # 顶部栏
│   │   ├── ZenEditor       # 极简编辑器
│   │   └── StatusBar       # 底部状态栏
│   │
│   ├── ScriptsPanel/       # 脚本面板
│   ├── WidgetsPanel/       # 桌面组件面板
│   ├── McpPanel/           # MCP管理面板
│   └── SettingsPanel/      # 设置面板
```

### 状态管理

```typescript
interface ManagerState {
  // 面板切换
  activePanel: 'notes' | 'scripts' | 'widgets' | 'mcp' | 'settings';

  // 笔记视图状态
  notesView: {
    mode: 'list' | 'grid' | 'timeline';
    searchQuery: string;
    selectedTags: string[];
    filterLogic: 'and' | 'or';
    editingNoteId: string | null;
  };

  // 导航历史
  history: string[];
}
```

---

## 三、视图模式

### 列表视图

垂直列表布局，每行显示完整信息：
- 标题 + 置顶图标
- 内容预览（100字）
- 更新时间 + 字符数

### 网格视图（4宫格）

卡片式布局，每行4个：
```typescript
<div className="grid grid-cols-4 gap-4">
  {notes.map(note => (
    <NoteCard key={note.id} note={note} />
  ))}
</div>
```

### 时间线视图

按日期分组展示：
- 今天 / 昨天 / 更早
- 每组内按时间排序

---

## 四、标签筛选系统

### AND/OR 逻辑切换

```typescript
// 标签过滤逻辑
const filteredNotes = notes.filter(note => {
  if (selectedTags.length === 0) return true;

  const noteTags = note.tags.split(/\s+/).filter(Boolean);

  if (filterLogic === 'and') {
    // AND: 必须包含所有选中的标签
    return selectedTags.every(tag => noteTags.includes(tag));
  } else {
    // OR: 包含任一选中的标签即可
    return selectedTags.some(tag => noteTags.includes(tag));
  }
});
```

### 标签筛选条

- 显示已选标签（可点击移除）
- 逻辑切换按钮（2个标签时显示）
- 添加筛选按钮
- 清除全部按钮

---

## 五、自动保存机制

### 防抖保存

```typescript
const ZenEditor = ({ note, onChange }) => {
  const saveTimerRef = useRef<NodeJS.Timeout>();

  const handleChange = (newContent: string) => {
    onChange(newContent);

    if (saveTimerRef.current) {
      clearTimeout(saveTimerRef.current);
    }

    saveTimerRef.current = setTimeout(async () => {
      const parsed = parseNoteContent(newContent);
      await saveNoteApi({
        ...note,
        title: parsed.title,
        content: parsed.cleanContent,
        tags: parsed.tags,
      });
    }, 1000);
  };
};
```

### localStorage 备份

实时备份到 localStorage，防止异常关闭：
```typescript
const AutoSaveBackup = {
  save: (noteId: string, content: string) => {
    localStorage.setItem(`autosave_${noteId}`, JSON.stringify({
      content,
      timestamp: Date.now(),
    }));
  },

  load: (noteId: string) => {
    const data = localStorage.getItem(`autosave_${noteId}`);
    return data ? JSON.parse(data) : null;
  },

  clear: (noteId: string) => {
    localStorage.removeItem(`autosave_${noteId}`);
  },
};
```

### 异常关闭保护

多种关闭场景的保存处理：
1. 正常关闭/返回 - 等待保存完成
2. Alt+F4 - Tauri close-requested 事件
3. 断电/崩溃 - 下次启动从 localStorage 恢复

---

## 六、备份恢复与内容比较

### 恢复对话框

发现备份时显示内容比较界面：
- 当前版本 vs 备份版本
- 差异高亮显示
- 选择使用哪个版本
- 手动合并选项

### Diff 视图

```typescript
interface DiffResult {
  hasChanges: boolean;
  changes: Array<{
    type: 'add' | 'delete' | 'modify';
    start: number;
    end: number;
  }>;
}
```

---

## 七、快捷键系统

### 配置结构

```typescript
interface ShortcutConfig {
  // 文件操作
  save: string;           // "Ctrl+S"
  close: string;          // "Escape"

  // 编辑操作
  cutLine: string;        // "Ctrl+X"
  deleteLine: string;     // "Ctrl+D"
  duplicateLine: string;  // "Ctrl+Shift+D"

  // 插入操作
  insertLineBelow: string;   // "Ctrl+Enter"
  insertLineAbove: string;   // "Ctrl+Shift+Enter"

  // ... 更多配置
}
```

### 快捷键引擎

```typescript
class ShortcutEngine {
  private config: ShortcutConfig;
  private textarea: HTMLTextAreaElement;

  private matchShortcut(event: KeyboardEvent, shortcut: string): boolean {
    const parsed = this.parseShortcut(shortcut);
    const ctrl = event.ctrlKey || event.metaKey;

    return (
      ctrl === parsed.ctrl &&
      event.shiftKey === parsed.shift &&
      event.altKey === parsed.alt &&
      event.key.toLowerCase() === parsed.key
    );
  }

  private executeCommand(command: string) {
    // 执行对应的编辑操作
  }
}
```

---

## 八、目录结构

```
src/
├── components/
│   ├── manager/
│   │   ├── ManagerPage.tsx
│   │   ├── ManagerSidebar.tsx
│   │   ├── notes/
│   │   │   ├── NotesPanel.tsx
│   │   │   ├── NoteEditor.tsx
│   │   │   ├── ZenEditor.tsx
│   │   │   ├── StatusBar.tsx
│   │   │   ├── views/
│   │   │   │   ├── ListView.tsx
│   │   │   │   ├── GridView.tsx
│   │   │   │   └── TimelineView.tsx
│   │   │   └── filters/
│   │   │       ├── TagFilterBar.tsx
│   │   │       └── TagSelector.tsx
│   │   ├── scripts/
│   │   ├── widgets/
│   │   ├── mcp/
│   │   └── settings/
│   └── shared/
│       ├── BackupRestoreDialog.tsx
│       └── DiffView.tsx
│
├── lib/
│   ├── hooks/
│   │   ├── useAutoSave.ts
│   │   ├── useShortcutEngine.ts
│   │   └── useBackupRestore.ts
│   ├── stores/
│   │   └── managerStore.ts
│   └── utils/
│       └── diff.ts
│
└── types/
    ├── note.ts
    └── shortcut.ts
```

---

## 九、实现计划

### 阶段 1: 核心架构重构 (1-2周)

- [ ] 重构 NotesPanel 为单面板 + 路由模式
- [ ] 实现 NoteEditor 全屏编辑器
- [ ] 实现 ZenEditor 极简编辑组件
- [ ] 实现 StatusBar 底部状态栏
- [ ] 集成自动保存机制

### 阶段 2: 视图模式 (1周)

- [ ] 实现 GridView 4宫格视图
- [ ] 实现 TimelineView 时间线视图
- [ ] 实现视图切换组件

### 阶段 3: 标签系统 (1周)

- [ ] 实现 TagFilterBar 标签筛选条
- [ ] 实现 TagSelector 标签选择器
- [ ] 支持 AND/OR 逻辑切换
- [ ] 实现标签聚合统计

### 阶段 4: 备份恢复 (1周)

- [ ] 实现 localStorage 备份机制
- [ ] 实现 BackupRestoreDialog 比较界面
- [ ] 实现 DiffView 差异显示
- [ ] 集成到 ZenEditor

### 阶段 5: 快捷键系统 (1周)

- [ ] 实现快捷键引擎 ShortcutEngine
- [ ] 支持常见编辑器操作
- [ ] 实现配置存储
- [ ] 实现设置页配置界面

### 阶段 6: 其他面板 (2周)

- [ ] ScriptsPanel 脚本管理
- [ ] WidgetsPanel 桌面组件
- [ ] McpPanel MCP 管理
- [ ] SettingsPanel 设置

### 阶段 7: 测试与优化 (1周)

- [ ] 各种关闭场景测试
- [ ] 边界情况处理
- [ ] 性能优化
- [ ] 用户体验打磨

---

## 十、技术风险

| 风险 | 缓解措施 |
|------|----------|
| 路由模式状态管理复杂 | 使用 Zustand + 良好状态设计 |
| 自动保存可靠性 | localStorage 备份 + 多重触发点 |
| 快捷键冲突 | 提供配置界面，默认安全组合 |
| 数据库迁移 | 版本化迁移脚本 |

---

## 附录：错误提示规范

因为是本地软件，错误提示应准确描述问题：

- ❌ "保存失败，请检查网络"
- ✅ "本地服务连接失败"

保存状态：
- `saved` → ✓
- `saving` → ●
- `unsaved` → ○
- `error` → ✗ 连接失败
