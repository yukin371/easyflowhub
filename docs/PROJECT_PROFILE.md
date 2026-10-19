# EasyFlowHub Project Profile

> status: active
> owner: EasyFlowHub maintainers
> last_verified: 2026-10-18
> verified_against: 2026-10-18 working tree (v0.2 checkpoint)

## 1. Summary

- Project type: `hybrid desktop app + embedded script runtime`
- Product positioning: `Windows-first 的个人桌面效率中枢，以动作优先的 Launcher 连接捕获、执行与流程`
- Primary goal: `让个人用户在当前工作上下文中快速记录、找到并执行下一步动作，减少切换应用和重复输入`
- Primary users: `个人效率用户优先；开发者和本仓库维护者作为高级集成用户`
- Product layers: `Hub + Launcher + Manager`
- Capability layers: `Capture（捕获） + Act（执行） + Flow（编排）`
- Current phase: `0.1.1 包线上的 v0.2.0 功能实现与自动化验证；Windows 打包态人工 smoke 待回填；v1.x 保留长期发布治理线`

### 产品方向

EasyFlowHub 的产品主入口不是一组彼此独立的笔记、待办和脚本页面，而是一个低打扰的桌面工作流中枢。当前近期期待通过 v0.2.0 验证个人效率 Hub，围绕 `Capture -> Understand -> Retrieve -> Act` 形成闭环：

- **Hub**：承载应用状态、数据、动作库和统一反馈，让笔记、待办、脚本及后续扩展共享同一套上下文。
- **Launcher**：面向日常使用的快速入口，通过全局快捷键、搜索、最近使用和收藏动作，帮助用户在当前上下文中完成下一步操作。
- **Manager**：面向整理、配置、审计和恢复的工作台，管理笔记、待办、脚本、设置以及受控的 AI 集成。

三层能力按用户任务组织，而不是按技术模块组织：

- **Capture**：从 quick note、剪贴板、选中文本和文件等入口快速收集信息，进入可继续整理的 Inbox / 笔记。
- **Act**：执行脚本、打开应用或链接、处理文本和文件，并把结果写回剪贴板、文件、笔记或通知。
- **Flow**：将多个动作串成可复用流程，逐步加入参数传递、条件、重试、取消和输出路由。

- **v0.2.0**：在当前 `0.1.1` 包版本线上推进截屏、翻译、Inbox 捕获、文件检索、资源下载、录音和录音后转写，并以 Windows-first 的毛玻璃工作空间统一体验。
- v1.0.0 文档仍作为既有发布治理基线，负责 quick note、notes manager、todo、scripts、settings 和 Windows 行为的稳定闭环；v1.1.0 作为较长期的 Action Launcher 与高级集成路线保留。

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
- Integration / E2E command: `pwsh -File .\scripts\run-smoke.ps1`
- Release artifact smoke: `pwsh -File .\scripts\run-smoke.ps1 -CheckReleaseArtifacts`
- Smoke test path: `bun tauri dev -> 打开 manager -> 新建 quick note -> 打开 todo card -> 切换 settings / scripts / notes / relay 面板`

### 2026-10-18 v0.2 checkpoint

- 前端：40 个测试文件 / 288 项通过
- Rust：51 项 library tests、Windows target `cargo check` 通过
- TypeScript、Vite production build、Manager theme token audit、Go tests 通过
- Windows portable、NSIS、MSI 产物重新生成并通过 artifact preflight；portable 启动预检保持运行 5 秒通过
- 真实 Windows 交互、Snow Shot、Relay、麦克风和高对比度视觉 smoke 仍按 [windows-package-smoke.md](./checklists/windows-package-smoke.md) 待人工回填

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
| `scriptmgr-go/internal` | backend services | 脚本发现、执行、HTTP、MCP、API relay、扩展清单、notes sync、runtime 管理 |
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

### 产品架构形态

- `Hub` 是产品层的共享状态和反馈中枢，不等同于新增一个业务模块。
- `Launcher` 是面向用户的动作入口，后续应消费统一 action registry；它不替代 `src/modules` 的 manager builtin module registry。
- `Manager` 是整理和配置入口，继续承载现有模块面板，不应成为日常操作的唯一入口。
- `Capture`、`Act`、`Flow` 是产品能力分层。v1.0.0 主要交付 Capture 和 Act 的稳定基础，Flow 保持为后续演进方向。
- v1.1.0 的 action registry / contract 应在现有脚本能力之上扩展到应用、文件、URL、文本和 AI 动作；脚本运行时仍由 `scriptmgr-go` 负责。

## 7. Canonical Shared Capability Owners

| Concern | Canonical owner | Notes |
|---|---|---|
| Manager workspace shell | `easyflowhub-app/src/components/manager` | 管理中心布局、面板切换、管理窗口事件 |
| Frontend module registry | `easyflowhub-app/src/modules` | 功能模块注册、启用/禁用、侧边栏挂载 |
| Frontend native command wrappers | `easyflowhub-app/src/lib/tauri` | 前端不应重复散落 `invoke()` 命令名 |
| Notes SQLite persistence | `easyflowhub-app/src-tauri/src/notes.rs` | 笔记 CRUD、回收站、图片落盘 |
| App settings persistence | `easyflowhub-app/src-tauri/src/settings.rs` | 应用级设置、窗口尺寸、编辑器配置 |
| System clipboard | `easyflowhub-app/src-tauri/src/clipboard.rs`, `easyflowhub-app/src/lib/tauri/clipboard.ts` | Windows 文本剪贴板读写；路径动作严格 native，Launcher 上下文/结果 native-first |
| Window / appearance / widgets | `easyflowhub-app/src-tauri/src/lib.rs`, `appearance.rs`, `widget.rs` | 原生窗口、托盘、全局快捷键、桌面小组件 |
| Script discovery / execution / tasks | `scriptmgr-go/internal/api`, `discovery`, `executor`, `runtime`, `store` | 脚本业务逻辑归 Go 侧，不应在 Rust/TS 侧重复实现 |
| MCP server protocol | `scriptmgr-go/internal/mcp` | JSON-RPC、tool schema、dynamic tool routing、notifications |
| External MCP client wrappers | `scriptmgr-go/internal/mcpcli` | 连接外部 MCP 服务器，不与服务端实现混用 |
| API relay / provider routing | `scriptmgr-go/internal/relay` | OpenAI 兼容代理、加权路由、failover、provider health |
| Extension manifest registry | `scriptmgr-go/internal/extensions` | 扩展清单扫描与贡献发现，v1 不执行任意插件代码 |
| Notes file sync repo | `scriptmgr-go/internal/notes` | 文件仓库同步，与桌面内 SQLite CRUD 分离 |

产品层边界：

- `easyflowhub-app/src/modules` 继续只拥有 manager builtin module 注册和启停，不承担未来的通用 action registry。
- Action registry、触发器、输入输出和流程编排在实现前必须先形成独立的长期 owner 决策；不得把 Launcher 逻辑散落到各个现有面板。
- `scriptmgr-go/internal` 继续拥有脚本发现、参数校验、执行和任务记录；Launcher 通过稳定接口消费这些能力，不复制脚本业务逻辑。

## 8. Non-Negotiable Constraints

- 不要在 feature / component 中直接散写 Tauri `invoke()` 命令名；优先经由 `src/lib/tauri`。
- 不要在 `src-tauri/src` 中重复实现 `scriptmgr-go` 已拥有的脚本发现、执行或 MCP 路由逻辑。
- 不要把跨模块共享能力直接塞进随机 `utils` 文件；必须先确认 canonical owner。
- 新原生命令若被前端调用，必须同时补齐 Rust 注册与前端 typed wrapper。
- API relay 的 provider 选择、失败切换和健康状态必须集中在 `scriptmgr-go/internal/relay`。
- ScriptMgr / Relay 本地 HTTP 默认只监听 loopback；CORS 来源策略归 `scriptmgr-go/internal/localapi`，API 配置快照不得回显密钥。
- v1 扩展系统只接受 manifest 贡献，不在 `internal/extensions` 中执行任意代码插件。
- `docs/roadmap.md` 只保留 active 内容，长期边界决策进入 `docs/decisions`。

## 9. Known Risks / Recurring Pitfalls

- 仓库历史中存在过期设计文档，最明显的是旧版 `easyflowhub-app/src/ARCHITECTURE.md`，容易误导 AI。
- TS / Rust / Go 三层并存，命令名、类型形状和行为约束很容易不同步。
- 多窗口、快捷键、autostart、桌面固定等行为强依赖 Windows，回归成本高。
- `go test ./...` 本地耗时明显长于前端测试，容易被忽略但必须保留。
- 负载均衡 relay 目前只实现 OpenAI 兼容 HTTP 路径、基础权重和 failover，尚未覆盖更复杂的 provider 认证差异与流量配额策略。
- manager 中的 Relay 面板当前直接编辑 JSON，结构校验主要依赖后端 `relay.SaveConfig`，尚未收敛为表单式配置体验。
- `api_key` 仍兼容并可能明文保存在本地 `relay.json`；CORS 和 loopback 限制不构成本机进程认证，优先通过 `api_key_env` 提供密钥。参见 [ADR-0007](./decisions/ADR-0007-relay-local-api-security.md)。
- Launcher 已形成动作搜索、最近使用、收藏、参数输入、执行反馈和 Windows 全局入口的最小闭环；非脚本动作接入、真实打包验收和 v0.2.0 上下文扩展仍需继续推进。
- v0.2.0 的主要新能力已完成首条实现和自动化验证；真实 Windows 打包态、Snow Shot/Relay Provider、麦克风与高对比度视觉仍需按 checkpoint 清单回填。转写 Provider 仍属于后续 slice。

## 10. Unknowns

- `TBD`: 统一 logging owner 仍未成形，当前 TS / Rust / Go 侧日志策略分散。  
  Confirm via: `src`, `src-tauri/src`, `scriptmgr-go/internal` 全仓搜索日志入口并形成 ADR。
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

## 12. 产品方向检查清单

- [ ] 新功能是否能明确归入 Capture、Act 或 Flow
- [ ] 面向个人用户的日常动作是否可以从 Launcher 进入，而不要求先打开 Manager
- [ ] 新增动作是否复用统一输入、输出、权限和执行记录约定
- [ ] 是否保持 Windows-first 的全局快捷键、托盘和桌面窗口体验
- [ ] 高级 relay / MCP / extensions 能力是否仍作为受控增强，而不是普通用户的首屏入口
- [ ] v0.2.0 新能力是否能通过 Launcher 复用统一输入、输出、附件和失败反馈
- [ ] 毛玻璃主题是否保留足够不透明度、焦点可见性和无模糊 fallback
