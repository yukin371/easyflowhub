# S4 架构门禁 Spike（设计 / 验收标准）

> 状态：`implemented`（2026-11-25 设计 → A/B/C 三批实施 → 通过，23 项自检 + 8 次变异；4 项未测，见 §9.4）
>
> 上游：[native-runtime-redesign.md](./2026-11-24-native-runtime-redesign.md) §4.3（S4 判据）、§5.7（分层规则）
> 决策：[ADR-0009-windows-native-runtime.md](../decisions/ADR-0009-windows-native-runtime.md)
> 门禁：[ARCHITECTURE_GUARDRAILS.md](../ARCHITECTURE_GUARDRAILS.md) 第 10 节、`scripts/check-native-architecture.mjs`
> 前序：[S1](./2026-11-25-native-shell-spike.md)、[S2](./2026-11-25-richedit-island-spike.md)、[S3](./2026-11-25-notes-db-compat-spike.md)

## 1. 这一阶段要回答什么

主设计把 S4 的判据写成一句话：**"分层规则能否在 CI 中自动拦住越界依赖？"** 通过标准是"`check-native-architecture.mjs` 对 4 类故意违规全部报错，正常代码通过"。

这句话有一个陷阱。「正常代码通过」是**已经在发生的事实**——从 S1 起 CI 每次都是绿的。也就是说，在这个 spike 之前，门禁的**每一条规则都可以是空实现**，而 CI 依然全绿。绿不能证明规则有效，只能证明规则没有误报。

所以 S4 的实质不是"让门禁跑起来"，而是**让门禁失败一次**：构造只违反一条规则的代码树，确认它红；再故意把规则改坏，确认自检本身也红。两次都成立，"门禁有效"才算有证据。

**本阶段不回答**：门禁的规则集是否完整（只证明了这 4 类 + 3 个新发现的绕过方式会被拦）、clippy / rustfmt 级别的代码质量、跨 crate 的 API 兼容性、CI runner 的缓存行为。

## 2. 范围

**做：**

- 把判断逻辑从 CI 脚本里抽成 `analyzeNativeArchitecture(nativeRoot)`，使其能被指向任意目录树
- 用 `os.tmpdir()` 下的临时工作区写 fixture：每个用例只改基线的一处
- 覆盖 4 类违规 + 未登记 crate + 两条 skip 分支 + 边界（正好 800 行）+ `target/` 不扫描 + 若干"不该报"的反向用例
- 用变异测试证明自检有牙齿（改坏规则 → 自检必须红）
- 接入 `.github/workflows/test.yml` 的 `architecture-gates` job 与 `scripts/run-smoke.ps1`

**本阶段明确不做（并说明理由）：**

| 不做 | 理由 |
|---|---|
| 剥离注释后再扫描 | 正确剥注释要处理 `r#"…"#`、`'\''`、嵌套块注释；剥离器自己出错会**漏报**，而漏报比误报危险。宁可误报——错误信息带文件名，改注释成本很低。该限制已写成可执行用例（自检 #11） |
| 引入测试框架（vitest / jest） | 仓库没有 `package.json`，`node --test` 是 Node 22 内置能力，零依赖零安装。为 23 个用例引入整套工具链不划算 |
| 检查 `[features]` / `[patch]` 里的依赖 | 这两处不产生边，只影响解析结果；真要绕，`[dependencies]` 已经够了 |
| 检查 `build.rs` 的越界调用 | `build.rs` 也走 `.rs` 扫描（同目录同后缀），符号规则已覆盖；但它能做的事（下载、生成代码）超出分层规则的范围 |

## 3. 设计

### 3.1 为什么必须先把判断抽出来

原脚本是**顶级执行式**的：`import` 时就 `fs.readFileSync(repoRoot/native/…)`，然后直接 `process.exit(1)`。结果是它**结构上不可测**——唯一能喂给它的输入就是那棵正确的树。既不能造违规，也不能造"该通过但看起来像违规"的边缘情况。

改法：判断下沉到 `scripts/lib/native-arch-guard.mjs`，签名 `analyzeNativeArchitecture(nativeRoot) → { skipped, reason, crates, unknownCrates, edgeViolations, cycles, symbolViolations, dependencyViolations, sizeViolations, sections, failed }`；CLI 只负责打印与退出码。

**这一步是重构，不是改规则**，并且被证明而不是被声称：成功路径与失败路径（在 `hub-win` 里种一个 `use egui::Context;`）都拿 HEAD 里的旧脚本对同一棵树跑了 diff，输出**逐字节相同**，退出码相同。

### 3.2 fixture 策略：基线 + 单点变异

一个"随便写点错代码"的 fixture 只能证明"有东西报错"。要证明**是哪条规则**报的错，必须让每次只有一处不同：

- `BASELINE` 是一棵满足全部规则的 5 crate 小树（app / hub-ui / hub-editor / hub-win / hub-core）
- 每个违规用例 `clone()` 基线，只改一处（加一条 dep、改一个文件内容、换一个 deps 数组）
- 断言精确到条目文本（`hub-win -> hub-core: not allowed (allowed: none)`），不是只看 `failed === true`

另外有一个"反事实"用例：直接对**真实的 `native/`** 跑一次并要求通过。否则 fixture 写歪了、和真实树脱节，自检会变成一场自我满足的演出。

### 3.3 反向用例和正向用例一样重要

门禁太严的后果不是"更安全"，而是**被加豁免**——一旦有人为了过门禁往规则里塞 `except`，整道门就作废了。所以以下四种"看起来危险、实际合法"的写法必须有专门的用例守着：

- `egui` 出现在 `hub-ui`（它就该在那里）
- `windows_sys::` 出现在 `hub-editor` / `hub-win`（它们的本职工作）
- 文件正好 800 行（等于上限应当通过）
- `target/` 下的生成代码（构建产物不该让门禁红）

### 3.4 变异测试：怎么证明自检不是空转

自检全绿同样可能是空转（比如断言写成了 `assert.ok(true)`）。方法是**反向验证**：故意改坏被测规则，看对应用例是否变红。

| # | 变异 | 期望 | 实测 |
|---|---|---|---|
| 1 | `HARD_CAP` 800 → 900 | 尺寸用例红 | **第一轮 0 红（存活）** → 修测试后 2 红 |
| 2 | 关掉 `cycles.push` | 环用例红 | 1 红 |
| 3 | `hub-core` 符号规则改 `__mutated__` | 泄漏用例红 | 2 红 |
| 4 | 边检查恒允许（`if (false)`） | 两条边用例红 | 2 红 |
| 5 | manifest 依赖检查恒通过 | 两条 manifest 用例红 | 2 红 |
| 6 | `BANNED_DEPS` 删掉 `windows-sys` | `windows_sys` 两条红 | 2 红 |
| 7 | `tableDep` 退回只认行首 | 表形式用例红 | **第一轮 0 红（存活）** → 补用例后 1 红 |
| 8 | 不跳过 `[workspace.*]` 段 | 反向用例红 | 1 红 |

变异 1 和 7 的存活，是**自检自身的两个缺陷**，见 §9.2。

## 4. 规则与判据

门禁输出 6 个 section（原 5 个，本次新增 `forbidden dependencies`）：

| section | 规则 | 判据 |
|---|---|---|
| unknown crates | 新 crate 必须登记到 `ALLOWED_EDGES` | 目录里有 manifest 但表里没有 → 报错并提示去改哪个文件 |
| dependency edges | 内部依赖边必须在矩阵内 | 越界边逐条报，附允许列表 |
| dependency cycles | 内部依赖不得成环 | 环必须闭合（首尾同名），且只报一次 |
| layer symbol leaks | 源码不得引用越层 crate | `crate::` 路径 或 行首 `use crate` 两种形态 |
| forbidden dependencies | manifest 不得声明越层 crate | 含 `[dependencies]`、`[dependencies.foo]`、`[target.'cfg(…).dependencies]`、`[target.'cfg(…).dependencies.foo]` |
| file-size debt | 单文件 ≤ 800 行 | 等于 800 通过，801 报错；`target/` 不扫描 |

"禁哪些 crate"合成**一张表** `BANNED_DEPS`，符号扫描与 manifest 检查都从它派生——两张独立的表迟早会漂移，而漂移的方向一定是某一侧漏掉。

## 5. 实施顺序（A/B/C）

| 批 | 内容 | 提交 |
|---|---|---|
| A | 抽 `lib/native-arch-guard.mjs`，CLI 变薄；双路径 diff 证明行为不变 | `refactor(native-guard): split layering rules from the CLI` |
| B | 23 项自检（其中 18 项覆盖原有 4 类）；变异测试暴露自检两个缺陷并修掉 | `test(native-guard): prove the gate fails on a violation` |
| C | 补自检暴露的三个真实缺口（`windows_sys`、manifest、target 表），接 CI 与 smoke | 本批 |

## 9. 判决（2026-11-25）

**通过。** 四类违规全部能报，正常代码通过；并且自检本身经过了 8 次变异验证。

### 9.1 验收

#### A. 可测性

| ID | 结果 | 证据 |
|---|---|---|
| A1 | 通过 | 抽取前后，成功路径输出 `diff` 为空；在 `hub-win` 植入 `use egui::Context;` 后，失败路径输出 `diff` 也为空，两边退出码均为 1 |
| A2 | 通过 | `analyzeNativeArchitecture(root)` 可对任意路径求值；全部用例在 `os.tmpdir()/hub-arch-*` 下运行并在 `t.after` 中清理 |
| A3 | 通过 | 真实树 `node scripts/check-native-architecture.mjs` → `passed (5 crates)`，6 个 section 全 `ok` |

#### B. 四类违规 + 边界（18 项）

| ID | 结果 | 用例 |
|---|---|---|
| B1 | 通过 | 越界边（`hub-win -> hub-core`）报 `not allowed (allowed: none)` |
| B2 | 通过 | `[dependencies.hub-core]` 表形式同样被识别 |
| B3 | 通过 | 两节点环 `hub-core <-> hub-ui` 报 1 条且首尾闭合；断言按**成员集合**而非固定起点（DFS 起点随目录顺序变） |
| B4 | 通过 | 未登记 crate `hub-extra` → 提示登记 `ALLOWED_EDGES` |
| B5 | 通过 | `hub-core` 的 `use egui::Context;` → 报 `forbidden symbol` 且带文件名 |
| B6 | 通过 | `hub-ui` 的 `use windows::Win32::…` → 报错 |
| B7 | 通过 | 801 行报错（消息含 `801 lines > hard cap 800`）；800 行通过；`target/` 下 830 行不报 |
| B8 | 通过 | 两条 skip 分支：无 `native/`、有目录但无 manifest |
| B9 | 通过 | 外部依赖（`rusqlite` 等）不会被误认成内部 crate 或边 |
| B10 | 通过 | 反向 4 项：`egui` 在 `hub-ui`、`windows_sys` 在 `hub-editor`/`hub-win`、800 行、`target/` |

#### C. 自检有效性（变异）

| ID | 结果 | 证据 |
|---|---|---|
| C1 | 通过 | 8 次变异，8 次都被杀（其中 2 次第一轮存活，见 §9.2） |
| C2 | 通过 | 23 项在本地与 CI 均全绿；`node --test` 退出码 0 |

#### G. 全局

| ID | 结果 | 证据 |
|---|---|---|
| G-1 | 通过 | `.github/workflows/test.yml` 的 `architecture-gates` job 新增一步 `Native architecture guard self-check` |
| G-2 | 通过 | `scripts/run-smoke.ps1` 新增 `Native architecture guard self-check` 步骤；PS 语法校验通过 |
| G-3 | 通过 | 本文档 + 主设计 §4.3 + `ARCHITECTURE_GUARDRAILS.md` 第 10 节 + `docs/roadmap.md` 同步 |
| G-4 | 通过 | 4 项未测显式列在 §9.4 |

### 9.2 自检自身的两个缺陷（第一轮变异存活的两次）

这两条值得单独记，因为它们是**测试写错**，而不是规则写错——而错误的测试比没有测试更危险，它会给出"已验证"的错觉。

1. **尺寸用例自证循环**。fixture 的行数写成 `fileWithLineCount(HARD_CAP + 1)`，而 `HARD_CAP` 是从被测模块导入的。把门禁改成 900，fixture 自己就变成 901 行，**仍然超限**，测试照样绿。修法：fixture 用字面量 801/800，并单加一条用例把 `HARD_CAP === 800`（设计文档里的数字）钉死。
2. **表形式没被覆盖**。`[target.'cfg(windows)'.dependencies]` 的 kv 形式走的是"section 名"正则，与 `[target…dependencies.foo]` 的表形式走的是**另一个**正则。只测 kv 形式时，把表形式那条正则改坏不会有任何用例变红。补一条表形式用例后，变异 7 才被杀。

另有一处**假设错误**被测试当场抓住：我原以为两节点环会报成 `hub-ui -> hub-editor -> hub-core -> hub-ui`（从 app 进入），实际 DFS 从 `hub-core` 先进，报出 `hub-core -> hub-ui -> hub-core`。断言改为"闭合 + 成员集合"，而不是把错误的路径文本写进测试。

### 9.3 三个真实缺口（写自检时发现，不是读规则时想到的）

这三条的共性是：**都能把被禁 crate 弄进某一层，而源码里一个 `banned::` 都不出现**——原规则只看源码符号，所以全部漏掉。

| # | 缺口 | 具体形态 | 修法 |
|---|---|---|---|
| 1 | `windows_sys` 不在 `hub-core` 的禁用模式里 | `use windows_sys::Win32::Foundation::HWND;` 在 `hub-core` 里不报错。规则写的是 `windows::`，而本项目用的是 `windows-sys`（下划线形式），`\bwindows::` 匹配不到 | 禁用表按 crate 名派生正则，`windows` 与 `windows-sys` 映射到 `windows` / `windows_sys` 两个源形态 |
| 2 | manifest 里声明但不使用 | `hub-core` 的 `Cargo.toml` 写 `egui = "*"`，源码不 `use` → 完全不报。而"声明了但没用"距离"用了"只差一行 | 新增 `forbidden dependencies` 检查，读 manifest 的依赖名 |
| 3 | `[target.'cfg(windows)'.dependencies]` 解析不到 | `parseCargoToml` 的 section 正则锚在行首（`^dependencies`），带前缀的 target 表整段被忽略——**放在 cfg 后面就等于隐身** | section 正则改成允许任意前缀（`(?:^|\.)dependencies`），同时显式跳过 `[workspace.*]`（那是共享版本表，不是本 crate 的依赖） |

三条都是在**同一个 commit** 里先写红测试、再改规则，红→绿的顺序留在提交历史里。

### 9.4 未测（以及为什么不测）

| 未测项 | 原因 |
|---|---|
| 注释内的 `crate::` 误报 | 已知且**有意保留**。剥注释的剥离器自身出错会漏报，漏报比误报危险。已写成用例 #11 钉住当前行为，错误信息带文件名以便快速改注释 |
| CI runner 上的实际耗时 | 本地 23 项约 2 s；GitHub 的 `ubuntu-latest` 未单独计时。只影响反馈速度，不影响判据 |
| 规则集的完整性 | 只证明"这 4 类 + 3 个绕过方式"会被拦。是否存在第 4 个绕过方式（例如 `cfg_attr` 条件导入、`include!` 拼接）未穷举 |
| 真实越界提交的拦截演练 | 没有真的往 `main` 提一个越界 PR 看 CI 是否红。理由是 fixture 已在同一份代码路径上验证过，且 CI 步骤与本地命令完全相同 |

### 9.5 交付物

| 类型 | 文件 |
|---|---|
| 规则模块 | `scripts/lib/native-arch-guard.mjs`（`BANNED_DEPS` 单一来源，派生出符号正则与 manifest 检查） |
| CLI | `scripts/check-native-architecture.mjs`（只格式化 + 退出码） |
| 自检 | `scripts/check-native-architecture.selfcheck.mjs`（23 项，`node:test`，零依赖） |
| CI | `.github/workflows/test.yml` → `architecture-gates` 新增一步 |
| 本地 | `scripts/run-smoke.ps1` 新增一步 |

**复现：**

```bash
node scripts/check-native-architecture.mjs            # 真实树，应 passed (5 crates)
node --test scripts/check-native-architecture.selfcheck.mjs   # 23 项，应全绿
```

**顺带的收益**：`BANNED_DEPS` 让"某一层不许碰什么"从两处分散的正则变成一张可读的表。以后新增 crate 时，除了登记 `ALLOWED_EDGES`，还必须同时回答"它禁什么"——否则 manifest 检查对它形同虚设（这是当前设计的一个已知取舍：未登记的 crate 已经被 `unknown crates` 拦下，所以不会静默通过）。
