# 工作台的按钮：卡片入口补到原生侧，行内按钮第一次真点

上一批给工作台加了「新建笔记」，并顺手写下一句「前端 `createTodoCardWindow` 仍无调用者」。
这句话是错的：`src/components/manager/todos/TodoPanel.tsx` 里一直有「悬浮卡片」按钮。
真正的问题更窄也更尴尬——**接管态下那块 React 面板不在屏幕上**，按钮存在但没人能按到。

所以这一批补的是原生工作台的入口，以及把行内那几个按钮从「只有单测覆盖」变成「门禁真点过」。

- 状态：`completed`（2026-11-28）
- 门禁：`scripts/check-native-manager.py` **23 → 32**
- 单测：原生 `cargo test --locked --workspace` **335 → 338**（hub-ui +3）
- 改动：`native/crates/hub-ui/src/manager.rs`、`native/app/src/manager.rs`、`scripts/check-native-manager.py`

## 1. 卡片入口：同一条请求，不是一个新窗口

待办页的标题行右侧加「悬浮卡片」，返回 `Action::TodoCard`，控制器 push `Request::TodoCard`。
这和托盘菜单、`--todo-card` 走的是**同一个请求**，而 `open_todo_card` 已经是幂等的（已开则
`Focus`），所以三处入口不会各开一张卡。日志 `manager_todo_card from=button` 与
`todo_card action=open` 两条一起，才说明"这次点击真的开了窗"。

## 2. 命中矩形从"一个字段"变成"一张表"

上一批的 `State` 只有 `new_note: Option<Rect>`。要覆盖行内按钮，这个形状不够：

- 名字：`new-note` / `todo-card` / `tab:trash` / `row:<id>` / `pin:<id>` / `trash:<id>` /
  `restore:<id>` / `purge:<id>`。行按钮带 id，因为列表里同一个"置顶"有好几个。
- 生命周期：`hits` 每帧清空重建。行滚走了，它的名字就不再出现——**上一条日志留着，
  但门禁只读最新一条**，所以不会被点到旧坐标。
- 写日志：`app` 侧用 `HashMap<String, [i32; 4]>` 做差集，只写移动过的那些。每帧每按钮一行
  会把其它日志埋掉。

行本身也要上报（`row:<id>`）：按钮只在**选中的行**下面出现，所以门禁得先能"选中第三行"——
而"第三行"正是这个文件从前拒绝硬编码的那种坐标。

## 3. 门禁怎么点

`check-native-manager.py` 的老注释写着"行内按钮一律不点，因为坐标会变"。前提是对的，
结论是错的——待办卡片早就给了更好的契约：**产品上报矩形，门禁点上矩形**。所以这一批把
那句话改掉，并补上真点击：

| 动作 | 断言 |
|---|---|
| 点行 | 只该行的 `pin/trash/open` 出现，别的行没有 |
| 置顶 | `manager_pin … ok=Some(true)` 且库里 `is_pinned=1` |
| 移到回收站 | `manager_trash` 且 `deleted_at` 非空（带时间戳） |
| 恢复 | `manager_restore` 且 `deleted_at IS NULL` |
| 彻底删除 | `manager_purge … permanent=true` 且该行数为 0 |
| 悬浮卡片 | `manager_todo_card from=button` + `todo_card action=open` + 卡片窗存在 |

断言读日志时用"点击前的位置标记"（`Trace.tail`）：日志文件在同一次运行里是追加的，
一条旧行不能替这一次作答。

## 4. 两个坑，一个是真 bug

**回收站的行点了不会有反应。** `note_row` 原来只在返回 `Action::Open` 时才设
`state.selected`，而被删的行没有 `Open`——于是「恢复」永远只能作用于**刚删掉的那一行**
（它的选中状态是从笔记页带过去的）。门禁点击回收站里的另一行、等着 `manager_restore`，
什么也没等到，暴露了它。修法是一句话：选中与有没有动作无关。`hub-ui` 里补了一条
headless 测试，按下+松开两帧，断言 `selected` 与 `restore:<id>` 都出现。

**笔记窗口盖住工作台右上角。** 点行会打开笔记，而快速笔记是置顶创建的；门禁的点击是
屏幕坐标上的**真鼠标事件**，于是"点按钮"变成了"点笔记正文"。诊断靠打印
`WindowFromPoint`——落点是 `RICHEDIT50W`。修法不是改产品，而是门禁在每次点击前把
生成的笔记挪到 `-4000,-4000`（`#0` 保留，另有断言要看它活着）。

顺带说明：`SetForegroundWindow` 在这台机器上偶尔会被拒绝（诊断里 `fg=` 与工作台句柄
不一致），但点击仍然落在正确窗口——说明焦点不是必要条件，**z 序才是**。而 z 序有时也没用：
有一个笔记窗口在还没写上标题的时候就被创建了（`GetWindowTextW` 为空），按标题挪不走。
所以门禁最后是自适应的：落点被占就把工作台平移 160 px 再试，最多六次。上报的矩形是
**客户区相对坐标**，窗口移动不会让它们失效。

## 5. 顺手把待办解析合到一处

`hub-ui/src/manager.rs` 里有一份自己的 `todo_items`，注释写着"放在这里是有意的，因为
这是视图的列表"。上一批做完桌面卡片后这句话就过期了：卡片读的是 `hub-core::todo`，
两处各解析一遍，第一次有人写有序列表就会分叉。现在列表直接用
`hub_core::todo::lines`，`TodoItem` 删除，`manager.rs` 从 799 行回到 767 行（离 800 的
上限只剩一行时不该再加东西）。

## 6. 未做

- 回收站里的「彻底删除」没有二次确认，门禁点下去就真删。产品上大概需要一次确认。
- 待办页目前只读（列出 `- [ ]` 行），勾选仍只在桌面卡片里做。
- `Action::Open` 与行点击重复：现在点行即打开，旁边的「打开」按钮是给"已选中但要再开"用的。
