# EasyFlowHub Extension Lifecycle Design

> date: 2026-04-07
> status: designed, pending implementation
> scope: `scriptmgr-go/internal/extensions`, manager Relay / extension surfaces

## Goal

把当前“只读 manifest 扫描”推进到下一阶段前，先明确扩展的安装、启停、签名、目录约束和权限边界，避免后续在 CLI、HTTP、manager 和磁盘布局上再次各做一套。

## Current Baseline

当前仓库已经具备：

- `internal/extensions` 扫描 `plugin.json`
- state dir / executable dir / env roots 多来源发现
- duplicate id / invalid manifest 标记
- manager 中查看扩展贡献，并可导入 relay provider / route

当前仓库还没有：

- 安装 / 卸载流程
- enable / disable 状态持久化
- 签名验证
- 受控目录布局

## Non-Goals

- v1.1 之前不执行任意第三方代码插件
- 不把 manager builtin module toggle 伪装成通用插件系统
- 不引入远程自动下载和自动更新

## Lifecycle Model

扩展生命周期定义为：

`discover -> validate -> verify -> install -> enable -> contribute -> disable -> uninstall`

其中：

- `discover / validate` 已存在
- `verify / install / enable / disable / uninstall` 是下一阶段新增能力
- `contribute` 仍然只代表声明式配置贡献，不执行第三方代码

## Directory Constraints

扩展目录拆成三类：

### 1. Bundled Extensions

- 位置：`<exe-dir>/extensions/bundled/<extension-id>/plugin.json`
- 来源：随应用或 `scriptmgr` 分发
- 权限：只读
- 生命周期：可启用/禁用，不允许卸载

### 2. Installed Extensions

- 位置：`<state-dir>/extensions/installed/<extension-id>@<version>/plugin.json`
- 来源：受控安装流程写入
- 权限：可安装、启用/禁用、卸载
- 要求：只能通过 `scriptmgr extensions install/uninstall` 改动

### 3. Dev / Override Roots

- 位置：`SCRIPTMGR_EXTENSION_DIRS`
- 来源：开发者本地目录
- 权限：只读发现，不纳入正式安装状态
- 目的：开发调试、临时覆盖、CI 验证

结论：

- 下一阶段不再把 `<exe-dir>/extensions` 和 `<state-dir>/extensions` 当作无结构散目录直接扫描
- registry 应优先扫描上述三类明确子目录，并为每个扩展标注 source type：`bundled`、`installed`、`dev`

## State Model

新增扩展状态文件：

- 路径：`<state-dir>/extensions/state.json`

建议字段：

```json
{
  "version": 1,
  "extensions": {
    "relay-openai-compatible": {
      "enabled": true,
      "installed_version": "1.0.0",
      "source": "bundled",
      "installed_at": "2026-04-07T00:00:00Z"
    }
  }
}
```

规则：

- 默认行为：
  - `bundled` 默认启用
  - `installed` 默认禁用，需显式启用
  - `dev` 默认禁用，除非显式开启 dev mode
- registry 返回列表时要合并 manifest 与 state，暴露 effective status
- enable / disable 只改变 state，不改 manifest

## Install / Uninstall Flow

### Install

建议新增：

- `scriptmgr extensions install <path>`

约束：

- 只接受本地目录或本地归档文件
- 安装过程必须：
  - 解包到 staging dir
  - 校验 manifest
  - 执行签名校验
  - 检查 duplicate id 与版本冲突
  - 原子移动到 `installed/<id>@<version>`
  - 更新 `state.json`

### Uninstall

建议新增：

- `scriptmgr extensions uninstall <id>`

约束：

- 只允许删除 `installed` 源的扩展
- `bundled` 和 `dev` 扩展不允许卸载
- uninstall 前先 disable，再清理目录和 state

## Enable / Disable Flow

建议新增：

- `scriptmgr extensions enable <id>`
- `scriptmgr extensions disable <id>`

规则：

- enable / disable 是状态切换，不复制文件
- 当前 manager 和 relay 面板读取扩展列表时，只展示 effective status，不直接猜测目录存在即启用
- 对 relay provider / route 贡献，只有 enabled 扩展才允许进入“可导入候选”

## Signing Model

v1.1 建议采用最小可信模型：

- 每个扩展包含：
  - `plugin.json`
  - `plugin.sig`
- `plugin.sig` 是 `plugin.json` 的 detached signature
- 信任公钥放在：
  - `<state-dir>/extensions/trust/keys.json`

规则：

- `bundled` 扩展允许通过仓库内置公钥白名单验证
- `installed` 扩展必须签名通过才能进入 managed install
- `dev` 扩展默认跳过签名，但列表中必须标记 `unverified`

如果签名能力在下一阶段实现成本过高，允许先走“双阶段落地”：

1. 先实现目录约束 + install/enable/disable
2. 再补 detached signature 与信任链

但无论如何，签名必须在进入“可公开安装生态”前落地。

## API / UI Impact

下一阶段需要补齐：

- CLI
  - `extensions install`
  - `extensions uninstall`
  - `extensions enable`
  - `extensions disable`
- HTTP API
  - `POST /api/extensions/install`
  - `POST /api/extensions/{id}/enable`
  - `POST /api/extensions/{id}/disable`
  - `DELETE /api/extensions/{id}`
- manager
  - 展示 source type、enabled、verified、managed/unmanaged 状态
  - 对 `dev` / `bundled` / `installed` 提供不同动作按钮

## Recommended Rollout

### Phase 2A

- 目录约束
- `state.json`
- enable / disable
- manager 状态展示

### Phase 2B

- 本地 install / uninstall
- staged install + atomic move
- duplicate / version 冲突处理

### Phase 2C

- detached signature
- trust key management
- manager 安装流

## Open Questions

- `installed/<id>@<version>` 是否保留多版本共存，还是每个 id 只保留一个 active version
- `dev` 扩展在 manager 中是否默认隐藏，避免普通用户误触
- relay contribution 导入后，扩展被 disable 时是否需要提示“已有导入配置不会自动删除”

## Conclusion

下一阶段应继续坚持：

- `internal/extensions` 只拥有声明式扩展生命周期，不执行任意第三方代码
- 安装、启停、签名、目录约束都必须围绕 manifest registry 展开，而不是另起一个平行插件系统
