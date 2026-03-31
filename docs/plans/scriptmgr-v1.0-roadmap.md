# scriptmgr v1.0 Roadmap

**项目**: scriptmgr-go - 个人脚本管理工具，带 MCP 接口
**版本**: v1.0
**日期**: 2026-03-25
**目标**: 面向个人开发者的 AI 辅助脚本管理工具

---

## 1. 概述

scriptmgr 是一个 CLI 工具和 MCP 服务器，帮助开发者通过统一接口管理和执行脚本（PowerShell、Python、Batch）。它为 AI 助手提供渐进式脚本工具披露功能。

### 当前状态
- 基础 CLI：list, run, describe, history, tasks, sessions, favorites, roots
- MCP 服务器，支持渐进式工具加载
- 异步脚本执行和任务跟踪
- 基于 SQLite 的任务存储
- 与 Claude 的笔记集成

### v1.0 目标范围
更简单、更健壮的个人工具 - 专注于核心功能而非功能堆砌。

---

## 2. 需要修复的问题

### 2.1 严重问题：测试失败
**文件**: `internal/mcp/router_test.go`
**问题**: `TestToolRouter_LRUEviction` 测试失败

**根因**: 当 `LoadCategory` 发现分类已加载时，会更新 `LastUsedAt` 但提前返回，不会触发驱逐逻辑。

**修复**: 驱逐检查应在"已加载"检查之前触发。

---

## 3. 性能改进

### 3.1 脚本发现缓存
**当前**: `DiscoverScripts()` 每次调用都遍历文件系统
**问题**: 无缓存，大型脚本目录速度慢
**解决方案**: 添加带 TTL 的内存缓存

### 3.2 历史记录优化
**当前**: `AppendHistory()` 每次加载全部 200 条记录
**问题**: O(n) 复杂度，随历史记录增长
**解决方案**: 使用追加写入文件或流式处理

### 3.3 进程检查优化
**当前**: `processExists()` 调用外部 `tasklist` 命令
**问题**: 简单检查开销大
**解决方案**: 直接使用 `syscall` 或降低检查频率

---

## 4. 日志系统

### 4.1 当前状态
- 使用 `fmt.Printf` 输出到 stderr
- 无日志级别
- 无结构化日志

### 4.2 需求
- 日志级别：DEBUG, INFO, WARN, ERROR
- 结构化输出（JSON）便于程序解析
- 可配置输出（stdout/stderr/file）
- 文件输出日志轮转

### 4.3 实现
```
使用 slog（Go 1.21+ 内置）
- 默认输出到 stderr
- 文件输出使用 JSON 格式
- 包含：时间戳、级别、消息、模块
```

---

## 5. 生产环境准备

### 5.1 缺失的 CLI 标志
- `--version` 标志
- `--help` 一致性
- `--log-level` 标志

### 5.2 HTTP 服务器改进
- 健康检查端点：`GET /health`
- 优雅关闭（排出连接）
- 请求超时中间件

### 5.3 配置
- 配置文件支持（YAML/JSON）
- 环境变量覆盖
- 启动时验证

---

## 6. 测试覆盖率

### 6.1 当前状态
```
Go 文件总数: 43
测试文件: 12 (~28% 覆盖率)
无测试的包:
- api（关键）
- discovery
- store
- config
```

### 6.2 目标覆盖率
- 核心包：60%+ 覆盖率
- API 层：基于 Mock 的测试
- Store：基于接口提高可测试性

### 6.3 测试策略
- 纯函数的单元测试
- Store 的集成测试
- 外部依赖的 Mock 接口

---

## 7. 小改改进

### 7.1 CLI 增强
- 状态输出彩色化
- 长时间操作进度指示
- 更好的错误消息

### 7.2 历史记录管理
- `history purge` 命令
- 可配置的历史记录限制
- 导出历史记录为 CSV/JSON

### 7.3 脚本元数据
- 支持 YAML 元数据文件
- 从注释自动检测脚本描述

---

## 8. v1.0 范围外

以下内容推迟到 v1.0 之后：
- 团队协作功能
- 脚本调度/cron
- Web UI 仪表板
- 云同步
- 认证/授权
- 脚本编辑器
- 脚本分享/导出

---

## 9. 实施阶段

### 阶段 1：修复关键问题
- [x] 修复 `TestToolRouter_LRUEviction` - 测试现已通过
- [x] 验证所有测试通过

### 阶段 2：日志系统
- [x] 使用 slog 实现结构化日志
- [x] 将 main.go, serve.go, mcp.go, server.go 中的 fmt.Fprintf(stderr) 替换为 slog
- [x] 用户面向输出保留 fmt.Printf

### 阶段 3：性能
- [ ] 添加脚本发现缓存
- [ ] 优化历史记录追加

### 阶段 4：生产环境完善
- [x] 添加 --version 标志
- [x] 改进 --help 输出

### 阶段 5：测试覆盖率
- [x] 为 api 包添加测试（containsFold, containsPath, normalizeRootPath）
- [x] 为 discovery 包添加测试（matchesSearch, isSameOrNestedRoot）
- [x] 修复 bug：matchesSearch 大小写不敏感搜索

---

## 10. v1.0 完成状态

| 标准 | 状态 |
|------|------|
| 所有测试通过 | ✅ |
| 测试覆盖率提升 | ✅（新增 api, discovery 测试）|
| 结构化日志已实现 | ✅ |
| --version 标志已添加 | ✅ |
| 用户输出保留 fmt.Printf | ✅ |
| 脚本发现缓存 | ⏳ 待完成 |
| 历史记录优化 | ⏳ 待完成 |

---

## 11. 审查期间发现的 Bug

1. **matchesSearch 大小写不敏感 bug**（已在 discovery.go 修复）
   - `strings.Contains(strings.ToLower(value), search)` 只对 value 进行了小写转换，search 未转换
   - 修复：对两边都进行小写转换

---

## 12. 成功标准

v1.0 就绪条件：
1. 所有测试通过（包括新增测试）
2. 核心包测试覆盖率 >= 60%
3. 生产代码路径中无 fmt.Printf
4. HTTP 服务器有健康检查
5. CLI 有 --version 标志
6. 脚本发现使用缓存

---

## 附录：文件位置

### 关键源文件
- `cmd/scriptmgr/main.go` - 入口点
- `internal/cli/` - CLI 命令
- `internal/api/` - 业务逻辑 API
- `internal/discovery/` - 脚本发现
- `internal/executor/` - 脚本执行
- `internal/mcp/` - MCP 服务器
- `internal/store/` - 持久化

### 配置位置
- 默认状态目录：`~/.config/scriptmgr/`
- MCP 配置：`~/.config/scriptmgr/mcp_config.json`
