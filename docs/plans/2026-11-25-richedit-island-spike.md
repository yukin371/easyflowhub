# S2 RichEdit 文本岛 Spike（设计 / 验收标准）

> 状态：`implemented`（2026-11-25 设计 → 按 A/B/C 三批实施 → 通过，2 项未测；见 §9）
>
> 上游：[native-runtime-redesign.md](./2026-11-24-native-runtime-redesign.md) §5.2（`hub-editor`）、§5.8（预热）、§6（S2 判据）、§7.1（键入 ≤ 16 ms）
> 决策：[ADR-0009-windows-native-runtime.md](../decisions/ADR-0009-windows-native-runtime.md)
> 门禁：[ARCHITECTURE_GUARDRAILS.md](../ARCHITECTURE_GUARDRAILS.md) 第 10 节、`scripts/check-native-architecture.mjs`
> 前序：[2026-11-25-native-shell-spike.md](./2026-11-25-native-shell-spike.md)（S1，热唤起 p95 48.9 ms）

## 1. 这一阶段要回答什么

S1 证明的是"壳够快"，但那个壳里**没有一个能打字的字符**。主设计把 S2 的判据写成一句话："RichEdit 作为 egui 子窗口，能否做到中文组合输入、光标跟随、深色背景、DPI 缩放、撤销/重做、粘贴图片"，通过标准是"三种输入法各 30 分钟无异常、1 万字无可感延迟、Undo 栈正确"。

所以本阶段回答三个问题，全部要求本机实测：

1. **系统文本控件能不能"贴在" egui 自绘面板里**，跟着布局走、不被 GL 输出盖掉、不另开一个看得见的窗口？
   这不是显然的：父窗口是 OpenGL 表面，子 HWND 依靠 GDI 画在自己那块矩形上，一旦父窗口没有 `WS_CLIPCHILDREN`，每次 `swap_buffers` 都会把子控件盖掉一帧——表现为"打字时闪"。
2. **键入到绘制的应用侧成本是多少，1 万字时会不会退化？**
   RichEdit 的经典退化是按字符重排全文。必须给出**两个文档长度下的两组分布**，而不是一个平均值。
3. **把键盘交给原生控件之后，壳层的按键还能不能到？**
   Esc 关面板、Ctrl+Z 撤销、Tab 不跑焦——这三件事决定了"文本岛"是嵌进产品还是嵌进一个孤岛。

**本阶段不回答**：Markdown 装饰好不好看（P3）、图片粘贴与资源落盘（P3，依赖 S3 的存储布局）、真实输入法的长时间人机验收（需要人，见 §2）。

## 2. 范围

**做：**

- 新建 `hub-editor` crate（门禁已预留这条边：只允许依赖 `hub-win` / `hub-core`，且禁止出现 `egui::`）
- 在 S1 的面板窗口里以**子窗口**承载 `RICHEDIT50W`（`Msftedit.dll`），矩形由 egui 布局给出、按物理像素对齐
- 深色底、文字色、中文字体（与面板同一字体族）
- 焦点取得与按键归属：Esc 上抛、Ctrl+Z 归控件、Tab 插入制表符
- 1 万中文字符加载；键入 / 粘贴 / 撤销 / 滚动 的自动化检查
- `--editor-bench N` 自动化采样 + `scripts/measure-native-editor.ps1` + 结果归档
- 主设计 §7.1 回填"原生 S2 键入实测"行

**本阶段明确不做（并说明理由）：**

| 不做 | 理由 |
|---|---|
| 图片粘贴、附件资源落盘 | 属 P3。图片要落盘到 `notes.db` 的附件布局，那是 S3 的范围；spike 里做等于先写一半存储层 |
| Markdown 装饰（标题/列表/引用/代码/链接） | 属 P3。S2 只回答"系统文本栈能不能当编辑器"，装饰是画在它上面的东西 |
| 自动保存、`notes.db` 读写 | 属 S3。spike 用内存文档，绝不碰真实笔记库（避免把实验数据写进用户数据） |
| **真实输入法 3 × 30 分钟人机验收** | **需要人工操作**，无法自动化。本 spike 只验证 IME 消息链路是否通（`ImmGetContext`、`WM_IME_*` 未被吞、候选窗定位交给系统），三连测在实施记录里标为「未测」，不伪装成已验证 |
| 右键菜单、拖放、滚动条美化 | P3。spike 允许用系统默认滚动条 |
| 语法高亮、多光标、块选择 | 不在 v2 编辑器范围（主设计 §5.2 未列） |

## 3. 现状事实（代码级）

| 事实 | 数值 / 位置 | 对 S2 的含义 |
|---|---|---|
| `hub-editor` 尚不存在 | `native/crates/` 只有 core / win / ui | 门禁里 `hub-editor: ["hub-win","hub-core"]` 的边**早就写好了**，说明几何必须用物理像素入参、不允许把 egui 类型漏进去 |
| 面板是 OpenGL 表面 | eframe 0.31 + `glow`，S1 已落地 | 子 HWND 可见性取决于父窗口 `WS_CLIPCHILDREN`；winit 默认不设，必须自己补 |
| `RICHEDIT50W` 类需要先加载 DLL | `LoadLibraryW("Msftedit.dll")` | 不加载则 `CreateWindowExW` 返回 NULL 且 `GetLastError` 往往是 1407（找不到窗口类）。老的 `RICHEDIT20W`（Riched20.dll）对双字体与 CJK 支持差，不用 |
| `EM_SETCHARFORMAT` 用 `CHARFORMATW`（92 字节） | `cbSize` 必须等于结构体大小 | 大小写错时 RichEdit **静默忽略**整条消息、不返回错误——典型症状是"设了颜色但字还是黑的" |
| egui 是逻辑点，RichEdit 是物理像素 | 150% 下 1 逻辑 = 1.5 物理 | 换算必须 `round(rect * ppp)`，否则右边和下边各差 1–2 px（S1 的锚定已经踩过一次同类问题） |
| §7.1 写的是"键入到字符上屏 ≤ 16 ms" | 主设计 line 664 | DWM 合成本身就是 60 Hz 一帧（16.7 ms）。这个预算**只能量到"控件绘制完成"**，合成必须单列，否则等于要求"比一个刷新周期还快"（口径见 §4.4） |
| 1 万字 CJK 文档 | 主设计判据"无可感延迟" | 需要两档测量（1k / 10k），只看单档会把退化掩盖在噪声里 |

## 4. 设计

### 4.1 承载方式：子窗口（D1）

```text
launcher 窗口 (eframe / glow, WS_CLIPCHILDREN)
└─ 客户区
   ├─ egui 绘制：标题、搜索框、卡片网格（S1 已有）
   └─ 子 HWND: RICHEDIT50W   ← 只占"正文"那一块矩形，其余全部由 egui 画
```

- 父窗口补 `WS_CLIPCHILDREN`：这是"GL 不盖子控件"的唯一保证。
- `CreateWindowExW(0, "RICHEDIT50W", parent=panel_hwnd, WS_CHILD | WS_VISIBLE | ES_MULTILINE | ES_AUTOVSCROLL | ES_WANTRETURN | WS_VSCROLL, x, y, w, h)`。
- **退路（写在这里，避免到时临时发明）**：若截图或实机显示 GL 仍盖住子控件（旧驱动、远程桌面、部分集显），改为 **owned popup**：`WS_POPUP`、`owner = 面板`、同一矩形，用 `SetWindowPos` 跟随。代价是 z-order 要自己维护，收益是不依赖父窗口的裁剪行为。
- **拒绝的方案**：用 egui 自带 `TextEdit` 当正文。它在 IME 组合、大文档、撤销栈三件事上都不达"编辑器"，S1 §2 已把它排除在 T0 之外。

### 4.2 几何同步（D2）

- egui 每帧 `allocate_exact_size` 出一块矩形（"留洞"，不是自绘）。
- 布局结束后换算：`x_px = round(rect.min.x * ppp)`、`w_px = round(rect.width() * ppp)`，依此类推。
- **只在变化时** `SetWindowPos`：缓存上一帧 `(x, y, w, h)`，四元组全等则跳过。每帧无条件 `SetWindowPos` 会让子控件每帧重绘，白白吃掉 16 ms 预算。
- 面板整体移动**不需要**动子窗口：子窗口坐标是父窗口客户区相对，跟着父窗口走。
- DPI 变化时 `ppp` 变了 → 下一帧 rect 换算自然得到新值；但**控件字体不随 DPI 变**，需要在 `ppp` 变化时重发 `EM_SETCHARFORMAT`（字号按 ppp 缩放）。

### 4.3 主题与字体（D3）

- 背景：`EM_SETBKGNDCOLOR(0, bg)`（`bg` 取自面板主题的实色底）。
- 文字：`EM_SETCHARFORMAT(SCF_ALL, CHARFORMATW { cbSize, dwMask: CFM_COLOR | CFM_FACE | CFM_SIZE | CFM_CHARSET, crTextColor, szFaceName: "Microsoft YaHei UI", yHeight: 11pt * 20 })`。
  `yHeight` 的单位是 1/20 磅，不是像素——写错会得到 0.5 pt 或 200 pt 的字。
- `EM_SETLANGOPTIONS(IMF_AUTOFONT | IMF_DUALFONT)`：中日韩与西文分别选字体，缺了这一步混排会跳。
- 关闭自动 URL/邮箱识别（`EM_SETEDITSTYLE` 清 `SES_…`，`EM_AUTOURLDETECT(0)`）：否则正文里的 `http://` 会被染成蓝色并加下划线，等于编辑器替用户改了样式。
- 字体必须与 egui 面板一致（S1 用 `msyh.ttc`），否则同一块面板里"标题"和"正文"的字形重量不一致，一眼能看出拼接。

### 4.4 测量口径（D4）——本阶段最重要的定义

四个数字，全部自动采集：

| 指标 | 打点 | 含义 | 是否门禁 |
|---|---|---|---|
| `insert` | `SendMessageW(WM_CHAR)` 前后 | 字符插入与局部重排成本 | 否（归因用） |
| `repaint` | `InvalidateRect` + `UpdateWindow` 前后 | **强制全量重绘**的成本（上界，见下） | 否（归因用） |
| **`type`** | 上面两段之和（同一个字符） | **app 侧「键入 → 字符已绘制」** | **是：p95 ≤ 16 ms** |
| `compose` | `type` 之后再 `DwmFlush()` | 含合成到下一帧 | 否，只报告 |

**口径声明（四条，必须在实施记录里可核对）：**

0. **`repaint` 为什么要强制重绘。** 实测：`UpdateWindow` 紧跟在 `WM_CHAR` 之后耗时 0 µs——RichEdit 在消息内部就同步画完了，没有脏区可供"刷新"，所以"插入 vs 绘制"在自然路径上根本无法分离。门禁因此改为 `insert` + 一次显式 `InvalidateRect` + `UpdateWindow` 的全量重绘：两段有重叠，**这个和是真实成本的上界**，对门禁而言是安全方向。

1. **合成不计入门禁**。DWM 合成是 60 Hz 的一帧（16.7 ms），`DwmFlush` 的返回值必然落在 `[paint, paint + 16.7]`，把相位等待算进去等于要求"比一个刷新周期还快"——这条指标就永远不可能达标，指标本身失去意义。因此门禁量**应用控制的部分**，合成单列报告（`compose` 的 p95 落在 17–20 ms 是正常的）。
   这与 S1 的处理方式是同一个：S1 也把"内容刷新（p50 140 ms）"单列、不进门禁。**16 ms 这个数字没有改**。
2. **不走真实键盘驱动**。`SendMessageW` 直接投递到控件，省掉键盘驱动与消息队列（后者 < 1 ms，量级可忽略）。但真实 IME 的按键节奏完全不同，这一点**无法自动化**，列入未测。
3. **每样本间隔 60 ms**，避免把上一字符的排队算进下一字符。另加一条**吞吐**指标：`PostMessageW` 连发 200 个 `WM_CHAR` 后泵消息直到绘制结束、测总耗时——它抓的是"消息队列被饿死"这类单次延迟看不出的问题。
4. **`compose` 不可测时标 `n/a`，不记数字**。窗口停靠 + 桌面空闲时 DWM 停止合成，`DwmFlush` 会返回 254 ms（≈ 空闲超时）；这个值不是编辑器成本。采样器先探一次，> 40 ms 即判定合成器休眠、整段标 `n/a`——宁可少一个数字，也不把空闲时间记成延迟（判据见 `editor_bench::COMPOSE_PLAUSIBLE_US`）。

### 4.5 焦点与按键归属（D5）

- 面板显示后，若当前焦点在正文，`SetFocus(richedit)`。**必须在 reveal 之后**：S1 的 `SetForegroundWindow` 已经抢过前台，提前 `SetFocus` 会被随后的显示流程覆盖。
- Esc：RichEdit 默认不处理（会响一声）。在 `WM_KEYDOWN` 里截获并回调给壳层关面板。
- Ctrl+Z / Ctrl+Y：**不截获**，交给 RichEdit 自己的撤销栈。壳层若也绑了撤销，必须先让出。
- Tab：截获并插入 `\t`，否则焦点会跳出面板（`WS_TABSTOP` + `IsDialogMessage` 的组合问题）。
- 这套截获是本阶段**唯一需要子类化**的地方（`SetWindowSubclass`，约 40 行）；所有分支必须 `DefSubclassProc` 兜底，否则消息会被吞掉。

### 4.6 crate 分层（D6）

```text
app        → hub-editor（把 egui::Rect 换成 (x, y, w, h) i32 再交出去）
hub-editor → hub-win（窗口/消息/DWM）、hub-core（纯逻辑）
hub-editor 里不允许出现 egui::（门禁硬约束）
```

`hub-editor` 对外只有：`create(parent_hwnd, rect) -> Editor`、`set_rect`、`set_theme`、`text`/`set_text`、`on_escape(callback)`、`bench(n) -> Samples`。不含布局决策、不含主题来源、不含持久化。

## 5. 验收标准

### Batch A：宿主与几何（阻塞后续）

| ID | 验收项 | 判定 |
|---|---|---|
| A1 | `hub-editor` 建起来，门禁仍然通过 | `check-native-architecture.mjs` 打印 `passed (5 crates)`；`cargo test` 全绿 |
| A2 | `RICHEDIT50W` 创建成功 | `CreateWindowExW` 非 NULL；`GetClassNameW` 回读 `RICHEDIT50W`；`EM_GETLIMITTEXT` 返回非 0 |
| A3 | 子窗口矩形与 egui 布局一致并跟随 resize | 记录 `rect_logical` 与 `rect_physical`，150% 下 `physical == round(logical * 1.5)`（±1 px）；改窗口宽度后矩形随之变化 |
| A4 | 深色底与文字色真正生效 | `WM_PRINTCLIENT` 到内存 DC 后采样像素：背景像素 = 主题底色；区域内存在与文字色一致的像素（**不靠肉眼**） |
| A5 | 字体与面板一致 | `EM_GETCHARFORMAT` 回读 face/size 与设置值一致 |
| A6 | 1 万字实测加载 < 200 ms | 记录 `load_ms`，且加载后首屏可见（`EM_GETLINECOUNT` 与可见行一致） |

### Batch B：延迟与行为（决定 S2 判决）

| ID | 验收项 | 判定 |
|---|---|---|
| B1 | **`type` p95 ≤ 16 ms、p99 ≤ 25 ms**（n = 200，1k 字符文档） | `--editor-bench 200` 输出的分布；超线即本批不通过 |
| B2 | 1 万字文档下不退化 | 同条件再跑 n = 100，`p95_10k ≤ 2 × p95_1k`；两个数字都写进记录，**不取平均** |
| B3 | 连发 200 字符不饿死消息队列 | `PostMessageW` 200 次后泵消息到绘制结束，总耗时 < 400 ms，且无单次 > 1 s 卡顿 |
| B4 | 粘贴 1 万字 < 150 ms | `EM_REPLACESEL` 前后计时 |
| B5 | 撤销栈正确 | 撤销到栈空后文本逐字节回到原文，重做回到终稿；且撤销栈确有深度（**判定口径已按 §9.3 修订**：RichEdit 把连续键入合并为 1 个单元，"50 次编辑 = 50 步"不成立也不该成立） |
| B6 | 光标跟随与滚动 | 文本超出可见高度后继续输入，`EM_GETFIRSTVISIBLELINE` 推进且插入点仍在可见区 |
| B7 | Esc 上抛，面板关闭 | 日志 `editor_esc` + 返回码；面板回到屏外停靠 |
| B8 | Ctrl+Z 不被壳层抢走 | 发送 `Ctrl+Z` 后 `text` 回退一步，且面板状态未变。**实施结果：组合键部分未测**（无法桩出 `GetKeyState(VK_CONTROL)`），只验到"非截获键仍到达控件"，见 §9.2 |
| B9 | IME 链路通（真机三输入法 = 未测） | `ImmGetContext` 非 NULL；`WM_IME_SETCONTEXT` / `WM_IME_STARTCOMPOSITION` 到达且未被截获分支吞掉；**三种输入法各 30 分钟人机验收标为未测** |
| B10 | `compose` 单独报告 | 输出 p50/p95，落在 17–20 ms 视为正常；**不参与判定** |

### Batch C：工具与归档（产出证据）

| ID | 验收项 | 判定 |
|---|---|---|
| C1 | `--editor-bench N` 一条命令输出分布 | 打印 `insert / paint / type / compose` 四组 p50/p95/p99/max/n |
| C2 | `scripts/measure-native-editor.ps1` 一条命令判预算 | 退出码 0 通过 / 1 超预算 / 0 SKIP（未构建） |
| C3 | 样本归档 | `docs/releases/native-s2/editor-<label>-<ts>.json` 可 `ConvertFrom-Json` |
| C4 | 主设计 §7.1 回填"原生 S2 键入实测"行 | 数值与归档一致 |
| C5 | 文档同步 | 本文件 §9、主设计 §6（S2 状态）、`docs/roadmap.md`、`native/README.md` |

### 全批次通用

| ID | 验收项 |
|---|---|
| G-1 | 每批次独立可回退，单批不依赖下一批 |
| G-2 | 文档同步：主设计 §7.1 / §6 状态、`docs/roadmap.md` P0、`native/README.md` |
| G-3 | 现有门禁全绿：前端 418 项、Rust 96 项、`tsc`、两条架构门禁、smoke 9 步 |
| G-4 | 每批实施完成即提交，提交信息说明"为什么"而不只是"改了什么" |

## 6. 实施顺序与理由

```text
Batch A（宿主 + 几何 + 主题 + 像素证据） → Batch B（延迟 + 行为） → Batch C（工具 + 归档）
```

- **A 是 A2/A3/A4 三连**：控件建得出来、位置放得对、颜色真的生效。这三条任一不过，"文本岛"就不成立，B 的数字没有意义（量一个看不见的东西的延迟）。
- **B 是判决点**：B1 不过，S2 就应该按 ADR-0009 的退路走（C# 控件或保留 WebView 编辑器），而不是继续调参。
- **C 不产出新代码路径**，只把 B 的手工命令固化成一条命令 + 归档。

## 7. 风险与回退

| 风险 | 应对 |
|---|---|
| GL 输出盖住子控件 | 先补 `WS_CLIPCHILDREN`；仍不行按 §4.1 退路改 owned popup，并在记录里写清是哪个环境显现 |
| 中文 IME 候选窗定位异常 | 用 IMM32 手动 `ImmSetCandidateWindow` 定位；最差回退"WebView 编辑器作为临时退路"（主设计 §7 已声明）。本阶段先证明消息链路通 |
| 1 万字退化 | 给两档分布而不是平均值；若退化 ≥ 2 倍，先关自动 URL/自动更正与 undo 深度，仍退化则 P3 引入分页载入 |
| 16 ms 不可达 | 先看归属于 `insert` 还是 `paint`：`paint` 慢说明整窗重绘（记录脏区大小）。**不允许把门禁改成 33 ms**——改口径要先改文档并说明理由 |
| 子类化吞掉消息 | 所有截获分支 `DefSubclassProc` 兜底；B7/B8 就是这条的回归测试 |
| `CHARFORMATW` 尺寸写错导致静默失败 | A4 用像素采样验证，不靠"我觉得设上了" |

## 8. 与主设计的关系

- 只回答"系统文本栈能不能贴在自绘壳里"，不回答"Markdown 好不好看"。
- 若 B1 不达标：编辑器路线按 ADR-0009 §7 的退路评估（C# 控件 / 保留 WebView 编辑器），结论与数据写进 ADR，不留在对话里。
- `hub-editor` 落地后，主设计 §5.2 的五层里还缺 `hub-bridge`（数据桥），那是 S3 的范围。

## 9. 实施记录

> 实施日期：2026-11-25。测量环境：本机 3840×2160 @150%（`system_dpi=144`），Windows，`native/` release 构建，`easyflowhub-native.exe --editor-bench 200`，三次运行取中位数。归档样本：`docs/releases/native-s2/editor-windows-144dpi-20261125-090806.json`（原始单次）、`editor-windows-144dpi-20261125-091009.json`（`measure-native-editor.ps1` 校验档，17 项结构证据全通过）。
>
> **判决：S2 通过**（B1/B2 各留 ≥ 3 倍余量），带 **2 项未测**（真机 IME 人机验收、真实 Ctrl+Z 组合键）与 **1 项口径偏差**（B5 的撤销步数，见 §9.3）。

### 9.1 关键数字（三次运行）

| 指标 | 结果 | 门槛 | 判定 |
|---|---|---|---|
| `type` p95（1k 文档，n=200） | **4.95 ms**（4.94 / 5.25 / 4.95） | ≤ 16 | **通过，余量 11 ms** |
| `type` p99（1k 文档） | **6.33 ms**（6.49 / 6.19 / 6.33） | ≤ 25 | 通过，余量 18.7 ms |
| `type` p50（1k 文档） | 2.9 ms（3.09 / 2.81 / 2.77） | — | — |
| `type` max | 6.8 ms | — | 无长尾 |
| 10k 文档 `type` p95（n=100） | **5.09 ms**（5.30 / 5.09 / 4.69） | ≤ 2 × 1k | 通过，比值 **0.95–1.07** |
| `insert` p50 / p95 | 1.7–2.0 / 3.9–4.4 ms | — | 归因用 |
| `repaint` p50 / p95（强制全量） | 1.1 / 1.6–1.8 ms | — | 归因用 |
| `compose` p50 / p95（仅报告） | 4.9–7.5 / 7.6–9.6 ms | 不判定 | — |
| 10k 加载 | 7.9–11.5 ms | < 200 | 通过 |
| 10k 粘贴（`EM_REPLACESEL`） | 13.3–14.4 ms | < 150 | 通过 |
| 连发 200 字符 | 203–225 ms | < 400 | 通过 |

**10k 文档的分布比 1k 还略快**（比值 0.95 / 0.97 / 1.01 / 1.07，没有一次 ≥ 1.1）。这不是巧合也不是取平均的产物：`insert` 的 p50 在两档几乎相同（1.6–2.3 ms），说明 RichEdit 的插入成本与文档长度无关——每次 `WM_CHAR` 只重排受影响的那一段，不是"按字符重排全文"。这是 S2 最想确认的一件事，它确认了。

### 9.2 逐条验收

**Batch A（宿主与几何）**

| ID | 结果 | 证据 |
|---|---|---|
| A1 | 通过 | `check-native-architecture.mjs` → `Native architecture guard passed (5 crates)`；`cargo test --workspace` 43 项全绿 |
| A2 | 通过 | `GetClassNameW` 回读 `RICHEDIT50W`；`EM_GETLIMITTEXT` = 2147483632（创建时用 `EM_EXLIMITTEXT(0x7FFF_FFF0)` 抬到上限，1 万字有余量） |
| A3 | 通过 | `resize.same_moved=false / changed_moved=true / restored_moved=true`：矩形不变时**不发** `SetWindowPos`（D2 的核心），变宽度才移动；150% 下物理矩形 606×1148+18+51 = `round(404×765.3×1.5)`，四舍五入而非截断（`hub-core::island` 有 5 个单测覆盖 15×1.5=22.5→23 这类边界）。"改窗口宽度后矩形随之变化"用人机拖动确认过，自动化只覆盖到 `set_rect` 层 |
| A4 | 通过（不靠肉眼） | `WM_PRINTCLIENT` 到内存 DC 采样 19392 点：主色 `0x2D2521` 占 98.49%（= 主题底色），**空文档文字色像素 = 0，写入 20 字后 = 135**；`ink_share` 0.000 → 0.015。§7 那条"`CHARFORMATW` 尺寸写错静默失败"因此有了一道真检查 |
| A5 | 通过 | `EM_GETCHARFORMAT(SCF_ALL)` 回读：`face=Microsoft YaHei UI`、`height=330 twips`（= 16.5 pt = 11 pt × 1.5 缩放）、`color=0xF7EEE8`，与写入值一致 |
| A6 | 通过 | 10k 加载 7.9–11.5 ms（门槛 200）；加载后 385→**589 显示行**（144 dpi 下 33 px 字宽，17 字/行），远多于 1148 px 首屏可容纳的 ~52 行，即"必须滚动"成立 |

**Batch B（延迟与行为）**

| ID | 结果 | 证据 |
|---|---|---|
| B1 | **通过** | p95 4.95 ms ≤ 16、p99 6.33 ms ≤ 25 |
| B2 | **通过** | 10k p95 5.09 ms，比值 0.95–1.07；两档数字都写进 §9.1，未取平均 |
| B3 | 通过 | 连发 200 字符 203–225 ms < 400，且总耗时内无单次 > 1 s 卡顿（泵消息有 `count*4+100` 上限，未触顶） |
| B4 | 通过 | `EM_REPLACESEL` 写入 10k = 13.3–14.4 ms < 150；`char_count` 回读 10000 |
| B5 | 通过（**判定口径有偏差，见 §9.3**） | 连续键入 10 字 → 撤销栈 **1** 个单元；移动光标后 10 字 → **2** 个单元；替换型编辑 → 2–3 个单元；**每种情形逐步撤销到栈空后文本都精确回到 `起点`**（逐字节比较），重做回到终稿 |
| B6 | 通过 | 小文档 `first_visible=0`（不需要滚动），10k 文档 `first_visible=569`、`offset_y=31295`。**但有一个前提**：`EM_SCROLLCARET` 在控件没有焦点时是空操作——首次运行时两条探针都返回 0，不是控件坏了，是没拿焦点。已在 `sync_editor` 创建后补 `editor.focus()`，这同时修掉了产品里"输入超过一屏后光标不跟随" |
| B7 | 通过 | `WM_KEYDOWN(VK_ESCAPE)` 返回 0（被吞）且壳层回调置位 → `esc_ok=true`。交互模式下该标志由 `update()` 消费、面板停靠回屏外 |
| B8 | **未测（组合键部分）** | 可自动验的部分已验：非截获键（`VK_Z`）经队列 + `TranslateMessage` 后仍插入字符（`fallthrough_ok=true`），说明子类化只吃 `WM_KEYDOWN` 里的 ESC/TAB，`DefSubclassProc` 兜底有效；撤销本身用 `EM_UNDO` 验过。**真实 Ctrl+Z 无法构造**：`GetKeyState(VK_CONTROL)` 需要真实键盘状态，桩不出来。风险判定为低（没有任何分支拦 Ctrl+Z），但按标准记未测 |
| B9 | **未测（人机部分）** | 可自动验的部分已验：`ImmGetContext` 非 NULL（`imm_ok=true`），且子类过程只检查 `WM_KEYDOWN`，`WM_IME_*` 一律 `DefSubclassProc`。**三种输入法各 30 分钟人机验收未做**——需要人坐在机器前打字，无法自动化，也未伪装为已验证 |
| B10 | 通过 | `compose` 单列输出 p50/p95，不参与判定 |

**Batch C（工具与归档）**

| ID | 结果 | 证据 |
|---|---|---|
| C1 | 通过 | `--editor-bench 200` 一条命令输出类名/格式/加载/像素/滚动/resize/撤销/按键 + `insert`/`repaint`/`type`/`compose` 四组分布 + 1k/10k 比值 |
| C2 | 通过 | `scripts/measure-native-editor.ps1` → `RESULT: PASS`、退出码 0；未构建时打印 SKIP 并退出 0；预算或结构证据任一失败 → 退出 1 |
| C3 | 通过 | `docs/releases/native-s2/editor-<label>-<ts>.json`，`ConvertFrom-Json` 与本项目 `JSON.parse` 均验证可解析 |
| C4 | 通过 | 主设计 §7.1 新增"原生 S2 实测"表，数值与 §9.1 一致 |
| C5 | 通过 | 本文件 §9、主设计 §4.3 状态段、`docs/roadmap.md`、`native/README.md` 同步 |

**全批次**

| ID | 结果 |
|---|---|
| G-1 | 每批独立可回退：B 批只动 `hub-editor`/`app`，A 批的 crate 与几何不受影响 |
| G-2 | 文档同步完成（主设计 §7.1/§4.3、roadmap P0、`native/README.md`） |
| G-3 | 现有门禁全绿：`cargo test --workspace` 43 项、架构门禁 5 crate、前端 418 项与 `tsc` 未受本次改动影响（改的都是 `native/`） |
| G-4 | 按 A/B/C + 文档分 4 个提交，提交信息写"为什么" |

### 9.3 偏差（必须显式记录，不能悄悄改判据）

**B5 的撤销步数："50 次编辑 = 50 次撤销"不成立，且不该成立。**

原文写的是"50 次编辑后 50 次 `EM_UNDO` 回到原文"。实测：连续键入 50 字 → 撤销栈只有 **1** 个单元；`EM_STOPGROUPTYPING` **无效**（5 字 + 该消息 + 5 字 仍是 1 个单元）；退格也不切开。只有"移动光标"或"替换型编辑"会切开（→ 2–3 个单元）。

这不是缺陷，是 `RICHEDIT50W` 的设计：一次连续键入合并为一个撤销单元，Word 也是如此。反过来说，**逐字符撤销是更差的操作体验**（用户按一次 Ctrl+Z 期望退掉刚敲的那句话，而不是一个字）。因此把 B5 的判据改为：

1. 逐步撤销到栈空后，文本**逐字节**回到原文（`undo_ok=true`）；
2. 再重做到栈空，文本回到终稿（`redo_ok=true`）；
3. 撤销栈**确有深度**（`grouped=2 > 1`）——否则前两条可以靠"一个快照"作弊。

判据改了、理由留了、原始数字（coalesced=1）也留了。门禁没有被放宽：B5 检查的是"撤销能把内容还原"，而"每次编辑一步"从来不是主设计的意图（原文是"Undo 栈正确"）。

### 9.4 作废的口径（为什么 §9.1 没有 1.58 ms）

首轮实现（60 轮、未拿焦点、桌面空闲）测出 `type` p95 **1.58 ms**、`repaint` **0.00 ms**、`scroll` 两条探针都是 0。这组数字**全部作废**，原因是测量定义错了，不是速度快：

| 现象 | 真正原因 | 处置 |
|---|---|---|
| `repaint` 恒为 0.00 ms | RichEdit 在 `WM_CHAR` 内部同步绘制，之后 `UpdateWindow` 没有脏区可刷 | 改为 `InvalidateRect` + `UpdateWindow` 的**强制全量重绘**，并改名为 `repaint`，明确声明是上界 |
| `scroll small=0 large=0` | `EM_SCROLLCARET` 在无焦点时是空操作 | 探针前 `focus()`；同时修掉产品侧的焦点缺失 |
| `compose` p50 254 ms | 窗口停靠 + 桌面空闲时 DWM 不合成，`DwmFlush` 变成等空闲超时 | 加合理性判定（> 40 ms 即视为合成器休眠），整段标 `n/a` 而不是记下 254 ms |
| `type` 1.58 ms | 未取得焦点，且合成器休眠 | 作废，改用带焦点的 4.95 ms |

带焦点后 `insert` p50 从 0.74 → 1.9 ms（约 2.6×）：焦点意味着每次字符要维护光标、必要时滚动视图，这是**产品里必然发生的成本**，早先那个数字省掉的是真实工作。所以 4.95 ms 才是可用的口径，且它仍有 3 倍余量。

### 9.5 代码级事实（补进 §3 的清单，供下一个人少踩）

| 事实 | 含义 |
|---|---|
| `EM_SCROLLCARET` 需要焦点 | 没有焦点时移动选区/调用该消息都不滚动，`EM_GETFIRSTVISIBLELINE` 恒为 0。`EM_LINESCROLL` 不需要焦点（用它区分"控件坏了"和"没拿焦点"） |
| RichEdit 在 `WM_CHAR` 内同步绘制 | "插入 vs 绘制"在自然路径上不可分离，只能量上界 |
| `EM_STOPGROUPTYPING` 对 `RICHEDIT50W` 无效 | 想切撤销单元只能靠光标移动或换编辑类型 |
| 空闲桌面下 `DwmFlush` 可达 254 ms | 合成器的等待时间不是编辑器成本，报告它之前必须先判断合成器是否在合成 |
| `SetWindowSubclass` 在 `Win32::UI::Shell` | 不是 `UI::Controls`；`UpdateWindow` 在 `Graphics::Gdi`，`SetFocus` 在 `UI::Input::KeyboardAndMouse`——首次编译在这三处各错一次 |
| `EM_GETSCROLLPOS` 要用 `POINT*` 传参 | 返回值不承载滚动位置 |

### 9.6 下一步

- **S3**（数据兼容）：`notes.db` 读写与附件布局；文本岛届时把内存文档换成真实笔记，`hub-bridge` 落地。
- **S4**（门禁自检）：对 `check-native-architecture.mjs` 注入 4 类故意违规，确认全部报错。
- S2 遗留人工项：三种输入法各 30 分钟（B9）、真实 Ctrl+Z（B8），在 RC 前补测并回填本节。

