# 进展日志 · 2026-11

> 从 [../roadmap.md](../roadmap.md) 拆出：ADR-0001 要求路线图只维护当前版本目标、活跃轨道、验证债与下一队列，不保留长历史。
> 按月归档，**新的条目先写进当月文件**，再把文件加进 `roadmap.md` 的索引。
> 相邻月份：[2026-10](./progress-2026-10.md) · [2026-04](./progress-2026-04.md)

- `2026-11-27` **T-Gate 0 关闭：选区捕获先有门禁，翻译迁到原生**。按用户给的顺序做完三件——先让门禁能失败，再迁实现，全部照 Tauri 版的语义而不是重新发明。
  - **选区门禁（18 项）**：起一个 `--note` 窗口当"用户的应用"，选中哨兵后按热键，把三个单测里不存在的东西搬进断言：另一个进程的选区、系统的剪贴板、真实的按键注入。四个决定：`SWP_SHOWWINDOW` 而不 `focus()`（先抢焦点会把 `Ctrl+C` 打到面板自己身上）、轮询序列号而非固定 sleep、`CAPTURE_BUDGET=45 ms`（60 的 75%）、序列号没动时**不**清理剪贴板（Tauri 版会 `empty()`，毁掉用户刚截的图）。
  - **门禁逼出的第一个真缺陷**：首轮 `reason=no_copy`，笔记从 43 字变成 1 字（`title="c"`）。`WM_HOTKEY` 在组合键物理按住时就投递，而注入的 `Ctrl` up 已把修饰键解掉，目标收到**裸 `c`** 把选区覆盖了——文本插入不动剪贴板序列号，症状看着只是"没复制"。修法：`GetAsyncKeyState` 轮询修饰键松开（上限 12 ms，实测 1.13 ms）；`replace()` 同样要等，否则 `v` 会替换掉译文。
  - **翻译迁移**：纯逻辑（16 个 `Problem`、`body`/`parse`、状态码映射）进 `hub-core`，网络只在 `app`（门禁禁 `reqwest`），卡片在 `hub-ui`。`native-tls` 而非 `rustls`（`aws-lc-rs` 要 cmake/nasm，而唯一请求打在回环）；每请求一条线程 + 自增 id（`blocking` 不能取消，单 worker 会让新请求排在废弃请求后面）。证据：`check-native-translation.py` **31/31**、捕获 18.4 ms、请求 3 ms、替换后笔记就是译文、502 的密钥在日志里出现 **0** 次、空选区零请求。
  - **第二个真缺陷**：替换后笔记里是**上一份剪贴板**。粘贴要在目标的消息循环里处理，而笔记窗口与面板**同线程**——UI 线程阻塞 60 ms 让目标没机会跑，剪贴板已恢复。`replace_selection` 改在独立线程粘贴。这两条都只有原生壳会有。
  - **回归**：12 个原生门禁串行重跑，11 个原样通过；`theme` 修掉两处门禁自身缺陷（写死 5 次方向键 → 按到 `selected_note` 出现、焦点环单点采样 → 沿描边扫一列取最蓝）后 **17/17**；`font` 的 `mapped − none ≤ 6` 是既有判据过期（HEAD 二进制同样 6.8 MB，它量的是"画出这些字要多少内存"，映射是否计费由 `heap − mapped` 回答），改口径后 **11/11**——顺带记账：面板 commit 91.5 → **98.6 MB**，100 MB 线只剩 1.4 MB 余量。详见 [selection-capture-gate](../plans/completed/2026-11-27-selection-capture-gate.md) 与 [translation-migration](../plans/completed/2026-11-27-translation-migration.md)。

- `2026-11-27` **原生壳有了单实例：一个会话一个壳，第二次启动是一条请求而不是第二个进程**。自启 + 手动启动原本得到两个托盘图标和一整轮 `RegisterHotKey failed (1409)`——第二个进程承诺了五个组合，一个都不归它。
  - 三个决定：`Local\` 而非 `Global\`（会话互相独立，RDP 用户各自要一个壳）；用自动重置事件而不是互斥本身传请求（互斥在语义上要等对方退出）；事件和互斥**一起**在 `claim()` 里建——落在这两者之间的信号会被丢掉，而那正是登录竞态，有专门单测。
  - **顺序是承重的**：claim 必须在 `Log::new` 之前，因为它在构造时就 `remove_file`——先开日志的第二个进程会先抹掉第一个的证据。门禁判据因此是"第二个进程一个字节都没写"。
  - 顺带修掉一个从没被按过的入口：`toggle` 先把 `visible` 置 false，UI 线程随后被 `Shell::hide` 的 `if !visible { return }` 挡住，**用热键隐藏从来没有真正停窗**；另外三条隐藏路径都在 UI 线程直接调用、`visible` 仍为 true，所以 25 项门禁全绿。拆出 `park()`，launcher 门禁补两项。
  - 转发早于面板窗口会被丢 → 启动请求是排队的：`reveal_when_ready` 等窗口（≤3 s）且每轮叫帧（窗口是帧建的），实测 82–91 ms。证据：新门禁 **17/17**、第二个实例 **0.09–0.12 s** 退出、`cargo test` **283**、其余全绿；`launcher.rs` 破 800 行 → 拆 `launcher/handle.rs`。详见 [plans/completed/2026-11-27-native-single-instance.md](../plans/completed/2026-11-27-native-single-instance.md)。

- `2026-11-27` **原生常驻入口 `--resident`：零窗口、满热键，`hub.rs` 顺带拆回硬顶之下**。登录自启要的是"什么都没有，但什么都能叫出来"——托盘在、五个组合在、面板已建好藏在屏外。
- `2026-11-27` **主题 token：7 个色槽扩成 21 个语义 token，删掉 5 处自己乘系数的调用点**。乘数没有方向——`gamma_multiply(0.72)` 在暗色面板上是"低一档"，换亮色主题就成了"比脚下的页面还暗"。派生改为 `lift(base, ink, t)`，方向由文字色决定，一条规则两个主题都对。
  - 面板顺带重排：三态可辨、搜索框有聚焦环、分组带计数、页脚 chip、空结果给下一步；列数阈值 96 → 130（4+1 孤卡 → 3+2），`PanelSpec::scale` 修掉物理/逻辑混算。证据：`check-native-theme.py` 17/17 连跑 3 次（采样真实像素），`cargo test` 274 项，五个原生门禁全绿。详见 [plans/completed/2026-11-27-theme-tokens.md](../plans/completed/2026-11-27-theme-tokens.md)。

- `2026-11-27` **Launcher 产品化：面板是预创建、藏到屏外的窗口，按下是移动它而不是创建它**。C2 通过后从 spike 面板变成产品件（§5.1 / §5.8）：440×860 逻辑像素，跟随光标所在显示器的工作区夹紧。
  - 三个不显然的决定：关闭 = 挪到 `-30000,-30000`（`SW_HIDE` 停掉 `WM_PAINT` 投递，eframe 循环停摆，第九次唤起会画出第八次）；揭示在热键线程、隐藏在 UI 线程（`GetFocus` 只回答调用线程），`epoch` 挡住连按交叉；门禁逼出四个真缺陷（空结果 Enter 后交出焦点、点别处不消失、全新目录 `Store::open` 不建父目录、预览重复标题）。证据：`check-native-launcher.py` 25/25 连跑 4 次，p95 2.4–3.2 / p99 8.0–14.1 ms（预算 60 / 100），泄漏按两次运行的增量差判。详见 [plans/completed/2026-11-27-c2-launcher-productized.md](../plans/completed/2026-11-27-c2-launcher-productized.md)。
- `2026-11-27` **C1 关闭：字库不再计入私有内存，常驻 130.1 → 91.5 MB，一个字没少**。S1 记的 37.7 MB「字体 + 图集」拆开看，字库字节是全部、图集只有 0.5 MB（`16384x32`），所以杠杆在字库本身。根因在 epaint：`FontData::from_owned` 走 `Cow::Owned`，`ab_glyph_font_from_font_data` 会 `bytes.clone()` 再建一个 `FontVec`——`msyh.ttc` 19.7 MB 至少一份常驻私有堆。
  - **手段换成映射，不是子集化**：`hub-win::font` 用 `CreateFileW` + `CreateFileMappingW` + `MapViewOfFile` 做只读映射，`FontData::from_static` 取 `&'static [u8]` 走 `FontRef` 零拷贝。不做子集化有三条理由——微软字体子集化产物是分发物（授权另说）、固定子集会丢掉用户任意输入的汉字（豆腐块比 37.7 MB 贵）、省掉字体工具与产物校验的构建链。方向没变，C1 里"三种口径都要记"照旧执行。
  - **数字（同一次会话，三次采样取中位数）**：快速笔记进程 mapped **91.5** / heap 129.1 / none 91.5（Commit）；私有工作集 56.4 / 94.0 / 56.4；工作集 90.6 / 128.0 / 90.7。Launcher 面板 mapped **92.6**（S1 归档的 130.1 就是它的旧值）。**mapped − none = 0.0 MB**，即字库不再向进程要私有内存；工作集只降 37.4 MB 中的一部分，被碰过的映射页仍常驻，只是文件支持、可回收——这一条不掩饰。
  - **波动写进门禁**：同一份二进制连续启动 Commit 在 91.5–99.3 MB 之间跳（GL 暂存、字形缓存时序），幅度与要证明的差值同量级。所以 `check-native-font.py` 每配置跑三次取中位数、打印 min–max，判据用差值而不是绝对值；100 MB 线仍照实断言，但它只是"目标"。
  - **门禁 11 项，核心是三次受控对照**（mapped / heap / none，只差加载路径）：光判"commit 小于 100"会在"字库静默加载失败、面板全是豆腐块"时也变好。另外三项是像素——mapped 与 heap 差 ≤1%（同一张脸，只有字节住处变了）、mapped 与 none 差 ≥2%（实测 4.65%，豆腐块与汉字分得开）。**变异验证**：把 `if !heap` 改成 `if false && !heap` 后四项转红（`RESULT: FAIL`），像素三项仍绿——只有内存那半边的判据该红，符合预期。它也抓到自己第一个缺陷：`capture()` 起初没接 `extra`，三次截图全走 mapped，等于拿一张图和自己比还判了"通过"。
  - **口径补齐**：`hub_win::ProcessProbe` 改读 `PROCESS_MEMORY_COUNTERS_EX2`，一次给出 Commit（`PrivateUsage`）/ 私有工作集 / 工作集三项——G0 §6 记的"原生只记了 Commit，私有工作集没记"到此修掉。
  - **顺带两件**：①`--no-font` 之前只对 spike 生效，产品路径无条件装字体，于是"在 spike 上量成本"与"产品实际行为"是两个程序——现在两条路径共用 `app::font::install_and_log`；②七个原生门禁（notes/pin/tools/manager/switcher/drag/recover）加 edge/size/store-schema 的工作目录默认值从 `native/target/<gate>` 挪到 `%TEMP%`，与 2026-11-26 修的 `measure-native-store.ps1` 同一缺陷：门禁临时状态长在产物目录里。
  - **未做**：65 MB 解释线（属 G1，必须写 ADR）、多屏混合 DPI（C3）、8 小时泄漏（G1）、`lite` 档、字体热替换（映射持文件对象，下次启动才换）。验证：`check-native-font.py` 11/11、`cargo test --workspace` 249、架构门禁 5 crates、回归 notes 25 / pin 11 / tools 8 / manager 19 / switcher 22 / edge 23 / size 11 / recover 16 / drag 9 / store-schema 24 全绿。详见 [plans/completed/2026-11-27-c1-font-not-private.md](../plans/completed/2026-11-27-c1-font-not-private.md)。

- `2026-11-27` **ADR 有了生命周期与季度复查；先盘点的结论是「整篇取代 0 份」**。9 份决策没有一份失效，真问题是记账：ADR-0004 §2 的"背景模糊"实现手段已被 ADR-0009 接管，而 0009 写的是 `supersedes: 无`（单向指针）；`decisions/README.md` 只列 7 条，ADR-0001/0002 不在索引里（ADR-0002 全仓零引用）；三处正文过期——ADR-0007 把 `scriptmgr/` 写成旧目录名、ADR-0006/0008 的 adapter 名与实际 trait `TranscriptionTransport` 不符、ADR-0001 第 6 条还写着 plans 平铺。
  - **状态词表五个，关键是 `amended`**：没有它，"方向还对、手段已换"的 ADR 只有两个坏选择——整篇标 `superseded`（把还生效的条款一起作废）或不动（让已死的条款继续被引用）。`amends` 必须带小节号，否则部分取代与整篇取代在读者眼里没有区别。
  - **关系双向**：A 写 `supersedes: B`，B 必须写 `superseded-by: A`。这条直接修掉 0004/0009 的不对称，报错文案点明代价（`one-way pointers are how a replaced decision keeps getting cited`）。
  - **两处已知失效条件写进正文**：ADR-0005 的 download owner（native 接管下载时）、ADR-0003 §5.1 的 `src/modules` module registry（Manager 迁入原生壳时）。它们今天都还成立，所以不标失效——只在失效发生时有话可说。
  - **季度复查**：90 天一轮，台账 `scripts/adr-review-log.json` 记 `date / kind / next / summary`；`next` 过期门禁即红；实际复查比上一轮 `next` 晚 30 天以上时那一次必须是 `deep`（`sweep` 只核对状态与引用，`deep` 还要找"代码里出现了新边界却没有 ADR"的空白）。首轮即 `deep`，下次 2027-02-25。
  - **门禁 `scripts/check-adr-lifecycle.mjs` 判九件事**，自检 `check-adr-lifecycle.selfcheck.mjs` **24/24**（含 4 条"不该报"：裸文件名、MIME、CIDR、`supersedes: 无`）。它上线第一次运行就抓到 ADR-0001 里 `decisions/README.md` 少写 `docs/` 的路径。

- `2026-11-27` **`docs/plans` 有了生命周期：43 份方案按状态分进五个目录，关掉"写完不动"这条路**。之前 41 份方案平铺在一个目录里，已交付的、废弃的和正在实施的看起来一模一样，唯一的状态信号是每份文件里的 `状态:` 一行——而那一行从来没人有义务去改。
  - **状态改由位置表达**：`proposals/`（7 想清楚没排期）、`active/`（8 当前依据）、`completed/`（17 已交付）、`rejected/`（1 评估后废弃）、`archive/`（10 历史 / superseded / deferred）。归档动作就是 `git mv`，不是加一句"已完成"；`active/` 因此会随交付变短，这是它和"只加不改"的区别。旧 `docs/archive/plans/` 的两份 AI CLI facade 一并并入 `plans/archive/`，两个平行归档地不再存在。
  - **搬的是字节，不是重写**：43 次 `git mv` 保历史，126 处相对链接按"文件新位置"重算（移动文件指向未移动文件的链接也要改，例如 `../decisions/` 变 `../../decisions/`——第一轮漏了这条，`check-doc-budget` 当场报出 20 条断链），另有 90 处链接**标签**里写的是旧路径，一并改成实际目标。
  - **门禁 `scripts/check-plan-lifecycle.mjs` 判六件事**：根目录只许 `README.md`（散文件等于没有状态）；头部有且只有一行状态行（`> 状态：` + 反引号包住的 token）；token 与目录必须一致（写错了是"move it, do not relabel it"）；活跃目录要 `YYYY-MM-DD-` 前缀，`archive/` 保留历史名；每个方案都要在索引里；`superseded` 必须指向存在的取代者；**全文任何 `docs/plans/...md` 提及都要能解析**——脚本注释、`doc-budget.json` 的钉子同样算，这是"移动不破坏引用"的兜底。自检 `check-plan-lifecycle.selfcheck.mjs` **15/15**（含 4 条"不该报"的用例，例如 `archive/` 的旧文件名）。
  - **两道反向验证**：往 `docs/plans/` 根塞一个文件 → 报"1 file(s) in the root with no state"；把 `active` 的文件复制到 `completed/` → 报 `status is "active" but the file sits in completed/`，同时按未索引与命名规则各报一条。两处都能红，删掉即绿。

- `2026-11-27` **路线文档开始受预算约束：`roadmap.md` 从 90.2 KB 拆到 22.1 KB，并加了一道会失败的门禁**。ADR-0001 早就写了"路线图不保留长历史"，但每轮都往同一段里追加，几个月就长到 90 KB——规则没有门禁时只是注释。
  - **按 ADR-0001 分层**：`Recent Progress` 按月拆到 [progress-2026-11.md](./progress-2026-11.md)（26.8 KB）、[progress-2026-10.md](./progress-2026-10.md)（8.6 KB）、[progress-2026-04.md](./progress-2026-04.md)（4.3 KB）；已发布的 v0.2.0 整段移进 [v0.2.0.md](./v0.2.0.md)（30.4 KB）冻结。`roadmap.md` 只剩当前状态、活跃轨道与索引。
  - **拆的是字节，不是重写**：66 条带日期条目一条不少；22 处 `](./x)` 改成 `](../x)`；拿 `git show HEAD:docs/roadmap.md` 与四个新文件逐行对照，差异只有这 22 行和 v0.2.0 里 4 个 `###` 标题降级成 `##`。
  - **门禁 `scripts/check-doc-budget.mjs` 判三件事**：①单文件字节预算——软 40 KB、硬 128 KB，超软上限的文件在 `scripts/doc-budget.json` 里钉住、**只许缩不许涨**，`--pin` 拒绝抬高（否则它就变成洗增长的工具）；②相对链接能否解析（搬文件的头号事故，现在 255 条全过）；③`roadmap.md` 里一旦又冒出具名日期的条目就报错，并检查 `docs/roadmap/` 下每个分层文件都被索引链接到。
  - **首轮钉住三个超软上限的老文件**：`phase1-quicknote-shell` 107.6 KB、`native-runtime-redesign` 80.1 KB、`mcp-implementation-plan` 48.8 KB。它们还能改，但不能再长；要继续写就先拆。

- `2026-11-25` **G0 通过（有条件）：进入 Phase 1，栈版本锁定 egui/eframe 0.31**。裁决与得分板见 [../plans/completed/2026-11-25-g0-review.md](../plans/completed/2026-11-25-g0-review.md)。
  - **硬门槛不是险胜**：热唤起 p95 48.9 ms 对 60 ms，余量 18%；S2 键入 p95 4.95 ms 对 16 ms。退路（C# WinUI 3）需要的触发证据不存在。
  - **两条判据未满足，都转成有期限的退出条件**：S2 三种输入法各 30 分钟（无人验收 → C2，Phase 1 第一周硬性）；常驻 130.1 MB vs 65 MB 解释线（→ C1 字体子集化 ≤ 100 MB，加 C5 收口）。
  - **内存这条线补了现网参照**：实测在跑的 Tauri 版全树 9 进程 **Commit 429.6 MB / 私有工作集 203.9 MB**，其中 **WebView2 GPU 进程一项私有工作集 176.8 MB**，已超过原生壳全进程的 Commit 130.1 MB。三列口径分开记（任务管理器的「内存」列是私有工作集，不是 Commit），采样脚本 `scripts/measure-process-tree.ps1` 可复现，原生侧用 `-ProcessName app`。
  - **65 MB 不改**：改成 130 等于把"还没做任何优化"的数字定成标准。但如果 C1 压到 100 MB 就压不动，说明 65 在设计时未经实测校准——届时应由 G1 判断是否改线并写 ADR，而不是在 Phase 1 里悄悄挪。
  - **WAL 仍保持 `delete`**：ADR 暂不写（只能写"暂不切换"，是空文档），改为把触发条件登记进本 Track 的「待办」。

- `2026-11-25` **S4 通过：架构门禁从"能通过"变成"能失败"，并补上三个真实缺口**。设计与验收标准先落在 [../plans/completed/2026-11-25-native-architecture-gate-spike.md](../plans/completed/2026-11-25-native-architecture-gate-spike.md)，再按 A/B/C 三批实现。**G0 的四项 spike（S1–S4）至此全部完成（G0 已过，见上一条）。**
  - **绿不能证明规则有效**：从 S1 起 CI 每次都是绿的，但那时门禁的每一条规则都可以是空实现。S4 的实质是让它失败一次——23 项自检在 `os.tmpdir()` 的临时工作区里逐条打破规则（越界边含 `[dependencies.foo]` 表形式、两节点环、层符号泄漏、801 行、未登记 crate、两条 skip 分支），断言精确到条目文本而不是只看 `failed`。
  - **自检本身也做了变异**：8 次故意改坏规则，8 次都被杀。其中 2 次第一轮**存活**，暴露的是自检的缺陷而不是规则的缺陷——尺寸用例拿被测的 `HARD_CAP` 自己算自己（改到 900 时 fixture 也跟着变 901 行，测试照样绿）；表形式依赖走的是与 kv 形式**不同的正则**，只测 kv 就等于没测。两处都补了用例，`HARD_CAP === 800` 现在被字面量钉死。
  - **三个真实缺口，都是写测试时撞出来的**：①`hub-core` 里 `use windows_sys::…` 不报错——规则写的是 `windows::`，而本项目用的是 `windows-sys`，下划线让 `\b` 匹配不到；②manifest 声明 `egui` 但源码不 `use` 完全不报——而"声明了"距离"用了"只差一行；③`[target.'cfg(windows)'.dependencies]` 整段解析不到，**放在 cfg 后面就等于隐身**。禁表因此合成**一张** `BANNED_DEPS`，符号正则与 manifest 检查都从它派生，避免两侧漂移。
  - **重构也要求证，不是声称**：把判断抽成 `analyzeNativeArchitecture(nativeRoot)` 时，成功路径与失败路径（种一个 `use egui::Context;`）都拿 HEAD 里的旧脚本对同一棵树跑 diff，输出**逐字节相同**。抽取过程中我一度把 `app` 的 `/tauri/` 收窄成 `/\btauri::/`，那是悄悄放宽，已改回。
  - **未测**：注释内 `crate::` 的误报（**有意保留**：剥注释的剥离器出错会漏报，漏报比误报危险）、规则集完整性（未穷举 `cfg_attr` / `include!`）、真实越界 PR 的拦截演练。CI runner 上的耗时后续补测：`architecture-gates` 三步各自 < 1 s。

- `2026-11-26` **`notes.db` 的 DDL 只剩一份：Tauri 调 `hub-core::store` 的同一把梯子**（[../plans/completed/2026-11-26-notes-db-shared-migrator.md](../plans/completed/2026-11-26-notes-db-shared-migrator.md)，批 1–3）。Tauri 新增 `src-tauri/src/db.rs` 作唯一连接入口（路径 + `busy_timeout` + `foreign_keys` + `Rebuild::Run`，梯子没到顶就拒连并点名 rung），`notes.rs` / `inbox/schema.rs` / `settings.rs` 的 DDL 与三处 `db_path` 删除，`file_search_recent` 改调 `hub_core::store::ensure_file_search_recent`（仍懒建、不进梯子，开门零写入）。守卫方向随之翻转：`the_tauri_build_owns_no_ddl` 递归扫 Tauri `src/**/*.rs`、按 `#[cfg(test)]` 项跳过测试夹具，生产代码出现 `CREATE`/`ALTER`/`PRAGMA user_version` 即失败（反向验证：塞一个探针文件，按 `zz_probe.rs:1: CREATE TABLE` 报红）。S3 跨实现测试加两条断言：迁移后 `schema.is_current()` 且 `legacy_capture_kinds == false`。**`user_version` 仍不写**（§5），门禁里"原生写后仍是 0"的断言继续保持。

- `2026-11-25` **S3 通过：原生进程直读直写 `notes.db`，39 项门禁检查全绿（0 项口径放宽）**。设计与验收标准先落在 [../plans/completed/2026-11-25-notes-db-compat-spike.md](../plans/completed/2026-11-25-notes-db-compat-spike.md)，再按 A/B/C 三批实现。（当时 S4 未做。）
  - **双向兼容用真代码验，不自证**：native 写入后，读它的是 **Tauri 那份代码**（`get_note_helper` / `list_notes_with_conn` / `save_note_with_conn`，新抽出的连接级函数），不是复制一遍 SQL；另加**独立实现**——`sqlite3` CLI（3.50.4，另一套 build，原生侧 bundled 是 3.46.0）与我们的读者对 **42 行逐字段一致**（`rows-identical 42`）。反向也验了：Tauri 写 → native 读回同一内容。
  - **"无 schema 破坏"是可 diff 的**：`sqlite_master` 全量 dump 写前写后**逐字节相同**；`journal_mode=delete` / `user_version=0` / `page_size=4096` / `encoding=UTF-8` 四项不动；`integrity_check` 在 native 写后、Tauri 写后、独立 CLI 三次全 `ok`。**刻意不切 WAL**：那会改文件头 write version 并留下 `-wal`/`-shm`，属产品级决定（备份/同步/"复制 db"的语义都变），不该由 spike 替用户定。
  - **真实数据不是造的**：live 库 `143,360 B`，**40 条笔记 / 9 个设置 / 待办箱**全部读出，字段与 CLI 逐项相等；排序（置顶优先 + `updated_at` 降序）与产品查询口径一致。
  - **并发是实测的**：另一进程持 `IMMEDIATE` 写锁 400 ms 时本进程写入被 `busy_timeout` 吸收并成功；持锁 7 s（超 5 s 超时）时本进程**明确失败**（`database is locked`）、无半行、`integrity_check` 仍 `ok`、持锁方的行还在。
  - **G-1**：live 库字节数 / sha256 / mtime 三项写前写后完全相同——全程只碰工作副本（`%TEMP%\efh-s3-gate\s3-gate-<stamp>\root\notes.db`，`-WorkRoot` 可改；早先落在 `native/target/` 时在只读 checkout 上会假红成 `readonly database`）；`--store-bench --store-write` 还会**拒绝**指向真实 app data 目录（代码约束，不是操作纪律）。
  - **4 个真实发现**：①`autostart` 是 `{"enabled":true}` **对象不是布尔**（设计时按 bool 写，实测纠正）；②`notes` 表物理列序因 `ALTER TABLE` 追加而不同于建表文本，`INSERT ... VALUES` 会把 `tags` 写进 `created_at` 槽——静默损坏，全部语句显式列名；③PS 5.1 `Start-Process` 在环境变量大小写重复（`HTTPS_PROXY`/`https_proxy`）时直接抛异常，并发测试改用 .NET Process API；④PS 5.1 把原生 stderr 变成 `ErrorRecord`，`-is [string]` 过滤会吞掉 `database is locked`，门禁因此差点把"被拒绝"判成"失败"。
  - **未测（4 项，明确列出）**：22 389 个既有附件的批量可读性（只验契约与新写入文件）、待办 parser 完整口径（只看 `- [ ]`/`- [x]` 行数）、随机并发压力、Win10 双系统。
  - **工具化**：`--store-bench [--store-write] [--store-hold <ms>]` + `scripts/measure-native-store.ps1`（L1 native / L2 Tauri 真代码 / L3 独立 CLI / 并发 / G-1，一条命令 `RESULT: PASS`）；`cargo test --workspace` 43 → **58 项**；样本归档 `docs/releases/native-s3/`（两次独立运行逐项相同）。

- `2026-11-25` **S2 通过：文本岛键入 p95 4.95 ms（阈值 16，余量 11）**。设计与验收标准先落在 [../plans/completed/2026-11-25-richedit-island-spike.md](../plans/completed/2026-11-25-richedit-island-spike.md)，再按 A/B/C 三批实现并逐条验收。（当时 S3 / S4 未做。）
  - **工作区**：新增 `hub-editor` crate（第五个），`check-native-architecture.mjs` → `passed (5 crates)`；`cargo test --workspace` 27 → **43 项**。CI 的 `native-crates` job 自动覆盖新 crate。
  - **承载方式**：`RICHEDIT50W`（`Msftedit.dll`）作为 egui 面板的**子 HWND**，矩形由布局给出、按物理像素对齐；父窗口补 `WS_CLIPCHILDREN`，这是"GL 不盖子控件"的唯一保证。`hub-editor` 只吃 `PixelRect`、只吐文本，门禁禁止出现 `egui::`。
  - **实测**（n=200 @1k / n=100 @10k，150% 单屏，三次运行）：`type` p50 **2.9** / p95 **4.95** / p99 **6.33** / max 6.8 ms；10k 文档 p95 **5.09 ms**，比值 **0.95–1.07**（**曲线是平的，不是线性上涨**——插入成本与文档长度无关，这是 S2 最想确认的一件事）。10k 加载 8–11 ms、粘贴 13–14 ms、连发 200 字符 203–225 ms。`compose` 仅报告 p50 4.9–7.5 ms。
  - **"深色背景真的生效"变成了像素计数**：`WM_PRINTCLIENT` 到内存 DC 采样 19392 点——主色 `0x2D2521` 占 98.49%，**空文档文字色像素 0、写入 20 字后 135**。这条路子不需要截屏、不需要可见窗口，所以能在无人值守机器上跑，也不会被"桌面上盖了什么"骗过去。
  - **4 个真实发现（都是量出来的，不是猜的）**：①`EM_SCROLLCARET` **在控件没有焦点时是空操作**——首轮两条滚动探针都返回 0，不是控件坏了；补 `SetFocus` 同时修掉产品里"输入超过一屏后光标不跟随"；②RichEdit 在 `WM_CHAR` **内部**同步绘制，紧随其后的 `UpdateWindow` 实测 0 µs，所以"插入 vs 绘制"在自然路径上无法分离，门禁改为 `insert` + 强制全量重绘（上界）；③连续键入被 RichEdit 合并成**一个**撤销单元，`EM_STOPGROUPTYPING` 无效，只有移动光标才切开；④空闲桌面下 `DwmFlush` 返回 **254 ms**（合成器不合成），所以采样器一旦探到 > 40 ms 就整段标 `n/a`，不把空闲时间记成延迟。
  - **口径作废而不是"看起来更好"**：首轮测出的 p95 **1.58 ms** 连同 `paint 0.00` / `scroll 0/0` **全部作废**——那是不带焦点、合成器休眠的状态，产品里不会发生。带焦点后 `insert` p50 从 0.74 → 1.9 ms（约 2.6×），省掉的正是光标维护与滚动视图这些真实工作。作废原因逐条写进 S2 文档 §9.4，归档里不留一个更快的旧数字。
  - **B5 判据修订（显式记录，未悄悄放宽）**："50 次编辑 = 50 次撤销"不成立也不该成立；判据改为"撤销到栈空后逐字节回到原文 + 重做回到终稿 + **撤销栈确有深度**（光标移动后 2 个单元，替换型编辑 2–3 个）"。门禁查的是"能把内容还原"，不是"每一步一个单元"。
  - **未测（2 项，都在 §9.2 标明）**：①三种输入法各 30 分钟人机验收（需要人，只自动验了 `ImmGetContext` 非空、`WM_IME_*` 不被截获）；②真实 Ctrl+Z 组合键（桩不出 `GetKeyState(VK_CONTROL)`，只验到非截获键仍到达控件）。
  - **工具化**：`--editor-bench N` + `scripts/measure-native-editor.ps1`（预算 + **17 项结构证据**一起判：颜色是否上屏、光标是否滚动、矩形是否只在变化时移动、Esc/Tab 是否被截获、撤销是否有深度……一条命令 `RESULT: PASS` / 退出码 0），样本归档到 `docs/releases/native-s2/`。

- `2026-11-25` **S1 通过：原生壳热唤起 p95 48.9 ms（阈值 60，余量 11.1）**。设计与验收标准先落在 [../plans/completed/2026-11-25-native-shell-spike.md](../plans/completed/2026-11-25-native-shell-spike.md)，再按 A/B/C 三批实现并逐条验收。**G0 仍未过**：S2 / S3 / S4 未做。
  - **工作区**：`native/` 四个 crate（`hub-core` 纯几何 + 设置、`hub-win` Win32 薄封装、`hub-ui` egui 面板、`app` 装配）。`cargo test` 27 项（core 12 / win 5 / app 10）；`check-native-architecture.mjs` 从 `skip` 变成真实 `passed (4 crates)`，这条门禁到今天才算真的存在。egui / eframe **钉在 0.31**——spike 要可复现，升级即作废全部归档样本。
  - **两段式显示（本轮唯一的结构性发现）**：跨线程 `SetWindowPos` 是**同步**的，要等目标线程处理消息，所以"把搬窗放到热键线程"并不能省下唤醒成本，实测仍是 30–47 ms。改为 **phase 1 = 测量 + 夹紧 + 搬窗 + 聚焦（热键线程，计入门禁）**、**phase 2 = 刷新内容（UI 线程，p50 140 ms，只记录不门禁）**。门禁量的是前者（屏幕上是预热帧），后者作为"内容新鲜度"另行记录——这是本轮唯一放宽的口径，写在 §4.4 里。
  - **预热到稳态**：启动即建窗，预绘 3 帧（着色器编译 / 图集上传 / 稳态帧），唤起只剩搬窗。窗口以 `SWP_NOSIZE` 停在 `(-30000, -30000)` 且 `with_taskbar(false)`，**不能用 `SW_HIDE`**——隐藏会停发 `WM_PAINT`，winit 没有 redraw 可驱动，bench 第 2 轮起永久停住。
  - **实测**（n=200，3840×2160@150% 单屏）：shown p50 **47.8** / p95 **48.9** / p99 **49.3** / max 49.4 ms（Tauri 对照 87 / 101 / 130，n=31）。另测 `--scale` 1.0 与 2.0（各 n=20）p95 49.3 / 49.2，说明成本与缩放无关、基本是唤醒的常数项。锚定实测 `at=+2631+786 size=660x1290`：右侧余 549、底边正好贴住 12 的边距；取焦 0.1–0.8 ms。
  - **泄漏**：Private **130.0 → 122.2 MB（−7.8）**，20 / 50 / 200 轮句柄数同水平（+24…+28，一次性）。**基线必须取在预热 3 帧之后**：取在构造函数里会把一次性的着色器 / 图集 / 纹理算成泄漏（第一版看到 +22.5 MB 就是这个假象）。
  - **6 个真实缺陷**（都只在高 DPI 或第二轮才现形）：①进程没有 DPI 感知，坐标被整套虚拟化（3840×2160 报成 2560×1440 / scale 1.00，面板落在目标位置的 2/3）→ `SetProcessDpiAwarenessContext` 必须是 `main` 第一句；②`SW_HIDE` 饿死渲染循环；③停靠时 `SetWindowPos` 传 0 尺寸把窗口改成 0×0（eframe 截图 panic）；④`set_pixels_per_point(1.5)` 在 1.5 原生上**叠加成 2.25** → 改用 zoom factor `target/native`；⑤预热放在构造函数里会 panic `No fonts available`；⑥架构门禁扫到我自己注释里的 `windows::`——改注释措辞，不放宽门禁。
  - **常驻 130.1 MB 未达 65 MB 解释线（⚠️ 不掩饰）**：`--no-font` 对照测出 CJK 字体 + 图集占 **37.7 MB**（92.5 MB 基线），其余 ~92 MB 是 GL 驱动 + winit + egui 固定开销；仍比 Tauri 静置 262 MB 低 50%。字体子集化是首要可压缩项，留给 Phase 1。
  - **smoke 一直是红的（顺带发现并修掉）**：`Responsiveness budget` 拿 60 ms 判**现 Tauri 版**（p95 101 ms），从打点落地那天起每轮必然 FAIL——一个注定失败的步骤会把另外九个步骤的信号一起淹掉。改为 `Tauri latency baseline (report)` + `-NoFail`：数字照旧打印 `Verdict: FAIL`，只是不再染红整轮；判决继续在原生壳上，退役前（P4）当回归护栏用（越过 ~130 ms 才算真回归）。同轮补 CI 的 `native-crates` job（`cargo test --workspace` 27 项 + release 构建），此前 CI 完全不碰 `native/`，S1 只在作者机器上编译过。
  - **文档回填**：主设计 §4.3 状态、§7.1 新增"原生 S1 实测"表（含门禁口径），`native/README.md` 列出 Phase 1 必须偿还的捷径（`FindWindowW` 拿 HWND、`--scale` 模拟缩放、无 blit 缓存、面板内容静态、无托盘）。
  - **未测**：真实多显示器 + 混合 DPI 跨屏（本机单屏，`--scale` 只证明数学）、`lite` 在真实 RDP 回环（本机 `remote=0`）、首帧直接 blit 上一帧纹理（未做，当前靠预热帧留在 swap chain 达成等效）。

- `2026-11-24` **P0.5 完成：窗口生命周期 + 计时契约，静置 600 → 282.8 MB**。设计与验收标准先落在 [../plans/completed/2026-11-24-window-lifecycle-and-latency-contract.md](../plans/completed/2026-11-24-window-lifecycle-and-latency-contract.md)，再按 A/B/C 三批实施并逐条验收。
  - **Batch A（度量）**：`measure-footprint.ps1` 增加 `-Label` / `-OutFile`，三姿态（idle / launcher / manager-2notes）快照归档到 `docs/releases/footprint/0.2.0/`。注意 PS 5.1 里 `$json` 与开关 `-Json` 是同一个变量，赋值会直接报错。
  - **Batch B（生命周期）**：Manager 从 `tauri.conf.json` 移除，改由 `ensure_manager_window` 按需创建、关闭即销毁；`close_all_note_windows` 改用 `destroy()`。实测 **idle Private 600.0 → 261.6–282.8 MB（−53% ~ −56%）**、WebView2 进程 7 → 6、sidecar 静置 0（懒启动）；关闭 Manager 12 ms 内释放，重建 222–248 ms；10 轮开关与 2 笔记开关均无累积。**顺带修掉两个真实缺陷**：`single-instance` 插件必须最先注册（否则第二个进程先建窗再退出，留下 `about:blank` 与热键冲突），且不能在事件循环线程里就地 `build()` WebView（会死锁，改为 `async_runtime::spawn`）。
  - **Batch C（计时契约）**：热键回调打 `[timing] hotkey_down`（在选区捕获之前，180 ms 上限要计入），Launcher 连续两帧后回调 `report_first_frame`；`log_timing()` 无视 debug 开关落盘。实测 **p50 87 ms / p95 101 ms / p99 130 ms（n=31）**，作为原生壳的对照组写入设计 §7.1；目标仍是 p95 ≤ 60 ms。
  - 未触发"动因被证伪"分支（降幅 53% ≫ 25% 判定线）。Phase 0 剩余工作是 S1–S4 四个 spike。

- `2026-11-24` **光标跟随主题 + 修掉"清空正文不落库"**。光标色原默认硬编码 `#4f5a43`（深橄榄，暗色面板上几乎看不见）→ 改为**空串 = 跟随主题**：内联 `caretColor` 只在用户显式选色时输出，设置面板加「跟随主题色（推荐）」开关，旧档案的值在读写两侧都归一化为空。**清空不落库是双根因真 bug**：前端 `buildPersistParams` 与后端 `save_note_with_conn` 都在「空内容且无标题」时提前退出，两处都没区分**新建**与**已有**——修法一致地收紧成 `!exists && …`，新建仍跳过（不产生空壳），已有清空必须落库（前端由 `notePersistedRef` 决定传 `allowEmpty`）。实测 computed `caret-color = rgb(134, 171, 220)`（即 `--manager-accent`）、清空后该行 content 变 `""`；Rust 96 项 / 前端 418 项全绿。

- `2026-11-24` **Phase 0.5 首批止血：自启动四步修复 + 两个快速笔记缺陷**。自启动状态只存在注册表里，被外部清除后无法自愈且 UI 静默 OFF → 新增 `autostart.rs`：意图存 `settings` 表（**不进 `AppSettings`**，避免全量保存用旧快照覆盖）、`reconcile()` 启动补回、`apply()` 回读校验并回滚半切换、`get/set_autostart_state` 取代前端直调插件；对账边界：意图为关而系统存在时**不自动关闭**（可能被安装器重新添加）。同轮修 `EditorTextarea` 的 `quick-note` 变体把正文硬编码成 `#1a1a1a`（暗色下不可见，改 `var(--quick-text)`）与 `QuickNoteToolbar` 的 `•••` 绝对居中与预览/置顶按钮重叠（改三段 flex，300px 宽下无交叠）。验证：前端 413 项、Rust 91 项、tsc/build/architecture 全通过。

- `2026-11-24` **拿到首批内存基线 + 定位自启动失效**：启动应用实测托盘静置（manager / launcher 均 `visible:false`、无人操作）**Private 545.3 MB / Working Set 846.1 MB / 10 进程**（WebView2 7 个，最大单进程 352.9 MB；启动后 30 s 与 130 s 两次采样 564→546 MB，为稳态而非瞬时峰值），是原生 65 MB 解释线的 8.4 倍——"占用与是否在用无关、只与隐藏窗口数相关"从判断变成证据。附带发现开机自启动失效：`HKCU\...\Run` 无 EasyFlowHub 项，但 `StartupApproved\Run` 留有记录（说明曾创建后被移除）；根因是**自启动状态只存在于注册表项、未进 `settings` 表**，被外部清除后应用无法自愈且 UI 静默回 OFF。已写入设计 §2.4 与 Phase 0.5，修复含四步：偏好持久化 / 启动对账 / 写入后校验 / 单实例回调尊重 `--autostart`（当前会弹 Manager）。同时确定 Launcher 动作网格**固定 4 列、窄屏降 3 列**（列宽 <96 逻辑像素即降档，两套布局启动期算好并缓存，滚动位置按项索引保存）。`measure-footprint.ps1` 输出改为单次 success 流写入，可被 `Out-File` 重定向且空行不再被吞。

- `2026-11-24` **内存降为次级目标 + 度量只在本机**：用户确认"8 GB 机器内存足够、额外设备测试没意义、测延迟才有意义"，且"保障常用功能速度前提下自然越小越好"。§7.2 重写为"次级目标 + 解释线"（65/55 MB 不再阻塞发布），补常驻内存构成拆解（刚性/半刚性/软性 + 结构性下限 30–50 MB）、6 条必做免费优化、4 条禁止的反向优化、RC 趋势归档规则；Phase 0 移除第二台机器与 RDP 阻塞要求；G1 与 S1 判据明确"响应阻塞、内存解释"。

- `2026-11-24` **本机实测画像定下 Phase 0 的度量口径**：Ryzen 7 4800H / 31.4 GB / Radeon 集显 / **3840×2160 @150%** / Win11 26300。按 1366×768 估的数全部作废——150% 下字形面积 ×2.25，原计划"3500 常用字图集"≈30 MB+、全屏首帧纹理 ≈33 MB（3840×2160×4）。两条结论写入 ADR 作硬约束：图集只预栅格化首屏字符（300–800 字，3–6 MB），**launcher 必须是面板而非全屏层**。`measure-footprint.ps1` 起逐显示器输出分辨率 + 缩放与主 GPU，跨 DPI 不再误比内存。同期测得系统 WebView2 Runtime 一个月从 878.7 MB（v157）涨到 2029.3 MB（v158）。

- `2026-11-24` **launcher 语义澄清 + 目标环境修正**：用户明确它模仿 Quicker——**跟随光标的竖版动作面板，不是工作台**，工作台另有页面。尺寸按实测 `tauri.conf.json` 定为 440×860 逻辑像素（此前误写 960×640 横版），首帧缓冲 150% 下 3.4 MB 而非 33 MB；新增 §5.1.1「光标锚定与高 DPI」（逐档缓冲/图集表、5 条零延迟优化、3 个延迟坑：热键路径不枚举显示器、`pixels_per_point` 必须在 `show` 前设好、DPI 切换时新旧图集交接避免空白帧）。同时撤销 4 GB 画像（用户可接触的最低配置为 8 GB），最低改 8 GB / 4 核 / 集显；`lite` 重新定位为软件渲染 / RDP 降级档、上限 45→55 MB，Phase 0 度量改为本机集显 + `mstsc` 回环；launcher 仍预创建（懒创建会破主门禁）。

- `2026-11-24` **预热边界定稿**：60 ms 是终点，按"每天十次以上"分四级（P0 启动预热 / P1 会话内复用 / P2 冷启动 / 不预热）；"不做无限加码"写入非目标——继续下探会拖慢冷启动并增加预热过期类缺陷。ADR-0009 第 8 条同步补边界。

- `2026-11-24` 确认 Native Runtime 方向（ADR-0009 `accepted`）：分阶段、成熟方案优先、低占用并适配 4 GB/集显/远程桌面、UI 简洁优先 + 主题系统。设计补入 `lite` 降级、按姿态预算与架构门禁；新增 `scripts/check-native-architecture.mjs`（crate 分层 / 越界符号 / 单文件 800 行，`native/` 缺失时 skip）并接入 `run-smoke.ps1` 与 CI `architecture-gates`；`ARCHITECTURE_GUARDRAILS.md` 新增第 10 节 native 层图。**未改产品代码**，等待 Phase 0 四姿态/四 spike 与 G0。
