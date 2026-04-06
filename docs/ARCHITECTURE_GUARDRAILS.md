# EasyFlowHub Architecture Guardrails

> status: active
> owner: EasyFlowHub maintainers
> last_verified: 2026-04-07
> verified_against: e524f8522ecf0a15e40af3e6f7627a34e319e81d

## 1. Purpose

本文件定义 EasyFlowHub 的硬边界，重点解决三个问题：

1. 避免跨层重复实现同类能力
2. 避免前端、Tauri、Go 三侧职责漂移
3. 让 AI 在改动前先确认 canonical owner，而不是临时发明新位置

## 2. Layer Map

| Layer / Area | Responsibility | Must Not Do |
|---|---|---|
| `easyflowhub-app/src/components` + `src/pages` | 页面、面板、交互呈现 | 不直接承担原生命令协议、持久化细节、脚本业务逻辑 |
| `easyflowhub-app/src/hooks` + `src/modules` | 前端状态组织、模块注册、模块启停 | 不散落新的 Tauri 命令名或复制原生 bridge |
| `easyflowhub-app/src/lib` | 解析器、typed wrappers、共享前端工具 | 不偷放 feature 业务状态或管理中心布局逻辑 |
| `easyflowhub-app/src-tauri/src` | 桌面壳、窗口生命周期、SQLite、原生命令 | 不重复实现 Go 侧脚本发现、执行和 MCP 路由 |
| `scriptmgr-go/internal/*` | 脚本发现、执行、任务、HTTP、MCP、notes sync | 不反向拥有 Tauri UI 状态或桌面窗口行为 |
| `docs/*` | 当前目标、架构约束、模块上下文、决策生命周期 | 不复制源码可直接推导的大段结构说明 |

## 3. Dependency Direction

Allowed dependency direction:

```text
React components / pages
  -> hooks / modules / lib
  -> lib/tauri wrappers
  -> src-tauri native commands
  -> scriptmgr-go internal services
  -> SQLite / JSON / filesystem
```

Forbidden examples:

- `src/components` 不应直接硬编码 Tauri `invoke('...')` 命令名
- `src-tauri/src` 不应新增脚本发现 / 执行 / MCP 动态路由的平行实现
- `scriptmgr-go/internal/mcp` 不应吸收 discovery / executor 的核心业务逻辑
- 文档层不应继续维护与源码分叉的大型“全量结构抄录”文档

## 4. Canonical Owners

Every cross-cutting concern must have one canonical owner.

| Concern | Canonical owner | Reuse rule | Duplicate status |
|---|---|---|---|
| Frontend native command wrappers | `easyflowhub-app/src/lib/tauri` | 新前端原生命令优先扩展此目录 | `forbidden` |
| Manager workspace shell | `easyflowhub-app/src/components/manager` | 管理中心布局、导航、面板切换集中在此 | `restricted` |
| Feature module registration | `easyflowhub-app/src/modules` | 新模块通过注册系统进入 manager | `forbidden` |
| Notes SQLite persistence | `easyflowhub-app/src-tauri/src/notes.rs` | 笔记库 CRUD / trash / image 落盘集中在此 | `forbidden` |
| App settings persistence | `easyflowhub-app/src-tauri/src/settings.rs` | 设置读写集中于此，不在前端复制持久化协议 | `forbidden` |
| Window / appearance / widgets | `easyflowhub-app/src-tauri/src/lib.rs`, `appearance.rs`, `widget.rs` | 原生窗口与桌面行为不在 UI 侧复制 | `forbidden` |
| Script discovery / execution | `scriptmgr-go/internal/api`, `discovery`, `executor`, `runtime` | 扩展脚本业务时优先进入 Go 侧 | `forbidden` |
| MCP server + dynamic tool routing | `scriptmgr-go/internal/mcp` | schema、router、notification 只在此维护 | `forbidden` |
| External MCP client wrappers | `scriptmgr-go/internal/mcpcli` | 调用外部 MCP 用 client，不与 server 混用 | `forbidden` |
| Notes repo sync | `scriptmgr-go/internal/notes` | 文件仓库同步只在此维护 | `restricted` |
| Shared frontend parsing helpers | `easyflowhub-app/src/lib` | Markdown / todo / image parsing 优先扩展现有 helper | `restricted` |

## 5. New Shared Capability Gate

Before adding a new shared helper, service or abstraction, the implementer must answer:

1. 搜过哪些现有实现？
2. 为什么不能扩展现有实现？
3. 新能力的 canonical owner 是谁？
4. 会不会让 TS / Rust / Go 任意两侧同时拥有同一职责？
5. 哪些 `MODULE.md` / `roadmap` / `ADR` 需要同步？

任何一项答不清，就不要新增。

## 6. Naming And Placement Rules

- 前端原生命令 wrapper 只放 `easyflowhub-app/src/lib/tauri`
- 管理中心模块定义只放 `easyflowhub-app/src/modules/builtin` 或其注册入口
- 前端 parser / formatter 只放 `easyflowhub-app/src/lib`
- 桌面原生命令只放 `easyflowhub-app/src-tauri/src`
- `scriptmgr` 协议层代码只放 `scriptmgr-go/internal/http`、`internal/mcp`、`internal/mcpcli`
- 临时迁移适配层必须在注释或文档中显式标记 cleanup 条件

## 7. Forbidden Patterns

- 在多个 package 中复制同一种 helper 或 service
- 把 domain 规则塞进名字泛化的 `utils` 文件夹
- 为了赶进度绕开 `src/lib/tauri` 直接在组件中写命令名
- 在 Rust 侧复制 Go 侧已有的脚本业务逻辑
- 在 `docs/plans` 长期堆积已完成方案而不收敛到 roadmap / ADR / MODULE

## 8. Exception Process

Temporary exceptions are allowed only if all of the following are recorded:

- reason
- owner
- expiration condition
- cleanup plan
- verification risk

Record exceptions in:

- `docs/plans/...` for short-lived work
- `docs/decisions/...` for long-lived exceptions

## 9. Verification

Recommended enforcement mechanisms:

- `bun run test`
- `bunx tsc --noEmit`
- `cargo check --manifest-path src-tauri/Cargo.toml`
- `go test ./...`
- PR review 检查是否出现新的 duplicate owner
- 修改边界或 owner 后同步 `MODULE.md` / `roadmap` / `ADR`
