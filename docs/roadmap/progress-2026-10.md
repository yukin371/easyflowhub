# 进展日志 · 2026-10

> 从 [../roadmap.md](../roadmap.md) 拆出：ADR-0001 要求路线图只维护当前版本目标、活跃轨道、验证债与下一队列，不保留长历史。
> 按月归档，**新的条目先写进当月文件**，再把文件加进 `roadmap.md` 的索引。
> 相邻月份：[2026-11](./progress-2026-11.md) · [2026-04](./progress-2026-04.md)

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
