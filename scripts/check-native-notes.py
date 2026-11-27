"""Phase 2 gate: does the quick note actually persist what was typed?

What this checks, and why each step is a separate process:

    Q20  typing then going quiet writes a row (900 ms debounce)
    Q22  the row's title is the first line
    Q-S  a change no message reported is still caught (the 2 s scan)
    Q-T  the trailing tag line comes off the body and into its own column
    Q21  typing then closing at once flushes (the debounce is not a reason to lose it)
    Q23  a note that never received a keystroke leaves no row
    Q-L  reopening by id shows the body again and keeps the caret at the end of it
    G-1  the shell opened the copy of the database, not the live one
    G-2  the schema fingerprint is unchanged by a save

Each scenario gets its own process because the hotkey cannot be used to open a
second note: this machine runs the shipped build, which holds `Ctrl+Alt+N`, so
`RegisterHotKey` inside the native shell fails with 1409 and `SendInput` goes to
the other app. One process, one window, one scenario is also easier to read when
something fails -- and `Log::new` truncates the file at startup, so the log belongs
to exactly one process. Each scenario's section is captured as it finishes; the
failure dump at the end prints all of them, in order.

The control's text cannot be read from here: RichEdit does not marshal text
messages across the process boundary. `GetWindowTextW` answers with an empty
string, and `SetWindowTextW` returns success while changing nothing -- a silent
no-op, which is worse than a failure. Every "what is in the note" question is
therefore answered by a round trip through the database, which is also the thing
under test.

The run is self-contained: it launches the binary it was given, drives it with the
messages a keyboard would produce, reads the log and a copy of the database, then
stops every process it started. Nothing here touches `%APPDATA%` except to copy the
database out of it.

Usage:
    python scripts/check-native-notes.py --exe native/target/release/easyflowhub-native.exe
"""

import argparse
import hashlib
import os
import re
import shutil
import sqlite3
import subprocess
import sys
import tempfile
import time
from ctypes import wintypes

try:
    import ctypes

    user32 = ctypes.WinDLL("user32", use_last_error=True)
except (ImportError, OSError):  # not on Windows: this gate is Windows-only
    print("RESULT: SKIP (this gate drives a Windows window)")
    sys.exit(0)

# Per-monitor DPI: the shell places its window from the cursor position, and a
# virtualised coordinate would put it on a scaled rectangle we then measure wrong.
user32.SetProcessDpiAwarenessContext(ctypes.c_void_p(-4))
user32.FindWindowW.restype = wintypes.HWND
user32.FindWindowW.argtypes = [wintypes.LPCWSTR, wintypes.LPCWSTR]
user32.FindWindowExW.restype = wintypes.HWND
user32.FindWindowExW.argtypes = [wintypes.HWND, wintypes.HWND, wintypes.LPCWSTR, wintypes.LPCWSTR]
user32.SendMessageW.restype = wintypes.LPARAM
user32.SendMessageW.argtypes = [wintypes.HWND, wintypes.UINT, wintypes.WPARAM, wintypes.LPARAM]
user32.PostMessageW.restype = wintypes.BOOL
user32.PostMessageW.argtypes = [wintypes.HWND, wintypes.UINT, wintypes.WPARAM, wintypes.LPARAM]

WM_CHAR = 0x0102
WM_KEYDOWN = 0x0100
WM_KEYUP = 0x0101
VK_ESCAPE = 0x1B
VK_RETURN = 0x0D

NOTE_PREFIX = "easyflowhub-note"

# Scan period and debounce, from `app/src/save.rs`. The waits below are derived from
# these rather than hardcoded, so a tuning change cannot silently turn a scenario
# into a race.
SCAN = 2.0
AUTOSAVE = 0.9
SLACK = 0.8
# The scan can look at the document one period after a change; the write then waits
# out the debounce. This is the worst case plus slack.
SCAN_PATH = SCAN + AUTOSAVE + SLACK


def note_window(index: int) -> int:
    return user32.FindWindowW(None, f"{NOTE_PREFIX}#{index}")


def island(index: int) -> int:
    window = note_window(index)
    return user32.FindWindowExW(window, None, "RICHEDIT50W", None) if window else 0


def wait_for(predicate, timeout: float, step: float = 0.05):
    deadline = time.time() + timeout
    while time.time() < deadline:
        value = predicate()
        if value:
            return value
        time.sleep(step)
    return predicate()


def type_text(hwnd: int, text: str) -> int:
    """Type into the control as a keyboard would.

    A line break is *not* a single `WM_CHAR 0x0D`: RichEdit ignores it there and the
    whole note arrives as one paragraph -- which reads as "the title is the first
    line" passing while the body is wrong. The break needs the key event around the
    character, which is what a real Enter produces.
    """
    raw = text.encode("utf-16-le")
    sent = 0
    for offset in range(0, len(raw), 2):
        unit = int.from_bytes(raw[offset : offset + 2], "little")
        if unit == 0x0A:  # LF -> Enter
            user32.SendMessageW(hwnd, WM_KEYDOWN, VK_RETURN, 0x001C0001)
            user32.SendMessageW(hwnd, WM_CHAR, 0x0D, 1)
            user32.SendMessageW(hwnd, WM_KEYUP, VK_RETURN, 0xC01C0001)
        else:
            user32.SendMessageW(hwnd, WM_CHAR, unit, 1)
        sent += 1
    return sent


def silent_break(hwnd: int) -> int:
    """Insert a paragraph break with no `WM_CHAR` -- an edit nobody reports.

    RichEdit acts on `WM_KEYDOWN VK_RETURN` itself, so the break lands while the
    edit signal (which `hub-editor` raises on `WM_CHAR` and the text messages) is
    never raised. That is the "a signal was missed" case the 2 s scan exists for,
    and it is a real path: a keystroke can be consumed by an IME, a drag-drop, or a
    message nobody listed, and the text still has to reach the database.
    """
    return user32.SendMessageW(hwnd, WM_KEYDOWN, VK_RETURN, 0x001C0001)


def esc(hwnd: int) -> None:
    user32.PostMessageW(hwnd, WM_KEYDOWN, VK_ESCAPE, 0)


def sha256(path: str) -> str:
    digest = hashlib.sha256()
    with open(path, "rb") as handle:
        for chunk in iter(lambda: handle.read(1 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest()


def fingerprint(db: str) -> str:
    """Schema objects plus the column list: a migration that only adds a column
    would otherwise leave the `sqlite_master` text unchanged and look like no change
    at all."""
    connection = sqlite3.connect(f"file:{db}?mode=ro", uri=True)
    try:
        table = connection.execute(
            "SELECT type, name, tbl_name, COALESCE(sql, '') FROM sqlite_master ORDER BY type, name"
        ).fetchall()
        columns = connection.execute("PRAGMA table_info(notes)").fetchall()
        return "\n".join(
            ["|".join(str(part) for part in row) for row in table]
            + ["column|" + "|".join(str(part) for part in row) for row in columns]
        )
    finally:
        connection.close()


def rows(db: str) -> list:
    connection = sqlite3.connect(f"file:{db}?mode=ro", uri=True)
    try:
        return connection.execute(
            "SELECT id, title, content, tags, created_at, updated_at, is_pinned, deleted_at"
            " FROM notes ORDER BY created_at"
        ).fetchall()
    finally:
        connection.close()


def row_by_id(db: str, note_id: str):
    for row in rows(db):
        if row[0] == note_id:
            return row
    return None


def new_rows(db: str, known: set) -> list:
    return [row for row in rows(db) if row[0] not in known]


def read_log(path: str) -> str:
    if not os.path.exists(path):
        return ""
    with open(path, encoding="utf-8", errors="replace") as handle:
        return handle.read()


class Session:
    """One launched shell. See the module header for why it is one per scenario."""

    def __init__(self, exe: str, root: str, log: str) -> None:
        self.exe = exe
        self.root = root
        self.log = log
        self.process = None
        self.sections: list[tuple[str, str]] = []

    def start(self, note_id=None) -> int:
        self.stop()
        command = [self.exe, "--note", "--store-root", self.root, "--log", self.log]
        if note_id:
            command += ["--note-id", note_id]
        self.process = subprocess.Popen(
            command, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL
        )
        return wait_for(lambda: island(0), 25)

    def capture(self, label: str) -> str:
        """Snapshot this scenario's log. `Log::new` truncates the file at startup,
        so what is on disk now belongs to this process alone."""
        text = read_log(self.log)
        self.sections.append((label, text))
        return text

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
        # The next scenario finds its window by title, and every process numbers its
        # first note `#0`: the old one has to be off the desktop before the next
        # launch or `island(0)` answers for a process that is being killed.
        wait_for(lambda: note_window(0) == 0, 5)
        self.process = None


class Report:
    def __init__(self) -> None:
        self.results: list[tuple[str, bool, str]] = []

    def check(self, name: str, ok: bool, detail: str) -> bool:
        self.results.append((name, bool(ok), detail))
        print(f"{'PASS' if ok else 'FAIL'}  {name}: {detail}")
        return bool(ok)

    def failed(self) -> list:
        return [name for name, ok, _ in self.results if not ok]

    def finish(self) -> int:
        failed = self.failed()
        print()
        print(f"{len(self.results) - len(failed)}/{len(self.results)} checks passed")
        if failed:
            print("failed: " + ", ".join(failed))
            print("RESULT: FAIL")
            return 1
        print("RESULT: PASS")
        return 0


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--exe", default="native/target/release/easyflowhub-native.exe")
    parser.add_argument(
        "--work",
        default=os.path.join(tempfile.gettempdir(), "p2-gate"),
        help="scratch directory; the database copy lives here",
    )
    parser.add_argument(
        "--no-scan-check",
        action="store_true",
        help="skip the silent edit that only the 2 s scan can catch",
    )
    args = parser.parse_args()

    exe = os.path.abspath(args.exe)
    if not os.path.exists(exe):
        print(f"missing {exe}; build it with `cargo build --release -p app`")
        return 2

    live_root = os.path.join(os.environ["APPDATA"], "com.yukin.easyflowhub")
    live_db = os.path.join(live_root, "notes.db")
    if not os.path.exists(live_db):
        print(f"missing {live_db}; run the shipped build once so there is a schema to copy")
        return 2
    live_before = (os.path.getsize(live_db), sha256(live_db))

    work = os.path.abspath(args.work)
    root = os.path.join(work, "root")
    os.makedirs(root, exist_ok=True)
    db = os.path.join(root, "notes.db")
    # Copy only the database: the assets directory is 22k files and nothing in this
    # gate touches an attachment.
    shutil.copyfile(live_db, db)
    # `journal_mode=delete`, so there is no `-wal` to copy alongside. If that ever
    # changes, a single-file copy would silently miss committed pages -- hence the
    # reminder rather than a silent copy.
    for sibling in ("-wal", "-shm"):
        if os.path.exists(live_db + sibling):
            print(f"note: {live_db + sibling} exists; copy it too or the copy is stale")

    schema_before = fingerprint(db)
    known = {row[0] for row in rows(db)}
    log_path = os.path.join(work, "native-note.log")

    report = Report()
    session = Session(exe, root, log_path)
    try:
        # ---------------------------------------------------------------- Q20/Q22
        # Real characters, then silence. The debounce is measured from the last
        # keystroke, so the wait is 900 ms plus a frame plus slack.
        window = session.start()
        if not window:
            print("the shell never produced a text island; see " + log_path)
            return 2
        type_text(window, "买菜清单")
        time.sleep(AUTOSAVE + SLACK)
        fresh = new_rows(db, known)
        report.check("Q20 autosave writes a row after typing", len(fresh) == 1, f"new rows={len(fresh)}")
        if not fresh:
            session.capture("Q20")
            print("nothing was written; the rest of the run has no subject")
            return report.finish()
        note_id = fresh[0][0]
        known.add(note_id)
        report.check(
            "Q22 title is the first line",
            fresh[0][1] == "买菜清单",
            f"title={fresh[0][1]!r}",
        )
        report.check(
            "Q20 the write is logged as an autosave",
            f"id={note_id} reason=autosave created=true" in read_log(log_path),
            "note_save reason=autosave created=true",
        )

        # ------------------------------------------------- the 2 s scan (no signal)
        if not args.no_scan_check:
            silent_break(window)
            time.sleep(SCAN_PATH)
            scanned = row_by_id(db, note_id)
            report.check(
                "the scan catches a change no message reported",
                scanned is not None and scanned[2] == "买菜清单\n",
                f"content={scanned[2]!r} after a Return that raised no edit" if scanned else "row missing",
            )
            report.check(
                "the scan's write updated the row instead of adding one",
                f"id={note_id} reason=autosave created=false" in read_log(log_path),
                "note_save created=false",
            )
        else:
            silent_break(window)
            time.sleep(SCAN_PATH)

        # --------------------------------------------------- the tag line (Q-T)
        # Typed, not pushed: the trailing `# ...` line has to be peeled off the body
        # by the same parser the shipped build uses, through the same input path a
        # user takes.
        type_text(window, "鸡蛋\n# 工作 重要")
        time.sleep(AUTOSAVE + SLACK)
        tagged = row_by_id(db, note_id)
        report.check(
            "Q-T the body kept the text above the tag line",
            tagged is not None and tagged[2] == "买菜清单\n鸡蛋",
            f"content={tagged[2]!r}" if tagged else "row missing",
        )
        report.check(
            "Q-T tags came out of the last line",
            tagged is not None and tagged[3] == "工作 重要",
            f"tags={tagged[3]!r}" if tagged else "no row",
        )
        report.check(
            "no CR reached the row",
            tagged is not None and "\r" not in tagged[2],
            "CRLF normalised at the boundary",
        )
        report.check(
            "the row is not pinned by the window's z-order",
            tagged is not None and tagged[6] == 0,
            f"is_pinned={tagged[6]}" if tagged else "no row",
        )
        session.capture("Q20/Q22 + scan + tags")

        # ---------------------------------------------------------------- Q21
        # A keystroke followed immediately by Esc, well inside the 900 ms debounce.
        # A separate process because the first note's row is already on disk: with
        # the same window there would be no way to tell a close-flush from the
        # autosave that was already due.
        session.stop()  # no flush on purpose: the row is settled, nothing is pending
        before = len(rows(db))
        window = session.start()
        if not window:
            report.check("Q21 the shell started for the close test", False, "no island")
        else:
            typed_at = time.monotonic()
            type_text(window, "字")
            esc(window)
            typed_for = time.monotonic() - typed_at
            # The log is the clock here: the OS window outlives the note for a while
            # (eframe hides it and the tray keeps the process), so "the note closed"
            # is the log's `note_close`, not the absence of a window handle.
            closed = wait_for(lambda: "note_close" in read_log(log_path), 6)
            log = session.capture("Q21")
            session.stop()
            report.check(
                "Q21 Esc closed the note",
                bool(closed),
                "note_close within 6 s" if closed else "no note_close in the log",
            )
            landed = [row for row in rows(db) if row[0] not in known]
            report.check(
                "Q21 close flushes the last keystroke",
                len(landed) == 1 and landed[0][2] == "字",
                f"rows={len(landed)} content={landed[0][2]!r}" if landed else "no row",
            )
            report.check(
                "Q21 the flush happened before the debounce",
                typed_for < AUTOSAVE,
                f"Esc came {typed_for * 1000:.0f} ms after the keystroke (debounce {AUTOSAVE * 1000:.0f} ms)",
            )
            report.check(
                "Q21 the close path logged a save, not a drop",
                any("reason=close" in entry and "skipped" not in entry for entry in log.splitlines()),
                "note_save reason=close",
            )
            if landed:
                known.add(landed[0][0])
            report.check(
                "Q21 the count went up by exactly one",
                len(rows(db)) == before + 1,
                f"{before} -> {len(rows(db))}",
            )

        # ---------------------------------------------------------------- Q23
        # An untouched window: opened, closed, nothing typed.
        before = len(rows(db))
        window = session.start()
        if not window:
            report.check("Q23 the shell started for the empty test", False, "no island")
        else:
            esc(window)
            # Same clock as Q21: wait for the close to reach the log, not for the OS
            # window to disappear.
            wait_for(lambda: "note_close" in read_log(log_path), 6)
            log = session.capture("Q23")
            session.stop()
            report.check(
                "Q23 an untouched note leaves no row",
                len(rows(db)) == before,
                f"{before} -> {len(rows(db))}",
            )
            report.check(
                "Q23 the skip is explained in the log",
                any("skipped=empty" in entry for entry in log.splitlines()),
                "note_save skipped=empty",
            )

        # ------------------------------------------------------- reopen by id (Q-L)
        before = len(rows(db))
        row_before = row_by_id(db, note_id)
        window = session.start(note_id)
        report.check("the shell reopened the note", bool(window), f"island={window}")
        if row_before is None:
            report.check("the note still exists to reopen", False, "row missing")
        elif not window:
            report.check("the note loaded", False, "no island")
        else:
            # The island exists a couple of milliseconds before the text is pushed
            # into it (`Editor::create`, then `set_text`), so the load line is waited
            # for rather than read straight after the handle appears.
            wait_for(lambda: "note_load id=" in read_log(log_path), 3)
            log = read_log(log_path)
            report.check(
                "the load is logged as found",
                f"requested={note_id} found=true" in log,
                "note_load requested found=true",
            )
            loaded = [entry for entry in log.splitlines() if "note_load id=" in entry]
            chars = 0
            if loaded:
                found = re.search(r"chars=(\d+)", loaded[-1])
                chars = int(found.group(1)) if found else 0
            report.check(
                "the body was pushed into the control",
                chars > 0,
                f"note_load chars={chars}",
            )
            # The load parks the caret at the end of the *body*, which on this note
            # is not the end of the text: the row has tags, so the control also holds
            # a trailing `# 工作 重要` line. `at < of` is that distinction in the log.
            parked = re.search(r"note_caret id=\d+ at=(\d+) of=(\d+) tag_line=(\w+)", log)
            report.check(
                "the caret was parked before the tag line",
                parked is not None
                and parked.group(3) == "true"
                and int(parked.group(1)) < int(parked.group(2)),
                parked.group(0) if parked else "no note_caret line",
            )
            # The control is unreadable from out here, so the proof is a round trip:
            # type one character at the caret and see where it landed. Typed *at* the
            # end of the text, this character used to join the tag list instead.
            type_text(window, "尾")
            time.sleep(AUTOSAVE + SLACK)
            row_after = row_by_id(db, note_id)
            session.capture("Q-L")
            session.stop()
            report.check(
                "the body survived the reopen",
                row_after is not None and row_after[2] == row_before[2] + "尾",
                f"was {row_before[2]!r} now {row_after[2]!r}" if row_after else "row missing",
            )
            report.check(
                "the caret was at the end of the body, not inside the tag line",
                row_after is not None and row_after[3] == row_before[3],
                f"tags={row_after[3]!r} (expected {row_before[3]!r}; the keystroke must"
                " not join the tag list)"
                if row_after
                else "no row",
            )
            report.check(
                "reopening did not create a second row",
                len(rows(db)) == before,
                f"{before} -> {len(rows(db))}",
            )

        # ------------------------------------------------------------ G-2 schema
        report.check(
            "G-2 schema fingerprint unchanged",
            fingerprint(db) == schema_before,
            f"{len(schema_before.splitlines())} objects",
        )
    finally:
        session.stop()

    # ------------------------------------------------------- G-1 the right database
    # Not "the live file is byte-identical": the shipped build is running on this
    # machine and writes it whenever it likes, so that comparison fails for reasons
    # that have nothing to do with this gate. What matters is that every process
    # above opened the copy -- if it had opened the live database, the rows counted
    # here would have come from somewhere else entirely.
    opens = [
        re.search(r"open=true path=(.+?) sqlite=", entry)
        for label, text in session.sections
        for entry in text.splitlines()
        if "open=true" in entry
    ]
    paths = [match.group(1).strip() for match in opens if match]
    report.check(
        "G-1 the shell opened the copy, not the live database",
        bool(paths) and all(os.path.normcase(os.path.abspath(path)) == os.path.normcase(db) for path in paths),
        f"{len(paths)} opens, path={paths[0] if paths else 'none'}",
    )
    live_after = (os.path.getsize(live_db), sha256(live_db))
    print(
        "note: the live database is written by the shipped build too; "
        f"size/sha256 {'unchanged' if live_before == live_after else 'changed'} during this run"
    )

    code = report.finish()
    if code != 0:
        for label, text in session.sections:
            print()
            print(f"--- log: {label} ---")
            for entry in text.splitlines():
                print(entry)
    return code


if __name__ == "__main__":
    sys.exit(main())
