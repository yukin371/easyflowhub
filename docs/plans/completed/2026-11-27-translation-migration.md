# 翻译迁到原生：纯逻辑进 `hub-core`，只有网络落 `app`，一个请求一条线程

> 状态：`completed`
>
> 上游判据：[T-Gate 0 交接方案](../completed/2026-11-27-tauri-handover-t-gate-0.md) §6「T-Gate 0 真正关闭还缺两件」的第 1 件（后半）、§3「翻译为什么排最后」
> 归属批次：Phase 1 收口（选区捕获门禁之后，T-Gate 0 的最后一件事）
> 门禁：`python scripts/check-native-translation.py --exe <release exe>`（31 项）
> 数字来源：本机单屏 3840×2160 @125%、Win11 26300、release 构建、桩 Relay 绑 `127.0.0.1:0`

## 1. Tauri 版做过什么，这里照搬哪部分

`easyflowhub-app/src/features/translation/index.ts` 的是一条 Relay 适配器：读设置里的 `target_language` / `model` → 拼 `{base}/v1/chat/completions` 的 OpenAI 形状请求 → 发出去 → 把状态码和 body 折成一句人话。真正的 HTTP 在 `src-tauri/src/transcription.rs`，用 `reqwest::blocking`。

原生壳照搬的是**这条形状**，不是代码：同一份 `settings.json`、同一个请求体、同一组状态码映射、同样的"上游 body 永远不到用户眼前"。

## 2. 分层：`reqwest` 只能在 `app`

架构门禁的 `BANNED_DEPS` 表里，`hub-core` 禁 `reqwest`、`hub-win` 禁 rusqlite/egui。翻译的 HTTP 因此没有第二个落点，只能在 `app`：

| 层 | 文件 | 内容 | 为什么在这一层 |
|---|---|---|---|
| `hub-core` | `src/translation.rs`（405 行） | `Problem` 16 个变体（中文句子 + 日志 token）、`body()`、`parse()`、`status_problem()`、`COMPLETION_PATH`、`DEFAULT_RELAY_URL` | 纯文本进、纯文本出。16 条映射和 8 个单测都不需要起服务 |
| `app` | `src/translate.rs`（486 行） | `Transport` trait、`Relay`（`reqwest::blocking`）、`endpoint()`、`Engine` | 唯一需要 socket 的地方 |
| `app` | `src/launcher/translation.rs`（167 行） | 四个 `Shell` 方法：起请求、轮询、失败、替换、复制 | 需要面板状态与选区 |
| `hub-ui` | `src/launcher/translate.rs`（174 行） | 卡片：四行、`overflow_character: Some('…')`、只有 `Done` 才画按钮 | 只画，不知道该不该发请求 |

`Answer { status: u16, body }` 是分层的关键：网络层**不**决定"502 该说什么"，它把状态码当数字交上去，映射留在 `hub-core`：

```rust
match status { 401 | 403 => Auth, 404 => Missing, 429 => RateLimited, _ => Unavailable }
```

好处直接体现在测试上：`translate.rs` 的 10 个单测全部不需要监听端口，喂一个 `Answer { status: 502, body: "…sk-live-…" }` 就能断言"密钥不进句子"。

## 3. `native-tls` 而不是 `rustls`

Tauri 版 pin 的是 `rustls`，而 `rustls` 默认后端是 `aws-lc-rs`：一段 C + 汇编，构建时要 `cmake` 和 `nasm`。而本进程**唯一**要发的请求打在 `127.0.0.1` 上——为了一个本机回环请求引入第二套 TLS 实现和一条原生工具链，代价和收益完全不成比例。

`native-tls` 在 Windows 上就是 SChannel：https 仍然可用、信任库就是系统的、不编译任何 C。哪天 Relay 真的走公网，换的也只是 `reqwest` 的 feature 行。

## 4. 一个请求一条线程，答案是带 id 的

`blocking` 客户端**不能中途取消**。单 worker 的写法下，用户改了选区再按一次热键，新请求会排在**已废弃的旧请求**后面：面板转圈 60 秒，而答案早就不需要了。

所以 `Engine::start` 每个请求起一条命名线程 `translate`，答案带着自增的 id 回到通道里：

```rust
pub struct Engine { transport, tx, rx, next: u64, accept: u64 }
```

- `cancel()` 把 `accept` 置 0 —— 关闭面板时用；
- `poll()` 排空通道，**只留 `id == accept`** 的那一条，迟到的答案直接丢掉；
- `spawn` 失败时自己往通道塞一条 `Problem::Offline`，否则面板会永远转圈。

线程是"便宜的、会被丢弃的"，这在这个场景里恰好是对的：请求的代价在网络，不在线程。

## 5. 设置块：字段名必须和 Tauri 对齐

两个壳共用 `settings.json`。原生壳如果把自己的字段命名成 `target_lang`、`engine_model`，那么用户在任一侧保存设置时，另一侧的那块会被**整块抹掉**（保存是整块序列化的）。所以 `TranslationSettings` 的字段叫 `target_language` / `model`，与 Tauri 逐字一致；`target_language` 默认 `"zh-CN"`（BCP-47 标签，不用"中文"这种语言名，否则 Relay 侧无法匹配）。

只加了一个 Tauri 没有的字段 `relay_url`：Tauri 从 sidecar 握手拿到地址，原生壳没有 sidecar，而且门禁要指向自己起的桩服务器。

单测盯着这条：只写两个键的档案读回来 `relay_url` 仍是默认值；保存一次之后 `"translation"` 整块还在。

## 6. URL 归一化：一个函数吃掉七种写法

用户会把 base 填成 `http://127.0.0.1:8787`、`…/v1`、`…/relay/v1`、带尾斜杠、带 `/v1/chat/completions` 的完整地址。`endpoint()` 全部折成同一个结果：

```rust
let path = url.path().trim_end_matches('/');
let root = path.strip_suffix("/v1").unwrap_or(path);   // 只剥一层，不猜
url.set_path(&format!("{root}{COMPLETION_PATH}"));
```

拒绝的东西同样明确：非 `http(s)`、没有 host、带 `user:pass@`、带 `?query`、带 `#fragment` —— 每一种都对应一个真实的粘贴事故（把带 token 的 URL 粘进来，然后 token 出现在日志里）。

**顺序也是语义**：`run()` 先 `body()` 再 `endpoint()`。空选区时该说"没选文字"，而不是"URL 坏了"——用户没选字，跟他的 Relay 地址没关系。

## 7. 门禁逼出来的第二个真缺陷：粘贴跑在 UI 线程上

第一次跑 `check-native-translation.py`，"替换选区"这条失败，而且失败得很怪：笔记里出现的是 `sentinel-before-translate` —— **上一份剪贴板内容**。

原因不在翻译，在 `replace()` 跑在哪条线程上。粘贴是一次按键，目标窗口要在**自己的消息循环**里处理它；而原生壳的笔记窗口和面板**同一条 UI 线程**。`replace()` 在 UI 线程里阻塞等待 60 ms，目标根本没机会跑，而此时剪贴板已经被恢复成原内容了——于是粘贴发生时，剪贴板里是那份哨兵。

修法是把粘贴挪出 UI 线程：

```rust
let target = self.handle.previous();
self.dismiss("translate:replace");     // 先收面板，再粘
std::thread::Builder::new()
    .name("translate-replace".to_string())
    .spawn(move || { /* hub_win::selection::replace(target, &text) */ });
```

失败路径也在这里：`replace` 报错时把译文写进剪贴板（`copied=true` 记在日志里），用户至少能自己粘。

**这条只有原生壳会有。** Tauri 版里面板是 WebView、目标是别的进程，粘贴天然跨线程；同线程在这里是这个壳的结构（一个 eframe 循环管所有窗口），所以它必须自己解决。

## 8. 门禁：三个场景，一个桩服务器

`Stub` 用 `ThreadingHTTPServer` 绑 `127.0.0.1:0`，三种模式：`ok`（回一段固定译文）、`denied`（401）、`broken`（502，body 里塞 `SECRET = "sk-live-gate-4f2a"`）。

| 场景 | 断言的东西 |
|---|---|
| `success` | 捕获 43 字 → 请求形状（method / path / headers / body 共 11 项）→ 点"替换选区" → 笔记变成译文 |
| `refused` | 502 → `provider_unavailable`；日志里**找不到** `SECRET`；失败卡片**没有**"替换选区"按钮；笔记**没被动过** |
| `nothing` | `empty_input` + `stage=preflight`；桩服务器**零请求** |

第二行的第三条值得单说：它证明"上游 body 永不进日志"不是靠"我们记得不打印"，而是靠映射发生在 `hub-core` 里、`app` 只拿数字。

门禁还管住了自己的一个假通过：`translation_layout(shell, after=finish)` 只找 **finish 行之后**的 layout。否则"失败时没有替换按钮"会在**运行中卡片**（本来也没按钮）上通过。

## 9. 顺带的两件事

**`Cargo.lock` 在 `E:` 盘写不进去**（`os error 5`），而它被 git 跟踪。绕法是把 workspace 拷到 `/tmp/lk`、`mv` 走旧 lock 让 cargo 增量解析、`diff` 确认零删改、`mv` 回来、`cargo check --locked`。284 → 323 个 package，**零删除**。

**`app/src/launcher.rs` 874 行破 800 硬顶**，把四个翻译方法摘成 `app/src/launcher/translation.rs` 的 `impl Shell`（子模块看得见私有字段），回到 760。

## 9.1 回归时发现的一处既有门禁缺陷（不是本批引入）

跑 `check-native-font.py` 报 `the mapped font costs the process almost nothing: mapped - none = 6.9 MB (<= 6.0)`。用**上一批提交**构建的二进制复现，得到完全相同的 6.8 MB —— 与本批无关。

判据错在名字和尺子上。`none` 那次跑的是 `--no-font`，**没有字形、只有豆腐块**，所以它一个 glyph 页都不付；`mapped − none` 量的是"把面板上的这些字画出来要多少内存"（字形缓存 + 0.5 MB 图集），不是"映射有没有被计费"。6.0 这个阈值是在 spike 面板（一个标题 + 一列条目）上标定的，而主题 token 把面板做成了真界面（六张卡、chip、页脚、笔记行）之后，这笔账涨到了 6.7–6.9 MB。

**"19.7 MB 字库没进 commit" 本来就不由这一条回答**，它由 `heap − mapped` 回答：两次画的是同一批字形（像素差 0.00%），只有加载路径不同，实测 37.6 MB。所以改成：阈值 12 MB（远低于字库大小——真被计费会跳到 ~26 MB），判据改成它实际量到的东西，并在注释里写清两把尺子各自回答什么。

**顺带记一笔真数字**：面板 commit 从 C1 的 91.5 MB 涨到 **98.6 MB**（+7.1，私有用工作集 56.4 → 62.3），100 MB 这条线只剩 1.4 MB 余量。这是面板从 spike 变成真界面的代价，记账但不阻塞；下次给面板加内容之前应该先看一眼这个数。

## 10. 证据

| 项 | 结果 |
|---|---|
| `check-native-translation.py` | **31/31** |
| 捕获 43 汉字 | `chars=43 ms=18.4`（预算 45） |
| 请求往返（桩服务器） | `3 ms` |
| 替换后笔记 | `敏捷的棕色狐狸跃过懒狗。` |
| 502 的 body | 密钥 `sk-live-gate-4f2a` 在日志中出现 **0** 次 |
| 空选区 | 桩服务器收到 **0** 个请求 |
| `app::translate` 单测 | **10 项**（URL 七种形状、八种非法、状态码映射、200 非 completion、不重试、空选区不触网、缺 model 先于 URL、被取代的答案被丢、cancel 后连最新也丢） |
| `hub-core::translation` 单测 | **8 项** |
| `hub-core::settings` 单测 | **3 项**（默认值、只写两个键不丢 `relay_url`、保存后整块还在） |
| 回归 | `cargo test --locked --workspace` 全绿、架构门禁通过、`check-native-launcher.py` 27/27、`check-native-theme.py` 17/17 |

## 11. 没做的

- **流式输出**。Relay 支持 SSE，但面板卡片最多四行、译文通常一两百字；流式的收益是"看着它长出来"，代价是一条增量解析路径和一个新的取消语义。
- **语言自动识别**。目标语言由设置决定，没有"中译英/英译中"的自动切换。Tauri 版也没有，这里保持一致。
- **历史与收藏**。翻译结果不进库，关面板即丢。
- **真正的公网 Relay**。本轮只验到本机回环；`native-tls` 让 https 成为可能，但没有任何一条门禁跑过真实证书链。

## 12. 变更记录

| 日期 | 变更 |
|---|---|
| 2026-11-27 | 新增 `hub-core::translation`（`Problem` 16 变体 + `body`/`parse`/`status_problem`，8 单测）、`app::translate`（`Transport`/`Relay`/`Engine`，10 单测）、`app::launcher::translation`（4 个 `Shell` 方法）、`hub-ui::launcher::translate`（卡片）；`TranslationSettings` 进 `SpikeSettings`；`reqwest` 仅 `app` 依赖且用 `native-tls`；面板新增 `Translate` / `ReplaceSelection` / `CopyTranslation` 三个动作与第六张命令卡；`replace_selection` 改在独立线程粘贴，修掉同线程导致的"粘进上一份剪贴板"；新增门禁 `check-native-translation.py`（31 项）；`app/src/launcher.rs` 874 → 760 行 |
