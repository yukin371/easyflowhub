# T-Gate 0：Tauri 版停掉那些 webview（默认接管，`--legacy-ui` 回退）

> 状态：`completed`（Tauri 侧已交付并门禁化；原生侧三件全部交付——常驻入口见 §7、单实例互斥见 [completed/2026-11-27-native-single-instance.md](../completed/2026-11-27-native-single-instance.md)、选区与翻译见 [completed/2026-11-27-selection-capture-gate.md](../completed/2026-11-27-selection-capture-gate.md) 与 [completed/2026-11-27-translation-migration.md](../completed/2026-11-27-translation-migration.md)。T-Gate 0 关闭）
>
> 上游判据：[原生重构总设计](../active/2026-11-24-native-runtime-redesign.md) §Phase 1 过渡期约束表 T-Gate 0——「Tauri 版必须停止创建 launcher / 快速笔记的 webview，只保留一个按需的 Manager 实例」
> 归属批次：Phase 1 收口（C2 通过、Launcher 产品化之后）
> 门禁：`python scripts/check-tauri-takeover.py --exe <release exe>`（16 项）+ 一次运行时实测
> 数字来源：本机单屏 3840×2160 @150%、Win11 26300、release 构建

## 0. 一句话

Tauri 版从"两个栈都在服务"改成"只有一个按需的 Manager"：启动不再声明任何窗口、不注册任何全局热键、前端那三条会建 webview 的命令明确拒绝。同一台机器上实测宿主进程 **18 MB、0 个 WebView2 子进程**，而 `Ctrl+Alt+Space` 落在原生壳上（`shown_ms=6.94`）。

## 1. 退出条件对照

| 表面 | 之前 | 现在 |
|---|---|---|
| Launcher | `tauri.conf.json` 预创建 webview | 不再声明；原生壳的 parked 面板 |
| 快速笔记 | `create_note_window` 建 webview | 命令拒绝并说明原因 |
| 待办卡片 | `create_todo_card_window` 建 webview | 同上 |
| 全局热键 | N / M / D / H / Launcher（settings） | 一个都不注册 |
| 托盘 | 打开管理中心 / 新建快速笔记 / 退出 | 打开管理中心 / 退出 |
| Manager | 按需创建、关闭销毁 | 不变（这就是本进程剩下的全部） |

## 2. 四个不显然的决定

**1. 默认接管，不是默认旧壳。** 一个默认"继续做旧事"的开关会让过渡永久化：两个栈都能用，没人知道是谁应的键，要省下的内存一分没省。回退是显式的——`--legacy-ui` 或 `EFH_LEGACY_UI=1`，flag 压过环境变量（`shell::decide` 有单测钉住这条优先级）。

**2. "停止创建"发生在配置层，不是 show/hide 层。** `tauri.conf.json` 的 `windows` 数组清空，回退模式才由 `launcher::ensure_window` 现建同一个窗口（440×860、transparent、无边框、skipTaskbar）。把窗口留在配置里靠隐藏，等于宣称"没创建"，而任务管理器里那个进程还在。

**3. 门禁读的那份计划，就是注册用的那份。** `shell::shortcut_plan` 是唯一来源：注册循环遍历它，`--shell-report` 也遍历它。若门禁读第二份清单，它证明的只是"两份清单一致"。`--shell-report` 在 `tauri::Builder` 之前返回，不碰单实例互斥、不拉 WebView、不抢焦点——所以它能在"用户那份正在跑"的机器上判断一个构建的行为。

**4. 连 `Ctrl+Alt+M` 也要让——这是运行时抓到的，不是想出来的。** 第一版把 Manager 的键留在 Tauri：结构门禁 15/15 全绿，真跑起来原生壳当场报 `hotkey registered=false Ctrl+Alt+M: RegisterHotKey failed (win32 error 1409)`。两个进程抢一个组合，谁赢取决于启动顺序。改成接管态零热键后，原生侧 `hotkeys registered=5 failed=0`。

## 3. 改在哪

| 位置 | 改动 |
|---|---|
| `src-tauri/src/shell.rs`（新，含 5 个单测） | `decide`/`current`（flag > env > 默认）、`shortcut_plan`、`tray_plan`、`declared_windows`、`run_shortcut`、`report`、`moved_to_native` |
| `src-tauri/src/lib.rs` | `--shell-report` 早退；托盘菜单按模式分叉；热键注册改成遍历计划；回退模式才 `ensure_window` |
| `src-tauri/tauri.conf.json` | `app.windows` 清空 |
| `src-tauri/src/launcher.rs` | 新增 `ensure_window`（回退路径）；`register_launcher_trigger` / `show_launcher_window` 在接管态明确拒绝 |
| `src-tauri/src/app_windows.rs` | 快速笔记 / 待办卡片两条创建命令在接管态拒绝（不是静默成功） |
| `scripts/check-tauri-takeover.py`（新） | 16 项，见下 |
| `scripts/measure-latency.ps1` | 加 `--legacy-ui`：它测的是旧基线，接管后那个构建没有主窗可等 |

## 4. 证据

**结构门禁 16/16**

| 判什么 | 结果 |
|---|---|
| 默认接管、来源是 `default` | `takeover=True source='default'` |
| 启动不声明任何窗口 | `declaredWindows=[]`（另有一条独立读 `tauri.conf.json` 的断言，防止报告自己撒谎） |
| Launcher / 快速笔记 / 待办卡片都归原生 | `launcher=native quickNote=native todoCard=native` |
| 一个全局键都不注册 | `shortcuts=[]`，四条命令逐条断言 absent |
| 托盘去掉新建快速笔记 | `['show_manager','quit']` |
| 回退是真回退 | `--legacy-ui` 下 `launcher=webview`、四条键全注册、托盘三项 |
| 环境变量优先级 | `EFH_LEGACY_UI=1` → 回退（source=env）；`=0` 不会把 shell 脚本的"关"翻成"开" |

**运行时实测（同一台机器，先停旧常驻、跑新构建、再用原生壳按键）**

| 断言 | 结果 |
|---|---|
| 新构建常驻，什么都没打开 | 宿主 `18 MB`，**WebView2 子进程 0 个**，`EasyFlowHub Launcher` 窗口不存在 |
| 原生壳接管全部热键 | `hotkeys registered=5 failed=0` |
| `Ctrl+Alt+Space` 落到原生面板 | `launcher_shown shown_ms=6.94 move_ms=6.71 focus_ms=0.23 focused=true` |
| 按键之后 Tauri 仍然 0 个 webview | `webview children after hotkey=0` |
| 对照：旧构建同机常驻 | 宿主 `58.9 MB` + WebView2 子进程（`tasklist`） |

`cargo test --lib` 100 项通过 / 1 ignored；`cargo check` 干净（`notes.rs` 那条 `updated_at` 未读告警是既有的）。**一次偶发**：`logging::tests::snapshot_limits_and_clamps` 在整套运行时失败、单跑与复跑均过——时序敏感，与本次改动无关，已记在这里而不是悄悄重跑掩盖。

## 5. 已知差异（都写出来，不留给用户发现）

- ~~**选区 → 翻译 / 替换选区下线**~~ —— **已交付**。原生壳现在有自己的捕获（`hub-win::selection`，45 ms 预算）与翻译（`hub-core::translation` 纯逻辑 + `app::translate` 网络 + 面板卡片），语义照 Tauri 版：同一份 `settings.json` 的 `target_language` / `model`、同一个 `/v1/chat/completions`、同一组状态码映射。与 Tauri 版的三处有意差异：原生没有 sidecar，因此多一个 `relay_url` 设置项（默认 `http://127.0.0.1:8787`）；序列号没动时**不**清理剪贴板（Tauri 版会 `empty()`，那会毁掉用户刚截的图）；`TLS` 用 `native-tls`（SChannel）而不是 `rustls`。详见 [completed/2026-11-27-selection-capture-gate.md](../completed/2026-11-27-selection-capture-gate.md) 与 [completed/2026-11-27-translation-migration.md](../completed/2026-11-27-translation-migration.md)。
- **Manager 里的"新建快速笔记"入口会报错**。按约定选"拒绝并提示"而不是"转发给原生壳"：原生壳还没有单实例机制，转发会多出一个常驻进程互抢热键。（互斥已经就位，这条可以在 T-Gate 1 里改成转发。）
- **两个托盘图标**。Tauri 的托盘留着（它是打开 Manager 的入口，因为接管后它没有热键），原生壳也有自己的托盘。Phase 2 Manager 迁进原生后删掉 Tauri 那个。
- **Tauri 侧没有全局热键**。Manager 只能从托盘图标或开始菜单打开。

## 6. 没做的（T-Gate 0 已关闭）

1. ~~**原生壳没有"常驻但不开笔记"的入口**~~ —— 已交付，见 §7。
2. ~~**原生壳没有单实例互斥**~~ —— 已交付，见 [completed/2026-11-27-native-single-instance.md](../completed/2026-11-27-native-single-instance.md)。它顺带修掉一个从没被按过的入口：用热键隐藏面板从来没有真正停窗（三个既有隐藏路径都在 UI 线程直接调用，只有热键那条排队）。
3. ~~**选区与翻译迁移**~~ —— **已交付**（见 §5 第一条）。门禁 18 + 31 项，过程中修掉两个只有原生壳才会有的缺陷：修饰键未松开时注入裸 `c` 覆盖选区、同线程粘贴导致粘进上一份剪贴板。
4. 之外：`--shell-report` 只证明构建行为，不证明"启动之后 0 个 webview"——后者靠本文第 4 节那次实测。把实测脚本化现在只差"脚本起第二个进程、看它转发后退出"，互斥已就位，留待 T-Gate 1。

## 7. 原生常驻入口 `--resident`：零窗口、满热键

登录自启要的不是"打开一个笔记"，而是"什么都没有，但什么都能叫出来"：托盘在、五个组合在、面板已经建好藏在屏外。`easyflowhub-native.exe --resident` 就是这个形状。

**改在哪**

| 位置 | 改动 |
|---|---|
| `app/src/args.rs` | `--resident`（隐含 `--note`，即同一个组合根）；`--resident` + `--note-id` 直接拒绝（退出码 64） |
| `app/src/hub.rs` | `Config.resident`；`Hub::new` 第一帧不 `spawn`，改为调 `settle()`；`hub_start` 记 `notes=0 resident=true` |
| `app/src/hub/boot.rs`（新，228 行） | 把"只做一次"的四件事从 `hub.rs` 搬出来：开库、解析键表、注册热键、装托盘。`hub.rs` 795 → 605 行，回到硬顶之下 |
| `app/src/launcher.rs` | 每次按键在热键线程打一行 `launcher_toggle action=show|hide`：`launcher_shown` 是 UI 线程写的，两者之间那段延迟本身是另一个问题 |
| `scripts/check-native-resident.py`（新） | 16 项 |

**三个不显然的决定**

1. **常驻不是一个新模式，是同一个进程少开一个窗口。** `--resident` 在 `args` 里就把 `note` 置真，`main` 只有一条 `hub::run` 路径。另起一个"常驻入口"来跑同一套控制器，迟早和笔记那条分叉——而它们要抢的是同一批热键。
2. **`settle()` 原样复用，不新写判据。** 它的语义本来就是"没有笔记了：有托盘是静止态，没托盘是用户够不着的进程，得结束"。启动时直接问它，就不用第二处判断"什么时候该退"。证据是 `hub_idle notes=0 tray=0x…` 与 `hub_exit` 计数为 0。
3. **`--resident --note-id x` 拒绝，不猜。** 一句话说"打开 x"又说"什么都不开"，唯一的静默解法总有一半是错的。退出码 64 + 一句原因，写在 `parse` 里而不是 `main` 里，这样单测就能钉住。

**证据（`check-native-resident.py` 16/16，连跑 2 次）**

| 判什么 | 结果 |
|---|---|
| 启动零窗口 | `hub_start notes=0 resident=true`，1.5 s 后笔记窗口 0 个 |
| 面板先于任何笔记存在 | `launcher_window parked=true`，`rect=(-30000,-30000,660,1290)` |
| 零笔记是静止态 | `hub_idle notes=0 tray=0x29081C`，进程仍活 |
| 三个组合都注册上 | launcher / new_note / close_all → F9/F10/F11 |
| 热键唤起面板 | `launcher_shown shown_ms=2.93 move_ms=… focused=true`，且不产生笔记 |
| 热键开笔记 | `note_spawn id=0 live=1`，窗口 1 个，PID 不变 |
| 关掉最后一个笔记仍在 | `hub_idle`（第二次）+ `tray=0x…`，窗口 0 个，进程活 |
| 之后还能用 | 第二次 `launcher_shown`，面板回到屏内 |
| 全程没把自己当该退的进程 | `hub_exit` 行数 = 0 |

**门禁顺手抓到的一件事**：面板落定（`park`）后 50 ms 内注入的组合，系统会整个丢掉——日志里连 `launcher_toggle` 都没有。用上一提交的二进制复测同样丢，所以它是注入与隐藏抢同一个前台切换的竞速，不是产品回归；真人下一次按键是秒级之后。`check-native-launcher.py` 的 `reveal()` 因此先等"已 parked"再等 250 ms 才按，`check-native-resident.py` 也在 Esc 后等落地。

**已做（上一批）**：原生壳有了单实例互斥。自启 + 手动启动不再产生第二个托盘和一整轮 `RegisterHotKey failed (1409)`——第二次启动转发后退出（0.09–0.12 s），面板已经开着时是 `refocus` 而不是切换，`--autostart` 的那一次则完全不出声。门禁 `check-native-single-instance.py` 17 项，含"第二个进程一个字节都没写"（证明 claim 先于日志打开）。详见 [completed/2026-11-27-native-single-instance.md](../completed/2026-11-27-native-single-instance.md)。

**还没做**：自启注册表尚未指向原生 exe。argv 侧（`--autostart` 蕴含 `--resident`、且转发不出声）已经就绪，切换本身属于 G1。
