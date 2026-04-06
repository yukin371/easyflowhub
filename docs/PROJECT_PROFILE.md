# EasyFlowHub Project Profile

> status: active
> owner: EasyFlowHub maintainers
> last_verified: 2026-04-07
> verified_against: e524f8522ecf0a15e40af3e6f7627a34e319e81d

## 1. Summary

- Project type: `hybrid desktop app + embedded script runtime`
- Primary goal: `在一个 Windows 桌面壳内统一快速笔记、待办管理、脚本执行与 MCP 集成`
- Primary users: `个人效率用户 + 本仓库维护者`
- Current phase: `v1`

## 2. Stack

- Primary languages: `TypeScript, Rust, Go`
- Frameworks / runtimes: `React 19, Vite 7, Tauri 2, Bun, Vitest, Go 1.25.1`
- Storage: `SQLite（笔记与应用状态） + JSON / 文件系统（scriptmgr 元数据与脚本）`
- Infrastructure / deployment: `Windows-first desktop app + GitHub Actions release pipeline`

## 3. Verification

- Install command: `cd easyflowhub-app && bun install`
- Dev command: `cd easyflowhub-app && bun tauri dev`
- Build command: `cd easyflowhub-app && bunx tauri build`
- Type check command: `cd easyflowhub-app && bunx tsc --noEmit`
- Frontend test command: `cd easyflowhub-app && bun run test`
- Native check command: `cd easyflowhub-app && cargo check --manifest-path src-tauri/Cargo.toml`
- Go backend test command: `cd scriptmgr-go && go test ./...`
- Integration / E2E command: `TBD`
- Smoke test path: `bun tauri dev -> 打开 manager -> 新建 quick note -> 打开 todo card -> 切换 settings / scripts / notes 面板`

### 2026-04-07 本地验证结果

- `bun run test`: passed, 7 files / 84 tests
- `bunx tsc --noEmit`: passed
- `cargo check --manifest-path src-tauri/Cargo.toml`: passed, 仅存在 `src-tauri/src/notes.rs` dead_code warning
- `go test ./...`: passed, 本地耗时较长（约 114s）

## 4. Repo Topology

| Path | Role | Notes |
|---|---|---|
| `easyflowhub-app/src` | frontend app | React UI、模块注册、解析器、IPC wrapper、前端测试 |
| `easyflowhub-app/src-tauri/src` | native desktop shell | Tauri 命令、窗口生命周期、SQLite 持久化、sidecar 集成 |
| `scriptmgr-go/internal` | backend services | 脚本发现、执行、HTTP、MCP、notes sync、runtime 管理 |
| `docs` | project governance | roadmap、画像、架构边界、计划、决策、模板 |
| `scripts` | repo automation | 仓库维护与辅助脚本 |
| `image` | assets / docs images | README 与设计说明用截图资源 |

## 5. Runtime Entry Points

| Entry point | Type | Notes |
|---|---|---|
| `easyflowhub-app/src/main.tsx` | desktop frontend | React 挂载入口 |
| `easyflowhub-app/src-tauri/src/main.rs` | desktop runtime | Tauri 桌面入口，委托到 `lib.rs::run()` |
| `easyflowhub-app/src-tauri/src/lib.rs` | native shell | 注册命令、插件、窗口与快捷键 |
| `scriptmgr-go/cmd/scriptmgr/main.go` | CLI / service | `scriptmgr` 入口，承接 CLI / HTTP / MCP 模式 |
| `.github/workflows/test.yml` | CI | 前端测试、类型检查、Go 测试、Go vet、Go build |
| `.github/workflows/release.yml` | CI release | Tauri 打包、scriptmgr 构建、release 发布 |

## 6. Architecture Shape

- Architecture style: `modular desktop application with typed bridge + sidecar CLI`
- Core layers: `React components/pages -> hooks/modules/lib -> lib/tauri -> src-tauri native commands -> scriptmgr-go/internal -> SQLite / JSON / filesystem`
- Module boundary strategy: `前端按 feature + shared layer 混合组织，原生与 Go 侧按职责模块化`
- Shared capability strategy: `前端命令调用集中在 src/lib/tauri；桌面原生能力集中在 src-tauri/src；脚本协议与 MCP 集中在 scriptmgr-go/internal`

## 7. Canonical Shared Capability Owners

| Concern | Canonical owner | Notes |
|---|---|---|
| Manager workspace shell | `easyflowhub-app/src/components/manager` | 管理中心布局、面板切换、管理窗口事件 |
| Frontend module registry | `easyflowhub-app/src/modules` | 功能模块注册、启用/禁用、侧边栏挂载 |
| Frontend native command wrappers | `easyflowhub-app/src/lib/tauri` | 前端不应重复散落 `invoke()` 命令名 |
| Notes SQLite persistence | `easyflowhub-app/src-tauri/src/notes.rs` | 笔记 CRUD、回收站、图片落盘 |
| App settings persistence | `easyflowhub-app/src-tauri/src/settings.rs` | 应用级设置、窗口尺寸、编辑器配置 |
| Window / appearance / widgets | `easyflowhub-app/src-tauri/src/lib.rs`, `appearance.rs`, `widget.rs` | 原生窗口、托盘、全局快捷键、桌面小组件 |
| Script discovery / execution / tasks | `scriptmgr-go/internal/api`, `discovery`, `executor`, `runtime`, `store` | 脚本业务逻辑归 Go 侧，不应在 Rust/TS 侧重复实现 |
| MCP server protocol | `scriptmgr-go/internal/mcp` | JSON-RPC、tool schema、dynamic tool routing、notifications |
| External MCP client wrappers | `scriptmgr-go/internal/mcpcli` | 连接外部 MCP 服务器，不与服务端实现混用 |
| Notes file sync repo | `scriptmgr-go/internal/notes` | 文件仓库同步，与桌面内 SQLite CRUD 分离 |

## 8. Non-Negotiable Constraints

- 不要在 feature / component 中直接散写 Tauri `invoke()` 命令名；优先经由 `src/lib/tauri`。
- 不要在 `src-tauri/src` 中重复实现 `scriptmgr-go` 已拥有的脚本发现、执行或 MCP 路由逻辑。
- 不要把跨模块共享能力直接塞进随机 `utils` 文件；必须先确认 canonical owner。
- 新原生命令若被前端调用，必须同时补齐 Rust 注册与前端 typed wrapper。
- `docs/roadmap.md` 只保留 active 内容，长期边界决策进入 `docs/decisions`。

## 9. Known Risks / Recurring Pitfalls

- 仓库历史中存在过期设计文档，最明显的是旧版 `easyflowhub-app/src/ARCHITECTURE.md`，容易误导 AI。
- TS / Rust / Go 三层并存，命令名、类型形状和行为约束很容易不同步。
- 多窗口、快捷键、autostart、桌面固定等行为强依赖 Windows，回归成本高。
- `go test ./...` 本地耗时明显长于前端测试，容易被忽略但必须保留。
- GitHub Actions 当前仍固定 `setup-go: 1.21`，而 `scriptmgr-go/go.mod` 声明 `go 1.25.1`，需要显式对齐以避免 CI toolchain 漂移。

## 10. Unknowns

- `TBD`: 统一 logging owner 仍未成形，当前 TS / Rust / Go 侧日志策略分散。  
  Confirm via: `src`, `src-tauri/src`, `scriptmgr-go/internal` 全仓搜索日志入口并形成 ADR。
- `TBD`: 生产打包产物的完整人工 smoke checklist 尚未沉淀。  
  Confirm via: 新建 `docs/plans` 或 `docs/decisions` 中的 release / smoke 文档。
- `TBD`: E2E / Playwright 验收流尚未进入仓库默认验证链。  
  Confirm via: 新增浏览器或桌面验收脚本并写入 CI / runbook。

## 11. AI Initialization Checklist

- [ ] Read root `AGENTS.md`
- [ ] Read `docs/README.md`
- [ ] Read `docs/roadmap.md`
- [ ] Read `docs/ARCHITECTURE_GUARDRAILS.md`
- [ ] Read nearest `MODULE.md`
- [ ] Confirm whether the change touches TS, Rust, Go or multiple layers
- [ ] Prefer existing owners over new helpers / adapters
- [ ] Mark unknowns as `TBD`, do not fabricate
