# internal/relay MODULE

> status: active
> owner: EasyFlowHub script runtime maintainers
> last_verified: 2026-04-07

## 职责

`internal/relay` 是 OpenAI 兼容 API relay 的唯一 owner，负责 provider 配置、route 选择、基础 failover、health 状态和 proxy 请求转发。

## 数据流

relay config -> route match by path/model -> pick healthy provider -> forward request -> record success/failure -> expose snapshot

## 约定 & 陷阱

- provider 选路和失败切换只能写在这里，不能散到 `internal/http`、CLI 或前端。
- v1 只保证 OpenAI 兼容路径和基础 `429` / `5xx` failover，不代表所有 provider 差异都已统一。
- route 和 provider 的默认策略是最小可用版本，后续熔断、配额和观测应继续在本模块内演进。
- relay config 中的 `source` 只用于来源追踪和管理层展示，不参与路由选择语义；扩展导入时会写成 `extension:<id>`。
