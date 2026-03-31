# MCP CLI Wrapper 设计文档

> 创建日期: 2026-03-19
> 状态: 已设计，待实现

## 概述

将 scriptmgr 作为统一的 CLI 入口，封装其他 MCP 服务器（如 serena、drawio、playwright 等）的工具调用。

### 核心目标

1. **统一 CLI 入口** - 一个命令访问所有 MCP 工具
2. **简洁语法** - 最小化输入，直观易用
3. **通用配置** - 不依赖特定 AI 客户端
4. **JSON 输出** - 便于程序解析

---

## 架构设计

```
scriptmgr CLI
    │
    └── mcp <server> <tool> [args...]
            │
            ▼
        ┌─────────────────────┐
        │   MCP 客户端        │
        │   (internal/mcpcli) │
        └──────────┬──────────┘
                   │
                   │ 启动子进程
                   ▼
        ┌─────────────────────┐
        │   MCP 服务器        │
        │   (stdio 传输)      │
        │   如 serena         │
        └─────────────────────┘
```

### 组件说明

| 组件 | 职责 |
|------|------|
| MCP 客户端 (`mcpcli/`) | 启动 MCP 服务器子进程，通过 stdio 通信 |
| 配置发现 | 读取配置文件，获取服务器列表和启动命令 |
| CLI 处理器 | 解析命令行参数，调用客户端执行 |

---

## CLI 接口设计

### 命令格式

```bash
# 发现 MCP 服务器
scriptmgr mcp              # 列出所有可用服务器

# 列出工具
scriptmgr mcp <server>     # 列出该服务器的工具

# 调用工具
scriptmgr mcp <server> <tool> [args...]

# 实际示例
scriptmgr mcp serena find_symbol Foo depth=1
scriptmgr mcp serena list_dir . recursive=true
scriptmgr mcp drawio create_new_diagram '<xml>...'
scriptmgr mcp chrome click uid=abc123
```

### 参数处理规则

| 输入 | 转换为 |
|------|--------|
| `key=value` | 字符串 |
| `key=123` | 整数 |
| `key=true` | 布尔值 true |
| `key=false` | 布尔值 false |
| `key='{"a":1}'` | JSON 对象 |
| `key='[1,2,3]'` | JSON 数组 |

### 参数简化

- 第一个非 `key=value` 参数 → 作为工具的第一个必需参数
- `key=value` → 命名参数

```bash
# 这两个等价：
scriptmgr mcp serena read_file main.go
scriptmgr mcp serena read_file relative_path=main.go
```

---

## 配置发现机制

### 配置来源（按优先级）

```go
1. ~/.scriptmgr/mcp-config.json      // scriptmgr 专用配置（推荐）
2. ~/.claude/settings.json           // Claude Code
3. ~/.claude/settings.local.json     // Claude Code 本地
4. 环境变量 SCRIPTMGR_MCP_CONFIG      // 自定义配置路径
```

### scriptmgr 专用配置格式

```json
{
  "version": "1.0",
  "servers": {
    "serena": {
      "command": "uvx",
      "args": ["serena", "--transport", "stdio"],
      "env": {"PATH": "/custom/path"}
    },
    "drawio": {
      "command": "npx",
      "args": ["-y", "@nicholasxu/mcp-drawio"]
    },
    "playwright": {
      "command": "npx",
      "args": ["-y", "@executeautomation/playwright-mcp-server"]
    }
  }
}
```

### CLI 管理命令

```bash
# 列出已配置的服务器
scriptmgr mcp

# 添加服务器
scriptmgr mcp-add <name> <command> [args...]

# 示例
scriptmgr mcp-add serena "uvx serena --transport stdio"
scriptmgr mcp-add drawio "npx -y @nicholasxu/mcp-drawio"

# 移除服务器
scriptmgr mcp-remove <name>

# 从 Claude 配置导入
scriptmgr mcp-import-claude
```

---

## MCP 客户端实现

### 核心接口

```go
// internal/mcpcli/client.go

type Client struct {
    cmd     *exec.Cmd      // 子进程
    stdin   io.Writer      // 写入请求
    stdout  io.Reader      // 读取响应
    stderr  io.Reader      // 错误输出
}

// 启动 MCP 服务器
func NewClient(server ServerConfig) (*Client, error)

// 发送请求并等待响应
func (c *Client) Call(method string, params map[string]any) (map[string]any, error)

// 获取工具列表
func (c *Client) ListTools() ([]Tool, error)

// 调用工具
func (c *Client) CallTool(name string, args map[string]any) (any, error)

// 关闭连接
func (c *Client) Close() error
```

### 通信流程

```
1. 启动子进程 (command + args)
2. 发送 initialize 请求
3. 收到 initialize 响应
4. 发送 tools/list 获取工具
5. 按需调用 tools/call
6. 关闭时终止子进程
```

### 错误处理

- 服务器启动失败 → 明确错误信息
- 通信超时 → 30秒默认超时
- JSON 解析错误 → 返回原始响应

---

## 文件结构

### 新增文件

```
scriptmgr-go/
├── internal/
│   ├── mcpcli/
│   │   ├── client.go        # MCP 客户端核心
│   │   ├── client_test.go   # 客户端测试
│   │   ├── config.go        # 配置发现与解析
│   │   └── types.go         # 共享类型
│   └── cli/
│       └── mcp.go           # MCP CLI 命令 (扩展)
```

---

## 实现计划

| 步骤 | 任务 | 产出 |
|------|------|------|
| 1 | 配置发现 | `mcpcli/config.go` - 读取和解析配置 |
| 2 | 客户端核心 | `mcpcli/client.go` - 启动进程、通信 |
| 3 | CLI 命令 | 扩展 `cli/mcp.go` - 添加 `mcp` 命令 |
| 4 | 管理命令 | `mcp-add`, `mcp-remove`, `mcp-import-claude` |
| 5 | 测试 | 单元测试 + 集成测试 |

---

## 验收标准

- `scriptmgr mcp` 列出所有配置的服务器
- `scriptmgr mcp serena` 列出 serena 的工具
- `scriptmgr mcp serena find_symbol Foo` 正确调用工具
- `scriptmgr mcp-add myserver "npx my-mcp"` 添加新服务器
- `scriptmgr mcp-import-claude` 从 Claude 配置导入服务器

---

## 技术选型

- **传输层**: stdio (标准输入/输出)
- **协议**: JSON-RPC 2.0
- **配置格式**: JSON
- **复用**: 现有 `internal/mcp/` 中的类型定义
