# native/ — Windows 原生运行时（S1 + S2 + S3 spike）

这是 [Native Runtime 重构](../docs/plans/2026-11-24-native-runtime-redesign.md) 的可运行切片：[S1 验收](../docs/plans/2026-11-25-native-shell-spike.md)（热唤起 p95 48.9 ms）、[S2 验收](../docs/plans/2026-11-25-richedit-island-spike.md)（键入 p95 4.95 ms）与 [S3 验收](../docs/plans/2026-11-25-notes-db-compat-spike.md)（直读直写 `notes.db`，39 项检查）的实体。**它不是产品代码**：它回答三个问题——单进程 + 预创建窗口，热唤起能不能压到 p95 ≤ 60 ms；系统文本控件贴在 egui 自绘面板里，键入能不能压到 p95 ≤ 16 ms；以及脱离 Tauri 之后，现有 `notes.db` 还读不读得动、写下去旧版本还认不认。

## 结构

```text
native/
├─ Cargo.toml            workspace（app + 四个 crate）
├─ app/                  组合根：事件循环、热键线程接线、笔记壳（`note/`）、工作台（`manager.rs`）、自动保存（`save.rs`）、bench 驱动
├─ crates/hub-core/      纯逻辑：布局夹紧、列数、设置、快捷键表（`shortcuts`）、岛几何、RFC 3339 时钟、笔记文本解析 + `store`（notes.db 读写，无平台依赖，有单测）
├─ crates/hub-win/       win32：热键、显示器缓存、DWM、进程探针、窗口放置
├─ crates/hub-ui/        egui：调色板、字体、笔记标题条（`quicknote`）、切换器（`switcher`）、工作台（`manager`）、悬浮提示（`hint`）
└─ crates/hub-editor/    win32：RICHEDIT50W 文本岛（子窗口）、测量与像素证据
```

依赖方向由 `scripts/check-native-architecture.mjs` 强制（`app → hub-ui/hub-win/hub-core/hub-editor`，`hub-ui → hub-core`，`hub-editor → hub-win/hub-core`，其余无内部边），并禁止 `hub-editor` 出现 `egui::`——文本岛只吃物理像素矩形、只吐文本，布局决策留在 app 侧。**持久化只在 `hub-core::store`**（设计 §5.7 规则 3）：UI 不持有连接、不拼 SQL，`rusqlite` 也只有这一个 crate 依赖。

规则表在 `scripts/lib/native-arch-guard.mjs`（`ALLOWED_EDGES` 管内部边，`BANNED_DEPS` 管"这一层不许碰哪些外部 crate"，源码扫描与 `Cargo.toml` 检查都从同一张表派生）。这套规则自己有测试：

```bash
node scripts/check-native-architecture.mjs                      # 真实树
node --test scripts/check-native-architecture.selfcheck.mjs      # 23 项：每条规则各打破一次
```

自检不是摆设：曾用 8 次"故意改坏规则"验证过它真的会红，其中两次第一轮没红，因此修掉了自检自身两个缺陷（拿被测常量自证、表形式依赖没覆盖）。改规则时请照做，否则改坏的规则不会被任何东西发现。

## 跑起来

```bash
cd native
cargo run --release -p app -- --print-monitors     # 打印缓存布局与缩放
cargo run --release -p app -- --probe              # 预热后占用（含窗口数/可见数）
cargo run --release -p app -- --bench 200          # 200 轮自动唤起 + 分布
cargo run --release -p app -- --bench 200 --scale 2.0   # 模拟 200%
cargo run --release -p app -- --editor             # 交互模式：面板里带 RichEdit 正文
cargo run --release -p app -- --note               # Phase 1 快速笔记壳：标题条 + 文本岛（正文自动保存）
cargo run --release -p app -- --note --note-id a673b810-9a1e-4d3c-9b2f-8f0d1c2b3a45   # 按 id 重开已有笔记
cargo run --release -p app -- --editor-bench 200   # 键入分布 + 像素/滚动/撤销证据
cargo run --release -p app                         # 交互模式：Ctrl+Shift+Space 开关面板

# S3：读真实库（只读），再在副本上写探针
cargo run --release -p app -- --store-bench
cargo run --release -p app -- --store-bench --store-root target/s3/root --store-write --store-artifacts target/s3/art
cargo run --release -p app -- --store-hold 400 --store-root target/s3/root   # 持锁探针（并发用）
```

`--note` 是 Phase 1 的产品窗口（`app/src/note.rs` + `app/src/note/island.rs`），不是测量件：它不注册热键、不参与 bench，日志里 `note_window` / `island_created` 的尺寸就是 parity 清单 Q1–Q4、Q6–Q11 的判定依据。spike 的 `--bench` / `--editor` 保持原样，T-Gate 0 之后整个 spike 删除。

正文会写进 `notes.db`：默认 `%APPDATA%\com.yukin.easyflowhub`，`--store-root <dir>` 可改（门禁与探针都指向副本）。写盘节奏全在 `app/src/save.rs`——900 ms 空闲防抖、2 s 扫描兜底（有编辑信号漏了也能落库）、关闭前 flush、失败退避 1 s。日志三条：`note_doc`（这条笔记的 id 与是否从库里加载）、`note_load`（按 id 找到没有、灌进去多少字）、`note_save`（`reason=autosave|close`、`created=true|false`、`skipped=empty|unchanged`）。

`--note` 的外观全部来自 `quick_note.appearance`（字号、行距、内边距、插入符宽度、七个颜色）：

```bash
cargo run --release -p app -- --note --settings my-note.json
```

```json
{ "quick_note": { "appearance": {
  "font_pt": 12.0,
  "line_spacing": 1.1,
  "padding_x": 14,
  "colors": { "accent": [120, 180, 255] }
} } }
```

没写的字段取默认值（那份默认值就是 Tauri 的 `--manager-*` token），所以一个只改 `accent` 的文件不会把别的颜色清零。`caret_width` 会写系统插入符宽度并在退出时恢复，原因见计划文档 §8.1。

`scripts/grab-window.py` 是这台机器上唯一能截图的路子：沙箱禁用 `Add-Type`，PowerShell 的 `System.Drawing` 一行流跑不起来，而"行首 1 px 的光标有没有被裁掉"没法靠描述判断。它用 `ctypes` + `gdi32!BitBlt` 直接抓窗口矩形并写 BMP，不依赖任何第三方包；`--screen` 抓整个虚拟桌面，`--title` 按标题子串选窗口。光标可见性就是用它连拍 8 帧、数竖线列数得出的（改前 0/8、改后 4/8）。

`--bench` 用 `PostThreadMessageW(WM_HOTKEY)` 触发**与真实按键相同的处理路径**（去抖 → 取光标 → 命中缓存布局 → 唤醒 → 绘制），所以 200 轮不需要人手连按。退出码：`0` 达标，`2` 超预算，`3` 热键注册失败。

`--editor-bench` 在被停靠（隐藏）的窗口上跑完整证据：类名/字符格式回读、`WM_PRINTCLIENT` 像素采样、滚动、resize 跟随、撤销/重做、按键归属，最后是两组键入分布（1k / 10k 文档）。它**不需要可见窗口，也不截屏**，因此可以在无人值守的机器上跑。退出码：`0` 达标，`2` 超预算。

`--store-bench` 不带 `--store-write` 时是只读的；带 `--store-write` 时会**拒绝**指向真实 app data 目录（`%APPDATA%\com.yukin.easyflowhub`），必须先复制一份。探针行 id 固定为 `s3-probe-*`，每次运行先清掉自己上次留下的行，所以计数差值在重复运行时不变。退出码：`0` 自检通过，`2` 自检失败。

门禁侧：`scripts/measure-native-latency.ps1`（S1）、`scripts/measure-native-editor.ps1`（S2）、`scripts/measure-native-store.ps1`（S3）各自解析输出、判预算 + 结构证据、归档到 `docs/releases/native-s1|s2|s3/`；`run-smoke.ps1 -NativeBench` 会在 smoke 里跑快速版并写到临时目录，避免污染归档。

Phase 2 的笔记门禁是 `scripts/check-native-notes.py`（parity Q20–Q23 + 按 id 重开）：一条命令出 `RESULT: PASS/FAIL`，退出码 0/1。它把真实库**复制**到 `native/target/p2-gate/root/` 再驱动二进制，所以不碰 `%APPDATA%`；每个场景单独起进程，因为这台机器上跑着 shipped 版，它持有 `Ctrl+Alt+N`（原生壳 `RegisterHotKey` 得 1409）。两条来自实测的约束写在脚本里：`WM_CHAR 0x0D` 不产生换行（要 `WM_KEYDOWN VK_RETURN` + `WM_CHAR` + `WM_KEYUP` 三连）；跨进程读不到 RichEdit 文本（`GetWindowTextW` 回空串，`SetWindowTextW` 返回成功却什么都不改），所以"笔记里是什么"一律用数据库往返回答。

其余门禁大多走**真按键/真点击**，因为面板、工作台和标题条上的按钮长在 egui 里，投消息到不了那儿：

```bash
python scripts/check-native-switcher.py     # Q25 同键开关、Q13 Esc 只关面板、Q26 正文/标签命中、Q27 选中与新建（22 项）
python scripts/check-native-manager.py      # Q16 工作台开/数/关、K1 配置文件的键生效、K2 被换掉的键失效（19 项）
python scripts/check-native-tools.py        # Q7 工具面板：按钮开、按钮关、点正文关，岛随之让位与复位（8 项）
python scripts/check-native-pin.py          # Q9 置顶：OS 的 topmost 位 + 图标实心/空心两态，窗口不消失（11 项）
python scripts/check-native-drag.py         # Q10 拖动：左/中/右槽位移等于光标行程，按钮上起拖不动（9 项）
python scripts/check-native-recover.py      # Q24 数据恢复：卡片出现、模态、三个按钮各自的结果、陈旧草稿静默删（16 项）
python scripts/check-native-size.py         # C5 尺寸记忆：拖完写盘、schema 补号、别的键不丢、重启后新笔记照用（11 项）
python scripts/check-native-edge.py         # Q3 边缘 resize：四条边各改各的、下限 150×60、光标形状、认领按下（23 项）
```

`check-native-size.py` 是例外：它先试真拖窗口边缘，而这条路在 2k 之前是不通的（报告记 `drag=real|programmatic`，原因见"已知捷径"里无边框窗口那条）。Q3 之后边缘由应用自己画（`app/src/note/edge.rs`），`check-native-edge.py` 直接用 `SendInput` 拖边，不再需要 `SetWindowPos` 兜底。

`scripts/lib/wininput.py` 是共用注入层，脚本头部记了这条链上的四条实测：`hub-editor` 用 `GetKeyState` 读修饰键，所以投消息只会得到"一个没修饰的字母"；egui 的键与文本来自 winit，所以字符必须作为输入到达；`INPUT` 在 x64 上必须是 40 字节（写错 `SendInput` 返回 0 + 87，且静默不投递）；**合成点击在 RichEdit 上释放会留下鼠标捕获状态**，之后父窗口收到的光标坐标按子窗口原点解释（client (300,176) 报成 (182,31)），所以 `check-native-tools.py` 把"点正文"放在最后一步——真实鼠标的释放会清掉这个状态，用户看不到。第五条也来自失败：**两次合成点击若落在同一个 4 px 双击框内、间隔小于 `GetDoubleClickTime`（本机 500 ms），会被系统配成双击**，egui 的 `clicked()` 只对前半次为真（实测 0.4 s 连点 8 次丢了 3 次），`check-native-pin.py` 因此把间隔压到 ≥0.6 s 并左右交替偏 8 px。工作台门禁还把 `open_manager` 改成 `Ctrl+Alt+J` 再用它开窗——本机 `Ctrl+Alt+M` 属于 shipped 版，而"文件里的键真的生效"正是这一批要证的事。

要判"画出来的是什么"时用 `scripts/lib/winshot.py`：`PrintWindow(PW_RENDERFULLCONTENT)` 取客户区像素。不用屏幕 `BitBlt`——窗口被盖住时它读到的是桌面，而"窗口还在不在"恰恰是 `check-native-pin.py` 要回答的问题。

S3 的门禁是三层而不是一层，缺一层就不算"兼容"：**L1** native 读写真副本；**L2** 把结果交给 **Tauri 那份代码**（`cargo test --lib -- --ignored` 里的 `notes::tests::reads_and_writes_a_database_produced_by_the_native_shell`）；**L3** 独立的 `sqlite3` CLI（另一套 SQLite build）做 `integrity_check` 与逐行逐字段比对。只有 L1 过时文档写"仅自读通过"，不许折算成"兼容"。

文本岛的三个诊断示例（`cargo run -p hub-editor --example scroll_probe` / `--example undo_probe` / `--example esc_probe`）保留了"为什么必须拿焦点才滚动""RichEdit 怎么合并撤销单元""Esc 是按一次还是根本没被轮询"的实测结论，别再靠猜。

`scripts/esc_poke.rs` 是同一件事在**真实 app** 上的版本：它按 PID 找到窗口、向 RichEdit 子窗口投一次 `WM_KEYDOWN`，再看窗口有没有移出屏幕——用来区分"按键没送到"和"送到了但没人读"。`rustc -O -o esc_poke.exe scripts/esc_poke.rs` 即可，不参与 workspace（新增 crate 会被架构门禁判为未登记）。

## 快捷键与工作台

开着的笔记里按 `Ctrl+K` 打开切换器（同键再按关闭，`Esc` 只关面板），`Ctrl+Alt+M` 打开工作台（笔记列表 / 回收站 / 待办 / 设置四个分区）。**这些键都不是硬编码**：一张表在 `hub-core::shortcuts`，三处消费同一份数据——按键解析、悬浮提示、设置页的改键。

| 作用域 | 含义 | 命令 |
|---|---|---|
| `Global` | OS 注册（`RegisterHotKey`），焦点在任何程序都能按 | 新建笔记、关闭全部、最小化/恢复全部、打开工作台 |
| `Window` | 窗口内解析（岛的子类 proc 与 egui 两条路径） | 切换器、关闭笔记、置顶、工具面板、幽灵、撤销/重做 |

改键写在设置文件的 `shortcuts` 里，**只存改过的**，所以将来新增命令自带默认值：

```json
{ "shortcuts": { "switcher": "Ctrl+Shift+K", "open_manager": "Ctrl+Alt+J" } }
```

命令行键名就是表里的 `Command::id()`（`new_note`、`close_all`、`toggle_visible`、`open_manager`、`switcher`、`close_note`、`toggle_pin`、`toggle_tools`、`toggle_ghost`、`undo`、`redo`），键位写法见 `Chord::display()`（`Ctrl+Alt+J`、`Ctrl+Shift+Z`、`F9`）。启动时每条都记一行 `shortcut scope=… command=… key=…`，被拒的记 `shortcut_rejected`。

几条口径：

- **`Esc` 不可改**。它是"关闭"的语义本身（Q12/Q13），不是一条可重绑的偏好；窗口作用域下任意修饰键都触发它。
- **组合键必须有 `Ctrl`/`Alt`/`Shift`**，例外只有 `Esc` 与 `F1`–`F24`（它们不是字符键）。
- **冲突不拒绝启动**：`Command::ALL` 的顺序就是优先级，先者胜，败者记 `shadowed` 并跳过注册——`RegisterHotKey` 已经被别的程序占用（本机的 shipped 版占着 `Ctrl+Alt+N/D/H/M`）时也一样，只是记 `hotkey registered=false … 1409`。
- **改键立刻对所有已开窗口生效**：表是 `Arc<RwLock<Table>>`，不是 spawn 时的拷贝。全局键重注册前要先关掉旧的 `HotkeySet` 并等 60 ms，否则新组合会被 1409 顶掉。
- **每个可点且有键位的按钮都悬浮显示"功能名 + 快捷键"**，文案取自同一张表（`hub_ui::hint`）。不可改键的手势（`Ctrl+滚轮`）单独走一行提示，故意不与表混在一起。托盘提示反向处理：它只设置一次，所以**不印键位**——改键后那句话就成了无法修正的假话。

## 为什么这么切

- **热键在独立线程**：`RegisterHotKey` 绑定线程消息队列，放在 UI 线程会让每次唤起与绘制抢线程；回调里只做「读光标 + 查缓存 + 入队」。
- **显示器布局启动期缓存**：热键路径**不枚举**显示器（混合 DPI 下这是几十毫秒级的坑，见设计 §5.1.1）。
- **窗口预创建、隐藏态预热三帧**：设备上下文、图集、首帧纹理都在启动期付掉；隐藏后不再 `request_repaint`，空闲 CPU ≈ 0。
- **定位用 win32 物理像素**：egui 用逻辑点，换算一次就多一次舍入机会；面板缓冲按物理像素分配（200% 下 1.5 Mpx）。
- **正文是子 HWND，不是 egui 自绘**：IME、撤销、选区、光标、无障碍都是系统的事，重写它们正是"快的编辑器变慢"的地方。父窗口补 `WS_CLIPCHILDREN`，否则每次 `swap_buffers` 都会把子控件盖掉一帧（表现为"打字时闪"）。
- **文本岛创建后立刻 `SetFocus`**：`EM_SCROLLCARET` 在没有焦点时是空操作，不拿焦点就会出现"输入超过一屏后光标不跟随"。

## 已知捷径（Phase 1 必须还掉）

| 捷径 | 原因 | 还法 |
|---|---|---|
| `FindWindowW` 按标题取 `HWND` | eframe 0.31 不从 `App` 暴露窗口句柄 | Phase 1 从 winit 拿 `RawWindowHandle` 传下来，删掉这个函数 |
| `--scale` 模拟多档 DPI | 本机只有一块 150% 屏 | Phase 1 用真实多屏/改缩放复核，不只跑模拟 |
| 首帧不是"直接 blit 上一帧纹理" | eframe 不暴露目标帧缓冲 | 实测量不足 60 ms 时改 `egui_glow` 手动循环接管 swap + blit |
| 面板内容是静态样例 | 只为预热真实字符集与布局，不代表最终 UI | Phase 2 换成真实动作/最近项 |
| 无中键钩子 | 不在 60 ms 路径上，属 T0 parity | Phase 1 与功能一起落地（托盘已在 1b.3 落地，见计划文档 §8.6） |
| `store` 没有 schema 版本号，只有与 Tauri 同文的幂等列探测 | 现库 `user_version = 0`，单方面打版本号会让两套迁移体系分叉 | Phase 2 统一迁移器与版本号，原生与 Tauri 共用一份 |
| `store` 只覆盖 notes / settings / 附件的写入，不含 Inbox 完整 capture 路径 | S3 只回答"文件还读不读得动"，capture 是产品逻辑 | Phase 2 随 Inbox 迁移一起搬 |
| `store` 保持 `journal_mode = delete`，不切 WAL | 切 WAL 会改文件头并留 `-wal`/`-shm`，影响备份/同步/"复制 db"的语义，属产品级决定 | 若要切，先写 ADR 并复验备份链路；并发已有 `IMMEDIATE` + 5 s `busy_timeout` 兜底 |
| 文本岛无 Markdown 装饰、无图片粘贴 | 属 P3；装饰画在文本控件之上，不改变 S2 的结论 | P3 |
| 输入法只验到 `ImmGetContext` 非空 | 三种输入法各 30 分钟需要人 | RC 前人工验收，结果回填 S2 文档 §9 |
| 整窗透明度只活在窗口生命周期内 | 旧版同样不持久化（`QuickNotePage` 的 `opacity` 是 `useState`），关掉即回初值 | 原生侧只有"新笔记初值"（`quick_note.appearance.window_alpha`）。**每窗口透明度裁定不做**：调透明度是临时动作，收益低；C5 也只记全局一组宽高，不为窗口各造一行键 |
| GHOST 的实际 alpha 是 1/255，不是 0 | `LWA_ALPHA = 0` 的窗口不可命中（实测 `WindowFromPoint` 答背后的窗口），幽灵会点不回来 | 保留地板。曾试过"背景色键消除 + 内容 20%"（`LWA_COLORKEY \| LWA_ALPHA`），技术上成立但观感不对，已回退；结论见计划文档 §8.7 |
| 工作台里的按钮没有端到端点按门禁 | 点一下需要在 egui 布局里猜坐标，布局一改门禁就红，且红的原因与行为无关 | 写路径由 `hub-core` 的 store 单测（置顶/移入回收站/恢复/彻底删除都不动正文）与 `hub-ui::manager` 的 `Action` 单测覆盖；窗口行为（开、计数、`Esc` 关）由 `check-native-manager.py` 判。标题条上那两个按钮**有**点击门禁（`check-native-tools.py`），因为它们的坏法（拖拽面盖住按钮）只有真点击能发现 |
| 拖动自己搬窗口，不用 `ViewportCommand::StartDrag` | 那条命令把活交给 OS 的模态移动循环，而本机这套注入环境下它根本不接管：`GetWindowRect` 不动，连普通 `WS_OVERLAPPEDWINDOW` 也不动（2026-11-26 实测三种投法） | `note/drag.rs` 每帧按 `GetCursorPos` 的位移 `SetWindowPos`；光标必须用屏幕坐标，egui 的客户区坐标会跟着窗口走、算出的位移恒为 0 |
| 边缘 resize 自己画，系统那条边框不存在 | 无边框窗口 `WM_NCHITTEST` 对每条边都回 `HTCLIENT`（`style=0x16cf0000` 有 `WS_THICKFRAME` 也没用，2026-11-26 实测 inset 1..12 全是 1），OS 的 resize 循环没有入口 | `note/edge.rs`：band 5 逻辑像素、没按住的边不动、下限自己夹（OS 只拒尺寸不拒移动，不夹就会边滑边不变窄）；按下由 `hub-editor` 的 `on_press` 认领，否则同一按会同时放光标和框选文本 |
| 草稿备份是文件，不是 `localStorage` | 原生窗口没有 origin，也读不到 WebView 的 profile；两边**互相恢复不了对方的草稿** | `{store_root}/autosave/{id}.json`，字段与现版 `BackupDraft` 逐个相同（`title`/`content`/`tags`/`timestamp`），将来搬进表时列名已经对齐 |
| 工具面板的"点外部"只覆盖正文 | 标题条按住是拖窗（Q10），把 strip 也算"外部点击"会和拖动抢手势 | `•••`、`Ctrl+.` 与正文点击三个入口都能关；header 区的点击归拖动。旧版是页面上任意 `click`，那在 React 里没有"标题栏拖动"这个冲突 |
| 恢复卡片是手绘矩形 | egui 的自动布局不给按钮坐标，而门禁必须点中按钮 | `hub-ui/recover` 返回三个 `Rect`，`note/recover` 以物理像素写进 `recover_rect`，门禁读日志再点——按钮画到哪里就点哪里，padding 改了也不用改脚本 |
| 置顶用 `SetWindowPos`，不用 `ViewportCommand::WindowLevel` | winit 改 level 时按自己缓存的 `WindowFlags` 重算整套 `WS_*`，而本窗口是 `with_visible(false)` 建、`move_on_screen` 显示的；第一次改就把 `WS_VISIBLE` 抹掉，窗口从桌面消失（实测 2026-11-26，`IsWindowVisible` false） | `hub_win::window::set_topmost` 只动 z-order 位（`SWP_NOACTIVATE`）。**同样是这条约束**：任何让 winit 重算样式的 viewport 命令（decorations、visible）都会隐藏笔记窗口，要加这类命令先读 §8.11 |
| 改键的"写盘"方向没有端到端门禁 | 需要点设置页里的键位再按新键，同上一条的坐标问题 | 反向（文件 → 表 → OS 注册 → 真按键生效）由 `check-native-manager.py` 的 K1/K2 判；写盘一侧由 `SpikeSettings::save`（tmp + rename）与 `Table::rebind` 的单测钉住 |
| 工作台只做了笔记 / 回收站 / 待办 / 设置四个分区 | 收件箱与脚本分区依赖 capture 与执行链路，属 P3 | 随 P3 迁移一起补；待办分区只读正文里的 `- [ ]` 行，不写回 |
| `store` 的 schema 版本号仍未打 | 现库与 Tauri 同文、`user_version = 0`；单方面打版本号会让两套迁移分叉 | 与 Tauri 共用一份迁移器时一起定，见上一处说明 |

## 度量口径

`first_frame` 取**保守上界**：窗口在绘制前显示，下一帧开始时上一帧已完成 paint + swap，因此"下一帧开始 − 热键按下"一定不早于真实上屏；同时记录"本次绘制结束 − 热键按下"作为下界。门禁读上界。完整口径见 S1 文档 §4.4。

键入的 `type` 同样是**上界**：RichEdit 在 `WM_CHAR` 内部同步绘制，紧随其后的 `UpdateWindow` 实测 0 µs，所以门禁值 = `insert` + 一次强制 `InvalidateRect` + `UpdateWindow` 全量重绘（两段有重叠，会高估）。合成（`DwmFlush`）单列报告、**不进门禁**——它必然落在 `[paint, paint + 16.7 ms]`，算进去等于要求比一个刷新周期还快。完整口径（含"为什么作废了首轮 1.58 ms"）见 S2 文档 §4.4 与 §9。
