# src-tauri/src MODULE

> status: active
> owner: EasyFlowHub desktop runtime maintainers
> last_verified: 2026-04-07
> verified_against: e524f8522ecf0a15e40af3e6f7627a34e319e81d

## 1. Responsibility

`src-tauri/src` 是桌面原生壳层，负责窗口生命周期、快捷键、托盘、SQLite 持久化、Tauri 命令注册，以及把前端请求桥接到 `scriptmgr-go`。

## 2. Owns

This module is the canonical owner of:

- Tauri 插件初始化与命令注册
- manager / quick note / todo card 的原生窗口行为
- 笔记 SQLite CRUD、图片落盘与回收站
- 应用设置持久化
- 桌面外观、小组件与 MCP 信息暴露
- 与 `scriptmgr-go` 的桌面侧桥接

## 3. Must Not Own

This module must not contain:

- React UI 状态管理
- `scriptmgr-go` 已有的脚本发现、执行或 MCP 路由核心逻辑
- 大量前端解析器或业务展示逻辑

## 4. Entry Points

| Entry point | Role | Notes |
|---|---|---|
| `main.rs` | binary entry | 委托到 `lib.rs::run()` |
| `lib.rs` | runtime shell | 插件、命令、窗口、快捷键、托盘入口 |
| `notes.rs` | notes persistence | SQLite、图片、trash |
| `settings.rs` | settings store | 应用级设置 |
| `scriptmgr.rs` | script bridge | 调用 `scriptmgr-go` CLI / HTTP |
| `appearance.rs` / `widget.rs` / `modules.rs` / `mcp_server.rs` | native feature owners | 外观、桌面小组件、模块配置、MCP 信息 |

## 5. Key Dependencies

| Dependency | Why it exists | Risk if changed |
|---|---|---|
| Tauri plugins | SQLite、window state、shortcut、autostart 等原生能力 | 插件升级或 capability 变化容易造成平台回归 |
| `scriptmgr-go` binary | 脚本运行与外部协议桥接 | 路径解析或输出协议变化会破坏桌面层 |
| SQLite schema | 笔记与状态存储 | schema 变化需要同步数据兼容 |

## 6. Dependents

Main callers / consumers:

- `easyflowhub-app/src/lib/tauri`
- `easyflowhub-app/src/pages`
- `easyflowhub-app/src/components`

## 7. Invariants

These must remain true after changes:

- 新前端可调用命令必须在 `lib.rs` 注册，并在前端补 typed wrapper
- manager 窗口关闭时默认隐藏，不应直接退出应用
- quick note / todo card 标签命名必须保持唯一性
- Windows 特有行为（autostart、桌面固定、快捷键）变动后必须回归验证

## 8. Common Pitfalls

- 只改前端 wrapper 而忘记 Rust 注册
- 快捷键、防抖与窗口焦点行为相互影响
- 开发态与打包态 `scriptmgr` 路径解析差异
- Windows 特有 API 变动后没有回归桌面固定或 autostart

## 9. Reuse Rules

Before adding new code here, check:

- 是否已有对应的 Rust 命令模块可扩展
- 新能力是否真的属于桌面壳，而不是应下沉到 `scriptmgr-go`
- 是否需要同步更新前端 wrapper、类型与文档

## 10. Verification

- Primary verification: `cd easyflowhub-app && cargo check --manifest-path src-tauri/Cargo.toml`
- Secondary verification: `cd easyflowhub-app && bun tauri dev` 手工 smoke

## 11. Doc Sync Triggers

Update this file when any of these change:

- 命令注册边界
- 原生窗口 / 插件 owner
- 与 `scriptmgr-go` 的桥接职责
- 关键不变量
- 常见坑
