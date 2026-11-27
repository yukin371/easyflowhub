# Launcher 产品化：面板是藏到屏外的窗口，不是画出来的一层

> 状态：`completed`（2026-11-27 落地；C2 通过后 launcher 从 spike 面板变成产品件）
>
> 上游判据：[Phase 1 快速笔记壳](../active/2026-11-25-phase1-quicknote-shell.md) §5.1（面板形态与窗口策略）、§5.8（热唤起预算 p95 ≤ 60 ms / p99 ≤ 100 ms）
> 前序：[C2 三种输入法人机验收](../active/2026-11-25-c2-ime-acceptance.md)（Phase 1 唯一的人工启动项，已通过）
> 门禁：`python scripts/check-native-launcher.py`（25 项，两个场景进程 + 一次 bench）
> 数字来源：本机单屏 3840×2160 @150%、Win11 26300、release 构建


C2（三种输入法的人工验收）通过，Phase 1 唯一的硬性人工启动项解除，launcher 从 spike 面板变成产品件。这批只做 §5.1 与 §5.8 写下的那一件事：**面板是预创建、跟随光标的窗口；按下去是移动它，不是创建它。**

## 面板长什么样

440×860 逻辑像素（本机 150% ⇒ 660×1290 物理），跟随光标所在显示器的**工作区**夹紧，宽度够宽时两列、不够一列（`cols` 由 `place` 算，日志每次都写出来）。内容分三段：查询框、「动作」卡片网格（5 个命令）、「最近笔记」列表（最多 5 条）。

## 落点

| 位置 | 改动 |
|---|---|
| `crates/hub-core/src/shortcuts.rs` | 第 12 个命令 `Launcher`（`id=launcher`，全局作用域，默认 `Ctrl+Alt+Space`）——默认键沿用 Tauri 版，肌肉记忆不迁移，换个新默认键只会像"这个键坏了" |
| `crates/hub-ui/src/launcher.rs`（新，714 行） | 面板 widget：`Item`/`Action`/`State`，只画和选，不碰 SQL、不碰窗口。8 个单测（匹配标题/关键词/正文、cursor 环绕、空结果 Enter 无动作、重开清空、`elide` 不切汉字…） |
| `app/src/launcher.rs`（新，754 行） | 窗口与动作层：`Handle`（热键线程那一半）、`Shell`（UI 线程那一半）、`Launcher`（合成一个字段）、`Wiring`（环境参数）、`--launcher-bench` 的驱动与报告 |
| `app/src/launcher/recent.rs`（新，178 行） | 行模型：`Recent`、`refresh_recent`、`items`、`first_line`、`short_time`。抽出去的原因是 `launcher.rs` 撞了 800 行硬顶，而不是"这些代码恰好放得下" |
| `app/src/hub.rs`（794 行） | `Request` 加三个变体、`Hub` 多一个 `launcher` 字段、每帧声明一次 viewport；`request_for` 挪进 `commands.rs` |
| `app/src/hub/commands.rs` | `launcher_action`（**先隐藏再执行**）、`open_manager_tab`（已开的窗口切 tab，不复制窗口）、`open_note` 的 `source=`（`manager`/`launcher`） |
| `scripts/check-native-launcher.py`（新） | Phase 1 门禁，25 项，见下 |

## 三个不显然的决定

**1. 关闭 = 藏到屏外，不是 `SW_HIDE`。** `ShowWindow(SW_HIDE)` 会停掉 `WM_PAINT` 投递，winit 收不到重绘事件，eframe 的循环随之停摆——藏起来的第九次唤起会显示第八次的画面。所以窗口保持"可见"但挪到 `-30000,-30000`（OS 夹到 -20000），门禁判断"已关闭"的判据也因此是**矩形在任何显示器之外**，不是 `IsWindowVisible`。

**2. 揭示在热键线程，隐藏在 UI 线程。** `GetCursorPos` → 命中缓存显示器 → 算矩形 → `SetWindowPos` → `SetForegroundWindow` 这四步全在热键线程（实测 5.9 ms），省下唤 UI 线程的 30–47 ms。但 `GetFocus` 只回答调用线程，所以在热键线程隐藏会"窗口走了、键盘还归它"——用户在看不到的地方打字。于是隐藏是一个请求：`Request::LauncherHidden { epoch }`，由 UI 线程 `park`（`SetFocus(null)` + 移出屏）。

**3. `epoch` 防交叉。** 按一下、UI 线程醒来前再按一下，隐藏和显示会交叉。隐藏带着发起时的 epoch，`handle.epoch() != epoch` 就丢掉（日志写 `skipped=superseded`）。没有它，面板会在刚显示之后被藏起来，用户看到的是"这个键没反应"。

## 门禁抓到的四个真缺陷

| 缺陷 | 现象 | 现在 |
|---|---|---|
| **空结果按 Enter 之后面板再也不接受输入** | egui 的 `TextEdit::singleline` 把 Enter 当作"提交完成"并交出键盘焦点；没有可执行项时没人接手，`take_focus` 也不会再置位。之后每个键都被吞掉——看起来像窗口卡死 | `key_action` 里 Enter 若没选出动作就 `state.take_focus = true`，当帧把焦点要回来。复现方式留在评测记录里：输入 `回收`→`zzz`→Enter→退格 5 次，日志里一行 `launcher_query` 都不再出现 |
| **点别处不消失** | 原判据读 egui 的 `viewport().focused`，而 eframe 只在下一次事件唤醒时更新它：用户已经走了，面板还挂着，最长可达数秒 | 改问 OS：`hub_win::window::foreground()`（`GetForegroundWindow`）≠ 面板就算失焦。"键盘去哪了"是 OS 事实，不是 egui 的猜测 |
| **全新机器上库打不开** | `Store::open` 不建父目录，SQLite 答 "unable to open database file"，shell 静默退回 memory-only：这一场写下的笔记关窗即失，界面上一个字都不说 | `Store::open` 建父目录（`settings::save` 早就这么做），加单测：空目录 → 能开、能重开、`integrity_ok` |
| **笔记预览把标题重复一遍** | 快速笔记的正文第一行就是标题，`first_line` 原样取第一行 ⇒ 行里标题下面还是标题 | `first_line` 跳过与标题相同的那一行；正文为空就只有标题，不再重复 |

顺带把搜索面对齐 Ctrl+K：`Item::note` 现在拿整篇正文进 `haystack`（原来只有标题 + 预览），"螺丝"能找到「买螺丝刀和膨胀管」。

## 门禁

`scripts/check-native-launcher.py`：两个进程 + 一次 bench。

| 场景 | 判什么 |
|---|---|
| A（一个进程走完全程） | 按键前窗口就存在且 parked → 热键显示、落在光标工作区内、**Windows 的实际矩形等于日志里 `at=+x+y size=WxH`**（DPI 类缺陷）→ 拿到前台 → 输入 `回收` 过滤到 1 行且像素确实变了 → `zzz` 空结果按 Enter 没有任何副作用 → `回收站` 回车开出工作台 `tab=trash` 且面板先消失 → Esc 隐藏且 `focus_after=0` → 点别的窗口触发 `reason=blur` → 最近的笔记回车 `note_open id=…` |
| B（另一个进程，空库） | `新建笔记` 回车真的多出一个窗口，且面板在笔记拿键盘之前就不见了 |
| bench | `--launcher-bench 20` / `40`：p95/p99、泄漏、refresh 单独报告 |

两件环境事实写进了脚本注释：Tauri 版常驻时 `RegisterHotKey` 会以 1409 拒绝 `Ctrl+Alt+Space`，所以门禁写临时设置把键挪到 `Ctrl+Alt+F9/F10/F11` 的第一个可用键，并**要求注册成功**（"静默什么都没测"的门禁比失败的门禁更坏）；另一个是打字时机——显示之后约 80 ms 查询框才拿到键盘，早于 `launcher_ready` 的按键会被 egui 吞掉，所以每次显示后的第一个按键都等这一行。

## 证据

| 门禁 | 结果 |
|---|---|
| `scripts/check-native-launcher.py` | **25/25，连跑 3 次全过** |
| 揭示延迟（产品路径，n=40） | p50 1.8–2.1 ms、p95 2.4–3.2 ms、p99 8.0–14.1 ms（预算 60 / 100） |
| 内容刷新（只报告） | p95 7.4–12.1 ms；单次 `launcher_shown` 里 move 3.0–7.0 ms、focus < 1 ms |
| 泄漏（20 轮 vs 40 轮） | private 19.2–20.3 MB vs 19.3–20.4 MB（差值 ≤1.1 MB）、handles 187–189 vs 187–189：固定预热，不随轮数长 |
| 回归 | `check-native-notes.py` 25/25、`manager` 19/19、`switcher` 22/22、`store-schema` 24/24 |
| `cargo test --release --workspace` | 263 项（本批 +2：`Store::open` 建父目录、`first_line` 不重复标题；`--launcher-bench` 隐含 `--note`、`Launcher` 的默认键随命令表一起进测试） |
| `scripts/check-native-architecture.mjs` | 通过（`hub.rs` 794、`launcher.rs` 754、`launcher/recent.rs` 178、`hub-ui/launcher.rs` 714） |
| `scripts/check-doc-budget.mjs` | 通过（91 个文件；本文档即由它逼出来的一次拆分——§8.18 原本写在 `phase1-quicknote-shell` 里，那份文件 106.4 KB 封顶） |

## 没做的

- **Tauri 版停掉自己的 launcher webview**：这是 T-Gate 0 的退出条件，但要在 Tauri 的 `tauri.conf.json` / 侧栏逻辑里动刀，与本批"原生能做多快"是两件事，另批。
- **reveal 时刷新内容**：现在 reveal 只重排内存里的行，`launcher_recent` 只在启动、spawn 后、关闭后重读。这样做是为了把数据库读挡在 60 ms 之外，代价是"别处新建的笔记要等一个动作才出现在面板里"——等 Tauri 侧也走同一套写入路径后再决定要不要加一个轻量失效信号。
- **面板的 IME**：查询框收的是 `WM_CHAR`，候选窗定位跟随主窗口，输入法组合期在面板上方偏左，属可接受但没打磨。
- **`Ctrl+Alt+Space` 的冲突提示**：被别的程序占用时日志里写 `registered=false … 1409`，托盘与设置界面还没把这件事写给用户看。
