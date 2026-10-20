# src-tauri/src MODULE

> status: active
> owner: EasyFlowHub desktop runtime maintainers
> last_verified: 2026-10-20
> verified_against: working tree (scriptmgr sidecar lifecycle + inbox delete)

## 1. Responsibility

`src-tauri/src` 是桌面原生壳层，负责窗口生命周期、快捷键、托盘、SQLite 持久化、Tauri 命令注册，以及把前端请求桥接到 `scriptmgr-go`。

## 2. Owns

This module is the canonical owner of:

- Tauri 插件初始化与命令注册（`lib.rs` 仅做装配）
- Windows 外部应用发现与启动，包括 Snow Shot 官方 MCP bridge 截图会话调用
- Launcher 窗口行为（`launcher.rs`）与 manager / quick note / todo card 窗口行为（`app_windows.rs`）
- 笔记 / Inbox SQLite 持久化、图片与文件附件落盘、笔记回收站（Inbox 领域 owner 为 `inbox/` 模块）
- 受控单目录文件索引、最近打开记录、文件预览与 Windows 回收站操作
- URL 下载队列、目标目录 / 冲突策略持久化、进度与取消控制
- 录音后语音转文字：音频经 native 侧发往本机 Relay，transcript 附件落盘与读取
- Windows 系统剪贴板文本写入，供文件 / 下载路径动作复用
- 应用设置持久化
- 桌面外观、小组件与 MCP 信息暴露
- `scriptmgr` HTTP sidecar 进程生命周期：按需启动 `scriptmgr serve`、健康等待与退出回收
- 开发者模式诊断日志：有界内存环形缓冲、可选文件落盘与 sidecar 输出捕获
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
| `lib.rs` | composition root | 只保留 `run()`、插件装配、state 注入与命令注册；不含窗口或领域行为 |
| `launcher.rs` | launcher window owner | Launcher 选区捕获 / 替换、键盘 + 鼠标中键触发注册、显示 / 隐藏 / 置顶 / 拖拽，关闭改隐藏并统一发出 `launcher-hidden` / `launcher-focused` |
| `app_windows.rs` | manager / note / todo window owner | manager 关闭改隐藏、快速笔记与 todo 卡片窗口创建 / 定位 / 固定、快速笔记窗口批量显隐 |
| `notes.rs` | notes persistence | SQLite、图片、trash；`ensure_todo_inbox_note` 以事务原子复用或创建待办箱 |
| `inbox/` | capture persistence facade | 与 `notes.db` 共用数据库；按职责内分 `schema.rs`（表 / 迁移）、`attachments.rs`（附件身份与落盘 / transcript）、`repository.rs`（行访问）、`service.rs`（捕获 / 转换 / 删除用例与事务）、`commands.rs`（薄命令层，委托 service / repository） |
| `settings.rs` | settings store | 应用级设置 |
| `scriptmgr.rs` | script bridge | 调用 `scriptmgr-go` CLI / HTTP |
| `scriptmgr_server.rs` | sidecar lifecycle owner | 按需 spawn `scriptmgr serve --port 8765`、TCP + HTTP 健康等待、并发去重、应用退出回收子进程 |
| `logging.rs` | diagnostics owner | 500 行有界内存环形缓冲；`developer.debug_mode` 开启时追加写入 `logs/app.log`（2 MB 轮转）；捕获 sidecar stderr |
| `appearance.rs` / `widget.rs` / `modules.rs` / `mcp_server.rs` | native feature owners | 外观、桌面小组件、模块配置、MCP 信息 |
| `external_apps.rs` | external app bridge | Snow Shot 注册表 / 常见路径发现、MCP 状态检查与截图会话调用 |
| `clipboard.rs` | clipboard owner | Windows 系统文本剪贴板读写；路径复制与 Launcher 上下文/结果动作复用 |
| `file_search.rs` | file retrieval owner | 用户显式选择的单目录按需索引、搜索 / 预览 / 最近文件及带 generation 校验的文件操作 |
| `download.rs` | download owner | 用户主动 URL 下载队列；`.part` 续传、暂停 / 继续 / 取消 / 重试、冲突策略和完成后路径动作 |
| `voice.rs` | voice owner | Windows 默认 WASAPI 输入设备的专用采集线程、`cpal` 输入流、`hound` WAV 写入、pause / resume / stop 会话控制、`.wav.part` 完成提升、Inbox 音频归档与失败状态；非 Windows 保留明确 unavailable fallback |
| `transcription.rs` | transcription owner | 录音后语音转文字：按 Inbox item 读取音频附件，经 `TranscriptionTransport` trait 通过本机 Relay `POST /v1/audio/transcriptions` 发送 multipart 请求；25 MB 大小守卫、180 秒截止、固定中文错误分类、transcript 附件事务性替换与有界读取；不含任何 Provider 凭据 |

## 5. Key Dependencies

| Dependency | Why it exists | Risk if changed |
|---|---|---|
| Tauri plugins | SQLite、window state、shortcut、autostart 等原生能力 | 插件升级或 capability 变化容易造成平台回归 |
| `scriptmgr-go` binary | 脚本运行与外部协议桥接 | 路径解析或输出协议变化会破坏桌面层 |
| SQLite schema | 笔记与状态存储 | schema 变化需要同步数据兼容 |
| `cpal` + `hound` | Windows 麦克风输入流与 PCM WAV 封装 | 版本、Windows 音频后端或采样格式变化需要重新验证设备 / 权限 / WAV 可播放性；`cpal::Stream` 必须留在专用采集线程 |
| `reqwest`（blocking + multipart） | native 侧下载与转写 HTTP 请求 | 特性组合变化会影响 Range 续传或 multipart 上传；转写音频字节不得改经 WebView |

## 6. Dependents

Main callers / consumers:

- `easyflowhub-app/src/lib/tauri`
- `easyflowhub-app/src/pages`
- `easyflowhub-app/src/components`

## 7. Invariants

These must remain true after changes:

- 新前端可调用命令必须在 `lib.rs` 注册，并在前端补 typed wrapper
- `lib.rs` 只保留 `run()` / 插件装配 / state 注入 / 命令注册；窗口与领域行为必须留在各自 owner 模块（`launcher` / `app_windows` / `inbox`）。命令注册路径须指向命令实际所在模块（如 `inbox::commands::*`）。
- Inbox 内部分工固定为 schema / attachments / repository / service / commands；命令层保持薄委托，不得各自重复迁移、附件或事务逻辑。
- Inbox 以 additive schema 扩展 `notes.db`；不得改写现有 notes 表或 `asset:<uuid>` 图片引用格式。附件使用随机文件名，归档 / 转换不删除附件文件。
- Inbox 永久删除是唯一会删除附件文件的路径：未转换为笔记时删除 `assets/` 存储文件与 `audio/` 下同名原始录音；已转换为笔记（`converted_note_id` 非空）时保留文件，避免笔记正文里的 `asset:` 引用失效。文件删除为 best-effort，不能因单个文件失败回滚已提交的数据库删除。
- `scriptmgr` HTTP sidecar 的生命周期只由 `scriptmgr_server.rs` 拥有：前端每次 HTTP 调用前必须 `ensure_scriptmgr`（并发共享同一次启动，失败后允许重试），不得假设 `127.0.0.1:8765` 已就绪；应用退出必须回收本进程启动的子进程。已存在健康实例（外部启动或旧孤儿）时不得重复 spawn。
- scriptmgr 健康判定 = TCP 可连 + `GET /api/scripts` 返回 2xx。不得改用 HEAD：Go 1.22 method-specific pattern 对 HEAD 返回 405，会把健康服务误报为离线。
- 开发者模式（`developer.debug_mode`）只影响日志详细程度与是否落盘，不得改变任何安全边界或引入远程访问。日志缓冲固定 500 行上限，文件单份 2 MB 后轮转为 `app.log.1`；日志内容不得包含凭据。
- Inbox 转换笔记必须在同一 SQLite 事务中创建 note 并持久化 `converted_note_id`，重复执行返回原笔记，不能产生副本。
- 待办箱由 `ensure_todo_inbox_note` 单一 native owner 管理；使用 SQLite `BEGIN IMMEDIATE` 检查并创建，按 `TRIM(title) = '待办箱'` 复用 active note，跨 WebView 不得通过前端 `list -> create -> save` 组合创建。
- manager 窗口关闭时默认隐藏，不应直接退出应用
- quick note / todo card 标签命名必须保持唯一性
- Windows 特有行为（autostart、桌面固定、快捷键）变动后必须回归验证
- Snow Shot 已安装时，截图动作必须通过同目录官方 MCP bridge 调用可见截图会话；MCP 未启用提示用户在 Snow Shot「设置 → 系统」开启，不得误跳安装页。
- Snow Shot 自定义路径优先；自动发现需扫描 Windows 卸载注册信息，支持 D 盘等非系统盘安装。已发现主程序但找不到同目录 MCP bridge 时应报更新 / 修复错误。
- Launcher 不参与 window-state 恢复，默认隐藏且非置顶；关闭请求改为隐藏，不能销毁后导致全局入口失效。
- Launcher 各隐藏路径统一发出 `launcher-hidden`；前端依此取消可中止请求并清除当前上下文，不依赖失焦来判断关闭。
- 翻译偏好复用 settings SQLite 存储，旧数据默认简体中文、模型为空；密钥与 Provider 路由仍归 Go Relay 所有。
- 文件索引只扫描用户显式选择的单目录，不在启动时扫描且没有 watcher；首次建立 / 手动重建由点击刷新触发，成功的文件修改会重扫目录。root / paused 配置持久化，索引内容留在内存，应用重启后须再次刷新。
- 暂停会保留现有索引供查询并阻止手动刷新；暂停期间执行成功的文件修改会清空内存索引并更新 generation，恢复后需再次刷新。
- 文件路径操作的源和复制 / 移动目标必须在 configured root 内，并校验搜索结果 generation；当前 UI 的修改操作仅支持普通文件。复制、移动、重命名禁止静默覆盖，删除只能通过 Windows 回收站 API，非 Windows 明确失败。
- 最近文件只记录 Files 面板成功的原生打开动作，最多保留 100 条 SQLite 记录；它们不属于内存索引，读取时只返回当前 configured root 内仍存在的路径，不计入索引数量。任意用户目录图片预览使用有界 data URL，不扩大 Tauri asset scope。
- 下载队列不在启动时自动开始任务；任务元数据和目标目录 / 冲突策略写入 `downloads.json`，运行中的 HTTP worker 只在用户 enqueue / retry 后启动。暂停或失败保留 `.part` 临时文件，默认 `rename` 策略不静默覆盖已有文件。
- 下载完成后的打开、定位和复制路径只接受记录目标目录内仍存在的普通文件；持久化路径会在使用前重新 canonicalize，防止配置或旧记录把操作引向下载目录外。
- `.part` 续传必须验证 HTTP `206` 与 `Content-Range` 起点；下载元数据持久化失败时回滚内存状态，避免 UI 显示未落盘的进度或任务。
- 文件和下载的“复制路径”必须通过 native `copy_text_to_clipboard` 写入系统剪贴板；Launcher 文本上下文和结果复制优先使用 native clipboard bridge。WebView clipboard API 不能作为打包态唯一依赖。空文本、读取失败和系统剪贴板占用必须返回可理解错误。
- 录音只能由可见 Voice UI 显式开始；Windows native 由专用线程持有 `cpal::Stream` 与 WAV writer，Tauri 命令只通过通道发送控制消息，不能把平台 stream 放进全局状态或阻塞 WebView。native 会话状态负责录音线程和权限 / 设备错误，完成 WAV 在 Inbox 数据库提交成功后才标记 `saved`，失败不得伪造已保存状态。应用退出不承诺续录，但未完成 `.part` 不得静默删除。
- 录音固定输出 `48 kHz / mono / 16-bit PCM WAV`；先写同目录随机 `.wav.part`，停止时完成 writer 后再提升为 `.wav`。回调、写入、finalize、提升或 Inbox 归档失败必须进入 `failed` 并保留可诊断路径。
- 转写只由用户显式确认触发：native 命令不自动发送音频，确认状态不跨会话持久化；音频字节在 native 侧读取并 multipart 发往本机 Relay，不经过 WebView。EasyFlowHub 不存储、不读取 Relay 密钥，设置只保存 `transcription` 偏好（model / language）。
- 转写 Provider 必须经 `TranscriptionTransport` trait 接入，首个实现是本机 Relay；`POST /v1/audio/transcriptions` 请求上限 25 MB、截止 180 秒，仅接受 OpenAI 兼容 `text` 字段，空结果视为失败；错误分类使用固定中文文案，不回显原始 Provider 响应体。
- transcript 以 `inbox_attachments.kind = 'transcript'` 文本附件落盘（随机存储名 + 元数据行，模型记录在展示名中）；重新转写在同一事务内替换旧附件，不产生重复条目；`convert_inbox_item_to_note` 对 transcript 附件把文本嵌入笔记正文。
- Launcher 拖动期间不能因失焦隐藏，透明窗口空白点击也不能误关闭；用户可用关闭按钮、Esc 或重复触发收起。
- 用户显式唤起时必须激活原生窗口，通过 `launcher-opened` 重置搜索，通过 `launcher-focused` 恢复输入焦点。
- Launcher / quick note 的鼠标中心定位归原生壳所有；窗口尺寸按目标显示器 DPI 换算，再用物理工作区边界约束，禁止把物理位置传给逻辑坐标 builder。

## 8. Common Pitfalls

- 只改前端 wrapper 而忘记 Rust 注册
- 快捷键、防抖与窗口焦点行为相互影响
- 开发态与打包态 `scriptmgr` 路径解析差异
- 脚本 / 扩展 / 中转 / MCP 面板同时 "Failed to fetch" 时，先确认 sidecar 是否启动、8765 是否被其他程序占用，而不是逐个改面板
- 下载服务器不支持 Range 时，重试从头开始；网络和权限错误必须保留可读状态，不能阻塞其他窗口。
- Windows 特有 API 变动后没有回归桌面固定或 autostart
- 将 Snow Shot MCP 未启用误判为“未安装”，导致用户被送到下载页

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
