# 2026-11-27 · 主题 token 与 launcher 视觉

> 状态：`completed`（2026-11-27）
> 上游：[2026-11-24-native-runtime-redesign.md](../active/2026-11-24-native-runtime-redesign.md) §5.1 / §5.1.1 / §5.4
> 前一批：[2026-11-27-c2-launcher-productized.md](./2026-11-27-c2-launcher-productized.md)

## 1. 要解决的问题

`hub-ui::Palette` 有 7 个色槽：`panel` / `panel_strong` / `border` / `ink` / `ink_soft` / `ink_subtle` / `accent`。7 个槽要表达 9 种角色，于是每个"需要第 8 种颜色"的调用点自己做算术：

| 位置 | 系数 | 语义 |
|---|---|---|
| `launcher.rs` 未选中卡片 | `panel_strong.gamma_multiply(0.72)` | 卡片底 |
| `manager.rs` 列表行 | `panel_strong.gamma_multiply(0.55)` | 行底 |
| `recover.rs` 输入框 | `panel_strong.gamma_multiply(0.55)` | 同上 |
| `switcher.rs` 未选中行 | `panel_strong.gamma_multiply(0.6)` | 同上 |
| `recover.rs` 主按钮 | `accent.gamma_multiply(0.28)` | 强调色填充 |

四个文件、四个系数、一个意思。这不是调色板的问题，是主题的问题，而它以一种具体的方式坏掉：

**乘数没有方向。** `gamma_multiply(0.72)` 只会变暗。在这个暗色面板上，"比输入框暗 28%"碰巧读作"低一档"，所以这些数字通过了评审。换成亮色主题，同一行会把卡片画得**比它脚下的页面还暗**——那是一个洞，不是一张卡片。

## 2. 三个决定

### 2.1 派生一律"朝文字色移动"，不乘系数

新增 `lift(base, ink, t) = mix(base, ink, t)`。方向由 `ink` 决定：暗色主题里文字是亮的，抬起的表面就更亮；亮色主题里文字是暗的，同一个函数把表面压暗。一条规则，两个方向都对，调用点拿不到系数。

这条性质有单测，而且是拿一个手工构造的亮色主题验证的（`a_lifted_surface_moves_towards_the_text_colour`）：如果 `lift` 里写死了方向，那条断言会红。

### 2.2 角色拆开，不复用槽位

`panel_strong` 之前同时是搜索框、卡片、选中的 tab 和键位 chip。想让搜索框和卡片长得不一样，就得改两处不相干的界面。现在：

| 层 | 字段 | 消费方 |
|---|---|---|
| 页面 | `canvas` | 面板底、笔记窗底 |
| 卡片静息 | `surface_soft` | 动作卡、列表行 |
| 抬起一层 | `surface` | 搜索框、选中 tab、chip |
| 悬停 / 按下 | `surface_hover` / `surface_active` | 卡片与行 |
| 遮罩 | `scrim` | 恢复对话框（原 `Color32::from_black_alpha(96)`） |

文字三级加 `ink_disabled`，线两级（`border` / `border_strong`）加 `focus_ring`，强调三件（`accent` / `accent_soft` / `on_accent`），状态两件（`danger` / `danger_soft`）。圆角收成 4 档（`chip 4 / row 8 / card 12 / panel 14`），此前散落着 3、4、7、8、10、11 六个值。

### 2.3 派生只在构造函数里发生一次

用户能自定义快速笔记的 7 个颜色（`NoteColors`），所以每个派生 token 都必须能从这 7 个算出来——主题文件不可能要求用户填 15 个值。`standard()` / `lite()` / `from_colors()` 全部走同一个 `build()`，不存在"某个构造路径少算了派生值"。

`on_accent`（solid accent 填充上的文字）用**对比度**选，不用亮度阈值：出厂 accent `#7EA5D6` 用亮度阈值会选白字，落在 2.2:1；选面板色是 7.1:1。这条也有单测，并且钉了反方向（深 accent 必须翻成亮字）。

## 3. 面板视觉

改之前，选中行和悬停行是同一张图，除非你去量那 0.5 px 的描边差——而用户打字时不会去量。

| 状态 | 填充 | 描边 | 附加 |
|---|---|---|---|
| 静息 | `surface_soft` | `border` | — |
| 悬停 | `surface_hover` | `border_strong` | — |
| 选中 | `accent_soft` | `accent` 1.5 px | 左缘 2 px accent 竖条 |

其余一起改的：

- **搜索框有聚焦态**：拿到键盘时描边变 accent 1.5 px、放大镜同色。此前有没有键盘画得一模一样。
- **放大镜是画的**，不是打的字：CJK 回退字体没有 `⌕` 的字形，打出来是豆腐块。
- **分组标题带计数**（`动作 5` / `最近笔记 3`）：只写"最近笔记"会让人猜第五行是列表结束还是面板结束。
- **页脚键位画成 chip**：裸文本读作说明文字，chip 才读作"这个键可以按"。chip 之间用 `layout_no_wrap` 实测宽度排布，不用固定步进。
- **空结果给下一步**：`没有匹配的结果` 下面补一行 `试试 笔记 / 工作台 / 新建`。一个只有输入框的面板里，"什么也没找到"是死路。
- **字体 hash 与快捷键 monospace**：`Ctrl+Alt+N` 只有等宽才读作一个 token。
- **一处重绘**：聚焦环读的是上一帧的 `keyboard`，所以它是"晚一帧"的。拿到键盘时显式请求一次重绘，否则面板会一直停在无环的样子直到用户开始打字。

## 4. 顺带修的两个布局缺陷

### 4.1 第五张卡不能孤零零一行

440 宽在阈值 96 下是 **4 列**，而动作恰好 5 个：4 + 1，第二行一张卡悬在左边，读起来像缺图。阈值提到 **130** → 3 列（3 × 130.67），5 个动作排成 3 + 2。4 列要到 576 逻辑宽才回来。

96 这个数是按"文字不被裁掉"算的（`Ctrl+Alt+N` 在 96 宽里左右各剩 13），不是按"这一行看起来成立"算的。

### 4.2 列数此前依赖显示器缩放

`place` 拿到的工作区是**物理**像素，而 `padding` / `gap` / `min_cell_width` 是**逻辑**值。`columns_for` 直接相减，等于把两种单位混在一起算；答案"看起来对"只是因为默认阈值恰好等于 100% 下的边界值（440 → 96 → `>=` 通过）。在 200% 下同一段代码算出的格子是 172 物理 = 86 逻辑，却拿 86 去和 96 比——结论碰巧相同，纯属运气。

修法是 `PanelSpec` 增加 `scale`，`columns_for` 统一除一次。新增单测 `the_column_count_does_not_depend_on_the_monitor_scale`：440 逻辑在 100% / 150% / 200% 下必须给出同一个答案。

## 5. 证据

| 检查 | 结果 |
|---|---|
| `scripts/check-native-theme.py` | **17/17**，连跑 3 次 |
| 采样：页面 | `#181B21` == `Theme::standard().canvas` |
| 采样：搜索框 / 聚焦环 | `#21252D`（surface）/ `#7EA5D6`（accent） |
| 采样：选中卡 / 静息卡 / 悬停卡 | `#28313E`（accent_soft）/ `#1D2128`（surface_soft）/ `#2D3139`（surface_hover） |
| 采样：选中笔记 / 静息笔记 | `#28313E` / `#181B21` |
| lite 主题 | 页面 `#14161B`（比 standard 暗），卡片仍高于页面 |
| `cargo test` | **274 项**（app 78 / hub-core 114 / hub-editor 18 / hub-ui 48 / hub-win 16） |
| 回归 | launcher 25/25、notes 25/25、manager 19/19、switcher 22/22、store-schema 24/24 |
| `check-native-architecture.mjs` | 通过（最大文件 `launcher.rs` 696 行） |
| `check-doc-budget.mjs` / `check-plan-lifecycle.mjs` | 通过 |

门禁为什么读像素而不是只靠单测：主题自己的单测能证明"`surface_soft` 落在 `canvas` 与 `surface` 之间"，但证明不了面板**画的是这些值**。一个传了 `Color32::from_rgb(30, 30, 30)` 的调用点会通过 crate 里所有单测，然后画出一片灰。

门禁的采样矩形也不由它自己重算：`launcher_ready` 之后，每帧布局变化时产品打一行 `launcher_layout canvas=… search=… selected_card=…`，门禁只解析。这样 `hub-ui` 里挪一个常量，门禁跟着挪；如果门禁自己复制一份间距，它会在面板画到别处之后继续通过。

## 6. 没做的

- **窗口外阴影**。面板窗口不透明，画在窗口内的阴影只会是内发光。真投影需要 `with_transparent(true)` + DWM 逐像素合成，那会把残留合成成本和圆角锯齿一起带进 reveal 路径，属于另一批。
- **hover 渐变**。`animate_bool` 会让面板在静止时也持续请求重绘，和"停放态不重绘"的判据冲突，收益不值这个代价。
- **亮色主题**。`lift` 的方向已经支持，但没有第二套基础色，也没有系统主题跟随。
- **快速笔记 / Manager 的视觉重排**。它们换用了新 token（不再有 `gamma_multiply`），版式没动。

## 7. 变更记录

| 日期 | 说明 |
|---|---|
| 2026-11-27 | `hub-ui::Palette` → `Theme`：7 槽扩到 21 个语义 token + `Radii`，派生集中在 `build()`，删掉 5 处 `gamma_multiply` 与 1 处手写 alpha；面板三态、聚焦态、分组计数、chip 页脚、空结果引导；列数阈值 96 → 130（4+1 孤卡 → 3+2），`PanelSpec::scale` 修掉物理/逻辑混算；新增 `check-native-theme.py`（17 项，采样真实像素） |
