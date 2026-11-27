# 快速笔记的入口从 Manager 接回原生壳

> 状态：`completed`
> 上游判据：交接方案 [2026-11-27-tauri-handover-t-gate-0.md](./2026-11-27-tauri-handover-t-gate-0.md) §5——「Manager 里的"新建快速笔记"入口会报错。按约定选"拒绝并提示"而不是"转发给原生壳"：原生壳还没有单实例机制。**（互斥已经就位，这条可以在 T-Gate 1 里改成转发。）**」
> 结果：`create_note_window` 在接管态不再返回错误，而是把请求交给 `easyflowhub-native.exe`；`check-native-single-instance.py` 从 17 项到 **22/22**。

## 1. 一句话

T-Gate 0 把快速笔记窗口搬进了原生壳，却把「打开它」这条路留在 Tauri 侧——那条路当时只会回一句「已由原生壳接管」。互斥（[2026-11-27-native-single-instance.md](./2026-11-27-native-single-instance.md)）补上之后，这次启动可以是一条**请求**而不是第二个进程，于是可以真的转发了。

## 2. 一个事件，两种答案

转发靠的是 `hub_win::instance` 里的 auto-reset 事件：第二进程 `SetEvent`、第一进程 `WaitForSingleObject`，然后各自退出/应答。

`--note` 原本与普通二次启动**共用同一个门铃**，于是「给我一条笔记」被解释成「把面板显示出来」——请求方想要的答案丢了，而两边都以为自己成功了。

一条事件带不了 payload，也不能表达"哪一种请求"。所以干脆分成两条：

| 门铃 | 谁按 | 谁听 | 答案 |
| --- | --- | --- | --- |
| `Local\EasyFlowHub.Native.Shell.reveal` | 普通二次启动、`--resident` | reveal watcher | `launcher.reveal_when_ready()` |
| `Local\EasyFlowHub.Native.Shell.note` | `--note` | note watcher | 往 inbox 推 `Request::NewNote` / `OpenNote(id)` |

两条各一个 watcher 线程（`Guard::watch` / `Guard::watch_note`）。**不是**在一个线程里分支：note 的 spawn 要走窗口创建，慢的时候不该把面板 reveal 排在它后面。

两个事件都在 `claim()` 里与互斥一起创建——登录竞态的教训不变（信号落在"已持有"与"有人在等"之间会被丢掉）。测试补了三条：note 环能到达 note watcher、note 环**不会**唤醒 reveal watcher、不存在的 note 环报 `OpenEventW` 错误。

## 3. 那个带不了的 id

`--note-id <id>` 要送一个字符串过去。可行的方案各有一条否决理由：

- **命名共享内存**：要定尺寸、生命周期、并发协议，为一个短字符串引入一套 IPC。
- **`WM_COPYDATA`**：要一个壳从没公开过的窗口句柄；壳的窗口是 egui 建的，标题可枚举但类名不稳定，拿它当契约很脆。
- **就写一个文件**：小到可以全文读完，且失败方式是"没有笔记"而不是"打开错的笔记"。

选了第三种。`native/app/src/pending.rs`，文件 `pending-note` 就在设置文件旁边（`--settings` 一动两个一起动，门禁之间不会串）。

三条性质就是全部设计：

1. **写临时文件再 `rename`**。原地写会让读者读到半个 id，然后去开一条不存在的笔记。同卷 `rename` 原子，文件要么是上一个请求、要么是这一个。
2. **读一次就删**。环是 auto-reset，N 次启动坍缩成一次应答；文件留着，下一次**无关**的环会再开同一条笔记。
3. **60 秒过期**。崩在"写了"与"按铃"之间的孤儿不能在一小时后生效。60 s 远大于实测 0.1 s 的握手，又短到不会让人说"我没要这条笔记"。

`take()` 把"没有文件 / 读不动 / 太旧"三种情况合成一个 `None`，因为调用方对三者的答案一样（开一条新笔记）。

## 4. Tauri 侧：只转发，不解释

```rust
if crate::shell::current().takeover() {
    return crate::native_shell::open_note(note_id.as_deref());
}
```

`native_shell::argv` 是纯函数，单测直接钉形状：`--note` 在前，`--note-id` 带值在后（`args.rs` 的解析器要这个顺序：`--note-id` 吃下一个 token，而 `--note` 决定进 note 分支）。

`exe()` 的解析顺序：`EFH_NATIVE_EXE` → 本程序同目录 → `%LOCALAPPDATA%\EasyFlowHub\easyflowhub-native.exe`。环境变量排第一不是给生产用的：门禁要指向某个 target 目录里的构建，而安装目录里可能躺着一个更旧的壳——让旧的赢，门禁测的就是别的程序。`pick()` 收三个 `Option` 而不是去摸文件系统，这样"安装目录有个旧壳"这种情形可以在单测里表达。

`CREATE_NO_WINDOW`：原生壳是 console 子系统（它把 bench / probe 输出打到 stdout），直接 `spawn` 会为每条笔记闪一个黑窗。

**不等待子进程**。这个进程是窗口提供者，不是监督者：笔记窗口要活得比 Manager 久，而一个阻塞在 `wait()` 上的 Manager 会连带冻住自己的 webview。

### 诚实的记账

这条命令**目前没有前端调用者**。Manager 里那个按钮在更早一批随 webview 一起下线了，前端只剩 `create_note`（数据库行，不是窗口）。所以这一批交付的是"接口就绪 + 行为正确"，不是"用户现在能点到一个按钮"。

之所以仍然做：这是 `create_note_window` 从"永远返回错误"变成"真的能用"的唯一一处改动，接口就绪后再补按钮是前端一行的事；而如果反过来先补按钮，用户点到的就是报错。

## 5. 证据

`check-native-single-instance.py` **22/22**（原 17 + 5）：

```
PASS  --note as a second instance exits 0 and names the ring it rang: exit=0 in 0.08s
      stderr=instance action=forward ring=note
PASS  and the shell opens a note window for it: instance action=new-note windows=1 (was 0)
PASS  and the panel is untouched, so the two rings really are separate: reveals=2 (was 2)
PASS  --note-id hands the id to the running shell and it finds the row:
      seeded=True exit=0 note_open id=gate-seed source=manager
PASS  and a window opens for it: windows=2 (was 1)
```

第三条是这批的核心断言：`--note` 之后 reveal 的计数**没有变**。改名之前它一定会变，所以这条能失败。

第四条的门禁把行写进壳的 `notes.db`（`sqlite3` 模块直接插，不走 UI——点击建出来的笔记会连"点击者"一起证明），然后断言日志里出现 `note_open id=gate-seed` 且**没有** `found=false`。日志只在找不到时写 `found=false`，所以"没有它"就是"找到了"。

单测：原生 `cargo test --locked --workspace` **317**（`instance` +3、`pending` +5），Tauri `cargo test --lib --release` **105**（`native_shell` +5）。

回归：`resident` / `notes` / `launcher` / `recover` / `store-schema` / `manager` / `pin` / `tools` 八个原生门禁串行，全 PASS；架构门禁 `passed (5 crates)`。

## 6. 没做

- **待办卡片**（`create_todo_card_window`）仍然返回 `moved_to_native`。它在规范里还没有对等的原生窗口——先做出那个窗口，才有东西可转发。
- **Manager 里的按钮**。见 §4 的记账。
- **从笔记列表点开某一条**。`--note-id` 的链路已经通（第 4 条断言就是它），缺的是前端把行 id 传进 `create_note_window`。
- **门铃带 payload 的更通用做法**。如果以后要转发的不止"一个 id"，`pending-note` 的形态（单值、读一次、短过期）会不够用；那时再考虑共享内存，而不是在文件里塞 JSON。
