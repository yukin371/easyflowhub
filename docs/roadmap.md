# EasyFlowHub Roadmap

## v1.0.0

### Source Of Truth

- 正式需求基线：[PRD-v1.0.0.md](./PRD-v1.0.0.md)
- 项目画像：[PROJECT_PROFILE.md](./PROJECT_PROFILE.md)
- 架构边界：[ARCHITECTURE_GUARDRAILS.md](./ARCHITECTURE_GUARDRAILS.md)
- 长期产品方向：[decisions/ADR-0003-product-direction-and-action-first.md](./decisions/ADR-0003-product-direction-and-action-first.md)
- 交付拆解计划：[plans/active/2026-04-24-v1-delivery-plan.md](plans/active/2026-04-24-v1-delivery-plan.md)
- 任务看板：[plans/active/2026-04-24-v1-task-board.md](plans/active/2026-04-24-v1-task-board.md)

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
- 审查基线：[2026-10-19 架构与大文件审查](plans/completed/2026-10-19-architecture-and-file-size-review.md)。审查已完成；阶段A（F01–F05）与阶段B（F06–F14）已实施，阶段A（F01–F05）、阶段B（F06–F14）、阶段C（F11 + 第5节大文件拆分）与阶段D（V1-F5 CI 门禁）均已实施，四个阶段全部完成。
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

三层归档（ADR-0001：路线图不保留长历史。新条目写进当天的日志，周与月只做摘要，约定见 [journal/README.md](./roadmap/journal/README.md)）：

- [2026-11](./roadmap/journal/month/2026-11.md) —— Native 运行时 G0 与 S1–S4、`notes.db` 统一迁移器、Phase 0.5 止血
- [2026-10](./roadmap/journal/month/2026-10.md) —— 架构审查阶段 A–D、语音转写、v0.2.0 发布收尾
- [2026-04](./roadmap/journal/month/2026-04.md) —— v1.0.0 立项、PRD 与交付计划、extension 平台

## v0.2.0（已发布）

已归档：[roadmap/v0.2.0.md](./roadmap/v0.2.0.md) —— 目标、轨道、发布门禁与当年的进展记录，冻结不再追加。

## Native Runtime Track（`accepted`，ADR-0009）

> 状态：**G0 已通过（有条件，2026-11-25）**，进入 Phase 1。四项 spike（S1 壳 + S2 文本岛 + S3 数据 + S4 门禁）全部完成；5 条退出条件（C1 字库 / C2 三种输入法人机验收 / C3 多屏混合 DPI / C4 门禁常绿 / C5 内存线收口）见 [plans/completed/2026-11-25-g0-review.md](plans/completed/2026-11-25-g0-review.md) §9，其中 **C1 已于 2026-11-27 关闭**（[记录](plans/completed/2026-11-27-c1-font-not-private.md)）。栈版本锁定：egui/eframe **0.31**、`rusqlite 0.32 bundled`、RichEdit（系统组件不锁）。
>
> - 设计：[plans/active/2026-11-24-native-runtime-redesign.md](plans/active/2026-11-24-native-runtime-redesign.md)
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
| P0. 度量与 Spike | `[done]` 2026-11-25 | ~~三姿态内存基线~~（已完成，见设计 §2.2.1）+ ~~计时契约~~（已完成）+ ~~S1 壳（含预热路径）~~（**已完成，p95 48.9 ms，见 §7.1**）+ ~~S2 编辑器~~（**已完成，键入 p95 4.95 ms，2 项人工未测**）+ ~~S3 数据~~（**已完成，39 项检查，双向兼容 + 无 schema 破坏**）+ ~~S4 门禁~~（**已完成，23 项自检 + 8 次变异，补 3 个真实缺口**）；**G0 已通过（有条件）**，未闭合项转为 C1–C5 | `G0` ✅ |
| P0.5. 先行止血 | `[done]` 2026-11-24 | 现版本窗口生命周期改造：**manager 不预创建 + 关闭即销毁**、临时窗口关闭即销毁（launcher 仍预创建，不动主门禁）；**修复开机自启动失效**（偏好持久化 + 启动对账 + 写入校验）。实测静置 600 → 262–283 MB（−53% ~ −56%） | — |
| P1. 常驻壳 | `[in_progress]` 2026-11-25 启动 | 主题 token、托盘、热键、**预创建 Launcher**、快速笔记、待办卡片、`lite` 降级；Manager 暂由 Tauri 按需启动。**启动项**：C1 字库映射 ✅ 2026-11-27（Commit 130.1 → 91.5 MB，[记录](plans/completed/2026-11-27-c1-font-not-private.md)）、C2 三种输入法人机验收（第一周硬性，**待执行**） | `G1` |
| P2. 工作台 | `[pending]` | 笔记 / 收集箱 / 待办 / 脚本 / 设置 / 回收站原生，低频面板只留最小入口 | `G2` |
| P3. 编辑器 | `[pending]` | RichEdit 文本岛、Markdown 装饰、图片、预览、自动保存 | `G3` |
| P4. 下线 Tauri | `[pending]` | 删除 WebView 依赖，更新器与安装器切换，回滚演练 | `G4` |

### 范围边界

- v2 原生范围按设计第 3.3 节收窄，约 9k 行 UI 等价物；全量 13 面板 parity 不作为 v2 目标。
- files / downloads / voice / relay / mcp / extensions 降级为 worker + 最小入口（已确认）；tasks 合并进 scripts 运行记录。
- **不合并 `scriptmgr-go`**：合并对响应门禁贡献接近 0，成本是重写 12.8k 行 + 7.5k 行测试；仅当 J1–J4 判据触发时复核（见设计 Phase 5）。替代方案为 worker 生命周期优化。

### 待办

- 无待确认项。后续仅由 G0–G4 实测驱动调整；偏离主门禁的取舍必须走 ADR 例外。
- **WAL 切换的前置条件（G0-D3 裁决）**：当前保持 `journal_mode=delete`。出现下列任一情形时，**先写 ADR 再改**，不得在实现里顺手切：①原生与 Tauri 同时写同一库（双写者）；②引入同步 / 备份 / 云盘工具并发访问 `notes.db`；③需要读写不互斥才能满足的延迟目标。改动会改文件头 write version 并留下 `-wal`/`-shm`，备份与"复制 db"的语义都随之变化。
- **内存线复核（G0-C5）**：C1 落地后 Commit **91.5 MB**（≤ 100），触发条件未成立，故不要求列下一项压缩；**65 MB 解释线仍未动**，它是否可达由 G1 裁决且必须写 ADR。


## Candidate Next Version

### v1.1.0

- **首要产品目标：Action Launcher**：提供全局唤起、动作搜索、最近使用、收藏、参数输入和执行结果反馈。
- 先定义统一 action registry / contract，再把现有脚本包装为 `script` 动作，并逐步接入应用、文件、URL、文本和 AI 动作。
- Launcher 的输入应逐步支持当前剪贴板、选中文本、拖入文件和显式参数；输出应支持剪贴板、文件、笔记和通知。
- v1.1.0 不要求一次完成复杂 Flow 编辑器、社区市场、云同步或任意代码插件执行。
- AI 宿主切换与 relay 的正式需求基线：[PRD-v1.1.0-ai-host-routing.md](./PRD-v1.1.0-ai-host-routing.md)
- AI 宿主切换实施计划：[plans/active/2026-04-25-ai-host-routing-implementation-plan.md](plans/active/2026-04-25-ai-host-routing-implementation-plan.md)
- extensions `Phase 2-4`：启停状态持久化、热重载、安装 / 卸载。
- relay 运行时与负载均衡能力继续收口，明确正式支持的 route selection、weighted round robin 与最小 failover 边界。
- relay 密钥管理与配置体验进一步收口，减少 raw JSON 依赖。
- Claude / Codex 宿主配置切换继续收口，补齐 dry-run / diff / preset，而不是把协议不兼容的宿主硬塞进单一 relay。
- MCP 外部 server 生命周期管理增强。
- 在 Action Launcher 稳定后，再评估更强搜索、Flow 编排和扩展生态能力。

## Maintenance Rules

- PRD 负责版本需求边界，roadmap 只保留 active 执行内容。
- 深设计细节放入 `docs/plans` 或 `docs/decisions`，不要继续堆进 roadmap。
- **进展日志三层**：在 `docs/roadmap/journal/` 下（`log/YYYY-MM-DD.md` → `week/YYYY-Www.md` → `month/YYYY-MM.md`）。条目只写进当天的日志，周与月只做摘要；上层必须引用下层，否则门禁报错。已发布版本整段移进 `docs/roadmap/vX.Y.Z.md` 冻结（见 [v0.2.0](./roadmap/v0.2.0.md)）。依据 ADR-0001：路线图不保留长历史。
- **单文件预算**：`docs/**/*.md` 软上限 40 KB、硬上限 128 KB；日志三层各有自己的上限（见 `scripts/doc-budget.json` 的 `layers`，日志层**没有**软上限）。超软上限的文件钉在 `pinnedBytes` 里、只许缩不许涨。`node scripts/check-doc-budget.mjs` 同时检查相对链接能否解析、月/周/日是否一条不缺地被上层引用。
- 每轮 checkpoint 结束后，回填验证结果、剩余风险和 next queue。
- **方案走生命周期**：`docs/plans/` 下分 `proposals/`（想清楚没排期）、`active/`（当前依据）、`completed/`（已交付）、`rejected/`（不做）、`archive/`（历史 / superseded / deferred）。状态由目录位置表达，归档就是 `git mv`；`node scripts/check-plan-lifecycle.mjs` 判位置与头部状态行是否一致、索引有没有漏、以及移动后有没有留下悬空路径。
