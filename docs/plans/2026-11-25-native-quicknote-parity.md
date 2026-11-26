# 原生快速笔记 Parity 清单（T0）

- status: `approved` —— 2026-11-25 起草，**同日裁决完成**（§5 五项已全部定案：2 留 / 1 延后 / 1 不做 / 1 延后）；§3 的 A 级条目即为 Phase 1 的验收对象
- 实施进度：[2026-11-25-phase1-quicknote-shell.md](./2026-11-25-phase1-quicknote-shell.md) 记录每一批覆盖了哪几条、以及为什么这样拆；§3 的「现状」栏随该文档回填
- 上游：[native-runtime-redesign.md](./2026-11-24-native-runtime-redesign.md) §3.3（v2 收窄）、§3.4（允许差异的边界）、§5.1（窗口模型）、Phase 1（G1 门禁）
- 参照实现（**这是"parity"的唯一定义来源**，不是 UI_REQUIREMENTS.md）：
  - `easyflowhub-app/src/pages/QuickNotePage.tsx`（442 行，状态与渲染）
  - `easyflowhub-app/src/components/shared/QuickNoteToolbar.tsx`（32 px 标题条，三段式）
  - `easyflowhub-app/src/hooks/useQuickNoteShortcuts.ts`（窗口键位表）
  - `easyflowhub-app/src-tauri/src/app_windows.rs:60`（窗口创建参数）
  - `easyflowhub-app/src-tauri/src/settings.rs:13`（`QuickNoteSettings` 默认 400×300）

## 1. 这份清单解决什么问题

G1 门禁写着"T0 功能 parity"，但仓库里一直没有 parity 的对象——§8 风险表把 "parity checklist" 当缓解措施写进去，却没落成文件。后果在 2026-11-25 14:06 暴露了一次：spike 面板被当成"新的快速笔记"评判，于是"标题条去哪了""为什么拖不动""钉不住了"这些问题无法判断是**缺陷**还是**还没做**。

所以这份清单的定义只有一条：**在 Tauri 版上现在就能做、v2 必须还能做**的事。每条都要能用"看 / 按 / grep 日志"判定，不写"体验一致"这类无法反驳的话。

为什么以现版代码为准而不是 `UI_REQUIREMENTS.md`：需求文档与实现已经分叉（见 §5 C4），照文档实现会做出一个现版没有的东西。parity 的意思是与**用户现在手里那个**对齐，不是与三月的文档对齐。

## 2. 分级

| 级 | 含义 | 门禁效力 |
|---|---|---|
| **A** | 必须有。缺了就是缺陷 | 阻塞 T-Gate 0；§3 全部 ✓ 才允许停掉 Tauri 的快速笔记 webview |

> 「现状」一栏用四个符号：`✗` 未实现 · `◐` 代码与单测就绪、等人机目视 · `✓` 已按判定方式验过、附证据 · `—` 已挂起/延后。`◐` 不算通过，只是把"没做"与"做了但没看"分开。
| **B** | 允许差异，但有边界 | 不阻塞，但越界（如出现白边、错帧）按缺陷记 |
| **C** | 现版有、v2 未必留 | 已全部裁决（§5）。裁"留"的已升为 A 级条目（Q25–Q29），裁"延后/不做"的从 A 级摘除并注明原因 |

§3 的编号按起草顺序追加，不按章节顺序重排：Q25–Q29 是 2026-11-25 裁决"留"之后补入的，插入到对应小节，编号保持连续，避免已引用的编号漂移。

## 3. A 级 —— 必须有

判定方式一栏是给人照着做的动作，不是描述。做完把"现状"从 ✗ 改 ✓ 并附证据（截图路径或日志行）。

### 3.1 窗口与几何

| ID | 行为 | 判定方式 | 现状 |
|---|---|---|---|
| Q1 | 无系统边框、无系统标题栏 | 看窗口四边：没有系统标题栏，没有系统边框 | ◐ `with_decorations(false)` + `dwm applied=["dark","round","mica"]`；待目视 |
| Q2 | 默认 400×300 逻辑像素，尺寸取自设置 `quick_note.width/height` | 首次启动为 400×300；把设置改成 500×400 后新建，窗口随之变化 | ✓ 实测默认 `600x450` 物理、改 `500x400` 后 `750x600` 物理（@150%） |
| Q3 | 最小尺寸 150×60，不允许更小 | 拖到最小仍可点标题条三个按钮、仍可输入 | ✓ 2k：`scripts/check-native-edge.py`（23 项，连跑 3 次）用真实拖动判——左右/底边/右下角各改自己那条边（右边缘拖动时原点不动、左边缘拖动时右边缘是常量）；拖过下限停在 225×90 物理（150×60 × 1.5），原点不被拖走；岛跟随（`island_rect` 宽 == 客户区宽、高 == 客户区高 − 条高）。**先补的前提**：无边框窗口 `WM_NCHITTEST` 全回 `HTCLIENT`，系统没有 resize 边框，所以边缘是自己画的（`note/edge.rs`，见 [§8.16](./2026-11-25-phase1-quicknote-shell.md#816-边缘-resize一条边要自己画因为系统没有那条边2k2026-11-26)） |
| Q4 | 出现在光标所在显示器，四边夹紧在工作区内 | 两台显示器各触发一次，窗口不跨屏、不越界 | ◐ 算法与 7 个单测就绪；本机只触发过单屏（`+1524+1361` 为上翻结果），双屏待验 |
| Q5 | 关闭即销毁，新建是全新窗口 | 关掉后进程树窗口数回落；新建后滚动位置在顶部 | ✓ 1b.2：每笔记一个 viewport，关闭后外部 `EnumWindows` 只剩其余笔记（`note_close id=1 open_notes=2` → `note_destroyed id=1 open_notes=1`）；新建 `note_begin id=1` → `island_created id=1 caret=(0,0)`，是新的空文档。1b.3 起关完最后一个窗口进程也留着（`hub_idle notes=0 tray=<hwnd>`），窗口数照样回落到 0，只是进程不随之结束 |

### 3.2 标题条（32 逻辑像素，三段式）

现版结构见 `QuickNoteToolbar.tsx`：左槽透明度、中槽 `•••`、右槽预览 + 置顶。**预览按钮不列入 T0**（C3 已裁决延后到 Phase 3，见 §5），所以 v2 的右槽在 T0 只有置顶一个按钮——这是裁决后的正确形态，不是"少做了一个"。

| ID | 行为 | 判定方式 | 现状 |
|---|---|---|---|
| Q6 | 左槽显示不透明度百分比；降到 0 时显示 `GHOST` | Ctrl+滚轮降到 0，左槽文本为 `GHOST`；回到 20% 显示 `20%` | ✓ 1c：标签与窗口 alpha 同源（`StripState::opacity`）。实测 20% 时 `PrintWindow` 抓到的左槽显示 `20%` 且 `alpha=51`；滚到底日志 `label=GHOST`、`alpha=1`（窗口已不可见，标签只能靠日志与单测判定） |
| Q7 | 中槽 `•••` 开/关工具面板 | 点一次开、再点一次关；点面板外部也关 | ✓ 2e：`scripts/check-native-tools.py`（8 项）用真实鼠标点击判——点 `•••` 得 `note_tools open=true reason=button` 且岛从 y=48 让到 156；再点同一位得 `open=false reason=button`；点正文得 `open=false reason=body` 且窗口不关。附带修掉一个真 bug：拖拽区注册在按钮之后，egui 把点击给了最后注册的控件，**两个按钮对鼠标完全无响应**（键盘仍可用）；见 §8.10 |
| Q8 | ~~右槽预览按钮：编辑 ↔ 预览切换~~ | **挂起**：C3 延后到 Phase 3，T0 不实现，编号保留不复用 | — |
| Q9 | 右槽置顶按钮：**可开可关**，激活时高亮 | 初始置顶（与现版一致，图标是实心书签）；点一次窗口被其他窗口压住、图标变空心灰；再点恢复 | ✓ 2f：`scripts/check-native-pin.py`（11 项，连跑 3 次）用真实点击判——起始 `WS_EX_TOPMOST` 在、图标实心（图标中心 11 px 框里 116 个亮点、中心像素 `(134,171,220)`）；点一次得 `note_pin pinned=false topmost=true`、OS 位清掉、同框只剩 6 个亮点、中心回到条底色 `(31,46,68)`；再点全部恢复，且窗口位置不变。**修掉一个吞窗口的真 bug**：`ViewportCommand::WindowLevel` 让 winit 按缓存的 `WindowFlags` 重建整套样式，本窗口是 `with_visible(false)` 建、`move_on_screen` 显示的，于是"取消置顶"把 `WS_VISIBLE` 一并抹掉，窗口从桌面消失（Q16 只读日志，所以一直显示 ✓）；改走 `hub_win::window::set_topmost`，见 [§8.11](./2026-11-25-phase1-quicknote-shell.md#811-置顶按钮一次成功的置顶把窗口弄没了2f2026-11-26) |
| Q10 | 标题条任意位置按住可拖动窗口 | 在左/中/右三个槽位分别拖动，窗口跟随；点在按钮上不拖动 | ✓ 2g：`scripts/check-native-drag.py`（9 项，连跑 4 次）用真实拖动判——左/中/右槽各拖 96 px，`GetWindowRect` 的位移逐轴等于光标行程（容差 1 px）；从 `•••` 与置顶按钮起拖时窗口不动、也不误触开关（拖动不是点击）。**`ViewportCommand::StartDrag` 在这个环境里不动窗口**（同一套注入下普通 `WS_OVERLAPPEDWINDOW` 也不动，是 OS 移动循环没跑），改为每帧按光标位移 `SetWindowPos`（`note/drag.rs`，见 [§8.12](./2026-11-25-phase1-quicknote-shell.md#812-拖动标题条自己搬窗口比等-os-移动循环可靠2g2026-11-26)） |
| Q11 | 三段永不重叠 | 缩到 150 逻辑宽，三个槽位逐个点击都命中预期（左槽文本不可压住中槽按钮） | ✓ `slots()` 单测钉住 150 宽下三段有序、按钮不重叠；`GHOST` 实测 39 px 宽，左缘 16 + 39 = 55 < 三点左缘 61，余 6 px（§8.3） |

Q11 不是凑数：现版专门为此重排过一次（`QuickNoteToolbar.tsx` 的 `STRIP_STYLE` 注释写明工具按钮曾绝对居中、窄窗时压住眼睛图标）。

### 3.3 键盘

| ID | 行为 | 判定方式 | 现状 |
|---|---|---|---|
| Q12 | Esc **按一次**即关闭当前窗口 | 按一次窗口消失；日志恰好一行 `editor_esc` | ✓ 岛内 subclass 拦键 → 一帧内关闭；1b.2 起关闭只销毁该 viewport（`note_close reason=esc`），其余笔记不受影响 |
| Q13 | 切换器打开时 Esc 只关切换器 | 开切换器按 Esc，窗口仍在、切换器消失 | ✓ 2b：`scripts/check-native-switcher.py`——`Ctrl+K` 开面板后按 Esc 得 `note_switcher open=false` 且 `note_close=0`，再按一次 Esc 才 `note_close reason=esc`（面板只吃一次，不永久吃掉这个键） |
| Q14 | Ctrl+D 关闭当前窗口 | 同 Q12 | ✓ 实测 `note_key action=Close` + `note_close reason=shortcut open_notes=2` → `note_destroyed open_notes=1`；1b.3 起进程不再随窗口消失（托盘常驻），见 Q5 |
| Q15 | 编辑器内 Ctrl+Z / Ctrl+Shift+Z 撤销/重做 | 输入三段，撤三次回空，重做三次还原 | ◐ 走 `EM_UNDO`/`EM_REDO`，实测输入 `abc` 后 undo ×3 得 `len=0`、redo ×3 回到 `len=3`；"三段"未按三次独立编辑验（连续输入被 RichEdit 合成一个 unit，见 `undo_probe`） |
| Q16 | 置顶快捷键可切换（键位取 `shortcutConfig.toggleAlwaysOnTop`） | 按一次取消置顶，再按恢复 | ✓ 2a：键位改由 `hub-core::shortcuts` 那张表提供，表由设置文件的 `shortcuts` 覆盖；`Ctrl+T` → `note_pin pinned=false` 的旧实测不变。改键生效由 `check-native-manager.py` 的 K1/K2 判（`close_note` 从 `Ctrl+D` 改到 `Ctrl+Q`：新键生效、旧键失效）。**2f 补**：这条键盘路径与 Q9 同一个 `toggle_pin`，此前"只读日志"漏掉了它会隐藏窗口；现已改走 `set_topmost`，`check-native-pin.py` 覆盖 |
| Q17 | Ctrl+. 开/关工具面板 | 同 Q7 的键盘路径 | ◐ 表里有、代码已接；中文输入法把 `wParam` 改写成 `VK_PROCESSKEY`，脚本注入无法还原，**待真实键盘按一次**（§8.4） |
| Q18 | 全局键：Ctrl+Alt+N 新建、Ctrl+Alt+D 关闭全部、Ctrl+Alt+H 最小化/恢复全部、Ctrl+Alt+M 打开工作台 | 焦点在记事本时逐个按，行为生效 | ◐ 1b.3：四条全部注册（`hotkeys registered=4 failed=0`）。`Ctrl+Alt+N` 在焦点属于另一个程序时新建并拿到前台；`Ctrl+Alt+D` → `note_close reason=all` ×2 + `hub_idle`（进程存活）；`Ctrl+Alt+H` → 外部 `IsIconic=true` ×2，再按恢复。**`Ctrl+Alt+M` 只到「注册 + 记日志」**（`note_manager target=none phase=2`）：管理中心窗口属 Phase 2，没有可打开的目标。2c 起它打开工作台（`note_manager action=open target=manager` → `manager_open notes= trash=`），但本机 `Ctrl+Alt+M` 被 shipped 版占着，门禁改用它把 `open_manager` 绑到 `Ctrl+Alt+J` 再按——见 Q16 与 `check-native-manager.py` |

**切换器（C1 裁决"留"，2026-11-25 补入）。** 现版 `shortcutConfig.quickSwitcher` 默认 `Ctrl+K`（`types/shortcut.ts:20`），可改键；浮层结构见 `NoteSwitcherPanel.tsx`——查询输入 + 索引 0 的"新建笔记"行 + 最近笔记列表，`selectedIndex` 由窗口键盘表驱动。Q13 的"Esc 只关切换器"依赖这三条成立才有前提。

| ID | 行为 | 判定方式 | 现状 |
|---|---|---|---|
| Q25 | `Ctrl+K` 打开切换器，**同键再按关闭**（不是只开不开） | 按一次浮层出现、再按一次消失；输入框被清空 | ✓ 2b：`note_switcher open=true items=2` → 再按 → `open=false`，两次之间 `note_close=0`；`State::open()` 清空查询与选中。门禁 `check-native-switcher.py` |
| Q26 | 查询同时匹配标题、正文、标签 | 造一条正文含"螺丝刀"、标题无关的笔记，搜"螺丝刀"能出现 | ✓ 2b：标题"周末计划"、正文含"买螺丝刀和膨胀管"，搜"螺丝刀" → `note_switch to=gate-note-a`；另一条只在 `tags` 里有"重要"，搜"重要" → `to=gate-note-b`。haystack = 标题 + 正文 + 标签，小写化 |
| Q27 | `↑`/`↓` 移动选中，`Enter` 切换；选到索引 0 时 `Enter` 新建并切换 | 列表三条：↓ 两次选中第二条并切过去；↑ 回顶按 Enter 得到新空笔记 | ✓ 2b：查询后 ↓ 再 Enter → 窗口切到该笔记（`to=gate-note-a chars=14`），随后打一个字落进**被选中笔记**的行尾；索引 0 + Enter → `to=new seeded=true chars=4`，行立刻写成 `title="周报草稿"`（`reason=seed`）。↑/↓ 环绕、hover 不移动选中由 `switcher` 单测钉住 |

### 3.4 内容与保存

| ID | 行为 | 判定方式 | 现状 |
|---|---|---|---|
| Q19 | 打开即聚焦编辑区，光标在正文末尾 | 打开后直接打字，字符落在最后一行末尾 | ✓ 实测 `editor_focus focused=true caret=(0, 0)`，随后直接打字无需先点击。2i 修正"末尾"的含义：有标签行的笔记，光标落在**正文**末尾（标签行之前），否则续写的字会进标签——`note_caret at=7 of=15 tag_line=true`，门禁 Q-L 打字后正文加字、`tags` 不变（§8.14） |
| Q20 | 停止输入后自动保存（现版 900 ms 防抖，见 §7.1） | 输入后等 2 s，杀进程重开，内容在 | ✓ 1d：`scripts/check-native-notes.py` 打字后等 1.7 s，库里出现一行（`note_save reason=autosave created=true`）；另有 2 s 扫描兜底——只发 `WM_KEYDOWN VK_RETURN`（不发 `WM_CHAR`，因此不触发编辑信号）也能在 3.8 s 内落库（`created=false`） |
| Q21 | 关闭前强制 flush | 输入一个字立刻 Esc，再打开同一笔记，那个字仍在 | ✓ 1d：打"字"后 43–73 ms 按 Esc，行已写入（`note_save reason=close created=true title="字"`），远早于 900 ms 防抖 |
| Q22 | 标题取首个非空行 | 第一行输入"买菜清单"，列表标题即"买菜清单" | ✓ 1d：首行"买菜清单"，库中 `title="买菜清单"`；末行 `# 工作 重要` 被剥成 `tags`，正文留下 `买菜清单\n鸡蛋` |
| Q23 | 空笔记不产生垃圾条目 | 新建后不输入直接 Esc，列表里不出现空条目 | ✓ 1d：空窗 Esc 后行数 40 → 40（`note_save reason=close skipped=empty`）；已存在的笔记清空仍会写（`persisted` 分开两种情况） |

### 3.5 数据恢复

| ID | 行为 | 判定方式 | 现状 |
|---|---|---|---|
| Q24 | 检测到未合并备份时给三选一（保留当前 / 恢复备份 / 合并） | 手工造一条备份草稿，打开窗口出对话框，三个按钮行为各自正确 | ✓ 2h：草稿落 `{root}/autosave/{id}.json`（与现版同形的四个字段），打开时与行比较，比行新且不同才问；三个按钮各自写库/删文件。`check-native-recover.py` 15/15。卡片是模态的（`•••` 点不动）。**与现版不互通**：现版在 `localStorage`，原生读不到 |

### 3.6 透明度与幽灵模式（C2 裁决"留"，2026-11-25 补入）

现版实现：`QuickNotePage.tsx:257` 的 `wheel` 监听（`deltaY < 0` 加 0.2，否则减 0.2，钳在 `[0,1]`，**50 ms 节流**）；`toggleGhostMode`（`QuickNotePage.tsx:78`）把不透明度压到 0 并记住上一个可见值，再次触发时恢复 `max(0.2, lastVisible)`。

| ID | 行为 | 判定方式 | 现状 |
|---|---|---|---|
| Q28 | `Ctrl+滚轮` 按 **20% 一档**调整整窗不透明度，越界钳在 0% / 100% | 从 100% 向下滚到底为 0%（左槽显示 `GHOST`），向上滚到顶为 100%；单次快速连滚不会跳档 | ✓ 1c：`LWA_ALPHA` 整窗混合。两条通道（岛 `WM_MOUSEWHEEL`、标题条 egui）各有日志 `path=`；5 条连发只走一格（50 ms 节流）；像素实测 60% → `(109,114,121)` 对期望 `(108.6,114.2,121.2)`。GHOST 用 alpha 1 而非 0，因为 0 的窗口不可命中（§8.7） |
| Q29 | `Ctrl+Shift+H` 一键幽灵（不透明度 0），再按恢复**进入前的值** | 先滚到 40%，按 Ctrl+Shift+H 变 `GHOST`，再按回到 `40%`（不是 20%） | ✓ 1c：40% → `note_ghost ghost=true (was 0.40) restore=0.40` → 再按 → `ghost=false opacity=0.40`；`SendInput` 真按键，还原值不是固定的 20% |

Q28 的"一档 20%"与 Q6 的"左槽显示百分比"是同一条链的两端：Q6 判显示正确，Q28 判数值语义正确。原生实现路径不限（layered window / 分层绘制），但**百分比语义必须一致**（B5）。

`Ctrl+Shift+H` 只改不透明度，不改是否置顶、不改内容、不影响保存节奏——它属于"看得见但不挡事"，与 `Ctrl+Alt+H`（最小化/恢复全部窗口，见 Q18）是两件不同的事，别混。

## 4. B 级 —— 允许差异（有边界）

| ID | 项 | 边界 |
|---|---|---|
| B1 | 圆角与阴影 | 半径与主题 token 一致；不允许直角白边或阴影外露一圈 |
| B2 | 字体字距、字重 | 行高与段落间距节奏必须与现版同级；只允许字面差异 |
| B3 | 动效 | 允许更短或没有；不允许"先小后大"、空白帧、透明度跳变 |
| B4 | 滚动条样式 | 可用系统默认，但必须可用、不被裁切、不遮最后一行 |
| B5 | 透明度实现路径 | 现版是整窗 CSS 透明；原生可用 layered window 或分层绘制，只要百分比语义一致（20% 一档） |
| B6 | Markdown 渲染细节 | 代码高亮配色、表格边框可不同；图文顺序与链接可点必须一致 |

## 5. C 级 —— 已裁决（2026-11-25）

这五项现版有、主设计没写。裁决前不实现，是因为**未裁决就实现，等于替你做了产品决定**。2026-11-25 用户答复"全部按推荐来"，即下表"裁决"列。

| ID | 项 | 现版行为 | 裁决 | 后果 |
|---|---|---|---|---|
| C1 | 笔记切换器 | `Ctrl+K` 浮层，可搜标题/正文/标签、可新建并切换（`NoteSwitcherPanel.tsx`） | **留** | 升为 A 级 **Q25–Q27**；设计文档 §5.1 需补一句"每窗口一个 viewport，外加一个跨窗口的笔记切换器" |
| C2 | 透明度 + 幽灵模式 | `Ctrl+滚轮` 20% 一档；`Ctrl+Shift+H` 一键幽灵，再按恢复原值 | **留** | 升为 A 级 **Q28–Q29**；落地为 `quick_note.appearance.window_alpha`（新笔记的初值，默认 1.0），§5.4 写明它是**用户态设置**、不是主题装饰（模糊仍禁用） |
| C3 | 预览态 + 图片条 | 编辑/预览切换、图片缩略条、拖图入窗插入 | **延后到 Phase 3** | Q8 挂起、右槽 T0 只有置顶；图片插入依赖 Markdown 渲染，与 Phase 3 一起做 |
| C4 | 多标签页 | **现版没有**：每窗口一笔记 + 切换器 | **不做**（按现版对齐） | `UI_REQUIREMENTS.md` §1.2.2 的"多标签页 P0"不实现，已在原处加注记；标签页相关键位（Ctrl+W / Ctrl+Tab）同样不实现，切换职责归 C1 |
| C5 | 窗口尺寸记忆 | 全局一组宽高存 `settings.quick_note`，不按窗口记 | **已做**（2i，不按窗口记） | 拖动后静默 400 ms 写入 `quick_note.width/height`，同一进程的下一扇窗与下次启动的新笔记都用它；`quick_note.schema` 同批补上（首次写盘为 1）。**顺带发现**：无边框窗口的 `WM_NCHITTEST` 全返回 `HTCLIENT`，鼠标拖边缘改不了尺寸——这条属 Q3，另立一项 |

三条"留"的共同点：都是**用户可感知的日常路径**（切笔记、调透明度、闪一下），缺了手感立刻不对；三条"延后/不做"的共同点：都要新造功能或渲染管线，与 parity 无关。

### 5.1 C4 的连带影响（为什么不是"少个功能"那么简单）

`UI_REQUIREMENTS.md` §1.2.2 把多标签页定为 P0，还给了键位表：`Ctrl+W` 关标签、`Ctrl+Tab` / `Ctrl+Shift+Tab` 切标签。现版实现里这三条**都不存在**——关闭是关窗（`Ctrl+D` / Esc），切换是 `Ctrl+K` 浮层。

如果不裁决就直接照需求文档做，会有两个后果：一是做出了一个现版没有的东西（不是 parity，是扩容）；二是 `Ctrl+Tab` 与窗口级切换语义冲突，而 Windows 上 `Ctrl+Tab` 在多文档界面里几乎是肌肉记忆，实现错方向后改键会挨骂。所以这条不是"延期讨论"，是**明确不做并记录**。

## 6. spike 现状（为什么看起来"什么都没有"）

`easyflowhub-native-spike` 是 S1/S2 的验证件，`2026-11-25-richedit-island-spike.md` §2 明确写了不做真实 UI。对照 §3：

> 2026-11-25 起，产品窗口在 `--note` 模式下已经存在（`native/app/src/note.rs`），spike 只保留测量用途。下表描述的是 **spike**，不是 `--note`。

| 能力 | spike | 说明 |
|---|---|---|
| 标题条 / 拖拽 / 置顶开关 / 透明度 | 全无 | `main.rs:152` 只有 `with_decorations(false)` + `with_always_on_top()`（恒定，无法取消） |
| 拖拽 | 无 | 没实现 `HTCAPTION` 命中，也没从 egui 拖拽区 |
| Esc | 隐藏（park 到屏外）+ **释放键盘焦点** | 不是产品行为；产品行为是关闭窗口。焦点释放是 2026-11-25 补的（`esc_poke` 前后对比见 [C2 文档](./2026-11-25-c2-ime-acceptance.md) §5.3.1）。前台仍留在面板上，所以 Q12 在 spike 上只能验"不再写入"，不能验"焦点回到原窗口" |
| 内容保存 | 内存文档 | **不写 `notes.db`，关掉即失**，别拿它记东西 |
| 内容区 | RichEdit 文本岛 | 唯一"已经有"的部分 |

这张表也解释了 2026-11-25 的困惑：那个面板看起来像快速笔记，实际只有最后一行的能力是真的。

## 7. 判定手段

### 7.1 两处文档与代码不一致（以代码为准）

- `UI_REQUIREMENTS.md` §1.2.1 写"防抖保存 500 ms"，代码是 `AUTOSAVE_DELAY = 900`（`useQuickNoteSession.ts:30`、`useNotesArchive.ts:31`）。清单按 900 ms 写。
- `UI_REQUIREMENTS.md` §1.2.2 写"多标签页 P0"及 `Ctrl+W` / `Ctrl+Tab` 键位，实现是"每窗口一笔记 + `Ctrl+K` 切换器"。**已裁决按现版不做**，见 C4 与 §5.1。

### 7.2 工具

| 用途 | 手段 |
|---|---|
| 关闭即销毁（Q5） | `scripts/measure-process-tree.ps1` 前后对比窗口数与句柄 |
| Esc 一次生效（Q12） | 日志 grep `editor_esc`，一次按键恰好一行 |
| 环境与二进制指纹 | `scripts/collect-c2-env.ps1`（已按 CLSID 识别输入法，见 §9） |
| 其余 | 人工照 §3 的"判定方式"列执行，截图归档到 `docs/releases/native-quicknote/` |

## 8. 变更记录

| 日期 | 变更 |
|---|---|
| 2026-11-25 | 起草。A 级 24 条、B 级 6 条、C 级 5 项待裁决 |
| 2026-11-25 | **裁决完成**（§5）：C1 留、C2 留、C3 延后 Phase 3、C4 不做、C5 延后。C1/C2 升为 A 级 Q25–Q29，Q8 挂起，生效 A 级条目 23+5=**28 条**。状态 `draft` → `approved` |
| 2026-11-25 | 1b.1 键位回填：Q14 ✓（Ctrl+D 实测）、Q16 ◐（Ctrl+T 实测，键位未读配置）、Q19 ✓（`editor_focus` 日志）、Q12/Q15 ◐、Q17 ◐（中文输入法改写 vk，待人工按键）。实施记录见 [phase1-quicknote-shell.md](./2026-11-25-phase1-quicknote-shell.md) §8.4 |
| 2026-11-25 | 1b.2 多窗口回填：Q5 ✓（关闭只销毁该窗口，`open_notes` 2→1，其余笔记存活；新建是空文档）、Q12/Q14 更新为"销毁而非退进程"、Q18 ◐（`Ctrl+Alt+N` 在焦点属于别的窗口时实测生效，其余三条待托盘）。实施记录见 §8.5 |
| 2026-11-25 | 1b.3 托盘与全局键回填：Q18 ◐（四条注册，N/D/H 实测生效，M 因管理中心未做只记日志）、Q5 去掉「关完即退进程」的补注（托盘常驻后进程留下）。实施记录见 §8.6 |
| 2026-11-25 | 1c 透明度回填：Q28 ✓、Q29 ✓、Q6 ✓（标签与窗口 alpha 同源）。实现路径 `LWA_ALPHA`（B5 允许）；GHOST 不能是 alpha 0（实测不可命中，旧版靠 CSS 保住的指针事件在这里不成立）。实施记录见 §8.7 |
| 2026-11-25 | 1c.2 试做「背景完全消除 + 文字 20%」（色键，`LWA_COLORKEY \| LWA_ALPHA`）：技术上成立，但用户实测观感不对（文字像被描边、背景仍有薄片残留），**当日回退**。Q6/Q28/Q29 的验收标准不变（仍是 alpha 1 的幽灵）；Mica 挡色键等实测结论留在 §8.7 |
| 2026-11-26 | 1d 数据层回填：Q20 ✓、Q21 ✓、Q22 ✓、Q23 ✓（正文接 `notes.db`，门禁 `scripts/check-native-notes.py` 24/24）。附带：`--note-id` 按 id 重开（正文与光标位置都验到）、标签行剥离。实现记录见 [phase1-quicknote-shell.md](./2026-11-25-phase1-quicknote-shell.md) §8.8。**`Ctrl+K` 切换器（Q25–Q27）与数据恢复（Q24）仍未做** |
| 2026-11-26 | 2e Q7 收尾：`•••`/置顶按钮的点击路径修好（拖拽区必须在下、按钮在上，且按在按钮上不启动 `StartDrag`）；点正文关闭面板（`hub-editor` 新增 `on_click`，`app::click` 计数并唤醒本窗口）。`check-native-tools.py` 8/8。**§8.9 里"按钮点不动未验"由此变成已验** |
| 2026-11-26 | 2a–2d 交互层回填：Q13 ✓、Q16 ✓、Q25 ✓、Q26 ✓、Q27 ✓（`scripts/check-native-switcher.py` 22/22、`check-native-manager.py` 19/19）。快捷键收成一张表（`hub-core::shortcuts`）+ 设置文件稀疏覆盖 + 每个带键位按钮的悬浮提示；工作台（笔记/回收站/待办/设置）与切换器落地。Q18 仍 ◐（`Ctrl+Alt+M` 被 shipped 版占着，门禁改用改键后的 `Ctrl+Alt+J` 验通）。实施记录见 [phase1-quicknote-shell.md](./2026-11-25-phase1-quicknote-shell.md) §8.9。**Q24 数据恢复、收件箱与脚本分区仍未做** |
| 2026-11-26 | 2f Q9 置顶回填（✓）：`check-native-pin.py` 11/11，判 OS 的 `WS_EX_TOPMOST` 位与图标的填充/空心两态。**Q16 的 ✓ 是虚的**——同一函数发的 `WindowLevel` 会把窗口隐藏（winit 按缓存样式重算 `WS_VISIBLE`），键盘路径此前只验了日志；改走 `hub_win::window::set_topmost` 后两条路径一并修好。实施记录见 §8.11。Q10 拖动已排下一项 |
| 2026-11-26 | 2g Q10 拖动回填（✓）：`check-native-drag.py` 9/9（连跑 4 次）。`ViewportCommand::StartDrag` 在本环境不发车（`GetWindowRect` 不动，普通窗口探针同样不动），改由 `note/drag.rs` 每帧按光标位移 `SetWindowPos`，光标位置取 `GetCursorPos`（egui 的客户区坐标会跟着窗口跑，用它算位移恒为 0）。左/中/右槽位移逐轴精确、按钮起拖不动。实施记录见 §8.12 |
| 2026-11-26 | 2h Q24 数据恢复回填（✓）：草稿写 `{store_root}/autosave/{id}.json`（与现版同形），`save` 在写库前落盘、成功后删除；打开时按"与行不同且不比行旧"判定，`hub-ui/recover` 手绘三按钮卡片、`note/recover` 执行三种回答。`check-native-recover.py` 15/15（连跑 3 次）。**与现版不互通（`localStorage` vs 文件）**，且"保留当前"会删文件（现版只关对话框、下次再问）。实施记录见 §8.13 |
| 2026-11-26 | 2i 光标落点（Q19 的"末尾"修正）：带标签的笔记打开时光标停在标签行里，续写的字进了标签（§8.13 的遗留）。`note_text::caret_offset` 给出正文末尾，`hub-editor::caret.rs` 的 `position_of` 把字节换算成 RichEdit 的**字符位置**——`EM_SETSEL` 里一对 `CRLF` 只算 1，按 UTF-16 换算会正好落在标签行开头（实测 `at=8 of=17` vs 对的 `at=7 of=15`）。打开/切换/恢复草稿共用 `park_caret`。`check-native-notes.py` 25/25、`check-native-recover.py` 16/16、`check-native-switcher.py` 22/22、`cargo test` 212 项。实施记录见 [phase1-quicknote-shell.md](./2026-11-25-phase1-quicknote-shell.md) §8.14 |
| 2026-11-26 | 2j C5 尺寸记忆（✓）+ `quick_note.schema`：拖动结束后静默 400 ms 才写盘（`note/size.rs`，读改写文件、`sized()` 夹到 150×60…8000×8000），同一进程靠 `Arc<Mutex<(u32,u32)>>`、下次启动靠文件；2 px 容差用来吸收 `逻辑→物理→逻辑` 的取整。`schema` 缺失按 1 处理，遇到更高版本不降级。`check-native-size.py` 11/11。**顺带实测出一个 Q3 的问题**：无边框窗口边缘的 `WM_NCHITTEST` 全返回 `HTCLIENT`（`style=0x16cf0000` 有 `WS_THICKFRAME`），鼠标拖边缘改不了尺寸，门禁因此记 `drag=programmatic`。实施记录见 [phase1-quicknote-shell.md](./2026-11-25-phase1-quicknote-shell.md) §8.15 |
| 2026-11-26 | 2k Q3 边缘 resize（✓）：§8.15 挖出的"系统没有 resize 边框"由应用自己补——`note/edge.rs` 的 `Grip`/`grip_at`/`resized` 是纯函数（7 个单测），`Signal` 同时喂 `WM_LBUTTONDOWN` 认领与 `WM_SETCURSOR` 光标；`hub-win` 加 `set_frame`/`window_rect`/`left_button_down`。五条边各改各的、左边缘拖动时右边缘是常量、下限 150×60 自己夹（OS 只拒尺寸不拒移动，否则窗口会边滑边不变窄）。`check-native-edge.py` 23/23（连跑 3 次）。实施记录见 [phase1-quicknote-shell.md](./2026-11-25-phase1-quicknote-shell.md) §8.16 |
## 9. 附：`collect-c2-env.ps1` 输入法漏检的修正（同日）

同一轮对话里发现采集脚本把**已启用**的搜狗与微信输入法报成"未安装"。两个独立原因，都已在脚本里修掉：

| 原因 | 事实 | 修法 |
|---|---|---|
| 名称匹配 | 微信输入法的 TSF 服务注册名是 **`WeType`**（二进制 `wetype_tip.dll`），正则 `/wechat|微信/` 永远打不中 | 改为按 TIP CLSID 判定 |
| 描述读半边 | TIP 的 `Description` 只在 HKLM，HKCU 镜像通常为空；只读一个 hive 会看到无名服务 | HKLM + HKCU 合并，首个非空即用 |
| 捕获组少括号 | 正则把 `{}` 留在捕获组外，得到无括号 GUID，与表里的 `{GUID}` 恒不相等（14:08 首次改完仍全 red） | 括号纳入捕获组 |

修正后的实测（`docs/releases/native-c2/environment.md`，2026-11-25 14:08）：

| 输入法 | CLSID | 版本 | 二进制 |
|---|---|---|---|
| Microsoft Pinyin | `{81d4e9c9-…}` | n/a（inbox） | 无 InprocServer32 |
| 搜狗拼音 | `{e7ea138e-…}` | 16.9.0.5795 | `SogouTSF.ime` |
| 微信输入法 | `{86598fb9-…}` | 1.4.2.12 | `wetype_tip.dll` |

时间线佐证：`SogouTSF.ime` 落盘于 2026-10-27 13:20（**早于本次采集**，确实漏检）；`wetype_tip.dll` 落盘于 2026-11-25 12:01（采集前 2 小时）。两者当时都在语言列表里。
