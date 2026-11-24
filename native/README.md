# native/ — Windows 原生运行时（S1 spike）

这是 [Native Runtime 重构](../docs/plans/2026-11-24-native-runtime-redesign.md) 的第一个可运行切片，也是 [S1 验收](../docs/plans/2026-11-25-native-shell-spike.md) 的实体。**它不是产品代码**：它只回答一个问题——单进程 + 预创建窗口，热唤起能不能压到 p95 ≤ 60 ms。

## 结构

```text
native/
├─ Cargo.toml            workspace（app + 三个 crate）
├─ app/                  组合根：事件循环、热键线程接线、bench 驱动
├─ crates/hub-core/      纯逻辑：布局夹紧、列数、设置（无平台依赖，有单测）
├─ crates/hub-win/       win32：热键、显示器缓存、DWM、进程探针、窗口放置
└─ crates/hub-ui/        egui：调色板、字体、面板绘制与预热
```

依赖方向由 `scripts/check-native-architecture.mjs` 强制（`app → hub-ui/hub-win/hub-core`，`hub-ui → hub-core`，其余无内部边）。这条门禁在 `native/` 出现之前一直打印 `skip`；从这一版开始它是真的在跑。

## 跑起来

```bash
cd native
cargo run --release -p app -- --print-monitors     # 打印缓存布局与缩放
cargo run --release -p app -- --probe              # 预热后占用（含窗口数/可见数）
cargo run --release -p app -- --bench 200          # 200 轮自动唤起 + 分布
cargo run --release -p app -- --bench 200 --scale 2.0   # 模拟 200%
cargo run --release -p app                         # 交互模式：Ctrl+Shift+Space 开关面板
```

`--bench` 用 `PostThreadMessageW(WM_HOTKEY)` 触发**与真实按键相同的处理路径**（去抖 → 取光标 → 命中缓存布局 → 唤醒 → 绘制），所以 200 轮不需要人手连按。退出码：`0` 达标，`2` 超预算，`3` 热键注册失败。

门禁侧用 `scripts/measure-native-latency.ps1`（解析 `--bench` 输出、判预算、归档到 `docs/releases/native-s1/`）；`run-smoke.ps1 -NativeBench` 会在 smoke 里跑 40 轮的快速版并写到临时目录，避免污染归档。

## 为什么这么切

- **热键在独立线程**：`RegisterHotKey` 绑定线程消息队列，放在 UI 线程会让每次唤起与绘制抢线程；回调里只做「读光标 + 查缓存 + 入队」。
- **显示器布局启动期缓存**：热键路径**不枚举**显示器（混合 DPI 下这是几十毫秒级的坑，见设计 §5.1.1）。
- **窗口预创建、隐藏态预热三帧**：设备上下文、图集、首帧纹理都在启动期付掉；隐藏后不再 `request_repaint`，空闲 CPU ≈ 0。
- **定位用 win32 物理像素**：egui 用逻辑点，换算一次就多一次舍入机会；面板缓冲按物理像素分配（200% 下 1.5 Mpx）。

## 已知捷径（Phase 1 必须还掉）

| 捷径 | 原因 | 还法 |
|---|---|---|
| `FindWindowW` 按标题取 `HWND` | eframe 0.31 不从 `App` 暴露窗口句柄 | Phase 1 从 winit 拿 `RawWindowHandle` 传下来，删掉这个函数 |
| `--scale` 模拟多档 DPI | 本机只有一块 150% 屏 | Phase 1 用真实多屏/改缩放复核，不只跑模拟 |
| 首帧不是"直接 blit 上一帧纹理" | eframe 不暴露目标帧缓冲 | 实测量不足 60 ms 时改 `egui_glow` 手动循环接管 swap + blit |
| 面板内容是静态样例 | 只为预热真实字符集与布局，不代表最终 UI | Phase 2 换成真实动作/最近项 |
| 无托盘、无中键钩子 | 都不在 60 ms 路径上，属 T0 parity | Phase 1 与功能一起落地 |

## 度量口径

`first_frame` 取**保守上界**：窗口在绘制前显示，下一帧开始时上一帧已完成 paint + swap，因此"下一帧开始 − 热键按下"一定不早于真实上屏；同时记录"本次绘制结束 − 热键按下"作为下界。门禁读上界。完整口径见 S1 文档 §4.4。
