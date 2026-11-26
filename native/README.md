# native/ — Windows 原生运行时（S1 + S2 + S3 spike）

这是 [Native Runtime 重构](../docs/plans/2026-11-24-native-runtime-redesign.md) 的可运行切片：[S1 验收](../docs/plans/2026-11-25-native-shell-spike.md)（热唤起 p95 48.9 ms）、[S2 验收](../docs/plans/2026-11-25-richedit-island-spike.md)（键入 p95 4.95 ms）与 [S3 验收](../docs/plans/2026-11-25-notes-db-compat-spike.md)（直读直写 `notes.db`，39 项检查）的实体。**它不是产品代码**：它回答三个问题——单进程 + 预创建窗口，热唤起能不能压到 p95 ≤ 60 ms；系统文本控件贴在 egui 自绘面板里，键入能不能压到 p95 ≤ 16 ms；以及脱离 Tauri 之后，现有 `notes.db` 还读不读得动、写下去旧版本还认不认。

## 结构

```text
native/
├─ Cargo.toml            workspace（app + 四个 crate）
├─ app/                  组合根：事件循环、热键线程接线、笔记壳（`note/`）、自动保存（`save.rs`）、bench 驱动
├─ crates/hub-core/      纯逻辑：布局夹紧、列数、设置、岛几何、RFC 3339 时钟、笔记文本解析 + `store`（notes.db 读写，无平台依赖，有单测）
├─ crates/hub-win/       win32：热键、显示器缓存、DWM、进程探针、窗口放置
├─ crates/hub-ui/        egui：调色板、字体、面板绘制与预热
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

S3 的门禁是三层而不是一层，缺一层就不算"兼容"：**L1** native 读写真副本；**L2** 把结果交给 **Tauri 那份代码**（`cargo test --lib -- --ignored` 里的 `notes::tests::reads_and_writes_a_database_produced_by_the_native_shell`）；**L3** 独立的 `sqlite3` CLI（另一套 SQLite build）做 `integrity_check` 与逐行逐字段比对。只有 L1 过时文档写"仅自读通过"，不许折算成"兼容"。

文本岛的三个诊断示例（`cargo run -p hub-editor --example scroll_probe` / `--example undo_probe` / `--example esc_probe`）保留了"为什么必须拿焦点才滚动""RichEdit 怎么合并撤销单元""Esc 是按一次还是根本没被轮询"的实测结论，别再靠猜。

`scripts/esc_poke.rs` 是同一件事在**真实 app** 上的版本：它按 PID 找到窗口、向 RichEdit 子窗口投一次 `WM_KEYDOWN`，再看窗口有没有移出屏幕——用来区分"按键没送到"和"送到了但没人读"。`rustc -O -o esc_poke.exe scripts/esc_poke.rs` 即可，不参与 workspace（新增 crate 会被架构门禁判为未登记）。

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
| 整窗透明度只活在窗口生命周期内 | 旧版同样是组件 state，关掉即回 100% | Phase 2 随 `notes.db` 记住"每窗口一组不透明度"，与窗口尺寸记忆（parity C5）一起做 |
| GHOST 的实际 alpha 是 1/255，不是 0 | `LWA_ALPHA = 0` 的窗口不可命中（实测 `WindowFromPoint` 答背后的窗口），幽灵会点不回来 | 保留地板。曾试过"背景色键消除 + 内容 20%"（`LWA_COLORKEY \| LWA_ALPHA`），技术上成立但观感不对，已回退；结论见计划文档 §8.7 |
| 笔记只能按 id 重开，没有列表/切换器 | 工作台与 `Ctrl+K`（Q25–Q27）属 Phase 2；现在要重开一条笔记得先从库里拿 id | Phase 2 工作台批次；Q20–Q23 与按 id 重开已在 1d 接上 `notes.db` |

## 度量口径

`first_frame` 取**保守上界**：窗口在绘制前显示，下一帧开始时上一帧已完成 paint + swap，因此"下一帧开始 − 热键按下"一定不早于真实上屏；同时记录"本次绘制结束 − 热键按下"作为下界。门禁读上界。完整口径见 S1 文档 §4.4。

键入的 `type` 同样是**上界**：RichEdit 在 `WM_CHAR` 内部同步绘制，紧随其后的 `UpdateWindow` 实测 0 µs，所以门禁值 = `insert` + 一次强制 `InvalidateRect` + `UpdateWindow` 全量重绘（两段有重叠，会高估）。合成（`DwmFlush`）单列报告、**不进门禁**——它必然落在 `[paint, paint + 16.7 ms]`，算进去等于要求比一个刷新周期还快。完整口径（含"为什么作废了首轮 1.58 ms"）见 S2 文档 §4.4 与 §9。
