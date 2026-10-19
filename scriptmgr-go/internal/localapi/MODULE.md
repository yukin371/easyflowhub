# internal/localapi MODULE

> status: active
> owner: ScriptMgr local HTTP service maintainers
> last_verified: 2026-10-18

## 职责

`internal/localapi` 为 ScriptMgr 管理 API 与 Relay API 提供共享的本机地址和浏览器来源策略。

## 约定与边界

- CLI 默认地址由 `LoopbackAddress` 生成，固定绑定 `127.0.0.1`。
- 管理 API 显式地址只接受 loopback IP；通配地址和远程网卡绑定必须拒绝。Relay CLI 只接受端口并固定绑定 loopback。
- `WithBrowserOrigins` 只允许 Tauri 打包来源与 Vite 默认开发来源；不带 `Origin` 的本机客户端继续通过，未知来源拒绝并返回 `403`。
- `OPTIONS` 预检由该中间件处理，允许的来源按原值回显并设置 `Vary: Origin`。
- CORS 不是 API 认证，也不能阻止同机进程访问 loopback。监听范围策略与密钥回显边界见 [ADR-0007](../../../docs/decisions/ADR-0007-relay-local-api-security.md)。
