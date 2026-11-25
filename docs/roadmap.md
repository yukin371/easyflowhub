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

- `2026-11-25` **S4 通过：架构门禁从"能通过"变成"能失败"，并补上三个真实缺口**。设计与验收标准先落在 [plans/2026-11-25-native-architecture-gate-spike.md](./plans/2026-11-25-native-architecture-gate-spike.md)，再按 A/B/C 三批实现。**G0 的四项 spike（S1–S4）全部完成，待 G0 评审。**
  - **绿不能证明规则有效**：从 S1 起 CI 每次都是绿的，但那时门禁的每一条规则都可以是空实现。S4 的实质是让它失败一次——23 项自检在 `os.tmpdir()` 的临时工作区里逐条打破规则（越界边含 `[dependencies.foo]` 表形式、两节点环、层符号泄漏、801 行、未登记 crate、两条 skip 分支），断言精确到条目文本而不是只看 `failed`。
  - **自检本身也做了变异**：8 次故意改坏规则，8 次都被杀。其中 2 次第一轮**存活**，暴露的是自检的缺陷而不是规则的缺陷——尺寸用例拿被测的 `HARD_CAP` 自己算自己（改到 900 时 fixture 也跟着变 901 行，测试照样绿）；表形式依赖走的是与 kv 形式**不同的正则**，只测 kv 就等于没测。两处都补了用例，`HARD_CAP === 800` 现在被字面量钉死。
  - **三个真实缺口，都是写测试时撞出来的**：①`hub-core` 里 `use windows_sys::…` 不报错——规则写的是 `windows::`，而本项目用的是 `windows-sys`，下划线让 `\b` 匹配不到；②manifest 声明 `egui` 但源码不 `use` 完全不报——而"声明了"距离"用了"只差一行；③`[target.'cfg(windows)'.dependencies]` 整段解析不到，**放在 cfg 后面就等于隐身**。禁表因此合成**一张** `BANNED_DEPS`，符号正则与 manifest 检查都从它派生，避免两侧漂移。
  - **重构也要求证，不是声称**：把判断抽成 `analyzeNativeArchitecture(nativeRoot)` 时，成功路径与失败路径（种一个 `use egui::Context;`）都拿 HEAD 里的旧脚本对同一棵树跑 diff，输出**逐字节相同**。抽取过程中我一度把 `app` 的 `/tauri/` 收窄成 `/\btauri::/`，那是悄悄放宽，已改回。
  - **未测**：注释内 `crate::` 的误报（**有意保留**：剥注释的剥离器出错会漏报，漏报比误报危险）、CI runner 上的实际耗时、规则集完整性（未穷举 `cfg_attr` / `include!`）、真实越界 PR 的拦截演练。

- `2026-11-25` **S3 通过：原生进程直读直写 `notes.db`，39 项门禁检查全绿（0 项口径放宽）**。设计与验收标准先落在 [plans/2026-11-25-notes-db-compat-spike.md](./plans/2026-11-25-notes-db-compat-spike.md)，再按 A/B/C 三批实现。（当时 S4 未做。）
  - **双向兼容用真代码验，不自证**：native 写入后，读它的是 **Tauri 那份代码**（`get_note_helper` / `list_notes_with_conn` / `save_note_with_conn`，新抽出的连接级函数），不是复制一遍 SQL；另加**独立实现**——`sqlite3` CLI（3.50.4，另一套 build，原生侧 bundled 是 3.46.0）与我们的读者对 **42 行逐字段一致**（`rows-identical 42`）。反向也验了：Tauri 写 → native 读回同一内容。
  - **"无 schema 破坏"是可 diff 的**：`sqlite_master` 全量 dump 写前写后**逐字节相同**；`journal_mode=delete` / `user_version=0` / `page_size=4096` / `encoding=UTF-8` 四项不动；`integrity_check` 在 native 写后、Tauri 写后、独立 CLI 三次全 `ok`。**刻意不切 WAL**：那会改文件头 write version 并留下 `-wal`/`-shm`，属产品级决定（备份/同步/"复制 db"的语义都变），不该由 spike 替用户定。
  - **真实数据不是造的**：live 库 `143,360 B`，**40 条笔记 / 9 个设置 / 待办箱**全部读出，字段与 CLI 逐项相等；排序（置顶优先 + `updated_at` 降序）与产品查询口径一致。
  - **并发是实测的**：另一进程持 `IMMEDIATE` 写锁 400 ms 时本进程写入被 `busy_timeout` 吸收并成功；持锁 7 s（超 5 s 超时）时本进程**明确失败**（`database is locked`）、无半行、`integrity_check` 仍 `ok`、持锁方的行还在。
  - **G-1**：live 库字节数 / sha256 / mtime 三项写前写后完全相同——全程只碰 `native/target/s3-gate-<stamp>/root/notes.db` 副本；`--store-bench --store-write` 还会**拒绝**指向真实 app data 目录（代码约束，不是操作纪律）。
  - **4 个真实发现**：①`autostart` 是 `{"enabled":true}` **对象不是布尔**（设计时按 bool 写，实测纠正）；②`notes` 表物理列序因 `ALTER TABLE` 追加而不同于建表文本，`INSERT ... VALUES` 会把 `tags` 写进 `created_at` 槽——静默损坏，全部语句显式列名；③PS 5.1 `Start-Process` 在环境变量大小写重复（`HTTPS_PROXY`/`https_proxy`）时直接抛异常，并发测试改用 .NET Process API；④PS 5.1 把原生 stderr 变成 `ErrorRecord`，`-is [string]` 过滤会吞掉 `database is locked`，门禁因此差点把"被拒绝"判成"失败"。
  - **未测（4 项，明确列出）**：22 389 个既有附件的批量可读性（只验契约与新写入文件）、待办 parser 完整口径（只看 `- [ ]`/`- [x]` 行数）、随机并发压力、Win10 双系统。
  - **工具化**：`--store-bench [--store-write] [--store-hold <ms>]` + `scripts/measure-native-store.ps1`（L1 native / L2 Tauri 真代码 / L3 独立 CLI / 并发 / G-1，一条命令 `RESULT: PASS`）；`cargo test --workspace` 43 → **58 项**；样本归档 `docs/releases/native-s3/`（两次独立运行逐项相同）。

- `2026-11-25` **S2 通过：文本岛键入 p95 4.95 ms（阈值 16，余量 11）**。设计与验收标准先落在 [plans/2026-11-25-richedit-island-spike.md](./plans/2026-11-25-richedit-island-spike.md)，再按 A/B/C 三批实现并逐条验收。（当时 S3 / S4 未做。）
  - **工作区**：新增 `hub-editor` crate（第五个），`check-native-architecture.mjs` → `passed (5 crates)`；`cargo test --workspace` 27 → **43 项**。CI 的 `native-crates` job 自动覆盖新 crate。
  - **承载方式**：`RICHEDIT50W`（`Msftedit.dll`）作为 egui 面板的**子 HWND**，矩形由布局给出、按物理像素对齐；父窗口补 `WS_CLIPCHILDREN`，这是"GL 不盖子控件"的唯一保证。`hub-editor` 只吃 `PixelRect`、只吐文本，门禁禁止出现 `egui::`。
  - **实测**（n=200 @1k / n=100 @10k，150% 单屏，三次运行）：`type` p50 **2.9** / p95 **4.95** / p99 **6.33** / max 6.8 ms；10k 文档 p95 **5.09 ms**，比值 **0.95–1.07**（**曲线是平的，不是线性上涨**——插入成本与文档长度无关，这是 S2 最想确认的一件事）。10k 加载 8–11 ms、粘贴 13–14 ms、连发 200 字符 203–225 ms。`compose` 仅报告 p50 4.9–7.5 ms。
  - **"深色背景真的生效"变成了像素计数**：`WM_PRINTCLIENT` 到内存 DC 采样 19392 点——主色 `0x2D2521` 占 98.49%，**空文档文字色像素 0、写入 20 字后 135**。这条路子不需要截屏、不需要可见窗口，所以能在无人值守机器上跑，也不会被"桌面上盖了什么"骗过去。
  - **4 个真实发现（都是量出来的，不是猜的）**：①`EM_SCROLLCARET` **在控件没有焦点时是空操作**——首轮两条滚动探针都返回 0，不是控件坏了；补 `SetFocus` 同时修掉产品里"输入超过一屏后光标不跟随"；②RichEdit 在 `WM_CHAR` **内部**同步绘制，紧随其后的 `UpdateWindow` 实测 0 µs，所以"插入 vs 绘制"在自然路径上无法分离，门禁改为 `insert` + 强制全量重绘（上界）；③连续键入被 RichEdit 合并成**一个**撤销单元，`EM_STOPGROUPTYPING` 无效，只有移动光标才切开；④空闲桌面下 `DwmFlush` 返回 **254 ms**（合成器不合成），所以采样器一旦探到 > 40 ms 就整段标 `n/a`，不把空闲时间记成延迟。
  - **口径作废而不是"看起来更好"**：首轮测出的 p95 **1.58 ms** 连同 `paint 0.00` / `scroll 0/0` **全部作废**——那是不带焦点、合成器休眠的状态，产品里不会发生。带焦点后 `insert` p50 从 0.74 → 1.9 ms（约 2.6×），省掉的正是光标维护与滚动视图这些真实工作。作废原因逐条写进 S2 文档 §9.4，归档里不留一个更快的旧数字。
  - **B5 判据修订（显式记录，未悄悄放宽）**："50 次编辑 = 50 次撤销"不成立也不该成立；判据改为"撤销到栈空后逐字节回到原文 + 重做回到终稿 + **撤销栈确有深度**（光标移动后 2 个单元，替换型编辑 2–3 个）"。门禁查的是"能把内容还原"，不是"每一步一个单元"。
  - **未测（2 项，都在 §9.2 标明）**：①三种输入法各 30 分钟人机验收（需要人，只自动验了 `ImmGetContext` 非空、`WM_IME_*` 不被截获）；②真实 Ctrl+Z 组合键（桩不出 `GetKeyState(VK_CONTROL)`，只验到非截获键仍到达控件）。
  - **工具化**：`--editor-bench N` + `scripts/measure-native-editor.ps1`（预算 + **17 项结构证据**一起判：颜色是否上屏、光标是否滚动、矩形是否只在变化时移动、Esc/Tab 是否被截获、撤销是否有深度……一条命令 `RESULT: PASS` / 退出码 0），样本归档到 `docs/releases/native-s2/`。

- `2026-11-25` **S1 通过：原生壳热唤起 p95 48.9 ms（阈值 60，余量 11.1）**。设计与验收标准先落在 [plans/2026-11-25-native-shell-spike.md](./plans/2026-11-25-native-shell-spike.md)，再按 A/B/C 三批实现并逐条验收。**G0 仍未过**：S2 / S3 / S4 未做。
  - **工作区**：`native/` 四个 crate（`hub-core` 纯几何 + 设置、`hub-win` Win32 薄封装、`hub-ui` egui 面板、`app` 装配）。`cargo test` 27 项（core 12 / win 5 / app 10）；`check-native-architecture.mjs` 从 `skip` 变成真实 `passed (4 crates)`，这条门禁到今天才算真的存在。egui / eframe **钉在 0.31**——spike 要可复现，升级即作废全部归档样本。
  - **两段式显示（本轮唯一的结构性发现）**：跨线程 `SetWindowPos` 是**同步**的，要等目标线程处理消息，所以"把搬窗放到热键线程"并不能省下唤醒成本，实测仍是 30–47 ms。改为 **phase 1 = 测量 + 夹紧 + 搬窗 + 聚焦（热键线程，计入门禁）**、**phase 2 = 刷新内容（UI 线程，p50 140 ms，只记录不门禁）**。门禁量的是前者（屏幕上是预热帧），后者作为"内容新鲜度"另行记录——这是本轮唯一放宽的口径，写在 §4.4 里。
  - **预热到稳态**：启动即建窗，预绘 3 帧（着色器编译 / 图集上传 / 稳态帧），唤起只剩搬窗。窗口以 `SWP_NOSIZE` 停在 `(-30000, -30000)` 且 `with_taskbar(false)`，**不能用 `SW_HIDE`**——隐藏会停发 `WM_PAINT`，winit 没有 redraw 可驱动，bench 第 2 轮起永久停住。
  - **实测**（n=200，3840×2160@150% 单屏）：shown p50 **47.8** / p95 **48.9** / p99 **49.3** / max 49.4 ms（Tauri 对照 87 / 101 / 130，n=31）。另测 `--scale` 1.0 与 2.0（各 n=20）p95 49.3 / 49.2，说明成本与缩放无关、基本是唤醒的常数项。锚定实测 `at=+2631+786 size=660x1290`：右侧余 549、底边正好贴住 12 的边距；取焦 0.1–0.8 ms。
  - **泄漏**：Private **130.0 → 122.2 MB（−7.8）**，20 / 50 / 200 轮句柄数同水平（+24…+28，一次性）。**基线必须取在预热 3 帧之后**：取在构造函数里会把一次性的着色器 / 图集 / 纹理算成泄漏（第一版看到 +22.5 MB 就是这个假象）。
  - **6 个真实缺陷**（都只在高 DPI 或第二轮才现形）：①进程没有 DPI 感知，坐标被整套虚拟化（3840×2160 报成 2560×1440 / scale 1.00，面板落在目标位置的 2/3）→ `SetProcessDpiAwarenessContext` 必须是 `main` 第一句；②`SW_HIDE` 饿死渲染循环；③停靠时 `SetWindowPos` 传 0 尺寸把窗口改成 0×0（eframe 截图 panic）；④`set_pixels_per_point(1.5)` 在 1.5 原生上**叠加成 2.25** → 改用 zoom factor `target/native`；⑤预热放在构造函数里会 panic `No fonts available`；⑥架构门禁扫到我自己注释里的 `windows::`——改注释措辞，不放宽门禁。
  - **常驻 130.1 MB 未达 65 MB 解释线（⚠️ 不掩饰）**：`--no-font` 对照测出 CJK 字体 + 图集占 **37.7 MB**（92.5 MB 基线），其余 ~92 MB 是 GL 驱动 + winit + egui 固定开销；仍比 Tauri 静置 262 MB 低 50%。字体子集化是首要可压缩项，留给 Phase 1。
  - **smoke 一直是红的（顺带发现并修掉）**：`Responsiveness budget` 拿 60 ms 判**现 Tauri 版**（p95 101 ms），从打点落地那天起每轮必然 FAIL——一个注定失败的步骤会把另外九个步骤的信号一起淹掉。改为 `Tauri latency baseline (report)` + `-NoFail`：数字照旧打印 `Verdict: FAIL`，只是不再染红整轮；判决继续在原生壳上，退役前（P4）当回归护栏用（越过 ~130 ms 才算真回归）。同轮补 CI 的 `native-crates` job（`cargo test --workspace` 27 项 + release 构建），此前 CI 完全不碰 `native/`，S1 只在作者机器上编译过。
  - **文档回填**：主设计 §4.3 状态、§7.1 新增"原生 S1 实测"表（含门禁口径），`native/README.md` 列出 Phase 1 必须偿还的捷径（`FindWindowW` 拿 HWND、`--scale` 模拟缩放、无 blit 缓存、面板内容静态、无托盘）。
  - **未测**：真实多显示器 + 混合 DPI 跨屏（本机单屏，`--scale` 只证明数学）、`lite` 在真实 RDP 回环（本机 `remote=0`）、首帧直接 blit 上一帧纹理（未做，当前靠预热帧留在 swap chain 达成等效）。

- `2026-11-24` **P0.5 完成：窗口生命周期 + 计时契约，静置 600 → 282.8 MB**。设计与验收标准先落在 [plans/2026-11-24-window-lifecycle-and-latency-contract.md](./plans/2026-11-24-window-lifecycle-and-latency-contract.md)，再按 A/B/C 三批实施并逐条验收。
  - **Batch A（度量）**：`measure-footprint.ps1` 增加 `-Label` / `-OutFile`，三姿态（idle / launcher / manager-2notes）快照归档到 `docs/releases/footprint/0.2.0/`。注意 PS 5.1 里 `$json` 与开关 `-Json` 是同一个变量，赋值会直接报错。
  - **Batch B（生命周期）**：Manager 从 `tauri.conf.json` 移除，改由 `ensure_manager_window` 按需创建、关闭即销毁；`close_all_note_windows` 改用 `destroy()`。实测 **idle Private 600.0 → 261.6–282.8 MB（−53% ~ −56%）**、WebView2 进程 7 → 6、sidecar 静置 0（懒启动）；关闭 Manager 12 ms 内释放，重建 222–248 ms；10 轮开关与 2 笔记开关均无累积。**顺带修掉两个真实缺陷**：`single-instance` 插件必须最先注册（否则第二个进程先建窗再退出，留下 `about:blank` 与热键冲突），且不能在事件循环线程里就地 `build()` WebView（会死锁，改为 `async_runtime::spawn`）。
  - **Batch C（计时契约）**：热键回调打 `[timing] hotkey_down`（在选区捕获之前，180 ms 上限要计入），Launcher 连续两帧后回调 `report_first_frame`；`log_timing()` 无视 debug 开关落盘。实测 **p50 87 ms / p95 101 ms / p99 130 ms（n=31）**，作为原生壳的对照组写入设计 §7.1；目标仍是 p95 ≤ 60 ms。
  - 未触发"动因被证伪"分支（降幅 53% ≫ 25% 判定线）。Phase 0 剩余工作是 S1–S4 四个 spike。

- `2026-11-24` **光标跟随主题 + 修掉"清空正文不落库"**。两项都是上一个会话收尾时发现、用户确认要改的。
  - **光标颜色**：`editor.cursor_color` 的默认值原来是硬编码 `#4f5a43`（深橄榄），暗色面板上对比度极低。改为**空串 = 跟随主题**：CSS 里 `--editor-caret: var(--manager-accent)` 与 `.manager-caret / .quick-note-caret { caret-color: var(--editor-caret) }` 本来就存在，只是被 `EditorTextarea` 的内联 `caretColor` 覆盖了；现在内联只在用户显式选色时输出（`cursor_color || undefined`）。设置面板加「跟随主题色（推荐）」开关，跟随时色板与文本框禁用并显示当前主题 accent（`getComputedStyle` 读 `--manager-accent`，主题切换后重读），不再有指向某个具体颜色的误导。旧档案里的 `#4f5a43` 在读写两侧都归一化为空（`normalize_cursor_color`），本机 DB 已一次性迁移并留备份。
  - **清空不落库（真 bug，双根因）**：前端 `buildPersistParams` 在「空内容且无标题」时直接返回 `null`，保存请求根本没发出；后端 `save_note_with_conn` 在同样的条件下提前 return，即使收到请求也不写 UPDATE。两处都在防「自动保存把空壳笔记塞进列表」，但都没区分**新建**与**已有**。修法一致：新建空笔记仍跳过（不产生空壳），已有笔记清空必须落库。前端由 `useQuickNoteSession` 的 `notePersistedRef` 决定是否传 `allowEmpty`（`getNote` 命中或首次保存成功置真，新建笔记置假），后端把守卫收紧成 `!exists && …`。
  - **实测**（release 构建 + WebView2 CDP）：`get_settings` 返回 `cursor_color = ""`；快速笔记 `textarea` 的 computed `caret-color = rgb(134, 171, 220)` 即 `--manager-accent`，内联属性为 `none`；写入 10 字符后落库、清空后该行 content 变为 `""`（修复前会一直保留旧文本）。截图确认暗色下正文清晰、工具栏四段互不遮挡。
  - **验证**：Rust `cargo test --lib` 96 项（新增 5：守卫归一化、默认跟随主题、清空落库、新建空笔记不落库）、前端 57 文件 / 418 项（新增 `lib/notePersistence.test.ts` 5 项）、`tsc --noEmit` 通过；release exe 已部署到安装目录（旧版备份 `easyflowhub.exe.bak-20261124-2227`）。
- `2026-11-24` **Phase 0.5 首批止血：自启动四步修复落地 + 两个快速笔记缺陷**。用户报告"设置里的自启动开关不知何时被关掉了"，与 §2.4 的诊断一致（状态只存在于注册表，被外部清除后无法自愈、UI 静默 OFF）。新增 native `autostart.rs`：`settings` 表键 `autostart = {"enabled":bool}` 保存意图（**不放进 `AppSettings`**，避免设置面板全量保存用旧快照覆盖），`reconcile()` 在启动时补回被删的启动项，`apply()` 写入后回读校验并回滚半切换状态，`get/set_autostart_state` 取代前端对插件直调；`single_instance` 回调忽略 `--autostart`，开机不再抢弹 Manager。对账边界：意图为关而系统存在时**不自动关闭**（可能被安装器/用户重新添加）。实测（dev 构建 + WebView2 CDP）：删掉 `HKCU\...\Run\EasyFlowHub` 后重启自动补回，`StartupApproved` 写回 `02`；`set` 关→开链路正确。同轮修两个快速笔记 UI 缺陷：`EditorTextarea` 的 `quick-note` 变体把正文颜色硬编码成 `#1a1a1a`，暗色主题下正文不可见 → 改用 `var(--quick-text)`（实测 computed `rgb(220,230,244)`）；`QuickNoteToolbar` 的 `•••` 是绝对居中，与 flex 流里的预览/置顶按钮在窄窗口重叠 → 改为左/中/右三段 flex（实测 300px 宽下工具按钮 132–158、预览 234–262、置顶 264–292，无交叠）。验证：前端 56 文件 / 413 项、Rust `cargo test --lib` 91 项、tsc、vite build、`check-architecture` 全通过。
- `2026-11-24` **拿到首批内存基线 + 定位自启动失效**：启动应用实测托盘静置（manager / launcher 均 `visible:false`、无人操作）**Private 545.3 MB / Working Set 846.1 MB / 10 进程**（WebView2 7 个，最大单进程 352.9 MB；启动后 30 s 与 130 s 两次采样 564→546 MB，为稳态而非瞬时峰值），是原生 65 MB 解释线的 8.4 倍——"占用与是否在用无关、只与隐藏窗口数相关"从判断变成证据。附带发现开机自启动失效：`HKCU\...\Run` 无 EasyFlowHub 项，但 `StartupApproved\Run` 留有记录（说明曾创建后被移除）；根因是**自启动状态只存在于注册表项、未进 `settings` 表**，被外部清除后应用无法自愈且 UI 静默回 OFF。已写入设计 §2.4 与 Phase 0.5，修复含四步：偏好持久化 / 启动对账 / 写入后校验 / 单实例回调尊重 `--autostart`（当前会弹 Manager）。同时确定 Launcher 动作网格**固定 4 列、窄屏降 3 列**（列宽 <96 逻辑像素即降档，两套布局启动期算好并缓存，滚动位置按项索引保存）。`measure-footprint.ps1` 输出改为单次 success 流写入，可被 `Out-File` 重定向且空行不再被吞。
- `2026-11-24` **内存降为次级目标 + 度量只在本机**：用户确认"8 GB 机器内存足够、额外设备测试没意义、测延迟才有意义"，且"保障常用功能速度前提下自然越小越好"。§7.2 重写为"次级目标 + 解释线"（65/55 MB 不再阻塞发布），补常驻内存构成拆解（刚性/半刚性/软性 + 结构性下限 30–50 MB）、6 条必做免费优化、4 条禁止的反向优化、RC 趋势归档规则；Phase 0 移除第二台机器与 RDP 阻塞要求；G1 与 S1 判据明确"响应阻塞、内存解释"。
- `2026-11-24` **实测本机画像，修正 DPI 相关估算**：Ryzen 7 4800H / 31.4 GB / Radeon 集显 / **3840×2160 @150%** / Win11 build 26300。由此发现原估算按 1366×768 计不可用：150% 下字形面积 ×2.25，原计划的"3500 常用字图集"≈30 MB+，全屏首帧纹理 ≈33 MB（3840×2160×4）。改为**图集只预栅格化首屏字符集（300–800 字，3–6 MB）+ Launcher 必须是面板而非全屏层**，并写入 ADR 作为硬约束。`measure-footprint.ps1` 同步输出 `screen=分辨率 @DPI` 与主 GPU，避免跨 DPI 误比内存。顺带测得系统 WebView2 Runtime 已由 10 月的 878.7 MB（v157）涨到 2029.3 MB（v158）——一个月翻倍。
- `2026-11-24` **澄清 Launcher 语义 + 高 DPI 专项优化**：用户指出 launcher 模仿 Quicker 快速面板——**跟随光标的竖版动作面板，不是工作台**，工作台另有页面。据此改正设计：面板尺寸按实测 `tauri.conf.json` 定为 440×860 逻辑像素（此前误写为 960×640 横版）、首帧缓冲 150% 下 3.4 MB 而非全屏层 33 MB；新增 §5.1.1「光标锚定与高 DPI」，含逐档缓冲/图集表、5 条零延迟优化（只按面板尺寸分配缓冲、图集只栅格化首屏字符、缩放分键最多 2 档、图标按需栅格化、关闭 MSAA/中间 FBO/模糊）与 3 个延迟坑（热键路径不枚举显示器、`pixels_per_point` 必须在 `show` 前设好、DPI 切换时新旧图集交接避免空白帧）。同时修正 Phase 0.5：**launcher 仍预创建**（懒创建会破主门禁），止血只动 manager 与临时窗口。`measure-footprint.ps1` 扩展为逐显示器分辨率 + 缩放输出。
- `2026-11-24` **定稿预热边界**：用户确认"60 ms 已足够，最常用功能预热、其余冷启动、均衡即可"。设计新增 §5.8.1 预热分级（P0 启动预热 / P1 会话内复用 / P2 冷启动 / 不预热），判据为"每天十次以上"；同时把"不做无限加码"写入非目标——60 ms 是终点，继续下探会拖慢冷启动且增预热度过期类缺陷。ADR-0009 第 8 条同步补边界。
- `2026-11-24` **修正目标环境**：撤销 4 GB 画像（用户反馈可接触的最低配置为 8 GB，4 GB 机器"运行 Win10 都勉强"），最低支持改为 8 GB / 4 核 / 集显；`lite` 重新定位为"软件渲染 / RDP 带宽受限"降级档，**集显改为走 standard**；lite 常驻上限 45→55 MB（内存已非主要矛盾）；Phase 0 度量改为本机集显 + `mstsc` 回环，不再需要额外硬件。
- `2026-11-24` 确认 Native Runtime 方向（ADR-0009 `accepted`）：分阶段、成熟方案优先、低占用并适配 4 GB/集显/远程桌面、UI 简洁优先 + 主题系统。设计补入 `lite` 降级、按姿态预算与架构门禁；新增 `scripts/check-native-architecture.mjs`（crate 分层 / 越界符号 / 单文件 800 行，`native/` 缺失时 skip）并接入 `run-smoke.ps1` 与 CI `architecture-gates`；`ARCHITECTURE_GUARDRAILS.md` 新增第 10 节 native 层图。**未改产品代码**，等待 Phase 0 四姿态/四 spike 与 G0。

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
- **发布产物与 CI（2026-10-21）**：Release 与 Test 工作流通过。发布资产为 `EasyFlowHub_0.2.0_x64-setup.exe`、`EasyFlowHub_0.2.0_x64_en-US.msi`、`EasyFlowHub_0.2.0_portable.exe`、`scriptmgr.exe` 与 `latest.json`（`sha256` 与 CI 构建的 NSIS 安装包一致，释放为 prerelease，因此 `releases/latest` 仍指向 v0.1.1，需正式发布时手动切换）。期间修复 `Native Tests (Windows)` 的 `cargo check`：`tauri.conf.json` 将 `scriptmgr.exe` 声明为资源，缺少该文件时 Tauri build script 直接失败；PR 工作流现在先安装 Go 并构建 Windows amd64 sidecar 再运行 cargo 步骤（本地 `tauri build` 因 build.rs 先生成而从未暴露该问题）。
- **应用内更新签名链路（2026-10-21）**：已闭合。启用 `bundle.createUpdaterArtifacts` 后 CI 产出 `.exe.sig` / `.msi.sig`；发布工作流改为上传签名产物并按 Tauri v2 格式生成 `latest.json`（`platforms.windows-x86_64.signature/url` + `pub_date`，`json.tool` 校验）。旧公钥指向已弃用的 deskflow 密钥、与签名密钥不匹配，导致更新从未成功；现已换新无密码 minisign 密钥对（key ID `F8A2197D5E9678E6`），`tauri.conf.json` 公钥与 GitHub Secret 同步轮换。用 minisign 对 CI 产出的 `EasyFlowHub_0.2.0_x64-setup.exe` 验签通过（trusted comment 含 `version:0.2.0`）；已用新安装包重装本机应用，新公钥已烘焙进 `easyflowhub.exe`。旧密钥以 `*.bak` 备份在 `C:\Users\yukin\Desktop\secret\easyflowhub\`。v0.2.0 已按修好的流水线重新发布。
- **本地 release 构建资源嵌入修复（2026-10-22）**：本地增量构建时 cargo 不会因 `dist` 变化重跑 build script，`tauri::generate_context!` 会复用上一次的资源表；Vite 每次构建轮换 `assets/index-<hash>.js`，于是产出的 exe 内嵌资源与 `index.html` 引用不匹配，表现为 manager 白屏、launcher 全透明（`setup` 仍会执行，`page_load` 只到 `Started`）。`src-tauri/build.rs` 现声明 `cargo:rerun-if-changed=../dist`，前端产物变化会触发资源上下文重新生成。CI 为全新构建不受影响，该缺陷只影响本地打包。
- **Launcher 与快速笔记体验修复（2026-10-22）**：① Launcher 四周 4px 透明带来自 `.launcher-shell` 的 `padding` 与 `calc(100vw - 8px)` 叠加，已改为 `padding: 0` + panel 铺满，并去掉透明窗口内无意义的外阴影；② 快捷键冲突新增「自动避开冲突」，按候选组合真实试注册（`Ctrl+Shift+Space → Ctrl+Alt+K → Ctrl+Alt+J → Ctrl+Shift+F10 → Alt+Shift+Space`），成功即落盘并提示，全部失败才报错；Windows `RegisterHotKey` 为独占注册，对方用键盘钩子时无法强制抢占，故只提供避让与明确说明；③ 快速笔记新增「编辑 / 预览」切换，预览复用 `MarkdownPreview`（含 `asset://` 解析与双击放大），图片在正文内联显示，顶部缩略条在预览态隐藏。
- **Launcher 快捷键卡死修复（2026-10-22）**：快捷键回调运行在事件循环线程，而 `capture_selected_text` 会在其中同步执行 `SendInput` 模拟 Ctrl+C、读写剪贴板并 `sleep(45ms)`；当 QQ 截图等第三方工具因键盘钩子同时响应并抢占剪贴板 / 焦点时，这段被显著拉长，导致窗口画出后消息泵阻塞、表现为「未响应」且关闭按钮失效。现在抓取改为后台线程 + `SELECTION_CAPTURE_TIMEOUT_MS = 180ms` 硬超时（超时仅放弃「选中文本」增强，Launcher 照常弹出），并增加 `KEYBOARD_TRIGGER_DEBOUNCE_MS = 260ms` 防抖，吞掉同一次物理按键被钩子重复派发的多次触发；鼠标中键触发同样走超时版本。冲突本身无法程序化消除（对方使用键盘钩子时系统无法强制独占），仍以「自动避开冲突」换键为最终解法。
- **未完成项（发布后跟进）**：Windows 打包态人工 smoke 与 P2 Voice 验收顺延到 `v0.2.x`。
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

- `2026-10-21` 新增「关于与更新」设置段（版本显示 + 检查更新）、「实验性功能（预览）」分区，并更换应用图标为方案 2（玻璃流转）。
  - **版本与更新**：`AboutSection` 显示当前版本（`getVersion`），「检查更新」拉取 GitHub `releases/latest` 做 semver 比对，区分已是最新 / 发现新版本（带下载）/ 当前更高（内测或本地构建）/ 检查失败；比对逻辑抽为 `lib/updateCheck.ts` 纯函数。
  - **实验性功能**：`FeatureModule` 新增 `experimental` 标记；`files` / `downloads` / `voice` 改为实验 + 默认关闭，只在实验分区出现；顶部有「开发中功能」警告，启用模块带「实验」角标。翻译非模块，用 `lib/experimental`（localStorage）+ `hooks/useTranslationExperimental`（Tauri 事件跨窗口同步）独立门控，关闭时 Launcher 不注册翻译动作、设置不渲染翻译段与导航。
  - **图标**：采用方案 2（叠层毛玻璃 + 流向箭头），`design/icons/generate.py` 从 1024 基准图 LANCZOS 缩放生成全套 PNG / 多帧 ICO（16-256）/ ICNS（16-512），替换 `src-tauri/icons/`。
  - 验证：TypeScript、前端 56 文件 / 413 项、主题 token 审计、架构门禁通过。
- `2026-10-21` 修复 Windows 升级安装失败（`Error opening file for writing: ...\scriptmgr.exe`）：常驻 Go sidecar 在主窗口关闭到托盘后仍存活并锁住自身 `.exe`，NSIS 无法覆盖。新增 NSIS `installerHooks` 的 `NSIS_HOOK_PREINSTALL`，安装前先 `taskkill` 主程序再杀 sidecar 并短暂等待释放文件句柄；用一把仍被 sidecar 锁住的 `scriptmgr.exe`（PID 27688）实测新安装包退出码 0 并成功替换文件。
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

## Native Runtime Track（`accepted`，ADR-0009）

> 状态：**Phase 0 四项 spike（S1 壳 + S2 文本岛 + S3 数据 + S4 门禁）全部完成（2026-11-25），待 G0 评审**。分阶段推进；过 G0 后开始 Phase 1。
>
> - 设计：[plans/2026-11-24-native-runtime-redesign.md](./plans/2026-11-24-native-runtime-redesign.md)
> - 决策：[decisions/ADR-0009-windows-native-runtime.md](./decisions/ADR-0009-windows-native-runtime.md)
> - 架构约束：[ARCHITECTURE_GUARDRAILS.md](./ARCHITECTURE_GUARDRAILS.md) 第 10 节
> - 度量 / 门禁：`scripts/measure-native-latency.ps1`（原生热唤起，p95 48.9 ms 实测）、`scripts/measure-latency.ps1`（现 Tauri 版，**对照基线**）、`scripts/measure-footprint.ps1`（内存/磁盘）、`scripts/check-native-architecture.mjs`（分层）

### 已确认约束

1. **响应优先（最高优先级）**：以常驻内存换速度。主门禁为热唤起 热键→首帧 p95 ≤ 60 ms（`lite` ≤ 120 ms）、冷唤起 ≤ 400 ms。**60 ms 即终点**，不追加到 30–40 ms（继续下探会拖慢冷启动）。
   - **内存是响应约束下的次级目标**："保障常用功能速度的情况下自然越小越好"。65 MB（`lite` 55 MB）是解释线而非阻塞项；结构性下限约 30–50 MB（渲染设备 + 字体图集 + 首帧纹理），且随分辨率/缩放变化——4K@150% 下全屏首帧纹理 **≈33 MB**、3500 字图集 ≈30 MB+，因此 **Launcher 必须是面板而非全屏层、图集只预栅格化首屏字符集**。
   - **高像素设备专项**：缓冲只按面板尺寸分配（440×860 逻辑 @150% = 660×1290 ≈ 3.4 MB，全屏层则 33 MB，差 10 倍）；图集与图标缓存按缩放分键、跨屏最多保留 2 档；跨屏/改缩放时新旧图集交接，避免文字空白一帧。逐档数值见设计 §5.1.1。
   - 省内存分两类：**零响应代价的必须做**（隐藏态不重绘、列表虚拟化、图片降采样、关窗释放纹理、图集单档裁剪、worker 空闲退出）；**用响应换的一律不做**（关闭即销毁 Launcher、空闲释放图集、取消预创建）。
   - **预热只给常用路径**：P0（Launcher 窗口/渲染设备/字体图集/首帧纹理/最近项快照、快速笔记控件、SQLite 常开）与 P1（会话内复用 Manager 渲染设备与 FTS）预热；P2 低频能力（文件搜索/下载/语音/relay/MCP/扩展/回收站深扫）一律冷启动、允许首屏空态渐进填充。判据是"每天十次以上"，详见设计 §5.8.1。
2. **分阶段**：常驻壳 → 工作台 → 编辑器 → 下线 Tauri，每阶段可回退；迁移期按 T-Gate 0–3 逐格移交，最多 1 个 WebView 进程，T-Gate 0→1 不超过 8 周。
3. **成熟优先**：egui + 系统 RichEdit 为主路径；G0 失败才启用 C# WinUI 3 退路。
4. **目标环境**：最低 8 GB / 4 核 / 集显；4 GB 不承诺可用（允许启动，不进 CI、不占预算）。`lite` 只用于软件渲染 / RDP；**集显走 standard**（原设计让集显进 lite 是错的）。lite 的取舍是"仍预热 P0，但不预加载最近项、不做首帧纹理缓存"，不是"不预热"。
5. **UI 简洁优先**：实色分层，不做毛玻璃/模糊/渐变/发光；主题 token 负责美观；Mica 保留为可选主题（默认关闭、Win11 显式开启）。
6. **架构门禁**：crate 分层 + `check-native-architecture.mjs`，CI 阻塞，例外必须 ADR。
7. **Launcher 语义（产品澄清）**：launcher 是**跟随光标的竖版动作面板**（Quicker 式，440×860 逻辑像素，光标所在显示器居中夹紧），工作台（Manager）是另一个窗口、另一种入口，两者不合并；不做全屏遮罩。这个语义同时决定了首帧缓冲只有 3.4 MB（150% 下）而不是 33 MB。

### 动因

- 产品事实是 Windows-only，但运行时仍为每个窗口支付一套 WebView2 renderer；manager 与 launcher 以 `visible: false` 创建，托盘静置也常驻。
- 2026-10-22 白屏事故的三个根因（WebView2 profile 半写、`EBWebView` 损坏、cargo 增量未重新内嵌 `dist`）都来自 WebView 运行时与资源内嵌模型。
- 静态实测（2026-11-24）：安装目录 39.4 MB、`easyflowhub.exe` 22.8 MB、`scriptmgr.exe` 16.6 MB、WebView2 profile 55.5 MB。**托盘静置内存实测 545.3 MB Private / 846.1 MB Working Set / 10 进程**（详见设计 §2.2）；Launcher 与 Manager+2 笔记两个姿态待补。

### 目标

- 热唤起 p95 ≤ 60 ms（`lite` ≤ 120 ms）、冷唤起 ≤ 400 ms（**唯一阻塞发布的指标**）；常驻 ≤ 65 MB（`lite` ≤ 55 MB，解释线）；Manager + 2 快速笔记 ≤ 220 MB；安装包 ≤ 25 MB。
- 度量只在本机进行，不设第二台低配机、不做跨机对比——内存不是可用性瓶颈，延迟才是决策依据。
- 单进程原生 Rust 壳（egui 页面 + 系统 RichEdit 文本岛），T0–T2 原生，T3 低频能力退为按需 worker。
- 数据契约不变：继续使用 `notes.db`，双栈期单写者，升级前自动备份。

### Phase Plan

| Phase | Status | Goal | Gate |
|---|---|---|---|
| P0. 度量与 Spike | `[partial]` | ~~三姿态内存基线~~（已完成，见设计 §2.2.1）+ ~~计时契约~~（已完成）+ ~~S1 壳（含预热路径）~~（**已完成，p95 48.9 ms，见 §7.1**）+ ~~S2 编辑器~~（**已完成，键入 p95 4.95 ms，2 项人工未测**）+ ~~S3 数据~~（**已完成，39 项检查，双向兼容 + 无 schema 破坏**）+ ~~S4 门禁~~（**已完成，23 项自检 + 8 次变异，补 3 个真实缺口**）；延迟是重点，RDP 回环可选 | `G0` |
| P0.5. 先行止血 | `[done]` 2026-11-24 | 现版本窗口生命周期改造：**manager 不预创建 + 关闭即销毁**、临时窗口关闭即销毁（launcher 仍预创建，不动主门禁）；**修复开机自启动失效**（偏好持久化 + 启动对账 + 写入校验）。实测静置 600 → 262–283 MB（−53% ~ −56%） | — |
| P1. 常驻壳 | `[pending]` | 主题 token、托盘、热键、**预创建 Launcher**、快速笔记、待办卡片、`lite` 降级；Manager 暂由 Tauri 按需启动 | `G1` |
| P2. 工作台 | `[pending]` | 笔记 / 收集箱 / 待办 / 脚本 / 设置 / 回收站原生，低频面板只留最小入口 | `G2` |
| P3. 编辑器 | `[pending]` | RichEdit 文本岛、Markdown 装饰、图片、预览、自动保存 | `G3` |
| P4. 下线 Tauri | `[pending]` | 删除 WebView 依赖，更新器与安装器切换，回滚演练 | `G4` |

### 范围边界

- v2 原生范围按设计第 3.3 节收窄，约 9k 行 UI 等价物；全量 13 面板 parity 不作为 v2 目标。
- files / downloads / voice / relay / mcp / extensions 降级为 worker + 最小入口（已确认）；tasks 合并进 scripts 运行记录。
- **不合并 `scriptmgr-go`**：合并对响应门禁贡献接近 0，成本是重写 12.8k 行 + 7.5k 行测试；仅当 J1–J4 判据触发时复核（见设计 Phase 5）。替代方案为 worker 生命周期优化。

### 待办

- 无待确认项。后续仅由 G0–G4 实测驱动调整；偏离主门禁的取舍必须走 ADR 例外。


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
