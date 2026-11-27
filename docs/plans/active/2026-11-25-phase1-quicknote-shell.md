# Phase 1 · 快速笔记壳第一批：窗口与几何 + 标题条

> 状态：`active` —— 2026-11-25 起草并开工，1a 已落地（见 §9）。
> 判据来源：[./2026-11-25-native-quicknote-parity.md](./2026-11-25-native-quicknote-parity.md) §3.1–3.2（Q1–Q11）
> 上游：[G0 评审](../completed/2026-11-25-g0-review.md)（已批准）、[总设计](./2026-11-24-native-runtime-redesign.md) §5.1

## 0. 一句话

把"S2 的裸内容区"变成一个**看起来像快速笔记**的窗口：32 px 三段式标题条 + 底部文本岛 + 可拖动 + 可切置顶 + 尺寸与最小尺寸合规。

## 1. 本批覆盖到哪

| 条目 | 本批 | 说明 |
|---|---|---|
| Q1 无边框无系统标题栏 | ✅ | S1 已有 `with_decorations(false)`，本批补"可判"证据 |
| Q2 默认 400×300，取自设置 | ✅ | 新增 `quick_note.width/height`，键名与 Tauri 版一致 |
| Q3 最小 150×60 | ✅ | `with_min_inner_size`，交由 winit 落到 `WM_GETMINMAXINFO` |
| Q4 光标所在屏 + 四边夹紧 | ✅ | 复用 `hub-core::place` 的夹紧算法，锚点改为笔记尺寸 |
| **Q5 关闭即销毁，新建是全新窗口** | ✅ **1b.2** | 一个进程、每笔记一个 viewport；关闭只销毁那一个窗口，见 §8.5 |
| Q6 左槽显不透明度 / `GHOST` | ◐ | 文本与状态机本批做；**真实整窗透明度**在 1c（需 layered window） |
| Q7 中槽 `•••` 开关工具面板 | ✅ | 开合与高亮；面板内容留 Phase 2 |
| Q8 预览按钮 | — | C3 已延后 Phase 3，挂起 |
| Q9 右槽置顶可开可关 | ✅ | 初始置顶；点一次取消；再点恢复 |
| Q10 标题条任意处可拖动 | ✅ | `ViewportCommand::StartDrag`，三个槽位分别可拖 |
| Q11 三段永不重叠 | ✅ | 三段式布局 + 150 宽的单元测试 |

Q6 只做一半是**有意**的：百分比数字对不对（`opacity_label`）与窗口真透明是两条链，前者是纯函数、可以单测，后者要动 `hub-win` 的合成路径（`SetLayeredWindowAttributes` / DWM）。混在一批里，出问题时无法区分"数字错"还是"合成错"。

## 2. 批次拆分与理由

| 批 | 内容 | 为什么单独 |
|---|---|---|
| **1a**（本批） | 单窗口壳：几何 + 标题条 + 拖动 + 置顶 + 最小尺寸 | 全部可在**一个窗口**内验证，不需要改进程/窗口模型 |
| **1b.1** | 键位与焦点：Esc / Ctrl+D 关闭、撤销重做、置顶、工具面板、打开即聚焦 | 验证前提与 1a 相同——**按键看结果**，不需要改窗口模型 |
| **1b.2** | 多笔记窗口 + 关闭即销毁 + `Ctrl+Alt+N` 新建（✅ 见 §8.5） | 要引入 viewport-per-note，`Q5` 才能被真正满足 |
| **1b.3** | 托盘常驻 + 其余全局键（`Ctrl+Alt+D/H/M`）+ 关完窗口不退进程（✅ 见 §8.6） | 需要托盘生命周期；在此之前"关完最后一个笔记就退出"是唯一不自欺的选择 |
| **1c** | 整窗透明度 + 幽灵模式（Q28/Q29） | 需新增 `hub-win` 的合成路径；与 1a 的纯 UI 工作无耦合 |
| **1d** | `Ctrl+K` 切换器（Q25–Q27） | 需要 `notes.db` 的最近列表，属 Phase 2 数据层前置 |

设计文档原本把 Q1–Q11 当一个批次。拆开的依据是**验证前提不同**：1a 只需要"看窗口"，1b 要"看进程树"，1c 要"看合成结果"。把它们绑在一起，任何一项没通过都会拖住其余十项。

## 3. 决定

**D1 —— 拖动与置顶走 egui 视口命令，不新增 win32 FFI。**
`ViewportCommand::StartDrag`、`WindowLevel`、`Close` 在 egui 0.31 都可用，winit 已把它们翻成 `WM_NCLBUTTONDOWN(HTCAPTION)` 与 `WM_GETMINMAXINFO`。走这条路的好处不只是省代码：`hub-ui` 被门禁禁止依赖 `windows-sys`，UI 自己发起拖动才能让"整条标题条都是拖动区"成为一个纯 UI 属性；否则每个槽位都要回到 app 层注册命中区，Q11 的"三段不重叠"就变成三方约定而不是布局保证。

**D2 —— 尺寸口径：逻辑像素存设置，物理像素算布局。**
设置里是 `quick_note.width/height`（默认 400/300，与 Tauri 版 `QuickNoteSettings` 同名同值），乘光标所在显示器 `scale` 得到物理尺寸再进场。理由同 §5.1.1：面板缓冲按物理像素分配，反过来从物理除回去会有取整误差，表现为"打开时差 1 px 再跳一下"。

**D3 —— 最小尺寸 150×60 逻辑，交给 `with_min_inner_size`。**
不自己子类化 `WM_GETMINMAXINFO`：winit 已经处理了 DPI 换算与"最小尺寸不得小于窗口边框"这类边角。自己写会让"拖到最小时内容裁切"与"拖不动"两种现象无法区分。

**D4 —— 位置：光标所在显示器，锚点按笔记尺寸偏移，四边夹紧。**
`place()` 现在按"光标水平居中、垂直 1/3"放 440×860 的竖版面板。笔记是 400×300 的横向小窗，同样的偏移会让它盖住光标正下方。所以 `place_note()` 用同一套 `clamp_axis`，但锚点改为"左上角落在光标右下 8 px"，并在靠近工作区边缘时整窗翻到光标另一侧。夹紧仍是硬约束：**任何情况下整窗都在工作区内**。

**D5 —— Q5 与"温实例"冲突，本批按 Q5 走。**
总设计 §5.1 写"关闭后保留 1 个温实例供下次秒开"，parity Q5 写"关闭即销毁，新建是全新窗口"，两条在可观测层面互斥（进程树窗口数回落 vs 不回落）。
裁决：**Q5 优先**。依据是门禁强度不同——G1 的 60 ms 只约束 launcher（`hotkey → 面板出现`），快速笔记没有这条门禁。为一个没有指标约束的窗口保留热实例，换来的"秒开"无法被验收，却要让用户承担"关掉的窗口还在"的不确定感。温实例若将来真需要，前置条件是补一条快速笔记的打开 p95 门禁，而不是先做优化再补判据。

**D6 —— 标题条 32 逻辑像素，三段用布局分配而不是绝对定位。**
现版曾用"绝对居中"，窄窗时压住左侧图标，为此重排过一次（`QuickNoteToolbar.tsx` 的 `STRIP_STYLE` 注释）。这里直接用 `左右定宽 + 中间自适应`：左槽与右槽按内容宽度固定，中槽吃掉剩余空间并居中绘制。Q11 的判定因此是结构性的，而不是"目测没压住"。

**D7 —— 1a 的 Esc = 退出进程，不伪装成"关闭窗口"。**
没有托盘之前，"关闭窗口"只能是退出进程。写日志时用 `note_close`，并在字段里带 `windows_before/after`，1b 接上托盘后这里改成"销毁 viewport、进程存活"，日志字段不用改——**判据先于实现存在**，1b 才不会顺手把 Q5 判成"已满足"。

> 2026-11-25 更新（1b.2）：Esc 现在只销毁**该** viewport，其余笔记存活；托盘仍未到位，所以最后一个笔记关闭时进程退出（§8.5）。日志字段从 `windows_before/after` 落成 `open_notes=`，因为原始窗口总数里混着 winit 事件窗口和输入法宿主，不是笔记。

**D8 —— 复用 S1 的窗口路径，不重写。**
创建顺序仍是"`with_visible(false)` → `find_by_title` → 物理像素 `place` → `show_without_activation`"。S1 的 p95 就是在这个顺序下测出来的；1a 只改内容，不改创建路径，这样"标题条上线后热唤起变慢"的怀疑可以立刻排除。

## 4. 落点

| 文件 | 变化 | 为什么在这 |
|---|---|---|
| `hub-core/src/layout.rs` | 新增 `NoteSpec` / `place_note` / `MIN_NOTE_*` / `STRIP_HEIGHT` | 纯几何，必须能在没有窗口的测试里跑 |
| `hub-core/src/settings.rs` | 新增 `quick_note: QuickNoteSettings` | 键名与 Tauri 版对齐，将来共享配置文件不用翻译 |
| `hub-ui/src/quicknote.rs`（新） | 标题条绘制、`opacity_label`、`strip` 返回内容区 | 布局与文案属于 UI 层；`opacity_label` 是纯函数，单测覆盖 |
| `app/src/note.rs`（新） | `NoteApp`：视口命令、文本岛同步、Esc、日志 | 组合根；`spike.rs` 已 793 行，加不进去 |

**为什么不是改 `spike.rs`**：它离 800 行硬顶只剩 7 行。把标题条塞进去必然触发又一次"为了过门禁而抽文件"，而抽出来的边界是"哪段代码恰好放不下"，不是职责。新开 `note.rs` 让边界是"spike 是测量件，note 是产品件"，这条边界在 T-Gate 0 之后才有意义——那时 spike 整个删除，`note.rs` 留下。

## 5. 怎么验收

```powershell
# 构建
cargo build --manifest-path native/Cargo.toml --release -p app

# 打开笔记壳（不注册热键，直接可见）
native\target\release\easyflowhub-native.exe --note --log $env:TEMP\note.log
```

| 条目 | 动作 | 期望 |
|---|---|---|
| Q1 | 看窗口四边 | 无系统标题栏、无系统边框、无最大化/关闭按钮 |
| Q2 | 首次打开；把设置改成 `{"quick_note":{"width":500,"height":400}}` 后重开 | 日志 `note_window` 的物理尺寸从 `600x450` 变 `750x600`（@150%） |
| Q3 | 从右下角往左上拖到不能再小 | 停在 150×60；标题条三个槽位仍可点，文本岛仍能输入 |
| Q4 | 在两台显示器上各打开一次；把光标放到屏幕角上 | 整窗在该屏工作区内，不被任务栏遮住 |
| Q6 | 本批**不验**，只有单测 | `opacity_label` / `step_opacity` / `toggle_ghost` 共 4 项单测；窗口真实透明度在 1c，之前标签恒为 `100%` |
| Q7 | 点 `•••` | 面板开/关，按钮高亮跟随 |
| Q9 | 点置顶图标 | 图标高亮变化；用别的窗口盖它，第一次被压住，第二次回到最上 |
| Q10 | 在左/中/右三个槽位分别按住拖动 | 窗口跟随；按在按钮上不拖动（`•••` 与置顶仍可点） |
| Q11 | 把窗口拖到 150 宽，逐个点三个槽位 | 三次都命中预期，左槽文本不压住中槽 |

Q2 的判定要写清：`height` 与 `width` **独立**取设置，不按比例缩放。把"500 宽"写成"高度自动变 375"等于偷偷加了等比规则，而现版没有这条规则。

## 6. 明确不做

- 工具面板内容（`•••` 只开合一个占位面板，条目留 Phase 2）
- 真实笔记读写（`notes.db` 属 Phase 2；1a 的文本岛仍是内存文档）
- Markdown 装饰、图片、预览
- 尺寸/位置记忆（C5 已延后）
- 多窗口（D5）

## 7. 风险

| 风险 | 影响 | 缓解 |
|---|---|---|
| `StartDrag` 与文本岛抢鼠标 | 拖不动或拖了不该拖的地方 | 拖动区只覆盖标题条；内容区是子窗口，egui 收不到它的鼠标事件 |
| 150 宽时左槽文本压住中槽 | Q11 直接不通过 | 左槽按最大文本宽度（`GHOST` / `100%`）定宽，中槽用剩余宽度；单测钉 150 宽 |
| `with_min_inner_size` 在高 DPI 下按逻辑像素解释 | 最小尺寸变成 225×90 物理，看起来"拖不小" | 验收时记录实际拖到的最小物理尺寸；若与 150×60×scale 不符，说明 winit 换算有问题，记缺陷而不是改判据 |
| 1a 的 Esc 退进程被误当成产品行为 | 1b 时忘记实现"销毁但仍常驻" | D7：日志字段 `windows_before/after` 先定，1b 只改实现 |

## 8. 实施记录（1a）

写代码时撞到两个真缺陷，都不是"设计没想到"，而是**第一帧的假设错了**。

| # | 现象 | 根因 | 修法 |
|---|---|---|---|
| 1 | `set_ppp native_ppp=1.00 target=1.50 effective=1.00`，随后 ppp 变 **2.25** | 在 `CreationContext` 里调 `set_zoom_factor`：那时 `pixels_per_point()` 还报系统默认 1.0，于是申请 1.5 倍，而显示器本来就是 150% | 挪到 `update` 的第一帧（`ppp_done` 只做一次）。这跟 spike 里记过的"别用 `set_pixels_per_point` 直接设"是同一个坑的另一种形态 |
| 2 | `island_created rect=15000x14928` | 首帧 `ctx.screen_rect()` 是 **10000 px 的占位值**；按 ppp 换算后子窗口变成一万多像素宽 | 新增 `hub_win::window::client_size()`，把内容矩形与 OS 客户区求交；框架的自我尺寸只作参考，Windows 的 `GetClientRect` 才算数 |

两条都在 `--note` 实测里暴露，测试抓不到（一个是时序、一个是占位值），所以 §5 的验收表里 Q2 那条带实测数字——**用日志证明，不用"我看了没问题"**。

| 条目 | 1a 结论 |
|---|---|
| Q2 | 实测：默认 `600x450` 物理（400×300 逻辑 @150%）；把设置改成 500×400 后 `750x600` |
| Q3 | `min=225x90` 物理随 150% 缩放，符合"150×60 逻辑" |
| Q4 | 光标在 `1516,1969`（屏幕下缘）时窗口落在 `+1524+1361`，即上翻而非被任务栏截断 |
| 其余 | 见 parity 清单 §3 的 ◐ 标记 |

**与计划有出入的一处**：Q7 原写"点面板外部也关"。1a 只做了按钮开合，外部点击关闭没做——它需要一层全窗点击拦截，而内容区被 RichEdit 子窗口占着，egui 收不到那里的点击，得等 1b 或专门处理。已记在 Q7 的现状列，不当作"已实现"。

## 8.1 目视打磨（1a.1，2026-11-25 下午）

用户在 150% 屏上跑 `--note` 后给了三点：**UI 需要优化**、**换行后光标不可见、输入后又出现**、**希望光标加粗改蓝并进主题配置**。前两条是缺陷，第三条一半是缺陷一半是平台限制。

### 三个缺陷

| # | 现象 | 根因 | 修法 | 证据 |
|---|---|---|---|---|
| 3 | 字号比记事本大一圈，一行只放得下 20 来个汉字 | `font_pt: 11.0 * pixels_per_point()`：点是设备无关单位，乘 1.5 等于要 16.5 pt | 固定 11.5 pt（`appearance.font_pt`） | 150% 下字形高 22 物理 px（≈11 pt），改前约 30 px |
| 4 | 正文压在标题条下沿，岛比可用区域高 16 px | `PixelRect::from_logical` 只缩放宽高、**原点直接透传**：`y=32`（逻辑）被当成物理 | 四个值同单位缩放 | `island_created rect=600x402+0+32` → `+0+48` |
| 5 | **换行后光标消失，输入一个字又出现** | 文本岛左右内边距为 0：行首插入符的 x=0，宽度 1 px 的首列被客户区边缘裁掉；字符把它推到 x>0 就又看得见 | `EM_SETRECT` 给文本 12/10 逻辑内边距；`caret_width` 默认 2 逻辑 | 空文档连拍 8 帧：改前 **0/8** 帧能看到光标；改后 4/8 帧在 x=18 物理处出现 3×40 px 竖线（闪烁周期一半） |

第 5 条的 A/B 是把光标放到**空行行首**拍的——文字行首有字形干扰，"那里有竖线"可能只是"版"字的第一笔。空行没有字形，竖线只可能是插入符。

### 视觉改动

| 位置 | 改前 | 改后 |
|---|---|---|
| 正文内边距 | 0 | 12 / 10 逻辑（配置项） |
| 行距 | 16.5 pt 字体的自然行距，约 2.3 em | 1.0 倍（实测 26.7 逻辑 px，字 15 px）；可配 0.8–2.0 |
| `•••` | `•••` 字形，随 CJK 字体回退成另一个光学尺寸 | 三个 3.2 px 圆，画出来；颜色改为旧 UI 的 `color-mix(… 72%, transparent)`（含 0.32 下限） |
| 置顶图标 | `📌` emoji：egui 字体栈没有彩色 emoji，渲成一个白底黑块的方框（截图右上角那个方块） | 先自己拼了帽/身/尖的图钉 —— **这一版是错的**，1a.2 已改为照描旧 UI 的书签（§8.2） |
| 右侧滚动条 | `WS_VSCROLL` 的系统滚动条，深色面板上是一条 17 px 白槽 | 移除。滚轮与 `EM_SCROLLCARET` 仍在，位置指示留待自绘 |
| 右槽对齐 | 按钮在 44 px 槽里居中，右边缘留 10 px | 贴 `EDGE_PADDING`（12 px），与左槽标签对称 |

### 主题配置

`quick_note.appearance` 新增（`hub-core::settings`）：

```json
{
  "quick_note": {
    "appearance": {
      "font_family": "Microsoft YaHei UI",
      "font_pt": 11.5,
      "line_spacing": 1.0,
      "padding_x": 18,
      "padding_y": 18,
      "caret_width": 2,
      "colors": {
        "panel": [13, 21, 34],
        "strip": [31, 46, 68],
        "border": [38, 55, 72],
        "ink": [220, 230, 244],
        "ink_soft": [175, 190, 210],
        "ink_subtle": [138, 155, 178],
        "accent": [134, 171, 220]
      }
    }
  }
}
```

默认值就是 Tauri 那份 `--manager-*` token 的合成结果，所以不填这段时观感与旧版一致。`serde(default)` 两层都有，只写 `colors.accent` 也不会把其它颜色清成黑色——有单测钉住。

`caret_width` 有一个**系统级副作用**，必须写清楚：Windows 没有 per-window 插入符宽度，RichEdit 的 caret 尺寸来自 `SPI_GETCARETWIDTH`。所以这个值是通过 `SPI_SETCARETWIDTH` 写到系统里的，进程退出（`Editor::drop`）时恢复原值。副作用有二：① 生效期间其它应用新建的插入符也会变粗；② 进程被强杀时恢复不会执行，系统会停在 2 px——这是外观残留，不是数据风险，权衡后接受。不想要就设 0。

### 没做：蓝色插入符

RichEdit 的插入符是 **XOR 反色**：系统先 `InvertRect` 再画，颜色完全由背景决定，没有任何 API 能指定它。想让它呈蓝色只有一条路——**关掉系统插入符、自己画一个**：需要叠一个子窗口、接管闪烁（`GetCaretBlinkTime`）、跟随滚动与选区、并在 IME 组合期间保持位置正确。这条路的失败模式是"比现在的白光标更糟"（双光标、闪错相位、组合时错位）。

所以本轮交付的是"看得见的加粗光标"，蓝色留作独立批次，由用户决定是否值得这个代价。记在这里而不是悄悄跳过。

## 8.2 置顶图标返工（1a.2）

用户："置顶怎么不用和旧 UI 一样的图案，旧 UI 还是经过仔细调试的。"

对的，而且错的不只是"换了个图案"。1a.1 在修 `📌` 的渲染 bug 时，顺手把一个**已经调过的图标**也换掉了——修缺陷的范围滑到了改设计。旧 UI 的置顶按钮根本不是图钉，是**书签**：

| 项 | 旧 UI（`QuickNoteToolbar.tsx`） | 1a.1（错） | 1a.2 |
|---|---|---|---|
| 图形 | `M5 5a2 2 0 012-2h10a2 2 0 012 2v16l-7-3.5L5 21V5z`（`viewBox 0 0 24 24`） | 自拼图钉（圆帽 + 梯形身 + 三角尖） | 照描同一条路径 |
| 显示尺寸 | 16×16 | 约 13×10 | 16×16 |
| 描边 | `strokeWidth 2`，`linejoin round` | 无描边，纯填充 | 2 单位 = 16 px 下 1.33 px，顶点补圆做 round join |
| 两态 | `fill:none` + `--quick-muted` #8a9bb2 / `fill:currentColor` + `--quick-accent` #86abdc | 只有颜色变化，形状不变 | `fill` 随状态切换，配色照 token |
| 按钮 | 28×28，`background:none`、无边框 | 24×24，hover/激活时画 `panel` 圆角底板 + 1 px 边框 | 28×28，无底板 |

最后一行是第二处偏差，同一次改掉：那个圆角底板在旧 UI 里不存在，16 px 字形套一个方块，读起来就是"另一个控件"。

`•••` 也一并回到旧 UI 的配色规则：`color-mix(in srgb, --quick-text 72%, transparent)`，并保留它的 `0.32` 下限（ghost 模式下图标仍要点得中）。注意 1a.1 用的是 `ink_subtle` 全量，比旧 UI 暗一档。

实现上有一处 egui 限制必须绕：`Shape::Path` 的填充只对**凸多边形**有定义，而书签底部的 `(12, 17.5)` 是凹点，直接填会渲染成错误形状。所以填充拆成三块共边的凸多边形（上体 + 左右两个三角），面积完全一致。

## 8.3 标题条两侧留白（1a.3）

1a.2 交付时留了一处"我这边两边都 12 px、旧 UI 是 16 / 14"的差异并询问，用户答"两边都照旧 UI 分开"。旧 UI 的数值不是随便取的，它们来自两层嵌套 padding：

| 位置 | 旧 UI 的来源 | 数值 | 本仓库常量 |
|---|---|---|---|
| 标签左缘 | strip `paddingInline: 8` + 左槽 `paddingInline: 8` | **16** | `LABEL_INSET` |
| 按钮框右缘 | strip `paddingInline: 8`（按钮是最后一个 flex 子项） | **8** | `BUTTON_INSET` |
| 图标可见右缘 | 上项 + 按钮 28 与图标 16 的 6 px 居中余量 | **14** | `ICON_INSET`（推导，非手填） |

所以左右相差 2 px 是**设计如此**，不是舍入误差。改成 12/12 会让标签左移 4 px、图标右移 2 px，两头同时偏离。

1a.3 之前 `SIDE_SLOT_WIDTH` 是 44，配上 16 的标签内缩只剩 28 px 文字位；旧 UI 的左槽声明的是 `minWidth: 46`，而 `GHOST` 在 10 px + `letterSpacing 0.22em` 下约 44 px 宽，flex 会把它撑到 60 左右。本仓库改成 60 是同一个数的另一种写法，不引入 flex 计算。

实测（150% 屏，`--inset 2`，600 px 窗口截图的逻辑坐标）：标签墨迹起于 16，图标墨迹右缘距右边 14，差值 2 逻辑 px，与常量一致。

**一个如实记录的边界**：最小窗宽 150 px 时槽宽被压到 `width/3 = 50`，而 `GHOST` 在 11 pt 下实测 39 px 宽（59 物理 px，截图 24..82 列），16 + 39 = 55 会**越出左槽 5 px**。它没有压到 `•••`——三点左边在 61，仍有 6 px 余量——所以 Q11（三段不重叠）成立，但"标签在自己槽内"在最小窗宽下不成立。`the_ghost_label_clears_the_dots` 把这两个数都钉住了：默认 400 px 时必须装得下，最小 150 px 时必须让开三点。

**顺带修掉的一处错位**：标签到 16 之后，正文内边距 12 / 10 就露馅了——正文比标签**左** 4 px，而旧 UI 的 `EditorTextarea`（`quick-note` 变体）是 `padding: 18`，正文比标签右 2 px。既然这一轮的标准是"照旧 UI"，两个数一起改成 18 / 18，否则只挪标签会让那条 4 px 的错位从"不显眼"变成"看得出"。

实测（`--inset 2`，600 px 窗口，逻辑坐标）：标签墨迹起于 16.7（笔位 16，"1" 有 1 px 身位），正文墨迹起于 **18.0**，首行墨迹距标题条下沿 24（18 内边距 + 6 行首空隙）。

## 8.4 键位与焦点（1b.1）

1b 原本是一整块"多窗口 + 托盘 + 关闭即销毁"。拆成两小步先做键盘，理由和 1a 与 1c 分开时一样：**验证前提不同**。键盘只要"按一下，看有没有发生"，和 1a 同属单窗口；多窗口要数进程树里的窗口和句柄。绑在一起时，任何一个键位不对都会拖住整个窗口模型改造。

### 键位表

照 `useQuickNoteShortcuts.ts` 的**顺序**实现，因为顺序就是规格：

| 键 | 动作 | 顺序为什么重要 |
|---|---|---|
| `Esc` | 关闭 | 不看修饰键（旧版只判 `event.key === 'Escape'`） |
| `Ctrl+Z` | 撤销 | 放在最前，避免被后面的关闭逻辑截走 |
| `Ctrl+Shift+Z` | 重做 | 必须排在 `Ctrl+Z` 之后、关闭之前 |
| `Ctrl+T` | 置顶开关 | 旧版 `shortcutConfig.toggleAlwaysOnTop` 的默认值 |
| `Ctrl+D` | 关闭 | 旧版是 `commandKey && key === 'd'`，**不检查 Shift** |
| `Ctrl+.` | 工具面板 | 旧版判 `event.key === '.'`；`Shift+.` 是 `>`，所以不触发 |

表本身是纯函数（`app/src/keys.rs`），8 个单测钉住上面每一条，包括两个看起来像 bug 的行为：

- `Ctrl+Shift+D` 会关窗。旧版把复制行也绑在 `Ctrl+Shift+D`，而窗口级 keydown 在 capture 阶段先跑；我们照抄了关闭判断，所以**在做复制行之前必须先改这里**，否则复制行永远收不到键。
- `Ctrl+Shift+.` 不开面板。不是漏写，是旧版按字符比对的直接结果。

### 两条输入路径

焦点在文本岛时，Windows 把 `WM_KEYDOWN` 发给子窗口，winit 收不到；点了标题条之后，焦点回到主窗口，岛又收不到。两条路径喂同一张表，且互斥（一个键只可能走其中一条），所以不需要去重。这一点写进了模块注释——否则下一个人会加一个 "already handled" 守卫，而那个守卫会让其中一条路径永久失效。

撤销/重做**没有**交给 RichEdit 原生处理，而是统一走 `EM_UNDO` / `EM_REDO`。原因就是第二条路径：焦点在标题条时没有控件在链路里，原生 Ctrl+Z 不会发生。两条路径行为一致，比"省一次调用"重要。

### 中文输入法会改写虚拟键

这是本批唯一一个"代码写了但没能完整验证"的地方，如实记下。

| 事实 | 证据 |
|---|---|
| `Ctrl+.` 到达 subclass 时 `wParam` 是 **0xE5**（`VK_PROCESSKEY`），不是 0xBE | 临时探针日志 `RAW vk=0xE5 ctrl=true`，2026-11-25 |
| `ImmGetVirtualKey` 在 TSF 输入法（搜狗 / 微信）下返回 0xE5 本身 | 同一轮探针；TSF 不走 IMM32，这是文档里没有明说的边界 |
| 可用的还原路径是 `lParam` bits 16–23 的扫描码 + `MapVirtualKey` | MSDN 的 `WM_KEYDOWN` 定义；代码里两条路都写，先 IMM 后扫描码 |
| 合成输入（`keybd_event`）的 `lParam` 扫描码是 **0**，因此这条路径**无法用脚本验证** | `scan=0` 时直接返回原 vk，脚本按 Ctrl+. 没有任何反应 |

所以 Q17（`Ctrl+.` 开面板）在代码上是"已实现"，在判定上只能记 `◐`：真实键盘一定带扫描码，但本轮没有人工按过。用真实键盘按一次、日志出现 `note_key action=ToggleTools`，就可以改成 `✓`。

### 实测（150%，日志片段）

```
editor_focus focused=true caret=(0, 0)          ← Q19：打开即聚焦，空文档光标在末尾
note_key action=TogglePin
note_pin pinned=false                            ← Q16：Ctrl+T 取消置顶
note_key action=Close
note_close reason=shortcut windows_before=4 windows_after=3   ← Q14：Ctrl+D
note_key action=Undo / note_history undo can=true len=0        ← Q15：abc 被撤销
note_key action=Redo / note_history redo can=true len=3        ← Q15：重做还原
```

`len=` 是刻意加的：没有它，"重做了一次但什么都没发生" 和 "重做成功" 在日志里长得一模一样。

### 本批不做

| 项 | 为什么 |
|---|---|
| `Ctrl+K` 切换器（Q25–Q27） | 需要 `notes.db` 的最近列表，属 Phase 2 前置 |
| `Ctrl+Shift+H` 幽灵模式（Q29） | 需要整窗合成，属 1c |
| 置顶键位读配置 | 现在没有键位配置层；固定用旧版默认值 `Ctrl+T`，等配置层一起做 |
| `Ctrl+Shift+Z` 之外的编辑器键位（复制行、标题级别等） | 旧版有 12 条，属编辑器内部功能，不是窗口键 |

## 8.5 多窗口与销毁（1b.2）

1b 的验证前提和前两批不同：要数**进程里的窗口**，不是"看窗口"。所以这一批第一个决定不是"怎么建第二个窗口"，而是"谁持有事件循环"。

### 根视口不是笔记

eframe 把事件循环绑在根视口上：根窗口关闭 → `run_native` 返回 → 进程结束。若笔记 #0 就是根视口，关掉它会把其余笔记一起带走，正好是 Q5 的反面。于是：

- 根视口是 1×1 的控制器窗口（标题 `easyflowhub-hub`），首帧就被 `hub_win::window::park` 停到屏幕外。实测落点 `-24000,-24000`，与 `window.rs` 里记的系统夹取一致（要 `-30000` 只给到 `-24000`）。
- 每个笔记是一个 deferred viewport，标题 `easyflowhub-note#<id>`。

停到屏外而不是 `SW_HIDE`，是 S1 就付过学费的那条：隐藏会停掉 `WM_PAINT` 投递，而根视口一旦不能绘制就不能声明 viewport，笔记会**冻住**而不是关闭。

### 四个不显然的细节

| 细节 | 为什么 |
|---|---|
| 用标题把 viewport 和 `HWND` 对上 | deferred viewport 的回调只拿到 `Context`，eframe 0.31 不在回调里暴露 `HWND`，所以 `find_by_title` 从"找那个笔记窗口"变成精确查找。标题不可见（无边框、无任务栏），成本为零 |
| `Shared` 用 `unsafe impl Send/Sync` 包一层 | `show_viewport_deferred` 要求回调 `Send + Sync`，而 `NoteShell` 持有 `HWND` 和 RichEdit 子类指针。**没有任何东西真的跨线程**：eframe 在事件循环线程调用回调，win32 句柄也只在那条线程有效。`lock()` 里断言线程 id，把这条注释变成检查而不是承诺 |
| 去掉 1a 的 `set_zoom_factor` | 它是 `Context` 全局的：一个笔记会改所有笔记的缩放，多显示器下不是取整误差而是整体错标。也不需要——实测 `note_ppp id=0 ppp=1.25 (was 0.00)`，窗口放到位之后 winit 第一帧就报出真实缩放，岛每帧按 `ppp` 重算矩形 |
| 按键与 Esc 的回调必须指明 viewport | `ctx.request_repaint()` 的语义是"最后跑过一遍的那个视口"。单窗口时它恒等于根视口所以看不出来，两个笔记时它会指向**另一个窗口**，动作只能等该笔记自己的 500 ms 心跳——就是 1a 的"Esc 要按三次"换了件马甲。`keys::Bus` 和 `esc::handler` 因此都带 `ViewportId`，用 `request_repaint_of` |

### 证据（125%，日志）

```
hub_window parked=true focus_released=0
hotkey combo=Ctrl+Alt+N registered=true thread=40280
note_spawn id=0 live=1 cursor=1017,851 monitor_scale=1.25
note_window id=0 500x375+1025+468 dwm=["dark","round","mica"] foreground=true

note_key id=1 action=Close
note_close id=1 reason=shortcut open_notes=2
note_destroyed id=1 open_notes=1 process_windows=5 notes=1   ← 那一个窗口没了，#0 还在
note_close id=2 reason=os open_notes=3                       ← Alt+F4 走同一条销毁路径
note_destroyed id=2 open_notes=2 process_windows=6 notes=2
note_close id=0 reason=shortcut open_notes=1
note_destroyed id=0 open_notes=0 process_windows=4 notes=0
hub_exit reason=no_notes tray=pending hotkey=stops
```

两条实测结论：

- **销毁是真的**：关闭后从外部 `EnumWindows` 只剩 `note#0` + 控制器；不是 park 到屏外，也不是隐藏。
- **新建是全新窗口**：`note_begin id=1` → `island_created id=1 ... caret=(0, 0)`，新 RichEdit、空文档，旧笔记的文本与光标不受影响。
- **热键是全局的**：Q18 的判定条件是"焦点在别的程序上"，所以单独测了一次——把前台切到另一个窗口后再按 `Ctrl+Alt+N`，日志 `note_spawn id=1 live=2`，接着 `note_window ... foreground=true`，新笔记拿到前台并把光标放进正文。

`open_notes` 只数带笔记前缀的标题。`top_level_windows()` 还会返回 winit 的 14×14 事件窗口和两个输入法宿主（实测 `Winit Thread Event Target` / `MSCTFIME UI` / `Default IME`），直接数总数会因为与笔记无关的原因跳动，那样的数字不能当判据。

`note_close` 和 `note_destroyed` 必须成对：前者在窗口还在时记录（`open_notes` 仍含自己），后者在下一个 pass 记录。只有一行无法区分"请求关闭"和"已经关闭"——这正是 D7 定 `windows_before/after` 时想要的形状，只是字段名换成了更不会误导的 `open_notes`。

### 本批不做，以及为什么关完就退出

| 项 | 为什么 |
|---|---|
| 托盘常驻 | 没有托盘图标，就没有"进程还在"的可操作入口 |
| 关完最后一个笔记不退出 | 和上一条绑在一起：一个只能靠任务管理器结束的隐形进程比"退出"更糟。所以现在关完就 `hub_exit`，日志写明 `tray=pending`；1b.3 只改这一行的判断，不改字段 |
| `Ctrl+Alt+D/H/M`（Q18 其余三条） | 关闭全部 / 最小化全部需要托盘语义，打开工作台需要 launcher 面板 |
| 新建窗口的级联偏移 | 旧版也没有：`cursor_anchored_position` 把每个笔记都锚在光标上，多个笔记完全重合是复刻行为，不是缺陷 |

## 8.6 托盘与其余全局键（1b.3）

一批里的三件事其实是同一件：**让"进程还在"成为一个用户能操作的状态**。托盘是入口，三个全局键是入口的快捷键形态，"关完不退"是前两者成立之后才能改的判断。

### 托盘自己有一个消息窗口

托盘可以让控制器窗口代收（一次 `SetWindowSubclass`），但那个窗口的生命周期归 winit：菜单一旦卡住不消失，排查要跨三个 crate。代价只是一个 `RegisterClassW` + 一个隐藏窗口，换来的是这个模块里所有句柄都归自己管。

菜单 id 只有一张表（`command_for`），`TrackPopupMenu` 和外部直接投递的 `WM_COMMAND` 都走它——这既是 Win32 的正常写法，也让托盘在验证时可以被脚本驱动（下面"证据"里的三个菜单项就是这样点的）。

### 三个细节看起来可选，其实不是

| 细节 | 症状 |
|---|---|
| `SetForegroundWindow` 在前、`PostMessage(WM_NULL)` 在后 | 不做的话菜单在点别处后**留在屏幕上**：弹出菜单是独立窗口，只有 owner 是前台时才会被关掉（Win95 起的老问题） |
| 退出前 `NIM_DELETE` | `Drop` 只能覆盖普通路径，而控制器是走 `std::process::exit` 离开的（不跑析构）。漏掉的图标会一直留在通知区，直到用户把鼠标移上去——看起来就像进程崩了 |
| 延迟 250 ms 的防抖要**按槽位**计 | QQ/微信对同样的组合装了低级键盘钩子，一次物理按键会来两三条 `WM_HOTKEY`。共用一个时钟的话，先按 `Ctrl+Alt+N` 再按 `Ctrl+Alt+D` 的第二下会被当成回显丢掉 |

### 这一批真正的 bug：关闭打到了根视口

`NoteShell::close` 原来用 `ctx.send_viewport_cmd(Close)`，它的语义是"**这一趟 pass 的视口**"。从笔记自己的帧里调用时那正好是它自己，所以 Esc 和 `Ctrl+D` 一直是对的——对得纯属巧合。从托盘或 `Ctrl+Alt+D` 调用时，发起方是控制器，pass 跑在 ROOT 上，于是"关闭全部笔记"变成了"关闭 hub 窗口"，而 eframe 在根窗口关闭时结束进程。

改前实测（日志到此为止，没有 `hub_exit`）：

```
note_destroyed id=1 open_notes=0 process_windows=5 notes=0
hub_idle notes=0 tray=0x2201E0        ← 托盘还在，下一秒进程就退了
```

`close` 与 `toggle_pin` 都改成 `send_viewport_cmd_to(self.viewport, ...)`。这类 bug 不会在单窗口下出现，也不会在没有托盘时出现——1b.2 的"关完就退出"把它盖住了，是这一批把它挖出来的。

### 最小化的笔记不再布岛

最小化之后窗口的客户区是退化的（实测整窗 `190x31`，换算出来 `island_rect 237x1`）。把 RichEdit 按 1 px 高重排一次，既没人看得见，也可能让它丢掉滚动位置；恢复时又得再排一次。现在 `frame()` 在最小化时直接返回，岛保持原矩形，恢复后不需要重建——恢复路径显式请求一次重绘，不依赖 `SW_RESTORE` 是否产生 `WM_SIZE`。

### 证据（125%，一次运行）

```
hotkey combo=Ctrl+Alt+N/D/H/M registered=true ×4    hotkeys registered=4 failed=0 thread=37104
tray installed=true hwnd=0xAE01D8 icon=ico tip=EasyFlowHub · 快速笔记（Ctrl+Alt+N）
hub_start notes=1 ... tray=on

Ctrl+Alt+H → notes_visible action=minimize notes=2 → 外部 IsIconic=true ×2（整窗 190x31）
Ctrl+Alt+H → notes_visible action=restore  notes=2 → IsIconic=false，回到 400x300
Ctrl+Alt+D → note_close reason=all ×2 → note_destroyed open_notes=0 → hub_idle，进程存活
Ctrl+Alt+N → note_spawn id=2 live=1   ← 空闲状态下仍可新建
托盘右键   → #32768 菜单出现；WM_CANCELMODE 后消失
托盘三项   → WM_COMMAND 1 新建、2 关闭全部、3 → hub_exit reason=tray notes=0，退出码 0
Ctrl+Alt+M → note_manager reason=hotkey target=none phase=2
```

`icon=ico` 说明内嵌 `.ico` 的 32×32 帧被 `CreateIconFromResourceEx` 接受了（那些帧是 PNG 压缩的，Vista+ 可用但文档没对每种尺寸承诺），`IDI_APPLICATION` 的兜底没有触发——兜底仍然是真代码，不是 `unwrap`。

### 与旧版的差异（有意）

| 项 | 旧版 | 本批 | 为什么 |
|---|---|---|---|
| 托盘菜单首项 | 打开管理中心 | 关闭全部笔记 | 管理中心窗口属 Phase 2；托盘接管进程后，"关掉所有窗口"是唯一没有鼠标路径的命令（Esc 和 `Ctrl+D` 一次只关一个） |
| 左键点图标 | 显示/隐藏管理中心 | 打开同一个菜单 | 没有管理中心可切，弹菜单比猜用户想新建一个更不意外。Phase 2 接上管理中心后这里改回"切换" |
| `Ctrl+Alt+M` | 打开管理中心 | 注册 + 记日志 | 组合键必须由当前这份程序owns，否则旧版会接走它；目标窗口要等 Phase 2 |

### 本批不做

| 项 | 为什么 |
|---|---|
| 托盘气泡/通知 | 没有要通知的事件；旧版也没有 |
| 开机自启、托盘双击、"关闭窗口时缩到托盘" | 旧版没有前三项，属 Phase 2 的设置层 |
| 管理中心窗口（`Ctrl+Alt+M` 的目标） | 需要 launcher 面板，属 Phase 2 |
| 多显示器下的托盘图标（`NIM_SETVERSION` / 高 DPI 变体） | `CreateIconFromResourceEx` 已按 32×32 建图标，Windows 自行缩放；没有第二个显示器上的实测，不写进结论 |

## 8.7 整窗透明与幽灵模式（1c）

这一批把标题条左槽那个数字变成真的。1a–1b 期间它恒为 `100%`，因为那时还没有任何东西能把窗口调淡——一个显示 `GHOST` 却完全不透明的窗口，比一个老实的 `100%` 更糟。

### 路径：`LWA_ALPHA`，不是自绘遮罩

旧版是整窗 CSS `opacity`：标题条、正文、光标一起淡，桌面透上来。三个候选里只有一个能在这里成立：

| 候选 | 结果 |
|---|---|
| 岛（RichEdit）单独调 alpha | 岛只覆盖正文，标题条不淡，接缝一眼就能看到 |
| egui 画一层半透明遮罩 | 子窗口永远画在父窗口之上，遮罩会盖在正文**下面** |
| `SetLayeredWindowAttributes(LWA_ALPHA)` | DWM 把窗口表面当一张位图整体混合，RichEdit 跟着一起淡——**采用** |

顺带确认了一件事：`WS_EX_LAYERED` 不影响 GL 绘制。60% 时 `PrintWindow` 抓到的窗口表面与 100% 逐像素相同（`mean=(34.3,22.2,14.8)`，主色即面板 `#0D1522`），说明 `glow` 的 swap chain 依然落在重定向表面上。

### 两条滚轮通道，一张表

`Ctrl+滚轮` 在哪一侧被看到，取决于指针在哪个窗口上，而两个窗口的默认行为都不能要：

| 指针在哪 | 消息 | 不接会怎样 |
|---|---|---|
| 正文（岛） | 子类过程里的 `WM_MOUSEWHEEL`，`LOWORD(wParam)` 带 `MK_CONTROL` | RichEdit 把 `Ctrl+滚轮` 当**文本缩放**，每格涨一点字号 |
| 标题条（egui） | winit → `egui::Event::MouseWheel` | egui 把它当**全局 UI 缩放**；缩放因子属于 `Context`，一动所有笔记一起变大 |

两边都关掉：岛在 `hub_editor::dispatch::claim` 里吞掉这个组合（返回 0，消息不再往下传），egui 在 `Hub::new` 里写 `options.zoom_with_keyboard = false`（日志 `egui_zoom ctrl_wheel=false`）。日志多一个 `path=Island|Egui` 字段——否则"滚轮没反应"和"滚到另一个窗口去了"在日志里长得一模一样。

节流沿用旧版常量 50 ms（`app::wheel::Gate`，时钟注入所以能单测）。连发 5 条 `WM_MOUSEWHEEL` 实测只走一格：`note_wheel` 只有一行 `opacity=0.80 (was 1.00)`。槽位本身也只留最后一格（和 `keys::Bus` 同理），所以一帧最多一步。

### GHOST 只能是 alpha 1，不能是 0

这是本批唯一一个"照抄旧版数值就会出事"的地方。`LWA_ALPHA = 0` 的窗口**不可命中**：同一坐标 `WindowFromPoint` 在笔记上回答的是背后的窗口，改成 1 就回答 `RICHEDIT50W`。也就是说 alpha 0 的幽灵笔记点不到、拿不到焦点，而 `Ctrl+Shift+H` 是岛内快捷键——焦点只能靠点击拿到。本阶段正文还在内存里，一次"关掉全部"就等于丢字。

所以映射表底部有地板 `GHOST_ALPHA = 1`（`hub_win::alpha`）：视觉上等于消失（实测屏幕 `(251,253,251)`，邻居 `(252,254,252)`，差一个 1/255），但窗口仍可点、仍能滚回来。旧版靠 CSS `opacity: 0` 天然保留指针事件，这里用一个字节买回同一条路径。

### 已试验并搁置：背景色键 + 内容 20%

"背景彻底消除、文字保留 20%"试过一版，**结论是搁置**，记在这里以免重复投入。

做法是 `LWA_COLORKEY | LWA_ALPHA`，面板色作键（窗口两侧本来就是它：egui 画面板、`EM_SETBKGNDCOLOR` 填 RichEdit），键色像素 alpha 0，其余 51 = 20%。

三个实测结论对未来有用：

| 结论 | 证据 |
|---|---|
| **Mica 背板会挡住色键** | `DWMSBT_MAINWINDOW` 时正文 `(207,210,209)`（20% 平铺）；`DWMSBT_NONE` 时 `(252,254,252)`，与桌面逐值相等。要做色键必须先关 Mica |
| 色键对 RichEdit 子窗口**有效** | 子窗口背景像素与桌面**逐值相等**，不只是一层淡色 |
| 子窗口的命中**不跟随**父窗口色键 | 透明正文仍回答 `RICHEDIT50W`（标题条答笔记窗口）。想真正让路要给 RichEdit 加 `WS_EX_TRANSPARENT` |

搁置的原因（用户实测观感，2026-11-25）：

| 现象 | 说明 |
|---|---|
| 文字看起来像被描了边 | 没有加描边；是背景被剔除后，字体抗锯齿的边缘直接压在壁纸上，20% 的浅色字在浅色壁纸上只剩一圈硬边 |
| 背景无法"彻底"透明 | 键色只认精确匹配，egui 的圆角/抗锯齿边缘、以及 RichEdit 之外的过渡像素都留着 20%，视觉上仍是一层薄片 |
| 浅壁纸下读不到字 | 20% 是物理上限：浅色字 × 20% 叠在浅色壁纸上，对比度不足 |

因此幽灵态维持 alpha 1（完全不可见、仍可点击），"失焦后找不到位置"这个问题留到体验优化阶段用别的办法解决（候选：进入幽灵前短暂闪一下、或在托盘/任务栏留标记）。

### 证据（150%，一次运行）

```
note_begin id=0 logical=400x300 ... opacity=1.00
note_alpha id=0 phase=attach opacity=1.00 alpha=255 applied=true readback=Some(255, None)
```

`phase=attach` 说明 alpha 是在 `show_without_activation` **之前**落的：builder 里 `with_visible(false)`，窗口此刻还不可见，所以按设置值 40% 打开的笔记不会先以 100% 闪一帧再淡（B3 不允许透明度跳变）。

```
岛通道   note_wheel notch=Down path=Island opacity=0.80 (was 1.00) label=80%  → alpha=204
标题条   note_wheel notch=Down path=Egui   opacity=0.60 (was 0.80) label=60%  → alpha=153
五连滚   --down 5 --gap 0 → 只有一行 note_wheel（不跳档也不连跳）
普通滚轮 --raw -120 → note_wheel 计数不变（交给 RichEdit 滚动）
20%      note_alpha opacity=0.20 alpha=51 readback=Some(51)
GHOST    note_alpha opacity=0.00 alpha=1  readback=Some(1)   ← 地板，不是 0
```

像素侧（屏幕 DC 截图，背后是一片白 `(252,254,252)`）：

| 不透明度 | 正文实测 | 期望 |
|---|---|---|
| 100% | `(13,21,34)` | `(13,21,34)` |
| 60% | `(109,114,121)` | `a×面板+(1−a)×白 = (108.6,114.2,121.2)` |
| 40% | `(156,161,165)` | `(156.4,160.8,164.8)` |
| 40% 标题条 | `(164,171,178)` | `(163.6,169.6,178.4)` |
| GHOST | `(251,253,251)` | `(252,254,252)` + 1/255 地板 |

GHOST 那一行不能用肉眼下结论：它只差一个字节，肉眼看起来就是"没了"。数字才是证据。

Q29 的一次完整往返：

```
note_ghost id=0 ghost=true  opacity=0.00 (was 0.40) restore=0.40
note_ghost id=0 ghost=false opacity=0.40 (was 0.00)   ← 回到 40%，不是 20%
```

`Ctrl+Shift+H` 用 `SendInput` 真按（不是 `PostMessage`）：岛的修饰键读自 `GetKeyState`，投递的消息不会带修饰键，投出来的 `Ctrl+Shift+H` 会变成裸 `H`。`Ctrl+滚轮` 的标题条通道同理，脚本里必须先 `SendInput` 按住 Ctrl 再投 `WM_MOUSEWHEEL`。

左槽标签目视确认：20% 时 `PrintWindow` 抓到的窗口表面左槽显示 `20%`（`alpha=51`），不是 `100%`。

### 与旧版的差异（有意）

| 项 | 旧版 | 本批 | 为什么 |
|---|---|---|---|
| GHOST 的实际 alpha | 0 | 1/255 | 见上：0 会穿透，幽灵就捡不回来 |
| 滚轮监听范围 | `window` 上一个 `wheel` | 岛 + 标题条两条通道 | 子窗口不把滚轮转交给父窗口的 egui，只能各接一次 |
| 透明度初值 | 组件 state，恒 100% | `quick_note.appearance.window_alpha`（默认 1.0） | 与其它外观项同文件；它是**用户态初值**，不是主题装饰（C2） |

### 本批不做

| 项 | 为什么 |
|---|---|
| 幽灵态"背景完全消除 + 文字 20%" | 试过色键方案，观感不对（文字像被描边、背景仍有薄片残留），已回退；结论与 Mica 实测留在上一节 |
| 每窗口记住透明度 | 旧版也不记（组件 state，关掉即回 100%）；持久化属 Phase 2 的 `notes.db` |
| 淡入淡出过渡 | B3 允许没有；每次改 alpha 都会让 DWM 重组整个窗口，动画等于每帧一次重组 |
| 透明时单独淡化光标/文字 | 整窗混合已经覆盖；再叠一层就与旧版的 CSS 语义分叉 |
| 托盘里的"幽灵"菜单项 | 幽灵是窗内手势（旧版同样只在窗内）；托盘四项已经够用 |

## 8.8 数据层：正文接 `notes.db`（1d，2026-11-26）

1c 结束时，笔记正文还只是"内存文档"：关掉窗口就没了。这一批把它接到库里，覆盖 parity Q20–Q23，外加"按 id 重开一条已有笔记"——没有这一条，前面四条只能靠读库自证，连"重开还在不在"都验不了。

### 落点

| 层 | 文件 | 职责 |
|---|---|---|
| `hub-core` | `clock.rs` | 与旧版逐字一致的 RFC 3339（9 位小数、`+00:00`），不为此引入 `chrono` |
| `hub-core` | `note_text.rs` | 标题 / 正文 / 标签的拆分，逐条对齐 `easyflowhub-app/src/lib/noteParser.ts` |
| `hub-core` | `store/notes.rs` | `save_note_body`：只写 `title/content/tags/updated_at` 的 upsert |
| `app` | `save.rs` | **何时写**：900 ms 空闲防抖 + 2 s 扫描兜底 + 关闭 flush + 失败退避 |
| `app` | `note/island.rs` | `impl NoteShell` 里"岛"的那一半（从 `note.rs` 拆出，见下"文件上限"） |
| `app` | `hub.rs` / `args.rs` | 打开 store、`--note-id` 按 id 重开、`note_doc`/`note_load` 日志 |

### 三个"看起来省事、其实会出事"的地方

1. **不能整行 upsert。** `is_pinned` 是**列表置顶**（旧版列表 `ORDER BY is_pinned DESC, updated_at DESC`），与快速笔记的**窗口 z-order 书签**（Q9）是两件事。整行写会把"最后碰过的笔记"搬到列表最前；`deleted_at` 更糟——一个还开着的窗口能把回收站里的笔记复活。所以 `NoteWrite` 只带六个字段，`ON CONFLICT` 只更新四个。
2. **控件文本才是基线，不是"我推给它的字符串"。** RichEdit 会补段落标记、用 CRLF；拿推入值当基线，每一条重开的笔记都会在一秒后重写自己的行。落盘后调 `rebase(&control.text())`，第一次比较就已经是控制自己的答案。
3. **保存失败不能记成保存。** 失败时 `due_at` 不清空、`retry_at` 退避 1 s，下次比较仍然知道有没写下去的字。"标记为已保存"是丢字最安静的形态。

### Q23 是两件事，用一个标志分开

- **新笔记 + 空正文 → 不写行**（新建后不输入直接 Esc，列表里不该多一条空壳）；
- **已存在的笔记 + 清空 → 必须写**（否则库里的旧正文会在下次打开时"自己长回来"）。

一个布尔 `persisted` 同时表达这两件事：它只由 `created` 那次写置真，之后空正文就是合法状态。

### 编辑信号只是延迟优化，扫描才是保证

`hub-editor` 的 subclass proc 只对"可能改文本"的消息**通知**（`WM_CHAR`/`WM_IME_CHAR`/`WM_PASTE`/`WM_CUT`/`WM_CLEAR`/`WM_UNDO`，加 `VK_BACK`/`VK_DELETE`），一律 `return None` 交回 `DefSubclassProc`——它从不吞消息。即便如此，正确性不依赖它：每 2 s 把控件文本与上次写入的文本比一次。门禁就是这么验的——只发 `WM_KEYDOWN VK_RETURN`（RichEdit 自己插段落，不发 `WM_CHAR`，因此没有任何编辑信号），3.8 s 内行照样落库（`note_save created=false`）。

### 文件上限

`note.rs` 拆分前 833 行，超了架构守卫的 800 行硬上限。拆的是模块本来就有的一条缝：`theme_for` + `sync_editor`（建岛、填字、上主题、记 `note_load`）挪到 `note/island.rs`，`colorref` 一起走；`note.rs` 剩 700 行。守卫随即通过（5 crates）。

### 证据（`scripts/check-native-notes.py`，24/24）

| 检查 | 实测 |
|---|---|
| Q20 打字后落库 | 打"买菜清单"等 1.7 s，`note_save reason=autosave created=true title="买菜清单"` |
| 扫描兜底 | 只发 `WM_KEYDOWN VK_RETURN`，3.8 s 后正文变 `买菜清单\n`，`created=false` |
| Q22 标题 | `title='买菜清单'` |
| 标签剥离 | 末行 `# 工作 重要` → `tags='工作 重要'`，正文 `买菜清单\n鸡蛋`，无 `\r` |
| 不碰置顶 | `is_pinned=0` |
| Q21 关闭 flush | 打"字"后 **43–73 ms** 按 Esc，行已写入（`reason=close created=true`），远早于 900 ms |
| Q23 空笔记 | 行数 40 → 40，`note_save reason=close skipped=empty` |
| 按 id 重开 | `note_load requested=<id> found=true`、`note_load chars=15`；光标在末尾（再打"尾"落进 tags 行）；不新增行 |
| G-1 / G-2 | 四个进程都开的是被复制的库；schema 指纹（含 `PRAGMA table_info`）不变 |

门禁自身踩到的两个坑写在脚本头部：跨进程读不到 RichEdit 文本（`GetWindowTextW` 空串，`SetWindowTextW` 返回成功却不动一个字），以及 `WM_CHAR 0x0D` 不换行（要 keydown + char + keyup 三连）。所以"笔记里是什么"一律用数据库往返回答。

### 与旧版的差异（有意）

| 项 | 旧版 | 本批 | 为什么 |
|---|---|---|---|
| 重开一条笔记 | 前端持 `noteId` 走 IPC | `--note-id <uuid>` 命令行 | 没有工作台，命令行是唯一的"选一条笔记"入口；找到没有都记 `note_load requested= found=` |
| 正文写库时机 | `useQuickNoteSession` 的 900 ms 防抖 | 同一 900 ms，外加 2 s 扫描与关闭 flush | 防抖管手感，扫描管"信号漏了也不丢字"，两者不重复 |
| 空正文判定 | `buildPersistParams(..., { allowEmpty: notePersistedRef.current })` | `persisted` 标志 | 同一语义：已存在的笔记允许清空保存，新笔记不留空壳 |

### 本批不做

| 项 | 为什么 |
|---|---|
| 工作台（列表 / 搜索 / 收件箱 / 待办 / 脚本 / 设置 / 回收站） | Phase 2 其余批次；`Ctrl+Alt+M` 仍只记 `note_manager target=none phase=2` |
| `Ctrl+K` 切换器（Q25–Q27）与数据恢复（Q24） | 依赖列表与查询，属工作台批次 |
| 每窗口记住透明度 | 要 `settings` 表的一行一窗口键。裁定不做——调透明度是临时动作，收益低；尺寸记忆（C5）也只记全局一组宽高，不为窗口各造键（§8.15） |
| `store` 的 schema 版本号 | 当时与 Tauri 同文、`user_version = 0`，打了会让两套迁移分叉 —— **已由 §8.17 收掉**（版本是量出来的，不是盖上去的） |
| 正文长度上限 / 图片与 Markdown | 属 P3，与 1c 的结论无关 |

## 8.9 交互层：快捷键配置、切换器、工作台（2a–2d，2026-11-26）

1d 结束时笔记存得住但换不了：要打开另一条笔记得先从库里取 id、再走命令行。这一批补上三件事——一张**可以改的**键位表（并把"每个带键位的按钮都悬浮显示功能名 + 快捷键"变成纪律）、`Ctrl+K` 切换器（Q25–Q27）、`Ctrl+Alt+M` 指向的工作台（Q16 与 Q18 的另一半）。

### 落点

| 层 | 文件 | 职责 |
|---|---|---|
| `hub-core` | `shortcuts.rs` | `Command` / `Chord` / `Scope` / `Table`：11 条命令、两个作用域、冲突仲裁、键位与字符串双向映射 |
| `hub-core` | `settings.rs` | `shortcuts` 稀疏覆盖；`save()` 写 `.json.tmp` 再 `rename` |
| `hub-core` | `store/notes.rs` | `set_note_pin` / `restore_note` / `purge_note`，三条都只动自己那列 |
| `hub-ui` | `hint.rs` | 悬浮提示：功能名 + 键位，文案取自同一张表 |
| `hub-ui` | `keys.rs` | egui 按键 → `Chord` 的唯一映射（改键捕获与窗内解析共用，避免两条路径对 `1` 给出不同码） |
| `hub-ui` | `switcher.rs` | 面板状态机与绘制（查询、选中、Enter） |
| `hub-ui` | `manager.rs` | 工作台四个分区 + 设置页 |
| `app` | `keys.rs` | 两条输入路径汇到一张表；`Bus` 用一个 `AtomicU8` 存待处理命令 |
| `app` | `note/panel.rs` | 切换器窗口侧：隐藏岛、就地切换、新建 |
| `app` | `manager.rs` | 工作台窗口与它的副作用 |
| `app` | `hub/commands.rs` | `open_manager` / `open_note` / `rebind`（都碰控制器私有的状态） |

### 键位为什么必须是一张表

`Ctrl+K` 只是默认值，不是事实。三处必须读同一份数据，否则总有一处会撒谎：

1. **按键解析**——窗口里两条路径（岛的子类 proc、egui 的 winit 事件）都要认同一个组合；
2. **悬浮提示**——提示里的键位若不是从表里取的，改完键它就开始骗人；
3. **改键界面**——设置页显示"当前键"、捕获新键、写回文件，全是这张表。

表放在 `hub-core` 而不是窗口里：它是纯逻辑（`Chord::parse` 与 `Chord::display` 同源，15 条单测把每个默认键跑一遍往返），窗口与工作台只消费。表本身用 `Arc<RwLock<Table>>` 共享——**改键要触达已经打开的窗口**，spawn 时拷一份的写法只会得到"我改了，但那个窗口还是老键"。

### 冲突、固定键、以及为什么不拒绝启动

- `Command::ALL` 的顺序**就是**优先级：先者胜，败者由 `Table::shadowed()` 返回并跳过注册，日志写 `shortcut_rejected`。拒绝启动会把一个可以容忍的配置变成打不开的程序。
- `Esc` 不进表。它是"关闭"的语义本身（Q12 关窗口、Q13 关面板），不是偏好；窗口作用域下任意修饰键都触发。
- 组合键必须带 `Ctrl`/`Alt`/`Shift`，例外只有 `Esc` 与 `F1`–`F24`——它们不是字符键，占不到用户的打字。

### 三个坑

1. **改全局键要先让旧的松手。** `RegisterHotKey` 是先来先得：关掉旧 `HotkeySet` 之后立刻注册新组合会拿到 1409（旧线程还没跑到 `UnregisterHotKey`）。所以 `shutdown()` → `sleep 60 ms` → 重新注册，日志里 `hotkeys registered=… failed=…` 就是这一步的证据。
2. **子窗口永远画在父窗口之上。** 切换器要盖住正文，但正文是 RichEdit 子 HWND，egui 画不过它。开面板时 `Editor::hide()`（`SW_HIDE`）、关面板时 `Editor::show()`（`SW_SHOWNA`，恢复可见但不抢焦点），焦点在开面板时交回 frame（否则查询框一个字符也收不到）。
3. **种下的查询词不会自己落库。** 索引 0 按 Enter 会把查询词写进控件当正文，但 `Pump::tick` 的判断是"控件文本 == 基线就丢掉这次写"，而 `start_new` 刚把基线设成那段文本——于是"新建的笔记"在列表里查不到自己。改成 seed **立刻** `flush`（`reason=seed`），再把基线设回控件自报的文本（RichEdit 会补一个段落标记，不设回去就会在一秒后重写同一行）。这是本批唯一一处由门禁实测暴露出来的缺陷。

### 工作台为什么是 immediate viewport

它没有文本岛、没有需要跨帧保存的窗口状态，deferred 只会给它自己的点击多加一帧延迟。反过来，**所有副作用都不在帧里做**：开一条笔记要创建 viewport，改一个全局键要重注册热键，两者都在 `Hub` 手里，所以工作台把它们塞进 `Request` 队列，由控制器在下一轮处理。工作台只重画列表而不每帧查库：列表只在打开与写之后变。

### 证据（三个门禁，都要真按键）

| 门禁 | 判什么 | 结果 |
|---|---|---|
| `scripts/check-native-switcher.py` | Q25 同键开关、Q13 Esc 只关面板、Q26 正文/标签命中、Q27 选中与索引 0 新建、面板打开时岛被隐藏 | 22/22 |
| `scripts/check-native-manager.py` | Q16 用**改过的**键打开工作台且计数与库一致、Q18 Esc 只关工作台、K1 文件里的键生效、K2 被换掉的键失效 | 19/19 |
| `scripts/check-native-notes.py` | 1d 的 Q20–Q23 + 按 id 重开（本批回归） | 24/24 |

为什么这三个门禁要真按键：`hub-editor` 的修饰键读的是 `GetKeyState`，投一条消息只会得到"一个没修饰的字母"；egui 的键与文本来自 winit，投消息到不了它那儿。共用注入层是 `scripts/lib/wininput.py`，它的头部记了三条实测：`INPUT` 在 x64 上必须 40 字节（写错 `SendInput` 返回 0 + `ERROR_INVALID_PARAMETER`，且静默不投递）、中文要靠 `KEYEVENTF_UNICODE`、`SetForegroundWindow` 对非前台进程会被拒（`AttachThreadInput` 借一下就行）。

`cargo test --release --workspace` 190 项、`node scripts/check-native-architecture.mjs`（5 crates）与 `node scripts/check-architecture.mjs` 同时通过。`cargo fmt` 在本机没装（`rustfmt` 组件缺失），这一批的格式是按既有风格手写的。

### 与旧版的差异（有意）

| 项 | 旧版 | 本批 | 为什么 |
|---|---|---|---|
| 键位存哪 | 前端 `shortcutConfig`（设置页写 `notes.db`） | 设置文件 `shortcuts`（稀疏，只存改过的） | 桌面壳没有 IPC 通道去读前端配置；文件的键名用 `Command::id()`，与前端字段一一对应，将来搬进 `notes.db` 时映射是直的 |
| 悬浮提示 | 无 | 每个带键位的按钮都显示"功能名 + 快捷键" | 用户约束：改键之后"这个按钮是什么键"必须能就地查到 |
| 切换器 | React 浮层，`selectedIndex` 由键盘表驱动 | egui 浮层，隐藏 RichEdit 子窗口让位 | 同样的语义；差别只在 z-order 上——原生子窗口不会尊重父窗口的绘制顺序 |
| 工作台 | 独立页面路由 | 独立顶层窗口（viewport） | 工作台不是"一个更大的笔记"：没有岛、不置顶、可在任务栏出现 |
| 改键时机 | 设置页失焦即写 | 捕获到新组合立刻写文件 + 重注册 | 少一个"还没保存"的状态，也就不存在"改了没生效" |

### 本批不做

| 项 | 为什么 |
|---|---|
| 每窗口记住透明度 | `QuickNotePage` 的 `opacity` 是 `useState`，旧版同样不持久化；原生侧只保留"新笔记初值"（`appearance.window_alpha`）。**裁定不做**：调透明度是临时动作，收益低；尺寸记忆（C5）也只记全局一组宽高（§8.15） |
| `store` 的 schema 版本号 | **已由 §8.17 收掉**：不打 `user_version`，改成六级梯子 + 量出来的版本号 |
| 工作台按钮的端到端点按 / 改键的写盘方向 | 点一下要在 egui 布局里猜坐标，布局一改门禁就红、且红的原因与行为无关。写路径由 store 与 `manager` 的 `Action` 单测覆盖，窗口行为由 `check-native-manager.py` 覆盖 |
| 收件箱与脚本分区 | 依赖 capture 与执行链路，属 P3；待办分区只读正文里的 `- [ ]` 行，不写回 |

## 8.10 工具面板：点得到的按钮，点得掉的浮层（2e，2026-11-26）

Q7 在 parity 里挂了很久，挂的理由是"点面板外部也关"没做。这一批去补它，结果门禁先撞出一个更大的问题：**标题条上的两个按钮根本点不动**。

### 真 bug：拖拽区盖住了按钮

`quicknote::draw` 先注册两个按钮（`Sense::click()`），再注册整条 strip 的拖拽区（`Sense::drag()`）。egui 的规则是：指针下的多个控件里，**后注册的在上**。于是拖拽区盖住了两个按钮，鼠标按下去命中的是拖拽面：`drag_started()` 为真（被 `!on_button` 挡掉，所以也不拖），而按钮的 `clicked()` 永远不来。

键盘因此成了唯一入口——`Ctrl+T` 能置顶、`Ctrl+.` 能开面板，鼠标点同一个位置什么都不发生。这类缺陷单元测试一般不覆盖，人眼也容易以为"点了没反应是没做"，因为代码里明明写着 `if tools.clicked()`。

修法是两条一起：

1. **拖拽区先注册**，按钮后注册，点击才落在按钮上；
2. **`on_button` 守卫保留**——排序解决点击归属，但不解决 `drag_started`：egui 对同一次按下仍会给拖拽控件一个 `drag_started`，于是点按钮会顺手把窗口交给 `StartDrag`。实测后果是窗口被挪几像素，**下一次点击**就落到空处（日志里第二次 `note_tools` 永远不出现）。

新单测 `a_click_on_a_button_is_not_swallowed_by_the_drag_surface` 用 `Context::run` + `RawInput` 造一次真实的三帧点击（move / press / release），断言 `toggle_pin` 与 `toggle_tools` 都为真、`drag` 为假。把顺序改回去，这条测试立刻红——本批验证过。

### 点正文关闭：消息到不了 egui

"外部"基本就是正文，而正文是 RichEdit **子窗口**：按下发生在子窗口，父窗口的 winit 什么也收不到。所以关闭信号只能从岛的 subclass proc 发出来，和 `Ctrl+滚轮` 同一条路：

- `hub-editor` 新增 `on_click` 回调，接 `WM_LBUTTONDOWN`，**只上报不吞**（否则光标定位、双击选词、拖选全没了）；
- `app::click::Clicks` 把点击记成计数并 `request_repaint_of(viewport)`——和 `esc`/`wheel`/`keys::Bus` 同一个理由：消息处理器里不能进窗口代码，且不带 viewport 的重绘请求会唤醒别的窗口；
- `note/panel.rs` 的 `set_tools(open, reason)` 成为唯一写 `tools_open` 的地方，日志因此能分清 `reason=button|shortcut|body`。三个入口一个出口，是"为什么这次关了"可回答的前提。

### 门禁为什么把正文点击放在最后

这是实测逼出来的顺序，不写下来下一个改脚本的人会重新踩：**合成的一次点击如果在 RichEdit 上释放，会留下鼠标捕获状态，之后父窗口收到的光标坐标会按子窗口原点解释**——把 client (300,176) 报成 (182,31)，于是"点正文之后再点按钮"必然落空，还会顺手把窗口拖走。真实鼠标的释放会清掉这个状态，所以用户看不到；门禁看到的是注入层的毛病。

因此 `check-native-tools.py` 的判定顺序是：按钮开 → 岛让位 → 按钮关 → 岛复位 → **最后**才点正文。这条约束写在脚本头部，和"为什么必须真按键"写在一起。

### 顺带

`scripts/lib/wininput.py` 长出 `click` / `click_client` / `client_box`：`MOUSEEVENTF_ABSOLUTE` 的坐标是全桌面 0..65535，换算放在库里；`INPUT` 的 40 字节从注释变成了断言（写错时 `SendInput` 返回 0 + 87 且静默）。

### 证据

| 门禁 | 判什么 | 结果 |
|---|---|---|
| `scripts/check-native-tools.py` | Q7 开/关/点外部关、岛让位与复位、点击不关窗 | 8/8（连跑 3 次稳定） |
| `scripts/check-native-notes.py` | 1d 的 Q20–Q23 + 按 id 重开（回归） | 24/24 |
| `scripts/check-native-switcher.py` / `check-native-manager.py` | 2b/2c 回归 | 22/22、19/19 |
| `cargo test --release --workspace` | 含新的 z-order 单测与 `WM_LBUTTONDOWN` 上报单测 | 193 项 |

### 与旧版的差异（有意）

| 项 | 旧版 | 本批 | 为什么 |
|---|---|---|---|
| 点外部关闭 | 页面任意处 `click` 关面板 | 正文点击 + `•••` + `Ctrl+.`；标题条本身按住是拖窗，不算"外部点击" | 标题条是标题栏语义（按住拖动），把它也当"外部"会与拖动打架 |
| 按钮点击 | React 按钮，天然可点 | egui 控件 + 拖拽面排序 + `StartDrag` 守卫 | 原生侧控件叠放由注册顺序决定，不写清楚就会静默失灵 |

## 8.11 置顶按钮：一次「成功」的置顶把窗口弄没了（2f，2026-11-26）

Q9 在 parity 里是 ◐，挂的理由是"待目视"。补门禁时先写的是一条日志断言——点一下得 `note_pin pinned=false`——它一次就过了。然后随手加了一条"窗口还在吗"，答案是**不在**。

### 真 bug：`WindowLevel` 会重写整套窗口样式

`toggle_pin` 原来发的是 `ViewportCommand::WindowLevel(AlwaysOnTop|Normal)`。winit 0.30 在 Windows 上把它翻成 `WindowFlags::ALWAYS_ON_TOP` 的一个位，然后**用缓存的整套 `WindowFlags` 重算 `WS_*` / `WS_EX_*`**（`window_state::WindowFlags::apply_diff`）。

问题在于本窗口不是 winit 建出来的那个窗口：

- 建的时候是 `with_visible(false)`（`ViewportBuilder`），所以 winit 记的 `VISIBLE` 是 false；
- 真正显示它的是 `hub_win::window::move_on_screen`（`SetWindowPos` + `SWP_SHOWWINDOW` + `HWND_TOPMOST`），这一步 winit 完全不知道。

于是第一次改 level，winit 按自己的记录把样式重算成"不可见"，`WS_VISIBLE` 被抹掉，窗口从桌面上消失。实测（2026-11-26）：

```
start:      visible=True  topmost=True   style=0xc0118
after click: visible=False topmost=False  style=0x40110   note_pin pinned=false
```

连 `WS_EX_NOACTIVATE`（0x80000）也一起没了——同样是因为它只在 winit 之外设置过。窗口句柄还在、进程还活着、编辑器里的字也没丢，但用户看到的是"点了置顶，笔记没了"。

**Q16 一直是 ✓ 的原因就在这里**：那条只断言 `note_pin pinned=false`。日志是对的，行为是错的，而门禁只读了日志。这次补的不是功能，是**判据**。

### 修法：只动 z-order 位

`hub-win::window::set_topmost(hwnd, on)` 直接 `SetWindowPos(HWND_TOPMOST|HWND_NOTOPMOST, SWP_NOMOVE|SWP_NOSIZE|SWP_NOACTIVATE)`：z-order 是唯一需要改的东西，样式表的其他位一概不碰。`SWP_NOACTIVATE` 让"取消置顶"不把键盘从用户正在打字的窗口抢走。

`note/panel.rs::toggle_pin` 因此把两个状态绑在一次调用里：先翻 `strip.pinned`，再用同一次 `set_topmost` 去问系统，日志写 `pinned=… topmost=true|false|no-window`。高亮和 z-order 不可能各说各话——把它们分开写，就是下一个"图标变灰了但窗口还压着别人"的 bug。

### 高亮怎么判：数填充，不认颜色

Q9 的另一半是"图标变空心灰"。这属于画出来的东西，日志答不了，于是给门禁加了取像素：`scripts/lib/winshot.py` 用 `PrintWindow(PW_RENDERFULLCONTENT)` 抓客户区（不用屏幕 `BitBlt`——窗口被盖住时它读到的是桌面，而"窗口还在不在"正是这一批要证的事）。

判据是**填充面积**而不是颜色：置顶是实心书签、浮动是空心轮廓，在图标中心取 11×11 物理像素数亮点，实心 116 个、空心 6 个，差一个数量级；换配色也不会翻结论。中心像素的颜色只写进日志，供人读失败时一眼看出是"没重绘"还是"画错了"。

实测三态：

| 状态 | `WS_EX_TOPMOST` | 中心亮点数 | 中心像素 |
|---|---|---|---|
| 起始（置顶） | 有 | 116 | `(134,171,220)` accent |
| 点一次 | 无 | 6 | `(31,46,68)` 条底色 |
| 再点一次 | 有 | 116 | `(134,171,220)` |

### 门禁自己的坑：两次点击会被合成双击

第一版门禁连跑三次，每次都有一项要重试一次才过。原因是注入层不是用户：两次合成点击若在 `GetDoubleClickTime`（本机 500 ms）内落在同一个 4 px 的 `SM_*DOUBLECLK` 框里，系统会把它们配成一次双击，egui 的 `clicked()` 只对前半次为真。门禁点完第一次立刻读像素、读完立刻点第二次，间隔常常不到 500 ms，于是第二下静默丢失。

修法两条一起：两次点击至少隔 0.6 s，且左右交替偏 8 px（离开 4 px 双击框，仍在 42 px 的按钮内）。改完连跑三次 11/11，没有一条重试记录。

### 证据

| 门禁 | 判什么 | 结果 |
|---|---|---|
| `scripts/check-native-pin.py` | Q9 起始置顶、点一次取消（OS 位 + 空心图标）、窗口仍在、不关窗、再点恢复、点击不挪窗 | 11/11（连跑 3 次稳定） |
| `scripts/check-native-tools.py` | Q7 回归（同一套点击路径） | 8/8 |
| `scripts/check-native-notes.py` | 1d 数据回归 | 24/24 |
| `scripts/check-native-manager.py` | 2b/2c 回归（含 `Ctrl+T` 改键路径） | 19/19 |
| `cargo test --release --workspace` | 全工作区 | 193 项 |

### 顺带

`note.rs` 又长到 806 行，`toggle_pin` 移到 `note/panel.rs`（和 `toggle_tools` 同一个出口），回到 782 行。移动只改了 `pub(super)` 与文件位置，日志格式不变。

## 8.12 拖动标题条：自己搬窗口，比等 OS 移动循环可靠（2g，2026-11-26）

Q10 的代码一直"看起来做了"：`quicknote::draw` 在拖拽面上报 `drag`，`note.rs` 收到就发 `ViewportCommand::StartDrag`。补门禁时量了 `GetWindowRect`——**窗口一动不动**。

### 先确认不是笔记特有的毛病

三步排除，每步都量窗口矩形：

1. `StartDrag` 确实发了（`note_drag phase=begin` 有日志），窗口仍不动；
2. winit 的做法是 `PostMessage(WM_NCLBUTTONDOWN, HTCAPTION)`（`platform_impl/windows/window.rs` 的 `handle_os_dragging`），我绕过 winit 直接给笔记窗口投这条消息：不动；
3. 用 ctypes 现建一个普通 `WS_OVERLAPPEDWINDOW`，`SetForegroundWindow` 后同样投 `WM_NCLBUTTONDOWN`（还试了 `SendMessage` 版本），鼠标按住并移动十二步：**还是不动**。

所以不是笔记的 no-activate/popup 样式，也不是 winit 的封装——这套注入环境里 OS 的模态移动循环根本没接管。真实鼠标拖动很可能没事（真实拖动的移动来自硬件队列），但这条路径在本机无法用它验证，而且它对无激活窗口本来就依赖"系统愿意替你搬"。

### 改法：每帧按光标位移搬窗口

`app/src/note/drag.rs`：

- 按下时记下 `GetCursorPos()`；之后每帧取一次当前光标，算出与上一帧的差，`client_origin + 差` 交给 `hub_win::window::move_to`（`SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE`）；
- 松开由 egui 的 `pointer.primary_down()` 判——它知道这次按压起于本窗口，窗口外松手也能正常收尾，不会把窗口粘在光标上。

**光标必须来自屏幕坐标。** 第一版若用 egui 的 `interact_pos`，位移恒为 0：窗口一动，光标在客户区里的位置就跟着回到原处，差值永远是零。`GetCursorPos` 是那个"窗口在动、它不动"的参照系。

**为什么是逐帧增量而不是"按下点 + 总位移"。** 第一版按 `起点 + (光标 − 按下时光标)` 算，逻辑上更直接，但它把锚点的误差一次性算进结果：有一次实测 96 px 的手势把窗口搬了 107 px，因为锚点读在光标到达按下点之前。改成逐帧增量后误差不累积，锚点晚一帧只表现为起步慢一帧。

### 门禁

`scripts/check-native-drag.py`（9 项）：

| 判什么 | 怎么判 |
|---|---|
| 左/中/右槽各拖一次，窗口跟随 | 每轴位移与光标行程相等（容差 1 px）。中槽取 `width*0.35`（不是正中——正中是 `•••`），右槽取置顶按钮左侧 30 px 的空隙 |
| 每次拖动都有始有终 | `note_drag phase=begin` / `phase=end` 各 +3 |
| 按在置顶按钮上拖 | 窗口原地不动，且 `note_pin` 行数不变（拖动不是点击） |
| 按在 `•••` 上拖 | 窗口原地不动，`note_tools` 行数不变 |
| 收尾 | 进程活着、窗口可见、位置与拖动结果一致 |

两个来自实测的门禁细节：

- **进程的第一次按压有时会被吞**（Q9 的门禁也栽过），所以先做一次结果不计的"热身拖动"，再开始量；某次测量若位移为 0 才重试一次，**位移不对不重试**——107 px 那个 bug 正是"位移不对"，重试会把它盖掉。
- 拖动的方向按窗口在屏幕上的位置选（朝桌面中心），三次同向拖动不会撞到屏幕边缘。真撞上时 `SetWindowPos` 不夹紧，位移会小一截，门禁报的是"没跟上"，不是"窗口飞了"。

### 证据

| 门禁 | 结果 |
|---|---|
| `scripts/check-native-drag.py` | 9/9（连跑 4 次稳定） |
| `scripts/check-native-pin.py` / `check-native-tools.py` | 11/11、8/8（同一套点击路径回归） |
| `scripts/check-native-notes.py` / `check-native-manager.py` / `check-native-switcher.py` | 24/24、19/19、22/22 |
| `cargo test --release --workspace` | 193 项 |

### 顺带

`note.rs` 拆出 `note/drag.rs` 后是 786 行（`drag.rs` 105 行）。§8.10 里"按在按钮上不启动 `StartDrag`"的判断没有变，变的只是"接下来由谁搬窗口"。

## 8.13 数据恢复：一份草稿，三个回答（2h，2026-11-26）

Q24 一直是 `✗`，而且它是唯一一条与"数据安全"直接相关的 parity。这一批把它连起来：写草稿（`save`）、判草稿（`backup`）、问草稿（`note/recover` + `hub-ui/recover`）。

### 草稿从哪来，和现版为什么不同

现版把草稿放在 WebView 的 `localStorage`，键 `easyflowhub_note_autosave_{id}`，值是 `{title, content, tags, timestamp}`（毫秒）。原生窗口没有 origin，也读不到 WebView 的 profile，所以同样的记录写到 `{store_root}/autosave/{id}.json`，**字段逐个相同**（`easyflowhub-app/src/hooks/useBackupRestore.ts` 的 `BackupDraft`）。

字段相同不是为了互通——**两边永远读不到对方的草稿**，这一点写进了 README 的"已知捷径"。相同是为了：门禁能手工造一份与现版同形的文件，以及草稿将来搬进表时列名已经对齐。

写入时机是 `save::Saver::stash`：在 `tick` 决定"要保存"之后、真正写库之前落一次盘，因为**写库失败时这份文件是唯一的副本**。写库成功后 `clear_stash` 删掉它——先删后写会开一个"两份都不存在"的窗口。`Skipped("unchanged")` 也清：光标/段落标记造成的伪差异会让 `tick` 误判一次 dirty，留下的草稿与行完全相同，白白换来下一次打开的一条 `removed=identical`。

### 什么时候问

`backup::inspect` 照抄 `useBackupRestore` 的两个条件，**两个都是"删除而不问"**：

| 情况 | 处理 | 为什么 |
|---|---|---|
| 草稿与行相同 | 删除 | 上一次保存已经成功，只是删除没来得及；问它是问"要不要保留你已经有的东西" |
| `timestamp` 早于 `updated_at` | 删除 | 草稿写在一次已经入库的编辑之前，恢复它会倒退 |
| `updated_at` 读不出来 | 问 | 宽松方向：宁可多问，也不删一份只是没看懂的文件 |
| JSON 读不出来 | 不问也不删 | 半写的文件可能还能手工抢救，删了就没了 |

"保留当前"**删除文件**，这一点比现版更彻底：现版的 `clearBackupDraft` 只关对话框，同一个问题下次打开会再问一遍，用户每次都要点一次"使用当前版本"。按钮写着"使用当前版本"，那就应该把它执行掉。

### 三个回答各自写什么

| 按钮 | 编辑器 | 库 | 草稿 |
|---|---|---|---|
| 使用当前版本 | 不动 | 不动 | 删除 |
| 合并到当前内容 | 当前正文 + `\n--- 备份内容 ---\n` + 备份正文；标签取并集；标题不动 | `flush` 立即写 | 成功后删除 |
| 恢复备份版本 | 备份的显示文本 | `flush` 立即写 | 成功后删除 |

合并的文本与 `useNotesArchive.ts::onMergeDraft` 逐字相同（`[当前, '', '--- 备份内容 ---', 备份].filter(Boolean).join('\n')`，注意 `filter` 之后是**单**换行）。恢复/合并都走 `flush` 而不是等 900 ms：用户刚点完"恢复我的备份"就关窗，不能把刚恢复的文字再丢一次。

### 卡片：手画矩形，因为门禁要点它

`hub-ui::recover::draw` 不排版，所有矩形是卡片尺寸的算术结果，并把三个按钮的 `Rect` 随 `Shown` 返回。看着像在重造布局引擎，实际是：门禁必须点中按钮，而**唯一可靠的坐标来源是画它的那个函数**。若在脚本里硬编码坐标，padding 改 2 px 门禁就红，红的理由与行为无关；更糟的是按钮被画到够不着的地方时，硬编码的点击落在空白处，**同样没有日志**——门禁分不出"没点中"和"点了没反应"。

`note/recover.rs` 把这三个矩形以物理像素写进 `recover_rect`（`card=` / `screen=` 一并记，便于看失败），门禁从日志读坐标再点。

三个实测的坑：

- **egui 第一帧的 `screen_rect` 是占位值**：实测 216×356，而真实客户区是 400×268 逻辑。按它居中会把"使用当前版本"画到 `x = -45`（窗口外）。现在 `recover_frame` 在 area 小于卡片下限时直接跳过并请求下一帧——晚一帧，换来坐标有意义。
- **按钮行会溢出**：固定 126 px 一个时，卡片被 clamp 到 280 宽而三个按钮要 398 px。现在按钮宽度是"卡片可用宽度除以三"与 126 的较小值。
- **卡片必须是模态的**：scrim 用 `ctx.screen_rect()`（整窗）而不是正文矩形，并注册一个 `Sense::click` 的交互面。否则标题条的 `•••`、置顶、关闭仍然可点——一个能点掉标题条的"模态"不是模态。门禁直接量这一点：浮层期间点在 `•••` 上，`note_tools` 行数不变。

编辑器是子窗口，卡片画在它下面等于没画——所以浮层出现时 `editor.hide()`，回答后 `show()` + `focus()`。门禁顺手验了这一条（`IsWindowVisible(control)`），因为"点完按钮发现打不了字"是这类浮层最常见的毛病。

### 门禁

`scripts/check-native-recover.py`（15 项）。它自己往临时库插一条内容已知的笔记，再手写草稿文件，所以每个场景的期望文本是常量：

| 判什么 | 怎么判 |
|---|---|
| 草稿比行新 → 出卡片 | `note_recover ask=true` 恰好 1 行，且 `recover_rect` 给出三个按钮 |
| 三个按钮在窗口内 | 用 `client_box` 的宽高逐边比较 |
| 模态 | 点 `•••` 后 `note_tools` 行数不变 |
| 恢复 | 行变成草稿的 title/content/tags；草稿文件消失 |
| 合并 | 正文等于 `当前正文\n--- 备份内容 ---\n备份正文`；标签 `当前 备份`；文件消失 |
| 保留当前 | `updated_at` 与点击前完全相同；文件消失 |
| 编辑器回来了 | `IsWindowVisible(RichEdit)` 为真 |
| 答完不再问 | 重启后 `ask=true` 为 0，`note_recover` 只有 `found=false` 一行 |
| 陈旧草稿 | `removed=older`，文件消失，无 `ask` |
| 相同草稿 | `removed=identical`，文件消失，无 `ask` |
| 打字仍写草稿 | 新进程里输入"再写一行"，2 秒内文件出现且含这三个字 |

两个注入层的新事实（已写进脚本注释）：

- **三个 `WM_CHAR` 连发会丢最后一个**：三次运行里失败两次，草稿只拿到"再写"。改成两次发送、中间停 0.15 s 后稳定；重试条件也从"文件存在"改成"文件内容含目标文字"——`stash` 可能写不止一次，第一个文件不是"按键到了"的证据。
- 关闭后立刻重启时，**上一次的输入还在 debounce 里**，所以门禁在答完"保留当前"后等 1.4 s 让 autosave 落库，否则下一次打开会（正确地）问起那条未保存的行。

### 证据

| 门禁 | 结果 |
|---|---|
| `scripts/check-native-recover.py` | 15/15（连跑 3 次稳定） |
| `scripts/check-native-tools.py` / `check-native-pin.py` / `check-native-drag.py` | 8/8、11/11、9/9 |
| `scripts/check-native-notes.py` / `check-native-manager.py` / `check-native-switcher.py` | 24/24、19/19、22/22 |
| `cargo test --release --workspace` | 205 项 |

### 顺带与遗留

`save.rs` 长到 866 行，按守卫拆成 `save/mod.rs`（570）与 `save/tests.rs`（294）；`note.rs` 加完 Q24 是 805 行，把"打开时的判定"收进 `recover::start` 并压掉三处注释后 798。

**遗留一个真问题**：光标停在文档末尾，而带标签的笔记末尾是 `# …` 行——用户想续写正文，字却进了标签。`check-native-notes.py` 的 Q-L 已经把这件事当成现状断言了（`tags == 旧 tags + "尾"`）。它不是 Q24 的一部分，但这属于"打开笔记就想接着写"的日常路径，应单独立项。

## 8.14 光标落点：文档末尾不是正文末尾（2i，2026-11-26）

上一节留下的问题。表现是一句话：打开一条带标签的笔记、直接打字，字进了标签。`# 工作 重要` 加上一个 `尾` 之后，`parse` 仍认这一行是标签行，但标签列变成 `工作 重要尾`——两个标签变成一个，而且正文一个字没多。

### 两个原因，叠在一起

**一、位置选错了。** Q19 的判据写的是"光标在末尾"，而带标签的笔记末尾是标签行。标签行在打开时是被 `format_for_display` 拼回去的，所以"末尾"落在标签里。正确的位置是**正文末尾**，也就是正文与标签行之间那个换行的位置——在那里打字，字接在正文最后一行后面，标签行还是最后一行。

`hub_core::note_text::caret_offset` 回答这个问题，返回字节偏移：末行是标签行 → 返回该行前那个换行的偏移；末行不是标签行（含末尾空行）→ 返回文本长度；整篇只有一个标签行 → 也返回长度（上面没有正文可接，扩标签是唯一合理的动作，放到 0 会把 `#` 推到文字后面、把标签变成正文）。

**二、单位也错了。** 第一版按 UTF-16 单元换算：`买菜清单\r\n鸡蛋\r\n# 工作 重要` 里正文之后的偏移是 8。门禁立刻红了，行变成 `买菜清单\n鸡蛋\n尾# 工作 重要`——字落在标签行**开头**。原因是 RichEdit 的 `EM_SETSEL` 数的是**字符位置**：一对 `CRLF` 算 **1** 个，emoji 算 2 个。8 在它的计数里是标签行的第一个位置。

所以 `hub_editor::position_of(text, byte)` 做字节 → 字符位置的换算（`CR` 计 1，紧跟的 `LF` 计 0，其余按 `len_utf16`）。同一份文本，对的数是 `at=7 of=15`，错的是 `at=8 of=17`，两个数字都写进了 `note_caret` 日志：

```
note_caret id=0 at=7 of=15 tag_line=true
editor_focus id=0 focused=true caret=(7, 7)
```

`caret_to_end` 一直没暴露这个差别，是因为它用 `GetWindowTextLengthW`（UTF-16 长度，比字符位置大）当位置——**超出末尾**，RichEdit 自己夹到结尾。看起来对，其实靠的是越界。

### 改在哪

| 位置 | 改动 |
|---|---|
| `hub-core::note_text::caret_offset` | 新函数 + 4 组单测（标签行、无标签/中间 `#`/末尾空行、只有标签行、正文与标签之间的空行） |
| `hub-editor::caret.rs` | 新文件：`caret_to_end`/`select_all`/`caret` 从 `lib.rs` 搬来，加 `set_caret(位置)` 与 `position_of(文本, 字节)`；`lib.rs` 798 → 775 行 |
| `note/island.rs` | `park(editor, id, log)`：一次算位置、放光标、滚到光标、写 `note_caret`；`NoteShell::park_caret` 给其余两条路径用 |
| `note/panel.rs` | 切换器选中另一条笔记（Q27）也走 `park_caret` |
| `note/recover.rs` | 恢复/合并草稿后同样走 `park_caret`——草稿里也带标签行 |

`set_caret` 把位置夹在文档长度内：越界 RichEdit 自己会夹，但把控制没用过的数字报给 `caret()` 读回来，是那种要到很久以后才付账的静默不一致。

### 门禁

`check-native-notes.py` 的 Q-L 从"把缺陷当现状"改成断言修复：

| 判什么 | 怎么判 |
|---|---|
| 光标停在标签行之前 | `note_caret` 行 `tag_line=true` 且 `at < of` |
| 字进了正文 | 行内容 = 旧内容 + `尾` |
| 标签没被动 | `tags` 与点击前**完全相等**（旧断言是 `tags + "尾"`） |

`check-native-recover.py` 那条"打字仍写草稿"原来把 `content` 与 `tags` 拼起来判——正是这种宽容让缺陷活了下来——现在分开判：字在 `content` 里，`tags` 保持 `当前`（16/16）。

### 证据

| 门禁 | 结果 |
|---|---|
| `scripts/check-native-notes.py` | 25/25（Q-L 前是 23/25，两条红的就是这个缺陷） |
| `scripts/check-native-recover.py` | 16/16 |
| `scripts/check-native-switcher.py` | 22/22 |
| `cargo test --release --workspace` | 212 项（新增 4 项 `caret_offset`、3 项 `position_of`） |
| `scripts/check-native-architecture.mjs` | 通过（5 crates） |

`caret_offset` 与 `position_of` 都是纯函数，所以这一批的两条根因各有一组不需要窗口的测试；窗口里只剩"把数字交给 `EM_SETSEL`"这一步。

## 8.15 尺寸记忆：一组宽高，等窗口停稳再写（2j，2026-11-26）

C5 从"延后"变成"已做"。语义照 parity 原文：**全局一组宽高**，不按窗口记——拖完这扇窗，下一扇窗和下次启动的新笔记都用这个尺寸。

### 什么时候写

拖边缘一次会产生上百个 `WM_SIZE`，每个都写盘是上百次 rename，而文件是给**下一个进程**读的，重要的是拖动停在哪儿、不是经过哪儿。所以 `note/size.rs` 的做法是：

1. 每帧读一次客户区（`GetClientRect`），除以 `pixels_per_point` 换成逻辑尺寸；
2. 尺寸变了就记下"第一次看到它的时刻"，并 `request_repaint_after(400ms)`；
3. 400 ms 内尺寸再变就重置计时；停住了才落盘。

`request_repaint_after` 那一步不是保险，是必需：拖完手一松就没有输入事件了，没有这一帧，"停稳"这件事永远不会被观察到。

### 三个容易写错的地方

| 坑 | 处理 |
|---|---|
| 首帧的尺寸和 `spec` 差 1 px（`逻辑→物理→逻辑` 两次取整，加上 `place_note` 按工作区 clamp） | 第一帧只学基线、不比较；之后 2 px 以内算"没变"。否则每次启动都会写一次假的 resize |
| 写文件会把工作台刚存的改键冲掉（同一份设置文件） | 读改写**文件**（`SpikeSettings::load` → 改 `quick_note` → `save`），不是写内存快照 |
| 文件坏了（`{not json`）时 `load` 返回默认值，`save` 会把它变成一份默认文档 | 加载来源是 `Default` 且文件**存在** → 不写，日志 `saved=false reason=settings_unreadable`。这次 resize 在本进程内仍然生效 |

`Sizer::persist` 成功后同时更新 `Arc<Mutex<(u32,u32)>>`，`Hub::spawn` 从这个 cell 读 spec——所以同一进程里拖完窗口再按 `Ctrl+Alt+N`，新窗口就是新尺寸，不必重启。

### `quick_note.schema`

同批补的版本号，`QUICK_NOTE_SCHEMA = 1`，放在 `quick_note` 第一个字段：

- **缺失或 `0` 按 1 处理**。所有早于这个字段的设置文件，键的形状恰好就是版本 1 定义的，所以"没编号"不等于"不知道是什么"。
- **比 1 大的不降级**。读到一个 `2` 就原样留着，下次写盘还写 `2`：这个版本认识的键含义没变，但那个数字是另一个版本的作者留下的记录，覆盖它就是把"向前兼容的读"变成"悄悄改坏"。

`schema` 的存在不是为了现在分支，是为了将来某次"这个键含义变了"时**能**分支。`serde(default)` 只解决"缺键不报错"，解决不了"同一个键换了意思"。

### 顺带发现：无边框窗口拖不了边缘

门禁先试真拖（`SendInput` 按住右边框往左拖 160 px），再退回 `SetWindowPos`。实测拖不动，查了一下：

```
style=0x16cf0000 thick=True popup=False caption=True
hit_test inset=1..12 at=right-1..right-12 -> 1 (HTCLIENT)
```

样式里有 `WS_THICKFRAME`，但 `WM_NCHITTEST` 对窗口边缘一律返回 `HTCLIENT`——所以 OS 的调整大小循环根本没有入口。这不是 C5 的缺陷（尺寸从哪来变，C5 都记），但它让 Q3 判据里的"拖到最小仍可点标题条三个按钮"少了前提：**鼠标拖边缘改不了尺寸**。

修法有两条路，都不在 C5 内：一是像 `note/drag.rs` 那样自绘（边缘 5 px 内按下 → 每帧按光标增量 `SetWindowPos`，可被注入测到）；二是把边缘 hit test 交回 `DefWindowProc`（发 `WM_NCLBUTTONDOWN` 让 OS 接管，但 Q10 已经证明本机注入下 OS 移动/调整循环不接管，测不到）。**2k 走了第一条**，见 §8.16；`check-native-size.py` 因此从 `drag=programmatic` 变成 `drag=real`。

### 改在哪

| 位置 | 改动 |
|---|---|
| `hub-core::settings` | `QUICK_NOTE_SCHEMA`、`MAX_NOTE_WIDTH/HEIGHT`、`QuickNoteSettings.schema`、`sized(宽,高)`（夹到 150×60…8000×8000）、`normalized()`（0 → 1，不降级）；3 个单测 |
| `app/src/note/size.rs`（新） | `Sizer`：基线、静默期、2 px 容差、读改写文件、共享 cell；`NoteShell::track_size`；5 个单测 |
| `app/src/note.rs` | `Spawn` 带 `size` 与 `settings_path`；每帧调用一次 `track_size`；`apply_alpha` 搬到 `panel.rs`（文件撞 800 行上限，行为不变） |
| `app/src/hub.rs` | `note_size` 单元；`spawn` 从它读宽高（不是启动快照） |

### 门禁

`scripts/check-native-size.py`，11 项，两个进程：

| 判什么 | 怎么判 |
|---|---|
| 按设置开窗 | 种子文件 `400×300`（无 `schema`），客户区（物理）= `round(400×scale)` |
| 没拖就不写 | 启动 + 静默期后设置文件**逐字节**与种子相同 |
| 拖了会写 | 新宽度写进 `quick_note.width`，高度不动 |
| 写的时候带上版本 | `note_size` 日志 `saved=true schema=1`；文件里 `schema=1` |
| 别的键不丢 | `appearance.font_pt`、`shortcuts.switcher`、`margin` 都在 |
| 窗口不被弹回 | 写盘后客户区宽度 == 拖动后的宽度 |
| 下一个进程接着用 | 重启后客户区 = 记住的宽度 × scale，且文件没被再写一次 |

`drag=real|programmatic` 记在报告里，如实说明这次是哪种输入改的尺寸。2k 之前边缘拖不动，一直是 `programmatic`；Q3 补上自绘边缘后是真拖（`drag=real 600 -> 360`）。

### 证据

| 门禁 | 结果 |
|---|---|
| `scripts/check-native-size.py` | 11/11（2k 之前 `drag=programmatic`，之后 `drag=real`） |
| `scripts/check-native-notes.py` | 25/25 |
| `scripts/check-native-switcher.py` | 22/22 |
| `scripts/check-native-recover.py` | 16/16 |
| `scripts/check-native-tools.py` / `pin` / `drag` / `manager` | 8/8、11/11、9/9、19/19 |
| `cargo test --release --workspace` | 220 项（新增 3 项设置、5 项 `Sizer`） |
| `scripts/check-native-architecture.mjs` | 通过（5 crates，`note.rs` 779 行） |

## 8.16 边缘 resize：一条边要自己画，因为系统没有那条边（2k，2026-11-26）

§8.15 顺带测出来的事，Q3 必须自己收尾：无边框窗口没有系统调整边框。样式里有 `WS_THICKFRAME`，但 `WM_NCHITTEST` 对每条边都回 `HTCLIENT`，OS 的 resize 循环根本没有入口。所以"拖到最小仍可点标题条三个按钮"这句话里的"拖"，得由应用自己实现。

### 一条边，两个窗口

笔记是两扇窗口，按下位置不同，消息落点也不同：

| 按住哪里 | 消息到哪 | 怎么看见 |
|---|---|---|
| 标题条（含左上/右上角） | egui 的 strip 拖拽面 | `quicknote::draw` 给出 `actions.drag`，用 `pointer.press_origin()` 判是不是落在左右band里 |
| 正文区（左右边、底边、右下角） | RichEdit 子窗口 | `hub-editor` 新增的 press 钩子：`WM_LBUTTONDOWN` 先问外壳"这是你的吗" |

第二条的关键是**能认领**（`on_press` 回 `true` 时消息不再往下传）。不认领的话，一次边缘按下会同时做三件事：窗口开始变大、光标被放到按下点、文本开始被框选——而框选会跟着手指跑，松手后还得再点一下才能取消。

### 每帧跟一次，而不是交给模态循环

和 `note/drag.rs` 同构：没有模态循环可以托付，所以每帧问一次窗口在哪儿、光标走了多远，然后 `SetWindowPos` 一次同时改位置和尺寸。按钮状态用 `GetAsyncKeyState`——边缘拖动期间鼠标消息在 RichEdit 那边，egui 的 `pointer.primary_down()` 只认落进 egui 自己窗口的按下。

三条约束写进纯函数 `resized()`，由 7 个单测钉住：

- **没按住的边不动**：拖左边只改 `x` 与 `width`，右边缘必须是常量；
- **下限自己夹**：`with_min_inner_size` 由 `WM_GETMINMAXINFO` 生效，但 OS 只拒绝尺寸、不会拒绝移动——不自己夹就会出现"窗口一边往左滑、一边变不窄"的抖动。夹到 150×60 时，左边缘拖动要把 `x` 钉在 `右边缘 - 150`；
- **一次调用两轴都改**：左边缘是"移动 + 缩放"，分两次 `SetWindowPos` 会有一帧只生效一半。

### band 是 5 逻辑像素，且没有上边

- 5 px：原生 resize 边框是 `SM_CXSIZEFRAME + SM_CXPADDEDBORDER`（100% 下 8 px），但这扇窗没有边框可躲，band 就是可见的边缘本身，5 px 比 8 px 更"对得上眼睛"。
- 没有上边：上边是标题条，按住它就等于移动窗口（Q10）。同 5 px 两个含义会让最常用的手势在条顶失效。
- 窗口太窄时（小于 4 个 band）一律不认：创建期有一帧客户区是 1 px，那时"左右都算"会让两个分支互相抵消。

### 光标也要跟着改

一条只有按下去才知道存在的边，等于没有。两扇窗口各答一次：

- 子窗口：`WM_SETCURSOR` 里问同一个 band，命中就 `SetCursor(IDC_SIZEWE / IDC_SIZENS / IDC_SIZENWSE / IDC_SIZENESW)` 并认领消息，没命中就交给 RichEdit 保留 I 形；
- 标题条：egui 侧 `ctx.set_cursor_icon(ResizeHorizontal)`，否则同一条边在 y<32 的地方变成箭头。

### 改在哪

| 位置 | 改动 |
|---|---|
| `app/src/note/edge.rs`（新） | `Grip` / `grip_at()`（纯函数）、`resized()`（纯函数）、`Signal`（press + hover 两个钩子共用一次计算）、`Resize`（每帧跟光标）；7 个单测 |
| `crates/hub-editor/src/dispatch.rs` | `Press`/`Shape`；`WM_LBUTTONDOWN` 先问 `on_press`（可认领），`WM_SETCURSOR` 走 `on_hover`；7 个回调安装方法搬到这里（`lib.rs` 撞 800 行）；新增 3 个单测 |
| `crates/hub-win/src/window.rs` | `set_frame`（移动+缩放，`SWP_NOZORDER|SWP_NOACTIVATE`）、`window_rect`、`left_button_down`（`GetAsyncKeyState`） |
| `app/src/note.rs` | `mod edge`、`Resize`/`Signal` 字段、strip 上的 band 判定与光标、每帧 `resize_frame` |
| `app/src/note/island.rs` | 装 `on_press` / `on_hover` 两个钩子 |

### 门禁

`scripts/check-native-edge.py`，23 项，全部真实输入（`SendInput` 按住边拖）：

| 判什么 | 怎么判 |
|---|---|
| 右边拖 120 | 宽度 +120、高度不变、原点不变 |
| 左边拖 -90 | 原点 -90、宽度 +90、**右边缘常量** |
| 底边拖 70 | 高度 +70，位置与宽度不变 |
| 右下角拖 (60,40) | 两轴同时变 |
| 标题条里的左边 band | 走的是 resize 不是移动（原点 +80、宽度 -80） |
| 光标 | 右/下/右下/条内 band 各是 `IDC_SIZEWE/SIZENS/SIZENWSE/SIZEWE`，正文是 I 形（`0x10005`） |
| 边缘按下不是正文点击 | 先开工具面板，再拖边 → 面板仍开（认领生效）；点正文 → `note_tools open=false reason=body`（对照） |
| 下限 | 宽度夹到 225 物理（150×1.5）、高度夹到 90（60×1.5），原点不被拖走 |
| 岛跟随 | 最后一次 `island_rect` 宽 == 客户区宽，高 == 客户区高 − 条高 |
| 与 C5 合流 | 停稳后设置文件里的宽高 == 当前客户区 ÷ scale |
| 日志 | 每次手势一对 `note_resize phase=begin/end`，`grip=` 覆盖 right/left/bottom/bottom-right |

### 证据

| 门禁 | 结果 |
|---|---|
| `scripts/check-native-edge.py` | 23/23（连跑 3 次） |
| `scripts/check-native-notes.py` | 25/25 |
| `scripts/check-native-tools.py` / `pin` / `drag` / `size` | 8/8、11/11、9/9、11/11 |
| `scripts/check-native-switcher.py` / `recover` / `manager` | 22/22、16/16、19/19 |
| `cargo test --release --workspace` | 230 项（新增 7 项 band/夹取、3 项消息路径） |
| `scripts/check-native-architecture.mjs` | 通过（`note.rs` 752、`lib.rs` 733、`dispatch.rs` 603） |

### 没做的

- **双击标题条最大化**：不属于 Q3，也不在原版行为里。
- **上边缘 resize**：见上，条就是"移动"。
- **贴边吸附 / 多屏交界**：Q4 的目视项，与 resize 无关。

## 8.17 store 的 schema 版本号：算出来的，不是盖上去的（2l，2026-11-26）

从 1d 起，"`store` 没有 schema 版本号"一直挂在"已知捷径"里，理由是现库 `user_version = 0`、单方面打号会让两套迁移分叉。这次收掉这件事，但**不是**去打那个号——三条理由，每条都能被现有门禁反驳：

1. **`user_version` 是别人的字段。** 现库是 0，S3 门禁的 `measure-native-store.ps1` 明确断言原生写之后它**仍然是 0**（`user_version untouched` / `still 0`）。往里写数字等于悄悄改契约，下一个按版本号写迁移的人会撞上一个"列早就在了"的库。
2. **只有一方维护的号是负资产。** Tauri 若加了迁移而原生侧还写着旧号，"你是最新的"这句话就是假的；这种缓存比没有更坏。
3. **要的不是数字，是"哪几级已经在文件上"。** 这个问题有可判定的答案：探测每一级自己的产物。

所以 `SCHEMA_VERSION = 6` 是"这个构建会建到第几级"，`Schema::version` 是**量**出来的：自下往上，最高一级探测通过的编号。文件上有笔记的三级、没有收件箱，答案就是 3 而不是 6——不连续就停，因为"第 5 级在、第 4 级不在"说明不了梯子爬到了哪。

### 六级梯子

| 级 | 名字 | 探测什么 | 建什么 |
|---|---|---|---|
| 1 | `notes` | `notes` 表在 | `NOTES_DDL`（表 + 三个索引） |
| 2 | `notes.tags` | `tags` 列 **且** `idx_notes_tags` | 缺列则 `ALTER`，再建索引 |
| 3 | `notes.deleted_at` | 列 **且** `idx_notes_deleted_at` | 同上 |
| 4 | `inbox` | 两张收件箱表都在 | `INBOX_DDL` |
| 5 | `inbox_items.deleted_at` | 列 **且** `idx_inbox_items_deleted_at` | 同上 |
| 6 | `inbox_attachments.source_path` | 列在 | `ALTER ... source_path` |

`init_schema` 只做一件事：自下往上，探测失败的那一级就建。`settings` 不在梯子上——它没有会演化的列，"没有设置"不等于"落后"，所以照旧无条件 `CREATE TABLE IF NOT EXISTS`。`Store::open` 在梯子没到顶时返回错误而不是交出连接：半迁移的连接会让后面每条查询都答一个没人声明过的形状，失败点会跑到一次保存里，而那时已经看不到原因。

### 顺带修掉三个真缺陷

| 缺陷 | 现象 | 现在 |
|---|---|---|
| 索引建在"加列"的 `if` 里 | 索引被别的工具删掉后永远补不回来（探测只问列在不在），`sqlite_master` 与 Tauri 建的库渐渐不同 | 每级探测同时要求列与索引，建索引的语句在 `if` 外面；单测先 `DROP INDEX` 再开门，版本从 2 回到 6 |
| `inbox_attachments.source_path` 只靠 `ALTER` 补 | 原生新建的库该列靠 `ALTER` 追加；Tauri 的基表 DDL 里就有它，而它的 CHECK 重建用**按位置**的 `INSERT ... SELECT` | 基表 DDL 里写 `source_path TEXT`，`ALTER` 只留给旧库；门禁直接断言"最后一列是 `source_path`" |
| 漂移守卫只认 `ALTER TABLE notes ADD COLUMN` | Tauri 在 `CREATE TABLE` 里加列（`source_path` 就是这么加的）、或给收件箱加列，守卫都看不见 | 守卫解析 `notes.rs` 与 `inbox/schema.rs` 两处，把基表列与 `ALTER` 列按顺序合并，与 `NOTES_COLUMNS`／`INBOX_*_COLUMNS` 逐项比较，并额外断言真实库的物理列顺序就是这些 |

### 一件刻意不做：CHECK 重建

`inbox/schema.rs::migrate_capture_kinds_schema` 是唯一没有镜像的迁移：SQLite 改不了 `CHECK`，它要 `RENAME` 两张表、重建、按位置搬数据——全产品最危险的一段 DDL。原生侧只**检测**（`Schema::legacy_capture_kinds`，从 `sqlite_master.sql` 里找 `'audio'`／`'transcript'`），不执行：一个只想来读一条笔记的进程不该重写两张表，而 Tauri 下次启动本来就会做。检测结果不算失败——`judge` 里写明了原因，免得门禁去怪错的那一方。

### 改在哪

| 位置 | 改动 |
|---|---|
| `crates/hub-core/src/store/schema.rs`（新） | `SCHEMA_VERSION`、`Schema`（`version`/`missing`/`ahead`/`legacy_capture_kinds`）、`RUNGS`（六个 `Rung`）、`init_schema`（返回 `Schema`）、`schema()`、未知列探测、CHECK 探测；5 个单测 |
| `crates/hub-core/src/store/mod.rs` | DDL 与探测整块搬进 `schema.rs`；`Store::open` 检查梯子到顶；新增 `Store::schema()`；漂移守卫重写 |
| `app/src/store_bench.rs` | 证据里加 `schema` 对象，`judge` 要求梯子到顶（`legacy_capture_kinds` 不算失败） |
| `scripts/check-native-store-schema.py`（新） | 4 个场景、24 项 |

### 门禁

`scripts/check-native-store-schema.py`，四个场景各一个进程、各一个目录：

| 场景 | 判什么 |
|---|---|
| fresh | 空目录 → `6/6`、无缺无超、`notes` 八列、`inbox_attachments` 最后一列是 `source_path` |
| legacy | 0.1.x 形状（无 `tags`/`deleted_at`、旧 CHECK、无 `source_path`）→ `6/6`；正文还在、`tags` 补成空串、`legacy_capture_kinds=true` |
| ahead | 现库加一列 `notes.color` + 删掉 `idx_notes_deleted_at` → `ahead=["notes.color"]`、`color` 不被删、索引补回、版本仍 6 |
| no-op | 对当前文件再开两次 → 字节数与 sha256 完全相同（开门不写盘） |

四个场景都断言 `user_version` 仍是 0。

### 证据

| 门禁 | 结果 |
|---|---|
| `scripts/check-native-store-schema.py` | 24/24 |
| 真实库副本（39 条笔记 / 9 个设置） | `schema version=6/6 missing=- ahead=- legacy_capture_kinds=false`，`schema unchanged=true` |
| `scripts/check-native-notes.py` | 25/25（G-2 的 schema 指纹仍是 48 个对象） |
| `scripts/check-native-pin.py` / `manager` | 11/11、19/19 |
| `cargo test --release --workspace` | 235 项（新增 5 项：梯子到顶、旧库列集合、索引重建、未知列只报不删、开门不写） |
| `scripts/check-native-architecture.mjs` | 通过（`schema.rs` 585、`store/mod.rs` 453、`store_bench.rs` 783） |

### 没做的

- **给 Tauri 侧也接上这把梯子**：那是 Phase 2 的"统一迁移器"。这版只保证原生量出来的版本与 Tauri 建出来的形状一致（漂移守卫 + 真实库副本 + 未知列报警）。
- **`PRAGMA foreign_keys = ON`**：Tauri 在 `inbox::init_db` 里设了，原生侧没设，所以 `ON DELETE CASCADE` 对原生写无效（`store_attachment` 因此显式查父行）。这是连接级行为，与版本号无关，另立一项。

## 9. 变更记录

| 日期 | 变更 |
|---|---|
| 2026-11-25 | 起草。Q1–Q4、Q6–Q11 进 1a；Q5 → 1b（D5）；Q6 的合成部分 → 1c |
| 2026-11-25 | 1a 落地：`hub-core` 几何 + 设置、`hub-ui::quicknote`、`app::note`、`--note` 入口；修掉首帧两个缺陷（§8） |
| 2026-11-25 | 1a.1 目视打磨：修字号×ppp、岛原点不缩放、行首光标被裁三个缺陷；标题条改自绘图标、去系统滚动条；新增 `quick_note.appearance` 主题配置（§8.1） |
| 2026-11-25 | 1a.2 置顶图标返工：1a.1 自拼的图钉换回旧 UI 的书签路径（两态靠 `fill` 区分），按钮尺寸与配色回到 28×28 / muted-accent，去掉旧 UI 没有的圆角底板（§8.2） |
| 2026-11-25 | 1a.3 标题条留白回到旧 UI 的两层 padding：标签 16、按钮框 8（图标可见 14），`SIDE_SLOT_WIDTH` 44 → 60；正文内边距 12/10 → 18/18 以对齐标签；记下最小窗宽下 `GHOST` 越槽 5 px 但让开三点 6 px（§8.3） |
| 2026-11-25 | 1b 拆成 1b.1（键位，本批）与 1b.2（多窗口/托盘）；1b.1 落地键位表、两条输入路径、`EM_UNDO`/`EM_REDO` 统一、输入法改写 vk 的还原；记下 `Ctrl+.` 只能人工验证（§8.4） |
| 2026-11-25 | 1b.2 落地 viewport-per-note：根视口改为屏外控制器，每笔记一个 deferred viewport；关闭只销毁该窗口，`Ctrl+Alt+N` 经全局热键新建；去掉全局 `set_zoom_factor`；托盘与其余全局键留 1b.3，关完最后一个笔记即退出并在日志写明 `tray=pending`（§8.5） |
| 2026-11-25 | 1b.3 落地托盘常驻 + `Ctrl+Alt+D/H/M` + 关完不退：托盘自带隐藏消息窗口、菜单 id 单表、退出前 `NIM_DELETE`；修掉"关闭打到根视口"（`send_viewport_cmd` → `send_viewport_cmd_to`）；最小化的笔记跳过布岛（§8.6） |

| 2026-11-25 | 1c 落地整窗透明 + 幽灵模式：`hub-win::alpha` 用 `LWA_ALPHA` 让 DWM 整窗混合；`Ctrl+滚轮` 两条通道（岛 `WM_MOUSEWHEEL`、标题条 egui，两边各自的缩放都关掉）+ 50 ms 节流；`Ctrl+Shift+H` 幽灵往返；透明度初值进 `appearance.window_alpha`（§8.7） |
| 2026-11-25 | 1c.2 试做"背景完全消除 + 文字 20%"（`LWA_COLORKEY \| LWA_ALPHA`，面板色作键）：技术上成立（背景与桌面逐值相等），但用户实测观感不对——文字像被描边、背景仍有薄片残留——当日回退到 alpha 1 的幽灵。Mica 会挡色键、子窗口命中不跟随父窗口色键两条实测结论留在 §8.7 供后续参考 |
| 2026-11-26 | 1d 落地正文写库：`hub-core` 加 `clock` / `note_text` / `save_note_body`，`app` 加 `save.rs`（900 ms 防抖 + 2 s 扫描 + 关闭 flush + 失败退避）与 `--note-id`；`note.rs` 拆出 `note/island.rs` 回到 800 行内。Q20–Q23 由 `scripts/check-native-notes.py` 判（24/24）。工作台、切换器、每窗口透明度记忆留给后续批次（§8.8） |
| 2026-11-26 | 2a–2d 落地交互层：`hub-core::shortcuts` 一张表（11 命令 / 2 作用域 / 冲突先者胜）、`hub-ui` 的 `hint`（功能名 + 键位）、`switcher`、`manager` 与 `keys`，`app` 的 `note/panel.rs`、`manager.rs`、`hub/commands.rs`；新建笔记改成立刻 `flush(reason=seed)`（门禁实测缺陷）。Q13/Q16/Q25–Q27 由 `check-native-switcher.py`（22/22）与 `check-native-manager.py`（19/19）判（§8.9） |
| 2026-11-26 | 2e Q7：`hub-editor::on_click`（`WM_LBUTTONDOWN` 上报不吞）、`app::click`、`note/panel.rs::set_tools`；修掉拖拽区盖住按钮的真 bug（顺序 + `StartDrag` 守卫，附 z-order 单测）。`scripts/check-native-tools.py` 8/8（§8.10） |
| 2026-11-26 | 2g Q10：`note/drag.rs` 每帧按 `GetCursorPos` 的位移 `SetWindowPos`（`StartDrag` 在本环境不动窗口，普通窗口探针同样不动）；`check-native-drag.py` 9/9（连跑 4 次），按钮上起拖不动也不误触（§8.12） |
| 2026-11-26 | 2h Q24：`app/src/backup.rs`（草稿读写、判定、合并）+ `save::Saver::stash`/`clear_stash` + `hub-ui/src/recover.rs`（手绘卡片，返回按钮矩形）+ `note/recover.rs`（隐藏编辑器、三种回答）。`check-native-recover.py` 15/15，`save.rs` 拆成 `save/mod.rs` + `save/tests.rs`（§8.13） |
| 2026-11-26 | 2f Q9：置顶走 `hub_win::window::set_topmost`（`ViewportCommand::WindowLevel` 会按 winit 缓存的 `WindowFlags` 重写整套样式，把 `with_visible(false)` 建、`move_on_screen` 显示的笔记窗口直接抹掉——Q16 只读日志所以一直显示正常）；新增 `scripts/lib/winshot.py`（`PrintWindow` 取像素）与 `scripts/check-native-pin.py`（11/11，连跑 3 次）。`toggle_pin` 移入 `note/panel.rs`（§8.11） |
| 2026-11-26 | 2i 光标落点：`hub-core::note_text::caret_offset` 把光标放在正文末尾而非标签行内；`hub-editor::caret.rs`（从 `lib.rs` 拆出）加 `set_caret`/`position_of`——`EM_SETSEL` 数的是字符位置，`CRLF` 只算 1，按 UTF-16 换算会正好落到标签行开头（实测）。打开、切换、恢复草稿三条路径共用 `park_caret`。`check-native-notes.py` 25/25（Q-L 改判"字进正文、标签不动"）、`check-native-recover.py` 16/16；`cargo test` 212 项（§8.14） |
| 2026-11-26 | 2j C5 + schema：`note/size.rs`（每帧读客户区、静默 400 ms 才写、2 px 容差、读改写设置文件、`Arc<Mutex<(u32,u32)>>` 供同进程下一扇窗）；`hub-core::settings` 加 `QUICK_NOTE_SCHEMA`/`sized()`/`normalized()`（缺号按 1、更高版本不降级）。`check-native-size.py` 11/11；`cargo test` 220 项。**顺带实测**：无边框窗口边缘 `WM_NCHITTEST` 全返回 `HTCLIENT`（`style=0x16cf0000`），鼠标拖边缘改不了尺寸——属 Q3，另立一项（§8.15） |
| 2026-11-26 | 2k Q3 边缘 resize：`note/edge.rs`（`Grip`/`grip_at`/`resized` 三个纯函数 + `Resize` 每帧跟光标）、`hub-editor` 的 `Press`/`Shape` 与可认领的 `WM_LBUTTONDOWN`、`WM_SETCURSOR` 光标、`hub-win::set_frame`/`window_rect`/`left_button_down`；回调安装方法搬进 `dispatch.rs`（`lib.rs` 撞 800 行）。`check-native-edge.py` 23/23（连跑 3 次）；回归 25/25、8/8、11/11、9/9、11/11、22/22、16/16、19/19；`cargo test` 230 项（§8.16） |
| 2026-11-26 | 2l store schema：`store/schema.rs` 把迁移改成六级梯子，`SCHEMA_VERSION = 6` 是构建的上限、`Schema::version` 由探测**量**出来（不写 `user_version`，S3 的 G-1 仍要求它是 0）；顺带修掉"索引只建在加列分支里"（`DROP INDEX` 后能自愈）、`source_path` 进基表 DDL、漂移守卫扩到 `inbox/schema.rs` 与 `CREATE TABLE`；CHECK 重建只检测不执行。`check-native-store-schema.py` 24/24；回归 `check-native-notes.py` 25/25、`pin` 11/11、`manager` 19/19；真实库副本 6/6；`cargo test` 235 项（§8.17） |
