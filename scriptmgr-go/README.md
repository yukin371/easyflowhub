# scriptmgr

简易脚本管理工具，带 MCP 服务器接口。

## 功能特性

- **CLI 工具** - 管理本地 PowerShell、Python、Batch 脚本
- **MCP 服务器** - 为 AI 助手提供脚本调用接口
- **API Relay** - OpenAI 兼容的多 provider 中转、加权路由与基础 failover
- **渐进式工具披露** - MCP 工具按需加载，避免上下文过长
- **异步执行** - 后台运行脚本，追踪任务状态
- **脚本分类** - 支持 category/tag 组织脚本
- **笔记集成** - 与 Claude 笔记同步
- **Manifest 扩展** - 通过 `plugin.json` 暴露 relay / MCP / manager 扩展贡献

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
      "command": "<path-to-easyflowhub>/scriptmgr-go/scriptmgr.exe", // 替换为本机实际路径
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
| `relay serve` | 启动 OpenAI 兼容 relay |
| `relay config` | 查看当前 relay 配置与 provider health |
| `relay hosts status` | 查看 Claude / Codex 当前宿主配置 |
| `relay hosts switch` | 切换 Claude / Codex 指向的 AI 网关 / relay |
| `extensions list` | 列出 manifest 扩展 |
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
scriptmgr serve
```

默认监听 `127.0.0.1:8765`，也可绑定 `127.0.0.0/8` 或 `[::1]` 的 loopback 地址。非 loopback 地址（包括 `:8765`、`0.0.0.0` 和局域网 IP）会被拒绝；管理 API 可执行本机脚本，目前不提供远程认证模式。Tauri 打包与 Vite 开发来源采用 CORS 白名单；CLI 等不带 `Origin` 的本机客户端仍可调用。CORS 不提供本机进程认证。

端点：
- `GET /api/scripts` - 列出脚本
- `GET /api/scripts/{id}` - 脚本详情
- `POST /api/run` - 运行脚本
- `GET /api/tasks` - 任务列表
- `GET /api/tasks/{id}` - 任务详情
- `GET /api/roots` - 列出存储位置（`roots` 全部 / `custom` 已固定）
- `POST /api/roots` - 固定存储位置（`{"path": "..."}`）
- `DELETE /api/roots?path=...` - 取消固定
- `GET /api/scripts/{id}/source` - 读取脚本源码
- `PUT /api/scripts/{id}/source` - 保存脚本源码（原子替换）
- `POST /api/scripts` - 在已配置 root 中新建脚本
- `POST /api/scripts/import` - 导入外部脚本文件（不覆盖同名）
- `DELETE /api/scripts/{id}` - 移入脚本回收站
- `GET /api/trash/scripts` - 回收站列表
- `POST /api/trash/scripts/{id}/restore` - 恢复
- `DELETE /api/trash/scripts/{id}` - 彻底删除
- `DELETE /api/trash/scripts` - 清空脚本回收站
- `GET /api/relay/config` - 读取 relay 配置快照（供 manager 使用）
- `PUT /api/relay/config` - 保存 relay 配置
- `GET /api/extensions` - 列出 manifest 扩展

文件写入边界：目标必须在已配置 root 内，扩展名限定 `.ps1` / `.py` / `.bat` / `.cmd`，源码上限 2 MB。删除走 `trash/scripts/` 目录（跨盘时自动退化为复制 + 删除），恢复遇到同名文件会报错而不是覆盖。

说明：
- `scriptmgr serve` 负责管理接口
- `scriptmgr relay serve` 负责真正的 OpenAI 兼容转发入口

## Relay API

启动 relay：
```bash
scriptmgr relay serve --port 8787
```

Relay 默认绑定 `127.0.0.1:8787`。配置 API 会隐藏 API key、认证类 header 和 URL 中的 userinfo / query / fragment；原样保存占位值会保留既有值。旧 `api_key` 仍以明文保存在本地 `relay.json`，新配置应优先使用 `api_key_env`。

示例 `relay.json`：
```json
{
  "version": 1,
  "providers": [
    {
      "id": "primary",
      "name": "Primary OpenAI-Compatible",
      "base_url": "https://api.example.com",
      "api_key_env": "OPENAI_API_KEY",
      "enabled": true,
      "weight": 2,
      "model_patterns": ["gpt-*"]
    },
    {
      "id": "backup",
      "name": "Backup OpenAI-Compatible",
      "base_url": "https://backup.example.com",
      "enabled": true,
      "weight": 1,
      "model_patterns": ["gpt-*"]
    }
  ],
  "routes": [
    {
      "id": "default-openai",
      "path_prefixes": ["/v1/"],
      "model_patterns": ["gpt-*"],
      "provider_ids": ["primary", "backup"],
      "strategy": "weighted_round_robin"
    }
  ]
}
```

说明：

- 推荐使用 `api_key_env` 引用环境变量，而不是把密钥直接写进 `relay.json`
- 旧 `api_key` 仍兼容，但应视为迁移期字段
- Relay 与 ScriptMgr 默认只监听 loopback；未知浏览器来源由白名单策略拒绝，但本机进程仍可直接访问本地 API

当前正式支持的 relay / 负载均衡能力：

- OpenAI 兼容路径转发：`/v1/*`
- provider + route 配置驱动的选路
- `weighted_round_robin` 作为当前唯一正式承诺的负载策略
- 最小 failover：`401` / `403`、`429`、`5xx`、timeout；只有所有尝试均为限流时才向调用方返回 `429`，只有所有尝试均为鉴权失败时才返回最后一次 `401` / `403`，混合失败仍返回 `502`
- provider health 快照和管理接口
- extension manifest 注入的 relay provider / route 贡献

当前已验证的回归场景：

- upstream failure fallback
- streaming passthrough
- `429` retry
- explicit `5xx` retry
- timeout retry
- provider specific auth / env api key

当前不承诺的能力：

- 非 OpenAI 协议适配
- 完整熔断恢复与配额调度
- 自动基于 health 改写 Claude / Codex 宿主配置
- 完整 `cc-switch` 式运行时编排

端点：
- `GET /health`
- `GET /api/relay/config`
- `PUT /api/relay/config`
- `GET /api/extensions`
- `POST /v1/chat/completions`
- `POST /v1/responses`
- `GET /v1/models`

## Host Switch

`scriptmgr relay` 继续只负责 OpenAI 兼容 relay。  
Claude / Codex 的宿主配置切换由 `relay hosts` 负责，不把不同协议强行塞进同一个 relay owner。

查看当前宿主配置：

```bash
scriptmgr relay hosts status --json
```

只切 Claude：

```bash
scriptmgr relay hosts switch \
  --dry-run \
  --host claude \
  --claude-base-url https://gateway.example.com/anthropic
```

只切 Codex：

```bash
scriptmgr relay hosts switch \
  --dry-run \
  --host codex \
  --codex-base-url http://127.0.0.1:8787/v1 \
  --codex-provider relay
```

同时切 Claude 和 Codex：

```bash
scriptmgr relay hosts switch \
  --dry-run \
  --claude-base-url https://gateway.example.com/anthropic \
  --codex-base-url http://127.0.0.1:8787/v1
```

说明：

- Claude 当前写入 `~/.claude/settings.json` 中的 `env.ANTHROPIC_BASE_URL`
- Codex 当前写入 `~/.codex/config.toml` 中的 `model_provider` 和 provider `base_url`
- `--dry-run` 会优先输出结构化 diff 预览，不落盘；`--json` 同时包含结构化 changes 与完整 before/after
- `--json` 可用于结构化消费 preview 或最终状态
- 每次写入前都会生成备份，再原子替换原文件

## 配置

- 状态目录：`~/.config/scriptmgr/`
- MCP 配置：`~/.config/scriptmgr/mcp_config.json`
- Relay 配置：`~/.config/scriptmgr/relay.json`
- 扩展目录：`~/.config/scriptmgr/extensions/` 或可执行文件同级 `extensions/`

## 版本

当前版本：1.0.0

查看当前项目 Roadmap：[docs/roadmap.md](../docs/roadmap.md)

relay / extension 第一阶段设计见：[docs/plans/2026-04-07-api-relay-and-extension-phase1.md](../docs/plans/2026-04-07-api-relay-and-extension-phase1.md)
