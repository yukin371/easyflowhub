# DeskFlow Scripts Market - v2 规划

**版本**: v2.0
**日期**: 2026-03-25
**目标**: 构建脚本分享和发现平台

---

## 1. 概述

Scripts Market 是一个让用户发现、安装、上传和分享脚本的平台。打通个人脚本库与社区的桥梁。

### 核心价值

- **发现**: 找到他人分享的实用脚本
- **分享**: 贡献自己的脚本到社区
- **安装**: 一键安装脚本到本地库

---

## 2. 架构设计

### 2.1 整体架构

```
┌─────────────────────────────────────────────────────────┐
│                    Scripts Market                         │
│                                                          │
│  ┌──────────┐    ┌──────────┐    ┌──────────────────┐ │
│  │  Browse  │ → │  Search  │ → │    Install       │ │
│  └──────────┘    └──────────┘    └──────────────────┘ │
│        ↓                                            │
│  ┌──────────┐                                        │
│  │  Publish │                                        │
│  └──────────┘                                        │
└────────────────────────────┬──────────────────────────┘
                             │
         ┌───────────────────┼───────────────────┐
         ▼                   ▼                   ▼
┌─────────────────┐ ┌─────────────────┐ ┌─────────────────┐
│  GitHub Gist   │ │  GitHub Repo    │ │  Direct URL    │
│  (主存储)       │ │  (备份)         │ │  (镜像)        │
└─────────────────┘ └─────────────────┘ └─────────────────┘
```

### 2.2 存储方案

| 方案 | 优点 | 缺点 |
|------|------|------|
| **GitHub Gist** | 免费、无需服务器、API 成熟 | 单文件限制 10MB |
| **专用仓库** | 可管理多文件、版本控制 | 需要维护仓库 |
| **IPFS** | 去中心化 | 复杂度高 |

**推荐**: GitHub Gist 作为主存储

---

## 3. 数据模型

### 3.1 脚本索引

```typescript
interface MarketScript {
  id: string;           // UUID
  name: string;         // 显示名称
  description: string;   // 描述
  author: AuthorInfo;   // 作者信息
  tags: string[];       // 分类标签
  scriptType: 'powershell' | 'python' | 'batch';
  files: ScriptFile[];  // 文件列表
  metadata: ScriptMetadata;
  stats: ScriptStats;
  source: SourceInfo;
}

interface AuthorInfo {
  name: string;
  github?: string;
  avatar?: string;
}

interface ScriptFile {
  name: string;
  path: string;
  gistId?: string;
  size: number;
  checksum: string;
}

interface SourceInfo {
  type: 'gist' | 'repo' | 'direct';
  url: string;
  publishedAt: string;
  updatedAt: string;
}
```

### 3.2 Gist 存储格式

每个脚本使用一个 Gist 存储：

```
gist ID: xxxxxxx
├── script.json      # 元数据
├── script.ps1       # 主脚本
└── icon.png        # 可选图标
```

**script.json 内容:**
```json
{
  "version": "1.0.0",
  "name": "backup-database",
  "description": "...",
  "tags": ["backup", "database"],
  "author": "yukin",
  "scriptType": "powershell",
  "entryPoint": "backup-database.ps1",
  "parameters": [
    {
      "name": "TargetDB",
      "type": "string",
      "required": true,
      "description": "目标数据库名称"
    }
  ]
}
```

---

## 4. CLI 命令

### 4.1 用户命令

```bash
# 浏览市场
scriptmgr market browse [--category CATEGORY] [--page N]
scriptmgr market search QUERY

# 安装脚本
scriptmgr market install SCRIPT_ID [--to PATH]

# 上传脚本
scriptmgr market publish SCRIPT_PATH [--gist] [--description TEXT]

# 管理已安装
scriptmgr market list           # 列出已安装
scriptmgr market update [ID]   # 更新脚本
scriptmgr market remove ID      # 卸载脚本
```

### 4.2 开发者命令

```bash
# 初始化脚本包
scriptmgr market init [--name NAME] [--type TYPE]

# 打包发布
scriptmgr market package
scriptmgr market release [--tag VERSION]

# 验证脚本
scriptmgr market validate [--strict]
```

---

## 5. API 设计

### 5.1 市场索引 API

```bash
# 获取脚本列表
GET /api/market/scripts
  ?category=backup
  &tag=database,automation
  &search=backup
  &page=1
  &limit=20

# 获取热门脚本
GET /api/market/scripts?sort=popular

# 获取脚本详情
GET /api/market/scripts/{id}

# 获取脚本内容
GET /api/market/scripts/{id}/download
```

### 5.2 用户 API

```bash
# 获取已安装脚本
GET /api/market/installed

# 检查更新
GET /api/market/check-update?id={id}

# 收藏脚本
POST /api/market/scripts/{id}/star
DELETE /api/market/scripts/{id}/star
```

---

## 6. 前端集成

### 6.1 市场页面

```
┌─────────────────────────────────────────┐
│ Scripts Market                    🔍 搜索  │
├─────────────────────────────────────────┤
│ 分类: [全部] [备份] [开发] [系统] [网络] │
├─────────────────────────────────────────┤
│ ┌─────┐  ┌─────┐  ┌─────┐  ┌─────┐   │
│ │ 📄  │  │ 📄  │  │ 📄  │  │ 📄  │   │
│ │脚本1 │  │脚本2 │  │脚本3 │  │脚本4 │   │
│ │ ⭐12 │  │ ⭐8  │  │ ⭐23 │  │ ⭐5  │   │
│ └─────┘  └─────┘  └─────┘  └─────┘   │
├─────────────────────────────────────────┤
│              [加载更多]                  │
└─────────────────────────────────────────┘
```

### 6.2 脚本详情页

```
┌─────────────────────────────────────────┐
│ ← 返回                    [安装] [收藏]  │
├─────────────────────────────────────────┤
│ 📄 backup-database                       │
│ by yukin · v1.2.0 · ⭐ 23             │
│                                         │
│ 数据库定时备份脚本，支持 MySQL/PostgreSQL │
│                                         │
│ 标签: #backup #database #automation    │
│                                         │
│ ─────────────────────────────────────── │
│ [说明] [参数] [版本] [评论]             │
│                                          │
│ ## 使用方法                             │
│ ```                                     │
│ ./backup-database.ps1 -TargetDB mydb    │
│ ```                                     │
└─────────────────────────────────────────┘
```

---

## 7. 实现计划

### Phase 1: 基础功能

- [ ] 市场索引 API (GitHub Gist 驱动)
- [ ] `market browse` / `market search` 命令
- [ ] `market install` 命令
- [ ] 基础市场页面 UI

### Phase 2: 上传分享

- [ ] GitHub OAuth 集成
- [ ] `market publish` 命令
- [ ] Gist 创建/更新 API
- [ ] 上传流程 UI

### Phase 3: 高级功能

- [ ] 脚本评分和收藏
- [ ] 更新检查和通知
- [ ] 脚本依赖解析
- [ ] 自动安装依赖

### Phase 4: 社区

- [ ] 用户主页
- [ ] 脚本评论
- [ ] 社区榜单
- [ ] 脚本集锦 (Collection)

---

## 8. 安全考虑

### 8.1 脚本安全

- **签名验证**: 脚本发布者使用私钥签名
- **沙箱执行**: 可选的受限执行环境
- **权限声明**: 脚本声明需要的系统权限

### 8.2 恶意脚本防护

- **审核机制**: 提交后需要审核才能发布
- **举报功能**: 用户可举报可疑脚本
- **安全扫描**: 自动检测常见恶意模式

---

## 9. 部署

### 9.1 当前方案 (v2.0)

使用 GitHub Gist 作为存储，无需服务器：

```
User → scriptmgr CLI → GitHub Gist API → Store/Retrieve scripts
```

### 9.2 未来方案 (v2.x)

如果用户量增长，可迁移到专用后端：

```
User → DeskFlow/Scriptmgr → API Server → PostgreSQL + S3
                            ↓
                        GitHub Gist (backup)
```

---

## 10. 成功标准

- [ ] 用户可以浏览和搜索公开脚本
- [ ] 用户可以安装脚本到本地库
- [ ] 用户可以发布自己的脚本
- [ ] 脚本发现和安装流程 < 30 秒
- [ ] 市场页面加载 < 2 秒

---

## 附录：文件位置

```
ScriptsMarket/
├── SPEC.md                    # 本文档
├── api/                      # 市场 API
│   └── gist.go              # GitHub Gist 封装
├── cmd/
│   └── market/              # market 子命令
│       ├── browse.go
│       ├── search.go
│       ├── install.go
│       ├── publish.go
│       └── list.go
├── store/
│   └── market.db            # 本地已安装脚本索引
└── web/                     # 可选: 市场 Web 页面
```
