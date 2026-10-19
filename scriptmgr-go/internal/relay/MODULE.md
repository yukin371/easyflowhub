# internal/relay MODULE

> status: active
> owner: EasyFlowHub script runtime maintainers
> last_verified: 2026-10-18

## 职责

`internal/relay` 是 OpenAI 兼容 API relay 的唯一 owner，负责 provider 配置、route 选择、基础 failover、health 状态和 proxy 请求转发。

## 数据流

relay config -> route match by path/model -> pick healthy provider -> forward request -> record success/failure -> expose snapshot

## 约定 & 陷阱

- provider 选路和失败切换只能写在这里，不能散到 `internal/http`、CLI 或前端。
- v1 只保证 OpenAI 兼容路径和基础 `401` / `403` / `429` / `5xx` failover，不代表所有 provider 差异都已统一。
- 候选耗尽时，全部请求均限流才返回 `429`；全部鉴权失败才返回最后一次 `401` / `403`。混合错误、网络异常和无候选仍返回 `502`，避免误报限流或鉴权。该响应使用固定错误正文，不回显上游内容或密钥；后续 Provider 成功时仍直接返回成功结果。
- 当前唯一正式承诺的负载策略是 `weighted_round_robin`；更复杂的熔断恢复、配额和调度仍不在 v1/v1.1 正式承诺范围内。
- route 和 provider 的默认策略是最小可用版本，后续熔断、配额和观测应继续在本模块内演进。
- relay config 中的 `source` 只用于来源追踪和管理层展示，不参与路由选择语义；扩展导入时会写成 `extension:<id>`。
- provider 密钥优先通过 `api_key_env` 从环境变量解析；明文 `api_key` 仅保留兼容路径，不应继续作为推荐配置。
- Relay 与 ScriptMgr HTTP 的浏览器来源校验由 `internal/localapi` 统一拥有；默认监听 `127.0.0.1`，CORS 不作为本机 API 的认证机制。
- 所有 API 快照都必须脱敏 `api_key` 和认证类 header，包括 provider 状态、扩展清单与有效贡献。脱敏结果只用于 API 输出；转发必须使用原始配置。保存未改动的 `__EASYFLOWHUB_REDACTED__` 时保留当前密钥，清空/删除则清除。
- relay 的协议级回归样例优先收敛在 `service_test.go`，使用真实 HTTP upstream 行为覆盖 stream / `429` / timeout / auth；`internal/http` 和 CLI 只验证 wiring。
