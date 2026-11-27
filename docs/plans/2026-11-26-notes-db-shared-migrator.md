# `notes.db` 统一迁移器（Tauri 接入 `hub-core`）方案

> 状态：`implemented`（2026-11-26 批 1–3 落地；批 4 未做，见 §5）
>
> 上游：[native-runtime-redesign.md](./2026-11-24-native-runtime-redesign.md) §5.7（持久化归属 `hub-core::store`）、[ADR-0009](../decisions/ADR-0009-windows-native-runtime.md)
> 前序：[2026-11-25-notes-db-compat-spike.md](./2026-11-25-notes-db-compat-spike.md)（S3 双向兼容）、[2026-11-25-phase1-quicknote-shell.md](./2026-11-25-phase1-quicknote-shell.md) §8.17（梯子与 `SCHEMA_VERSION = 6`）
> 门禁：`scripts/check-native-store-schema.py`、`scripts/measure-native-store.ps1`、`cargo test --manifest-path easyflowhub-app/src-tauri/Cargo.toml --lib`

## 1. 结论

把 DDL 的所有权从 Tauri 移到 `hub-core::store`，Tauri 通过 path 依赖调**同一把梯子**；Tauri 侧只保留两件它才该管的事：连接策略（`busy_timeout`、`foreign_keys`）和"旧 CHECK 要不要就地重建"的选择（`Rebuild::Run`）。

这不只是删重复代码。接入之前守卫是**反向**的：`the_legacy_migrations_are_all_mirrored` 用 `include_str!` 把 Tauri 的源码读进来，要求 `hub-core` 的列清单与之相等，也就是"原生追 Tauri"。统一之后守卫换成"Tauri 的生产代码里不许出现 DDL"（`the_tauri_build_owns_no_ddl`），方向与所有权一致：越权的那一侧失败。旧守卫已随批 3 删除。

## 2. 现状事实（代码级）

| 事实 | 位置 | 含义 |
|---|---|---|
| notes 的 DDL 有两份 | `notes.rs:94`（`init_db`）与 `schema.rs:220`（`NOTES_DDL`） | 靠"同文"维持，没有机器检查 |
| inbox 的 DDL 有两份 | `inbox/schema.rs:9`（`init_db`）与 `schema.rs:247`（`INBOX_DDL`） | 同上；`source_path` 已经靠漂移守卫救过一次 |
| settings 的 DDL 有两份 | `settings.rs:227` 与 `schema.rs:236` | `IF NOT EXISTS`，无版本、无列演进 |
| 连接入口有四个 | `notes.rs:164`、`settings.rs:242`、`inbox/repository.rs:29`、`file_search.rs:637` | 每个都自己 `Connection::open` + `busy_timeout` + 各自迁移 |
| `db_path` 有两份 | `notes.rs:84`、`settings.rs:217` | 同一个 `notes.db`，两段一样的拼路径 |
| `PRAGMA foreign_keys = ON` 只在 Tauri 侧 | `inbox/schema.rs:12` | 原生侧没有 → `ON DELETE CASCADE` 只在 Tauri 进程里有效（原生 `store_attachment` 因此显式查父行） |
| 第四张表不在任何迁移里 | `file_search.rs:641`（`file_search_recent`） | 现库**没有**这张表（只读确认：4 表 14 索引，`user_version = 0`），它由第一次搜索时懒建 |
| `tauri-plugin-sql` 无 migration | `lib.rs:59`（`Builder::new().build()`） | 只是查询通道，不是第三个迁移器；但它是又一个能改 schema 的入口 |
| CHECK 重建用**按位置**的 `INSERT ... SELECT` | `inbox/schema.rs:107` | 见 §4.1，会丢后来加的列 |
| `PRAGMA foreign_keys = OFF` 写在事务里 | `inbox/schema.rs:127` | SQLite 在事务内忽略这个 pragma → 这条 `OFF` 一直没生效 |

## 3. 目标形态

### 3.1 依赖方向

```toml
# easyflowhub-app/src-tauri/Cargo.toml
# notes.db 的 schema 归 hub-core 所有：两个进程必须用同一把梯子，
# 否则只能靠"同文"提醒，而提醒不会在 CI 里失败。
hub-core = { path = "../../native/crates/hub-core" }
```

已实测可用：拿一个独立的 probe crate（自带空 `[workspace]`）对 `hub-core` 做 path 依赖，`cargo check` 通过（18s，含 `libsqlite3-sys` 编译）。两个 workspace 各自保留 `Cargo.lock`，`version.workspace = true` 的继承按 hub-core 自己的 workspace 根解析，不受影响。

这是**第一块被已发布应用依赖的 hub-core**。方向与 ADR-0009 的终局一致（原生壳最终取代 Tauri），过渡期两边必须迁同一个文件，所以依赖只能朝这个方向。

### 3.2 `hub-core` 的 API

```rust
// hub-core::store::schema
pub enum Rebuild {
    /// 撞到旧 CHECK 只报告（`Schema::legacy_capture_kinds`），不改表。原生用这个。
    Defer,
    /// 在同一次开门里重建两张 inbox 表。Tauri 启动路径用这个。
    Run,
}

pub fn init_schema(conn: &mut Connection, rebuild: Rebuild) -> rusqlite::Result<Schema>;
```

三个签名变化，每个都有理由：

- **返回 `rusqlite::Result` 而不是 `store::Result`**：这个模块只跟 SQLite 打交道。`Store::open` 里 `?` 会经既有的 `From<rusqlite::Error>` 升成 `StoreError`；Tauri 的 `SqliteResult` 直接接住。两侧都不需要错误类型互转。
- **`&mut Connection`**：迁移会改文件，重建需要事务。`&mut` 把"不许在别的事务里调我"变成编译期事实，也省掉 `unchecked_transaction`。
- **`Rebuild` 入参**：把"要不要做那件危险的事"从实现里挪到调用点，两边的裁决都留在自己代码里看得见。

### 3.3 Tauri 侧的形态

新增 `src-tauri/src/db.rs`（约 50 行），四个连接入口全部收敛到它：

```rust
pub(crate) fn connection(app: &AppHandle) -> Result<Connection, String> {
    let path = db_path(app)?;                      // 只此一份
    let mut conn = Connection::open(&path)?;
    conn.busy_timeout(BUSY_TIMEOUT)?;              // 必须在迁移之前
    conn.execute_batch("PRAGMA foreign_keys = ON;")?;  // 连接策略，不是 schema
    let report = hub_core::store::init_schema(&mut conn, Rebuild::Run)
        .map_err(|error| format!("Failed to initialize database: {error}"))?;
    if let Some(rung) = report.missing.first() {
        return Err(format!("Failed to initialize database: schema rung `{rung}` is missing after migrating"));
    }
    Ok(conn)
}
```

三个旧入口变成一行 `crate::db::connection(app)`，三个 `init_*` 变成薄壳（签名不变，测试不动）：

```rust
// notes.rs
pub(crate) fn init_db(conn: &mut Connection) -> SqliteResult<()> {
    hub_core::store::init_schema(conn, Rebuild::Run).map(|_| ())
}
```

| 删除 | 保留 |
|---|---|
| `notes.rs` 的笔记 DDL + 两段列探测（约 65 行） | `init_db` 薄壳（测试与 `ensure_todo_inbox_with_conn` 还在用） |
| `inbox/schema.rs` 的 `init_db` 体 + 三个 `migrate_*`（约 105 行） | `init_db` 薄壳、`test_memory_db` |
| `settings.rs` 的 `init_settings_table` + `db_path`（约 20 行） | `get_connection` 一行委托 |
| `notes.rs` 的 `db_path` | 迁移到 `db.rs` |

Tauri 净减约 190 行；`hub-core` 净增约 300 行（重建 + 搬过来的测试），所以 `schema.rs` 要改成目录：`schema/mod.rs`（梯子，约 600 行）+ `schema/rebuild.rs`（重建与它的测试，约 330 行），两边都在 800 行门禁内。

## 4. 接入前必须在阶梯上修掉的三处

### 4.1 重建改成"按列名交集搬运"

现在是按位置：

```sql
-- inbox/schema.rs:144
INSERT INTO inbox_attachments SELECT id, item_id, kind, name, mime_type, filename, size_bytes, created_at, NULL FROM inbox_attachments_legacy;
```

目标表 9 列，源 9 个表达式，`source_path` 被硬写成 `NULL`；`inbox_items` 那一条更直接——目标表压根没有 `deleted_at`，源表有也被丢掉，等 `migrate_deleted_at` 再把空列加回来。规律是**重建会丢所有"CHECK 那一版之后才加"的列**。现库 inbox 两张表都是 0 行，所以没有实际损失，但下一次加列就会真丢。

改成读两边的 `pragma_table_info`，取交集后生成列名明确的 `INSERT INTO t (c1, c2, …) SELECT c1, c2, … FROM t_legacy`：有就带上，没有就跳过，顺序不再有意义。顺带一个简化：重命名之后不再复制那段 DDL，直接 `execute_batch(INBOX_DDL)` 建新表——三份 DDL 里剩下的一份就是它。

### 4.2 `foreign_keys` 挪到事务外，并读取原值恢复

`PRAGMA foreign_keys` 在事务内是 no-op（SQLite 文档明确：只有在没有未提交事务时才能开关）。所以现在那条 `OFF` 只是注释，重建实际上是在 FK 开启的状态下跑的，靠"先删子表再删父表"的顺序侥幸成立；回滚测试里那条 `foreign_keys must be re-enabled after rollback`（`inbox/schema.rs:338`）也因此恒真——它证明的是"从未关掉"，不是"正确恢复"。

移动时按本意改正：进入时读 `PRAGMA foreign_keys`，事务外置 `OFF`，事务结束（成功或失败）恢复原值。删除顺序（先 `inbox_attachments_legacy` 后 `inbox_items_legacy`）保留，与 `legacy_alter_table` 下 `RENAME` 会改写外键引用这件事一起写进注释。

### 4.3 `file_search_recent` 归到一处，但不进阶梯

现库没有这张表。**不进阶梯**是刻意的：S3 门禁有一条 `db bytes match the live file`，原生开个门就往文件里加一张表会让它失败，而"打开不改文件"是这条门禁存在的原因。

做法：DDL 常量与 `ensure_file_search_recent(conn)` 放进 `hub-core`（唯一一份定义），`file_search.rs` 改成调它。懒建的位置不变（第一次搜索时才写），所以开门仍然零写入。

## 5. 决策点：还要不要写 `user_version`

统一之后"这个字段是别人的"这个理由消失了——只剩一个写者。但我仍建议**不写**，理由换成了读路径：

- 梯子量一次是十来个 `pragma_table_info`，代价可忽略；而盖号是一页写入，出现在每次开门的第一条命令上。
- 号一旦成为权威，就会有人用它跳过探测，于是"索引被删掉、开门自愈"这条性质消失。
- `user_version` 现为 0，S3 门禁有两条断言（`user_version untouched`、`user_version still 0`）。要盖就得改它们，而这些断言正是"原生不碰契约"的证据。

如果你要的是"外部工具能一眼看出文件多新"，成本更低的办法已经有了：`--store-bench` 的报告里就有 `schema.version`。**除非你明确要盖**，第 4 批不做。

## 6. 怎么证明

四层，每层都能单独失败：

| 层 | 内容 | 失败时说明 |
|---|---|---|
| 单元 | `cargo test -p hub-core`：搬过来的 4 条测试（`inbox_schema_is_additive_and_idempotent_for_existing_notes`、`legacy_inbox_tables_gain_audio_and_transcript_kinds`、`inbox_migration_rolls_back_on_failure_and_retries`、`legacy_attachments_gain_source_path_column`）+ 2 条新测试（列名交集搬运不丢 `deleted_at`；`Rebuild::Defer` 只报不做） | 重建逻辑不对 |
| 单元（Tauri） | `cargo test --manifest-path … --lib`：现有三条覆盖（幂等、两连接并发建待办箱、`reads_and_writes_a_database_produced_by_the_native_shell`）；后者加两条断言：迁移后 `schema.is_current()`、`legacy_capture_kinds == false` | 同一把梯子在同一份文件上没走完 |
| 反漂移 | `hub-core` 的 `the_tauri_build_owns_no_ddl`：递归扫 Tauri `src/**/*.rs`，**逐个 `#[cfg(test)]` 项**跳过（不是"截断文件"——`settings.rs` 的测试模块下面还有生产命令），其余不得出现 `CREATE TABLE` / `CREATE INDEX` / `CREATE UNIQUE INDEX` / `CREATE VIEW` / `CREATE TRIGGER` / `ALTER TABLE` / `DROP TABLE` / `DROP INDEX` / `PRAGMA user_version`；另断言扫到的文件数 > 20，避免路径指错时空过 | 有人把 DDL 又写回 Tauri |
| 端到端 | `python scripts/check-native-store-schema.py`（4 场景）+ `scripts/measure-native-store.ps1`（真实库副本、双向读写、`cargo test --lib -- --ignored`） | 真文件被改坏 |

本地跑 Tauri 那层要先 `go build` 出 sidecar（`tauri.conf.json` 把 `scriptmgr.exe` 当资源打包，缺文件时 build script 直接拒绝），CI 的 `native-windows` job 里已有这一步。

## 7. 分批

| 批 | 改动 | 验收 | 对用户可见行为 |
|---|---|---|---|
| 1 | `hub-core`：`Rebuild` 策略、重建搬到 `schema/rebuild.rs` 并按列名交集重写、FK pragma 修正、`&mut` + `rusqlite::Result`、`ensure_file_search_recent`；搬 4 条测试 | `cargo test -p hub-core` | 无（原生仍传 `Defer`） |
| 2 | Tauri：path 依赖、`db.rs`、三个薄壳、删三处 DDL 与两份 `db_path`、`file_search.rs` 改调 hub-core | `cargo check` + `cargo test --lib` | 无（同一套迁移，同一个结果） |
| 3 | 反漂移守卫（删旧漂移测试）+ 两条 Tauri 断言 + 文档（本方案转 `implemented`、native plan 与 README 的捷径表各改一行） | 前三层全绿 | 无 |
| 4（可选） | `user_version` 回声 | schema 门禁 6 场景、S3 门禁全绿 | 文件头多一个号 |

批 1 与批 2 各自可回退；批 1 对 Tauri 完全没有影响，这是先做它的原因。

### 7.1 实施记录（2026-11-26）

| 批 | 提交 | 证据 |
|---|---|---|
| 1 | `9e07107` | `cargo test -p hub-core` 109 项；workspace 242 项；架构门禁 `schema/mod.rs` 664 / `rebuild.rs` 301 / `store/mod.rs` 457 行（均 < 800） |
| 2 | `c958ed9` | `cargo check --locked` 通过（手写补 `Cargo.lock` 的 `hub-core` 包块，离线且 `spin 0.9.8` 已 yank，无法重生成）；`cargo test --lib` 95 passed / 1 ignored；真实库 sha256 与字节数不变 |
| 3 | 本批 | `cargo test -p hub-core` 112 项、workspace 244 项；`the_tauri_build_owns_no_ddl` 做了反向验证——往 `src/` 放一个含 `CREATE TABLE` 的探针文件，测试按 `zz_probe.rs:1: CREATE TABLE` 失败，删除后恢复绿；不做"按文件截断"，而是 `#[cfg(test)]` 项级跳过，`settings.rs`（测试模块下仍有 `pub fn update_settings`）与 `inbox/mod.rs`（`#[cfg(test)] mod schema;`）都没误报；S3 跨实现测试加的两条断言在原生写出的库上通过（39 → 41 条笔记，`version 6/6`、`legacy_capture_kinds=false`） |

E2E 门禁 `scripts/measure-native-store.ps1` 在批 3 当时曾于 `native/target` 下报 `attempt to write a readonly database`（同一份库复制到 `%TEMP%` 后立刻 `verdict=PASS`），**2026-11-27 已修**：工作目录默认落 `%TEMP%\efh-s3-gate\s3-gate-<stamp>`（`-WorkRoot` 可覆盖），并顺带跑出一次完整 `RESULT: PASS`——36 项检查全绿，含本批新加的两条断言（`tauri compat: passed`）。L2 需要写 `cargo` 的 target 目录，脚本继承 `CARGO_TARGET_DIR`，仓库不可写时指向外部目录即可。

## 8. 风险与回退

| 风险 | 处置 |
|---|---|
| hub-core 变成已发布应用的依赖，改 `store` API 要两边编译 | Tauri 只碰 `init_schema` 与 `ensure_file_search_recent` 两个符号，不碰 `Store`；这条写进 hub-core 的模块文档 |
| 两个 workspace 的 rusqlite 版本漂移 | 两处都是 `0.32` + `bundled`，Tauri 的 lock 会把 hub-core 拖到同一版本；S3 门禁同时打印两边的 `sqlite_version()` |
| 重建在首次启动变慢 | 只对旧 CHECK 库触发，一次性，现库不触发（`legacy_capture_kinds = false`） |
| 错误信息变糙（`store::Error` → `String`） | `db.rs` 自己检查 `missing` 并拼出带 rung 名的消息，保持现在 "Failed to initialize database: …" 的可读性 |
| 回退到 0.2.0 旧包 | 旧包只探测列，不看别的；批 2 只删 DDL、不改 DDL 文本，文件形状不变 |
| 每条命令多十来个 `pragma_table_info` / `sqlite_master` 读取 | 这些是内存里的 schema 读取，不碰数据页；现在 Tauri 每条命令也已经跑 7 个探测，翻倍后仍是微秒级。真要量，`measure-native-store.ps1` 的采样会给出数字 |
| 发布/构建必须能看到 `native/` | `release.yml` 与 `test.yml` 都是整仓 checkout，`working-directory: easyflowhub-app`，path 依赖解析到 `../../native/crates/hub-core`；若将来改成稀疏 checkout，构建会在这里失败而不是产出缺迁移的包 |

## 9. 不做

- **不写 `user_version`**（§5，除非你要求）。
- **不改 `journal_mode`**、不动 `page_size`/`encoding`（S3 契约项）。
- **不给原生侧开 `foreign_keys`**：那是连接策略，原生现在靠显式查父行，改成级联是另一个行为变更，单独立项。
- **不收掉 `tauri-plugin-sql`**：它没有 migration，改 schema 的能力来自命令层而不是插件；收掉它属于前端改动。
- **不动 `assets/` 与附件清理**（P3）。
