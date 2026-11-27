"""Phase 2 gate: parity Q24 -- an unmerged draft asks, and all three answers work.

    Q24  a draft newer than the row raises the card, with three buttons on screen
    Q24  the card is modal: the strip's `•••` does not open the tools panel
    Q24  "恢复备份版本" puts the draft in the row and deletes the file
    Q24  "合并到当前内容" appends the draft under `--- 备份内容 ---`, tags unioned
    Q24  "使用当前版本" leaves the row alone and deletes the file
    Q24  once answered, the next open does not ask again
    Q24  a draft that matches the row, or is older than it, is deleted silently
    Q24  typing still writes a draft, so the prompt has something to read

The draft is a file this script writes by hand, in the shipped `BackupDraft`
shape (`title`/`content`/`tags`/`timestamp`). That is the point of the gate: it
creates exactly the situation the shipped build creates after a crash, without
needing to crash anything.

## Why the coordinates come from the log

`recover_rect` publishes the three button rectangles from the same `Shown` value
that painted them. Hard-coding them here would make the gate fail whenever the
card's padding changed, for a reason unrelated to the behaviour under test -- and
worse, it would still pass if the buttons moved somewhere unreachable, because a
click on empty space produces no log line either.

## Why every answer is a fresh process

The row is rewritten between runs, so each scenario starts from a known document.
Restarting also proves the part that only a restart can: that the answer was
persisted, not just displayed.

Usage:
    python scripts/check-native-recover.py --exe native/target/release/easyflowhub-native.exe
"""

import argparse
import ctypes
import datetime
import json
import os
import re
import shutil
import sqlite3
import subprocess
import sys
import tempfile
import time

try:
    user32 = ctypes.WinDLL("user32", use_last_error=True)
except (ImportError, OSError, AttributeError):  # not on Windows
    print("RESULT: SKIP (this gate drives a Windows window)")
    sys.exit(0)

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "lib"))
import wininput  # noqa: E402
import winshot  # noqa: E402

NOTE_PREFIX = "easyflowhub-note"

RECT = re.compile(
    r"recover_rect card=([\d,-]+) screen=([\d,-]+) keep=([\d,-]+) "
    r"merge=([\d,-]+) restore=([\d,-]+)"
)
ISLAND_TOP = re.compile(r"island_(?:created|rect) id=0 (?:rect=)?\d+x\d+\+\d+\+(-?\d+)")

CURRENT = ("当前标题", "当前标题\n当前正文", "当前")
DRAFT = ("备份标题", "备份正文\n第二行", "备份")


def iso(seconds_ago: float) -> str:
    """`updated_at`, in the shape `clock::now_iso` writes."""
    when = datetime.datetime.now(datetime.timezone.utc) - datetime.timedelta(
        seconds=seconds_ago
    )
    return when.strftime("%Y-%m-%dT%H:%M:%S") + ".000000000+00:00"


def now_ms() -> int:
    return int(time.time() * 1000)


class Trace:
    def __init__(self, path: str) -> None:
        self.path = path

    def text(self) -> str:
        if not os.path.exists(self.path):
            return ""
        with open(self.path, encoding="utf-8", errors="replace") as handle:
            return handle.read()

    def count(self, needle: str) -> int:
        return sum(1 for line in self.text().splitlines() if needle in line)

    def later(self, needle: str, before: int, timeout: float) -> str:
        deadline = time.time() + timeout
        while time.time() < deadline:
            seen = [line for line in self.text().splitlines() if needle in line]
            if len(seen) > before:
                return seen[before]
            time.sleep(0.05)
        return ""


class Session:
    def __init__(self, exe: str, root: str, log: str) -> None:
        self.exe = exe
        self.root = root
        self.log = log
        self.trace = Trace(log)
        self.process = None

    def start(self, note_id: str) -> int:
        if os.path.exists(self.log):
            os.remove(self.log)
        self.process = subprocess.Popen(
            [
                self.exe,
                "--note",
                "--note-id",
                note_id,
                "--store-root",
                self.root,
                "--log",
                self.log,
            ],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        window = wininput.wait_for(lambda: note_window(0), 25)
        if not window:
            return 0
        wininput.wait_for(lambda: "editor_focus" in self.trace.text(), 10)
        return window

    def stop(self) -> None:
        if self.process is None:
            return
        if self.process.poll() is None:
            self.process.terminate()
            try:
                self.process.wait(timeout=10)
            except subprocess.TimeoutExpired:
                self.process.kill()
                self.process.wait(timeout=5)
        wininput.wait_for(lambda: note_window(0) == 0, 5)
        self.process = None


class Report:
    def __init__(self) -> None:
        self.results = []

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


user32.FindWindowExW.restype = ctypes.wintypes.HWND
user32.FindWindowExW.argtypes = [
    ctypes.wintypes.HWND,
    ctypes.wintypes.HWND,
    ctypes.wintypes.LPCWSTR,
    ctypes.wintypes.LPCWSTR,
]


def note_window(index: int) -> int:
    return user32.FindWindowW(None, f"{NOTE_PREFIX}#{index}")


def island(index: int) -> int:
    """The RichEdit control: `WM_CHAR` sent to the frame never reaches the text."""
    window = note_window(index)
    return user32.FindWindowExW(window, None, "RICHEDIT50W", None) if window else 0


def draft_path(root: str, note_id: str) -> str:
    return os.path.join(root, "autosave", note_id + ".json")


def write_draft(root: str, note_id: str, draft) -> None:
    dirpath = os.path.join(root, "autosave")
    os.makedirs(dirpath, exist_ok=True)
    title, content, tags = draft
    payload = {
        "title": title,
        "content": content,
        "tags": tags,
        "timestamp": now_ms(),
    }
    with open(draft_path(root, note_id), "w", encoding="utf-8") as handle:
        json.dump(payload, handle, ensure_ascii=False)


def read_row(db: str, note_id: str):
    conn = sqlite3.connect(db, timeout=5)
    try:
        row = conn.execute(
            "select title, content, tags, updated_at from notes where id = ?",
            (note_id,),
        ).fetchone()
    finally:
        conn.close()
    return row


def write_row(db: str, note_id: str, title: str, content: str, tags: str, updated: str) -> None:
    conn = sqlite3.connect(db, timeout=5)
    try:
        conn.execute(
            "update notes set title = ?, content = ?, tags = ?, updated_at = ? where id = ?",
            (title, content, tags, updated, note_id),
        )
        conn.commit()
    finally:
        conn.close()


def buttons(trace: Trace):
    """`{keep, merge, restore}` as `(x, y, w, h)` tuples in client pixels."""
    for line in reversed(trace.text().splitlines()):
        found = RECT.search(line)
        if not found:
            continue
        return {
            name: tuple(int(v) for v in found.group(index).split(","))
            for index, name in ((3, "keep"), (4, "merge"), (5, "restore"))
        }
    return {}


def press(session: Session, window: int, rect, attempts: int = 3) -> str:
    """Click the button until the answer is logged, nudged a few px per try."""
    x, y, w, h = rect
    for attempt in range(attempts):
        before = session.trace.count("note_recover_action")
        wininput.force(window)
        wininput.click_client(window, x + w / 2 + (attempt * 3), y + h / 2)
        line = session.trace.later("note_recover_action", before, 3)
        if line:
            return line
        print(f"  note: click attempt {attempt + 1} produced nothing; retrying")
    return ""


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--exe", default="native/target/release/easyflowhub-native.exe")
    parser.add_argument("--work", default=os.path.join(tempfile.gettempdir(), "p2-recover-gate"))
    args = parser.parse_args()

    exe = os.path.abspath(args.exe)
    if not os.path.exists(exe):
        print(f"missing {exe}; build it with `cargo build --release -p app`")
        return 2

    live_db = os.path.join(os.environ["APPDATA"], "com.yukin.easyflowhub", "notes.db")
    if not os.path.exists(live_db):
        print(f"missing {live_db}; run the shipped build once so there is a schema to copy")
        return 2

    work = os.path.abspath(args.work)
    root = os.path.join(work, "root")
    shutil.rmtree(work, ignore_errors=True)
    os.makedirs(root, exist_ok=True)
    db = os.path.join(root, "notes.db")
    shutil.copyfile(live_db, db)

    # A note of our own, so the assertions are on text this script knows.
    note_id = "gate-recover-0001"
    updated = iso(120)
    conn = sqlite3.connect(db, timeout=5)
    try:
        columns = {row[1] for row in conn.execute("pragma table_info(notes)")}
        extra = ", is_pinned, deleted_at" if {"is_pinned", "deleted_at"} <= columns else ""
        values = ", 0, null" if extra else ""
        conn.execute(
            "insert or replace into notes (id, title, content, tags, created_at, "
            f"updated_at{extra}) values (?, ?, ?, ?, ?, ?{values})",
            (note_id, CURRENT[0], CURRENT[1], CURRENT[2], updated, updated),
        )
        conn.commit()
    finally:
        conn.close()

    report = Report()
    session = Session(exe, root, os.path.join(work, "recover-1.log"))
    try:
        # --------------------------------------------------------------- the card
        write_draft(root, note_id, DRAFT)
        window = session.start(note_id)
        if not window:
            print("the shell never produced a note window; see " + session.log)
            return 2

        rects = wininput.wait_for(lambda: buttons(session.trace) or None, 15)
        asked = session.trace.count("ask=true")
        report.check(
            "Q24 a draft newer than the row raises the question",
            bool(rects) and asked == 1,
            f"ask lines={asked} buttons={sorted((rects or {}).keys())}",
        )
        if not rects:
            print("no recover_rect line; see " + session.log)
            return 2

        width, height = wininput.client_box(window)[2:]
        inside = all(
            0 <= x and 0 <= y and x + w <= width and y + h <= height
            for (x, y, w, h) in rects.values()
        )
        report.check(
            "Q24 the three buttons are inside the window",
            inside and len(rects) == 3,
            f"client={width}x{height} buttons={sorted(rects.items())}",
        )

        # -------------------------------------------------------------- modality
        strip = (wininput.wait_for(lambda: island_tops(session.trace.text()) or None, 10) or [0])[-1]
        opened = session.trace.count("note_tools")
        wininput.force(window)
        wininput.click_client(window, width / 2, strip / 2)
        time.sleep(0.8)
        report.check(
            "Q24 the card is modal (the strip's tools button does nothing)",
            session.trace.count("note_tools") == opened,
            f"note_tools lines {opened} -> {session.trace.count('note_tools')}",
        )

        # ----------------------------------------------------------- restore
        answer = press(session, window, rects["restore"])
        row = read_row(db, note_id)
        report.check(
            "Q24 'restore' applies the draft",
            "action=restore" in answer and row is not None and row[1] == DRAFT[1] and row[2] == DRAFT[2],
            f"{answer.strip() or 'no action'} row={(row[1], row[2]) if row else None}",
        )
        report.check(
            "Q24 'restore' deletes the draft",
            not os.path.exists(draft_path(root, note_id)),
            f"file exists={os.path.exists(draft_path(root, note_id))}",
        )
        session.stop()

        # ------------------------------------------------------------- merge
        write_row(db, note_id, CURRENT[0], CURRENT[1], CURRENT[2], iso(120))
        write_draft(root, note_id, DRAFT)
        session = Session(exe, root, os.path.join(work, "recover-2.log"))
        window = session.start(note_id)
        rects = wininput.wait_for(lambda: buttons(session.trace) or None, 15)
        answer = press(session, window, rects["merge"]) if rects else ""
        row = read_row(db, note_id)
        merged_content = "当前标题\n当前正文\n--- 备份内容 ---\n备份正文\n第二行"
        report.check(
            "Q24 'merge' appends the draft under the separator",
            "action=merge" in answer and row is not None and row[1] == merged_content,
            f"{answer.strip() or 'no action'} content={row[1]!r}" if row else "no row",
        )
        report.check(
            "Q24 'merge' unions the tags",
            row is not None and row[2].split() == ["当前", "备份"],
            f"tags={row[2]!r}" if row else "no row",
        )
        report.check(
            "Q24 'merge' deletes the draft",
            not os.path.exists(draft_path(root, note_id)),
            f"file exists={os.path.exists(draft_path(root, note_id))}",
        )

        # -------------------------------------------------------------- keep
        # Still in the same process: this is also the check that answering gives the
        # editor back, because the next step types into it.
        write_row(db, note_id, CURRENT[0], CURRENT[1], CURRENT[2], iso(120))
        session.stop()
        write_draft(root, note_id, DRAFT)
        session = Session(exe, root, os.path.join(work, "recover-3.log"))
        window = session.start(note_id)
        rects = wininput.wait_for(lambda: buttons(session.trace) or None, 15)
        before = read_row(db, note_id)
        answer = press(session, window, rects["keep"]) if rects else ""
        time.sleep(0.4)
        after = read_row(db, note_id)
        report.check(
            "Q24 'keep' leaves the row untouched",
            "action=keep" in answer and after == before,
            f"{answer.strip() or 'no action'} updated_at={after[3] if after else None}",
        )
        report.check(
            "Q24 'keep' deletes the draft",
            not os.path.exists(draft_path(root, note_id)),
            f"file exists={os.path.exists(draft_path(root, note_id))}",
        )

        # ------------------------------------------- the editor comes back
        control = wininput.wait_for(lambda: island(0) or None, 5)
        report.check(
            "Q24 answering gives the text control back",
            bool(control) and bool(user32.IsWindowVisible(control)),
            f"island={control} visible={bool(control) and bool(user32.IsWindowVisible(control))}",
        )

        # ------------------------------------------- the same question, no repeat
        # The document now has an unsaved line; let the autosave write it before
        # closing, or the next open would (correctly) ask about it.
        time.sleep(1.4)
        session.stop()
        session = Session(exe, root, os.path.join(work, "recover-4.log"))
        window = session.start(note_id)
        time.sleep(1.0)
        report.check(
            "Q24 once answered and saved, the next open is quiet",
            session.trace.count("ask=true") == 0
            and session.trace.count("note_recover note_id=") == 1,
            f"ask={session.trace.count('ask=true')} "
            f"lines={session.trace.count('note_recover note_id=')}",
        )
        session.stop()

        # ------------------------------------------------- a stale draft is dropped
        write_row(db, note_id, CURRENT[0], CURRENT[1], CURRENT[2], iso(10))
        path = draft_path(root, note_id)
        os.makedirs(os.path.dirname(path), exist_ok=True)
        with open(path, "w", encoding="utf-8") as handle:
            # 60 s older than the row above: an edit that already reached the row.
            json.dump(
                {
                    "title": DRAFT[0],
                    "content": DRAFT[1],
                    "tags": DRAFT[2],
                    "timestamp": now_ms() - 70_000,
                },
                handle,
                ensure_ascii=False,
            )
        session = Session(exe, root, os.path.join(work, "recover-5.log"))
        window = session.start(note_id)
        line = wininput.wait_for(
            lambda: next(
                (l for l in session.trace.text().splitlines() if "note_recover" in l), None
            ),
            10,
        )
        report.check(
            "Q24 a draft older than the row is deleted without asking",
            "removed=older" in (line or "") and not os.path.exists(path),
            f"{line.strip() if line else 'no line'} file={os.path.exists(path)}",
        )
        session.stop()

        # ------------------------------------- a draft equal to the row is dropped
        display = "当前标题\n当前正文\n# 当前"
        with open(path, "w", encoding="utf-8") as handle:
            json.dump(
                {
                    "title": CURRENT[0],
                    "content": "当前标题\n当前正文",
                    "tags": CURRENT[2],
                    "timestamp": now_ms(),
                },
                handle,
                ensure_ascii=False,
            )
        session = Session(exe, root, os.path.join(work, "recover-6.log"))
        window = session.start(note_id)
        line = wininput.wait_for(
            lambda: next(
                (l for l in session.trace.text().splitlines() if "note_recover" in l), None
            ),
            10,
        )
        report.check(
            "Q24 a draft identical to the row is deleted without asking",
            "removed=identical" in (line or "") and not os.path.exists(path),
            f"{line.strip() if line else 'no line'} file={os.path.exists(path)} "
            f"(display={display!r})",
        )
        # ------------------------------------------------- typing writes a draft
        # A fresh process with no draft: open, type, and the file has to appear
        # before the 900 ms save -- the prompt's other half, in the window a user
        # actually types in.
        session.stop()
        session = Session(exe, root, os.path.join(work, "recover-7.log"))
        window = session.start(note_id)
        control = wininput.wait_for(lambda: island(0) or None, 5)

        def landed():
            """The draft exists and holds the text -- not just exists.

            Checking only for the file is how this check passed while the draft held
            the row's own text: `stash` can write more than once, so the first file
            is not proof that a keystroke arrived.
            """
            path = draft_path(root, note_id)
            if not os.path.exists(path):
                return None
            try:
                with open(path, encoding="utf-8") as handle:
                    payload = json.load(handle)
            except (OSError, ValueError):
                return None
            body = payload.get("content", "") + "\n" + payload.get("tags", "")
            return payload if "再写一行" in body else None

        # Two pieces with a pause between them: three `WM_CHAR`s in a row lost the
        # last one in three runs out of four (measured), while two pairs landed.
        written = None
        for attempt in range(4):
            wininput.force(window)
            wininput.type_into_island(control, "再写")
            time.sleep(0.15)
            wininput.type_into_island(control, "一行")
            written = wininput.wait_for(landed, 1.5)
            if written:
                break
            print(f"  note: keystroke attempt {attempt + 1} produced no draft; retrying")
        draft = written or {}
        # The caret now parks at the end of the *body*, before the trailing `# 当前`
        # line, so the text has to arrive in `content` and leave `tags` alone. This
        # used to be either-or -- "it is in the draft, which is what this check is
        # about" -- and that tolerance is how the caret spent a release inside the
        # tag line. The exact placement is `check-native-notes.py`'s Q-L.
        report.check(
            "Q24 a keystroke still writes a draft",
            bool(written) and "再写一行" in draft.get("content", ""),
            f"file={bool(written)} content={draft.get('content', '')!r} "
            f"tags={draft.get('tags', '')!r}",
        )
        report.check(
            "Q24 the draft kept the tags and put the text above them",
            written and draft.get("tags", "") == CURRENT[2],
            f"tags={draft.get('tags', '')!r} (expected {CURRENT[2]!r})",
        )
    finally:
        session.stop()

    return report.finish()


def island_tops(text: str) -> list:
    return [int(match.group(1)) for match in ISLAND_TOP.finditer(text)]


if __name__ == "__main__":
    sys.exit(main())
