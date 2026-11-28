# Manager 的「新建笔记」按钮，与待办卡片的转发

上一批把待办卡片做成了真窗口，`create_todo_card_window` 却还回一句「已由原生壳接管」。
这一批收掉两条尾巴：命令改成**转发**（不再是拒绝），以及给工作台补一个**真的能点的新建按钮**——
在此之前空列表只会写「按 Ctrl+Alt+N 新建」，因为它没有可指的按钮。

- 状态：`completed`（2026-11-28）
- 门禁：`scripts/check-native-manager.py` **19 → 23**
- 单测：原生 `cargo test --locked --workspace` **335**、Tauri `--lib --release` **106**（+1）
- 改动：`easyflowhub-app/src-tauri/src/{native_shell,app_windows}.rs`、`native/crates/hub-ui/src/manager.rs`、`native/app/src/manager.rs`

## 1. 转发：两扇门不能共用一条 argv

`native_shell` 里已经有 `open_note`。加待办卡片时最省事的写法是给 argv 构造加个布尔：
`argv(note_id, todo_card)`。这个签名是错的，而且错得看不出来——`--note` 会开出一条
**空白笔记窗口**，`--todo-card` 才开卡片；参数传错时两边的调用者都觉得自己成功了。

所以是两个函数两个名字：`argv(Option<&str>)` 与 `todo_card_argv()`，后者只返回
`["--todo-card"]`，并有一条单测同时钉三件事：结果只有一个 flag、**不含** `--note`、
不等于 `argv(None)`。共用的是 `launch(kind, args)`——真正容易错的那些（解析 exe、
`CREATE_NO_WINDOW`、**不等子进程**）在两个入口上是同一件事，`kind` 只用来让日志回答
"哪扇门进的"。

服务端语义：原生侧 `Request::TodoCard` 已经是幂等的（已开则 `Focus`），所以重复点
不会堆出第二个卡片窗口。

## 2. 按钮：一个 `Action`，走的还是热键那条路

`hub-ui` 的头部右侧加了「新建笔记」（`border_strong` 描边，和「关闭」并排；
`right_to_left` 里先加的那个在最右，所以绘制顺序是「新建笔记」在左、「关闭」在右）。
点击返回 `Action::NewNote`，`app/src/manager.rs` 收到后 push `Request::NewNote`——
**与 `Ctrl+Alt+N` 请求完全相同**，因为开笔记会创建视口，而视口不能在某一帧里建。

空列表文案随之改成「没有笔记 · 点「新建笔记」或按 Ctrl+Alt+N」：按钮存在之后，
还只教快捷键等于藏了一半功能。

状态行写「已新建笔记」，日志一行 `manager_new_note from=button`——`from=button` 是给门禁
用的，它要证明的是"这一次是点击引起的"，而不是恰好有别的东西开了笔记。

## 3. 布局坐标不硬编码：`manager_hit`

管理器门禁此前明确写着「按钮不点」，理由是 egui 布局会动，硬编码坐标的失败与被测行为无关。
这条理由在待办卡片那边已经被更好的契约取代：**产品上报矩形，门禁按上报的坐标点**。

于是 `State` 多一个字段 `new_note: Option<egui::Rect>`（客户区**逻辑**点），
`app` 侧每帧比对，变化才写一行：

```
manager_hit button=new-note x=982 y=16 w=80 h=27
```

物理客户区像素，正是 `click_client()` 要吃的东西（脚本不必知道显示器缩放）。
只在变化时写，避免每帧一行把日志埋掉。

## 4. 门禁怎么判

`check-native-manager.py` 新增四项，插在"重开工作台并重读数据库"之后（那时管理器开着、
已有一条笔记）：

1. 日志里出现了 `manager_hit`，且 `w>0 h>0`——先证明矩形可读，否则后面三项无从谈起；
2. 按矩形中心 `click_client` 后出现 `manager_new_note from=button`；
3. `note_spawn id=1 live=2` **且** `easyflowhub-note#1` 窗口真的存在——日志说"开了一条"
   与"屏幕上多了一个窗口"是两件事，两条都要；
4. 管理器**没被关掉**（`manager_close` 仍为 1，即之前 Esc 那一次）。

实测：`x=982 y=16 w=80 h=27`、点击后 `note_spawn id=1 live=2 cursor=1489,530`、
`#1` 窗存在、管理器保持打开 → **23/23**。

## 5. 转发这一半的证据链（诚实版）

Tauri 侧仍然**没有可点的按钮**：前端 `createTodoCardWindow` 至今没有调用者，
唯一的入口是命令行与托盘。所以这半边的证据是链条而不是点击：

- argv 单测保证转发启动的是 `--todo-card` 而不是 `--note`；
- `check-native-todo-card.py` **17/17** 保证 `--todo-card` 真的开窗、真的能勾选写回；
- `shell.rs` 的 `--shell-report` 早就写着 `todoCard: native`（结构门禁 16/16 里的一项）。

三者合起来是"这条命令指向的东西是对的"，而不是"在 Manager 里点一下看见了卡片"。
后者要等前端有按钮，属于产品工作，不是接口工作。

## 6. 没做的

- 前端 `createTodoCardWindow` / 「新建待办卡片」入口（同上，产品侧）。
- 工作台上其他行内按钮仍不点（打开/置顶/删除/恢复/彻底删除）：它们的判定在 `hub-core`
  的 store 测试与 `hub-ui` 的 `Action` 测试里，点击不增加信息。
- `manager_hit` 只有 `new-note` 一个按钮。若以后要给更多按钮加点击门禁，先把它改成
  `manager_hit button=<id>` 的多行形式，而不是加第二个字段。
