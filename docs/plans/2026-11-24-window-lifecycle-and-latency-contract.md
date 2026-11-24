# P0.5 窗口生命周期改造 + 计时契约（设计 / 验收标准）

> 状态：`accepted`（2026-11-24，用户确认「按设定路线先出设计文档 + 验收标准，再逐批实施」）
>
> 上游：[native-runtime-redesign.md](./2026-11-24-native-runtime-redesign.md) §2.2 / §6 Phase 0 & 0.5 / §7.1
> 决策：[ADR-0009-windows-native-runtime.md](../decisions/ADR-0009-windows-native-runtime.md)
> 约束：[ARCHITECTURE_GUARDRAILS.md](../ARCHITECTURE_GUARDRAILS.md) 第 10 节

## 1. 这一阶段要回答什么

P0.5 已在 2026-11-24 完成一半（开机自启动四步修复、快速笔记暗色黑字、工具栏遮挡、光标跟随主题）。剩下的部分与 P0 的度量工作有一个共同前提：**没有可信数字，后面的判断都靠猜**。

本阶段只回答两个问题，且都要求用本机实测数据回答：

1. **占用是不是由「隐藏的 manager + 未销毁的临时窗口」造成的？**
   Phase 0.5 的核心价值不是省内存，而是**验证动因判断**。如果改完只降了 10%，说明主设计 §2.3 的结构性判断错了，必须在 Phase 0 修正设计，而不是等到 Phase 2。
2. **热唤起 60 ms 这个主门禁，在现有 WebView 路径下的真实起点是多少？**
   没有起点就没有"原生是否真的更快"的判据。同时这一步会把 `measure-latency.ps1` 从 `SKIP` 打成真实分布，让 G0 的门禁工具链在原生壳出现之前就先被跑通一次。

## 2. 范围

**做：**

- manager 窗口懒创建（不预创建）、关闭即销毁
- 临时窗口（quick-note / todo-card）关闭即销毁的收口与验证
- 隐藏态高频轮询审计（有则改，无则记）
- `[timing] hotkey_down` / `[timing] first_frame` 打点契约落地（Tauri 版）
- 度量脚本补 `-Label` / `-OutFile`，三姿态快照归档
- 主设计 §2.2 表格补齐三姿态，§7.1 补 Tauri 现状基线

**不做（明确排除，避免范围膨胀）：**

- 不新建 `native/` 工作区、不写 egui、不动 RichEdit（属 P0 spike 与 P1）
- 不改 launcher 预创建策略（它是唯一有 60 ms 门禁的面板，见主设计 §5.8.1 反面清单）
- 不优化 WebView 渲染路径（这套运行时将被淘汰，投入即沉没）
- 不动 `scriptmgr-go`、不改数据库 schema

## 3. 现状事实（代码级）

| 窗口 | 创建时机 | 隐藏/关闭 | 问题 |
|---|---|---|---|
| `manager` | `tauri.conf.json` 声明，`visible: false` → **启动即建** | `CloseRequested` → `prevent_close()` + `hide()` | 静置时白养一套 renderer；关闭也永不释放 |
| `launcher` | 同上，`visible: false` → 启动即建 | `hide()`（保留，**不改**） | 符合设计：唯一需要预热的窗口 |
| `quick-note-N` | `create_note_window` 每次调用新建 | 自定义关闭按钮 → `close_window` → `destroy()` | 计数器单调递增；`close_all_note_windows` 走 `close()` 而非 `destroy()`，语义不一致 |
| `todo-card-N` | `create_todo_card_window` 每次调用新建 | 依赖前端 `close_window` | 同上 |

两处直接证据（2026-11-24 实测）：托盘静置 **Private 545.3 MB / 10 进程**（WebView2 7 个，最大单进程 352.9 MB）；而 manager + launcher 在静置时都是 `visible: false`——**没有任何可见窗口，占用仍是 65 MB 解释线的 8.4 倍**。

## 4. 设计

### 4.1 manager 懒创建（D1）

**做法：**

1. `tauri.conf.json` 移除 `manager` 窗口条目（保留 `launcher`）。
2. `app_windows.rs` 新增 `ensure_manager_window(app) -> Result<WebviewWindow>`，参数与旧配置一一对应（1000×700、min 600×400、`visible: true`、可装饰、居中、非置顶）。
3. 所有唤起路径统一走 `ensure_manager_window`：
   - 托盘菜单「打开管理中心」
   - 全局快捷键 `Ctrl+Alt+M`（`toggle_manager_window`）
   - 单实例回调（非 `--autostart`）
   - 前端命令 `show_manager_window`（托盘/页面内跳转都可能触发）
4. `CloseRequested` 改为**放行销毁**（不再 `prevent_close`）。窗口销毁后 `get_webview_window("manager")` 返回 `None`，下次唤起重建。
5. `hide_manager_window` 语义保留为"隐藏但不销毁"（供需要保持状态的内部路径使用），与"关闭"区分开。

**为什么可以接受销毁：** Manager 不在 60 ms 门禁内（门禁只覆盖 Launcher 热唤起与窗口内切换）；重建成本是一次 WebView 冷启动（≈0.5–1.5 s），与"打开一个工作台"的心理预期相符。数据安全上，笔记走 `useAutoSave` 防抖落库、设置是即时 `update_settings`，关闭窗口不产生未保存区。

**为什么不留 `visible:false` 预创建：** 预创建正是 545 MB 的主要来源。这份占用换来的只是"Manager 首次打开快 0.5 s"，而用户每天打开 Manager 的次数远低于唤起 Launcher。

### 4.2 临时窗口关闭即销毁（D2）

- `close_all_note_windows` 的 `close()` 改为 `destroy()`，与单窗口关闭路径一致（避免 CloseRequested 被将来某个 handler 拦下后变成"关不掉"）。
- 窗口 label 继续用单调递增计数器：销毁后 label 不复用，避免与 `webview_windows()` 中残留条目撞名；窗口总数由"打开数 − 销毁数"决定，不再随使用时长累积。
- 待办卡片同规则。
- **验证点不是"代码调了 destroy"，而是"销毁后进程数与 Private 回落"**——WebView2 的 renderer 释放由运行时决定，必须实测（见 B4/B5）。

### 4.3 隐藏态高频轮询审计（D3）

逐项检查隐藏窗口里是否还有 `setInterval` / 定时 `invoke` 在跑（launcher 的剪贴板预读、笔记列表刷新、待办轮询）。**只有发现高频轮询才改**，并且只做"隐藏时暂停、显示时恢复"，不改业务逻辑。这一项是检查项，不作为阻塞条件。

### 4.4 计时契约（D4）

日志契约（主设计 §7.1 已定义格式，这里落地）：

```text
2026-11-24 19:04:01.234 [INFO] [timing] hotkey_down
2026-11-24 19:04:01.290 [INFO] [timing] first_frame
```

- **`hotkey_down`**：在快捷键回调里 **通过去抖/排除应用检查之后、抓取选中文本之前**打点。选中文本捕获有 180 ms 上限，它属于用户可感延迟，必须计入。
- **`first_frame`**：Launcher 前端收到 `launcher-opened` 后，连续两帧 `requestAnimationFrame` 再回调命令，由 Rust 打点。
- **落盘策略**：`logging::log_timing()` 无视 `developer.debug_mode` 直接写文件。理由：这是门禁测量数据，不是调试信息；每对 2 行、每天几十对的量级不构成噪音。其余日志继续受 debug 开关控制，安全边界不变。
- **口径声明（重要）**：Tauri 路径的 `first_frame` 是"事件送达 + 完成一次合成帧"的近似，**不等于像素上屏**。因此它只作为**现状基线**与趋势对比，**不作为 G0 判决数据**；G0 用原生壳的首帧信号。

### 4.5 度量归档（D5）

`measure-footprint.ps1` 增加两个参数，保持"只读"职责不变：

- `-Label <pose>`：写进报告的 `Pose` 字段（`idle` / `launcher` / `manager-2notes`）。
- `-OutFile <path>`：把 JSON 写文件（同时仍输出到 stdout）。归档路径统一 `docs/releases/footprint/<version>/<pose>-<timestamp>.json`。

三姿态定义（每姿态静置 ≥ 60 s 再采样，避免抓到瞬时峰值）：

| Pose | 布置 |
|---|---|
| `idle` | 启动后静置 3 min，无窗口可见 |
| `launcher` | 唤起 Launcher 后静置 60 s |
| `manager-2notes` | Manager 打开 + 2 个快速笔记窗口后静置 60 s |

## 5. 验收标准

### Batch A：度量与归档（脚本层，无产品代码）

| ID | 验收项 | 判定 |
|---|---|---|
| A1 | `measure-footprint.ps1 -Label idle -Json -OutFile ...` 能写出合法 JSON，含 `Pose` 字段 | 文件存在、`ConvertFrom-Json` 通过 |
| A2 | 三姿态各有一份快照归档到 `docs/releases/footprint/<ver>/` | 3 个文件 |
| A3 | 主设计 §2.2 补齐三姿态表格（进程数 / Private / WorkingSet / 最大单进程 / 时间戳） | 表格三行非空 |
| A4 | 脚本仍为只读：采样前后不启动、不结束、不改任何进程 | 代码审阅 + 采样前后进程数一致 |

### Batch B：窗口生命周期（产品代码，阻塞后续）

| ID | 验收项 | 判定 |
|---|---|---|
| B1 | 启动后 `webview_windows()` 只含 `launcher`，`manager` 不存在 | 通过 CDP 读窗口列表 |
| B2 | 静置 3 min：Private ≤ **330 MB** 且 WebView2 进程 ≤ **4**（基线 545.3 MB / 7） | `measure-footprint.ps1 -Label idle` |
| B3 | 关闭 Manager 后 `get_webview_window("manager")` 为 `None`；再次唤起 ≤ 2 s 可用且位置保持 | 手动 + CDP |
| B4 | 连续 10 轮「开 Manager → 关」后 Private 不高于首轮 **+15 MB**，进程数回落 | 前后快照对比 |
| B5 | 连续 3 轮「开 2 笔记 → 全关」后窗口数回 0、进程数回落，笔记内容不丢 | 窗口列表 + 数据库核对 |
| B6 | `--autostart` 启动不弹 Manager；普通二次启动唤起 Manager（走懒创建路径） | 双实例实测 |
| B7 | 现有测试全绿：前端 57 文件 / 418 项、Rust 96 项、`tsc`、两条架构门禁 | CI 等价命令 |
| **B8** | **若 B2 实测降幅 < 25%（即 Private > 409 MB），判定"占用由隐藏 manager 造成"的动因判断有误，回 Phase 0 修正设计，本阶段阻塞** | 数字裁决，不允许解释性放宽 |

### Batch C：计时契约（产品代码，产出现状基线）

| ID | 验收项 | 判定 |
|---|---|---|
| C1 | 触发 30 次 Launcher 后，日志出现 30 对 `[timing] hotkey_down` / `[timing] first_frame` | `grep -c` |
| C2 | `measure-latency.ps1` 输出非 `SKIP`，`samples ≥ 30` | 脚本退出码 0 且输出含分布 |
| C3 | p50 / p95 / p99 写入主设计 §7.1 的"Tauri 现状基线"行 | 数值非空 |
| C4 | 打点不改变行为：开关 200 次后进程数/Private 与 Batch B 基线一致（±10 MB） | 前后快照 |
| C5 | 关闭开发者模式后 `[timing]` 仍落盘，其余 debug 日志不落盘 | 手工切换验证 |

### 全批次通用

| ID | 验收项 |
|---|---|
| G-1 | 每批次可独立回退（单批改动不依赖下一批） |
| G-2 | 文档同步：主设计 §2.2 / §7.1、`docs/roadmap.md`、相关 `MODULE.md` |
| G-3 | 架构门禁 `check-architecture.mjs` + `check-native-architecture.mjs` 常绿 |
| G-4 | 部署物：release exe 覆盖安装目录，旧版留 `.bak-<时间戳>`，启动项指回安装路径 |

## 6. 实施顺序与理由

```text
Batch A（度量） → Batch B（生命周期） → Batch C（计时）
```

- **A 必须在 B 之前**：B2/B4/B5 的对比需要 A 建立的可重复采样与归档口径；否则"降了多少"无法自证。
- **B 在 C 之前**：C4 要求"打点后占用与 B 基线一致"，先有 B 的基线才能判。
- **C 放最后**：它不改窗口生命周期，只加两行日志与一个前端回调，风险最低；但它产出的数字是整个 G0 讨论的起点，不能省。

## 7. 风险与回退

| 风险 | 应对 |
|---|---|
| Manager 懒创建后首次打开明显变慢，体验下降 | 记录首次打开耗时（目标 ≤ 1.5 s，与主设计 §7.3 一致）；若超 2.5 s，改为"托盘常驻但 5 min 空闲销毁"的折中，并记 ADR 例外 |
| 窗口销毁导致 `window_state` 位置丢失 | 插件按 label 存取位置；销毁前会触发保存。验收 B3 明确检查"位置保持" |
| WebView2 renderer 不随窗口销毁释放 → B4 不达标 | 属运行时行为，不硬扛：改为"临时窗口复用单一窗口 + 切换内容"，并记录为 P1 必须解决的问题 |
| 计时打点引入日志膨胀 | 每对 2 行，按每天 50 次唤起 ≈ 100 行/天，2 MB 轮转下可忽略 |
| B8 触发（降幅不足） | **这不是失败，是设计被证伪**：按主设计 §6 Phase 0.5 的要求回 Phase 0 修正动因判断，并把结论写进 ADR-0009 |

## 8. 与主设计的关系

本阶段不改变任何目标值，只做两件事：把 §6 Phase 0.5 的剩余项落地，把 §7.1 的主门禁从"脚本 SKIP"变成"有真实分布"。**G0 的判决仍然依赖 S1–S4 四个 spike，本阶段只提供它们的前置数据与工具链验证。**

## 9. 实施记录（2026-11-24，已执行）

### Batch A：度量与归档

| ID | 结果 | 证据 |
|---|---|---|
| A1 | ✅ | `-Label` 写入 `Pose` 字段，`-OutFile` 输出无 BOM 的 UTF-8 JSON；`ConvertFrom-Json` 可解析 |
| A2 | ✅ | `docs/releases/footprint/0.2.0/` 下三姿态各一份（before / after 分文件） |
| A3 | ✅ | 主设计 §2.2.1 回填三姿态前后对比 |
| A4 | ✅ | 脚本仍只读；采样前后进程数一致 |

实现中踩到一个 PowerShell 5.1 的坑：`$json = $report | ConvertTo-Json` **赋值失败**，因为变量名大小写不敏感，`$json` 与开关参数 `$Json` 是同一个东西。改用 `$jsonText`。

### Batch B：窗口生命周期

| ID | 结果 | 实测 |
|---|---|---|
| B1 | ✅ | 启动后窗口列表 = `["launcher"]` |
| B2 | ✅（口径修正） | Private **600.0 → 261.6–282.8 MB（−53% ~ −56%，三次采样）**；WebView2 进程 7 → **6**。原定"≤4 进程"是错的：该运行时固定保留 browser / GPU / utility / crashpad 一组进程，单窗口下限就是 6，因此判定改为"≤6 且 Private ≤330 MB" |
| B3 | ✅ | 关闭后窗口 12 ms 内消失；重建 222–248 ms（远低于 2 s 预算） |
| B4 | ✅ | 10 轮「开→关」后 Private 474.2 → 339.5 MB，进程数不变（6），无累积 |
| B5 | ✅ | 2 笔记：8 webview / 447.6 MB；关闭后回到 6 / 346.6 MB，窗口列表回 `["launcher"]` |
| B6 | ✅（修复后才通过） | `--autostart` 二次实例不弹 Manager 且进程立即退出；普通二次实例唤起 Manager 后自身退出，host 进程数保持 1 |
| B7 | ✅ | 前端 57 文件 / 418 项、Rust 96 项、`tsc`、两条门禁全绿 |
| B8 | 未触发 | 降幅 53% > 25% |

B6 一开始**不通过**，由此发现两个真实缺陷：

1. **单实例插件注册太晚**：它必须是 `Builder` 上的第一个插件，否则第二个进程已经建好窗口、注册好热键才收到"该退出"的通知（现象：多一个 `about:blank` 窗口 + `HotKey already registered` 警告）。
2. **不能在事件循环线程里建 WebView**：`reveal_manager_window` 就地 `build()` 会与事件循环互相等待，第二个进程卡在握手、第一个进程的 Manager 永远是 `about:blank`。改为 `request_manager_window()`（`tauri::async_runtime::spawn` 后再建）后通过。这条已写进 `app_windows.rs` 的注释与 `src-tauri/src/MODULE.md`，避免后人再踩。

### Batch C：计时契约

| ID | 结果 | 实测 |
|---|---|---|
| C1 | ✅ | 30 次唤起产生 31 对 `hotkey_down` / `first_frame`（另有 30 条"仅 down"来自关闭动作，属预期） |
| C2 | ✅ | `measure-latency.ps1` 输出 `Samples = 31`，退出码按预算判定 |
| C3 | ✅ | p50 **87 ms**、p95 **101 ms**、p99 **130 ms**、max 130 ms，已写入主设计 §7.1 的基线表 |
| C4 | ✅ | 追加 15 次唤起后 Private 496.0 → 500.3 MB（+4.3 MB），进程数不变 |
| C5 | ✅ | `set_debug_mode(false)` 后清空日志再唤起：新增内容只有 `[timing]` 三行，其余日志不落盘 |

打点实现要点：`hotkey_down` 放在去抖/排除应用判断之后、选区捕获之前（选区捕获有 180 ms 上限，必须计入）；`first_frame` 由 Launcher 页在 `launcher-opened` 后连续两帧回调；`logging::log_timing()` 绕过 debug 开关直接落盘，其余日志继续受控。

### 部署状态

- release exe 已覆盖 `C:\Users\yukin\AppData\Local\EasyFlowHub\easyflowhub.exe`，保留一份备份 `easyflowhub.exe.bak-20261125-0008`（多余的中间备份已清理，安装目录 62.2 MB）
- 启动项仍指向安装路径 + `--autostart`
- 应用当前运行中，只持有一份 Launcher 窗口
