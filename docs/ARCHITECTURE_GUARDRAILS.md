# EasyFlowHub Architecture Guardrails

> status: active
> owner: EasyFlowHub maintainers
> last_verified: 2026-04-07
> verified_against: 752435f + 2026-04-07 working tree

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
| `scriptmgr-go/internal/*` | 脚本发现、执行、任务、HTTP、MCP、relay、extensions、notes sync | 不反向拥有 Tauri UI 状态或桌面窗口行为 |
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
- `scriptmgr-go/internal/http` 不应私自拥有 relay 的 provider 选择和失败切换逻辑
- `scriptmgr-go/internal/extensions` 在 v1 不应执行任意插件代码，只加载声明式贡献
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
| Window / appearance / widgets | `easyflowhub-app/src-tauri/src/{lib,launcher,app_windows,appearance,widget}.rs` | 原生窗口与桌面行为不在 UI 侧复制；`lib.rs` 只做装配 | `forbidden` |
| Inbox capture persistence | `easyflowhub-app/src-tauri/src/inbox/`（schema / attachments / repository / service / commands） | 捕获、归档、附件与转笔记只经此模块 | `forbidden` |
| Script discovery / execution | `scriptmgr-go/internal/api`, `discovery`, `executor`, `runtime` | 扩展脚本业务时优先进入 Go 侧 | `forbidden` |
| MCP server + dynamic tool routing | `scriptmgr-go/internal/mcp` | schema、router、notification 只在此维护 | `forbidden` |
| External MCP client wrappers | `scriptmgr-go/internal/mcpcli` | 调用外部 MCP 用 client，不与 server 混用 | `forbidden` |
| API relay / provider routing | `scriptmgr-go/internal/relay` | provider config、route selection、failover、proxy 只在此维护 | `forbidden` |
| Extension manifest loading | `scriptmgr-go/internal/extensions` | 插件清单扫描与贡献发现只在此维护 | `forbidden` |
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
- relay 代理与 provider 逻辑只放 `scriptmgr-go/internal/relay`
- manifest 扩展发现只放 `scriptmgr-go/internal/extensions`
- 临时迁移适配层必须在注释或文档中显式标记 cleanup 条件

## 7. Forbidden Patterns

- 在多个 package 中复制同一种 helper 或 service
- 把 domain 规则塞进名字泛化的 `utils` 文件夹
- 为了赶进度绕开 `src/lib/tauri` 直接在组件中写命令名
- 在 Rust 侧复制 Go 侧已有的脚本业务逻辑
- 在 `internal/http`、`internal/mcpcli` 或随机 `utils` 中复制 relay 选路逻辑
- 让 v1 插件系统直接执行第三方代码而没有边界和权限设计
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

- 共享 owner 或跨层改动先过 `docs/checklists/review-checklist.md`
- `bun run test`
- `bunx tsc --noEmit`
- `cargo check --manifest-path src-tauri/Cargo.toml`
- `go test ./...`
- PR review 检查是否出现新的 duplicate owner
- 修改边界或 owner 后同步 `MODULE.md` / `roadmap` / `ADR`
- 自动门禁：`node scripts/check-architecture.mjs`（import 环 / lib·features 反向依赖 / `lib/tauri` 外原始 invoke / 文件规模债，基线见 `scripts/architecture-baseline.json`）在 CI `architecture-gates` 与 Windows native `native-windows` 两个 PR 门禁中运行；schema-handler 配对由 `go test` 的 `TestStaticSchemasHaveRegisteredHandlers` 覆盖
- 原生门禁：`node scripts/check-native-architecture.mjs`（本文件第 10 节；`native/` 缺失时 skip 并返回 0），已接入 `run-smoke.ps1` 与 CI `architecture-gates`
- 原生门禁的自检：`node --test scripts/check-native-architecture.selfcheck.mjs`（23 项，在临时工作区里逐条打破规则）。**只跑门禁不跑自检，等于假设规则永远不会被改坏**——两者同属 `architecture-gates`
- 文档门禁：`node scripts/check-doc-budget.mjs`（单文件字节预算 40 KB 软 / 128 KB 硬、超软上限的文件在 `scripts/doc-budget.json` 里钉住只许缩、相对链接必须解析、`docs/roadmap.md` 不许出现带日期的进展条目且 `docs/roadmap/` 下每个分层文件都要被索引），自检 `node --test scripts/check-doc-budget.selfcheck.mjs`（11 项）；同在 `architecture-gates`。理由见 ADR-0001：路线图只留当前版本状态，历史按月下沉到 `docs/roadmap/progress-YYYY-MM.md`

## 10. Native Runtime Layer Map（enforced，ADR-0009）

> status: enforced。`native/` 已于 2026-11-25 落地（S1–S4 完成）。本节与 `check-native-architecture.mjs` 共同构成硬约束；规则本身由 `check-native-architecture.selfcheck.mjs` 用 23 项 fixture 与 8 次变异验证（见 [S4 文档](./plans/2026-11-25-native-architecture-gate-spike.md) §9）。

Native Runtime 采用单进程 Rust workspace，层级固定如下：

| crate | 职责 | 允许依赖 | 禁止 |
|---|---|---|---|
| `app` | 装配根：初始化、窗口注册、命令路由 | 全部内部 crate | 业务逻辑、SQL、Win32 细节 |
| `hub-ui` | 视图、组件、主题、交互状态 | `hub-editor`、`hub-win`、`hub-core` | `windows::*`、`rusqlite`、HTTP 客户端 |
| `hub-editor` | RichEdit 文本岛与 Markdown 装饰 | `hub-win`、`hub-core` | `egui`/`eframe` 类型、业务状态 |
| `hub-bridge` | scriptmgr worker 生命周期与协议 | `hub-core` | `egui`、`windows` UI 调用、SQL |
| `hub-win` | 窗口、DWM、托盘、热键、钩子、单实例、更新 | 无内部依赖 | `egui`、SQL、领域逻辑 |
| `hub-core` | 领域服务、SQLite、文件、解析 | 无内部依赖 | `egui`、`windows`、`tauri`、HTTP 客户端 |

硬规则：

1. 依赖单向，禁止环；新增 crate 必须先登记到 `check-native-architecture.mjs` 的 `ALLOWED_EDGES` **和 `BANNED_DEPS`**（后者决定该 crate 不许声明/引用哪些外部 crate；漏登记会被 `unknown crates` 拦下，不会静默通过）。
2. 平台能力只经 `hub-win` 暴露的 trait；UI 层出现 `use windows::` **或 `use windows_sys::`** 即违规——两者是同一家族，`windows` 与 `windows-sys` 都禁。
3. 持久化只在 `hub-core`；UI 不持有连接、不拼 SQL。
4. 网络只在 `hub-bridge`；worker 契约版本化 JSON。
5. UI 与 core 只通过 DTO / 命令交互，不共享可变全局状态。
6. `.rs` 单文件 ≤ 800 行。
7. 例外按第 8 节流程记录，且必须写 ADR。

替换关系：`src-tauri/src` 下的窗口 / 笔记 / 设置 / 脚本桥接 owner 在 Phase 1–3 逐步迁移到对应 crate；迁移完成前，两套 owner 不得同时写入同一数据（见 ADR-0009 第 6 条单写者）。
