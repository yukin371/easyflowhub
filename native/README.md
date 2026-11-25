# native/ — Windows 原生运行时（S1 + S2 spike）

这是 [Native Runtime 重构](../docs/plans/2026-11-24-native-runtime-redesign.md) 的可运行切片：[S1 验收](../docs/plans/2026-11-25-native-shell-spike.md)（热唤起 p95 48.9 ms）与 [S2 验收](../docs/plans/2026-11-25-richedit-island-spike.md)（键入 p95 4.95 ms）的实体。**它不是产品代码**：它回答两个问题——单进程 + 预创建窗口，热唤起能不能压到 p95 ≤ 60 ms；以及系统文本控件贴在 egui 自绘面板里，键入能不能压到 p95 ≤ 16 ms。

## 结构

```text
native/
├─ Cargo.toml            workspace（app + 四个 crate）
├─ app/                  组合根：事件循环、热键线程接线、bench 驱动
├─ crates/hub-core/      纯逻辑：布局夹紧、列数、设置、岛几何（无平台依赖，有单测）
├─ crates/hub-win/       win32：热键、显示器缓存、DWM、进程探针、窗口放置
├─ crates/hub-ui/        egui：调色板、字体、面板绘制与预热
└─ crates/hub-editor/    win32：RICHEDIT50W 文本岛（子窗口）、测量与像素证据
```

依赖方向由 `scripts/check-native-architecture.mjs` 强制（`app → hub-ui/hub-win/hub-core/hub-editor`，`hub-ui → hub-core`，`hub-editor → hub-win/hub-core`，其余无内部边），并禁止 `hub-editor` 出现 `egui::`——文本岛只吃物理像素矩形、只吐文本，布局决策留在 app 侧。

## 跑起来

```bash
cd native
cargo run --release -p app -- --print-monitors     # 打印缓存布局与缩放
cargo run --release -p app -- --probe              # 预热后占用（含窗口数/可见数）
cargo run --release -p app -- --bench 200          # 200 轮自动唤起 + 分布
cargo run --release -p app -- --bench 200 --scale 2.0   # 模拟 200%
cargo run --release -p app -- --editor             # 交互模式：面板里带 RichEdit 正文
cargo run --release -p app -- --editor-bench 200   # 键入分布 + 像素/滚动/撤销证据
cargo run --release -p app                         # 交互模式：Ctrl+Shift+Space 开关面板
```

`--bench` 用 `PostThreadMessageW(WM_HOTKEY)` 触发**与真实按键相同的处理路径**（去抖 → 取光标 → 命中缓存布局 → 唤醒 → 绘制），所以 200 轮不需要人手连按。退出码：`0` 达标，`2` 超预算，`3` 热键注册失败。

`--editor-bench` 在被停靠（隐藏）的窗口上跑完整证据：类名/字符格式回读、`WM_PRINTCLIENT` 像素采样、滚动、resize 跟随、撤销/重做、按键归属，最后是两组键入分布（1k / 10k 文档）。它**不需要可见窗口，也不截屏**，因此可以在无人值守的机器上跑。退出码：`0` 达标，`2` 超预算。

门禁侧：`scripts/measure-native-latency.ps1`（S1）与 `scripts/measure-native-editor.ps1`（S2）各自解析输出、判预算 + 结构证据、归档到 `docs/releases/native-s1|s2/`；`run-smoke.ps1 -NativeBench` 会在 smoke 里跑快速版并写到临时目录，避免污染归档。

文本岛的两个诊断示例（`cargo run -p hub-editor --example scroll_probe` / `--example undo_probe`）保留了"为什么必须拿焦点才滚动""RichEdit 怎么合并撤销单元"的实测结论，别再靠猜。

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
| 无托盘、无中键钩子 | 都不在 60 ms 路径上，属 T0 parity | Phase 1 与功能一起落地 |
| 文本岛正文是内存文档、无保存 | 存储布局属 S3 | Phase 2 接 `notes.db` |
| 文本岛无 Markdown 装饰、无图片粘贴 | 属 P3；装饰画在文本控件之上，不改变 S2 的结论 | P3 |
| 输入法只验到 `ImmGetContext` 非空 | 三种输入法各 30 分钟需要人 | RC 前人工验收，结果回填 S2 文档 §9 |

## 度量口径

`first_frame` 取**保守上界**：窗口在绘制前显示，下一帧开始时上一帧已完成 paint + swap，因此"下一帧开始 − 热键按下"一定不早于真实上屏；同时记录"本次绘制结束 − 热键按下"作为下界。门禁读上界。完整口径见 S1 文档 §4.4。

键入的 `type` 同样是**上界**：RichEdit 在 `WM_CHAR` 内部同步绘制，紧随其后的 `UpdateWindow` 实测 0 µs，所以门禁值 = `insert` + 一次强制 `InvalidateRect` + `UpdateWindow` 全量重绘（两段有重叠，会高估）。合成（`DwmFlush`）单列报告、**不进门禁**——它必然落在 `[paint, paint + 16.7 ms]`，算进去等于要求比一个刷新周期还快。完整口径（含"为什么作废了首轮 1.58 ms"）见 S2 文档 §4.4 与 §9。
