# Phase 3: ScriptsPanel 设计文档

**日期**: 2026-03-23
**状态**: 设计完成，待实现
**关联**: [ScriptMgr 路线图](./scriptmgr-roadmap.md)

---

## 概述

为 DeskFlow ManagerPage 实现脚本管理面板，侧重于**监控和配置**而非执行（AI 是主要执行者）。

### 设计决策

| 决策点 | 选择 | 理由 |
|--------|------|------|
| 通信方式 | HTTP API | 无需 Tauri 封装，支持 WebSocket 实时推送 |
| 布局 | 分类侧边栏 + 列表 | 脚本按类别组织是核心使用模式 |
| 详情交互 | 右侧抽屉 | 不离开列表，快速切换对比 |
| 执行者 | AI | GUI 侧重监控配置，非手动执行 |

---

## 整体架构

```
┌─────────────────────────────────────────────────────────────┐
│  ManagerPage                                                │
├──────────────┬──────────────────────────────┬───────────────┤
│ Sidebar      │ ScriptsPanel                 │ ScriptDrawer  │
│              │                              │ (条件渲染)     │
│ ┌──────────┐ │ ┌──────────────────────────┐ │ ┌───────────┐ │
│ │ 🔍 搜索   │ │ │ Toolbar: 搜索 + 刷新     │ │ │ 脚本详情   │ │
│ ├──────────┤ │ ├──────────────────────────┤ │ │           │ │
│ │ 📁 utils │ │ │                          │ │ │ 参数说明   │ │
│ │ 🎬 media │ │ │   ScriptListView         │ │ │           │ │
│ │ 💻 dev   │ │ │                          │ │ │ MCP 配置   │ │
│ │ 🔧 system│ │ │   - 脚本名称              │ │ │           │ │
│ │ ────────  │ │ │   - 描述预览              │ │ │ 执行历史   │ │
│ │ 📌 其他   │ │ │   - 标签 + MCP状态        │ │ │           │ │
│ └──────────┘ │ │                          │ │ │ ✕ 关闭    │ │
│              │ └──────────────────────────┘ │ └───────────┘ │
└──────────────┴──────────────────────────────┴───────────────┘
```

---

## 组件结构

```
ScriptsPanel/
├── index.tsx                 # 主容器
├── ScriptCategoryTree.tsx    # 左侧分类树
├── ScriptListView.tsx        # 脚本列表
├── ScriptListItem.tsx        # 单个脚本行
├── ScriptDrawer.tsx          # 右侧详情抽屉
└── hooks/
    ├── useScripts.ts         # 数据获取
    └── useScriptStore.ts     # 状态管理
```

---

## 状态管理

### Zustand Store

```typescript
interface ScriptStore {
  // 数据
  scripts: ScriptSummary[];
  categories: CategoryInfo[];
  selectedCategory: string | null;   // null = 全部
  selectedScriptId: string | null;

  // UI 状态
  drawerOpen: boolean;
  searchQuery: string;

  // Actions
  fetchScripts: () => Promise<void>;
  setSelectedCategory: (category: string | null) => void;
  setSelectedScript: (id: string | null) => void;
  openDrawer: (scriptId: string) => void;
  closeDrawer: () => void;
}
```

### 数据流

```
┌─────────────────────────────────────────────────────────┐
│  ScriptStore                                            │
│  ┌─────────────┐    ┌─────────────┐    ┌────────────┐  │
│  │ scripts[]   │───►│ categories  │───►│ filtered[] │  │
│  └─────────────┘    └─────────────┘    └────────────┘  │
│         │                                      │        │
│         ▼                                      ▼        │
│  ScriptCategoryTree                    ScriptListView  │
│                                                         │
│  selectedScriptId ─────────────────► ScriptDrawer      │
└─────────────────────────────────────────────────────────┘
```

---

## ScriptDrawer 详情抽屉

### 布局

```
┌──────────────────────────────┐
│ 📄 backup-files        ✕ 关闭 │  ← 标题栏
├──────────────────────────────┤
│ 描述                          │
│ 自动备份指定目录到归档位置      │
├──────────────────────────────┤
│ 参数                          │
│ ┌──────────────────────────┐ │
│ │ source*  [string]        │ │
│ │   源目录路径              │ │
│ │ target*  [string]        │ │
│ │   目标目录路径            │ │
│ │ compress [boolean]       │ │
│ │   是否压缩 (默认: true)   │ │
│ └──────────────────────────┘ │
├──────────────────────────────┤
│ MCP 配置                      │
│ [✓] 暴露给 AI                 │
│ 工具名: script_backup_files   │
├──────────────────────────────┤
│ 最近执行                      │
│ ┌──────────────────────────┐ │
│ │ ✓ 2024-03-23 14:30  1.2s │ │
│ │ ✓ 2024-03-22 09:15  0.8s │ │
│ │ ✗ 2024-03-21 18:00  err  │ │
│ └──────────────────────────┘ │
│ [查看全部] →                  │
└──────────────────────────────┘
```

### 功能分区

| 区块 | 内容 | 操作 |
|------|------|------|
| 标题栏 | 名称 + 关闭按钮 | 关闭抽屉 |
| 描述 | 脚本描述 | 无 |
| 参数 | 参数列表+类型+说明 | 无 |
| MCP 配置 | 暴露开关 + 工具名 | 切换开关 |
| 执行历史 | 最近3条 | 跳转监控面板 |

---

## API 客户端

### 文件：`src/lib/api/scriptmgr.ts`

```typescript
const BASE_URL = 'http://localhost:8765';

// 脚本 API
export const scriptsApi = {
  list: (search?: string) =>
    fetchJSON(`${BASE_URL}/api/scripts${qs({search})}`),

  get: (id: string) =>
    fetchJSON(`${BASE_URL}/api/scripts/${id}`),
};

// 任务 API
export const tasksApi = {
  list: (status?: string, limit?: number) =>
    fetchJSON(`${BASE_URL}/api/tasks${qs({status, limit})}`),

  get: (id: string) =>
    fetchJSON(`${BASE_URL}/api/tasks/${id}`),

  log: (id: string, opts?: {offset?, limit?, tail?}) =>
    fetchJSON(`${BASE_URL}/api/tasks/${id}/log${qs(opts)}`),

  cancel: (id: string) =>
    fetchJSON(`${BASE_URL}/api/cancel/${id}`, {method: 'POST'}),
};

// 类别 API (从 scripts 提取)
export const categoriesApi = {
  list: async () => {
    const { scripts } = await scriptsApi.list();
    return aggregateCategories(scripts);
  },
};
```

### 错误处理

```typescript
// 服务未启动时友好提示
catch (err) {
  if (err instanceof TypeError && err.message.includes('fetch')) {
    throw new Error('ScriptMgr 服务未启动，请先运行 scriptmgr serve');
  }
  throw err;
}
```

---

## 实现计划

### 文件清单

| 文件 | 操作 | 说明 |
|------|------|------|
| `src/lib/api/scriptmgr.ts` | 新增 | HTTP API 客户端 |
| `src/stores/scriptStore.ts` | 新增 | Zustand 状态管理 |
| `src/components/manager/scripts/ScriptsPanel.tsx` | 重写 | 主面板 |
| `src/components/manager/scripts/ScriptCategoryTree.tsx` | 新增 | 分类树 |
| `src/components/manager/scripts/ScriptListView.tsx` | 新增 | 脚本列表 |
| `src/components/manager/scripts/ScriptDrawer.tsx` | 新增 | 详情抽屉 |

### 实现顺序

```
Step 1: API 客户端 + 类型定义
   └─ 验证：curl 测试 API 可用

Step 2: Zustand Store
   └─ 验证：状态可正常更新

Step 3: ScriptCategoryTree + ScriptListView
   └─ 验证：分类切换、列表渲染

Step 4: ScriptDrawer
   └─ 验证：详情展示、MCP 开关

Step 5: 集成到 ManagerPage
   └─ 验证：路由切换正常
```

### 预计工作量

1-2 天

---

## 后续扩展

完成 ScriptsPanel 后，继续 Phase 3 其他面板：

1. **TasksPanel (执行监控)** - 任务状态、日志查看
2. **McpPanel (MCP 配置)** - 服务状态、工具管理
3. **SettingsPanel** - 整合脚本相关设置
