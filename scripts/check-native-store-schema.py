"""Gate: the store's schema version is measured, and the ladder ends where it says.

    python scripts/check-native-store-schema.py --exe native/target/release/easyflowhub-native.exe

Why a version number at all, and why it is not `PRAGMA user_version`:

`hub-core::store` shares `notes.db` with the Tauri build, which migrates by probing
for columns. There is no version counter in that file (`user_version = 0`), and the
S3 gate asserts it stays that way. So the version this gate checks is *derived*: a
ladder of rungs (`store/schema.rs`), each with a probe, and `version` is the highest
rung that is actually present. That makes it checkable from the outside with nothing
but SQLite, which is exactly what this script does.

Four scenarios, each in its own directory, each a separate process:

    1  fresh         an empty directory becomes a complete schema (version 6/6)
    2  legacy        a 0.1.x file (no tags, no deleted_at, old Inbox CHECK, no
                     source_path) is brought to the same columns, keeping its data
    3  ahead         a newer build's column is reported, not deleted, and an index
                     that went missing is rebuilt
    4  no-op         opening a current file twice writes nothing (byte-identical)

Every scenario also asserts `user_version` is still 0: the native side may not take
over that field, because the Tauri build would then be reading a version number it
never wrote.
"""

import argparse
import hashlib
import json
import os
import shutil
import sqlite3
import subprocess
import sys
import tempfile

# The shape a fresh file must have, written out here rather than imported from the
# Rust constants: a gate that reads its expectation from the code under test cannot
# notice the code changing.
NOTES_COLUMNS = ["id", "title", "content", "tags", "created_at", "updated_at", "is_pinned", "deleted_at"]
INBOX_ITEM_COLUMNS = [
    "id",
    "kind",
    "title",
    "content",
    "source",
    "context",
    "captured_at",
    "archived_at",
    "converted_note_id",
    "deleted_at",
]
INBOX_ATTACHMENT_COLUMNS = [
    "id",
    "item_id",
    "kind",
    "name",
    "mime_type",
    "filename",
    "size_bytes",
    "created_at",
    "source_path",
]

SCHEMA_VERSION = 6


class Report:
    def __init__(self) -> None:
        self.results: list[tuple[str, bool, str]] = []

    def check(self, name: str, ok: bool, detail: str) -> bool:
        self.results.append((name, bool(ok), detail))
        print(f"{'PASS' if ok else 'FAIL'}  {name}: {detail}")
        return bool(ok)

    def finish(self) -> int:
        failed = [name for name, ok, _ in self.results if not ok]
        print()
        print(f"{len(self.results) - len(failed)}/{len(self.results)} checks passed")
        if failed:
            print("failed: " + ", ".join(failed))
            print("RESULT: FAIL")
            return 1
        print("RESULT: PASS")
        return 0


def sha256(path: str) -> str:
    digest = hashlib.sha256()
    with open(path, "rb") as handle:
        for chunk in iter(lambda: handle.read(1 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest()


def columns(db: str, table: str) -> list:
    with sqlite3.connect(db) as conn:
        return [row[1] for row in conn.execute(f"PRAGMA table_info({table})")]


def scalar(db: str, sql: str):
    with sqlite3.connect(db) as conn:
        row = conn.execute(sql).fetchone()
        return row[0] if row else None


def run(exe: str, root: str) -> tuple:
    """One `--store-bench` process. It is read-only; the migration happens in `open`."""
    completed = subprocess.run(
        [exe, "--store-bench", "--store-root", root],
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
        timeout=120,
    )
    text = (completed.stdout or "") + (completed.stderr or "")
    payload = None
    marker = "---STORE-JSON---"
    if marker in text:
        tail = text.split(marker, 1)[1].strip()
        try:
            payload = json.loads(tail)
        except json.JSONDecodeError:
            payload = None
    return completed.returncode, payload, text


def seed_legacy(db: str) -> None:
    """The 0.1.x shape, taken from the migration tests upstream.

    Three things are deliberately old: `notes` has no `tags`/`deleted_at`, the Inbox
    tables carry the pre-audio CHECK constraints, and `inbox_attachments` has no
    `source_path`.
    """
    with sqlite3.connect(db) as conn:
        conn.executescript(
            """
            CREATE TABLE notes (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL DEFAULT '',
                content TEXT NOT NULL DEFAULT '',
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                is_pinned INTEGER NOT NULL DEFAULT 0
            );
            INSERT INTO notes VALUES ('old', 'Old note', 'preserved body', '2026-01-01', '2026-01-02', 0);

            CREATE TABLE inbox_items (
                id TEXT PRIMARY KEY,
                kind TEXT NOT NULL CHECK (kind IN ('text', 'link', 'image', 'file')),
                title TEXT NOT NULL DEFAULT '', content TEXT NOT NULL DEFAULT '',
                source TEXT NOT NULL DEFAULT 'manager', context TEXT NOT NULL DEFAULT '',
                captured_at TEXT NOT NULL, archived_at TEXT DEFAULT NULL,
                converted_note_id TEXT DEFAULT NULL
            );
            CREATE TABLE inbox_attachments (
                id TEXT PRIMARY KEY,
                item_id TEXT NOT NULL REFERENCES inbox_items(id) ON DELETE CASCADE,
                kind TEXT NOT NULL CHECK (kind IN ('image', 'file')),
                name TEXT NOT NULL, mime_type TEXT NOT NULL, filename TEXT NOT NULL UNIQUE,
                size_bytes INTEGER NOT NULL, created_at TEXT NOT NULL
            );
            """
        )


def scenario_fresh(report: Report, exe: str, work: str) -> None:
    root = os.path.join(work, "fresh")
    os.makedirs(root, exist_ok=True)
    db = os.path.join(root, "notes.db")
    report.check("fresh: no database before the run", not os.path.exists(db), root)

    code, payload, text = run(exe, root)
    if payload is None:
        report.check("fresh: the bench produced a measurement", False, text.strip()[:400])
        return
    schema = payload["schema"]
    report.check("fresh: the run itself passed", code == 0 and payload["pass"], f"exit={code}")
    report.check(
        "fresh: the ladder ends at the version this build declares",
        schema["version"] == SCHEMA_VERSION and schema["expected"] == SCHEMA_VERSION,
        f"{schema['version']}/{schema['expected']}",
    )
    report.check("fresh: nothing missing", schema["missing"] == [], str(schema["missing"]))
    report.check("fresh: nothing ahead", schema["ahead"] == [], str(schema["ahead"]))
    report.check("fresh: notes has the declared columns", columns(db, "notes") == NOTES_COLUMNS, str(columns(db, "notes")))
    report.check(
        "fresh: inbox_attachments has source_path in the last slot",
        columns(db, "inbox_attachments") == INBOX_ATTACHMENT_COLUMNS,
        str(columns(db, "inbox_attachments")),
    )
    report.check(
        "fresh: user_version is still 0",
        scalar(db, "PRAGMA user_version") == 0,
        str(scalar(db, "PRAGMA user_version")),
    )


def scenario_legacy(report: Report, exe: str, work: str) -> None:
    root = os.path.join(work, "legacy")
    os.makedirs(root, exist_ok=True)
    db = os.path.join(root, "notes.db")
    seed_legacy(db)
    before_version = scalar(db, "PRAGMA user_version")

    code, payload, text = run(exe, root)
    if payload is None:
        report.check("legacy: the bench produced a measurement", False, text.strip()[:400])
        return
    schema = payload["schema"]
    report.check("legacy: the run itself passed", code == 0 and payload["pass"], f"exit={code}")
    report.check(
        "legacy: a 0.1.x file is brought to the current version",
        schema["version"] == SCHEMA_VERSION and schema["missing"] == [],
        f"{schema['version']}/{schema['expected']} missing={schema['missing']}",
    )
    report.check(
        "legacy: the old Inbox CHECK is reported, not rewritten here",
        schema["legacy_capture_kinds"] is True,
        f"legacy_capture_kinds={schema['legacy_capture_kinds']}",
    )
    report.check(
        "legacy: notes gained tags and deleted_at",
        sorted(columns(db, "notes")) == sorted(NOTES_COLUMNS),
        str(columns(db, "notes")),
    )
    report.check(
        "legacy: inbox_attachments gained source_path",
        columns(db, "inbox_attachments") == INBOX_ATTACHMENT_COLUMNS,
        str(columns(db, "inbox_attachments")),
    )
    report.check(
        "legacy: inbox_items gained deleted_at",
        sorted(columns(db, "inbox_items")) == sorted(INBOX_ITEM_COLUMNS),
        str(columns(db, "inbox_items")),
    )
    report.check(
        "legacy: the existing note still has its body",
        scalar(db, "SELECT content FROM notes WHERE id = 'old'") == "preserved body",
        str(scalar(db, "SELECT content FROM notes WHERE id = 'old'")),
    )
    report.check(
        "legacy: the added tags column defaults to empty",
        scalar(db, "SELECT tags FROM notes WHERE id = 'old'") == "",
        repr(scalar(db, "SELECT tags FROM notes WHERE id = 'old'")),
    )
    report.check(
        "legacy: user_version was 0 before and after",
        before_version == 0 and scalar(db, "PRAGMA user_version") == 0,
        f"{before_version} -> {scalar(db, 'PRAGMA user_version')}",
    )


def scenario_ahead(report: Report, exe: str, work: str) -> None:
    root = os.path.join(work, "ahead")
    os.makedirs(root, exist_ok=True)
    db = os.path.join(root, "notes.db")
    # A current file first, then two edits a newer build (or a bad tool) would leave:
    # a column this build never heard of, and a dropped index.
    run(exe, root)
    with sqlite3.connect(db) as conn:
        conn.execute("ALTER TABLE notes ADD COLUMN color TEXT")
        conn.execute("DROP INDEX idx_notes_deleted_at")
        conn.commit()

    code, payload, text = run(exe, root)
    if payload is None:
        report.check("ahead: the bench produced a measurement", False, text.strip()[:400])
        return
    schema = payload["schema"]
    report.check("ahead: the run itself passed", code == 0 and payload["pass"], f"exit={code}")
    report.check(
        "ahead: the unknown column is named",
        schema["ahead"] == ["notes.color"],
        str(schema["ahead"]),
    )
    report.check(
        "ahead: the unknown column is still there afterwards",
        "color" in columns(db, "notes"),
        str(columns(db, "notes")),
    )
    report.check(
        "ahead: the dropped index was rebuilt",
        scalar(db, "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name='idx_notes_deleted_at'") == 1,
        "idx_notes_deleted_at",
    )
    report.check(
        "ahead: a missing index is not reported as a missing rung any more",
        schema["version"] == SCHEMA_VERSION and schema["missing"] == [],
        f"{schema['version']}/{schema['expected']} missing={schema['missing']}",
    )
    report.check(
        "ahead: user_version is still 0",
        scalar(db, "PRAGMA user_version") == 0,
        str(scalar(db, "PRAGMA user_version")),
    )


def scenario_noop(report: Report, exe: str, work: str) -> None:
    root = os.path.join(work, "noop")
    os.makedirs(root, exist_ok=True)
    db = os.path.join(root, "notes.db")
    run(exe, root)
    before = (os.path.getsize(db), sha256(db))
    run(exe, root)
    run(exe, root)
    after = (os.path.getsize(db), sha256(db))
    report.check(
        "no-op: two more opens leave the file byte-identical",
        before == after,
        f"{before[0]} bytes {before[1][:12]} -> {after[0]} bytes {after[1][:12]}",
    )


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--exe", default=os.path.join("native", "target", "release", "easyflowhub-native.exe"))
    parser.add_argument(
        "--work",
        default=os.path.join(tempfile.gettempdir(), "store-schema-gate"),
        help="scratch directory; every scenario gets its own subdirectory",
    )
    parser.add_argument("--keep", action="store_true", help="keep the scratch databases")
    args = parser.parse_args()

    exe = os.path.abspath(args.exe)
    if not os.path.exists(exe):
        print(f"missing {exe}; build it with `cargo build --release -p app`")
        return 2

    work = os.path.abspath(args.work)
    if os.path.isdir(work) and not args.keep:
        shutil.rmtree(work, ignore_errors=True)
    os.makedirs(work, exist_ok=True)

    print(f"store schema gate: exe={exe} work={work}")
    report = Report()
    scenario_fresh(report, exe, work)
    scenario_legacy(report, exe, work)
    scenario_ahead(report, exe, work)
    scenario_noop(report, exe, work)

    if not args.keep:
        shutil.rmtree(work, ignore_errors=True)
    return report.finish()


if __name__ == "__main__":
    sys.exit(main())
