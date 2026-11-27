# store 的 schema 版本号：算出来的，不是盖上去的（2l）

> 状态：`completed`（2026-11-26 落地；24 项门禁全过）
>
> 上游：[Phase 1 快速笔记壳](../active/2026-11-25-phase1-quicknote-shell.md) 数据层（§5）与"已知捷径"里的「`store` 没有 schema 版本号」
> 前序：[notes.db 的 DDL 只剩一份](./2026-11-26-notes-db-shared-migrator.md)（Tauri 与本侧共用同一把梯子）
> 门禁：`python scripts/check-native-store-schema.py`（24 项，四个场景各一个进程、各一个目录）


从 1d 起，"`store` 没有 schema 版本号"一直挂在"已知捷径"里，理由是现库 `user_version = 0`、单方面打号会让两套迁移分叉。这次收掉这件事，但**不是**去打那个号——三条理由，每条都能被现有门禁反驳：

1. **`user_version` 是别人的字段。** 现库是 0，S3 门禁的 `measure-native-store.ps1` 明确断言原生写之后它**仍然是 0**（`user_version untouched` / `still 0`）。往里写数字等于悄悄改契约，下一个按版本号写迁移的人会撞上一个"列早就在了"的库。
2. **只有一方维护的号是负资产。** Tauri 若加了迁移而原生侧还写着旧号，"你是最新的"这句话就是假的；这种缓存比没有更坏。
3. **要的不是数字，是"哪几级已经在文件上"。** 这个问题有可判定的答案：探测每一级自己的产物。

所以 `SCHEMA_VERSION = 6` 是"这个构建会建到第几级"，`Schema::version` 是**量**出来的：自下往上，最高一级探测通过的编号。文件上有笔记的三级、没有收件箱，答案就是 3 而不是 6——不连续就停，因为"第 5 级在、第 4 级不在"说明不了梯子爬到了哪。

## 六级梯子

| 级 | 名字 | 探测什么 | 建什么 |
|---|---|---|---|
| 1 | `notes` | `notes` 表在 | `NOTES_DDL`（表 + 三个索引） |
| 2 | `notes.tags` | `tags` 列 **且** `idx_notes_tags` | 缺列则 `ALTER`，再建索引 |
| 3 | `notes.deleted_at` | 列 **且** `idx_notes_deleted_at` | 同上 |
| 4 | `inbox` | 两张收件箱表都在 | `INBOX_DDL` |
| 5 | `inbox_items.deleted_at` | 列 **且** `idx_inbox_items_deleted_at` | 同上 |
| 6 | `inbox_attachments.source_path` | 列在 | `ALTER ... source_path` |

`init_schema` 只做一件事：自下往上，探测失败的那一级就建。`settings` 不在梯子上——它没有会演化的列，"没有设置"不等于"落后"，所以照旧无条件 `CREATE TABLE IF NOT EXISTS`。`Store::open` 在梯子没到顶时返回错误而不是交出连接：半迁移的连接会让后面每条查询都答一个没人声明过的形状，失败点会跑到一次保存里，而那时已经看不到原因。

## 顺带修掉三个真缺陷

| 缺陷 | 现象 | 现在 |
|---|---|---|
| 索引建在"加列"的 `if` 里 | 索引被别的工具删掉后永远补不回来（探测只问列在不在），`sqlite_master` 与 Tauri 建的库渐渐不同 | 每级探测同时要求列与索引，建索引的语句在 `if` 外面；单测先 `DROP INDEX` 再开门，版本从 2 回到 6 |
| `inbox_attachments.source_path` 只靠 `ALTER` 补 | 原生新建的库该列靠 `ALTER` 追加；Tauri 的基表 DDL 里就有它，而它的 CHECK 重建用**按位置**的 `INSERT ... SELECT` | 基表 DDL 里写 `source_path TEXT`，`ALTER` 只留给旧库；门禁直接断言"最后一列是 `source_path`" |
| 漂移守卫只认 `ALTER TABLE notes ADD COLUMN` | Tauri 在 `CREATE TABLE` 里加列（`source_path` 就是这么加的）、或给收件箱加列，守卫都看不见 | 守卫解析 `notes.rs` 与 `inbox/schema.rs` 两处，把基表列与 `ALTER` 列按顺序合并，与 `NOTES_COLUMNS`／`INBOX_*_COLUMNS` 逐项比较，并额外断言真实库的物理列顺序就是这些 |

## 一件刻意不做：CHECK 重建

`inbox/schema.rs::migrate_capture_kinds_schema` 是唯一没有镜像的迁移：SQLite 改不了 `CHECK`，它要 `RENAME` 两张表、重建、按位置搬数据——全产品最危险的一段 DDL。原生侧只**检测**（`Schema::legacy_capture_kinds`，从 `sqlite_master.sql` 里找 `'audio'`／`'transcript'`），不执行：一个只想来读一条笔记的进程不该重写两张表，而 Tauri 下次启动本来就会做。检测结果不算失败——`judge` 里写明了原因，免得门禁去怪错的那一方。

## 改在哪

| 位置 | 改动 |
|---|---|
| `crates/hub-core/src/store/schema.rs`（新） | `SCHEMA_VERSION`、`Schema`（`version`/`missing`/`ahead`/`legacy_capture_kinds`）、`RUNGS`（六个 `Rung`）、`init_schema`（返回 `Schema`）、`schema()`、未知列探测、CHECK 探测；5 个单测 |
| `crates/hub-core/src/store/mod.rs` | DDL 与探测整块搬进 `schema.rs`；`Store::open` 检查梯子到顶；新增 `Store::schema()`；漂移守卫重写 |
| `app/src/store_bench.rs` | 证据里加 `schema` 对象，`judge` 要求梯子到顶（`legacy_capture_kinds` 不算失败） |
| `scripts/check-native-store-schema.py`（新） | 4 个场景、24 项 |

## 门禁

`scripts/check-native-store-schema.py`，四个场景各一个进程、各一个目录：

| 场景 | 判什么 |
|---|---|
| fresh | 空目录 → `6/6`、无缺无超、`notes` 八列、`inbox_attachments` 最后一列是 `source_path` |
| legacy | 0.1.x 形状（无 `tags`/`deleted_at`、旧 CHECK、无 `source_path`）→ `6/6`；正文还在、`tags` 补成空串、`legacy_capture_kinds=true` |
| ahead | 现库加一列 `notes.color` + 删掉 `idx_notes_deleted_at` → `ahead=["notes.color"]`、`color` 不被删、索引补回、版本仍 6 |
| no-op | 对当前文件再开两次 → 字节数与 sha256 完全相同（开门不写盘） |

四个场景都断言 `user_version` 仍是 0。

## 证据

| 门禁 | 结果 |
|---|---|
| `scripts/check-native-store-schema.py` | 24/24 |
| 真实库副本（39 条笔记 / 9 个设置） | `schema version=6/6 missing=- ahead=- legacy_capture_kinds=false`，`schema unchanged=true` |
| `scripts/check-native-notes.py` | 25/25（G-2 的 schema 指纹仍是 48 个对象） |
| `scripts/check-native-pin.py` / `manager` | 11/11、19/19 |
| `cargo test --release --workspace` | 235 项（新增 5 项：梯子到顶、旧库列集合、索引重建、未知列只报不删、开门不写） |
| `scripts/check-native-architecture.mjs` | 通过（`schema.rs` 585、`store/mod.rs` 453、`store_bench.rs` 783） |

## 没做的

- **给 Tauri 侧也接上这把梯子**：那是 Phase 2 的"统一迁移器"。这版只保证原生量出来的版本与 Tauri 建出来的形状一致（漂移守卫 + 真实库副本 + 未知列报警）。
- **`PRAGMA foreign_keys = ON`**：Tauri 在 `inbox::init_db` 里设了，原生侧没设，所以 `ON DELETE CASCADE` 对原生写无效（`store_attachment` 因此显式查父行）。这是连接级行为，与版本号无关，另立一项。
