# T-Gate 1：「0 个 webview」变成一条能失败的断言

> 状态：`completed`
> 上游判据：T-Gate 0 的收尾项——[2026-11-27-tauri-handover-t-gate-0.md](./2026-11-27-tauri-handover-t-gate-0.md) §6.4「`--shell-report` 只证明构建行为，不证明启动之后 0 个 webview」
> 门禁：`python scripts/check-tauri-resident.py --exe <release exe>`（18 项）
> 数字来源：本机单屏 3840×2160 @150%、Win11 26300、release 构建、`CARGO_TARGET_DIR=/tmp/tauri-target`

## 0. 一句话

结构门禁判**计划**，这道门禁判**进程**：它启动真的二进制，从 Windows 那边数 WebView2 子进程、数窗口，并且**第一次跑就抓到**「关掉 Manager 会把托盘一起带走」——T-Gate 0 之后才出现的回归。

## 1. 为什么结构门禁不够

`check-tauri-takeover.py` 在 `tauri::Builder` 之前就返回：它回答「这个构建**会**声明哪些窗口、**会**注册哪些热键」。这挡不住三件事：

- 从零开始不建窗口，**不等于**运行中不建。回退路径、插件、按需创建都可能把 webview 拉起来。
- 这个构建的日志在**有界内存缓冲**里（除非用户开了调试），所以「0 个 webview」没法从日志读，只能从外部量。
- 第二个进程的行为（转发后退出、还是变成第二个宿主）本身就是断言对象。

## 2. 怎么观察

| 判什么 | 用什么 |
|---|---|
| WebView2 子进程 | `CreateToolhelp32Snapshot` + `Process32First/Next` 拿父链，只数 `msedgewebview2.exe` |
| 窗口 | `EnumWindows` + `GetWindowThreadProcessId` + `IsWindowVisible`，只看属于被测 pid 的顶层可见窗 |
| 提交内存 | `GetProcessMemoryInfo` 的 `PrivateUsage`，**打印不断言**（它是佐证，不是判据） |

两个必须做的过滤，否则断言永远不成立：

- **单实例会合窗口**：插件保留 `{identifier}-siw`（类名 `{identifier}-sic`）作为转发信箱，`IsWindowVisible` 报 true。它不是「打开的窗口」，按类名后缀排除。
- **测试模式窗口**：`easyflowhub-note` 之类前缀不属于本进程，不需要过滤（pid 已经限定），但 `-sic` 必须在。

## 3. 四个状态

| 状态 | 断言 | 实测 |
|---|---|---|
| `resting` | 一个进程、0 个 WebView2、0 个窗口，且**隔 2 秒再量一次**仍是 0（排除「还没起来」） | `private=4.6 MB`、`webviews=[]`、两次都 0 |
| `quiet` | `--autostart` 二次启动：exit 0、<3 s、什么都没叫出来 | `0.04 s`、仍是同一个 pid、0 窗口 0 webview |
| `on demand` | 普通二次启动：Manager 出现、真的建了 webview、宿主还是同一个进程 | 窗口 `EasyFlowHub`（类 `Tauri Window`）、webview 从 0 长到 6 |
| `closed` | 关掉 Manager：webview 归零、**进程还在**、再叫一次能回来 | 归零后 `alive=True`；二次启动 `0.04 s` 又建回来 |

外加一条：在接管态按 `Ctrl+Alt+Space`，本进程**什么都不建**（那个键属于原生壳，这里只是证明它没有偷偷留着监听）。

## 4. 门禁逼出的真缺陷：关掉 Manager 会把托盘一起带走

第一次跑（修复前）：

```
PASS  closing the Manager releases its webview: windows=0 webviews=0
FAIL  and the tray process is still there to be asked again: alive=False pids=[] exit=0
```

根因在 Tauri 的默认规则，不在本项目的代码：`runtime` 收到最后一个窗口的 `Destroyed`、窗口表清空，就发 `ExitRequested { code: None }`；没有人 `prevent_exit` 就 `ControlFlow::Exit`。T-Gate 0 之前 launcher 窗口一直存在（只是隐藏在屏外），所以窗口表永远不空——**这是接管之后才出现的回归**。而接管态下本进程一个热键都不注册，托盘是唯一入口：关掉 Manager 等于把唯一入口也关了，用户只能重新启动程序。

修法（`src-tauri/src/lib.rs` 的 `app.run`）：

```rust
tauri::RunEvent::ExitRequested { code: None, api, .. } => {
    if shell::current().takeover() {
        api.prevent_exit();
    }
}
```

只挡 `code: None`。Tauri 的文档把 `None` 定义为「用户交互导致」（也就是最后一个窗口被关），`Some(_)` 是 `app.exit()`／`restart()`——托盘「退出」走的就是 `app.exit(0)`，仍然照常退出。回退模式不挡：那里 launcher 窗口一直在，语义没变。

**为什么不允许「关掉就退出」**：Phase 0.5 定的形状是「按需创建、关闭销毁」，省的是 webview 的内存；进程本身安静时只占 4.6 MB 私有提交，留着才叫托盘应用。

## 5. 门禁自身的三处时序修正

写这道门禁的过程里，有三条断言先失败、但都不是产品问题——记下来，因为下一道门禁还会踩：

1. **`webviews 1 → 6` 不是第二个 Manager。** WebView2 的 browser / gpu / renderer / crashpad 是陆续起来的，同一个窗口在不同时刻读 1 或 6 都正常。判「只有一个 Manager」要看**窗口数**，webview 数只打印。
2. **webview 先死、窗口后销毁。** `WM_CLOSE` 之后 webview 归零、HWND 要再过一会儿才消失，只等 webview 会把窗口的最后一口气读成「它留着」。两次关闭都要等两者同时归零。
3. **宿主已死时，第二次启动会变成新宿主。** 修复前那条 `alive=False` 之后，门禁又发了一次启动，它自己占了互斥、常驻不退出，把 `subprocess.run` 挂满 60 秒并抛栈。现在二次启动有上限（20 s），超时算答案（`code=None`）；「还能不能再叫回来」那三条只在宿主仍活着时才跑，否则明确写「not reached」，不让症状盖住原因。

## 6. 证据

```
18/18 checks passed
RESULT: PASS
```

| 断言 | 结果 |
|---|---|
| 常驻后 0 窗口、0 WebView2，两次采样 | `windows=0 webviews=0`（间隔 2 s） |
| `--autostart` 二次启动 | `exit=0 in 0.04s`，`pids=[31024]` 不变 |
| 普通二次启动 → Manager | `hwnd` 出现，webview 由 0 长到 6，宿主不变 |
| 再叫一次不建第二个 | `managers=1 windows=1` |
| 关闭 → webview 归零、进程仍在 | `windows=0 webviews=0 alive=True` |
| 再叫一次能回来 | `exit=0 in 0.04s`，新 hwnd |
| 接管态按 `Ctrl+Alt+Space` | `webviews=0 windows=0` |
| 宿主退出后无孤儿 WebView2 | `remaining=[]` |

交叉验证：同一构建 `check-tauri-takeover.py` **16/16**；`cargo test --lib --release` **100 passed / 0 failed / 1 ignored**（与 T-Gate 0 基线一致）。

## 7. 怎么重建（这一步本身就是坑）

`easyflowhub-app/src-tauri/target` 在这台机器上**写不进去**：`cargo` 报 `failed to open .../target/release/.cargo-lock` 与 `.fingerprint/.../lib-easyflowhub_lib` 的 `os error 5`（拒绝访问），即使关掉沙箱也一样——目录允许**新建**文件，不允许**原地改**已有文件。所以本批用：

```bash
cd easyflowhub-app/src-tauri
CARGO_TARGET_DIR=/tmp/tauri-target cargo build --release
```

代价是冷启动要重编 `aws-lc-sys`（rustls 后端，上千个 C 文件、单线程，实测约 20 分钟；不是死锁，`build-script-main.exe` 在跑 `cl.exe`）。增量重建约 2 分钟。**另有一处必须注意**：`E:` 盘上那份 11-25 00:16 的 `easyflowhub.exe` 早于接管代码（`shell.rs` 18:06 才提交），拿它跑这道门禁量到的是旧行为（常驻 6 个 webview、`Ctrl+Alt+Space` 还会建窗口）——**先看时间戳，别拿旧二进制下结论**。

## 8. 没做的

- **Manager 里的「新建快速笔记」仍然拒绝**（不是转发）。原生壳已有单实例互斥，这条可以改成「调用 `easyflowhub-native.exe --note`」，属于 T-Gate 1 的下一项。
- **两个托盘图标**还在。Phase 2 把 Manager 迁进原生后删 Tauri 那个。
- 门禁**不进** `run-smoke.ps1` 的默认步骤：它需要独占会话（单实例互斥按 bundle identifier，和用户那份共用），必须显式 `-TauriResident` 才跑，否则会把正在用程序的人踢掉。
