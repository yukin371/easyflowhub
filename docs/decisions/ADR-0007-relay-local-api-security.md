# ADR-0007：Relay 本地 API 暴露与密钥回显边界

## 状态

已接受（2026-10-18，v0.2.0 发布前安全收口）

## 背景

ScriptMgr 管理 API 与 OpenAI 兼容 Relay 都供本机桌面应用使用。两项服务此前默认监听所有网卡，并对任意浏览器来源返回通配 CORS；Relay 配置快照及扩展贡献视图还会回显 `api_key` 和认证类 header。Manager 以 JSON 编辑配置，因此读取时隐藏密钥不能导致无修改保存时丢失密钥。

## 决策

- `scriptmgr serve` 与 `scriptmgr relay serve` 只允许绑定 loopback IP（`127.0.0.0/8` 或 `::1`）。管理 API 拒绝通配地址、主机名和非 loopback IP；远程管理不在当前产品支持范围内。Relay CLI 只接受端口并固定绑定 `127.0.0.1`。
- 两项 HTTP 服务共用 `scriptmgr/internal/localapi` 的浏览器来源策略，只允许 Tauri 的 `tauri://localhost` / `http://tauri.localhost` 和 Vite 默认开发来源 `http://localhost:1420` / `http://127.0.0.1:1420`。不带 `Origin` 的本机 CLI 请求继续可用，未知浏览器来源返回 `403`。
- Relay 配置快照、provider 状态视图、扩展清单和有效贡献对 API 输出前，脱敏 `api_key`、含 userinfo / query / fragment 的 `base_url`，以及名称包含 auth、key、token、secret、credential、password、cookie、signature、session、assertion、bearer、private 或 dpop 的非空 header 值；返回的是副本，不修改运行时 provider 配置。
- Manager 回传未改动的 `__EASYFLOWHUB_REDACTED__` 时，服务按 provider id 与 header 名从当前配置恢复原值；未改动的敏感 `base_url` 也会恢复。用户清空/删除密钥字段会移除密钥；新增 provider 没有可恢复的旧值时，不会把该标记当作凭据持久化。`api_key_env` 仍按普通配置字段显示。
- `api_key` 继续兼容并以明文保存在本地 `relay.json`；新配置推荐使用 `api_key_env`。本地 API 不引入账号认证，也不把 CORS 当作认证机制。

## 后果与限制

- 桌面服务不能通过 CLI 绑定到远程网卡，因此未认证管理 API 不会意外暴露到局域网；浏览器型调用另受来源白名单约束。
- CORS 只约束浏览器读取，不阻止同机进程直接访问 loopback API。当前本地服务仍信任本机用户会话和本机进程边界。
- 明文 `api_key` 的磁盘保护没有改变；环境变量或后续独立系统密钥存储仍是进一步收敛方向。
- header 脱敏按名称启发式匹配，不能识别所有自定义凭据命名。Provider 密钥优先使用 `api_key_env`，不要把秘密放进名称无法识别的自定义 header。

## 验证

- `go test ./internal/localapi ./internal/http ./internal/relay ./internal/cli`
- `go test ./...`
- `bun run test -- src/lib/api/scriptmgr.test.ts`
- `bunx tsc --noEmit`
