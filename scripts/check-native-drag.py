"""Phase 2 gate: parity Q10 -- the strip is a title bar, the buttons are not.

    Q10  a drag from the left slot, the middle slot and the right slot each move the
         window by exactly the distance the cursor travelled
    Q10  a drag that starts on `•••` or on the pin moves nothing and toggles nothing
         (a press that travels is not a click)

Both halves are judged from `GetWindowRect`, not from the log: "the window follows"
is a statement about the window, and a log line saying `phase=begin` would pass
while the window sat still. That is not hypothetical -- `ViewportCommand::StartDrag`
logged the drag and moved nothing (see `app/src/note/drag.rs`).

The drag itself is real input: `SendInput` press, a dozen move steps, release. The
steps are what the app follows; one jump would also produce the movement, but it
would not distinguish "following the cursor" from "being teleported once".

Where the slots are: the point is expressed in client coordinates as a fraction of
the strip, and the three chosen x values are the label, an empty part of the middle
slot, and the gap left of the pin. The centre of the strip is deliberately *not*
used -- that is the `•••` button, and the last two checks prove it is not a drag
handle.

Usage:
    python scripts/check-native-drag.py --exe native/target/release/easyflowhub-native.exe
"""

import argparse
import ctypes
import os
import re
import shutil
import subprocess
import sys
import tempfile
import time
from ctypes import wintypes

try:
    import ctypes

    user32 = ctypes.WinDLL("user32", use_last_error=True)
except (ImportError, OSError, AttributeError):  # not on Windows
    print("RESULT: SKIP (this gate drives a Windows window)")
    sys.exit(0)

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "lib"))
import wininput  # noqa: E402  (path first, then the import)

NOTE_PREFIX = "easyflowhub-note"
ISLAND_TOP = re.compile(r"island_(?:created|rect) id=0 (?:rect=)?\d+x\d+\+\d+\+(-?\d+)")

# Geometry from `hub-ui::quicknote`, in logical pixels.
BUTTON_SIZE = 28.0
BUTTON_INSET = 8.0
STRIP_HEIGHT = 32.0

# How far each drag travels. Large enough to be unmistakable at 150%, small enough
# that three of them in one direction stay on a 4K desktop.
TRAVEL = 96


def note_window(index: int) -> int:
    return user32.FindWindowW(None, f"{NOTE_PREFIX}#{index}")


def origin(hwnd: int) -> tuple:
    """`(left, top)` of the window rectangle, in screen pixels."""
    rect = wintypes.RECT()
    user32.GetWindowRect(hwnd, ctypes.byref(rect))
    return rect.left, rect.top


def island_tops(text: str) -> list:
    return [int(match.group(1)) for match in ISLAND_TOP.finditer(text)]


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


class Session:
    def __init__(self, exe: str, root: str, log: str) -> None:
        self.exe = exe
        self.root = root
        self.log = log
        self.trace = Trace(log)
        self.process = None

    def start(self) -> int:
        self.process = subprocess.Popen(
            [self.exe, "--note", "--store-root", self.root, "--log", self.log],
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


def safe_delta(hwnd: int) -> tuple:
    """`(dx, dy)` heading for the middle of the desktop, so nothing is clamped."""
    left, top = origin(hwnd)
    width, height = user32.GetSystemMetrics(0), user32.GetSystemMetrics(1)
    dx = -TRAVEL if left > width / 2 else TRAVEL
    dy = -TRAVEL if top > height / 2 else TRAVEL
    return dx, dy


def drag_and_check(report: Report, hwnd: int, label: str, x: float, y: float, delta: tuple) -> bool:
    """One measured drag. Retried only when the window did not move *at all*.

    The retry is not cosmetic: the very first synthetic press after the window
    appears is sometimes swallowed (the same first-interaction quirk the pin gate
    retries around), and `warm_up` below usually absorbs it. A drag that moves the
    window by the wrong distance is never retried -- that is the failure this gate
    exists to catch, and retrying it would hide the 107-px-for-96-px class of bug
    that the first implementation of `note/drag.rs` had.
    """
    for attempt in range(2):
        before = origin(hwnd)
        wininput.force(hwnd)
        wininput.drag_client(hwnd, x, y, delta[0], delta[1])
        time.sleep(0.4)
        after = origin(hwnd)
        moved = (after[0] - before[0], after[1] - before[1])
        if moved != (0, 0) or attempt == 1:
            break
        print(f"  note: the {label} drag produced no movement at all; retrying")
    # One pixel of slack: the injection quantises the cursor to whole pixels, and at
    # 150% the window origin is a rounded version of the same point.
    ok = abs(moved[0] - delta[0]) <= 1 and abs(moved[1] - delta[1]) <= 1
    return report.check(
        f"Q10 a drag from the {label} moves the window by the cursor's travel",
        ok,
        f"want={delta} got={moved} origin {before} -> {after}",
    )


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--exe", default="native/target/release/easyflowhub-native.exe")
    parser.add_argument("--work", default=os.path.join(tempfile.gettempdir(), "p2-drag-gate"))
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
    shutil.copyfile(live_db, os.path.join(root, "notes.db"))
    log_path = os.path.join(work, "native-drag.log")
    if os.path.exists(log_path):
        os.remove(log_path)

    report = Report()
    session = Session(exe, root, log_path)
    try:
        window = session.start()
        if not window:
            print("the shell never produced a note window; see " + log_path)
            return 2

        tops = wininput.wait_for(lambda: island_tops(session.trace.text()) or None, 10) or []
        strip = tops[-1] if tops else 0
        _, _, width, height = wininput.client_box(window)
        if not (0 < strip < height):
            print(f"no strip height in the log (tops={tops}); see {log_path}")
            return 2

        scale = strip / STRIP_HEIGHT
        pin_left = width - BUTTON_INSET * scale - BUTTON_SIZE * scale
        delta = safe_delta(window)
        middle_y = strip / 2.0

        # Consume the first press of the process on something whose result is not
        # measured: it is sometimes swallowed, and a gate that failed on it would be
        # reporting the harness, not the product.
        wininput.force(window)
        wininput.drag_client(window, width * 0.35, middle_y, delta[0] // 8, delta[1] // 8)
        time.sleep(0.3)
        begins = session.trace.count("note_drag id=0 phase=begin")
        ends = session.trace.count("note_drag id=0 phase=end")

        drag_and_check(report, window, "left slot", 16 * scale, middle_y, delta)
        drag_and_check(report, window, "middle slot", width * 0.35, middle_y, delta)
        drag_and_check(report, window, "right slot", pin_left - 30 * scale, middle_y, delta)

        report.check(
            "Q10 every drag was started and ended",
            session.trace.count("note_drag id=0 phase=begin") == begins + 3
            and session.trace.count("note_drag id=0 phase=end") == ends + 3,
            f"begin={session.trace.count('note_drag id=0 phase=begin') - begins} "
            f"end={session.trace.count('note_drag id=0 phase=end') - ends} (want 3/3)",
        )

        # ------------------------------------------- buttons are not drag handles
        before = origin(window)
        pins = session.trace.count("note_pin id=0")
        tools = session.trace.count("note_tools id=0")
        wininput.force(window)
        wininput.drag_client(
            window,
            pin_left + BUTTON_SIZE * scale / 2,
            middle_y,
            delta[0],
            delta[1],
        )
        time.sleep(0.4)
        report.check(
            "Q10 dragging from the pin moves nothing",
            origin(window) == before,
            f"origin {before} -> {origin(window)}",
        )
        report.check(
            "Q10 dragging from the pin is not a click (no pin toggle)",
            session.trace.count("note_pin id=0") == pins,
            f"note_pin lines {pins} -> {session.trace.count('note_pin id=0')}",
        )

        before = origin(window)
        wininput.force(window)
        wininput.drag_client(window, width / 2, middle_y, delta[0], delta[1])
        time.sleep(0.4)
        report.check(
            "Q10 dragging from the tools button moves nothing",
            origin(window) == before,
            f"origin {before} -> {origin(window)}",
        )
        report.check(
            "Q10 dragging from the tools button does not open the panel",
            session.trace.count("note_tools id=0") == tools,
            f"note_tools lines {tools} -> {session.trace.count('note_tools id=0')}",
        )
        report.check(
            "Q10 the note is still alive and still where the drags left it",
            session.process.poll() is None
            and origin(window) == before
            and bool(user32.IsWindowVisible(window)),
            f"alive={session.process.poll() is None} origin={origin(window)}",
        )
    finally:
        session.stop()

    return report.finish()


if __name__ == "__main__":
    sys.exit(main())
