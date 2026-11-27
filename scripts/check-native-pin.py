"""Phase 2 gate: parity Q9 -- the pin button pins, unpins, and says which it is.

    Q9  the note starts pinned: `WS_EX_TOPMOST` on, the bookmark filled
    Q9  one real click on the pin unpins it: topmost off, the bookmark hollow
    Q9  the highlight tracks the state, not the click: it comes back on the
        second click, and the window is in the same place both times
    Q9  the note is still there afterwards -- the state that used to break

The third line is the point of this script. `ViewportCommand::WindowLevel` looked
implemented (the log said `pinned=false`) and it **hid the window**: winit applies a
level change by rebuilding the whole style from its cached `WindowFlags`, and this
window is created `with_visible(false)` and shown by `move_on_screen`. So the click
threw the note off the desktop, and a log-only gate would have called it a pass.
The fix is `hub_win::window::set_topmost`, and this gate would have caught the old
one at the first pixel grab.

Two things about the judgement:

**"Covered by other windows" is decided by the OS, not by pushing another window
around.** `WS_EX_TOPMOST` *is* the difference between "stays above" and "can be
covered"; a gate that drags some other app in front would be measuring the shell,
not the product, and would fail whenever the desktop layout is odd. So the bit is
read from the window itself, and the pixel check answers the other half of Q9 --
whether the icon *looks* different.

**The icon is judged by fill, not by colour.** Pinned is a filled bookmark, floating
is its outline; counting bright pixels in a small box centred on the icon separates
the two by an order of magnitude (measured 121 vs 6 at 150%), and it keeps working
if the palette is restyled. The colour is logged for a human reading a failure.

Usage:
    python scripts/check-native-pin.py --exe native/target/release/easyflowhub-native.exe
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

try:
    import ctypes

    user32 = ctypes.WinDLL("user32", use_last_error=True)
except (ImportError, OSError, AttributeError):  # not on Windows
    print("RESULT: SKIP (this gate drives a Windows window)")
    sys.exit(0)

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "lib"))
import wininput  # noqa: E402  (path first, then the import)
import winshot  # noqa: E402

NOTE_PREFIX = "easyflowhub-note"
GWL_EXSTYLE = -20
WS_EX_TOPMOST = 0x00000008

# `island_created` carries the first layout as `rect=WxH+X+Y`; later moves come from
# `island_rect` as `WxH+X+Y`. Only the top edge is wanted: it is the strip height,
# and the strip height is what the button's vertical centre is derived from.
ISLAND_TOP = re.compile(r"island_(?:created|rect) id=0 (?:rect=)?\d+x\d+\+\d+\+(-?\d+)")

# Geometry from `hub-ui::quicknote`: a 28 px button box whose right edge is 8 px in
# from the window edge, centred on the 32 px strip. Kept as constants because the
# gate must click where a user clicks; if these change, the click landing nowhere
# is the signal.
BUTTON_SIZE = 28.0
BUTTON_INSET = 8.0
STRIP_HEIGHT = 32.0

# Fill vs outline, in an 11 px box centred on the icon. Measured 121 / 6 at 150%;
# the gap is wide on purpose so a font or palette change cannot flip the verdict.
FILLED_AT_LEAST = 60
HOLLOW_AT_MOST = 30


def note_window(index: int) -> int:
    return user32.FindWindowW(None, f"{NOTE_PREFIX}#{index}")


def island_tops(text: str) -> list:
    return [int(match.group(1)) for match in ISLAND_TOP.finditer(text)]


def topmost(hwnd: int) -> bool:
    style = user32.GetWindowLongW(hwnd, GWL_EXSTYLE) & 0xFFFFFFFF
    return bool(style & WS_EX_TOPMOST)


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

    def start(self):
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


class PinButton:
    """Where the button is, and what it currently looks like."""

    # Two synthetic clicks at the same spot closer than the system double-click
    # time are delivered as *one* double-click, and egui's `clicked()` is false for
    # the second half -- measured here at 0.4 s gaps: 3 of 8 clicks vanished. The
    # button is 42 physical px wide, so alternating 8 px either side of its centre
    # leaves the 4 px double-click box *and* stays on the button.
    MIN_GAP = 0.6
    NUDGE = 8.0

    def __init__(self, hwnd: int, strip: int, width: int) -> None:
        self.hwnd = hwnd
        self.scale = strip / STRIP_HEIGHT
        self.size = BUTTON_SIZE * self.scale
        self.x = width - BUTTON_INSET * self.scale - self.size
        self.y = strip / 2.0 - self.size / 2.0
        # A 7 px (11 physical at 150%) box inside the button: the bookmark's body.
        self.half = max(3, int(round(5.0 * self.scale / 1.5)))
        self.last_click = 0.0
        self.side = 1.0

    def click(self) -> None:
        """Click the button, alternating sides so no two clicks pair up."""
        waited = time.time() - self.last_click
        if waited < self.MIN_GAP:
            time.sleep(self.MIN_GAP - waited)
        wininput.force(self.hwnd)
        self.side = -self.side
        wininput.click_client(
            self.hwnd,
            self.x + self.size / 2 + self.side * self.NUDGE,
            self.y + self.size / 2,
        )
        self.last_click = time.time()

    def look(self) -> tuple:
        """`(filled_pixels, centre_rgb)` for the current frame."""
        width, _, raw = winshot.client_pixels(self.hwnd)
        if not raw:
            return 0, (0, 0, 0)
        centre_x = int(self.x + self.size / 2)
        centre_y = int(self.y + self.size / 2)
        filled = winshot.ink_count(raw, width, centre_x, centre_y, self.half)
        return filled, winshot.rgb(raw, width, centre_x, centre_y)


def click_for_line(session: Session, button: PinButton) -> str:
    """Click until the log shows the change, never more than once per attempt."""
    for attempt in range(4):
        before = session.trace.count("note_pin id=0")
        button.click()
        line = session.trace.later("note_pin id=0", before, 1.5)
        if line:
            return line
        print(f"  note: pin click attempt {attempt + 1} produced nothing; retrying")
    return ""


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--exe", default="native/target/release/easyflowhub-native.exe")
    parser.add_argument("--work", default=os.path.join(tempfile.gettempdir(), "p2-pin-gate"))
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
    log_path = os.path.join(work, "native-pin.log")
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
        button = PinButton(window, strip, width)

        # ------------------------------------------------------------ starts pinned
        filled, centre = button.look()
        report.check(
            "Q9 the note starts pinned (OS topmost bit)",
            topmost(window),
            f"WS_EX_TOPMOST={topmost(window)}",
        )
        report.check(
            "Q9 the pinned icon is the filled bookmark",
            filled >= FILLED_AT_LEAST,
            f"filled pixels={filled} (needs >={FILLED_AT_LEAST}) centre={centre}",
        )

        # ---------------------------------------------------------------- unpin
        unpinned = click_for_line(session, button)
        report.check(
            "Q9 a click on the pin unpins the note",
            "pinned=false" in unpinned and "topmost=true" in unpinned,
            unpinned.strip() or "no note_pin line",
        )
        report.check(
            "Q9 the OS dropped the topmost bit",
            not topmost(window),
            f"WS_EX_TOPMOST={topmost(window)} after the click",
        )

        hollow = wininput.wait_for(
            lambda: (lambda look: look if look[0] <= HOLLOW_AT_MOST else None)(
                button.look()
            ),
            3,
        )
        filled_now, centre_now = hollow or button.look()
        report.check(
            "Q9 the icon went hollow (the highlight follows the state, not the click)",
            bool(hollow) and filled_now <= HOLLOW_AT_MOST,
            f"filled pixels={filled_now} (needs <={HOLLOW_AT_MOST}) centre={centre_now}",
        )

        # ------------------------------------- the window is still on the desktop
        visible = bool(user32.IsWindowVisible(window))
        left, top, box_w, box_h = wininput.client_box(window)
        report.check(
            "Q9 the note survived the unpin (this is what used to break)",
            visible and (box_w, box_h) == (width, height) and session.process.poll() is None,
            f"visible={visible} client={box_w}x{box_h} (was {width}x{height})",
        )
        report.check(
            "Q9 unpinning did not close the note",
            session.trace.count("note_close") == 0,
            f"note_close lines={session.trace.count('note_close')}",
        )

        # --------------------------------------------------------------- repin
        repinned = click_for_line(session, button)
        report.check(
            "Q9 a second click pins it again",
            "pinned=true" in repinned,
            repinned.strip() or "no note_pin line",
        )
        report.check(
            "Q9 the OS took the topmost bit back",
            topmost(window),
            f"WS_EX_TOPMOST={topmost(window)}",
        )

        again = wininput.wait_for(
            lambda: (lambda look: look if look[0] >= FILLED_AT_LEAST else None)(
                button.look()
            ),
            3,
        )
        report.check(
            "Q9 the filled highlight came back",
            bool(again),
            f"filled pixels={(again or button.look())[0]} (needs >={FILLED_AT_LEAST})",
        )
        report.check(
            "Q9 the pin click never moved the window",
            wininput.client_box(window)[:2] == (left, top),
            f"origin={wininput.client_box(window)[:2]} was {(left, top)}",
        )
    finally:
        session.stop()

    return report.finish()


if __name__ == "__main__":
    sys.exit(main())
