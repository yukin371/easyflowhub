# ScriptMgr 开发路线图

> **创建日期**: 2026-03-23
> **最后更新**: 2026-03-23
> **维护原则**: 本文档是 ScriptMgr 开发的唯一进度追踪文档

---

## 项目概述

ScriptMgr 是一个脚本管理工具，支持 CLI 和 GUI 界面，并通过 MCP 协议让 AI 助手能够调用脚本库。

### 核心能力

1. **脚本发现与管理** - 自动扫描、分类、元数据管理
2. **脚本执行** - 同步/异步执行、日志捕获、状态追踪
3. **MCP Server** - 让 AI 通过 MCP 协议调用脚本
4. **GUI 管理界面** - Tauri + React 桌面应用

---

## 关联设计文档

| 文档 | 描述 | 状态 |
|------|------|------|
| [MCP 集成设计](./2026-03-15-mcp-integration-design.md) | MCP Server 架构、动态加载、异步通知 | ✅ 已验证 |
| [MCP 实施计划](./2026-03-15-mcp-implementation-plan.md) | 详细实施步骤和代码示例 | ✅ 已验证 |
| [MCP CLI Wrapper](./2026-03-19-mcp-cli-wrapper-design.md) | CLI 封装其他 MCP 服务器 | ✅ 已实现 |
| [Notes MCP Sync](./2026-03-19-notes-mcp-sync-design.md) | 笔记同步 MCP 工具 | ✅ 已实现 |
| [Manager Page Redesign](./2026-03-17-manager-page-redesign.md) | GUI 管理界面设计 | ✅ 已完成 |
| [Phase3 ScriptsPanel](./2026-03-23-phase3-scripts-panel-design.md) | 脚本管理面板设计 | 🚧 待实现 |

---

## 开发阶段

### Phase 1: 基础能力重构 ✅ 完成

**目标**: 模块化代码结构 + 异步执行 + 存储层

| 任务 | 文件 | 状态 | 备注 |
|------|------|------|------|
| 代码模块化重构 | `internal/` | ✅ | 拆分为 api/discovery/executor/store 等模块 |
| SQLite 任务存储 | `internal/store/store.go` | ✅ | 任务状态持久化 |
| 异步执行框架 | `internal/executor/async.go` | ✅ | 支持 detach 执行 |
| 参数校验 | `internal/validator/validator.go` | ✅ | Schema 校验前置 |
| 输出截断 | `internal/executor/output.go` | ✅ | 防止长输出撑爆上下文 |

**验收命令**:
```bash
scriptmgr run <id>              # 同步执行
scriptmgr run <id> --detach     # 异步执行
scriptmgr status <task_id>      # 查询状态
scriptmgr cancel <task_id>      # 取消任务
```

---

### Phase 2: MCP 核心 🚧 进行中

**目标**: MCP Server + 动态工具加载 + HTTP API

#### 2.1 MCP Server 基础 ✅

| 任务 | 文件 | 状态 | 备注 |
|------|------|------|------|
| JSON-RPC Server | `internal/mcp/server.go` | ✅ | 处理 initialize/tools/list/tools/call |
| stdio 传输层 | `internal/mcp/transport/transport.go` | ✅ | 标准输入输出通信 |
| 工具 Schema | `internal/mcp/schema.go` | ✅ | 工具定义和校验 |
| 基础工具 | `list_scripts`, `run_script` | ✅ | 脚本列表和执行 |

#### 2.2 MCP CLI Wrapper ✅

| 任务 | 文件 | 状态 | 备注 |
|------|------|------|------|
| MCP 客户端 | `internal/mcpcli/client.go` | ✅ | stdio 通信 |
| 配置发现 | `internal/mcpcli/config.go` | ✅ | 多来源配置 |
| CLI 命令 | `internal/cli/mcp.go` | ✅ | mcp/mcp-add/mcp-remove |

**验收命令**:
```bash
scriptmgr mcp serve                        # 启动 MCP Server
scriptmgr mcp                              # 列出服务器
scriptmgr mcp <server> <tool> [args]       # 调用工具
scriptmgr mcp-add serena "uvx serena"      # 添加服务器
```

#### 2.3 Notes MCP 工具 ✅

| 任务 | 文件 | 状态 | 备注 |
|------|------|------|------|
| Notes API | `internal/notes/api.go` | ✅ | 统一接口 |
| 文件处理 | `internal/notes/file.go` | ✅ | Markdown + YAML frontmatter |
| 双向同步 | `internal/notes/sync.go` | ✅ | DB ↔ File |
| 文件监视 | `internal/notes/watcher.go` | ✅ | 自动同步 |
| MCP 工具 | `get/set/sync/list/get_note` | ✅ | 5 个笔记工具 |

#### 2.8 笔记多仓库管理 ✅ (2026-03-25)

| 任务 | 描述 | 状态 | 备注 |
|------|------|------|------|
| 多仓库配置 | `notes/config.go` 支持多仓库 | ✅ | `repos[]` + `current_repo_id` |
| API 方法 | `ListRepos/AddRepo/RemoveRepo/SetCurrentRepo` | ✅ | `notes/api.go` |
| MCP 工具 | `list_note_repos/add_note_repo/remove_note_repo/set_current_note_repo` | ✅ | 4 个新工具 |
| 配置文件 | `~/.config/deskflow/notes_mcp_config.json` | ✅ | JSON 格式持久化 |

**MCP 工具示例**:
```bash
# 列出所有仓库
list_note_repos

# 添加新仓库
add_note_repo {"id": "work", "name": "Work Notes", "path": "E:/Notes/Work"}

# 切换当前仓库
set_current_note_repo {"id": "work"}
```

#### 2.4 HTTP API 服务 ✅

| 任务 | 文件 | 状态 | 备注 |
|------|------|------|------|
| HTTP Server | `internal/http/server.go` | ✅ | REST API |
| WebSocket | `internal/http/websocket.go` | ✅ | 实时推送 |

#### 2.5 动态工具加载 ✅ 完成

| 任务 | 描述 | 状态 | 备注 |
|------|------|------|------|
| `load_category` | 按需加载类别下的脚本工具 | ✅ | `internal/mcp/router.go` |
| `unload_category` | 卸载类别工具释放上下文 | ✅ | 支持幂等操作 |
| `list_categories` | 列出所有脚本类别 | ✅ | 返回类别名和脚本数 |
| `search_scripts` | 按关键词搜索脚本 | ✅ | 跨名称/描述/标签搜索 |
| LRU 自动卸载 | 超过工具数量限制时自动卸载 | ✅ | 基于 LastUsedAt |
| 工具命名规范 | 动态工具前缀 `script_` | ✅ | ID 转小写下划线 |
| 动态 Schema 生成 | 根据脚本参数生成 inputSchema | ✅ | 支持 string/integer/boolean |
| 单元测试 | 覆盖核心功能 | ✅ | 13 个测试用例通过 |

**关键文件**:
- `internal/mcp/router.go` - 工具路由器核心
- `internal/mcp/router_test.go` - 单元测试
- `internal/api/api.go` - 新增 ListCategories/ListScriptsByCategory

**验收命令**:
```bash
# MCP 工具调用示例
list_categories                    # 列出所有类别
load_category {"category": "xxx"}  # 加载类别
search_scripts {"keyword": "test"} # 搜索脚本
unload_category {"category": "xxx"} # 卸载类别
```

**参考设计**: [MCP 实施计划 - 2.2 动态工具注册](./2026-03-15-mcp-implementation-plan.md)

#### 2.6 异步任务工具 ✅ 完成

| 任务 | 描述 | 状态 | 备注 |
|------|------|------|------|
| `get_task_result` | 获取异步任务结果 (支持 wait) | ✅ | 60s 超时等待 |
| `read_log` | 读取任务日志 (分页/tail) | ✅ | 支持 offset/limit/tail |
| 任务完成通知 | MCP notification 推送 | ✅ | notifications/task_completed |

#### 2.7 进程生命周期管理 ✅ 完成

| 任务 | 描述 | 状态 | 备注 |
|------|------|------|------|
| PID 文件 | `~/.scriptmgr/run/scriptmgr.pid` | ✅ | WritePID / CheckExistingPID |
| Port 文件 | `~/.scriptmgr/run/scriptmgr.port` | ✅ | WritePort / ReadPort |
| 启动锁 | 防止竞态启动 | ✅ | AcquireLock / ReleaseLock |
| 僵尸进程清理 | 检测并清理残留 | ✅ | CleanupStaleFiles / IsProcessRunning |
| 跨平台支持 | Windows + Unix | ✅ | tasklist / signal(0) |

**关键文件**: `internal/runtime/runtime.go`, `internal/cli/serve.go`

**参考设计**: [MCP 实施计划 - 2.4 进程生命周期管理](./2026-03-15-mcp-implementation-plan.md)

---

### Phase 3: GUI 完善 🚧 进行中

**目标**: Tauri 管理界面扩展

**设计文档**: [Phase3 ScriptsPanel 设计](./2026-03-23-phase3-scripts-panel-design.md)

#### 3.1 前端 API 集成 ⬜

| 任务 | 描述 | 状态 |
|------|------|------|
| API 客户端 | 连接 Go HTTP API | ⬜ |
| 状态管理 | Zustand store | ⬜ |

#### 3.2 脚本管理面板 ✅

| 任务 | 描述 | 状态 |
|------|------|------|
| 分类侧边栏 | 类别树导航 | ✅ |
| 脚本列表 | 搜索 + 列表视图 | ✅ |
| 详情抽屉 | 参数说明 + MCP 配置 | ✅ |
| 执行历史 | 最近执行快捷入口 | ✅ |

#### 3.3 MCP 配置面板 ✅

| 任务 | 描述 | 状态 |
|------|------|------|
| 服务状态 | 运行/停止/端口 | ✅ |
| 静态工具列表 | 13 个内置工具 | ✅ |
| 类别管理 | 加载/卸载类别 | ✅ |
| 统计概览 | 工具数/类别数/脚本数 | ✅ |

#### 3.4 执行监控面板 ✅

| 任务 | 描述 | 状态 |
|------|------|------|
| 运行中任务 | 进度、取消按钮 | ✅ |
| 完成任务历史 | 成功/失败列表 | ✅ |
| 日志查看器 | 分页、tail 模式 | ✅ |

**参考设计**: [Manager Page Redesign](./2026-03-17-manager-page-redesign.md) | [ScriptsPanel 设计](./2026-03-23-phase3-scripts-panel-design.md)

---

## 当前进度总览

```
Phase 1: 基础能力重构    ████████████████████ 100%
Phase 2: MCP 核心        ████████████████████ 100%
  2.1 MCP Server 基础    ████████████████████ 100%
  2.2 MCP CLI Wrapper    ████████████████████ 100%
  2.3 Notes MCP 工具     ████████████████████ 100%
  2.4 HTTP API 服务      ████████████████████ 100%
  2.5 动态工具加载       ████████████████████ 100%
  2.6 异步任务工具       ████████████████████ 100%
  2.7 进程生命周期       ████████████████████ 100%
  2.8 笔记多仓库         ████████████████████ 100%
Phase 3: GUI 完善        ████████████████████ 100%
  3.1 前端 API 集成     ████████████████████ 100%
  3.2 脚本管理面板      ████████████████████ 100%
  3.3 MCP 配置面板      ████████████████████ 100%
  3.4 执行监控面板      ████████████████████ 100%
  3.5 笔记仓库 UI       ⬜ 待完成
```

---

## 优先级排序

### P0 - 立即开始

1. **动态工具加载** - MCP 的核心价值，让 AI 能按需调用脚本
2. **异步任务工具** - 长时间脚本执行必需

### P1 - 近期规划

3. **进程生命周期管理** - 生产环境稳定性
4. **脚本管理面板** - GUI 基础功能

### P2 - 后续迭代

5. **MCP 配置面板** - 高级配置
6. **执行监控面板** - 运维支持

---

## 技术栈

| 层级 | 技术 |
|------|------|
| CLI | Go (scriptmgr-go) |
| MCP Server | Go + JSON-RPC 2.0 + stdio |
| HTTP API | Go + net/http |
| 数据库 | SQLite (modernc.org/sqlite) |
| GUI | Tauri + React + TypeScript |
| 前端状态 | Zustand |

---

## 配置文件

| 文件 | 路径 | 用途 |
|------|------|------|
| MCP 配置 | `~/.scriptmgr/mcp-config.json` | MCP 服务器列表 |
| 任务数据库 | `~/.scriptmgr/data/tasks.db` | 异步任务状态 |
| 运行时文件 | `~/.scriptmgr/run/` | PID/Port/锁文件 |
| 任务日志 | `~/.scriptmgr/logs/` | 执行输出日志 |

---

## 变更日志

### 2026-03-25 (2.8 笔记多仓库管理完成)
- ✅ 支持多仓库配置：`notes/config.go` 重构
- ✅ 新增 MCP 工具：`list_note_repos`, `add_note_repo`, `remove_note_repo`, `set_current_note_repo`
- ✅ 每个仓库显示笔记数量 (`note_count`)
- ✅ 配置文件：`~/.config/deskflow/notes_mcp_config.json`
- **UI 部分待完成**: 需要在 DeskFlow SettingsPanel 添加仓库管理 UI

### 2026-03-23 (Phase 3.4 执行监控面板完成)
- ✅ 实现 TasksPanel 组件
- ✅ 运行中/已完成任务 Tab 切换
- ✅ 任务状态显示 (pending/running/completed/failed/cancelled)
- ✅ 取消运行中任务按钮
- ✅ 日志查看器 (Tail/全部模式切换)
- ✅ 5秒自动刷新运行中任务
- ✅ 添加侧边栏"执行"入口
- **Phase 3 GUI 完善完成!** 进度 100%

### 2026-03-23 (Phase 3.3 MCP 配置面板完成)
- ✅ 实现 McpPanel 组件
- ✅ 服务状态检测 (运行/停止)
- ✅ 静态工具列表展示 (13 个工具)
- ✅ 类别管理 (加载/卸载切换)
- ✅ 添加侧边栏 MCP 入口
- Phase 3 进度更新至 60%

### 2026-03-23 (Phase 3.2 ScriptsPanel 完成)
- ✅ 实现 HTTP API 客户端 (`src/lib/api/scriptmgr.ts`)
- ✅ 实现 React Context 状态管理 (`scriptStore.tsx`)
- ✅ 实现分类侧边栏 (`ScriptCategoryTree.tsx`)
- ✅ 实现脚本列表视图 (`ScriptListView.tsx`)
- ✅ 实现详情抽屉 (`ScriptDrawer.tsx`)
- ✅ 集成到 ManagerPage
- Phase 3 进度更新至 40%

### 2026-03-23 (Phase 3 设计)
- ✅ 完成 ScriptsPanel 设计文档
- 设计决策：HTTP API、分类侧边栏+列表、右侧抽屉
- 重点调整：AI 是执行者，GUI 侧重监控和配置
- Phase 3 进度更新至 20%

### 2026-03-23 (Phase 2 完成)
- ✅ 确认 Phase 2.7 进程生命周期管理已实现
- ✅ `internal/runtime/runtime.go` - PID/Port/Lock 文件管理
- ✅ 跨平台进程检测 (Windows tasklist / Unix signal)
- ✅ 僵尸进程自动清理
- ✅ 添加 12 个单元测试
- **Phase 2 MCP 核心完成!**

### 2026-03-23 (异步任务工具完成)
- ✅ 实现 `get_task_result` MCP 工具 - 获取异步任务状态，支持 wait 阻塞等待 (60s 超时)
- ✅ 实现 `read_log` MCP 工具 - 读取任务日志，支持分页 (offset/limit) 和 tail 模式
- ✅ 扩展 `internal/mcp/schema.go` - 新增工具 schema 和参数校验
- ✅ 扩展 `internal/mcp/server.go` - 新增 callGetTaskResult 和 callReadLog 处理器
- ✅ 扩展 ScriptAPI 接口 - 新增 ListTasks 和 ReadTaskLog 方法
- Phase 2 进度更新至 90%

### 2026-03-23 (动态工具加载完成)
- ✅ 实现 `internal/mcp/router.go` - 工具路由器核心
- ✅ 实现 `list_categories` / `load_category` / `unload_category` / `search_scripts` MCP 工具
- ✅ 实现 LRU 缓存机制，自动卸载最久未用的类别
- ✅ 实现动态 Schema 生成，根据脚本参数自动生成 inputSchema
- ✅ 扩展 API 层：`ListCategories()` / `ListScriptsByCategory()`
- ✅ 单元测试：13 个测试用例全部通过
- Phase 2 进度更新至 80%

### 2026-03-23 (初始)
- 创建统一 roadmap 文档
- 整合 5 个相关设计文档的进度
- Phase 1 确认完成，Phase 2 进度 60%

---

## 更新规则

1. **完成任务后** - 更新对应任务状态为 ✅
2. **开始任务时** - 更新状态为 🚧 并添加实际开始日期
3. **发现阻塞时** - 在任务备注中记录问题和解决方案
4. **每周回顾** - 更新进度条和优先级
5. **设计变更时** - 同步更新关联的设计文档
