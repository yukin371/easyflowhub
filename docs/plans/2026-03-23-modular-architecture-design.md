# Deskflow 模块化架构设计

## 概述

将 Deskflow 的功能模块（笔记、脚本、MCP 等）改为可配置的模块化架构，支持运行时启用/禁用，并为未来插件系统预留接口。

## 设计决策

| 决策点 | 选择 | 理由 |
|--------|------|------|
| 配置方式 | 设置面板 + 配置文件 | 普通用户友好，高级用户可编辑配置 |
| 配置位置 | `~/.config/deskflow/modules.json` | 与 Tauri 配置目录一致 |
| 插件预留 | 接口 + 注册表 | 为动态加载打基础，但不实现完整生命周期 |
| 默认启用 | 笔记、设置 | 保持新用户体验简洁 |
| 模块依赖 | 无依赖 | 设计简单，模块完全独立 |

## 架构设计

### 整体架构

```
┌─────────────────────────────────────────────────────────────────┐
│                          App 启动                               │
│                              ↓                                  │
│                    loadConfig() + registerBuiltinModules()      │
│                              ↓                                  │
│  ┌───────────────────────────────────────────────────────────┐  │
│  │                     ModuleRegistry                         │  │
│  │  modules: Map<string, FeatureModule>                       │  │
│  │  config: ModulesConfig ← ~/.config/deskflow/modules.json   │  │
│  └───────────────────────────────────────────────────────────┘  │
│                              ↓                                  │
│  ┌───────────────────────────────────────────────────────────┐  │
│  │                     ManagerPage                            │  │
│  │  ┌─────────────┐  ┌────────────────────────────────────┐  │  │
│  │  │ Sidebar     │  │  动态渲染 enabled modules          │  │  │
│  │  │ (动态生成)   │  │  <module.component />             │  │  │
│  │  └─────────────┘  └────────────────────────────────────┘  │  │
│  └───────────────────────────────────────────────────────────┘  │
│                              ↑                                  │
│  ┌───────────────────────────────────────────────────────────┐  │
│  │                   SettingsPanel                            │  │
│  │  功能模块开关 → toggleModule() → 更新配置文件              │  │
│  └───────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
```

### 核心接口

#### FeatureModule

```typescript
interface FeatureModule {
  // === 核心属性 ===
  id: string;                    // 唯一标识
  name: string;                  // 显示名称
  icon: string;                  // 侧边栏图标（单字）
  caption: string;               // 英文副标题
  defaultEnabled: boolean;       // 默认是否启用
  component: React.ComponentType; // 面板组件

  // === TODO: 插件系统扩展点 ===
  // 描述信息
  // description?: string;
  // version?: string;
  // author?: string;

  // 生命周期钩子
  // onLoad?: () => Promise<void>;
  // onUnload?: () => void;
  // onActivate?: () => void;
  // onDeactivate?: () => void;

  // 权限声明
  // permissions?: string[];

  // 插件元数据
  // pluginPath?: string;
  // isBuiltin?: boolean;
}
```

#### ModuleRegistry

```typescript
class ModuleRegistry {
  private modules = new Map<string, FeatureModule>();
  private config: ModulesConfig | null = null;

  // 注册模块
  registerModule(module: FeatureModule): void;

  // 获取所有启用的模块
  getEnabledModules(): FeatureModule[];

  // 获取所有可切换的模块（排除设置）
  getToggleableModules(): FeatureModule[];

  // 检查模块是否启用
  isEnabled(id: string): boolean;

  // 切换模块启用状态
  async toggleModule(id: string, enabled: boolean): Promise<void>;

  // 加载配置文件
  async loadConfig(): Promise<void>;

  // 保存配置文件
  async saveConfig(): Promise<void>;

  // === TODO: 插件系统扩展 ===
  // async loadPlugin(path: string): Promise<void>;
  // unloadPlugin(id: string): void;
}
```

### 配置文件格式

**位置**: `~/.config/deskflow/modules.json`

```json
{
  "version": 1,
  "modules": {
    "notes": { "enabled": true },
    "scripts": { "enabled": false },
    "mcp": { "enabled": false },
    "tasks": { "enabled": false },
    "components": { "enabled": false },
    "trash": { "enabled": false }
  }
}
```

**配置优先级**:
1. 配置文件存在 → 使用配置文件的值
2. 配置文件不存在 → 使用模块的 `defaultEnabled`
3. 用户在设置面板修改 → 写入配置文件

## 文件结构

```
src/
├── modules/                    # 新增：模块系统
│   ├── types.ts               # FeatureModule 接口定义
│   ├── registry.ts            # ModuleRegistry 实现
│   ├── config.ts              # 配置文件读写（Tauri IPC）
│   └── builtin/               # 内置模块定义
│       ├── index.ts           # 统一导出 + registerBuiltinModules()
│       ├── notes.ts           # 笔记模块
│       ├── scripts.ts         # 脚本模块
│       ├── mcp.ts             # MCP 模块
│       ├── tasks.ts           # 执行记录模块
│       ├── components.ts      # 组件预览模块
│       ├── trash.ts           # 回收站模块
│       └── settings.ts        # 设置模块
├── components/
│   └── manager/
│       ├── ManagerPage.tsx    # 改造：使用动态模块
│       ├── ManagerSidebar.tsx # 改造：使用动态模块
│       └── SettingsPanel.tsx  # 改造：新增模块开关区域
```

## 实现步骤

### Phase 1: 核心模块系统

1. 创建 `src/modules/types.ts` - 定义 FeatureModule 接口
2. 创建 `src/modules/config.ts` - 配置文件读写逻辑
3. 创建 `src/modules/registry.ts` - ModuleRegistry 实现
4. 创建 `src/modules/builtin/` - 所有内置模块定义

### Phase 2: 组件改造

1. 改造 `ManagerPage.tsx` - 使用动态模块渲染
2. 改造 `ManagerSidebar.tsx` - 使用动态模块生成导航
3. 改造 `SettingsPanel.tsx` - 新增模块开关区域

### Phase 3: 后端支持

1. 在 Rust 后端添加配置文件读写命令
2. 确保配置目录存在

## 内置模块列表

| ID | 名称 | 图标 | 默认启用 | 可切换 |
|----|------|------|---------|--------|
| notes | 笔记 | 墨 | ✅ | 否（核心模块） |
| scripts | 脚本 | 策 | ❌ | ✅ |
| mcp | MCP | 链 | ❌ | ✅ |
| tasks | 执行 | 行 | ❌ | ✅ |
| components | 组件 | 件 | ❌ | ✅ |
| trash | 回收站 | 藏 | ❌ | ✅ |
| settings | 设置 | 序 | ✅ | 否（核心模块） |

## 插件系统预留

### 接口扩展（注释形式）

```typescript
interface FeatureModule {
  // 当前实现的属性...

  // TODO: 以下为插件系统预留
  // description?: string;       // 模块描述
  // version?: string;           // 模块版本
  // author?: string;            // 作者
  // onLoad?: () => Promise<void>;   // 加载时钩子
  // onUnload?: () => void;          // 卸载时钩子
  // onActivate?: () => void;        // 激活时钩子
  // onDeactivate?: () => void;      // 离开时钩子
  // permissions?: string[];         // 权限声明
  // pluginPath?: string;            // 外部插件路径
  // isBuiltin?: boolean;            // 是否内置
}
```

### 未来插件目录结构（参考）

```
~/.config/deskflow/plugins/
├── my-plugin/
│   ├── manifest.json    # 插件元信息
│   ├── index.js         # 插件代码
│   └── assets/          # 资源文件
```

### manifest.json 示例（参考）

```json
{
  "id": "my-plugin",
  "name": "我的插件",
  "version": "1.0.0",
  "author": "developer",
  "main": "index.js",
  "permissions": ["fs:read", "http:fetch"]
}
```

## 测试计划

1. **单元测试**: ModuleRegistry 的注册、启用/禁用逻辑
2. **集成测试**: 配置文件读写、模块持久化
3. **E2E 测试**: 设置面板切换模块、侧边栏动态渲染
