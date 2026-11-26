"""Phase 2 gate: the workbench, and a rebound keyboard.

    Q16  `Ctrl+Alt+M` opens the workbench  -- here through a *rebound* key, because
         the shipped build owns the original combination on this machine
    Z    the list the workbench shows comes from the database, and it says how much
         of it there is: live notes and trash are counted separately
    Q18  `Esc` closes the workbench and leaves the note alone
    K1   a key written into the settings file is the key that works, for a global
         command and for a window command
    K2   the key the file replaced no longer does anything

Why the rebind is part of this gate rather than a separate one: `open_manager` is a
global registration, and the only way to open the workbench from a test process here
is to own a combination the shipped build does not. Choosing that combination *is*
the feature -- the file is read, the table is built from it, the OS registration
follows it -- so the gate that needs it also proves it.

The workbench's own buttons are not clicked here. A click needs a coordinate inside
an egui layout that is free to change, and a gate that hardcodes one would fail for
a reason that has nothing to do with the behaviour. What the buttons do is covered
where it is decided: `hub-core`'s store tests write the pin/trash/restore/purge
columns and `hub-ui`'s manager tests drain the `Action` queue. This file covers the
window: it opens, it counts, it closes, and the note behind it is untouched.

Usage:
    python scripts/check-native-manager.py --exe native/target/release/easyflowhub-native.exe
"""

import argparse
import json
import os
import shutil
import sqlite3
import subprocess
import sys
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
MANAGER_TITLE = "easyflowhub-manager"
STAMP = "2026-11-26T02:31:00.000000000+00:00"

# The combination this gate gives the workbench: `Ctrl+Alt+M` is held by the shipped
# build, and pressing it here would open *that* window instead.
MANAGER_KEY = "Ctrl+Alt+J"
VK_J = 0x4A
# The window-scope control: `close_note` moves off `Ctrl+D`.
CLOSE_KEY = "Ctrl+Q"
VK_Q = 0x51
VK_D = 0x44

# Two live notes and one in the trash, so "live" and "trash" cannot be the same
# number by accident.
SEED = [
    ("gate-live-1", "买菜清单", "买菜清单\n西红柿 鸡蛋", "", None),
    ("gate-live-2", "周报", "周报\n本周三条结论", "工作", None),
    ("gate-trash", "已经删掉的", "已经删掉的", "", STAMP),
]


def note_window() -> int:
    return user32.FindWindowW(None, f"{NOTE_PREFIX}#0")


def manager_window() -> int:
    return user32.FindWindowW(None, MANAGER_TITLE)


def seed(db: str) -> None:
    connection = sqlite3.connect(db)
    try:
        connection.execute("DELETE FROM notes")
        for note_id, title, content, tags, deleted in SEED:
            connection.execute(
                "INSERT INTO notes (id, title, content, tags, created_at, updated_at,"
                " is_pinned, deleted_at) VALUES (?, ?, ?, ?, ?, ?, 0, ?)",
                (note_id, title, content, tags, STAMP, STAMP, deleted),
            )
        connection.commit()
    finally:
        connection.close()


def counts(db: str) -> tuple:
    connection = sqlite3.connect(f"file:{db}?mode=ro", uri=True)
    try:
        live = connection.execute("SELECT COUNT(*) FROM notes WHERE deleted_at IS NULL").fetchone()[0]
        trash = connection.execute("SELECT COUNT(*) FROM notes WHERE deleted_at IS NOT NULL").fetchone()[0]
        return (live, trash)
    finally:
        connection.close()


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
    def __init__(self, exe: str, root: str, settings: str, log: str) -> None:
        self.exe = exe
        self.root = root
        self.settings = settings
        self.trace = Trace(log)
        self.process = None
        self.sections = []

    def start(self) -> tuple:
        self.stop()
        self.process = subprocess.Popen(
            [
                self.exe,
                "--note",
                "--store-root",
                self.root,
                "--settings",
                self.settings,
                "--log",
                self.trace.path,
            ],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        window = wininput.wait_for(note_window, 25)
        if not window:
            return (0, 0)
        wininput.wait_for(lambda: "editor_focus" in self.trace.text(), 10)
        control = wininput.wait_for(
            lambda: user32.FindWindowExW(window, None, "RICHEDIT50W", None), 10
        )
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
        wininput.wait_for(lambda: note_window() == 0, 5)
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


def open_workbench(session: Session, window: int, before: int = 0, tries: int = 3) -> str:
    """Send the rebound global chord and wait for the workbench to say it opened.

    `before` is how many open lines the log already had: a global hotkey that was
    registered but refused (1409, another program owns it) delivers nothing, and the
    retry loop is the only way to tell that apart from "the log has not been written
    yet".
    """
    for attempt in range(tries):
        wininput.force(window)
        wininput.chord(VK_J, ctrl=True, alt=True)
        line = session.trace.later("note_manager action=open", before, 2.0)
        if line:
            return line
        print(f"  note: {MANAGER_KEY} attempt {attempt + 1} opened nothing; retrying")
    return ""


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--exe", default="native/target/release/easyflowhub-native.exe")
    parser.add_argument("--work", default=os.path.join("native", "target", "p2-manager-gate"))
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
    live, trash = counts(db)

    # The settings file is the whole input of the first scenario: two overrides, one
    # per scope. Everything else is the default, which is also a check -- a partial
    # file must not blank the settings it does not mention.
    settings_path = os.path.join(work, "settings.json")
    with open(settings_path, "w", encoding="utf-8") as handle:
        json.dump(
            {"shortcuts": {"open_manager": MANAGER_KEY, "close_note": CLOSE_KEY}},
            handle,
            ensure_ascii=False,
            indent=2,
        )
    log_path = os.path.join(work, "native-manager.log")

    report = Report()
    session = Session(exe, root, settings_path, log_path)
    try:
        # ------------------------------------------------------- K1 the file is read
        window, control = session.start()
        if not window or not control:
            print("the shell never produced a text island; see " + log_path)
            return 2
        log = session.trace.text()
        report.check(
            "K1 the settings file was used, not the defaults",
            "source=file" in log,
            "settings source=file",
        )
        report.check(
            "K1 the global command took the key from the file",
            f"shortcut scope=global command=open_manager key={MANAGER_KEY}" in log,
            f"open_manager key={MANAGER_KEY}",
        )
        report.check(
            "K1 the window command took the key from the file",
            f"shortcut scope=window command=close_note key={CLOSE_KEY}" in log,
            f"close_note key={CLOSE_KEY}",
        )
        report.check(
            "K1 the OS accepted the rebound global key",
            f"hotkey combo={MANAGER_KEY} registered=true" in log,
            "hotkey registered=true",
        )
        report.check(
            "a file that names two keys leaves the other nine alone",
            "shortcut scope=window command=switcher key=Ctrl+K" in log
            and "shortcut scope=global command=new_note key=Ctrl+Alt+N" in log,
            "switcher=Ctrl+K, new_note=Ctrl+Alt+N",
        )

        # ------------------------------------------------------------ Q16 open it
        opened = open_workbench(session, window)
        report.check("Q16 the rebound key opened the workbench", bool(opened), opened.strip())
        shell = wininput.wait_for(manager_window, 10)
        report.check(
            "Q16 the workbench is a window of its own",
            bool(shell) and user32.IsWindowVisible(shell),
            f"hwnd=0x{shell:X}" if shell else "no {MANAGER_TITLE} window",
        )
        listed = session.trace.later("manager_open", 0, 5)
        report.check(
            "the workbench counted the database, live and trash separately",
            f"notes={live} trash={trash}" in listed and "store=on" in listed,
            listed.strip() or "no manager_open line",
        )
        report.check(
            "the note window is still there and still visible",
            bool(user32.IsWindowVisible(window)) and session.trace.count("note_close") == 0,
            f"note_close={session.trace.count('note_close')}",
        )

        # ------------------------------------------------------- Q18 Esc closes it
        report.check("Q18 the workbench took the keyboard", wininput.force(shell), "foreground")
        wininput.tap(wininput.VK_ESCAPE)
        closed = session.trace.later("manager_close", 0, 5)
        report.check("Q18 Esc closed the workbench", bool(closed) and "reason=esc" in closed, closed.strip())
        report.check(
            "Q18 Esc did not reach the note behind it",
            session.trace.count("note_close") == 0,
            f"note_close={session.trace.count('note_close')}",
        )
        report.check(
            "Q18 the window is gone, not just hidden",
            bool(wininput.wait_for(lambda: manager_window() == 0, 5)),
            "handle dropped",
        )

        # Reopening must build a fresh shell rather than reveal a stale one: the list
        # is read from the database on every open.
        opens = session.trace.count("note_manager action=open")
        shells = session.trace.count("manager_open")
        reopened = open_workbench(session, window, opens)
        report.check("Q16 the workbench can be opened again", bool(reopened), reopened.strip())
        second = session.trace.later("manager_open", shells, 5)
        report.check(
            "reopening re-read the database",
            f"notes={live} trash={trash}" in second,
            second.strip() or "no second manager_open line",
        )
        session.capture("Q16/Q18 + K1 global")
        session.stop()

        # ------------------------------------ K2 the replaced key stops answering
        window, control = session.start()
        if not window or not control:
            report.check("the shell started for the rebind test", False, "no island")
        else:
            wininput.force(window)
            wininput.chord(VK_D, ctrl=True)  # the key `close_note` used to have
            time.sleep(1.2)
            report.check(
                "K2 the old key does nothing",
                session.trace.count("note_key id=0 command=close_note") == 0,
                f"close_note presses={session.trace.count('note_key id=0 command=close_note')}",
            )
            report.check(
                "K2 the note is still open",
                session.trace.count("note_close") == 0
                and bool(user32.IsWindowVisible(window)),
                f"note_close={session.trace.count('note_close')}",
            )
            wininput.chord(VK_Q, ctrl=True)
            fired = session.trace.later("note_key id=0 command=close_note", 0, 4)
            report.check(
                "K2 the new key closes the note",
                bool(fired) and f"key={CLOSE_KEY}" in fired,
                fired.strip() or "no note_key line",
            )
            report.check(
                "K2 the close ran the close path",
                bool(wininput.wait_for(lambda: "note_close" in session.trace.text(), 6)),
                "note_close",
            )
            session.capture("K2 window rebind")
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
