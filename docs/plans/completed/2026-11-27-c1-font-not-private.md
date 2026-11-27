# C1：字库不再计入私有内存（映射，而不是子集化）

> 状态：`completed`（2026-11-27 落地；G0 退出条件 C1 关闭，G1 的 65 MB 线未动）
>
> - 上游判据：[G0 评审](./2026-11-25-g0-review.md) §7 C1「Commit ≤ 100 MB，同时记录 WS / 私有工作集 / Commit 三种口径」
> - 归属批次：Phase 1 启动项（[总设计](../active/2026-11-24-native-runtime-redesign.md) Phase 1）
> - 门禁：`python scripts/check-native-font.py --exe <release exe>`（11 项），`run-smoke.ps1 -NativeFont` 可跑
> - 数字来源：本机单屏 3840×2160 @150%、Win11 26300、release 构建

## 0. 一句话

S1 记下的 130.1 MB 里有 37.7 MB 是「CJK 字库 + 图集」；这一批把字库**映射进地址空间**而不是读进堆，Commit 降到 **91.5 MB**（同一台机器、同一条命令），并且**一个字都没少**——没有子集化，也没有换字体。

## 1. 起点：37.7 MB 到底是什么

S1 用 `--no-font` 做过对照：130.1 MB 有字库，92.5 MB 没有，差 37.7 MB。但那只是一个总数，三种可能混在一起：字库字节、ab_glyph 的解析开销、字形图集。

这一批先把口径补齐（`hub_win::ProcessProbe` 改读 `PROCESS_MEMORY_COUNTERS_EX2`），再量三项：

| 口径 | 含义 | 为什么必须分开 |
|---|---|---|
| `commit`（`PrivateUsage`） | 进程向系统承诺的私有提交量 | 文件映射的页**不计入**它，所以它是「映射成立没有」的判据 |
| `private_ws` | 常驻且私有的工作集 | 排除可与其它进程共享的页 |
| `working_set` | 全部常驻，含本进程碰过的映射页 | 映射不是"免费"，只是"不私有"——这一项必须照实记 |

结论先行：**字库是全部 37.7 MB，图集只有 0.5 MB**（`16384x32`，epaint 初始宽条）。所以"图集优化"不是杠杆，字库字节才是。

## 2. 根因：epaint 在 owned 路径上会再拷一份

`epaint-0.31.1/src/text/fonts.rs` 的 `ab_glyph_font_from_font_data` 分两条路：

```rust
Cow::Borrowed(bytes) => FontRef::try_from_slice_and_index(bytes, index)   // 零拷贝
Cow::Owned(bytes)    => FontVec::try_from_vec_and_index(bytes.clone(), …) // 整份克隆
```

而 `FontData::from_owned(Vec<u8>)` 走的是第二条。也就是说 `msyh.ttc`（19 704 352 B）在启动时被拷了一份给 `FontVec`，此后**至少一份 19.7 MB 常驻私有堆**。`from_static(&'static [u8])` 走第一条，一个字节都不拷。

`&'static` 是这条路的门槛，也是决定实现形态的地方：要想零拷贝，字库的生存期必须覆盖整个进程。

## 3. 决定

**D1 —— 映射系统字体，不做子集化。**
C1 原文写的是"字体子集化"，那是当时对手段的猜测。落地时改成映射，三条理由：

1. **许可证**：`msyh.ttc` 是微软字体，子集化产物随安装包分发是另一件事（要单独评估授权）。映射是"用用户机器上已有的字体"，不产生分发物。
2. **覆盖率**：子集化必须预设字符集。笔记正文由用户输入，任何固定子集都会在某些字上显示豆腐块——这个边界比 37.7 MB 更贵。映射保留整份字体，任意汉字可用。
3. **构建链**：子集化要引入字体工具与产物校验（可复现、可审计），映射只要一个 win32 调用。

方向没变（"别再为字库付私有内存"），换的是手段。C1 里"三种口径都要记"的要求照旧执行。

**D2 —— 只在 `hub-win` 做 win32，`hub-ui` 只接字节。**
`check-native-architecture.mjs` 禁止 `hub-ui` 依赖 `windows-sys`，这条规则在这里起了作用：映射（`CreateFileW` / `CreateFileMappingW` / `MapViewOfFile`）落在 `hub-win::font`，`hub-ui::theme` 只提供 `install_mapped(ctx, &'static [u8], index)` 与 `install_owned(...)` 两个入口。**"字体长什么样"归 UI，"字节从哪来"归平台层**，这条边界不是新造的，是门禁早就画好的。

**D3 —— 堆路径保留，且是产品的一部分。**
映射会失败（文件被替换、被独占、权限异常），那时退回 `std::fs::read` 并记 `mode=heap note="fallback: …"`。一个渲染豆腐块的笔记，比多花 19 MB 更糟。

**D4 —— 保留 `--font-heap` 作为对照运行，`--no-font` 下沉到产品路径。**
没有对照组，"字体省了 37 MB"只能跟记忆里的数字比。`--font-heap` 用同一套代码、同一个字体、同一扇窗，只把字节放回堆里；`--no-font` 之前只对 spike 生效，产品（`--note`）路径根本不看这个开关——C1 第一次归因失败就是踩在这上面。

**D5 —— 图集不动。**
0.5 MB。动它属于"为了一个数字改渲染质量"，收益为零。

## 4. 数字

三种配置 × 三次采样取中位数。**同一次会话内**比较，避免"昨天的数字"混进来：

### 4.1 快速笔记进程（`--note`，本次交付对象）

| 配置 | Commit | 私有工作集 | 工作集 | 图集 |
|---|---:|---:|---:|---|
| mapped（产品路径） | **91.5** | 56.4 | 90.6 | 16384×32 |
| heap（旧路径，`--font-heap`） | 129.1 | 94.0 | 128.0 | 16384×32 |
| none（`--no-font`） | 91.5 | 56.4 | 90.7 | 16384×32 |

- **mapped − none = 0.0 MB**：字库不再向进程要私有内存（图集 0.5 MB 在噪声内）。
- **heap − mapped = 37.6 MB**：被去掉的正是 epaint 那份克隆。
- **工作集只降 37 MB 中的一部分**（128.0 → 90.6）：被碰过的映射页仍常驻，只是**文件支持、可回收、可共享**。这一点不掩饰——Commit 降 37.6、WS 降 37.4 是同一批页的两种记法，不是两份收益。

### 4.2 Launcher（spike 面板，G1 的 60 ms 门禁对象）

| 配置 | Commit | 私有工作集 | 工作集 |
|---|---:|---:|---:|
| mapped | **92.6** | 55.7 | 82.4 |
| heap | 130.3 / 125.1（两次） | 98.3 / 94.1 | 124.7 / 123.3 |
| none | 92.6 | 55.6 | 82.2 |

S1 归档的 **130.1 MB → 92.6 MB**，这就是 G0 §3 记分板上那个 ⚠️ 的现值。

### 4.3 波动，以及为什么门禁取中位数

同一份二进制连续启动，Commit 会在 **91.5 – 99.3 MB** 之间跳（GL 驱动暂存、字形缓存时序），幅度与"要证明的差值"同量级。所以：

- 门禁每种配置跑三次取中位数，并打印 min–max；
- 判据用**差值**（heap − mapped ≥ 25 MB、mapped − none ≤ 6 MB），不用绝对值；
- 100 MB 线仍照实断言（C1 的目标），但只作为"目标"而不是"机制"。

**诚实记录**：91.5–99.3 意味着 100 MB 这条线只剩 0.7–8.5 MB 余量。它是达标的，但不是宽松达标。

## 5. 门禁（`scripts/check-native-font.py`，11 项）

门禁的骨架是**三次受控对照**（mapped / heap / none，只差一个加载路径），而不是"commit 小于 100 MB"这一个数字——否则"字库静默加载失败、面板渲染豆腐块"也会让数字变好。

| # | 判据 |
|---|---|
| 1–2 | 产品路径确实是 mapped，且 `bytes=` 等于磁盘文件大小（整份映射，没有偷偷丢字符） |
| 3–4 | heap 那次真的走了堆、none 那次真的跳过了（对照不是摆设） |
| 5 | `heap − mapped ≥ 25 MB`（证明被去掉的东西真的存在） |
| 6 | `mapped − none ≤ 6 MB`（字库不向进程要私有内存，图集含在内） |
| 7–8 | Commit ≤ 100 MB；私有工作集同线 |
| 9 | mapped 与 heap 的窗口像素差 ≤ 1%（同一张脸、同一批字形，只有字节的住处变了） |
| 10 | 窗口墨迹 > 0.5%（确实在画字） |
| 11 | mapped 与 none 的像素差 ≥ 2%（实测 4.65%：豆腐块与汉字可区分，像素能判） |

### 变异验证（门禁必须先红过）

把 `font.rs` 的 `if !heap` 改成 `if false && !heap`（即：无论怎么调用都走堆），重新构建后跑门禁——四项转红：

```
FAIL  mapped mode is in force: … mode=heap bytes=19704352 note="fallback: "
FAIL  the heap path costs what was removed: heap - mapped = -0.9 MB (>= 25.0)
FAIL  the mapped font costs the process almost nothing: mapped - none = 37.8 MB (<= 6.0)
FAIL  C1 target: commit <= 100 MB: 136.8 MB
7/11 checks passed
RESULT: FAIL
```

像素三项仍然全绿，这是对的：变异只改字节住在哪，不改字形——**只有内存那半边的判据该红**。改回后重跑 11/11。

同时它也抓到了自己的第一个缺陷：`capture()` 起初没接收 `extra`，三次截图全是 mapped 路径——像素检查在拿一张图和它自己比，结果是"通过"。这与 Q9 那次「日志说 pinned、窗口其实被隐藏」是同一类错误：**只判自己产生的那一半证据**。

## 6. 顺带修掉的两件事

**S1 —— 产品路径不认 `--no-font`。** `spike.rs` 尊重它，`hub.rs` 无条件装字体，于是"在 spike 上量字库成本"与"产品实际行为"是两个程序。现在两条路径共用 `app::font::install_and_log`，一行日志只有一个来源。

**S2 —— 七个原生门禁把工作目录写进了构建树。** `check-native-{notes,pin,tools,manager,switcher,drag,recover}` 的 `--work` 默认是 `native/target/<gate>`，另加 `edge` / `size` / `store-schema` 三个。这与 2026-11-26 修的 `measure-native-store.ps1` 是同一个缺陷：门禁的临时状态长在产物目录里，删 target 就等于删证据，换机/换壳时还会撞上目录权限。全部改到 `%TEMP%`（默认值即变，命令行仍可覆盖）。

## 7. 与 G0 账目的对账

| G0 记的 | 现在 |
|---|---|
| 常驻 130.1 MB（Commit） | **91.5 MB** |
| 「字体子集化后大概落在 92–100 MB」 | 实测 91.5–99.3，**落在预估区间的下沿** |
| 65 MB 解释线 | **未动**。C5 的触发条件是"C1 落地后 Commit 仍 > 100 MB"，没有触发；65 MB 是否可达属 G1 裁决，**必须写 ADR**，不在 Phase 1 里悄悄挪 |

## 8. 没做

- **65 MB 线的裁决**：属 G1（见上）。已知结构性下限：GL + winit + egui ≈ 90 MB。
- **多屏 / 混合 DPI 下的映射行为**：映射与 DPI 无关，未单独测；跨 DPI 算 C3。
- **8 小时泄漏**：本批只做静态footprint，长跑仍归 G1 的"连续 8 小时无增长"。
- **lite 档**：`lite` 影响的是绘制路径，字库同一份，未单独跑。
- **字体热替换**：映射后若系统更新字体文件，本进程继续用旧页（文件映射持有文件对象）。下次启动才换——与"字体更新需要重启"一致，不算缺陷。

## 9. 验证汇总

| 项 | 结果 |
|---|---|
| `scripts/check-native-font.py` | 11/11 |
| 变异（强制堆路径） | 3 项转红，`RESULT: FAIL` |
| `cargo test --workspace --release` | 249 项（新增：映射逐字节一致、失败阶段可辨、策略三条） |
| `check-native-architecture.mjs` | 通过（5 crates；`spike.rs` 792 行回到 800 内） |
| 回归门禁 | notes 25 / pin 11 / tools 8 / manager 19 / switcher 22 / edge 23 / size 11 / recover 16 / drag 9 / store-schema 24，全绿 |
