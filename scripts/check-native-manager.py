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

The workbench's buttons are clicked, and none of their coordinates is written down
here. The window reports where each control landed (`manager_hit button=pin:note-1
x=… y=…`), which is the same contract the todo card uses for its strip, and the
script clicks what was reported. That replaced an earlier note in this file that
said row buttons must not be clicked because a hardcoded coordinate would be
brittle -- true, but the fix was to stop hardcoding, not to stop clicking.

What this buys: the pin, trash, restore and purge buttons are now covered where
they can actually break. The store tests prove the SQL works and the `hub-ui`
tests prove the right `Action` is returned; neither can show that the button is
under the pointer, or that the controller applies the action to the same note the
list is showing.

The trash section goes one step further than a single row: its sub-tabs, the
per-row box that picks a row for the batch, 恢复选中 with two rows ticked, and
清空回收站 (which must ask first and then take everything). Each of those is a
click at a reported rectangle followed by a read of the database, because "the
list looked different" is not evidence that anything was written.

Usage:
    python scripts/check-native-manager.py --exe native/target/release/easyflowhub-native.exe
"""

import argparse
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
    import ctypes

    user32 = ctypes.WinDLL("user32", use_last_error=True)
except (ImportError, OSError, AttributeError):  # not on Windows: this gate is Windows-only
    print("RESULT: SKIP (this gate drives a Windows window)")
    sys.exit(0)

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "lib"))
import wininput  # noqa: E402  (path first, then the import)

NOTE_PREFIX = "easyflowhub-note"
MANAGER_TITLE = "easyflowhub-manager"
CARD_TITLE = "easyflowhub-todo-card"
STAMP = "2026-11-26T02:31:00.000000000+00:00"

# One reported control, in physical client pixels -- the coordinates `click_client()`
# takes. The name is what the widget called itself: `new-note`, `todo-card`,
# `tab:trash`, `row:<id>`, `pin:<id>`, `trash:<id>`, `restore:<id>`, `purge:<id>`.
HIT = re.compile(
    r"manager_hit button=(\S+) x=(-?\d+) y=(-?\d+) w=(\d+) h=(\d+)"
)

# The combination this gate gives the workbench: `Ctrl+Alt+M` is held by the shipped
# build, and pressing it here would open *that* window instead.
MANAGER_KEY = "Ctrl+Alt+J"
VK_J = 0x4A
# The window-scope control: `close_note` moves off `Ctrl+D`.
CLOSE_KEY = "Ctrl+Q"
VK_Q = 0x51
VK_D = 0x44

# Three live notes and three in the trash, so "live" and "trash" cannot be the same
# number by accident, and so a batch restore has more than one row to take. One of
# the live ones is the todo inbox: a note whose body is checkbox lines, one of them
# already ticked -- so "the pane hides what is done" and "the pane can tick" are two
# different observations on the same list.
SEED = [
    ("gate-live-1", "买菜清单", "买菜清单\n西红柿 鸡蛋", "", None),
    ("gate-live-2", "周报", "周报\n本周三条结论", "工作", None),
    ("gate-trash", "已经删掉的", "已经删掉的", "", STAMP),
    ("gate-trash-2", "删掉的第二条", "删掉的第二条\n正文", "", STAMP),
    ("gate-trash-3", "删掉的第三条", "删掉的第三条", "", STAMP),
    (
        "gate-todo",
        "待办箱",
        "待办箱\n- [ ] 买牛奶\n- [x] 交房租 @done:2026-11-20T09:00:00.000Z\n- [ ] 回邮件",
        "",
        None,
    ),
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


def query(db: str, sql: str, params: tuple = ()):
    """One row, retried: the app may be mid-write when the check runs."""
    deadline = time.time() + 5.0
    while True:
        try:
            connection = sqlite3.connect(db, timeout=2.0)
            try:
                return connection.execute(sql, params).fetchone()
            finally:
                connection.close()
        except sqlite3.OperationalError:
            if time.time() > deadline:
                return None
            time.sleep(0.05)


def hit(text: str, name: str):
    """The latest rectangle reported for `name`, or None.

    Latest, not first: a click that changes the list relayouts the rows, and the gate
    must click where the button is *now*.
    """
    found = None
    for line in text.splitlines():
        match = HIT.search(line)
        if match and match.group(1) == name:
            found = tuple(int(match.group(i)) for i in (2, 3, 4, 5))
    return found


def hit_names(text: str) -> set:
    return {match.group(1) for line in text.splitlines() if (match := HIT.search(line))}


HWND_TOP = 0
SWP_NOSIZE = 0x0001
SWP_NOMOVE = 0x0002
SWP_NOZORDER = 0x0004
SWP_NOACTIVATE = 0x0010
user32.SetWindowPos.argtypes = [
    ctypes.c_void_p,
    ctypes.c_void_p,
    ctypes.c_int,
    ctypes.c_int,
    ctypes.c_int,
    ctypes.c_int,
    ctypes.c_uint,
]
user32.IsZoomed.argtypes = [ctypes.c_void_p]
user32.WindowFromPoint.restype = ctypes.c_void_p


def describe(hwnd: int) -> str:
    cls = ctypes.create_unicode_buffer(256)
    user32.GetClassNameW(hwnd, cls, 256)
    box = ctypes.wintypes.RECT()
    user32.GetWindowRect(hwnd, ctypes.byref(box))
    root = user32.GetAncestor(hwnd, 2)  # GA_ROOT
    return (
        f"0x{hwnd:X} class={cls.value!r} rect=({box.left},{box.top},"
        f"{box.right - box.left}x{box.bottom - box.top}) root=0x{root:X}"
    )


def describe_process(hwnd: int) -> list:
    """Every top-level window of `hwnd`'s process, one line each.

    Printed only when a click is about to land on something that is not the
    workbench -- that happened twice (a note covering the button), and "which window
    is this" is the question the failure asks.
    """
    pid = ctypes.wintypes.DWORD()
    user32.GetWindowThreadProcessId(hwnd, ctypes.byref(pid))
    found = []

    @ctypes.WINFUNCTYPE(ctypes.c_bool, ctypes.c_void_p, ctypes.c_void_p)
    def visit(window, _):
        owner = ctypes.wintypes.DWORD()
        user32.GetWindowThreadProcessId(window, ctypes.byref(owner))
        if owner.value == pid.value:
            title = ctypes.create_unicode_buffer(128)
            user32.GetWindowTextW(window, title, 128)
            cls = ctypes.create_unicode_buffer(128)
            user32.GetClassNameW(window, cls, 128)
            box = ctypes.wintypes.RECT()
            user32.GetWindowRect(window, ctypes.byref(box))
            found.append(
                f"0x{window:X} {cls.value} {box.left},{box.top} "
                f"{box.right - box.left}x{box.bottom - box.top} {title.value!r}"
            )
        return True

    user32.EnumWindows(visit, 0)
    return found


def window_at(x: int, y: int):
    """Which window would receive a click at this screen point, and its title."""
    hwnd = user32.WindowFromPoint(ctypes.wintypes.POINT(int(x), int(y)))
    title = ctypes.create_unicode_buffer(256)
    user32.GetWindowTextW(hwnd, title, 256)
    return hwnd, title.value


def raise_window(hwnd: int) -> None:
    """Put the workbench above the notes it just opened.

    A click here is a real mouse event at a screen point, not a message posted to a
    handle, so whatever is on top receives it. Clicking a row opens that note in
    front; without this the next click would land on the note instead of the button.
    """
    wininput.force(hwnd)
    user32.SetWindowPos(hwnd, HWND_TOP, 0, 0, 0, 0, SWP_NOSIZE | SWP_NOMOVE | SWP_NOACTIVATE)
    time.sleep(0.05)


def park_spawned_notes() -> int:
    """Move the notes the list opened off the work area.

    The quick note is created on top (and can be pinned there), so a note opened by
    clicking a row sits over the workbench -- and a click here is a real mouse event
    at a screen point, so the note receives it. Nothing below asserts anything about
    where a note is, so moving them off screen only makes the clicks land where they
    were aimed. The first note (`#0`) is left alone: the gate checks it survives.
    """
    moved = 0
    for index in range(1, 8):
        hwnd = user32.FindWindowW(None, f"{NOTE_PREFIX}#{index}")
        if hwnd:
            user32.SetWindowPos(
                hwnd, 0, -4000, -4000, 0, 0, SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE
            )
            moved += 1
    return moved


def clear_spot(shell: int, point: tuple, tries: int = 6) -> bool:
    """Make sure a click at `point` (client pixels) lands on the workbench.

    A note is created on top, so a note opened by the list sits over the corner where
    a button is. `park_spawned_notes` moves the ones it can find by title; a note
    whose window has not taken its title yet cannot be found that way, and that is
    the case this used to lose: the workbench was nudged left and right in a loop
    that oscillated between two positions and never cleared the spot.

    So the *blocker* is moved, not the workbench -- far enough that the point is
    free, and always in the same direction so six tries cannot undo each other.
    """
    for _ in range(tries):
        left, top, _, _ = wininput.client_box(shell)
        under, _title = window_at(left + point[0], top + point[1])
        if user32.GetAncestor(under, 2) == shell:  # GA_ROOT
            return True
        root = user32.GetAncestor(under, 2) or under
        box = ctypes.wintypes.RECT()
        if root and user32.GetWindowRect(root, ctypes.byref(box)):
            user32.SetWindowPos(
                root,
                0,
                box.left + 900,
                box.top + 520,
                0,
                0,
                SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
            )
        time.sleep(0.15)
    return False


def latest_rect(session: "Session", name: str, current, timeout: float = 0.12):
    """Prefer a newer rectangle for `name` when the window redrew after being raised.

    The workbench relayouts after a write (a restored row leaves the trash, the rest
    move up) and only then reports the new rectangles. Clicking the one that was
    current when the click was decided lands where the button used to be -- the
    click is swallowed by empty space and the test blames the product.
    """
    deadline = time.time() + timeout
    while time.time() < deadline:
        now = hit(session.trace.text(), name)
        if now and now != current:
            return now
        time.sleep(0.05)
    return current


def click_button(session: "Session", shell: int, name: str, timeout: float = 6.0):
    """Click the centre of the rectangle the window reported for `name`."""
    deadline = time.time() + timeout
    while time.time() < deadline:
        park_spawned_notes()
        rect = hit(session.trace.text(), name)
        if rect and rect[2] > 0 and rect[3] > 0:
            raise_window(shell)
            rect = latest_rect(session, name, rect)
            point = (rect[0] + rect[2] // 2, rect[1] + rect[3] // 2)
            clear = clear_spot(shell, point)
            left, top, _, _ = wininput.client_box(shell)
            under, under_title = window_at(left + point[0], top + point[1])
            wininput.click_client(shell, point[0], point[1])
            print(
                f"  click {name} at {point} rect={rect}"
                f" box={(left, top)} zoomed={bool(user32.IsZoomed(shell))}"
                f" fg=0x{user32.GetForegroundWindow():X}"
                f" clear={clear}"
                f" under={describe(under)}({under_title!r})"
            )
            if not clear:
                for line in describe_process(shell):
                    print(f"    window {line}")
            return rect
        time.sleep(0.05)
    print(f"  click {name}: no rectangle was reported")
    return None


def open_confirm(session: "Session", shell: int, row_id: str, timeout: float = 5.0):
    """Click 彻底删除 and wait for a *fresh* report of the dialog's buttons.

    Fresh matters: the rectangle is only written when a control moves or reappears,
    and the dialog reopens at the same place. Watching from a mark taken before the
    click is what distinguishes "it reopened" from "that line was already there".
    """
    mark = len(session.trace.text())
    click_button(session, shell, f"purge:{row_id}")
    return wininput.wait_for(
        lambda: hit(session.trace.tail(mark), "confirm-ok") is not None, timeout
    )


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

    def tail(self, mark: int) -> str:
        """Everything written after `mark`, so an earlier run cannot answer for this one."""
        return self.text()[mark:]

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

    `before` is how many open lines the log already had, so "opened" means "opened
    *now*". The loop is here because of a measured fact: the first chord after the
    workbench has been closed and the note has taken the foreground back is
    sometimes lost -- repeatably so, across runs, while every later attempt lands.
    Nothing in the shell's hotkey path depends on focus (the registration lives on
    the hotkey thread and `WM_HOTKEY` is posted there), so this is the injection
    side: a synthetic `Ctrl+Alt` chord injected right after a foreground change does
    not always reach the queue. Retrying is safe because it is guarded by the log --
    a chord that did arrive leaves a line, and a second chord sent blindly would then
    close what the first one opened.
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
    parser.add_argument("--work", default=os.path.join(tempfile.gettempdir(), "p2-manager-gate"))
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
        # ------------------------------------- the workbench's own 新建笔记 button
        # The list used to say "按 Ctrl+Alt+N 新建" because there was nothing to click.
        # Now there is, and the rectangle is reported rather than guessed: a layout
        # that moves the button must not fail this gate.
        shell = wininput.wait_for(manager_window, 10)
        new_note = hit(session.trace.text(), "new-note")
        report.check(
            "the workbench reports where its 新建笔记 button is",
            bool(new_note) and new_note[2] > 0 and new_note[3] > 0,
            (
                f"x={new_note[0]} y={new_note[1]} w={new_note[2]} h={new_note[3]}"
                if new_note
                else "no manager_hit line"
            ),
        )
        if new_note:
            spawns = session.trace.count("note_spawn")
            click_button(session, shell, "new-note")
            clicked = session.trace.later("manager_new_note", 0, 4)
            report.check(
                "clicking 新建笔记 is the same request Ctrl+Alt+N sends",
                bool(clicked) and "from=button" in clicked,
                clicked.strip() or "no manager_new_note line",
            )
            spawned = session.trace.later("note_spawn", spawns, 8)
            second_note = wininput.wait_for(
                lambda: user32.FindWindowW(None, f"{NOTE_PREFIX}#1"), 8
            )
            report.check(
                "the click produced a real second note window",
                bool(spawned) and bool(second_note),
                (spawned.strip() if spawned else "no note_spawn line")
                + f"; window={'yes' if second_note else 'no'}",
            )
            report.check(
                "the workbench stays open after making the note",
                bool(manager_window()) and session.trace.count("manager_close") == 1,
                f"closes={session.trace.count('manager_close')}",
            )
        session.capture("Q16/Q18 + K1 global + new note button")

        # ------------------------------------ the list's buttons, really clicked
        # Nothing here hardcodes a coordinate: every click uses the rectangle the
        # window reported for that control. Assertions read the log from a mark taken
        # *before* the click, so a line left by an earlier run cannot pass for this
        # one -- the log is appended to across runs.
        names = hit_names(session.trace.text())
        report.check(
            "the workbench reports every row it draws",
            {"row:gate-live-1", "row:gate-live-2"} <= names,
            ", ".join(sorted(n for n in names if n.startswith("row:"))) or "no row rects",
        )

        # Selecting a row is what reveals its buttons. A click also opens the note --
        # that is what this list does -- so the workbench is raised first.
        mark = len(session.trace.text())
        click_button(session, shell, "row:gate-live-2")
        revealed = wininput.wait_for(
            lambda: hit(session.trace.tail(mark), "pin:gate-live-2") is not None, 4
        )
        report.check(
            "clicking a row reveals that row's buttons, and only that row's",
            bool(revealed) and hit(session.trace.tail(mark), "pin:gate-live-1") is None,
            "pin:gate-live-2 reported" if revealed else "no pin rect for the clicked row",
        )

        before = session.trace.count("manager_pin")
        click_button(session, shell, "pin:gate-live-2")
        pinned = session.trace.later("manager_pin", before, 5)
        stored = query(db, "SELECT is_pinned FROM notes WHERE id = ?", ("gate-live-2",))
        report.check(
            "the 置顶 button pins that note, in the database",
            "id=gate-live-2" in pinned and "ok=Some(true)" in pinned and stored == (1,),
            (pinned.strip() or "no manager_pin line") + f"; is_pinned={stored}",
        )

        before = session.trace.count("manager_trash")
        click_button(session, shell, "trash:gate-live-2")
        trashed = session.trace.later("manager_trash", before, 5)
        stored = query(db, "SELECT deleted_at FROM notes WHERE id = ?", ("gate-live-2",))
        report.check(
            "the 移到回收站 button soft-deletes it, with a stamp",
            "id=gate-live-2" in trashed and "ok=Some(true)" in trashed and bool(stored and stored[0]),
            (trashed.strip() or "no manager_trash line") + f"; deleted_at={stored}",
        )

        mark = len(session.trace.text())
        click_button(session, shell, "tab:trash")
        listed = wininput.wait_for(
            lambda: hit(session.trace.tail(mark), "row:gate-trash") is not None, 4
        )
        report.check(
            "the trash tab lists what was just moved there, still selected",
            bool(listed) and hit(session.trace.tail(mark), "restore:gate-live-2") is not None,
            "trash rows reported" if listed else "gate-trash not listed after the switch",
        )

        mark = len(session.trace.text())
        click_button(session, shell, "row:gate-trash")
        wininput.wait_for(lambda: hit(session.trace.tail(mark), "restore:gate-trash") is not None, 4)
        before = session.trace.count("manager_restore")
        click_button(session, shell, "restore:gate-trash")
        restored = session.trace.later("manager_restore", before, 5)
        stored = query(db, "SELECT deleted_at FROM notes WHERE id = ?", ("gate-trash",))
        report.check(
            "the 恢复 button puts the note back",
            "id=gate-trash" in restored and "ok=Some(true)" in restored and stored == (None,),
            (restored.strip() or "no manager_restore line") + f"; deleted_at={stored}",
        )

        # ------------------------------------- 彻底删除 asks first
        # The one irreversible button in this window: it used to purge on a single
        # click, and the gate proved it by deleting a note. Now the first click must
        # only ask, and only the second yes may delete. Both ways out are checked
        # too, because "cancel" that leaves the dialog open is not a cancel.
        mark = len(session.trace.text())
        click_button(session, shell, "row:gate-live-2")
        wininput.wait_for(lambda: hit(session.trace.tail(mark), "purge:gate-live-2") is not None, 4)

        before = session.trace.count("manager_confirm action=ask")
        offered = open_confirm(session, shell, "gate-live-2")
        asked = session.trace.later("manager_confirm action=ask", before, 5)
        stored = query(db, "SELECT COUNT(*) FROM notes WHERE id = ?", ("gate-live-2",))
        report.check(
            "彻底删除 asks before it deletes anything",
            "id=gate-live-2" in asked and bool(offered) and stored == (1,),
            (asked.strip() or "no manager_confirm ask line")
            + f"; confirm-ok={'yes' if offered else 'no'}; rows={stored}",
        )

        before = session.trace.count("manager_confirm action=cancel")
        click_button(session, shell, "confirm-cancel")
        cancelled = session.trace.later("manager_confirm action=cancel", before, 5)
        stored = query(db, "SELECT COUNT(*) FROM notes WHERE id = ?", ("gate-live-2",))
        report.check(
            "取消 keeps the note and closes the dialog",
            bool(cancelled) and stored == (1,),
            (cancelled.strip() or "no manager_confirm cancel line") + f"; rows={stored}",
        )

        # Esc is the keyboard's way out, and it must not take the workbench with it.
        # That layering is the reason the view reports whether anything was open.
        before = session.trace.count("manager_confirm action=ask")
        offered = open_confirm(session, shell, "gate-live-2")
        wininput.force(shell)
        wininput.tap(0x1B)
        escaped = session.trace.later("manager_confirm action=cancel reason=esc", 0, 5)
        stored = query(db, "SELECT COUNT(*) FROM notes WHERE id = ?", ("gate-live-2",))
        report.check(
            "Esc cancels the dialog and leaves the workbench open",
            bool(offered) and bool(escaped) and stored == (1,) and manager_window() is not None,
            (escaped.strip() or "no esc cancel line")
            + f"; rows={stored}; manager={'open' if manager_window() else 'gone'}",
        )

        before = session.trace.count("manager_purge")
        offered = open_confirm(session, shell, "gate-live-2")
        click_button(session, shell, "confirm-ok")
        purged = session.trace.later("manager_purge", before, 5)
        answered = session.trace.later("manager_confirm action=ok", 0, 2)
        stored = query(db, "SELECT COUNT(*) FROM notes WHERE id = ?", ("gate-live-2",))
        report.check(
            "the second yes removes the row for good",
            bool(offered)
            and "id=gate-live-2" in purged
            and "permanent=true" in purged
            and bool(answered)
            and stored == (0,),
            (purged.strip() or "no manager_purge line")
            + f"; answer={(answered.strip() or 'none')}; rows={stored}",
        )

        # ------------------------------------- the trash's sub-tabs, and a batch
        # The shipped panel splits its trash into notes / captures / scripts. This
        # window owns notes; the other two chips answer "where did that go" instead of
        # listing something the native shell cannot see. "A sentence replaced the
        # list" is only observable if the sentence reports a rectangle, so it does.
        mark = len(session.trace.text())
        # Leave and come back: rectangles are written when a control appears, and the
        # workbench is already on the trash after the purge above.
        click_button(session, shell, "tab:notes")
        click_button(session, shell, "tab:trash")
        chips = wininput.wait_for(
            lambda: all(
                hit(session.trace.tail(mark), f"trash-view:{name}") is not None
                for name in ("notes", "inbox", "scripts")
            ),
            4,
        )
        report.check(
            "the trash reports its three sub-tabs",
            bool(chips),
            "notes/inbox/scripts"
            if chips
            else ", ".join(sorted(n for n in hit_names(session.trace.tail(mark)) if n.startswith("trash"))),
        )

        before = session.trace.count("manager_trash_view")
        mark = len(session.trace.text())
        click_button(session, shell, "trash-view:inbox")
        notice = wininput.wait_for(
            lambda: hit(session.trace.tail(mark), "trash-notice") is not None, 4
        )
        switched = session.trace.later("manager_trash_view", before, 5)
        report.check(
            "收集箱 says where that trash lives instead of listing notes",
            bool(notice) and "view=inbox" in switched,
            (switched.strip() or "no manager_trash_view line")
            + f"; notice={'yes' if notice else 'no'}",
        )

        mark = len(session.trace.text())
        click_button(session, shell, "trash-view:notes")
        back = wininput.wait_for(
            lambda: hit(session.trace.tail(mark), "row:gate-trash-2") is not None, 4
        )
        report.check(
            "笔记 brings the list back",
            bool(back),
            "rows reported again" if back else "no row rect after switching back",
        )

        # ------------------------------------- picking rows and restoring them
        # Two ticks, one button. The bar appears with the first tick, which moves the
        # list down; the gate re-reads the rectangle it is about to press, which is
        # the entire reason the product reports rectangles instead of the script
        # guessing them.
        mark = len(session.trace.text())
        click_button(session, shell, "check:gate-trash-2")
        click_button(session, shell, "check:gate-trash-3")
        offered = wininput.wait_for(
            lambda: hit(session.trace.tail(mark), "restore-selected") is not None, 4
        )
        report.check(
            "ticking two trash rows offers 恢复选中",
            bool(offered),
            "restore-selected reported" if offered else "the batch bar never appeared",
        )

        before = session.trace.count("manager_restore_many")
        click_button(session, shell, "restore-selected")
        restored = session.trace.later("manager_restore_many", before, 5)
        left = query(
            db,
            "SELECT COUNT(*) FROM notes WHERE id IN ('gate-trash-2','gate-trash-3')"
            " AND deleted_at IS NOT NULL",
        )
        report.check(
            "恢复选中 restores every ticked row in one write",
            "count=2" in restored and left == (0,),
            (restored.strip() or "no manager_restore_many line") + f"; still trashed={left}",
        )

        # An emptied trash must stop offering the destructive header button: one that
        # stays on screen can only answer "清空 0 条". Leaving and returning is also
        # what proves the frame was redrawn with no such control -- a fresh chip rect
        # in the tail is the evidence that a frame happened at all.
        mark = len(session.trace.text())
        click_button(session, shell, "tab:notes")
        click_button(session, shell, "tab:trash")
        redrew = wininput.wait_for(
            lambda: hit(session.trace.tail(mark), "trash-view:notes") is not None, 4
        )
        report.check(
            "an emptied trash stops offering 清空回收站",
            bool(redrew) and hit(session.trace.tail(mark), "empty-trash") is None,
            "redrew without an empty-trash rect" if redrew else "no fresh chip rect",
        )

        # ------------------------------------- 清空回收站 asks first, then takes all
        # Same rule as 彻底删除: a question, not a decision. The dialog names a count,
        # so the check is on `verb=empty` and on the rows still being there.
        click_button(session, shell, "tab:notes")
        mark = len(session.trace.text())
        click_button(session, shell, "row:gate-live-1")
        wininput.wait_for(
            lambda: hit(session.trace.tail(mark), "trash:gate-live-1") is not None, 4
        )
        click_button(session, shell, "trash:gate-live-1")
        mark = len(session.trace.text())
        click_button(session, shell, "tab:trash")
        wininput.wait_for(lambda: hit(session.trace.tail(mark), "empty-trash") is not None, 4)

        before = session.trace.count("manager_confirm action=ask")
        mark = len(session.trace.text())
        click_button(session, shell, "empty-trash")
        dialog = wininput.wait_for(
            lambda: hit(session.trace.tail(mark), "confirm-ok") is not None, 5
        )
        asked = session.trace.later("manager_confirm action=ask", before, 5)
        stored = query(db, "SELECT COUNT(*) FROM notes WHERE id = ?", ("gate-live-1",))
        report.check(
            "清空回收站 asks first, and the row is still there",
            bool(dialog) and "verb=empty" in asked and stored == (1,),
            (asked.strip() or "no manager_confirm ask line") + f"; rows={stored}",
        )

        before = session.trace.count("manager_empty_trash")
        click_button(session, shell, "confirm-ok")
        emptied = session.trace.later("manager_empty_trash", before, 5)
        left = query(db, "SELECT COUNT(*) FROM notes WHERE deleted_at IS NOT NULL")
        report.check(
            "the second yes empties the trash for good",
            "count=1" in emptied and "permanent=true" in emptied and left == (0,),
            (emptied.strip() or "no manager_empty_trash line") + f"; trashed rows={left}",
        )

        # ------------------------------------- todos, ticked in the workbench
        # The pane used to be read-only: only the desktop card could flip a checkbox,
        # which made the workbench a viewer of its own data. Every line now reports two
        # targets -- the box ticks, the text jumps to the note -- and this gate presses
        # where the product says they are, then reads the note body back out of the
        # database. The seeded body has one line already ticked, so the default filter
        # has something to hide.
        mark = len(session.trace.text())
        click_button(session, shell, "tab:todos")
        listed = wininput.wait_for(
            lambda: hit(session.trace.tail(mark), "todo-tick:gate-todo:1") is not None, 4
        )
        tail = session.trace.tail(mark)
        report.check(
            "the todo tab lists the checkboxes of every note",
            bool(listed) and hit(tail, "todo-tick:gate-todo:3") is not None,
            "todo rows reported" if listed else "no todo-tick rect",
        )
        report.check(
            "未完成 is the default view, so the ticked line is not listed",
            bool(listed) and hit(tail, "todo-tick:gate-todo:2") is None,
            "gate-todo:2 "
            + ("hidden" if hit(tail, "todo-tick:gate-todo:2") is None else "listed"),
        )

        before = session.trace.count("manager_todo_toggle")
        click_button(session, shell, "todo-tick:gate-todo:1")
        toggled = session.trace.later("manager_todo_toggle", before, 5)
        stored = query(db, "SELECT content FROM notes WHERE id = ?", ("gate-todo",))
        body = stored[0] if stored else ""
        report.check(
            "clicking the box ticks that line, in the note body",
            "done=true" in toggled
            and "line=1" in toggled
            and "created=false" in toggled
            and "- [x] 买牛奶 @done:" in body,
            (toggled.strip() or "no manager_todo_toggle line") + f"; body={body!r}",
        )

        before = session.trace.count("manager_todo_filter")
        mark = len(session.trace.text())
        click_button(session, shell, "todo-filter:all")
        filtered = session.trace.later("manager_todo_filter", before, 5)
        both = wininput.wait_for(
            lambda: hit(session.trace.tail(mark), "todo-tick:gate-todo:2") is not None, 4
        )
        report.check(
            "全部 brings the completed lines back",
            "filter=all" in filtered and bool(both),
            (filtered.strip() or "no manager_todo_filter line")
            + f"; done row={'listed' if both else 'missing'}",
        )

        # The untick path is a different rewrite: it has to drop the `@done:` stamp,
        # not just change `x` back to a space. A body that kept the stamp would show
        # a completion date on an open task.
        before = session.trace.count("manager_todo_toggle")
        click_button(session, shell, "todo-tick:gate-todo:2")
        untoggled = session.trace.later("manager_todo_toggle", before, 5)
        stored = query(db, "SELECT content FROM notes WHERE id = ?", ("gate-todo",))
        body = stored[0] if stored else ""
        report.check(
            "unticking drops the @done stamp as well as the x",
            "done=false" in untoggled
            and "- [ ] 交房租" in body
            and "交房租 @done" not in body,
            (untoggled.strip() or "no manager_todo_toggle line") + f"; body={body!r}",
        )

        # The text is the other target: it opens the note the task lives in, which is
        # how "where do I go to write more" is answered from the list. `note_open`
        # carries the *note* id; `note_spawn` carries the window's index, which is a
        # different number and the wrong one to watch.
        before = session.trace.count("note_open id=gate-todo")
        click_button(session, shell, "todo-open:gate-todo:3")
        jumped = session.trace.later("note_open id=gate-todo", before, 8)
        report.check(
            "clicking a task's text opens the note it lives in",
            "source=manager" in jumped,
            jumped.strip() or "no note_open line for the todo's note",
        )
        session.capture("todos: ticked, filtered, jumped")

        # ------------------------------------- the card, from the workbench
        # The frontend panel has had this button all along; in takeover mode that
        # panel is not on screen, so the tray was the only way in. Same request, one
        # window -- `todo_card action=open` is the controller saying so.
        mark = len(session.trace.text())
        # Leave the tab and come back: the rectangles are written when a control
        # appears, and the workbench is still on 待办 from the section above. Clicking
        # 待办 while already there would be a no-op and the fresh report would never
        # happen.
        click_button(session, shell, "tab:notes")
        click_button(session, shell, "tab:todos")
        offered = wininput.wait_for(
            lambda: hit(session.trace.tail(mark), "todo-card") is not None, 4
        )
        report.check(
            "the todo tab offers the desktop card",
            bool(offered),
            "todo-card rect reported" if offered else "no todo-card rect",
        )
        before = session.trace.count("manager_todo_card")
        cards = session.trace.count("todo_card action=open")
        click_button(session, shell, "todo-card")
        asked = session.trace.later("manager_todo_card", before, 5)
        opened = session.trace.later("todo_card action=open", cards, 8)
        card = wininput.wait_for(lambda: user32.FindWindowW(None, CARD_TITLE), 8)
        report.check(
            "the button opens the same card the tray opens",
            bool(asked) and "from=button" in asked and bool(opened) and bool(card),
            (asked.strip() or "no manager_todo_card line")
            + f"; card={'yes' if card else 'no'}"
            + f"; manager={'open' if manager_window() else 'gone'}",
        )
        session.capture("Q16/Q18 + K1 global + new note button + row buttons + card")
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
