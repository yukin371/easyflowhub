# EasyFlowHub API Relay And Extension Phase 1

## Goal

在当前仓库内落地一套可继续扩展的 API relay 与扩展系统基础，而不是把 provider 配置、负载逻辑和插件入口分散到 CLI、HTTP、前端模块与临时文档里。

## External References

- `cc-switch`: 借鉴它的 provider 抽象、热切换、本地 proxy、自动故障转移和 health monitoring 思路。  
  Source: <https://github.com/farion1231/cc-switch>
- `codexmanager`: 借鉴它“安全优先”的配置管理方式，包括 diff / 备份 / 原子写入 / 明确边界。  
  Source: <https://github.com/siddhantparadox/codexmanager>

## Constraint

- `codexmanager` 仓库为 `GPL-3.0`，当前仓库未明确声明兼容 GPL 的整体分发策略。  
  结论：只借鉴设计思路，不直接复制代码。

## Phase 1 Decision

### New Owners

- `scriptmgr-go/internal/relay`
  - owner: OpenAI 兼容 relay、route selection、provider health、failover
- `scriptmgr-go/internal/extensions`
  - owner: manifest 扫描、贡献发现、扩展根目录管理

### Why This Split

- relay 是运行时流量决策，不应该继续散落在 `internal/http` 或 CLI。
- extension registry 是声明式元数据发现，不应该一上来执行任意第三方代码。
- 当前前端“模块系统”本质只是 builtin module toggle，不能冒充真正插件系统。

## Implemented In Phase 1

- `scriptmgr relay serve`
  - 提供 OpenAI 兼容入口 `/v1/*`
  - 提供管理入口 `/api/relay/config`、`/api/extensions`、`/health`
- relay config
  - provider
  - route
  - weighted round robin
  - basic failover on `429` / `5xx`
  - in-memory provider health snapshots
- extension registry
  - 扫描 `plugin.json`
  - 支持 state dir + executable dir + env roots
  - 标记 invalid manifest 与 duplicate id
- starter extension pack
  - `scriptmgr-go/extensions/relay-openai-compatible/plugin.json`
- manager integration
  - `scriptmgr serve` 代理暴露 relay / extension 管理接口
  - manager 新增 Relay 面板，可查看 provider health、编辑 `relay.json`、查看 manifest 扩展
  - 扩展卡片支持将 manifest 中的 relay provider / route 贡献导入当前编辑器；冲突时中止导入并显式报错，新增项写入 `source=extension:<id>`，provider 默认禁用

## Deliberately Deferred

- 任意代码插件执行
- 签名校验、安装/卸载流
- 熔断恢复、实时指标、配额调度
- 非 OpenAI 兼容协议适配

## Next Phase

1. 为 relay 增加真实上游回归，覆盖流式响应、`429`、timeout 与不同鉴权方式
   - progress: `internal/relay/service_test.go` 已补自动化样例，当前剩余 external provider smoke
2. 收敛 provider 密钥管理，避免继续依赖明文 JSON
   - progress: provider 已支持 `api_key_env`，当前剩余存量配置迁移与兼容字段退役策略
3. 为扩展系统增加安装、启停、签名和权限模型
   - design: `2026-04-07-extension-lifecycle-design.md`
