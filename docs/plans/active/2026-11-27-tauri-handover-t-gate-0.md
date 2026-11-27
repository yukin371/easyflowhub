# T-Gate 0：Tauri 版停掉那些 webview（默认接管，`--legacy-ui` 回退）

> 状态：`active`（Tauri 侧已交付并门禁化；T-Gate 0 还剩两件：原生常驻入口、选区/翻译迁移）
>
> 上游判据：[原生重构总设计](./2026-11-24-native-runtime-redesign.md) §Phase 1 过渡期约束表 T-Gate 0——「Tauri 版必须停止创建 launcher / 快速笔记的 webview，只保留一个按需的 Manager 实例」
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

- **选区 → 翻译 / 替换选区下线**。Tauri launcher 会先抓前台应用的选中文字（Ctrl+C 往返 + 剪贴板恢复），面板顶部给翻译结果并可写回原窗口；原生动作面板没有这条链路。这是停掉 webview 的直接代价，下一批单独迁移（捕获、relay 调用、目标语言设置、结果面板、显式替换各是一块）。
- **Manager 里的"新建快速笔记"入口会报错**。按约定选"拒绝并提示"而不是"转发给原生壳"：原生壳还没有单实例机制，转发会多出一个常驻进程互抢热键。
- **两个托盘图标**。Tauri 的托盘留着（它是打开 Manager 的入口，因为接管后它没有热键），原生壳也有自己的托盘。Phase 2 Manager 迁进原生后删掉 Tauri 那个。
- **Tauri 侧没有全局热键**。Manager 只能从托盘图标或开始菜单打开。

## 6. 没做的（T-Gate 0 真正关闭还缺两件）

1. **原生壳没有"常驻但不开笔记"的入口**。`easyflowhub-native.exe` 无参跑的是 S1 spike 面板，产品壳在 `--note` 分支，而 `hub::new` 启动时无条件 `spawn` 一个笔记窗口。要让安装器/自启动拉起"只有托盘 + parked 面板"的壳，需要 `--resident`（不 spawn 首窗；`settle()` 已经能在 notes=0 且有托盘时留在原地）。
2. **选区与翻译迁移**（见上）。
3. 之外：`--shell-report` 只证明构建行为，不证明"启动之后 0 个 webview"——后者靠本文第 4 节那次实测。把实测脚本化需要先解决单实例互斥（第二个进程会转发 argv 并退出），留待 T-Gate 1。
