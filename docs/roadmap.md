# EasyFlowHub Roadmap

## v1.0.0

### Source Of Truth

- 正式需求基线：[PRD-v1.0.0.md](./PRD-v1.0.0.md)
- 项目画像：[PROJECT_PROFILE.md](./PROJECT_PROFILE.md)
- 架构边界：[ARCHITECTURE_GUARDRAILS.md](./ARCHITECTURE_GUARDRAILS.md)
- 长期产品方向：[decisions/ADR-0003-product-direction-and-action-first.md](./decisions/ADR-0003-product-direction-and-action-first.md)
- 交付拆解计划：[plans/2026-04-24-v1-delivery-plan.md](./plans/2026-04-24-v1-delivery-plan.md)
- 任务看板：[plans/2026-04-24-v1-task-board.md](./plans/2026-04-24-v1-task-board.md)

### 产品方向

- **用户优先级**：以个人效率用户为首要对象，开发者、本仓库维护者和 AI 工作流用户作为高级集成用户。
- **平台边界**：保持 Windows-first，优先保证全局快捷键、托盘、多窗口、悬浮窗口和桌面交互的稳定体验。
- **产品分层**：
  - **Hub**：统一承载应用状态、动作库、数据和反馈。
  - **Launcher**：面向日常使用的快速动作入口，连接当前上下文与下一步操作。
  - **Manager**：面向整理、配置、审计和恢复的工作台。
- **能力分层**：
  - **Capture**：快速笔记、剪贴板、选中文本和文件捕获，进入可继续整理的 Inbox / 笔记。
  - **Act**：执行脚本、应用、URL、文本和文件动作，并把结果写回剪贴板、文件、笔记或通知。
  - **Flow**：组合多个动作，逐步支持参数传递、条件、重试、取消和输出路由。
- **版本策略**：v1.0.0 不重写现有桌面应用，先完成稳定发布闭环；v1.1.0 以统一 action registry 和 Action Launcher 为第一产品演进目标。

### Current Goal

- 先完成 quick note、notes manager、todo、scripts、manager/settings 的正式发布闭环。
- 把 Windows 桌面关键路径和打包产物行为收敛到可验证、可交接、可回归。
- 以受控方式纳入 relay / MCP / extensions，不让高级能力反向阻塞 v1 核心发布。
- 保持现有 v1.0.0 架构和用户主路径稳定，不为产品转向提前重写页面或运行时。

### Phase Plan

| Phase | Status | Goal | Key scope | Gate |
|---|---|---|---|---|
| Phase 1. Experience Stability | `[in_progress]` | 稳定最核心的桌面体验 | quick note、notes manager、todo、窗口关闭、自启动、托盘 | `CP1` |
| Phase 2. Automation Baseline | `[in_progress]` | 收口 scripts / tasks / settings 的可验证主路径 | 脚本发现、执行、记录、模块启停、关键设置持久化 | `v1 baseline` |
| Phase 3. Controlled Advanced Capabilities | `[in_progress]` | 让 relay / MCP / extensions 以受控范围进入 v1 | relay 回归、MCP catalog、extension contribution aggregation | `CP2` |
| Phase 4. Release Governance | `[in_progress]` | 完成发版前验证和文档同步 | smoke、checklist、CI、README / docs 对齐 | `CP3` |

### Active Tracks

#### Track A. Core Experience

- owner scope: `easyflowhub-app/src`, `easyflowhub-app/src-tauri/src`
- 重点：收尾 quick note、notes manager、todo、Windows 多窗口与关闭行为。
- 当前任务：
  - `V1-A1` `[in_progress]` quick note Markdown / 图片 / 历史兼容回归
  - `V1-A2` `[in_progress]` todo 聚合、回跳、端到端闭环回归
  - `V1-A3` `[in_progress]` autostart、托盘、关闭/隐藏行为在打包产物中复验

#### Track B. Automation Workspace

- owner scope: `easyflowhub-app/src/components/manager`, `easyflowhub-app/src/modules`, `scriptmgr-go/internal`
- 重点：让 scripts、tasks、settings 不只是“有入口”，而是“有标准验证主路径”。
- 当前任务：
  - `V1-B1` `[in_progress]` 固定标准脚本样例，验证列表、执行、记录/时间线
  - `V1-B2` `[in_progress]` manager 模块启停、快捷键、自启动设置持久化回归
  - `V1-B3` `[planned]` 汇总 scripts / manager 的发布前手工 smoke 结果

#### Track E. 产品方向基线

- owner scope: `easyflowhub-app/src/components/manager`, `easyflowhub-app/src/modules`, `scriptmgr-go/internal`, `docs`
- 重点：为 v1.1.0 的 Action Launcher 保留清晰边界，避免把现有 manager 模块直接误当成通用动作系统。
- 当前任务：
  - `V1-E1` `[in_progress]` 盘点现有脚本、笔记、待办、窗口和小组件能力可复用的动作入口；本轮先接入 ScriptMgr。
  - `V1-E2` `[done]` 设计并实现 action registry / contract：动作身份、输入、输出、执行状态、最近使用和收藏。
  - `V1-E3` `[done]` 明确并落实 Launcher 与 `src/modules` manager builtin registry 的 owner 边界。
  - `V1-E4` `[done]` 完成 Windows Launcher 窗口、`Ctrl+Alt+Space` 全局入口、脚本参数输入和执行反馈的最小闭环。

#### Track C. Controlled Integrations

- owner scope: `scriptmgr-go/internal/relay`, `scriptmgr-go/internal/extensions`, `scriptmgr-go/internal/mcpcli`, `easyflowhub-app/src/components/manager/{relay,mcp,extensions}`
- 重点：把 relay / MCP / extensions 保持在“受控增强”范围，不膨胀成平台级阻塞项。
- 当前任务：
  - `V1-C1` `[in_progress]` relay 真实上游回归：stream、`429`、`5xx`、timeout、鉴权差异
  - `V1-C2` `[in_progress]` provider 密钥从明文 JSON 继续收敛到环境变量或独立密钥层
  - `V1-C3` `[in_progress]` extensions contribution aggregation 的前端真实消费与审计视图收口
  - `V1-C4` `[planned]` 明确 extensions Phase 2+ 留到 `v1.1.0`，不作为 `v1.0.0` 阻塞项

#### Track D. Release Governance

- owner scope: `docs`, `.github/workflows`, `scripts`
- 重点：让 PRD、roadmap、README、checklist 与当前交付面一致。
- 当前任务：
  - `V1-D1` `[done]` Windows package smoke checklist 与 scripted smoke 已建立
  - `V1-D2` `[in_progress]` README / PRD / roadmap 同步到正式 v1 范围
  - `V1-D3` `[in_progress]` 每轮 checkpoint 后回填验证结果与剩余风险

#### Track F. Architecture Health

- owner scope: `easyflowhub-app/src`, `easyflowhub-app/src-tauri/src`, `scriptmgr-go/internal`, `.github/workflows`, `docs`
- 审查基线：[2026-10-19 架构与大文件审查](./plans/2026-10-19-architecture-and-file-size-review.md)。审查已完成；阶段A（F01–F05）与阶段B（F06–F14）已实施，阶段A（F01–F05）、阶段B（F06–F14）、阶段C（F11 + 第5节大文件拆分）与阶段D（V1-F5 CI 门禁）均已实施，四个阶段全部完成。
- 当前任务：
  - `V1-F1` `[done]` 盘点规模、依赖、owner 和异常路径；24 个实现文件物理行数超过 500，剔除 Rust 内联测试后约 23 个；TS 静态依赖未发现循环。
  - `V1-F2` `[done]` 优先修复审查 F01–F05：跨进程 sessions 旧快照覆盖、回收站失败误删、Inbox 非事务迁移、自动保存旧响应、todo 旧行号误写，并补故障/并发回归。
  - `V1-F3` `[done]` 收口 sidecar 恢复、模块失败回滚、配置提交、MCP 加载状态、附件生命周期等 owner 边界（F06–F10、F12–F14；F11 留到阶段C 大文件拆分）。
  - `V1-F4` `[done]` 在既有 owner 内按职责拆分 SettingsPanel、mcp/server、native inbox/lib 和编辑页面，保持接口兼容。2026-10-20：编辑页面完成——QuickNotePage → `useQuickNoteSession`，NotesPanel → `notes/NotesArchivePanel` + `useNotesArchive`，TodoPanel/TodoCardPage 共享 `useTodoGroups` + `useTodoEditSession`（写入仍归 useTodos/F05，保存版本仍归 useAutoSave/F04，解析仍归 lib/todoParser）；2026-10-20 完成剩余三项——SettingsPanel → `components/manager/settings/*`（分域 Section + hook，并修复 F08 UI 半边：普通保存不再夹带未提交 Launcher 草稿）；mcp/server.go 拆为 protocol/tools_*/notifications + 收敛 server.go，F11 `get_task_result` 等待改为真实可取消轮询；native inbox.rs → `inbox/{schema,attachments,repository,service,commands}`，lib.rs 收敛为装配根并抽出 launcher/app_windows/window_position。102 个 Tauri 命令与 MCP 工具表面保持兼容。
  - `V1-F5` `[done]` 建立持续门禁：`scripts/check-architecture.mjs` 拦截 import 环、lib/features 反向依赖、`lib/tauri` 外原始 invoke（按 `scripts/architecture-baseline.json` 基线只减不增）、实现文件规模债（>500 只减不增、>800 硬门禁）；schema-handler 配对由 Go 测试 `TestStaticSchemasAllHaveRegisteredHandlers` 覆盖。`test.yml` 新增 `architecture-gates` 与 `native-windows`（windows-latest cargo check + test）两个 PR 门禁。

### Release Checkpoints

#### CP1. Experience Stability Gate `[active]`

- quick note 在 `Esc`、`Alt+F4`、工具按钮关闭下都只关闭当前窗口。
- quick note 图片插入、删除、大图展示与旧笔记兼容完成回归。
- todo 从“笔记创建 -> 聚合展示 -> 勾选/编辑 -> 回跳”没有明显断裂。
- autostart、托盘、多窗口行为在 Windows 打包产物下验证通过。

#### CP2. Controlled Integration Gate `[active]`

- relay 至少完成一轮真实上游回归，覆盖 stream、`429`、`5xx`、timeout。
- manager Relay 面板在服务未启动、运行时未启动、非法 JSON 下有可理解反馈。
- provider API key 不继续依赖推荐的明文 JSON 配置方式。
- extensions / MCP 在 v1 范围内保持“只读聚合或受限导入”，不升级为完整平台承诺。

#### CP3. Release Governance Gate `[queued]`

- 至少一条标准验证链可被团队重复执行。
- roadmap、PRD、README、相关 `MODULE.md` 与当前代码 owner 不冲突。
- 打包 smoke 与默认 scripted smoke 的结果已回填。

### Validation Debt

- 架构审查阶段A（F01–F05）与阶段B（F06–F14）均已实施并有故障注入/并发回归覆盖；2026-10-20 又补齐待办箱跨窗口原子创建、自动保存导航竞态、Go 锁真实子进程回归和回收站列表的有效条目保留。真实 Windows 多进程/多窗口打包态 smoke（sidecar 杀进程恢复、MCP load/unload 与 tools/call 一致、录音永久删除命中原件、设置第 N 次写失败回滚、扩展目录无 relay 查询）仍未执行，不能把单测通过当作实机验收。2026-10-20 阶段C 后快照：前端 52 文件 / 381 项 + tsc、Go 全量 + vet（新增 F11 等待与 schema-handler 一致性回归）、Rust 88 项 + cargo check 通过；CI 门禁（V1-F5）已建立为 architecture-gates + native-windows 两个 PR 门禁。QuickNotePage(657)/TodoCardPage(532) 仍略超 500 行阈值，余量为内联 JSX，可作后续展示层拆分。
- quick note 的 `Esc` / `Alt+F4` / 工具按钮关闭回归仍需在打包产物中复验。
- 最新人工验收已暴露并修复一轮图片放大、撤销、manager 图片展示问题，但仍缺修复后的 Windows 二次复验。
- notes manager 仍缺一次发布前 CRUD / 回收 / 恢复 / 重开窗口一致性回归；Inbox 图片、录音回收与恢复已有自动化覆盖，打包态复验仍待完成。
- scripts 仍缺发版级标准样例脚本与记录链路复验。
- manager/settings 仍缺模块启停、快捷键、自启动的发布前人工验证。
- relay 仍缺真实上游场景回归与密钥层收口结论。
- Windows 产物目录当前同时出现 `DeskFlow_*` 与 `EasyFlowHub_*` 命名安装包，发版前需要确认正式交付物命名。

### Next Queue

1. 先关闭 `CP1`，因为它仍是最直接的发布阻塞项。
2. 固定 scripts / tasks / settings 的标准验证主路径，完成 `Phase 2` 收口。
3. 把 relay / MCP / extensions 的 v1 边界写死，避免在发版前继续膨胀范围。
4. 在每轮 checkpoint 后同步 README、PRD、roadmap 与 smoke 结果。
5. 在 v1.0.0 发布收口后，优先推进 Action Launcher 的 action contract 和最小全局入口，不先进入复杂 Flow 编辑器或扩展市场。

### Recent Progress

- `2026-10-20` 已完成阶段 D（V1-F5 持续门禁）：新增 `scripts/check-architecture.mjs`（拦截 import 环、lib/features 反向依赖、`lib/tauri` 外原始 invoke、实现文件规模债，配套 `scripts/architecture-baseline.json` 债务基线只减不增）；schema-handler 配对由 Go 测试覆盖；`test.yml` 新增 `architecture-gates` 与 `native-windows`（windows-latest cargo check + lib test）两个 PR 门禁，补上审查指出缺失的 Windows native PR 检查。至此架构审查四个阶段（A/B/C/D）全部实施。

- `2026-10-20` 已完成阶段 C（F11 + 第5节大文件拆分）：mcp/server.go 拆为 protocol/tools_*/notifications + 收敛 server.go，F11 `get_task_result` 等待改为真实可取消轮询（`taskWaiter`），schema/handler 同源、router 唯一 dispatch 并移除 legacy fallback；SettingsPanel 拆为 `components/manager/settings/*` 并修复 F08 UI 半边（普通保存不夹带未提交 Launcher 草稿）；native inbox.rs 拆为 `inbox/{schema,attachments,repository,service,commands}`，lib.rs 收敛为装配根并抽出 launcher/app_windows/window_position（102 命令不变）；编辑页面抽出 useQuickNoteSession/useNotesArchive/useTodoGroups/useTodoEditSession。验证：前端 52 文件 / 381 项 + tsc、Go `./...` + vet、Rust 88 项 + `cargo check` 通过。

- `2026-10-20` 已完成阶段 A 跟进收口：native notes 新增 `ensure_todo_inbox_note`，以 SQLite `BEGIN IMMEDIATE` 跨窗口原子复用/创建“待办箱”；前端 `useTodos` 改用 typed bridge，不再组合 `list -> create -> save`。Manager 自动保存导航增加请求序号与当前笔记会话校验，旧异步请求不能回填新笔记。Go store 增加真实 OS 子进程锁回归；回收站 HTTP 列表在存在损坏条目时仍返回有效条目并携带 warning；MCP LRU 使用序号消除同一时间戳下的随机淘汰，WebSocket 客户端计数改为加锁读取。全量前端 47 个测试文件 / 359 项、Rust 88 项、Go `./...`、TypeScript、`cargo check` 和 `go vet` 已通过；真实 Windows smoke 仍待人工执行。

- `2026-10-19` 已完成架构审查阶段B（F06–F14，单一状态 owner）：
  - F06 sidecar 恢复：native `scriptmgr_server.rs` 的 ready 门闩改为先探活再复用，sidecar 被杀/崩溃后下一次 ensure 自动重拉；TS `lib/api/scriptmgr` 在网络层失败时 `invalidateScriptmgr()` 丢弃缓存 URL 重新握手，HTTP 错误响应不失效缓存（避免误判）。
  - F07 模块回滚：`modules/registry.ts` 失败时恢复确切旧值而不是 delete（已持久化 true 的模块不再退回 defaultEnabled），回滚也通知订阅者让 UI 同步。
  - F08 设置原子性：native `update_settings` 抽出 `update_settings_with_conn`，8 个域改为单事务写入，中途失败整体回滚（trigger 故障注入测试覆盖）。
  - F09 MCP 加载状态单一 owner：HTTP 不再维护平行的 `loadedCategories` 布尔表，改为持有真实 `*mcp.ToolRouter` 并委托 Load/Unload/List，load 不存在的分类返回 404 而不是假装成功；消除了解锁后读 map 的竞态。
  - F10 录音资源身份：`inbox_attachments` 增加 additive `source_path` 列，voice 捕获记录原始 `audio/{id}.wav` 路径，永久删除未转笔记项时按记录的 source 清理原件（assets UUID 副本无法回指原件）。
  - F12 分类一致：`api.go` 新增 `effectiveCatalog()`，ListCategories/ListScriptsByCategory 与列表共用 override-aware 视图，设置 override 后各入口分类一致。
  - F13 扩展解耦：HTTP 新增 `ExtensionCatalog` 窄接口并可独立注入（`NewServerWithCatalog`），`/api/extensions`、contributions、`/api/mcp/servers` 不再依赖 relay 初始化。
  - F14 Voice 终态：新增 `transition_terminal` 单写路径，`saved`/`failed` 互不覆盖，pause/resume 迟到回执不再翻转已终态会话；测试改为直接驱动生产转移函数而非重写状态机。
  验证：前端 46 文件 / 353 项 + tsc、Go 全量 + vet（新增 F12/F09/F13 回归）、Rust 85 项 + cargo check（新增 F08/F10/F14 回归）通过。真实 Windows 打包态 smoke（sidecar 杀进程恢复、MCP load 与 tools/call 一致、录音原件清理）仍待人工执行。F11 留到阶段C 大文件拆分。
- `2026-10-19` 已完成架构审查阶段A（F01–F05，数据完整性）：F01 `internal/store` 新增跨进程 sessions 锁（O_EXCL 锁文件 + 过期锁回收 + 原子 rename 写），`executor/async.go` 的 Start/RunWorker/Cancel/refreshSessions 全部改为按任务 ID 的锁定单条读改写，`syncTask` 不再吞掉 tasks.db 错误；F02 回收站 manifest/metadata 写失败时回滚脚本到原路径，绝不删除唯一副本；F03 Inbox capture-kind 迁移包进单事务，失败回滚后重启可重试，foreign_keys pragma 失败后恢复；F04 `useAutoSave` 引入 token + 串行 promise 链，旧响应不再清新备份/回填新草稿，flush 等待真实 in-flight；F05 新增 `lib/todoParser.locateTodoLine`（按文本身份重定位、同名多条一律拒绝）、共享 `hooks/useTodos`（加载/保留期/勾选/编辑/新建唯一编排）、native `save_note` 增加 `expected_updated_at` compare-and-swap（opt-in，冲突返回 `note_conflict:`）。新增回归：Go store 并发/过期锁/单条更新 6 项、executor 对账 5 项、Rust CAS 4 项 + Inbox 迁移回滚 1 项、前端 locateTodoLine/useTodos/useAutoSave 19 项。验证：前端 46 文件 / 349 项、tsc、Go 全量 + vet、Rust 81 项 + cargo check 通过。真实 Windows 多进程/多窗口打包态 smoke 仍待人工执行。
- `2026-10-20` 已按架构审查第 5 节完成 Phase C 首个大文件拆分：`SettingsPanel.tsx`（1318 行）收敛为导航 / 分区组合 / 页面级反馈，各设置域拆到 `components/manager/settings/*Section.tsx` 与 `use<Domain>Settings` hooks（AppSettings 聚合、Launcher 触发、快捷键配置、自启动、开发者日志），对外导出与 `SettingsPanel.test.tsx` 保持兼容。同时关闭 F08 的 UI 侧残留：普通保存不再夹带未提交的 Launcher 草稿，只回写已提交的 `launcher_trigger` 镜像，Launcher 在自己入口提交成功后刷新镜像；新增 `useAppSettings` / `useShortcutsSettings` hook 回归与面板级 F08 回归测试固定该行为。验证：`bunx tsc --noEmit`（本目录无错误）、SettingsPanel + settings hooks 定向 vitest 19 项通过、Manager 主题 token 审计覆盖新增 Section 文件。
- `2026-10-19` 已完成 Phase 5 第二条切片"录音后语音转文字"（ADR-0008）：native 新增 `transcription.rs` owner，按 `TranscriptionTransport` trait 通过本机 Relay `POST /v1/audio/transcriptions`（multipart）转写，25 MB 大小守卫、180 秒截止、固定中文错误分类；请求在 native 侧执行，音频字节不进入 WebView，密钥与路由仍归 Go Relay。`inbox_attachments.kind` 以 additive schema 增加 `transcript`，transcript 作为文本附件落盘并在重新转写时同事务替换，`convert_inbox_item_to_note` 对 transcript 附件把文本嵌入笔记正文。前端新增 typed bridge、共享 `TranscribeControl`（文件 / 大小 / 时长 / Provider / 模型 + 隐私确认后才发送）、Voice 面板与 Inbox 面板挂载、设置页「转写」段（模型 / 语言）。本轮验证：前端 41 个测试文件 / 299 项、Rust 65 项 library tests、TypeScript、Vite 生产构建、Manager 主题 token 审计、Windows target `cargo check`、Go tests、`git diff --check` 通过（Rust 仅有既有 `NoteInput` 未使用字段 warning）。Windows 打包态麦克风 smoke、真实 Provider 成功转写与重新打开 Manager 后 transcript 恢复仍待人工验收；实时转写、说话人识别、本地模型适配器与自动转写不在本切片。
- `2026-10-16` 已完成 Action Launcher 最小闭环骨架：独立 Action Registry、脚本动作适配、搜索/最近/收藏、参数输入、执行反馈，以及 Windows `Ctrl+Alt+Space` 全局入口；前端、Rust、Go 验证通过，后续继续补充非脚本动作和真实 Windows 打包验收。
- `2026-10-16` 已补充 Launcher 触发策略：可选择键盘快捷键、鼠标中键或两者并用；快捷键可重新录入，Windows 前台进程支持排除列表，配置由设置页桥接到 native 层并在保存时重新注册。

- `2026-04-25` 已新增 `scriptmgr relay hosts status|switch`，用于安全切换 Claude / Codex 宿主配置到指定 AI 网关或本地 relay 上游；实现放在 `scriptmgr-go/internal/hostconfig`，明确与 `internal/relay` 的协议与 owner 边界分离，避免把 Claude 协议误塞进现有 OpenAI relay。
- `2026-04-24` 已基于上述修复重新完成一轮 Tauri Windows 打包，新的 `EasyFlowHub.exe`、NSIS、MSI 产物已生成，并再次通过 `scripts/check-release-artifacts.ps1` 预检；当前进入“修复后待人工复验”状态。
- `2026-04-24` 已根据人工验收反馈修复一轮图片与编辑器回归：`useHistory` 历史入栈缺陷已修复并补 `useHistory.test.tsx`；quick note 图片缩略图与 manager markdown 预览已支持双击放大；`NotesPanel` / `NoteEditor` 已改为分离正文与附件图片，避免 manager 打开含图笔记时展示原始 `asset:` markdown。
- `2026-04-24` 已继续修正 manager 预览排版细节：图片不再默认整块居中，改为与正文绕排，解决图文混排阅读断裂问题。
- `2026-04-24` 已继续修正 manager 编辑器撤销链路：正文编辑接入显式历史栈，`useShortcutEngine` 开始处理 `Ctrl+Z / Ctrl+Shift+Z`，避免删除或移动图片 markdown 时撤销失效。
- `2026-04-24` 已把人工验收发现与修复状态回填到 `docs/checklists/windows-package-smoke-2026-04-24.md`，当前仍待用户基于修复后的产物完成第二轮 Windows smoke。
- `2026-04-24` 已执行 Windows 产物预检并通过：`check-release-artifacts.ps1` 确认 NSIS / MSI / portable 产物存在，同时新增 `docs/checklists/windows-package-smoke-2026-04-24.md` 记录本轮 package smoke 预检结果与待完成人工项。
- `2026-04-24` 已把 manager 侧 todo / trash 主路径也补进自动化：新增 `TodoPanel.test.tsx` 覆盖勾选、来源跳转与悬浮卡片入口，新增 `TrashPanel.test.tsx` 覆盖单条恢复、批量恢复与清空回收站。
- `2026-04-24` 已为 todo / notes manager 补最小页面级回归：新增 `TodoCardPage.test.tsx` 覆盖待办勾选保存、打开原笔记跳转和待办箱创建；新增 `NotesPanel.test.tsx` 覆盖 `MANAGER_OPEN_NOTE_EVENT` 打开指定笔记与关闭编辑器时 `flushSave` 草稿。
- `2026-04-24` 已为 quick note 补最小自动化回归：新增 `QuickNotePage.test.tsx` 覆盖 `Escape` 关闭与 `destroy()` 失败回退 `close_window`，新增 `useEditorImageInsertion.test.tsx` 覆盖图片粘贴保存链，并扩展 `imageAssets.test.ts` 覆盖 legacy asset URL 提取与 quick note 图片移除相关约束。
- `2026-04-24` 已新增 [plans/2026-04-24-v1-task-board.md](./plans/2026-04-24-v1-task-board.md)，把 `P1-A ~ P4-D` 进一步拆成可领取任务卡、依赖关系和完成定义。
- `2026-04-24` 已新增 [plans/2026-04-24-v1-delivery-plan.md](./plans/2026-04-24-v1-delivery-plan.md)，把 `v1.0.0` 拆成 4 个交付阶段、任务包、owner 边界和验收口径。
- `2026-04-24` 新增正式 [PRD-v1.0.0.md](./PRD-v1.0.0.md)，并将 roadmap 收敛为执行状态与阶段推进视图。
- `2026-04-14` 已继续推进 manager 模块化：`SettingsPanel` 改为消费共享 `useToggleableModules()` hook，并补 `SettingsPanel.test.tsx` 固定模块渲染和开关行为。
- `2026-04-14` 已为 `manager_modules` 单一宿主入口补齐前端自动化回归，包括 `ManagerExtensionEntries.test.tsx`、`ExtensionsPanel.test.tsx`、`ManagerPage.test.tsx`。
- `2026-04-11` 已新增 builtin `Extensions` 面板，并把 extension `manager_modules` 收口为只读 entry host，不再直接注入 `src/modules` registry。
- `2026-04-11` 已推进 extension contribution aggregation 主链路：后端暴露 `/api/extensions/contributions`，并新增 effective catalog / overlay 相关消费基础。
- `2026-04-07` 已完成 Windows package smoke、scripted smoke、CI toolchain 对齐，以及 docs / checklist 的第一轮治理收口。

## v0.2.0

> 说明：当前应用包版本为 `0.2.0`（2026-10-21 发布），本节定义本轮用户体验版本 `0.2.0` 的范围与门禁。既有 `v1.0.0` / `v1.1.0` 文档保留为发布治理和长期平台路线，不与本节的近期功能优先级冲突。

### Source Of Truth

- 需求基线：[PRD-v0.2.0.md](./PRD-v0.2.0.md)
- 实施计划：[plans/2026-10-16-v0.2.0-product-delivery-plan.md](./plans/2026-10-16-v0.2.0-product-delivery-plan.md)
- 阶段 checkpoint：[plans/2026-10-18-v0.2.0-checkpoint-summary.md](./plans/2026-10-18-v0.2.0-checkpoint-summary.md)
- 视觉与入口决策：[decisions/ADR-0004-glass-theme-and-efficiency-workspace.md](./decisions/ADR-0004-glass-theme-and-efficiency-workspace.md)

### 产品目标

围绕个人用户的日常工作上下文，形成 `Capture -> Understand -> Retrieve -> Act` 闭环：

- **P0 捕获与理解**：通过 Launcher 调用 Snow Shot 截屏、Inbox 快速笔记、剪贴板 / 选中文本翻译、统一 Launcher 上下文。
- **P1 检索与资源管理**：指定目录文件搜索、轻量预览、基础文件操作、URL 下载队列。
- **P2 音频输入（延期至 v0.2.x）**：录音与语音转文字已有实现，但不纳入 v0.2.0 核心 RC；待 Windows 麦克风和真实 Provider 验收后再发布。
- **视觉基线**：冷色明亮毛玻璃工作空间，统一 Manager、Launcher、Quick Note 和浮层的 token；保留关闭模糊后的不透明 fallback。

### Phase Plan

| Phase | Status | Goal | Key scope | Gate |
|---|---|---|---|---|
| Phase 0. Foundation | `[in_progress]` | 统一动作、上下文、附件和主题 token | action context、Inbox / attachment、入口保护、design tokens | `F0` |
| Phase 1. Capture | `[in_progress]` | 让截图和临时内容快速进入 Inbox | Snow Shot 会话入口、剪贴板 / 文本 / 链接 / 图片 / 文件捕获、归档与笔记整理 | `F1` |
| Phase 2. Understand | `[in_progress]` | 让文本内容可即时理解 | 选中文本 / 剪贴板翻译、Provider adapter、结果回流 | `F2` |
| Phase 3. Retrieve | `[in_progress]` | 快速找到并处理本地资源 | 指定目录索引、文件搜索、预览和基础操作 | `F3` |
| Phase 4. Download | `[in_progress]` | 收口资源获取链路 | 下载队列、暂停 / 继续 / 重试、完成后动作 | `F4` |
| Phase 5. Voice | `[deferred: v0.2.x]` | 将声音纳入 Inbox | Windows 录音、音频附件、录音后语音转文字（Relay Provider + 隐私确认） | `F5` |
| Phase 6. Glass Theme | `[in_progress]` | 迁移主要页面并验证可读性 | Manager / Launcher / Quick Note / Settings token 化、明暗主题切换 | `F6` |

### Active Tracks

- **翻译首条切片（2026-10-17～2026-10-18）**：已接入复用本机 Relay 的剪贴板翻译，目标语言 / 模型设置、复制译文、原文与译文写入 Inbox；Windows 快捷键 / 中键入口会在 Launcher 聚焦前读取前台选区，结果面板提供显式替换原文选区，并在原生侧恢复来源窗口与文本剪贴板。真实 Provider、不同编辑器 / 剪贴板格式和 Windows 打包验收仍未完成，F2 保持进行中。
- **本轮验证（截至 2026-10-18）**：前端 40 个测试文件 / 288 项、Rust 51 项通过；TypeScript、Vite 生产构建、主题 token 审计、Windows target `cargo check`、Rust library tests、Go tests、Windows Tauri 打包和 release artifact preflight 通过。打包 sidecar 已与当前 Go 构建哈希一致，NSIS / MSI 均含 sidecar，portable 程序启动保持运行 5 秒；浏览器模拟 Relay / 原生边界验证明暗主题、复制 / 收集箱回流、长译文滚动、窗口尺寸和高对比度显示。这些验证不替代真实 Provider、Windows 剪贴板 / SQLite 或完整打包态人工 smoke。
- **Phase 0 模糊关闭回退（2026-10-18）**：`data-blur="off"` 除切换不透明表面外，也会关闭 Manager 与 Launcher 的 `backdrop-filter`；主题审计现验证两者均停用滤镜。Windows 打包态低性能 / 关闭透明效果和高对比度验收仍待人工记录。
- **Relay / 管理 API 安全边界（2026-10-18）**：ScriptMgr 管理 API 仅接受 loopback IP，显式通配或远程网卡地址会被拒绝；Relay 默认 loopback。浏览器来源采用白名单；配置快照、扩展清单和有效贡献中的 API key、已知认证 header 与含凭据的 `base_url` 已脱敏，未改占位值保存会保留原值。header 脱敏仍按名称启发式处理；本地 `relay.json` 仍可能明文保存兼容密钥，同机进程也可直接访问 loopback API。细节见 [ADR-0007](./decisions/ADR-0007-relay-local-api-security.md)。
- **Windows sidecar 打包修复（2026-10-18）**：发现 Go sidecar 新鲜度只比较 `cmd/scriptmgr/main.go`，且 Tauri 包未声明 sidecar 资源；发布工作流另有 Ubuntu 生成 `.exe` 的平台错误。现在 Release 构建监视 Go 源码与模块文件并强制执行 `go build`，失败即中止；Tauri 将 sidecar 映射到应用资源根目录，NSIS / MSI payload 哈希已对照新构建验证。发布工作流补上 Go toolchain、Windows amd64 sidecar 构建和哈希守卫。应用包版本已提升到 `0.2.0`，v0.2.0 的 Windows 人工 smoke 未完成。
- **发布结果（2026-10-21）**：`0.2.0` 已按上述门禁完成本地发布检查并打 tag `v0.2.0`，由 GitHub Actions Release 工作流产出 Windows portable / NSIS / MSI、Go sidecar 与 `latest.json`。自动化门禁、本地 Windows 打包预检、sidecar 哈希对照和浏览器 / 原生窗口 smoke 通过；上一行列出的 Windows 打包态人工验收项（Snow Shot 会话、真实 Relay Provider、Files 回收站 / 权限、Downloads 取消 / 重试、主题高对比度、安装后 sidecar 启动）仍作为 follow-up 待回填 checklist，不阻塞本次发布但需在 `v0.2.x` 收口。P2 Voice 的麦克风实机验收同样延后，Phase 5 保持进行中。
- **Track V0-A. Launcher and Context**：继续复用 action registry，统一键盘 / 中键入口、排除软件、输入上下文和输出路由；新增 `external.snow-shot.capture` 外部动作。
- **Track V0-B. Capture and Inbox**：Snow Shot 提供可见截图会话；用户复制截图后粘贴进 Inbox。剪贴板文本、图片、文件和链接统一落入本地 Inbox，再由 Manager 整理。
- **Phase 1 Launcher 捕获回流（2026-10-18）**：Launcher 的“收集剪贴板内容”动作现会在执行时优先读取系统剪贴板图片（PNG/JPEG/WebP/GIF，32 MB 上限），将 Snow Shot 复制的截图直接归档到 Inbox；没有可读图片时继续回退到唤起时读取的文本快照。图片为空或超限会阻止写入并给出可恢复错误，避免把截图误报成文本或产生半成品。
- **Track V0-C. Retrieve and Download**：先支持指定目录和最近文件，再实现下载队列；不以全盘实时索引作为首个门槛。
- **Phase 4 设计（2026-10-17）**：下载器归 Tauri native，采用 `.part` 临时文件、HTTP Range 续传、状态轮询和明确冲突策略；详细边界见 [ADR-0005](./decisions/ADR-0005-download-owner-and-safety.md)。
- **Phase 4 下载首条切片（2026-10-17～2026-10-18）**：已接入 Manager Downloads builtin、URL 队列、目标目录与冲突策略、`.part` 临时文件 / Range 续传、暂停 / 继续 / 取消 / 重试、完成后打开 / 定位 / 复制路径和加入 Inbox；覆盖策略需明确确认，队列重启后不自动恢复。前端下载相关测试 6 项、Rust 下载测试 11 项通过，新增本地 HTTP 流写入 `.part`、取消保留 `.part` 和 `fail / rename / overwrite` 冲突策略覆盖；Reqwest 已启用默认 Rustls crypto provider，避免 native worker 因 provider 缺失 panic。URL 路径和 RFC 5987 `Content-Disposition` 文件名会先做 UTF-8 percent-decoding，再经过 Windows 文件名校验；仍待 Windows 打包态公开文件、取消、失败重试、冲突和权限 smoke。
- **文件检索首条切片（2026-10-17）**：已接入 Manager Files builtin、单目录显式刷新索引、文件名 / 路径 / 扩展名 / 模糊与最近访问排序、文本 / 小图预览、打开 / 定位 / 复制路径 / 重命名 / 移动 / 复制 / 回收站删除。root / paused 设置持久化，索引操作带 generation 校验，冲突与越界错误回显。F3 仍待 Windows 打包态回收站、权限 / 锁定 / 长路径人工 smoke；多目录、分页和实时索引不在本切片。
- **文件检索错误恢复（2026-10-17）**：复制路径在 native 校验通过后由 Files 面板写入系统剪贴板；原生打开成功后最近文件记录采用 best-effort，不会把记录失败误报成打开失败；文件操作后的索引重扫失败会清空旧索引并递增 generation，Files 面板会重新读取状态和结果。F3 的 Windows 打包态 smoke 仍未完成。
- **路径与 Launcher 剪贴板 native 闭环（2026-10-18）**：文件与下载的“复制路径”统一通过 `copy_text_to_clipboard` 写入 Windows 系统剪贴板；Launcher 的上下文读取和翻译/脚本结果复制也优先经过 native bridge，浏览器预览才回退到 WebView Clipboard API。空文本、读取失败、剪贴板占用和非 Windows 不可用均返回独立错误，Files / Downloads 面板保留严格 native 错误反馈。新增 read/write typed bridge 与 Rust/UI 回归测试；仍需在 Windows 打包态验证其他进程占用剪贴板时的提示。
- **文件检索验证（2026-10-17）**：前端全量 33 个测试文件 / 253 项通过，`cargo check` 通过，`file_search` 原生聚焦测试 9 项通过，Vite 构建通过；构建仍有既有的大 chunk 警告，Rust 仍有既有 `NoteInput` 未使用字段 warning。浏览器开发页可验证布局与 Launcher 焦点，但 Tauri IPC 在浏览器环境不可用，不能替代 Windows 打包态 smoke。
- **Track V0-D. Voice and Providers**：先保存音频原件，再接入录音后转写；Provider 通过适配层隔离。
- **Phase 5 采集实现（2026-10-18）**：Voice native 已切换为 Windows 专用采集线程：由 `cpal` 打开默认 WASAPI 输入设备，`hound` 写入 `48 kHz / mono / 16-bit PCM WAV`，以 `.wav.part` 写入并在停止时完成 WAV、原子提升为 `.wav`，随后复用 Inbox 音频归档流程。Tauri 命令只发送 pause / resume / stop 控制消息，不跨线程保存 `cpal::Stream`；录音失败保留可诊断的 `.part` 路径并进入 `failed`，只有 Inbox 提交成功才进入 `saved`。Voice 面板已提供暂停、继续、停止、状态轮询、时长 / 设备信息、打开录音和跳转收集箱；当前仍需 Windows 打包态麦克风权限 / 设备 / 隐私设置 smoke，录音后转写仍不属于本切片。
- **Phase 5 验证快照（2026-10-18，当时）**：该阶段实现时前端 38 个测试文件 / 279 项、Rust 51 项、TypeScript、Vite 生产构建和 Windows target `cargo check` 通过；后续全量基线更新为 39 个文件 / 285 项。仍需真实 Windows 麦克风权限、设备占用、WAV 播放、`.part` 保留和 Inbox 回流 smoke；Voice 是 P2，不阻塞 P0/P1 核心 RC。
- **Track V0-E. Glass Workspace**：先建 token 和 fallback，再按页面迁移，保持内容可读和状态可见。
- **Phase 6 主题残留收口（2026-10-18）**：Settings、MCP、Tasks、Notes 编辑器与状态栏的表面、状态徽章、错误提示和控件已改用 `manager-*` token；开关滑块仍保留高对比前景。深色主题不再依赖这些页面的 `bg-white` / `bg-gray` / `amber` 硬编码，行为测试与类型检查通过。
- **Phase 6 Manager 余下页面收口（2026-10-18）**：Beautification、Component Preview、Trash 和共享 SettingsSectionCard 的页面表面、筛选按钮、空态、恢复/删除状态与加载态也已改用 `manager-*` token；Component Preview 内用于展示浅色/深色组件本身的 mini preview 仍保留独立示意色，不参与 Manager 主题表面。

### Non-goals

- 社区插件市场、任意代码插件执行、云同步和多端协作。
- 一开始就完成全盘实时索引或替代 Everything 的完整文件能力。
- 复杂 Flow 编辑器、实时转写、说话人识别和专业音频编辑。

### v0.2.0 Release Gate

- Windows 打包产物中，中键 / 键盘入口、快捷键重录和排除软件可验证。
- Snow Shot 外部动作在 Windows 打包产物中覆盖已安装后进入截图界面、D 盘注册安装发现、MCP 未启用指引、未安装打开官方安装页、自定义路径优先和启动失败恢复；Inbox 捕获和翻译至少各有一条自动化或脚本化回归路径。
- P1 的文件搜索与下载覆盖取消、失败、冲突和恢复反馈。
- P2 如进入 `v0.2.x` 发布候选，转写失败不丢音频，结果可回写笔记；P2 不阻塞 `v0.2.0` 的 P0/P1 核心发布。
- 主要页面已迁移到统一 token，关闭模糊或系统高对比度下仍可用。
- NSIS / MSI 安装后的 ScriptMgr sidecar 可启动，且打包文件与本地 Windows amd64 构建哈希一致。
- 真正的 Relay Provider 和 Windows 打包态文本选区 / 剪贴板链路完成至少一轮人工验证。

### Snow Shot 外部动作边界

- EasyFlowHub 不捆绑 Snow Shot，不复制其代码；Launcher 通过随 Snow Shot 安装的官方 MCP bridge 启动应用并调用可见截图界面。
- 检测顺序为“用户自定义路径 -> Windows 已安装应用注册信息 -> 常见安装目录 -> PATH”，可发现 D 盘等非系统盘安装；未安装才打开 <https://snowshot.top/download.html?os=windows>。
- Snow Shot MCP integration 默认关闭，需用户在「设置 → 系统」显式开启；未开启时给出设置指引。已安装但 bridge 缺失 / 不兼容时提示更新，不作为未安装处理。
- Snow Shot 的更新、许可证、MCP 开关、截图保存目录、标注行为和用户快捷键由 Snow Shot 管理；EasyFlowHub 不模拟其用户快捷键、不注册 Snow Shot 快捷键，也不假定截图文件路径。
- MCP 仅用于打开可见截图会话；自动截图结果回流和 OCR / 翻译联动另行评估。

### Recent Progress

- `2026-10-21` 发布 `v0.2.0`：版本号在 `package.json`、`tauri.conf.json`、`Cargo.toml` / `Cargo.lock` 同步为 `0.2.0`，新增 [发布说明](./releases/v0.2.0.md)。发布前检查修复了架构门禁的规模债阻塞：Quick Note 窗口的快捷键映射抽为 `hooks/useQuickNoteShortcuts`、切换器 / 工具栏 / 工具面板 / 图片条抽为 `components/shared` 组件、`toggle_always_on_top`、`hide_window`、`close_window` 收入新增 `lib/tauri/window.ts`（`QuickNotePage` 658 → 423 行，`lib/tauri` 外原始 `invoke()` 由 4 处降到 3 处）；回收站面板拆为 `trash/TrashNotesView`、`trash/TrashInboxView`、`trash/TrashScriptsView` 与共享展示件（530 → 209 行）。基线经 `node scripts/check-architecture.mjs --update-baseline` 重新快照，门禁恢复通过。验证：`run-smoke.ps1` 全部通过（前端 54 文件 / 395 项、TypeScript、Vite 构建、主题 token 审计、`cargo check`、Rust 91 项、Go 全量测试）、架构门禁通过、Windows NSIS / MSI 打包与 release artifact 预检通过。

- `2026-10-21` Launcher 在翻译模型未配置或设置读取失败时新增结构化“去设置”入口：点击后隐藏 Launcher、打开 Manager，并自动定位到“设置 → 翻译”；普通 Relay / 网络错误不会显示该按钮。新增 manager typed bridge、跨窗口导航事件和 Launcher / Manager / SettingsPanel 回归测试；聚焦 70 项、全量前端 394 项通过，TypeScript 与 Vite 构建通过。

- `2026-10-21` 完成本地重新安装、前端全量测试、TypeScript、Vite 构建、Rust 检查以及 Windows NSIS/MSI 打包；安装包已包含重新构建的 `scriptmgr.exe` sidecar。按 Manager、Quick Note、Todo Card、Folder Widget、Launcher 和各 Manager 面板完成浏览器截图与交互 smoke。浏览器预览中 Files / Downloads 原先会直接显示 Tauri IPC 的 `invoke` 异常，已统一转换为可理解的“桌面服务暂不可用”提示，同时保留真实权限、网络和冲突错误；新增 2 个面板回归测试。Windows 原生窗口已启动并确认 Manager 可加载真实笔记，因窗口位于当前截图捕获范围外，独立窗口的像素截图仍需在可见桌面位置复验。

- `2026-10-20` 按发布范围决定将 Voice / 转写延期到 `v0.2.x`，不作为 `v0.2.0` P0/P1 核心 RC 门槛；Launcher 改为窄长四列紧凑面板（`440×860`），快捷键设置补充第三方全局热键可能同时响应的说明。当前本机保存的 Launcher 组合为 `Ctrl+Alt+Space`。
- `2026-10-20` Notes 工作区调整为默认打开“笔记”，并将“笔记”标签置于“收集箱”之前；语音入口仍可直接打开收集箱。
- `2026-10-20` 收集箱删除改为进入统一回收站，Inbox 以 additive `deleted_at` 软删除，新增收集箱回收站列表、恢复、永久删除与清空；恢复保留原归档状态和附件，只有永久清理才移除未转换笔记的图片 / 音频副本及录音原件，已转换笔记仍引用的资源予以保留。回收站按设置保留期清理过期收集项。验证：前端全量 52 个测试文件 / 385 项、TypeScript；Rust 全量 91 项、Inbox 聚焦 20 项、`cargo check` 通过。`cargo fmt --check` 因当前 toolchain 未安装 `rustfmt` 未能运行。
- `2026-10-19` 脚本从「只读目录」升级为可管理工作台：Go 侧新增 roots 固定、源码读写、新建、导入、删除到回收站与回收站恢复/清空共 12 个 HTTP 端点（写入限定在已配置 root、白名单扩展名、2 MB 上限；跨盘移动自动退化为复制 + 删除）；前端新增脚本工作台（源码编辑、手动执行、退出码与输出）、新建 / 导入 / 存储位置对话框。执行记录并入脚本页作为「运行记录」tab 并移除独立侧栏模块，轮询改为窗口隐藏不轮询、有运行中任务 3 秒、空闲 15 秒。回收站统一管理笔记与脚本。设置新增开发者模式：native 有界日志环形缓冲（500 行）+ `logs/app.log`（2 MB 轮转）+ sidecar stderr 捕获，设置页可查看/清空/打开日志目录。验证：前端 44 文件 / 325 项、`tsc`、Rust 76 项、Go 全量测试通过。
- `2026-10-19` 修复管理中心「脚本 / 扩展 / 中转 / MCP」四个面板同时 `Failed to fetch`：根因是 `scriptmgr serve` 从未被任何一方启动——前端硬编码 `127.0.0.1:8765`、`setBaseUrl` 没有调用方，Rust 侧只有 CLI 桥接。新增 native owner `scriptmgr_server.rs`（按需 spawn、TCP + `GET /api/scripts` 健康等待、并发去重、应用退出回收子进程），前端 `ensureScriptmgr` 在每次 HTTP 调用前确保 sidecar 就绪；健康检查由 HEAD 改为 GET，避免 Go method-specific 路由返回 405 把在线服务误判为离线。同时补齐 Inbox 录音 CRUD（当时为永久删除 + 二次确认；2026-10-20 改为统一回收站）与「打开资源文件夹」入口；设置模块改为侧栏底部固定项并换用 `⚙` 图标；删除未被引用的重复 `components/ScriptsPage.tsx`。验证：前端 41 文件 / 309 项、`tsc`、Rust 73 项、`cargo check` 通过。
- `2026-10-19` 修复扩展页白屏（连带侧边栏消失）：服务恢复后扩展面板第一次拿到真实数据，`/api/extensions` 在零 manifest 时返回 `extensions: null`（Go 把 nil slice 序列化为 null），面板直接 `.filter` 崩溃并卸载整棵 React 树。修复分两层：`lib/api/scriptmgr` 作为 wire-format 适配层统一把 null 列表归一化为 `[]`（extensions / contributions 各数组 / scripts / tasks / mcp servers / relay snapshot），面板侧再补防御性默认值；同时新增 `PanelErrorBoundary` 包裹动态面板，单个面板异常只显示内联错误与「重试」，侧边栏与其他页面保持可用。验证：前端 42 文件 / 315 项、`tsc` 通过。
- `2026-10-17` 已完成 Inbox 第一条纵向切片：新增 `notes.db` 中独立 Inbox / attachment 表、受限图片与本地文件捕获、来源 / 上下文 / 时间元数据、归档恢复、链接和附件打开，以及事务性幂等的“整理为笔记”；Notes 面板内以“收集箱 / 笔记”切换呈现，不增加侧栏入口。Launcher 新增一键收集剪贴板内容，并在每次唤起时刷新剪贴板内容。审查后补充图片读取与 Base64 解码前的大小限制、标准 HTTP(S) URL 校验，以及归档页可见的主题化反馈。最终前端 30 个文件 / 186 项测试、生产构建、TypeScript、Rust `cargo check`、Inbox Rust 6 项聚焦单测与 diff 检查通过；原生开发版收集箱页面已确认可加载。Windows 打包态的文件选择、文件拖放、剪贴板图片与 Snow Shot 复制粘贴 smoke 尚未完成；普通文件转笔记目前保留附件名称和已归档 Inbox 原件，笔记内直接打开附件仍待补齐。
- `2026-10-17` Launcher 交互第二轮：面板改为紧凑的图标工具栏和方块网格，搜索支持直接输入、IME 组合与点击图标展开；窗口每次唤起都会激活并聚焦，原生焦点变化也会把输入焦点送回搜索框；点击透明窗口空白处不再误关闭，关闭按钮、Esc 和重复触发仍可收起。原生窗口收紧到 `564×320`，玻璃面板铺满窗口主体，只留 4px 圆角透明边，移除大面积无功能画布。鼠标中心定位按显示器 DPI 计算，支持多屏负坐标与工作区边界；默认非置顶、拖动不因失焦关闭。Native Windows focus、原生拖动和关闭仍需打包后实机确认。
- `2026-10-16` 已完成 Manager 第二轮页面收口：侧栏固定为 224px 并独立滚动，导航标题支持截断，主区补齐最小尺寸约束；Relay / Scripts 页面迁移到冷色玻璃 token，降低深色浅色残留和卡片嵌套；Launcher 标题栏空白区支持原生拖动，交互按钮保持可点击。浏览器预览中的 Tauri IPC 错误也已转换为可理解的服务状态提示。
- `2026-10-16` 已继续清理待办、扩展目录和脚本详情的主题残留：状态提示、错误横幅、空态、筛选和输入控件统一使用 manager token；脚本详情抽屉改为响应式宽度，避免窄窗口挤压列表；浏览器预览在 Tauri IPC 不可用时显示可理解的服务提示。
- `2026-10-16` 本轮验证通过：Vitest 27 个文件 / 152 项、TypeScript、Vite 构建、Rust `cargo check`、`git diff --check`；构建仅保留大 chunk 提示，Rust 仅有已有未使用字段 warning。待 Windows 原生窗口再次确认拖动和全宽 Manager 侧栏。
- `2026-10-16` 已完成第二轮视觉与交互收口：Manager 笔记区移除多层卡片嵌套并统一冷色 token；Launcher 支持“软件 / 工具”和“脚本 / 动作”分组、分割线、跟随鼠标定位、拖曳标题栏和置顶切换；快捷键替换增加失败回滚，鼠标 Hook 安装失败可重试；设置模型补齐 `launcher_trigger`，避免保存普通设置时覆盖 Launcher 配置。
- `2026-10-17` 已将 Snow Shot 外部动作升级为截图会话调用：通过相邻官方 MCP bridge 启动应用、检查 MCP 可用状态并调用 `snow_shot_screenshot_begin`；增加 Windows 卸载注册信息检测以支持 D 盘安装，并保留用户自定义路径优先。MCP 默认关闭，需用户在 Snow Shot「设置 → 系统」手动开启。
- `2026-10-16` 已完成 Snow Shot 外部动作的首版骨架：Launcher 注册 `external.snow-shot.capture`，通过 Tauri 检测自定义路径、常见目录和 PATH；已安装时启动，未安装时打开官方 Windows 安装页；设置页支持保存可选的自定义可执行文件路径。
- `2026-10-17` 已为 Snow Shot 官方 MCP 截图调用补充工具能力、MCP 未启用、服务错误、D 盘注册路径解析和返回值测试；Windows 原生截图会话仍待用户开启 MCP 后实机 smoke。
- `2026-10-16` 已补齐 Snow Shot 检测优先级、无效路径、启动失败和 camelCase IPC 返回值测试；完整前端测试 27 个文件 / 148 项通过，TypeScript 与 Rust 检查通过。
- `2026-10-16` 已完成第一轮毛玻璃主题迁移：全局冷色玻璃 tokens、深色 / 浅色 / 系统主题切换、Manager / Launcher / Quick Note / 美化设置页面适配，并为不支持模糊的环境保留不透明 fallback；待 Windows 打包产物进行最终可读性与高对比度复验。
- `2026-10-16` 已完成一轮 Windows 主题实机验收：原生 Tauri Manager 能加载真实笔记，深色 / 浅色主题可切换，Launcher 搜索与收藏筛选可用；期间发现并修复暗色 Manager 的 `bg-white/*` 半透明残留。Quick Note / Todo 子窗口与 Snow Shot 原生截图会话仍需下一轮独立窗口 smoke，Snow Shot 需预先开启 MCP integration；记录见 [windows-theme-smoke-2026-10-16.md](./checklists/windows-theme-smoke-2026-10-16.md)。

## Candidate Next Version

### v1.1.0

- **首要产品目标：Action Launcher**：提供全局唤起、动作搜索、最近使用、收藏、参数输入和执行结果反馈。
- 先定义统一 action registry / contract，再把现有脚本包装为 `script` 动作，并逐步接入应用、文件、URL、文本和 AI 动作。
- Launcher 的输入应逐步支持当前剪贴板、选中文本、拖入文件和显式参数；输出应支持剪贴板、文件、笔记和通知。
- v1.1.0 不要求一次完成复杂 Flow 编辑器、社区市场、云同步或任意代码插件执行。
- AI 宿主切换与 relay 的正式需求基线：[PRD-v1.1.0-ai-host-routing.md](./PRD-v1.1.0-ai-host-routing.md)
- AI 宿主切换实施计划：[plans/2026-04-25-ai-host-routing-implementation-plan.md](./plans/2026-04-25-ai-host-routing-implementation-plan.md)
- extensions `Phase 2-4`：启停状态持久化、热重载、安装 / 卸载。
- relay 运行时与负载均衡能力继续收口，明确正式支持的 route selection、weighted round robin 与最小 failover 边界。
- relay 密钥管理与配置体验进一步收口，减少 raw JSON 依赖。
- Claude / Codex 宿主配置切换继续收口，补齐 dry-run / diff / preset，而不是把协议不兼容的宿主硬塞进单一 relay。
- MCP 外部 server 生命周期管理增强。
- 在 Action Launcher 稳定后，再评估更强搜索、Flow 编排和扩展生态能力。

## Maintenance Rules

- PRD 负责版本需求边界，roadmap 只保留 active 执行内容。
- 深设计细节放入 `docs/plans` 或 `docs/decisions`，不要继续堆进 roadmap。
- 完成的历史条目及时压缩，只保留仍影响执行的近期进展。
- 每轮 checkpoint 结束后，回填验证结果、剩余风险和 next queue。
