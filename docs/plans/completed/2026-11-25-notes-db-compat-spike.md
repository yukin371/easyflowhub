# S3 `notes.db` 数据兼容 Spike（设计 / 验收标准）

> 状态：`completed`（S3 数据兼容 spike 通过：39 项门禁检查；4 项未测，见 §9.2）
>
> 上游：[../active/2026-11-24-native-runtime-redesign.md](../active/2026-11-24-native-runtime-redesign.md) §5.7（分层与持久化归属）、§6（S3 判据）
> 决策：[../../decisions/ADR-0009-windows-native-runtime.md](../../decisions/ADR-0009-windows-native-runtime.md)
> 门禁：[../../ARCHITECTURE_GUARDRAILS.md](../../ARCHITECTURE_GUARDRAILS.md) 第 10 节、`scripts/check-native-architecture.mjs`
> 前序：[./2026-11-25-native-shell-spike.md](./2026-11-25-native-shell-spike.md)（S1）、[./2026-11-25-richedit-island-spike.md](./2026-11-25-richedit-island-spike.md)（S2）

## 1. 这一阶段要回答什么

主设计把 S3 的判据写成一句话：**"原生进程能否直接读写现有 `notes.db` 与附件目录？"** 通过标准是"读取全部笔记/待办/设置正确；写入后 Tauri 版本仍可读；无 schema 破坏"。

这句话看起来像"文件在就能读"，但它包含三个彼此独立的失败模式，必须分开验：

1. **读得对不对**：不是"能 `SELECT` 出 40 行"，而是每一行、每个字段（含 CJK 正文、空标题、`tags`、软删标记、时间戳）都与现有数据逐个相等，排序与过滤口径一致。
2. **写得能不能被旧版本接住**：这是双向兼容，不是单向。原生写完后，**现有 Tauri 版**（用户机器上正跑着的那个二进制）必须能读、能改、能再写。这一条不能用"我们自己的读取器读得出来"来充数——那只是自证。
3. **会不会破坏契约**：schema 一个字符都不许变，`journal_mode`、`user_version`、`page_size` 不许被顺手改掉。这类破坏不会立刻报错，而是在某个旧版本打开时变成"表不存在"或"数据库被锁"。

**本阶段不回答**：迁移器（谁负责把 v1 数据升到 v2）、云同步、附件去重与清理、全文搜索索引（FTS 表目前不存在）、`notes/` 目录下遗留 `.md` 文件的归属（无人写入，属历史残留）。

## 2. 范围

**做：**

- 在 `hub-core::store` 落地真实读写（设计 §5.7 规则 3 已指定位置，**不新增 crate**）
- 读：笔记全量 + 单条、软删过滤、排序、设置键值、`待办箱` 笔记定位
- 写：新建 / 更新（含 `updated_at` 变化）/ 软删；附件落盘（遵守既有暂存-改名顺序）
- 双向兼容验证：native → Tauri 用**真的 Tauri 函数**（不是复制的 SQL）；Tauri → native 用 Tauri 的写函数造数据
- schema 指纹（`sqlite_master` 全文）前后逐字节比较；`integrity_check`；`journal_mode`/`user_version` 不变
- 两连接并发写的真实行为（`SQLITE_BUSY` 是被 `busy_timeout` 吸收，还是变成报错/损坏）
- `--store-bench` 自动化采样 + `scripts/measure-native-store.ps1` + 归档 `docs/releases/native-s3/`

**本阶段明确不做（并说明理由）：**

| 不做 | 理由 |
|---|---|
| 在真实用户库上写 | 实验数据不许进用户数据。全程用副本（`notes.db` 的拷贝），脚本里显式复制并校验副本哈希 |
| 改 `journal_mode` 为 WAL | 会把文件头 write version 改成 2 并留下 `-wal`/`-shm`。虽然 SQLite ≥3.7 都能读，但这是**产品行为变更**（备份脚本、文件复制、同步工具都会受影响），不该由 spike 替用户决定。并发靠 `IMMEDIATE` + `busy_timeout` 已足够（有 C1 证据） |
| 提升 `user_version` | 现库 `user_version = 0`，迁移体系是"列存在性探测"而非版本号。原生单方面打版本号会让两套迁移体系分叉，下一个人会按版本号写迁移而实际列已经在了 |
| schema 迁移器 | 属 Phase 2。本阶段只要求"跑同样的幂等迁移不出错、不改 DDL" |
| 附件去重 / 清理 / 缩略图 | 属 P3。`assets/` 现有 22389 个文件，清理策略要先有产品口径 |
| 待办表 | 待办**不是表**（见 §3），是 `待办箱` 笔记里的 Markdown 复选行。建表等于改契约 |
| 全文搜索索引 | `file_search_recent` 当前不存在（按需创建），FTS 从未落地 |

## 3. 现状事实（代码级 + 实测）

| 事实 | 数值 / 位置 | 对 S3 的含义 |
|---|---|---|
| 数据库路径 | `app_data_dir()/notes.db`（`notes.rs:84`、`settings.rs` 同） | Windows 上即 `%APPDATA%\com.yukin.easyflowhub\notes.db`；`identifier` 见 `tauri.conf.json:5` |
| 实测文件 | 143 360 B，mtime `2026-11-25 08:31` | 有真实数据可验，不必造假样本 |
| 实测行数 | `notes` **40**（软删 0）、`settings` **9**、`inbox_items` **0**、`inbox_attachments` **0** | "读取全部"是可数的，不是"看起来有内容" |
| `settings` 的 9 个键 | `quick_note`、`trash`、`editor`、`launcher_trigger`、`todo`、`translation`、`transcription`、`developer`、`autostart` | 前 8 个是 `AppSettings` 整体 JSON，`autostart` 单独写（`write_setting_json`）——**不能用一次全量写覆盖它** |
| PRAGMA 实测 | `journal_mode=delete`、`page_size=4096`、`encoding=UTF-8`、`user_version=0`、`foreign_keys=0`（连接级） | 四个值都是"不许变"的契约项；`foreign_keys` 是每连接的，不在文件里 |
| schema 演进方式 | 列存在性探测：`SELECT COUNT(*) FROM pragma_table_info('notes') WHERE name='tags'`（`notes.rs:116-158`） | 没有版本号可用；幂等迁移必须**与 Tauri 同文**，否则两套探测会打架 |
| `notes` 表实际列序 | `id,title,content,created_at,updated_at,is_pinned,tags,deleted_at` | `tags`/`deleted_at` 是后加的，物理列序与 `CREATE TABLE` 文本不同 → **不许 `INSERT INTO notes VALUES (...)`**，必须写列名 |
| 待办不是表 | `ensure_todo_inbox_with_conn`（`notes.rs:612`）：`TRIM(title) = '待办箱'` 取 `updated_at DESC, id ASC` 第一条 | 待办的"读取正确"= 能定位这条笔记并取出正文，条目由前端 parser 决定 |
| 附件命名 | `assets/{uuid}{ext}`，先写 `.{storage_key}.pending` 再 `rename`（`attachments.rs:120-165`） | 顺序即原子性；`filename` 列 **UNIQUE**，重名会撞 |
| 附件规模 | `assets/` 下 **22 389** 个文件 | 目录枚举不是免费操作，读路径不许全目录扫 |
| 写入事务 | `TransactionBehavior::Immediate`（`notes.rs:614`） | 立即拿 RESERVED 锁，避免"先读后升级写"的 `SQLITE_BUSY` 死锁；原生必须一致 |
| 锁等待 | `busy_timeout(5s)`（`notes.rs:167`） | 原生缺这条会把正常的瞬时争用变成硬失败 |
| **兼容性可以真验** | `notes.rs` 已有 `get_note_helper(&Connection,…)`、`save_note_with_conn`、`ensure_todo_inbox_with_conn` 等 `*_with_conn` 纯函数 + 单测 | "Tauri 仍可读"用**真的 Tauri 代码**验，不必复制 SQL 自证 |
| 两个独立 Cargo.lock | `native/Cargo.lock` 与 `easyflowhub-app/src-tauri/Cargo.lock` 各自锁 `libsqlite3-sys` | bundled SQLite 版本会**漂移**（任一侧 `cargo update`）。所以每次归档都记录两侧 `sqlite_version()`，漂移表现为 diff 而不是损坏 |
| 第三方读取器 | 本机 `sqlite3` CLI = **3.50.4**（另一套 build） | 用它做独立实现读取 + `integrity_check`，比自证更有说服力 |

## 4. 设计

### 4.1 位置：`hub-core::store`（D1）

设计 §5.7 规则 3 已写死"持久化只在 `hub-core::store`"，且门禁对 `hub-core` 的禁止符号是 `egui|eframe|windows|tauri|reqwest`——**不含 `rusqlite`**。所以：

- 新增 `native/crates/hub-core/src/store/`（`mod.rs` 连接与 schema、`notes.rs`、`settings.rs`、`attachments.rs`），加 `rusqlite = { version = "0.32", features = ["bundled"] }`，与 Tauri 侧**同主版本**。
- crate 数仍为 5，`ALLOWED_EDGES` 不动，门禁仍应打印 `passed (5 crates)`。
- `hub-core/Cargo.toml` 里"拒绝 rusqlite"的注释是**错的**（写着 guard 会拒，实际不拒且设计要求它在这里），顺手改掉，不让下一个人被注释误导。

`bundled` 的理由：不要求系统装 SQLite，也不受 `winsqlite3.dll` 版本影响；代价是 CI 多编译一次 C（可接受，`native-crates` job 已有 rust-cache）。

### 4.2 路径解析（D2）

- 显式 `--store-root <dir>` 优先；缺省 `%APPDATA%\com.yukin.easyflowhub`。
- **不**调 `SHGetKnownFolderPath`：`hub-core` 不许碰 `windows::`，且 Tauri 的 `app_data_dir()` 在 Windows 上就是 `%APPDATA%\{identifier}`。
- 这条是**假设**，所以必须证：bench 打印解析出的 root、`notes.db` 的字节数与 mtime，脚本断言"文件存在且大小与副本一致"。解析错了会立刻表现为"库是空的"，而不是悄悄写到一个新库里。

### 4.3 迁移与写入（D3/D4/D5）

- 打开连接后先跑与 Tauri **同文**的 `CREATE TABLE IF NOT EXISTS` + 列探测迁移，再 `busy_timeout(5s)`；迁移必须幂等，写完不做任何 DDL。
- 写事务一律 `IMMEDIATE`；写完 `PRAGMA integrity_check` 必须返回 `ok`。
- `INSERT` 必须显式列名（见 §3 列序一条）。
- 附件：`{uuid}{ext}` 命名，先 `.pending` 后 `rename`，再在同一事务里插行；失败回滚时删掉两处文件。

### 4.4 兼容性怎么证（D6）——三层，缺一不可

| 层 | 手段 | 它能证明什么 | 它不能证明什么 |
|---|---|---|---|
| L1 自读 | native 读真实库副本 | 字段解析正确 | 写入是否对旧版友好 |
| L2 旧版真代码 | `cargo test -p easyflowhub` 里的读测试，直接调 `get_note_helper` / `save_note_with_conn` | **用户机器上那份代码**能读能写 | 不覆盖"旧版建库→新版读"（由 L1 + L3 覆盖） |
| L3 独立实现 | `sqlite3` CLI（3.50.4，另一套 build）读 + `integrity_check` | 文件本身没坏、不是只有我们的读取器能读 | 业务语义 |

三层都过才写"通过"；只有 L1 过就写"仅自读通过"，不许折算成"兼容"。

### 4.5 并发（D7）

两个进程同时开是**真实场景**（Tauri 常驻 + 原生壳并存），所以要有证据而不是推理：

- 连接 A 开 `IMMEDIATE` 事务并写入但不提交 → 连接 B（另一进程）在 `busy_timeout` 内重试 → 期望：要么 B 在 A 提交后成功，要么 B 明确返回 `SQLITE_BUSY`；**任何情况下** `integrity_check` 仍为 `ok`、A 的数据在。
- 不测"长得像死锁"的场景：`IMMEDIATE` 就是为了让这种情况不发生。

## 5. 验收标准

判据只写可执行、可复算的。`n`/时长都来自本次运行的归档样本。

### A. 读

| ID | 判据 | 通过条件 |
|---|---|---|
| A1 | 全量笔记 | 读出的条数 = `SELECT COUNT(*) FROM notes WHERE deleted_at IS NULL`；**每个字段**与 `sqlite3` 同一条查询逐字节相等（`id/title/content/tags/created_at/updated_at/is_pinned`） |
| A2 | 软删过滤 | 默认列表不含 `deleted_at` 非空的行；`include_deleted` 时条数 = 表总行数 |
| A3 | 排序 | `is_pinned DESC, updated_at DESC` 与实际输出序列一致 |
| A4 | 待办 | 能定位 `TRIM(title)='待办箱'` 的笔记（`updated_at DESC, id ASC` 第一条），正文与 L1 读到的原文逐字节相等；条目数按 `- [ ]`/`- [x]` 行数报告（前端 parser 的口径） |
| A5 | 设置 | 9 个键全读出；`AppSettings` 那 8 个键的 `value` 能被 `serde_json` 反序列化成对象，`autostart` 能被反序列化成**对象且含 `enabled` 布尔**（实测形状是 `{"enabled":true}`，不是裸 `true`——见 §9.3） |
| A6 | 空库/缺列 | 对空文件、只有 `notes`（无 `tags`/`deleted_at`）的旧库：迁移后能读出数据且不报错（这是"旧版本建的库"路径） |

### B. 写

| ID | 判据 | 通过条件 |
|---|---|---|
| B1 | native 写 → 独立实现读 | 新行被 `sqlite3` CLI 查到，7 个字段相等 |
| B2 | native 写 → Tauri 读 | Tauri 的 `get_note_helper` 返回该行，字段相等 |
| B3 | native 改 → Tauri 读 | Tauri 读到新正文，且 `updated_at` 已变化 |
| B4 | native 软删 → Tauri 列表 | Tauri 的列表函数不含它，但 `get_note_helper` 仍能取到（软删语义） |
| B5 | schema 不变 | 前后 `sqlite_master` 全文（`type/name/tbl_name/sql` 排序拼接）**逐字节相同**；`journal_mode`/`user_version`/`page_size`/`encoding` 四个值相同 |
| B6 | 完整性 | 每次写后 `PRAGMA integrity_check` = `ok`（native 写后与 Tauri 写后各一次） |
| B7 | 附件 | native 落盘一个文件：`{uuid}{ext}` 命名、大小与 SHA-256 与源一致、行内 `filename` 可在 `assets/` 命中；**且**Tauri 侧能按 `filename` 找到同一文件 |

### C. 并发与工具

| ID | 判据 | 通过条件 |
|---|---|---|
| C1 | 两连接并发写 | A 持写事务期间 B 的重试被 `busy_timeout` 吸收或明确报 busy；最终 `integrity_check=ok`、A 的写入存在、B 的写入要么成功要么未写入（无半行） |
| C2 | 一条命令出结论 | `scripts/measure-native-store.ps1` 打印结构化检查项 + `RESULT: PASS`，退出码 0/1 |
| C3 | 反向 | Tauri 的 `save_note_with_conn` 写 → native 读到；覆盖"旧版写新版读" |
| C4 | 归档 | `docs/releases/native-s3/` 下留样本 JSON（含两侧 `sqlite_version()`、行数、指纹哈希），文档里的数字可复算 |

### G. 全局

- **G-1** 全程操作副本，真实用户库的字节数与 mtime 不变（脚本前后各记一次，变了就 FAIL）。
- **G-2** `check-native-architecture.mjs` 仍 `passed (5 crates)`；`cargo test --workspace` 全绿。
- **G-3** 未测项必须显式列出，不许以"应该没问题"折算成通过。

## 6. 实施顺序

- **A 批（读）**：`store` 模块 + 路径解析 + 幂等迁移 + 读查询；A1–A6。
- **B 批（写）**：插入/更新/软删 + 附件 + schema 指纹 + integrity；B1–B7，含 Tauri 侧读测试。
- **C 批（并发与工具）**：并发探针 + `measure-native-store.ps1` + 归档 + 文档回填。

## 7. 风险与预案

| 风险 | 预案 |
|---|---|
| 真实库含本地化/异常 UTF-8 | 字段比较用**字节**（`Vec<u8>`）而非 `String` 相等，避免"看着一样"掩盖编码差异 |
| native 的 bundled SQLite 比 Tauri 新，写出旧版不认的格式 | 归档里记录两侧 `sqlite_version()`；B5 的指纹比较 + L2/L3 读取是硬证据。若真出现，退回"锁死同一 `libsqlite3-sys`" |
| 附件目录 22389 文件，误做全目录扫描拖慢路径 | 读路径禁止枚举 `assets/`，只做"按 `filename` 命中"（`Path::exists`） |
| 并发测试不稳定（时序相关） | 固定"持锁 500 ms"的窗口 + 重试 3 次取最坏结果；报告里写清是**构造**的争用，不是随机撞上的 |
| 把副本测出的结论说成"用户库已验证" | 文档统一写"真实库的**副本**"，G-1 用来证明没动原库 |

## 8. 与其它阶段的关系

- 依赖 S2 的结论（文本岛的正文最终要落进 `notes.content`）。
- 为 P3（图片粘贴、Markdown）提供存储布局的验证：附件命名与暂存-改名顺序在本阶段被固定下来。
- S4（门禁自检）独立，不依赖本阶段。

## 9. 实施记录

> 状态：`implemented`（2026-11-25）→ **S3 通过**，39 项门禁检查全绿，两层兼容性证据（Tauri 真代码 + 独立 SQLite 实现）都到位；4 项未测、0 项口径放宽。
>
> 环境：本机 Win11 / 单屏 150%；live 库 `143,360 B`、`sha256 E693F564…`；native 侧 bundled SQLite **3.46.0**，独立读取器 `sqlite3` CLI **3.50.4**。
> 归档：`docs/releases/native-s3/store-windows-144dpi-20261125-094256.json`、`…-094339.json`（两次独立运行，逐项相同）。

### 9.1 验收逐条

#### A. 读（真实库副本，40 条笔记 / 9 个设置）

| ID | 结果 | 证据 |
|---|---|---|
| A1 | 通过 | 读出 **40** 条（= `SELECT COUNT(*)`）；**42 行**（含本次写入的 2 行探针）与 `sqlite3` CLI 的 `-json` 输出逐字段一致 —— `rows-identical 42`，目录三字段、正文、时间戳、`is_pinned`、`deleted_at` 全部相同 |
| A2 | 通过 | 默认列表 40 = `with_deleted` 40（live 库软删 0 条）；写入探针后 `deleted=1`，该行**不在**默认列表、仍可单条读出 |
| A3 | 通过 | `pinned_first=true`、`updated_desc=true`；两项分开报告，避免"sorted=true"掩盖是哪一半坏了 |
| A4 | 通过 | 定位到 `待办箱`（id `031ea7a3…`，`TRIM(title)` 匹配、`updated_at DESC, id ASC` 取首条），正文 46 B，`- [x]` 1 行 / `- [ ]` 0 行 |
| A5 | 通过 | 9 个键全读出，**9/9** 都能被 `serde_json` 解析；`autostart` 按**对象**取 `enabled=true`（设计时按 bool 写的，实测纠正，见 §9.3） |
| A6 | 通过 | 旧形态库（无 `tags`/`deleted_at`）迁移后原行仍在：`content='preserved'`、`tags=''`（单测 `a_legacy_table_without_tags_or_deleted_at_is_migrated_in_place`） |

#### B. 写

| ID | 结果 | 证据 |
|---|---|---|
| B1 | 通过 | native 插入的 `s3-probe-created` 被 `sqlite3` CLI（**另一套 build**）读到，标题/标签/时间/置顶四项相符 |
| B2 | 通过 | **Tauri 的 `get_note_helper`** 读出该行，正文（含 CJK、制表符、换行、emoji）、`tags='s3,probe'`、`updated_at=2026-11-25T00:00:01+00:00` 全部相符 |
| B3 | 通过 | Tauri 读到 native 之后的更新内容；Tauri 自己再写一次后 `updated_at` 变化（`2026-11-25T01:40:08…`），且 native 复读回该内容 → 双向 |
| B4 | 通过 | `list_notes_with_conn`（Tauri 真代码）不含 `s3-probe-deleted`，但 `get_note_helper` 仍能取到且 `deleted_at` 非空 |
| B5 | 通过 | `sqlite_master` 全量 dump 前后**逐字节相同**（`schema_same=true`，两份 dump 落在归档工作目录）；`journal_mode=delete`、`user_version=0`、`page_size=4096`、`encoding=UTF-8` 四项写前写后一致 |
| B6 | 通过 | native 写后、Tauri 写后各一次 `PRAGMA integrity_check` = `ok`；独立 CLI 再查一次也是 `ok` |
| B7 | 通过 | 写入 `{uuid}.bin`（1024 B，模式 `i % 251`），`size_bytes` 与文件一致，`.pending` 无残留；Tauri 按 `filename` 在同目录找到同一文件并校验了第 2 个字节 = 1 |

#### C. 并发与工具

| ID | 结果 | 证据 |
|---|---|---|
| C1 | 通过 | ①另一进程持 `IMMEDIATE` 写锁 **400 ms** 时，本进程的写被 `busy_timeout` 吸收并最终成功；②持锁 **7 s**（> 5 s 超时）时本进程**明确失败**（`database is locked`），未留半行，`integrity_check` 仍 `ok`，持锁方的 `s3-hold-row` 在 |
| C2 | 通过 | `scripts/measure-native-store.ps1` 一条命令 → **39 项结构化检查 + `RESULT: PASS`**，退出码 0；两次运行结果一致 |
| C3 | 通过 | Tauri 的 `save_note_with_conn` 写 → native 复读得到 `"Tauri 改写：\n- [ ] 反向验证"` 与新的时间戳 |
| C4 | 通过 | 样本归档 `docs/releases/native-s3/`（含两侧 SQLite 版本、行数、live 库 sha256、39 项检查、完整行 dump） |

#### G. 全局

| ID | 结果 | 证据 |
|---|---|---|
| G-1 | 通过 | live 库**字节数 / sha256 / mtime 三项写前写后完全相同**；全程只操作 `native/target/s3-gate-<stamp>/root/notes.db` 副本 |
| G-2 | 通过 | `check-native-architecture.mjs` → `passed (5 crates)`（存储落 `hub-core::store`，**没有新增 crate**）；`cargo test --workspace` **43 → 58 项**全绿；Tauri `notes::` 9 项 + 1 项 `#[ignore]` 兼容测试 |
| G-3 | 通过 | 4 项未测显式列在 §9.2，没有折算成"通过" |

### 9.2 未测（以及为什么不测）

| 未测项 | 原因 |
|---|---|
| 既有附件的批量可读性 | `assets/` 有 **22 389** 个文件，把整目录复制进工作区只为证明"文件还在"不划算。B7 验的是**契约**：新文件按既有命名与暂存顺序落盘、旧代码能按 `filename` 命中。既有文件的读取路径与本次写入完全相同（同一个 `Path::exists`），但没有逐个跑过 |
| 待办 parser 的完整口径 | native 只按 `- [ ]` / `- [x]` 计数；前端 `todoParser` 还处理有序列表、分割线分组、`@done:` 元数据、保留期。两边的"待办条数"因此**不可直接比较**，本阶段只证明"能定位这条笔记并原样取出正文" |
| 随机并发压力 | C1 只构造了两种确定性情形（400 ms 吸收、7 s 拒绝）。随机并发下的 `SQLITE_BUSY` 分布属 Phase 2 的双写者课题 |
| Win10 22H2 双系统 | 与 S1 同一条限制：本机只有 Win11，发布前 checklist 覆盖 |

### 9.3 四个真实发现（都是量出来的）

1. **`autostart` 是对象不是布尔**。设计阶段按 `bool` 写判据，实测值是 `{"enabled":true}`（`settings.rs::write_setting_json` 写的是结构体）。按 bool 读会永远返回 `None`，症状是"自启动设置读不到"——而它恰好是**单独写**的那个键，正是最容易被全量写覆盖的地方。判据已按实测改写，没有迁就旧假设。
2. **`notes` 表的物理列序与 `CREATE TABLE` 文本不同**：`tags`、`deleted_at` 由 `ALTER TABLE` 追加，落在末尾（`id,title,content,created_at,updated_at,is_pinned,tags,deleted_at`）。所以 `INSERT INTO notes VALUES (…)` 会把 `tags` 写进 `created_at` 槽——**静默损坏**，直到某天时间戳排序全乱才会被发现。原生侧所有语句显式列名，并有单测守着这一点。
3. **PS 5.1 的 `Start-Process` 在环境变量大小写重复时直接抛异常**（本机同时有 `HTTPS_PROXY` 与 `https_proxy`，报"字典中的关键字已添加"）。并发测试因此改用 `.NET Process API`，让 PowerShell 不去枚举环境块。
4. **PS 5.1 会把原生命令的 stderr 变成 `ErrorRecord`**，原来的 `Where-Object { $_ -is [string] }` 会把错误文本一起丢掉——于是门禁读不到 `database is locked`，把"被明确拒绝"判成"检查失败"。这属于**门禁自身的缺陷**，改法是保留 `ToString()` 文本而不是宽松判据（见 §9.4 第 2 条）。

### 9.4 门禁自身的两个缺陷（前两次运行 FAIL 的原因）

首次运行 37/39，两次 FAIL 与产品无关，都是脚本问题，**修脚本而不是放宽判据**：

| 现象 | 真正原因 | 修法 |
|---|---|---|
| `native reads the Tauri write` FAIL | 反向复读时没传 `--store-artifacts`，而完整行 dump 只落在归档文件里（控制台 JSON 有意裁掉 dump，否则 40 条 CJK 正文会淹掉 CI 日志）→ 检查读到空数组 | 反向复读也传 `--store-artifacts`，从文件读 dump |
| `long lock is refused` FAIL | 见 §9.3 第 4 条：stderr 被过滤，读到空文本；退出码其实已经非 0 | `Invoke-Native` 保留所有行的 `ToString()`，判据仍是"退出码非 0 **且** 文本含 locked/busy"，两者都不放松 |

另有一个测量口径问题：`sqlite3` 的 `.output` **不改变输出模式**，不加 `-json` 时写出的是竖线分隔文本，比较脚本会直接解析失败。改为 `-json` + `.output`（CLI 自己写 UTF-8 字节，避免 PowerShell 用控制台码页解码 CJK 造成假差异）。

### 9.5 交付物

| 类型 | 文件 |
|---|---|
| 存储实现 | `native/crates/hub-core/src/store/{mod,notes,settings,attachments}.rs`（+20 项单测） |
| 依赖 | `hub-core` 新增 `rusqlite 0.32`（`bundled`）与 `uuid`；顺带纠正了该 crate 注释里"门禁会拒绝 rusqlite"的错误说法（门禁不拒，设计 §5.7 规则 3 也要求它在这里） |
| 采样 | `native/app/src/store_bench.rs`；`--store-bench` / `--store-root` / `--store-artifacts` / `--store-write` / `--store-hold` |
| 门禁 | `scripts/measure-native-store.ps1`（L1 + L2 + L3 + 并发 + G-1，一条命令出结论） |
| 旧代码改动 | `notes.rs`：抽出 `list_notes_with_conn`（命令层只剩取连接），新增 `#[ignore]` 的双向兼容测试——这是 Tauri 侧第一条跨实现的数据兼容测试 |
| 归档 | `docs/releases/native-s3/` |

**顺带的产品级收益**：`--store-bench --store-write` **拒绝**写入真实 app data 目录（`same_path` 判定后直接退出 1）。把"实验数据不许进用户库"变成了代码约束，而不是操作纪律。
