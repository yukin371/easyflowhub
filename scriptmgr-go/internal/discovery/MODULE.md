# internal/discovery MODULE

> status: active
> owner: EasyFlowHub script runtime maintainers
> last_verified: 2026-04-11

## 职责

`internal/discovery` 负责把仓库默认目录、用户持久化 `roots.json` 和 extension `script_roots` 合并为脚本发现的唯一入口，并据此扫描脚本元数据。

## 相关文档

- 当前版本目标与阶段顺序：`docs/roadmap.md`
- Stage B 真实消费链方案：`docs/plans/2026-04-11-stage-b-contribution-consumption-plan.md`
- 扩展聚合 owner：`scriptmgr-go/internal/extensions/MODULE.md`

## 数据流

repo auto roots + persisted `roots.json` + extension effective `script_roots` -> `ScriptRoots()` -> `DiscoverScripts()` -> API / HTTP / MCP

## 约定 & 陷阱

- `internal/discovery` 只消费 `EffectiveContributions`，不自行扫描扩展目录中的 manifest。
- extension `script_roots` 是 read-only overlay，不写回 `roots.json`。
- 相对路径的 extension root 按扩展目录解析，不按当前工作目录解析。
- duplicate / nested roots 继续在本模块统一去重；单个缺失目录不应拖垮全局扫描。
