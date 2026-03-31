# ScriptMgr MCP 集成设计

> 创建日期: 2026-03-15
> 状态: 已验证

## 概述

将 ScriptMgr 封装为 MCP Server，让 AI（如 Claude Code）能高效调用脚本库，同时提供可视化 GUI 管理界面。

### 核心目标

1. **减少上下文消耗** - 渐进式加载，AI 只看到相关工具
2. **异步执行** - 长任务不阻塞 AI，完成后回调通知
3. **可视化管理** - GUI 管理脚本、配置 MCP、监控执行

---

## 架构设计

### 整体架构

```
┌─────────────────────────────────────────────────────────────┐
│                        AI agent (如Claude Code)             │
└─────────────────────────┬───────────────────────────────────┘
                          │ MCP Protocol
                          ▼
┌─────────────────────────────────────────────────────────────┐
│                   scriptmgr mcp                             │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │
│  │ Tool Router │  │ Session Mgr │  │ Callback Dispatcher │  │
│  │ (动态注册)   │  │ (异步任务)   │  │ (完成通知)          │  │
│  └──────┬──────┘  └──────┬──────┘  └──────────┬──────────┘  │
└─────────┼────────────────┼────────────────────┼─────────────┘
          │                │                    │
          ▼                ▼                    ▼
┌─────────────────────────────────────────────────────────────┐
│                       Core API (共享层)                      │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │
│  │ Discovery   │  │ Executor    │  │ Registry            │  │
│  │ (脚本发现)   │  │ (执行引擎)   │  │ (类别/标签索引)     │  │
│  └─────────────┘  └─────────────┘  └─────────────────────┘  │
└─────────────────────────┬───────────────────────────────────┘
                          │
          ┌───────────────┼───────────────┐
          ▼               ▼               ▼
┌─────────────────┐ ┌───────────┐ ┌─────────────────────────┐
│ scriptmgr list  │ │ scriptmgr │ │ scriptmgr-shell (Tauri) │
│ describe run    │ │ sessions  │ │ GUI 管理界面            │
│ (现有 CLI)      │ │ cancel    │ │                         │
└─────────────────┘ └───────────┘ └─────────────────────────┘
```

### 组件说明

| 组件 | 职责 |
|------|------|
| Tool Router | 动态注册 MCP tools，初始只暴露最小工具集 |
| Session Manager | 管理异步执行任务，追踪状态 |
| Callback Dispatcher | 脚本完成后通过 MCP notifications 推送结果 |
| Core API | CLI 和 MCP 共享的业务逻辑层 |

---

## MCP 工具协议

### 初始最小工具集

AI 启动时可见的工具：

```json
[
  {
    "name": "list_categories",
    "description": "列出所有脚本类别，返回类别名称和脚本数量",
    "inputSchema": { "type": "object", "properties": {} }
  },
  {
    "name": "load_category",
    "description": "加载指定类别的脚本工具，之后可调用该类别下的脚本",
    "inputSchema": {
      "type": "object",
      "properties": {
        "category": { "type": "string", "description": "类别名称" }
      },
      "required": ["category"]
    }
  },
  {
    "name": "unload_category",
    "description": "卸载指定类别的脚本工具，释放上下文空间",
    "inputSchema": {
      "type": "object",
      "properties": {
        "category": { "type": "string", "description": "类别名称" }
      },
      "required": ["category"]
    }
  },
  {
    "name": "search_scripts",
    "description": "按关键词搜索脚本，返回匹配的脚本 ID 和简要描述",
    "inputSchema": {
      "type": "object",
      "properties": {
        "query": { "type": "string", "description": "搜索关键词" }
      },
      "required": ["query"]
    }
  },
  {
    "name": "get_task_result",
    "description": "获取异步任务的结果。设置 wait=true 会阻塞直到任务完成",
    "inputSchema": {
      "type": "object",
      "properties": {
        "task_id": { "type": "string" },
        "wait": { "type": "boolean", "default": false }
      },
      "required": ["task_id"]
    }
  },
  {
    "name": "read_log",
    "description": "读取任务的完整日志，支持分页",
    "inputSchema": {
      "type": "object",
      "properties": {
        "task_id": { "type": "string" },
        "offset": { "type": "integer", "default": 0 },
        "limit": { "type": "integer", "default": 100 },
        "tail": { "type": "boolean", "default": false }
      },
      "required": ["task_id"]
    }
  }
]
```

### 动态加载后的脚本工具

调用 `load_category("文件处理")` 后动态注册：

```json
[
  {
    "name": "script_batch_rename",
    "description": "批量重命名文件",
    "inputSchema": {
      "type": "object",
      "properties": {
        "path": { "type": "string" },
        "pattern": { "type": "string" },
        "dry_run": { "type": "boolean", "default": true }
      },
      "required": ["path", "pattern"]
    }
  }
]
```

**命名约定**: 动态脚本工具前缀 `script_`

---

## 异步执行与回调

### 执行流程

```
AI                          MCP Server                    脚本进程
 │                               │                           │
 │  1. script_xxx({...})         │                           │
 │ ─────────────────────────────>│                           │
 │                               │                           │
 │  2. 立即返回 task_id           │                           │
 │ <─────────────────────────────│                           │
 │                               │                           │
 │                               │  3. 启动脚本执行            │
 │                               │ ─────────────────────────>│
 │                               │                           │
 │  (AI 可继续其他工作)            │                           │
 │                               │                           │
 │                               │  4. 脚本完成                │
 │                               │ <─────────────────────────│
 │                               │                           │
 │  5. MCP Notification          │                           │
 │ <─────────────────────────────│                           │
```

### 同步/异步模式

工具定义中增加 `async` 参数：

```json
{
  "name": "script_xxx",
  "inputSchema": {
    "properties": {
      "path": { "type": "string" },
      "async": {
        "type": "boolean",
        "default": false,
        "description": "true=异步执行返回 task_id，false=同步等待"
      }
    }
  }
}
```

### 回调通知格式

```json
{
  "method": "notifications/task_completed",
  "params": {
    "task_id": "task_abc123",
    "script_id": "batch_rename",
    "status": "success",
    "duration_ms": 3500,
    "exit_code": 0,
    "output_summary": "重命名了 42 个文件",
    "output_full_available": true
  }
}
```

### 降级方案（轮询）

如果回调不支持，提供轮询工具：

```json
{
  "name": "get_task_result",
  "description": "获取异步任务的结果",
  "inputSchema": {
    "properties": {
      "task_id": { "type": "string" },
      "wait": { "type": "boolean", "default": false }
    }
  }
}
```

---

## GUI 管理界面

### 页面结构

```
┌─────────────────────────────────────────────────────────────────┐
│  ScriptMgr                              [_] [□] [×]             │
├─────────────────────────────────────────────────────────────────┤
│  [脚本管理]  [MCP 配置]  [执行监控]  [设置]                      │
├─────────────────────────────────────────────────────────────────┤
│                        (页面内容区域)                            │
└─────────────────────────────────────────────────────────────────┘
```

### 页面 1: 脚本管理

- 脚本列表（按类别分组）
- 脚本详情和参数说明
- 元数据编辑
- MCP 暴露开关

### 页面 2: MCP 配置

- MCP 服务状态（运行/停止/端口）
- 初始暴露工具配置
- 默认加载类别
- 单个脚本的 MCP 暴露覆盖

### 页面 3: 执行监控

- 运行中任务列表（进度、取消）
- 最近完成任务（成功/失败）
- 任务详情和输出日志

---

## 数据模型

### 脚本元数据扩展

```json
{
  "id": "batch_rename",
  "name": "批量重命名",
  "description": "按模式批量重命名文件",
  "category": "文件处理",
  "tags": ["文件", "批量", "重命名"],
  "author": "yukin",
  "version": "1.0.0",

  "parameters": [
    {
      "name": "path",
      "type": "string",
      "required": true,
      "description": "目标目录路径"
    },
    {
      "name": "pattern",
      "type": "string",
      "required": true,
      "description": "匹配模式"
    },
    {
      "name": "dry_run",
      "type": "boolean",
      "default": true,
      "description": "试运行"
    }
  ],

  "mcp": {
    "exposed": true,
    "async_default": false,
    "timeout_ms": 30000,
    "dangerous": false,
    "summary_template": "重命名了 {{count}} 个文件"
  }
}
```

### MCP 配置文件

`scriptmgr-mcp.json`:

```json
{
  "version": "1.0",
  "server": {
    "port": 9527,
    "host": "127.0.0.1"
  },
  "dynamic_loading": {
    "initial_tools": ["list_categories", "load_category", "search_scripts"],
    "default_categories": ["文件处理"],
    "max_tools_per_load": 20
  },
  "execution": {
    "default_timeout_ms": 60000,
    "max_concurrent_tasks": 5,
    "output_max_length": 1000
  },
  "callbacks": {
    "enabled": true,
    "include_output_summary": true,
    "include_full_output": false
  }
}
```

### 任务状态存储

SQLite 表结构：

```sql
CREATE TABLE tasks (
  task_id TEXT PRIMARY KEY,
  script_id TEXT NOT NULL,
  status TEXT CHECK(status IN ('pending', 'running', 'success', 'failed', 'cancelled')),
  created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
  started_at DATETIME,
  completed_at DATETIME,
  exit_code INTEGER,
  duration_ms INTEGER,
  input_json TEXT,
  output_summary TEXT,
  output_full TEXT,
  error_message TEXT
);
```

---

## 开发里程碑

### Phase 1: 基础能力（1-2 周）

**目标**: 为 MCP 准备底层能力

| 任务 | 产出 |
|------|------|
| Core API 抽象 | `internal/api/` |
| 任务状态存储 | `internal/store/` |
| 异步执行框架 | `internal/executor/` |
| 单元测试 | `*_test.go` |

**验收标准**:
- `scriptmgr run --async <id>` 返回 task_id
- `scriptmgr status <task_id>` 查询结果
- `scriptmgr cancel <task_id>` 取消执行

### Phase 2: MCP 核心（2-3 周）

**目标**: 实现 MCP server 和动态加载

| 任务 | 产出 |
|------|------|
| MCP Server 模式 | `cmd/mcp/` |
| 初始工具集 | `internal/mcp/tools/` |
| 动态工具注册 | `internal/mcp/router/` |
| 回调通知 | `internal/mcp/callback/` |
| 配置解析 | `internal/config/` |

**验收标准**:
- Claude Code 可连接 `scriptmgr mcp`
- AI 可调用初始工具集
- `load_category` 后可调用该类别脚本
- 异步任务完成后 AI 收到通知

### Phase 3: GUI 完善（2 周）

**目标**: 扩展 Tauri shell

| 任务 | 产出 |
|------|------|
| 脚本管理页面 | `src/pages/Scripts.tsx` |
| MCP 配置页面 | `src/pages/McpConfig.tsx` |
| 执行监控页面 | `src/pages/Monitor.tsx` |
| 状态同步 | `src/stores/` |

**验收标准**:
- GUI 可查看/编辑脚本元数据
- GUI 可配置 MCP 暴露规则
- GUI 可查看异步任务状态和历史

---

## 技术挑战与修正

### 挑战 1: MCP 客户端回调支持度（高风险）

**问题**: 主流 MCP 客户端（Claude Desktop、Claude Code）的底层 Prompt 未被教导处理"自定义通知"。AI 可能收到 task_id 后就认为任务结束，忽略后续通知。

**修正策略**:

```
异步模式优先级：
1. 内部阻塞模式 (async=false) — AI 最自然，推荐短任务
2. 轮询模式 (async=true + get_task_result) — 长任务标准方案
3. 回调通知 — 客户端支持时的增强体验（不依赖）
```

**工具描述调整**:

```json
{
  "name": "script_batch_rename",
  "description": "批量重命名文件。\n\n【异步模式】设置 async=true 时立即返回 task_id，请使用 get_task_result(task_id, wait=true) 等待结果。",
  "inputSchema": {
    "properties": {
      "path": { "type": "string" },
      "async": {
        "type": "boolean",
        "default": false,
        "description": "true=异步(需轮询结果)，false=阻塞等待"
      }
    }
  }
}
```

### 挑战 2: Tauri 前端交互复杂度

**问题**: Tauri 通过 CLI 交互增加复杂度，Rust 层调用 Go sidecar 不够直接。

**修正策略**: MCP Server 同时暴露 HTTP/WebSocket API 给 Tauri 前端

```
┌─────────────────────────────────────────────────────────────┐
│                   scriptmgr serve                           │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │
│  │ MCP Server  │  │ HTTP API    │  │ WebSocket (实时)    │  │
│  │ (AI 调用)   │  │ (Tauri)     │  │ (Tauri 推送)       │  │
│  └─────────────┘  └─────────────┘  └─────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
```

**HTTP API 端点**:

```
GET  /api/scripts              # 脚本列表
GET  /api/scripts/:id          # 脚本详情
PUT  /api/scripts/:id/metadata # 更新元数据
POST /api/scripts/:id/run      # 执行脚本
GET  /api/tasks                # 任务列表
GET  /api/tasks/:id            # 任务状态
POST /api/tasks/:id/cancel     # 取消任务
GET  /api/mcp/config           # MCP 配置
PUT  /api/mcp/config           # 更新配置
```

### 挑战 3: 动态工具卸载（低风险）

**问题**: AI 频繁调用 `load_category` 后工具越积越多，导致 Token 爆炸。

**修正策略**: 增加 `unload_category` 工具 + LRU 自动卸载

**新增工具**:

```json
{
  "name": "unload_category",
  "description": "卸载指定类别的脚本工具，释放上下文空间",
  "inputSchema": {
    "properties": {
      "category": { "type": "string" }
    },
    "required": ["category"]
  }
}
```

**LRU 自动卸载**:

```json
// load_category 返回值
{
  "loaded": ["script_a", "script_b"],
  "unloaded": ["script_x", "script_y"],
  "message": "已加载 文件处理 (12个工具)，已卸载 网络工具 (5个工具)"
}
```

### 挑战 4: 工具返回值规范化（高风险）

**问题**: 脚本输出可能极其庞大（如构建日志上千行），直接返回会撑爆 AI 上下文。

**修正策略**: 强制截断 + 按需读取

**返回值结构**:

```json
{
  "task_id": "task_abc123",
  "status": "success",
  "exit_code": 0,
  "duration_ms": 3500,
  "output": {
    "truncated": true,
    "preview": "前 500 字符...",
    "total_length": 50000,
    "log_path": "~/.scriptmgr/logs/task_abc123.log"
  },
  "summary": "构建成功，生成 3 个文件"
}
```

**新增工具**:

```json
{
  "name": "read_log",
  "description": "读取任务的完整日志，支持分页",
  "inputSchema": {
    "properties": {
      "task_id": { "type": "string" },
      "offset": { "type": "integer", "default": 0 },
      "limit": { "type": "integer", "default": 100 },
      "tail": { "type": "boolean", "default": false, "description": "读取最后 N 行" }
    },
    "required": ["task_id"]
  }
}
```

### 挑战 5: Server 发现与生命周期（中风险）

**问题**: MCP Server 通常通过 stdio 启动，但 GUI 需要独立启动后台服务。可能出现端口冲突、多实例、僵尸进程等问题。

**修正策略**: PID/Port 文件管理

**运行时文件**:

```
~/.scriptmgr/
├── run/
│   ├── scriptmgr.pid      # 当前运行实例的 PID
│   ├── scriptmgr.port     # HTTP API 端口
│   └── scriptmgr.lock     # 启动锁，防止竞态
├── logs/
│   └── task_*.log         # 任务日志
└── data/
    └── tasks.db           # SQLite 数据库
```

**启动流程**:

```go
func StartServer() error {
    // 1. 检查锁文件，防止竞态启动
    if err := acquireLock(); err != nil {
        return fmt.Errorf("另一个实例正在启动")
    }
    defer releaseLock()

    // 2. 检查 PID 文件，处理僵尸进程
    if pid, err := readPID(); err == nil {
        if isProcessRunning(pid) {
            return fmt.Errorf("服务已在运行 (PID: %d)", pid)
        }
        // 清理僵尸进程的残留文件
        cleanupStaleFiles()
    }

    // 3. 选择可用端口
    port := findAvailablePort(defaultPort)

    // 4. 写入 PID 和 Port 文件
    writePID(os.Getpid())
    writePort(port)

    // 5. 启动服务
    return serve(port)
}
```

**GUI 集成**:

```
Tauri 启动时:
1. 读取 ~/.scriptmgr/run/scriptmgr.port
2. 如果文件存在且进程存活 → 复用现有服务
3. 如果不存在或进程已死 → 启动新服务
```

### 挑战 6: Schema 校验前置（中风险）

**问题**: AI 经常漏传 required 字段或类型传错，导致脚本执行失败或行为异常。

**修正策略**: Core API 层集成 JSON Schema 校验，不信任任何外部输入

**校验流程**:

```go
func (e *Executor) Run(scriptID string, params map[string]any) (*Result, error) {
    // 1. 获取脚本元数据
    script, err := e.registry.Get(scriptID)
    if err != nil {
        return nil, err
    }

    // 2. Schema 校验（在任何执行之前）
    if err := validateParams(script.Parameters, params); err != nil {
        return nil, &ValidationError{
            ScriptID: scriptID,
            Errors:   err.Details(),
        }
    }

    // 3. 类型转换和默认值填充
    processedParams, err := processParams(script.Parameters, params)
    if err != nil {
        return nil, err
    }

    // 4. 执行脚本
    return e.execute(script, processedParams)
}
```

**校验错误返回**:

```json
{
  "error": "validation_failed",
  "script_id": "batch_rename",
  "errors": [
    {
      "field": "path",
      "code": "required",
      "message": "path 是必填字段"
    },
    {
      "field": "count",
      "code": "type_mismatch",
      "message": "期望 integer，实际 string",
      "value": "abc"
    }
  ]
}
```

**校验规则**:

| 规则 | 说明 |
|------|------|
| required | 必填字段检查 |
| type | 类型匹配（string, integer, boolean, array） |
| enum | 枚举值限制 |
| minimum/maximum | 数值范围 |
| pattern | 正则匹配 |
| default | 缺省时填充默认值 |

---

## 风险与缓解

| 风险 | 缓解措施 |
|------|----------|
| MCP 回调不被支持 | **轮询为主**，提供 `get_task_result` 工具，回调仅作增强 |
| 动态工具注册时机 | `load_category` 返回值列出新注册的工具名 |
| 工具数量爆炸 | `unload_category` + LRU 自动卸载，配置 `max_loaded_tools` |
| 输出过长撑爆上下文 | **强制截断** + `read_log` 按需分页读取 |
| 多实例/端口冲突 | PID/Port 文件管理 + 启动锁 |
| 僵尸进程残留 | 启动时检查 PID 存活，清理残留文件 |
| AI 参数错误 | **Schema 校验前置**，返回详细错误信息 |
| 并发任务过多 | 配置 `max_concurrent_tasks`，排队执行 |
| 脚本执行超时 | 强制超时取消，记录错误状态 |
| GUI 与 MCP 状态不同步 | HTTP API + WebSocket 实时推送 |
| Tauri 交互复杂 | 前端直连 HTTP API，绕过 CLI |

---

## 技术选型

- **MCP 库**: `github.com/mark3labs/mcp-go` 或 `github.com/sourcegraph/jsonrpc2`
- **数据库**: SQLite（任务状态存储）
- **GUI 框架**: Tauri + React/TypeScript（现有）

---

## 核心决策总结

| 决策 | 选择 | 理由 |
|------|------|------|
| MCP 架构 | 内置于 scriptmgr-go | 复用现有逻辑，单一代码库 |
| 异步执行 | **轮询为主，回调可选** | 客户端兼容性，不依赖通知机制 |
| 技能加载 | AI 按需请求 + LRU 卸载 | 最小初始上下文，防止工具爆炸 |
| 输出处理 | **强制截断 + read_log** | 防止长输出撑爆 AI 上下文 |
| 进程管理 | PID/Port 文件 + 启动锁 | 避免多实例冲突和僵尸进程 |
| 参数校验 | **Schema 校验前置** | 不信任 AI 输入，提供清晰错误信息 |
| GUI 范围 | 全功能 | 脚本管理 + MCP 配置 + 执行监控 |
| 前端通信 | HTTP API + WebSocket | 绕过 CLI，Tauri 直连更简单 |
| 开发策略 | 全栈并行 | mock 数据先跑通 |

**预计开发周期**: 5-7 周
