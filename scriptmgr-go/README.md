# scriptmgr

简易脚本管理工具，带 MCP 服务器接口。

## 功能特性

- **CLI 工具** - 管理本地 PowerShell、Python、Batch 脚本
- **MCP 服务器** - 为 AI 助手提供脚本调用接口
- **渐进式工具披露** - MCP 工具按需加载，避免上下文过长
- **异步执行** - 后台运行脚本，追踪任务状态
- **脚本分类** - 支持 category/tag 组织脚本
- **笔记集成** - 与 Claude 笔记同步

## 安装

```bash
go build -o scriptmgr.exe ./cmd/scriptmgr
```

## 快速开始

### 列出所有脚本
```bash
scriptmgr list
```

### 运行脚本
```bash
scriptmgr run <script-id> [--async] [args...]
```

### 启动 MCP 服务器

```bash
scriptmgr mcp
```

Claude Code 配置示例：
```json
{
  "mcpServers": {
    "scriptmgr": {
      "command": "D:/AllScripts/deskflow/scriptmgr-go/scriptmgr.exe", // 路径根据实际情况调整
      "args": ["mcp"]
    }
  }
}
```

## CLI 命令

| 命令 | 说明 |
|------|------|
| `list [--json] [--search TEXT]` | 列出脚本 |
| `describe <script-id>` | 显示脚本详情 |
| `run <script-id> [args...]` | 运行脚本 |
| `history [--limit N]` | 查看执行历史 |
| `tasks [--status STATUS]` | 列出任务 |
| `favorites [list\|add\|remove]` | 管理收藏 |
| `mcp` | 启动 MCP 服务器 |
| `serve [addr]` | 启动 HTTP API 服务器 |
| `--version` | 显示版本 |

## MCP 工具

### 静态工具（始终可用）
- `list_scripts` - 列出所有脚本
- `run_script` - 执行脚本
- `list_categories` - 列出脚本分类
- `load_category` - 加载分类工具
- `search_scripts` - 搜索脚本

### 分类加载后可用
- `script_<id>` - 动态加载的脚本工具

## HTTP API

启动服务器：
```bash
scriptmgr serve :8765
```

端点：
- `GET /api/scripts` - 列出脚本
- `GET /api/scripts/{id}` - 脚本详情
- `POST /api/run` - 运行脚本
- `GET /api/tasks` - 任务列表
- `GET /api/tasks/{id}` - 任务详情

## 配置

- 状态目录：`~/.config/scriptmgr/`
- MCP 配置：`~/.config/scriptmgr/mcp_config.json`

## 版本

当前版本：1.0.0

查看完整 Roadmap：[docs/plans/scriptmgr-v1.0-roadmap.md](../docs/plans/scriptmgr-v1.0-roadmap.md)
