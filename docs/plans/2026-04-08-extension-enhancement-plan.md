# EasyFlowHub 可扩展性增强方案

> date: 2026-04-08
> status: designed
> scope: `scriptmgr-go/internal/extensions`, `scriptmgr-go/internal/relay`, `easyflowhub-app`

## 目标

让 EasyFlowHub 拥有类似 VSCode 的扩展机制，支持第三方通过声明式 manifest 贡献 provider、route、script roots 和 manager 模块，并支持运行时动态加载/卸载，不需要重启应用。

## VSCode 扩展模型对齐

参考 VSCode 扩展系统的核心要素，本方案选取以下三个维度：

| VSCode 概念 | 本项目对应 | 实现阶段 |
|-------------|-----------|---------|
| Contribution Points | `plugin.json` contributions | Phase 1 |
| Activation Events | enable/disable 状态 | Phase 2 |
| File Watching + Reload | 目录监控 + 动态重扫描 | Phase 3 |

不纳入 v1 范围：隔离 Extension Host（goroutine 级隔离可后续再做）。

## 核心改进点

### 1. Registry v2 — 贡献聚合层

当前 registry 只做目录扫描，返回 `ListedExtension` 列表。需要新增 `ContributionAggregator`：

```
ExtensionRoots -> Registry.Scan() -> ListedExtensions
                                 -> ContributionAggregator.Merge() -> EffectiveContributions
```

`EffectiveContributions` 合并所有 enabled 扩展的 contributions，过滤掉 disabled 扩展的贡献，对 relay 和 manager 暴露统一的贡献视图。

### 2. 激活状态模型

扩展状态分三层：

```
manifest.status     -> "loaded" | "invalid"
state.json enabled  -> true | false
computed            -> EffectiveStatus("active" | "inactive" | "error")
```

只有 `EffectiveStatus == "active"` 的扩展，其 contributions 才会被聚合。

### 3. 目录监控 + 热重载

使用 `fsnotify` 监视所有 extension roots。当 `plugin.json` 或目录结构变化时：

1. 接收文件系统事件
2. 增量更新 registry 缓存
3. 重新计算 EffectiveContributions
4. 通知订阅者（relay config 重新加载、manager UI 刷新）

### 4. Relay 动态配置合并

当前 relay 的 provider/route 来自静态 `relay.json`。增强后：

```
StaticConfig (relay.json)
  + ExtensionContributions (from EffectiveContributions)
  = EffectiveRelayConfig
```

relay 使用 merged config 路由请求，但不持久化 extension contributions 到 `relay.json`。

## 分阶段实施计划

### Phase 1: Contribution Aggregation（1-2 周）

**目标**：建立扩展贡献的聚合与验证层。

**工作项**：
- 新增 `ContributionAggregator` 结构，计算 effective contributions
- 扩展 `types.go` contributions 验证逻辑（header allowed keys、URL 格式等）
- relay service 新增 `EffectiveConfig()` 方法，返回 merged view
- 新增 `GET /api/extensions/contributions` 端点，查看所有 contributions
- 单元测试覆盖 contributions merge 逻辑

**验证标准**：
- manager 可以看到所有 enabled 扩展的 provider/route 贡献
- relay 在路由时可以命中 extension 贡献的 provider

---

### Phase 2: Activation Model（1 周）

**目标**：实现 enable/disable 生命周期和状态持久化。

**工作项**：
- 新增 `<state-dir>/extensions/state.json` 读写
- `Registry` 新增 `Enable(id)` / `Disable(id)` / `GetEffectiveStatus(id)`
- 修改 `ContributionAggregator`，只聚合 enabled 扩展的 contributions
- `state.json` 字段：`enabled`、`installed_version`、`source`、`installed_at`
- CLI 新增：`scriptmgr extensions enable <id>` / `disable <id>`
- manager Extensions 面板显示 enabled 状态，支持 toggle

**验证标准**：
- disable 一个扩展后，其 contributions 立即从 relay 路由中消失（不需要重启）
- 重启后 enabled 状态保持

---

### Phase 3: File Watching + Hot Reload（1 周）

**目标**：监视扩展目录变化，动态更新 registry 和 contributions。

**工作项**：
- 引入 `github.com/fsnotify/fsnotify` 依赖
- `Registry` 新增 `StartWatching()` / `StopWatching()`
- 当 `plugin.json` 变更时，增量刷新该扩展的 manifest
- 当扩展目录增删时，重新扫描并更新 contributions
- 变更事件通过 channel 传递给 `ContributionAggregator`
- relay 订阅变更事件，自动重新计算 effective config

**验证标准**：
- 在运行时往 extensions 目录添加新扩展，relay 能立即感知（不需要重启）
- 删除一个扩展目录，其 contributions 从 live config 中消失

---

### Phase 4: Install / Uninstall 流程（1-2 周）

**目标**：让用户可以安装/卸载第三方扩展。

**工作项**：
- `scriptmgr extensions install <path>` CLI 命令
- staging dir + atomic move 安装流程
- 目录布局收敛：`installed/<id>@<version>/plugin.json`
- `scriptmgr extensions uninstall <id>`
- manager Extensions 面板新增 Install / Uninstall 按钮

**验证标准**：
- 可以从本地目录安装扩展，安装后立即出现在 extensions 列表中
- uninstall 后，目录清理干净，contributions 消失

---

### Phase 5: Signature Verification（后续版本）

**目标**：签名验证确保扩展来源可信。

**工作项**：
- `plugin.sig` detached signature 支持
- `<state-dir>/extensions/trust/keys.json` 信任公钥管理
- bundled 扩展走内置公钥白名单验证
- installed 扩展必须签名通过才能启用

---

## API 变更

### 新增端点

```
GET    /api/extensions              # 列表（含 effective status）
GET    /api/extensions/contributions  # 所有 enabled 扩展的贡献聚合
POST   /api/extensions/reload      # 手动触发重扫描
POST   /api/extensions/<id>/enable
POST   /api/extensions/<id>/disable
```

### 新增 CLI 命令

```
scriptmgr extensions list
scriptmgr extensions enable <id>
scriptmgr extensions disable <id>
scriptmgr extensions install <path>
scriptmgr extensions uninstall <id>
```

## 数据流

```
[Extension Roots]
       |
       v
[Registry.Scan()] --> [ListedExtensions]
       |
       v
[State Loading] --> [EffectiveStatus]
       |
       v
[ContributionAggregator.Merge()]
       |
       v
[EffectiveContributions] --> [Relay Service (live)]
                          --> [Manager UI]
                          --> [/api/extensions/contributions]
```

## 目录布局（目标）

```
<state-dir>/
  extensions/
    state.json                    # 扩展启用状态
    installed/                    # 已安装扩展
      <id>@<version>/
        plugin.json
    bundled/                      # 内置扩展（未来）
      <id>/
        plugin.json
    trust/
      keys.json                   # 信任公钥

<exe-dir>/
  extensions/
    bundled/                      # 随分发包内置
      <id>/
        plugin.json
```

## 风险与应对

| 风险 | 应对 |
|------|------|
| 目录监控在高负载下频繁触发 | debounce 机制，100ms 内合并多次事件 |
| extension 贡献与 static config 冲突 | extension contributions 只读，冲突时 static 优先 |
| 扩展目录被恶意写入 | v1 不执行第三方代码，只读声明式 manifest |

## Open Questions

1. 多版本共存：每个 id 是否只保留一个 active version？
2. 扩展贡献冲突时是否有优先级？
3. hot reload 触发时，relay 现有请求是否中断？
