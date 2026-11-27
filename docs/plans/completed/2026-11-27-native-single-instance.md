# 原生壳单实例互斥：一个会话一个壳，第二次启动是一次请求

> 状态：`completed`
>
> 上游判据：[T-Gate 0 交接方案](../active/2026-11-27-tauri-handover-t-gate-0.md) §6「T-Gate 0 真正关闭还缺两件」的第 2 件、§7「还没做」
> 归属批次：Phase 1 收口（主题 token 之后）
> 门禁：`python scripts/check-native-single-instance.py --exe <release exe>`（17 项）
> 数字来源：本机单屏 3840×2160 @150%、Win11 26300、release 构建

## 1. 要解决的问题

登录自启拉起原生壳，用户在登录瞬间又点了一次图标（或者干脆是两次点击落在同一个 50 ms 里）。没有互斥时，第二个进程会把第一个进程做过的事再做一遍：

- 第二个托盘图标，看起来一模一样，用户不知道哪个是活的；
- 第二轮 `RegisterHotKey` 全部 `1409`——第二个进程承诺了五个组合，一个都不归它；
- 第二个进程同样打开 `notes.db`、同样建一个藏到屏外的面板窗口。

Tauri 版有 `tauri-plugin-single-instance`，原生壳从 S1 起就没有。这一条一直卡着"把自启真指向原生 exe"，也卡着"把 0 个 webview 这件事脚本化"（脚本化要起第二个进程看它的 argv 转发）。

## 2. 三个决定

**`Local\` 而不是 `Global\`。** Windows 会话是相互独立的桌面：控制台一个用户、RDP 另一个用户，两人各自想要自己的壳是合理的。`Global\` 会让第二个人拒绝启动，而拒绝的理由（"另一个会话已经有一个了"）对他来说无法处理。被保护的东西——托盘图标、全局组合、藏起来的窗口——本来就是按会话的。

**用事件承载请求，不用互斥本身。** 互斥带不了载荷；而且等它在语义上是要等对方**退出**，那恰好是第二次启动的反面。自动重置事件是一只门铃：第二个进程按一下就走，第一个醒来应答。

**事件和互斥一起在 `claim()` 里创建，不在监听线程里创建。** 落在"已经持有壳"和"有人在等信号"之间的信号会被丢掉，而这个窗口正是登录竞态：自启的实例正在被 claim，桌面还在铺垫，第二次点击已经到了。事件先建好，门铃就是通的——自动重置事件在被等待之前一直保持置位，所以后来的 `WaitForSingleObject` 仍然立刻返回。这条有一个专门的单测（`a_forward_sent_before_the_watcher_starts_is_not_lost`）。

## 3. 顺序：claim 必须在 `Log::new` 之前

`Log::new` 的第一件事是 `remove_file`——它假设自己是这份日志唯一的主人。如果第二个进程先开日志再发现自己是多余的，它会**先抹掉第一个进程的证据**，然后退出；之后所有门禁看到的是一个"启动了但什么都没写"的壳。

所以 `main.rs` 的顺序是：`claim()` → 拿到壳才 `Log::new` → 开库 → 注册组合 → 装托盘。

这条顺序在门禁里是可观察的，而且只有一种写法能观察到：给第二个进程一个**它自己的** `--log` 路径，然后断言这个文件**不存在**（`it wrote no log at all (the claim precedes the log)`）。如果顺序被反过来，检查立刻变红。

## 4. 第二次启动的语义

| 情况 | 行为 | 为什么 |
|---|---|---|
| 第一个进程 | 持有互斥，正常跑 | — |
| 用户再点一次图标 / 再跑一次 `--resident` | 转发后退出 0，持有者 **reveal** | "我想要眼前这个东西"，不是"切换它的状态" |
| 自启的第二个进程（`--autostart`） | 转发后**不出声**，退出 0 | 登录时桌面还在铺垫，弹面板是错的第一印象；与 Tauri 版 `--autostart` 同义，注册表项可平移 |
| 转发早于面板窗口存在 | 等窗口（≤3 s），再 reveal | 见 §5 |
| 面板已经在屏上 | `refocus`，不再 show | 双击它是因为看不见它，多数情况是被别的窗口压住 |

`reveal` 而不是 `toggle`：第二次双击不能读成"把它关掉"。

`--autostart` 同时蕴含 `--resident`：登录入口不该开笔记窗口。两个含义合在一个 flag 上，因为它们是同一件事（"这是登录拉起的那一次"）的两个面。

## 5. 顺带修的一个真 bug：用热键隐藏从来没有真正停窗

`Handle::toggle` 在热键线程上先把 `visible` 置为 false（它靠这个判断方向），然后把隐藏请求排给 UI 线程。UI 线程处理时调用 `Shell::hide`，而它的第一行是：

```rust
if !self.handle.visible() { return; }
```

于是**只要隐藏是从热键来的，这个函数就立刻返回**：窗口留在屏上，标志位却是"已隐藏"。下一次按同一个键它会走 show 分支——`visible` 变 true，窗口原地再显示一次——所以症状是"按一下没反应，再按一下好像动了"，而日志里只有 `action=hide`，没有 `launcher_hide`，更没有停窗。

**为什么 25 项 launcher 门禁没抓到**：它测了三个隐藏入口——`reason=action`（回车执行后）、`reason=esc`、`reason=blur`——**三个都在 UI 线程上直接调用 `Shell::hide`**，那时 `visible` 还是 true，守卫成立，一切正常。热键这条入口是唯一排队的那条，一次都没被按过第二下。

**修法**是把"停窗"从守卫里拆出来：

```rust
fn park(&mut self, reason: &str) { /* 真的挪窗口、还键盘、写 launcher_hide */ }

pub fn hide(&mut self, reason: &str) {        // 面板自己的三条路
    if !self.handle.visible() { return; }
    self.park(reason);
}

pub fn hide_if_current(&mut self, epoch: u64, reason: &str) {  // 排队那条路
    if self.handle.epoch() != epoch { /* skipped=superseded */ return; }
    self.park(reason);   // 不经 visible 守卫：epoch 已经证明这是最新意图
}
```

那条 `visible` 守卫想近似的东西（"这次隐藏还算数吗"）在排队路径上正好由 epoch 回答，而且答得更准。

**门禁补在它该在的地方**：`check-native-launcher.py` 新增两项——"用同一个热键打开它"和"用同一个热键关掉它，且窗口真的挪走"。25 → 27 项。

## 6. 转发早于面板窗口：排队而不是丢掉

第一次跑门禁时转发是被丢掉的：

```
instance action=reveal reason=second-launch
launcher_toggle ready=false reason=no_window
```

面板的窗口由第一帧创建（`show_viewport_deferred`），而转发在 356 ms 就到了——比第一帧早。按键在这个时刻丢掉是对的（用户按的是"现在"，他会再按），**启动请求不是**：它是排队的，早一个帧到达不构成丢弃的理由。

`Handle::reveal_when_ready` 因此等到 `hwnd` 存在（上限 3 s），**并且每轮都 `request_repaint_of(ROOT)`**：窗口是帧建出来的，而屏幕上什么都没有时控制器会静止——只睡觉不叫帧，等的是一个没人会画的帧。实测等待 82–91 ms（第一帧 + 一帧）。

## 7. 证据

| 项 | 结果 |
|---|---|
| `hub-win::instance` 单测 | **7 项**：第二个 claim 被拒、`owned()`、drop 后还能被 claim、无主时 signal 报错、转发到达监听器、**转发早于监听器不丢**、两次转发合成一次 reveal |
| `args` 单测 | +2：`--autostart` 蕴含 resident/note、普通 `--resident` 不是 autostart |
| `check-native-single-instance.py` | **17/17** |
| 第二个实例退出耗时 | **0.09–0.12 s**（预算 3 s） |
| 第二个实例写了几个字节 | **0**（`second.log` 不存在 → claim 先于日志） |
| 转发到面板上屏的等待 | `waited_ms=82.1` / `90.7` / `3.8`（已就绪时） |
| 持有者侧计数器 | `hub_start=1`、`tray installed=true`=1、`instance claimed=`=1 |
| 转发后仍有热键 | 第三次 `launcher_shown`，进程活着 |
| 进程数 | `processes=1`（`tasklist` 计数） |
| 回归 | launcher **27/27**、theme 17/17、resident 16/16（连跑 2 次）、notes 25、manager 19、switcher 22、store-schema 24 |
| `cargo test --workspace --release` | **283 项**（274 → 283） |
| 架构门禁 | 通过；`app/src/launcher.rs` 875 行破 800 硬顶 → 拆出 `launcher/handle.rs`（254 行），`launcher.rs` 回落到 647 |

**门禁自己修掉的一个竞态**：`check-native-resident.py` 把 `hotkeys registered=` 当就绪信号，然后立刻断言 `hub_start`——而 `hub_start` 写在它之后约 20 ms。十次里大约一次会在负载下变红，读起来完全像"壳没启动"。现在两行都等齐再断言。

## 8. 没做的

- **自启注册表还没指向原生 exe**。本轮只让 `--autostart` 这个 argv 可用（并让它在第二个实例上不出声），把登录项切过去仍属 G1 的活。
- **转发不带载荷**。第二个实例的 `--note-id x` 会被丢掉：互斥只转"请显示面板"这一个意图。要转发参数就得有 IPC 载荷（命名管道），而目前没有需要它的场景。
- **不隔离调试构建与发行构建**。两者同名互斥是有意的（对用户的桌面来说它们是同一个壳），代价是开发时不能同时跑两份——门禁因此把"已有壳在跑"报成无法运行（退出码 2）而不是失败。

## 9. 变更记录

| 日期 | 变更 |
|---|---|
| 2026-11-27 | 新增 `hub-win::instance`（命名互斥 + 转发事件 + 监听线程，7 项单测）；`main.rs` 在产品路径最前面 claim，第二个实例转发后退出 0；`--autostart` 蕴含 `--resident` 且转发不出声；`launcher::Handle` 新增 `reveal` / `reveal_when_ready`（等窗口且叫帧）；`Shell::hide` 拆出 `park`，修掉热键隐藏从不停窗的旧 bug；拆出 `app/src/launcher/handle.rs`；新增门禁 `check-native-single-instance.py`（17 项），launcher 门禁 25 → 27 项，resident 门禁修正就绪竞态 |
