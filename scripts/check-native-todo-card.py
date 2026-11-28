"""Phase 2 gate: the native desktop todo card is a real window over the real list.

    the card opens as `easyflowhub-todo-card`, 320x400 logical, borderless
    the list is the seeded notes, grouped, open items before done ones
    one real click on a checkbox writes `- [x] ... @done:<iso>` to `notes.db`
    the row looks different afterwards, measured from pixels, not from the log
    a second click takes the tick and the stamp back off
    clicking the text edits it, and Enter writes the new text in place
    Esc closes the card and leaves the process alive (the tray is still there)
    the tray's "待办卡片" opens it again -- the menu path is wired, not only the flag

Why the pixels: the write assertions only prove the database changed. A card that
ticks the row but keeps painting it as open would pass every log check and be
unusable. The tick and the strike-through are two visual facts, so two pixel
checks: the checkbox centre changes state, and the text loses its ink.

Why the second open goes through `WM_COMMAND`: `--todo-card` proves the flag works.
The tray is the path a user has, and its ids are a stable table in `hub-win::tray`,
so the gate pokes the same id the menu does instead of opening the menu.

Usage:
    python scripts/check-native-todo-card.py --exe native/target/debug/easyflowhub-native.exe
"""

import argparse
import ctypes
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
    user32 = ctypes.WinDLL("user32", use_last_error=True)
except (ImportError, OSError, AttributeError):  # not on Windows
    print("RESULT: SKIP (this gate drives a Windows window)")
    sys.exit(0)

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "lib"))
import wininput  # noqa: E402
import winshot  # noqa: E402

# Signed explicitly: an untyped `FindWindowW` returns a 32-bit int, and a window
# handle above 2 GB would be truncated into "some other window".
user32.FindWindowW.restype = wintypes.HWND
user32.FindWindowW.argtypes = [wintypes.LPCWSTR, wintypes.LPCWSTR]
user32.PostMessageW.argtypes = [wintypes.HWND, wintypes.UINT, wintypes.WPARAM, wintypes.LPARAM]

CARD_TITLE = "easyflowhub-todo-card"
TRAY_CLASS = "EasyFlowHubTray"
WM_COMMAND = 0x0111
MENU_TODO_CARD = 4
VK_ESCAPE = 0x1B
VK_RETURN = 0x0D
VK_A = 0x41

# The window the product opens: `hub_ui::todocard::{WIDTH,HEIGHT}`.
CARD_W, CARD_H = 320.0, 400.0

# The done row sits *between* two open ones on purpose: a list that only echoed the
# body order would pass, and the grouping is the thing being tested.
SEED = [
    ("inbox", "待办箱", "- [ ] 买菜\n- [x] 倒垃圾\n- [ ] 交电费"),
    ("proj", "项目", "# 项目\n- [ ] 写完门禁\n- [x] 评审"),
]

HIT = re.compile(
    r"todocard_hit note=(\S+) line=(\d+) x=(-?\d+) y=(-?\d+) w=(\d+) h=(\d+)"
)
RELOAD = re.compile(r"todocard_reload store=on groups=(\d+) open=(\d+) done=(\d+)")


def card_window():
    return user32.FindWindowW(None, CARD_TITLE) or 0


def tray_window():
    return user32.FindWindowW(TRAY_CLASS, None) or 0


def client_size(hwnd):
    rect = wintypes.RECT()
    user32.GetClientRect(hwnd, ctypes.byref(rect))
    return rect.right - rect.left, rect.bottom - rect.top


class Report:
    def __init__(self):
        self.results = []

    def check(self, name, ok, detail):
        self.results.append((name, bool(ok), detail))
        print(f"{'PASS' if ok else 'FAIL'}  {name}: {detail}")
        return bool(ok)

    def finish(self):
        failed = [name for name, ok, _ in self.results if not ok]
        print()
        print(f"{len(self.results) - len(failed)}/{len(self.results)} checks passed")
        if failed:
            print("failed: " + ", ".join(failed))
            print("RESULT: FAIL")
            return 1
        print("RESULT: PASS")
        return 0


class Trace:
    def __init__(self, path):
        self.path = path

    def text(self):
        if not os.path.exists(self.path):
            return ""
        with open(self.path, encoding="utf-8", errors="replace") as handle:
            return handle.read()

    def count(self, needle):
        return sum(1 for line in self.text().splitlines() if needle in line)

    def later(self, needle, before, timeout):
        deadline = time.time() + timeout
        while time.time() < deadline:
            seen = [line for line in self.text().splitlines() if needle in line]
            if len(seen) > before:
                return seen[before]
            time.sleep(0.05)
        return ""


class Session:
    def __init__(self, exe, root, log):
        self.exe = exe
        self.root = root
        self.log = log
        self.trace = Trace(log)
        self.process = None

    def start(self):
        self.process = subprocess.Popen(
            [self.exe, "--resident", "--todo-card", "--store-root", self.root, "--log", self.log],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        return wininput.wait_for(card_window, 25)

    def alive(self):
        return self.process is not None and self.process.poll() is None

    def stop(self):
        if self.process is None:
            return
        if self.process.poll() is None:
            self.process.terminate()
            try:
                self.process.wait(timeout=10)
            except subprocess.TimeoutExpired:
                self.process.kill()
                self.process.wait(timeout=5)
        wininput.wait_for(lambda: card_window() == 0, 5)
        self.process = None


def seed(root):
    """Copy the live schema and replace its rows with the gate's list."""
    live = os.path.join(os.environ["APPDATA"], "com.yukin.easyflowhub", "notes.db")
    if not os.path.exists(live):
        print(f"missing {live}; run the shipped build once so there is a schema to copy")
        return None
    db = os.path.join(root, "notes.db")
    shutil.copyfile(live, db)
    con = sqlite3.connect(db)
    con.execute("DELETE FROM notes")
    stamp = "2026-11-28T04:00:00+00:00"
    for note_id, title, content in SEED:
        con.execute(
            "INSERT INTO notes (id,title,content,created_at,updated_at,is_pinned,tags,deleted_at)"
            " VALUES (?,?,?,?,?,0,'',NULL)",
            (note_id, title, content, stamp, stamp),
        )
    con.commit()
    con.close()
    return db


def wait_hit(session, note, line, not_y=None, timeout=4):
    """The newest rectangle for a row, optionally one that is not at `not_y`.

    A tick moves its row from the open block to the done block, so the card rewrites
    the hits with new y values. Sampling the old position would measure the row that
    took its place -- measured: the second click landed on a different task.
    """
    deadline = time.time() + timeout
    while time.time() < deadline:
        for hit in hits(session.trace.text()):
            if hit["note"] == note and hit["line"] == line and hit["y"] != not_y:
                return hit
        time.sleep(0.05)
    return None


def body(db, note_id):
    con = sqlite3.connect(db)
    row = con.execute("SELECT content FROM notes WHERE id=?", (note_id,)).fetchone()
    con.close()
    return row[0] if row else ""


def hits(text):
    """The latest rectangle per todo.

    The card rewrites every hit after a reload, and a tick moves its row from the
    open block to the done block -- so the first occurrence is the position the row
    *used* to have. Later lines overwrite earlier ones; the dict keeps the order of
    first appearance, which is the card's own reading order.
    """
    latest = {}
    for match in HIT.finditer(text):
        hit = {
            "note": match.group(1),
            "line": int(match.group(2)),
            "x": int(match.group(3)),
            "y": int(match.group(4)),
            "w": int(match.group(5)),
            "h": int(match.group(6)),
        }
        latest[(hit["note"], hit["line"])] = hit
    return list(latest.values())


def ink(hwnd, x, y, half):
    """Bright pixels in a box: the text's own ink, at this monitor scale."""
    width, _, raw = winshot.client_pixels(hwnd)
    if not raw:
        return -1
    return winshot.ink_count(raw, width, x, y, half)


def centre_rgb(hwnd, x, y):
    width, _, raw = winshot.client_pixels(hwnd)
    if not raw:
        return None
    return winshot.rgb(raw, width, x, y)


def click(hwnd, hit):
    wininput.force(hwnd)
    wininput.click_client(hwnd, hit["x"] + hit["w"] / 2, hit["y"] + hit["h"] / 2)
    time.sleep(0.35)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--exe", default="native/target/debug/easyflowhub-native.exe")
    parser.add_argument("--work", default=os.path.join(tempfile.gettempdir(), "p2-todo-card-gate"))
    args = parser.parse_args()

    exe = os.path.abspath(args.exe)
    if not os.path.exists(exe):
        print(f"missing {exe}; build it with `cargo build -p app`")
        return 2

    work = os.path.abspath(args.work)
    root = os.path.join(work, "root")
    os.makedirs(root, exist_ok=True)
    db = seed(root)
    if not db:
        return 2
    log_path = os.path.join(work, "native-todo-card.log")
    if os.path.exists(log_path):
        os.remove(log_path)

    report = Report()
    session = Session(exe, root, log_path)
    try:
        window = session.start()
        if not window:
            print("the card never appeared; see " + log_path)
            print("RESULT: FAIL")
            return 1

        # The hits are written once the card has laid its list out, which is the
        # frame after the window exists -- not the instant it is found. All five,
        # because the lines are appended one by one and a half-written frame reads
        # as a card with two rows.
        wininput.wait_for(lambda: session.trace.text().count("todocard_hit") >= 5, 15)
        text = session.trace.text()
        reloaded = RELOAD.search(text)
        report.check(
            "the seeded list is read once at open",
            reloaded is not None
            and (int(reloaded.group(1)), int(reloaded.group(2)), int(reloaded.group(3)))
            == (2, 3, 2),
            f"todocard_reload groups=open/done {reloaded.groups() if reloaded else 'missing'}",
        )

        width, height = client_size(window)
        scale = height / CARD_H if height else 1.5
        report.check(
            "the card is the shipped 320x400, borderless",
            abs(width - CARD_W * scale) <= 2 and abs(height - CARD_H * scale) <= 2,
            f"client={width}x{height} at scale {scale:.2f} (no frame pixels included)",
        )

        listed = hits(text)
        keys = {(hit["note"], hit["line"]) for hit in listed}
        report.check(
            "every seeded todo is on the card",
            keys == {("inbox", 0), ("inbox", 1), ("inbox", 2), ("proj", 1), ("proj", 2)},
            f"{len(listed)} rows: {sorted(keys)}",
        )
        order = [hit["line"] for hit in listed if hit["note"] == "inbox"]
        report.check(
            "open items come before done ones",
            order == [0, 2, 1],
            f"inbox rows in card order: {order} (the done line 1 is last)",
        )

        first = next(hit for hit in listed if hit["note"] == "inbox" and hit["line"] == 0)
        text_box = (
            first["x"] + first["w"] + 8,
            first["y"] + first["h"] // 2,
        )
        before_ink = ink(window, text_box[0] + 26, text_box[1], 12)
        before_dot = centre_rgb(window, first["x"] + first["w"] // 2, first["y"] + first["h"] // 2)

        before = session.trace.count("todocard_toggle")
        click(window, first)
        line = session.trace.later("todocard_toggle", before, 3)
        report.check(
            "a real click on the checkbox ticks the row",
            "done=true" in line and "created=" in line and "error=" not in line,
            line.strip() or "no todocard_toggle line",
        )
        report.check(
            "the tick is written to the note body with a timestamp",
            body(db, "inbox").splitlines()[0].startswith("- [x] 买菜 @done:20"),
            f"line 0 = {body(db, 'inbox').splitlines()[0]!r}",
        )

        # The tick moves the row to the done block. Measure *there*, with the
        # rectangle the card reported after the reload.
        done_hit = wait_hit(session, "inbox", 0, not_y=first["y"]) or first
        after_ink = ink(window, done_hit["x"] + done_hit["w"] + 34, done_hit["y"] + done_hit["h"] // 2, 12)
        after_dot = centre_rgb(
            window, done_hit["x"] + done_hit["w"] // 2, done_hit["y"] + done_hit["h"] // 2
        )
        report.check(
            "the finished row's text loses its ink",
            before_ink > 0 and after_ink >= 0 and after_ink < before_ink * 0.7,
            f"bright pixels in the text box: {before_ink} -> {after_ink}",
        )
        report.check(
            "the checkbox itself changes state",
            before_dot is not None and after_dot is not None and before_dot != after_dot,
            f"checkbox centre: {before_dot} -> {after_dot}",
        )

        before = session.trace.count("todocard_toggle")
        click(window, done_hit)
        line = session.trace.later("todocard_toggle", before, 3)
        report.check(
            "a second click takes the tick back off",
            "done=false" in line and "error=" not in line,
            line.strip() or "no second todocard_toggle line",
        )
        report.check(
            "the unstamped line is exactly what the frontend would write",
            body(db, "inbox").splitlines()[0] == "- [ ] 买菜",
            f"line 0 = {body(db, 'inbox').splitlines()[0]!r}",
        )

        # Editing: click the text (not the box), select all, type, Enter.
        target = wait_hit(session, "proj", 1)
        wininput.force(window)
        wininput.click_client(
            window, target["x"] + target["w"] + 60, target["y"] + target["h"] / 2
        )
        time.sleep(0.4)
        wininput.chord(VK_A, ctrl=True)
        time.sleep(0.1)
        wininput.type_text("gate-ok")
        wininput.tap(VK_RETURN)
        line = session.trace.later("todocard_commit", 0, 4)
        report.check(
            "editing the text writes it in place",
            "id=proj line=1 created=false" in line and "error=" not in line,
            line.strip() or "no todocard_commit line",
        )
        report.check(
            "the rewritten line keeps its marker and its state",
            body(db, "proj").splitlines()[1] == "- [ ] gate-ok",
            f"line 1 = {body(db, 'proj').splitlines()[1]!r}",
        )

        wininput.force(window)
        wininput.tap(VK_ESCAPE)
        report.check(
            "Esc closes the card",
            wininput.wait_for(lambda: card_window() == 0, 5),
            "the window is gone",
        )
        time.sleep(0.8)
        report.check(
            "closing the card does not end the process",
            session.alive(),
            "the shell is still running: the tray is the last surface",
        )

        tray = tray_window()
        report.check("the tray exists", tray != 0, f"tray hwnd=0x{tray:X}")
        before = session.trace.count("todo_card action=open")
        # Count from *here*: every write above reloaded too, so "the second reload
        # line" would be the one after the first tick.
        reloads = session.trace.count("todocard_reload")
        if tray:
            user32.PostMessageW(tray, WM_COMMAND, MENU_TODO_CARD, 0)
        reopened = wininput.wait_for(card_window, 8)
        report.check(
            "the tray's 待办卡片 opens it again",
            bool(reopened) and session.trace.count("todo_card action=open") > before,
            f"hwnd=0x{reopened:X}" if reopened else "the card did not come back",
        )
        reopened_reload = session.trace.later("todocard_reload", reloads, 3)
        report.check(
            "the second open is a fresh read, not a stale list",
            bool(reopened_reload) and "open=3 done=2" in reopened_reload,
            reopened_reload.strip() or "no second todocard_reload line",
        )
    finally:
        session.stop()

    return report.finish()


if __name__ == "__main__":
    sys.exit(main())
