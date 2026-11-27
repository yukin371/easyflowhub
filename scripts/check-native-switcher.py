"""Phase 2 gate: the switcher, driven the way a user drives it.

    Q25  `Ctrl+K` opens the panel, and the same key closes it again
    Q13  `Esc` closes *only* the panel; the note stays
    Q26  the query matches the body and the tags, not just the title
    Q27  index 0 creates a note; any other row switches this window to that note
    Z    the panel is drawn over the island (the island has to be hidden, or the
         child window paints on top of the list)

What is different from the notes gate: everything here happens in the note's frame
window, not in the RichEdit island. `WM_CHAR` at the control cannot carry a
modifier and cannot reach egui, so the chords are injected with `SendInput` against
the foreground window -- see `scripts/lib/wininput.py` for what that costs and what
it requires.

The database is the oracle, as it is in the notes gate: the control's text cannot be
read across the process boundary. "The window switched to note B and the caret is at
its end" is therefore checked by typing one character and seeing which row gained it.

Each scenario is its own process, for the reason the notes gate gives: `Log::new`
truncates the log at startup, and one window per scenario is easier to read when
something fails.

Usage:
    python scripts/check-native-switcher.py --exe native/target/release/easyflowhub-native.exe
"""

import argparse
import os
import shutil
import sqlite3
import subprocess
import sys
import tempfile
import time

try:
    import ctypes

    user32 = ctypes.WinDLL("user32", use_last_error=True)
except (ImportError, OSError, AttributeError):  # not on Windows: this gate is Windows-only
    print("RESULT: SKIP (this gate drives a Windows window)")
    sys.exit(0)

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "lib"))
import wininput  # noqa: E402  (path first, then the import)

NOTE_PREFIX = "easyflowhub-note"
SLACK = 0.8
AUTOSAVE = 0.9

# Two notes whose titles say nothing about what will be searched. Q26 is judged by
# finding them through the body and through the tags.
NOTE_A = (
    "gate-note-a",
    "周末计划",
    "周末计划\n买螺丝刀和膨胀管",
    "",
)
NOTE_B = (
    "gate-note-b",
    "工作会议",
    "工作会议\n记一下本周进度",
    "工作 重要",
)
STAMP = "2026-11-26T02:31:00.000000000+00:00"


def note_window(index: int) -> int:
    return user32.FindWindowW(None, f"{NOTE_PREFIX}#{index}")


def island(index: int) -> int:
    window = note_window(index)
    return user32.FindWindowExW(window, None, "RICHEDIT50W", None) if window else 0


def seed(db: str) -> None:
    """Replace the copy's notes with the two rows this gate searches for.

    The copy exists for the schema, not for its content: a list whose length depends
    on what the shipped build left behind cannot be asserted.
    """
    connection = sqlite3.connect(db)
    try:
        connection.execute("DELETE FROM notes")
        for note_id, title, content, tags in (NOTE_A, NOTE_B):
            connection.execute(
                "INSERT INTO notes (id, title, content, tags, created_at, updated_at,"
                " is_pinned, deleted_at) VALUES (?, ?, ?, ?, ?, ?, 0, NULL)",
                (note_id, title, content, tags, STAMP, STAMP),
            )
        connection.commit()
    finally:
        connection.close()


def rows(db: str) -> list:
    connection = sqlite3.connect(f"file:{db}?mode=ro", uri=True)
    try:
        return connection.execute(
            "SELECT id, title, content, tags, created_at, updated_at, is_pinned, deleted_at"
            " FROM notes ORDER BY created_at, id"
        ).fetchall()
    finally:
        connection.close()


def row_by_id(db: str, note_id: str):
    for row in rows(db):
        if row[0] == note_id:
            return row
    return None


class Trace:
    """The log, read as events rather than as a snapshot."""

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
        """The first line containing `needle` beyond the `before` already seen."""
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
        self.sections = []

    def start(self) -> tuple:
        """Launch, wait for the island, and wait until the island owns the keyboard.

        `editor_focus` is the readiness marker, not the window handle: a chord sent
        between the two is delivered to a window whose control has not taken focus
        yet, and it disappears. Measured as an intermittent first-chord loss.
        """
        self.stop()
        self.process = subprocess.Popen(
            [self.exe, "--note", "--store-root", self.root, "--log", self.log],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        window = wininput.wait_for(lambda: note_window(0), 25)
        if not window:
            return (0, 0)
        wininput.wait_for(lambda: "editor_focus" in self.trace.text(), 10)
        control = wininput.wait_for(lambda: island(0), 10)
        return (window, control)

    def capture(self, label: str) -> str:
        text = self.trace.text()
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


def open_panel(session: Session, window: int, tries: int = 3) -> bool:
    """`Ctrl+K`, retried only while nothing happened.

    The retry is safe because it is guarded by the log: a chord that did arrive
    leaves a line, and a second one would then close what the first opened. So the
    loop waits for evidence before sending again, and never sends twice blindly.
    """
    for attempt in range(tries):
        wininput.force(window)
        wininput.chord(0x4B, ctrl=True)  # VK_K
        line = session.trace.later("note_switcher id=0 open=true", 0, 1.5)
        if line:
            return True
        print(f"  note: Ctrl+K attempt {attempt + 1} produced no panel; retrying")
    return False


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--exe", default="native/target/release/easyflowhub-native.exe")
    parser.add_argument("--work", default=os.path.join(tempfile.gettempdir(), "p2-switcher-gate"))
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
    os.makedirs(root, exist_ok=True)
    db = os.path.join(root, "notes.db")
    shutil.copyfile(live_db, db)
    seed(db)
    log_path = os.path.join(work, "native-switcher.log")

    report = Report()
    session = Session(exe, root, log_path)
    try:
        # ------------------------------------------------- Q25 open + Q26 + Q27 pick
        window, control = session.start()
        if not window or not control:
            print("the shell never produced a text island; see " + log_path)
            return 2
        report.check("Q25 Ctrl+K opened the panel", open_panel(session, window), "note_switcher open=true")
        opened = session.trace.later("note_switcher id=0 open=true", 0, 1)
        report.check(
            "Q25 the panel listed every live note",
            "items=2" in opened,
            opened.strip() or "no open line",
        )
        report.check(
            "the island is hidden while the panel is open",
            not user32.IsWindowVisible(control),
            "a child window paints over its parent, so the list needs it hidden",
        )

        # Q26: the query is in neither note's title.
        seen = session.trace.count("note_switch id=0")
        wininput.type_text("螺丝刀")
        wininput.tap(wininput.VK_DOWN)
        wininput.tap(wininput.VK_RETURN)
        switched = session.trace.later("note_switch id=0", seen, 4)
        report.check(
            "Q26 the query matched a note by its body",
            "to=gate-note-a" in switched,
            switched.strip() or "no note_switch line",
        )
        # The count in that line is the control's own text, which is CRLF and carries
        # a final paragraph mark -- A's 13 characters arrive as 14. What is asserted
        # is the identity (the row that was picked), and the round trip below is what
        # proves the window really shows it.
        report.check(
            "Q27 the window took on the picked note's text",
            "chars=" in switched and 'title="周末计划"' in switched,
            switched.strip() or "no note_switch line",
        )
        report.check(
            "the island came back after the pick",
            bool(wininput.wait_for(lambda: user32.IsWindowVisible(control), 3)),
            "SW_SHOWNA after the panel closed",
        )

        # The database is the only way to see inside the control: one character at
        # the caret, which the switch parked at the end of the picked body.
        wininput.type_into_island(control, "尾")
        time.sleep(AUTOSAVE + SLACK)
        picked = row_by_id(db, NOTE_A[0])
        report.check(
            "Q27 the pick really moved the window",
            picked is not None and picked[2] == NOTE_A[2] + "尾",
            f"content={picked[2]!r}" if picked else "row missing",
        )
        report.check(
            "switching created no row and left the other note alone",
            len(rows(db)) == 2,
            f"rows={len(rows(db))}",
        )
        session.capture("Q25/Q26/Q27 pick")
        session.stop()

        # --------------------------------- Q25 same key closes + Q26 tags + Q13 Esc
        window, control = session.start()
        if not window or not control:
            report.check("the shell started for the toggle test", False, "no island")
        else:
            report.check("Q25 the panel opened for the toggle test", open_panel(session, window), "")
            # Q26 again, now through the tag column: neither the title nor the body
            # of B contains the search, only its tags do.
            seen = session.trace.count("note_switch id=0")
            wininput.type_text("重要")
            wininput.tap(wininput.VK_DOWN)
            wininput.tap(wininput.VK_RETURN)
            by_tag = session.trace.later("note_switch id=0", seen, 4)
            report.check(
                "Q26 the query matched a note by its tags",
                "to=gate-note-b" in by_tag,
                by_tag.strip() or "no note_switch line",
            )

            opened_before = session.trace.count("note_switcher id=0 open=true")
            report.check("Q25 the panel reopened", open_panel(session, window), "")
            wininput.force(window)
            wininput.chord(0x4B, ctrl=True)
            closed = session.trace.later("note_switcher id=0 open=false", 0, 3)
            report.check(
                "Q25 the same key closed it",
                bool(closed),
                closed.strip() or "no close line",
            )
            report.check(
                "Q25 closing the panel did not close the note",
                session.trace.count("note_close") == 0,
                f"open=true x{opened_before}, note_close={session.trace.count('note_close')}",
            )

            report.check("Q13 the panel reopened for the Esc test", open_panel(session, window), "")
            wininput.force(window)
            wininput.tap(wininput.VK_ESCAPE)
            escaped = session.trace.later("note_switcher id=0 open=false", 0, 3)
            report.check(
                "Q13 Esc closed the panel",
                bool(escaped),
                escaped.strip() or "no close line",
            )
            report.check(
                "Q13 Esc stopped at the panel",
                session.trace.count("note_close") == 0,
                f"note_close={session.trace.count('note_close')}",
            )
            report.check(
                "the island is visible again, so the body is back",
                bool(wininput.wait_for(lambda: user32.IsWindowVisible(control), 3)),
                "SW_SHOWNA",
            )
            # And the *next* Esc belongs to the note again, which is the other half
            # of Q13: the panel eats one Esc, it does not eat the key forever.
            wininput.force(window)
            wininput.tap(wininput.VK_ESCAPE)
            report.check(
                "Q13 the next Esc closes the note",
                bool(wininput.wait_for(lambda: "note_close" in session.trace.text(), 6)),
                "note_close reason=esc",
            )
            session.capture("Q25/Q26 tags/Q13")
            session.stop()

        # ------------------------------------------- Q27 index 0 creates from a query
        seed(db)
        window, control = session.start()
        if not window or not control:
            report.check("the shell started for the create test", False, "no island")
        else:
            report.check("Q27 the panel opened for the create test", open_panel(session, window), "")
            seen = session.trace.count("note_switch id=0")
            wininput.type_text("周报草稿")
            wininput.tap(wininput.VK_RETURN)
            created = session.trace.later("note_switch id=0", seen, 4)
            report.check(
                "Q27 Enter at the top row created a note",
                "to=new" in created and "seeded=true chars=4" in created,
                created.strip() or "no note_switch line",
            )
            found = [row for row in rows(db) if row[1] == "周报草稿"]
            report.check(
                "Q27 the new note is a row, not a window state",
                len(found) == 1 and found[0][2] == "周报草稿",
                f"rows={[row[1] for row in rows(db)]}",
            )
            report.check(
                "Q27 the seed was written by the seed path, not by a close",
                "reason=seed" in session.trace.text(),
                "note_save reason=seed",
            )
            session.capture("Q27 create")
            session.stop()
    finally:
        session.stop()

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
