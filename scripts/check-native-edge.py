"""Phase 2 gate: parity Q3 -- the note's own edge resizes it, down to the floor.

    Q3  a drag from the right / left / bottom edge, and from the bottom-right
        corner, changes exactly the edges it holds
    Q3  the left edge keeps the right one still, including when it hits the floor
    Q3  a window dragged narrower than 150 x 60 stops at the floor
    Q3  the press that starts a resize does not reach the text (no caret, no
        selection, no body-click side effect)
    Q3  the island follows the window, and C5 remembers the size it ended at

## Why the edges are dragged for real

There is no OS resize loop to test: `WM_NCHITTEST` answers `HTCLIENT` for every
edge of this undecorated window (measured 2026-11-26), which is why the first
version of the C5 gate recorded `drag=programmatic`. Q3 replaces that absence with
a hand-drawn grip, and the only honest evidence for it is `SendInput` on the edge
itself, followed by `GetWindowRect`.

## Why every gesture is asserted per axis

"Resizing works" is not the property; "the edges you are not holding stay still"
is. A gesture that moves the origin and the width by independent amounts passes a
width-only check and still looks broken -- that is the 107-px-for-96-px bug from
`note/drag.rs` in a new place.

Usage:
    python scripts/check-native-edge.py --exe native/target/release/easyflowhub-native.exe
"""

import argparse
import ctypes
import json
import os
import re
import shutil
import subprocess
import sys
import time
from ctypes import wintypes

try:
    user32 = ctypes.WinDLL("user32", use_last_error=True)
except (ImportError, OSError, AttributeError):  # not on Windows
    print("RESULT: SKIP (this gate drives a Windows window)")
    sys.exit(0)

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "lib"))
import wininput  # noqa: E402

NOTE_PREFIX = "easyflowhub-note"

# `note_resize id=0 phase=begin grip=right rect=600x450+200+150`
RESIZE_LINE = re.compile(
    r"note_resize id=(\d+) phase=(begin|end) grip=([a-z-]+) rect=(\d+)x(\d+)\+(-?\d+)\+(-?\d+)"
)
# `island_rect id=0 600x418+0+32`, and the creation line, which spells it `rect=`.
ISLAND_LINE = re.compile(r"island_(?:created|rect) id=0 (?:rect=)?(\d+)x(\d+)\+(-?\d+)\+(-?\d+)")

# `hub-ui::quicknote`'s strip, in logical pixels. The island's top edge is the
# scale-free way to measure `pixels_per_point` -- the C5 and tools gates do the same.
STRIP_HEIGHT = 32.0

# Parity Q3's floor, in logical pixels.
MIN_WIDTH = 150
MIN_HEIGHT = 60

# How long the window must hold one size before C5 writes it (`note/size.rs`).
QUIET = 0.4

user32.SetWindowPos.argtypes = [
    wintypes.HWND,
    wintypes.HWND,
    ctypes.c_int,
    ctypes.c_int,
    ctypes.c_int,
    ctypes.c_int,
    ctypes.c_uint,
]
user32.SetWindowPos.restype = wintypes.BOOL

# `IDC_*` from `winuser.h`: system cursors, so the handle this script loads is the
# same one the app gets and the comparison is meaningful.
IDC_SIZEWE = 32644
IDC_SIZENS = 32645
IDC_SIZENWSE = 32642
IDC_SIZENESW = 32643


class CURSORINFO(ctypes.Structure):
    _fields_ = [
        ("cbSize", wintypes.DWORD),
        ("flags", wintypes.DWORD),
        ("hCursor", wintypes.HANDLE),
        ("ptScreenPos", wintypes.POINT),
    ]


user32.GetCursorInfo.argtypes = [ctypes.POINTER(CURSORINFO)]
user32.LoadCursorW.argtypes = [wintypes.HINSTANCE, ctypes.c_void_p]
user32.LoadCursorW.restype = wintypes.HANDLE


def system_cursor(identifier: int) -> int:
    return user32.LoadCursorW(None, identifier)


def cursor_now() -> int:
    info = CURSORINFO()
    info.cbSize = ctypes.sizeof(CURSORINFO)
    user32.GetCursorInfo(ctypes.byref(info))
    return info.hCursor


def cursor_at(window: int, x: float, y: float) -> int:
    """The cursor shape after hovering a client point for a moment."""
    left, top, _, _ = wininput.client_box(window)
    wininput.move_to(left + x, top + y)
    time.sleep(0.25)
    return cursor_now()


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

    def last(self, pattern):
        for line in reversed(self.text().splitlines()):
            found = pattern.search(line)
            if found:
                return found
        return None

    def all(self, pattern) -> list:
        out = []
        for line in self.text().splitlines():
            found = pattern.search(line)
            if found:
                out.append(found)
        return out


class Session:
    def __init__(self, exe: str, root: str, settings: str, log: str) -> None:
        self.exe = exe
        self.root = root
        self.settings = settings
        self.trace = Trace(log)
        self.process = None

    def start(self) -> int:
        if os.path.exists(self.trace.path):
            os.remove(self.trace.path)
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
        window = wininput.wait_for(lambda: note_window(), 25)
        if window:
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


def note_window() -> int:
    return user32.FindWindowW(None, f"{NOTE_PREFIX}#0")


def box(window: int):
    """Client area as `(x, y, w, h)`, in physical pixels."""
    return wininput.client_box(window) if window else None


def read_settings(path: str):
    try:
        with open(path, encoding="utf-8") as handle:
            return json.load(handle)
    except (OSError, ValueError):
        return None


def place(window: int, x: int, y: int) -> None:
    """Park the window at a known origin so no gesture runs off the desktop."""
    user32.SetWindowPos(window, 0, x, y, 0, 0, 0x0001 | 0x0004 | 0x0010)


def drag_edge(window: int, x: float, y: float, dx: int, dy: int, steps: int = 12) -> None:
    wininput.force(window)
    wininput.drag_client(window, x, y, dx, dy, steps=steps)


def gesture(report: Report, window: int, name: str, start: tuple, delta: tuple) -> tuple:
    """One edge drag; returns `(before, after)` client boxes.

    Retried once only when *nothing* moved: the first synthetic press of a process
    is sometimes swallowed (the same quirk the pin and drag gates absorb), and a
    gate that failed on it would report the harness. A drag that moved by the
    wrong amount is never retried.
    """
    for attempt in range(2):
        before = box(window)
        drag_edge(window, start[0], start[1], delta[0], delta[1])
        time.sleep(0.35)
        after = box(window)
        if after != before or attempt == 1:
            return before, after
        print(f"  note: the {name} gesture produced no change at all; retrying")
    return before, after


def changed(before: tuple, after: tuple) -> tuple:
    """Per-edge change as `(dx, dy, dw, dh)`."""
    return (
        after[0] - before[0],
        after[1] - before[1],
        after[2] - before[2],
        after[3] - before[3],
    )


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--exe", default="native/target/release/easyflowhub-native.exe")
    parser.add_argument("--work", default=os.path.join("native", "target", "q3-edge-gate"))
    args = parser.parse_args()

    exe = os.path.abspath(args.exe)
    if not os.path.exists(exe):
        print(f"missing {exe}; build it with `cargo build --release`")
        return 2

    work = os.path.abspath(args.work)
    root = os.path.join(work, "root")
    settings_path = os.path.join(work, "settings.json")
    shutil.rmtree(work, ignore_errors=True)
    os.makedirs(root, exist_ok=True)

    # A temp settings file, so a resize never touches the live one -- and so the
    # C5 half of the last check has something to read back.
    with open(settings_path, "w", encoding="utf-8") as handle:
        json.dump({"quick_note": {"width": 400, "height": 300}}, handle, indent=2)

    report = Report()
    session = Session(exe, root, settings_path, os.path.join(work, "edge.log"))
    try:
        window = session.start()
        if not window:
            print("the shell never produced a note window; see " + session.trace.path)
            return 2

        startup = box(window)
        position = wininput.wait_for(lambda: session.trace.last(ISLAND_LINE), 10)
        if not startup or not position:
            print(f"no island rectangle in the log; see {session.trace.path}")
            return 2
        scale = int(position.group(4)) / STRIP_HEIGHT
        report.check(
            "Q3 the note opens at the size the settings file asks for",
            abs(startup[2] - round(400 * scale)) <= 2
            and abs(startup[3] - round(300 * scale)) <= 2,
            f"client={startup[2]}x{startup[3]} scale={scale}",
        )

        # Known origin, so every gesture stays on the desktop and none of the
        # travels below is clamped by the injection.
        place(window, 200, 150)
        time.sleep(0.3)
        bounds = box(window)
        left, top, width, height = bounds
        right_edge = width - 2

        # Consume the first press of the process on something harmless: a body
        # press, which is a caret placement and changes no geometry.
        wininput.force(window)
        wininput.drag_client(window, width / 2, height / 2, 2, 2, steps=2)
        time.sleep(0.2)

        # ------------------------------------------------------------ right edge
        before, after = gesture(report, window, "right edge", (right_edge, height / 2), (120, 0))
        dx, dy, dw, dh = changed(before, after)
        report.check(
            "Q3 dragging the right edge widens the window by the cursor's travel",
            abs(dw - 120) <= 2 and dh == 0,
            f"size {before[2]}x{before[3]} -> {after[2]}x{after[3]} (d={dw},{dh})",
        )
        report.check(
            "Q3 the right edge drag leaves the origin alone",
            dx == 0 and dy == 0,
            f"origin {before[:2]} -> {after[:2]}",
        )

        # ------------------------------------------------------------- left edge
        before, after = gesture(report, window, "left edge", (1, height / 2), (-90, 0))
        dx, dy, dw, dh = changed(before, after)
        report.check(
            "Q3 dragging the left edge moves the origin and grows the width",
            abs(dx + 90) <= 2 and abs(dw - 90) <= 2 and dh == 0,
            f"origin {before[:2]} -> {after[:2]}, width {before[2]} -> {after[2]}",
        )
        report.check(
            "Q3 the left edge drag keeps the right edge where it was",
            (before[0] + before[2]) - (after[0] + after[2]) == 0,
            f"right edge {before[0] + before[2]} -> {after[0] + after[2]}",
        )

        # ----------------------------------------------------------- bottom edge
        before, after = gesture(report, window, "bottom edge", (width / 2, height - 2), (0, 70))
        dx, dy, dw, dh = changed(before, after)
        report.check(
            "Q3 dragging the bottom edge makes the window taller and nothing else",
            abs(dh - 70) <= 2 and dw == 0 and dx == 0 and dy == 0,
            f"size {before[2]}x{before[3]} -> {after[2]}x{after[3]} origin {before[:2]} -> {after[:2]}",
        )

        # --------------------------------------------------------------- corner
        current = box(window)
        before, after = gesture(
            report,
            window,
            "bottom-right corner",
            (current[2] - 2, current[3] - 2),
            (60, 40),
        )
        dx, dy, dw, dh = changed(before, after)
        report.check(
            "Q3 the bottom-right corner moves both axes in one gesture",
            abs(dw - 60) <= 2 and abs(dh - 40) <= 2 and dx == 0 and dy == 0,
            f"size {before[2]}x{before[3]} -> {after[2]}x{after[3]}",
        )

        # ---------------------------------------------------- the strip's band
        # The top half of the left edge belongs to egui's strip, not to the island:
        # the same gesture has to work there, and it must not become a window move.
        before, after = gesture(report, window, "strip band", (1, 16 * scale), (80, 0))
        dx, dy, dw, dh = changed(before, after)
        report.check(
            "Q3 the left band inside the strip resizes instead of moving the window",
            abs(dx - 80) <= 2 and abs(dw + 80) <= 2 and dh == 0,
            f"origin {before[:2]} -> {after[:2]}, width {before[2]} -> {after[2]}",
        )

        # ------------------------------------------------------------ the cursor
        # The band a press can start a resize in has to look like one, or the grip
        # exists only for people who already know about it. Two windows answer
        # `WM_SETCURSOR`: the island below the strip, and egui above it.
        current = box(window)
        want_we = system_cursor(IDC_SIZEWE)
        want_ns = system_cursor(IDC_SIZENS)
        want_nwse = system_cursor(IDC_SIZENWSE)
        got = cursor_at(window, current[2] - 2, current[3] / 2)
        report.check(
            "Q3 the right edge advertises a horizontal resize cursor",
            got == want_we,
            f"cursor=0x{got:X} want=0x{want_we:X}",
        )
        got = cursor_at(window, current[2] / 2, current[3] - 2)
        report.check(
            "Q3 the bottom edge advertises a vertical resize cursor",
            got == want_ns,
            f"cursor=0x{got:X} want=0x{want_ns:X}",
        )
        got = cursor_at(window, current[2] - 2, current[3] - 2)
        report.check(
            "Q3 the bottom-right corner advertises the diagonal cursor",
            got == want_nwse,
            f"cursor=0x{got:X} want=0x{want_nwse:X}",
        )
        got = cursor_at(window, 2, 16 * scale)
        report.check(
            "Q3 the band inside the strip advertises it too",
            got == want_we,
            f"cursor=0x{got:X} want=0x{want_we:X}",
        )
        got = cursor_at(window, current[2] / 2, current[3] / 2)
        report.check(
            "Q3 the body keeps the text cursor",
            got not in (want_we, want_ns, want_nwse, system_cursor(IDC_SIZENESW)),
            f"cursor=0x{got:X}",
        )

        # ------------------------------------------- the press is not a body click
        # An edge press is claimed before RichEdit sees it, so Q7's body-click
        # handler must not run. The observable form of that: open the tools panel,
        # drag an edge, and the panel must still be open. A press that fell through
        # would close it (`note_tools open=false reason=body`), which is exactly the
        # panel's own outside-click rule.
        wininput.force(window)
        now = box(window)
        wininput.click_client(window, now[2] / 2, 16 * scale)
        opened = wininput.wait_for(
            lambda: session.trace.count("note_tools id=0 open=true reason=button"), 3
        )
        report.check(
            "Q3 the tools panel is open for the next check",
            bool(opened),
            "note_tools open=true lines="
            f"{session.trace.count('note_tools id=0 open=true reason=button')}",
        )
        body_closes = session.trace.count("note_tools id=0 open=false reason=body")
        current = box(window)
        drag_edge(window, 1, current[3] / 2, 30, 0)
        time.sleep(0.4)
        after_edge = session.trace.count("note_tools id=0 open=false reason=body")
        report.check(
            "Q3 the press that starts a resize is claimed, not a body click",
            after_edge == body_closes,
            f"body closes {body_closes} -> {after_edge}",
        )
        current = box(window)
        wininput.force(window)
        wininput.click_client(window, current[2] / 2, current[3] / 2)
        time.sleep(0.35)
        report.check(
            "Q3 a press in the body still reaches the control",
            session.trace.count("note_tools id=0 open=false reason=body") == body_closes + 1,
            "body closes "
            f"{body_closes} -> {session.trace.count('note_tools id=0 open=false reason=body')}",
        )

        # ------------------------------------------------------------- the floor
        # Last of the gestures: it leaves the window at its smallest, which is the
        # state the island and C5 checks below want anyway.
        current = box(window)
        before, after = gesture(
            report,
            window,
            "floor",
            (current[2] - 2, current[3] / 2),
            (-(current[2] + 40), 0),
        )
        report.check(
            "Q3 a window dragged past the minimum stops at 150 logical pixels",
            abs(after[2] - round(MIN_WIDTH * scale)) <= 2,
            f"width {before[2]} -> {after[2]} (floor {round(MIN_WIDTH * scale)} at scale {scale})",
        )
        report.check(
            "Q3 the floor does not drag the origin along with it",
            after[0] == before[0],
            f"origin.x {before[0]} -> {after[0]}",
        )
        # Height, from the bottom edge, against its own floor.
        current = box(window)
        before, after = gesture(
            report,
            window,
            "height floor",
            (current[2] / 2, current[3] - 2),
            (0, -(current[3] + 40)),
        )
        report.check(
            "Q3 a window dragged shorter than the minimum stops at 60 logical pixels",
            abs(after[3] - round(MIN_HEIGHT * scale)) <= 2,
            f"height {before[3]} -> {after[3]} (floor {round(MIN_HEIGHT * scale)})",
        )

        # ------------------------------------------------------------ the island
        island = session.trace.last(ISLAND_LINE)
        current = box(window)
        report.check(
            "Q3 the island follows the window it was resized inside",
            island is not None
            and abs(int(island.group(1)) - current[2]) <= 2
            and abs(int(island.group(2)) - (current[3] - round(STRIP_HEIGHT * scale))) <= 2,
            f"island={island.groups() if island else None} client={current}",
        )

        # ------------------------------------------------------------------ C5
        # Q3 and C5 compose: the size the edge drag ended at is the size the next
        # note opens at. Waiting on the file, not on a duration -- the write is a
        # temp file plus a rename, so a miss means "try again", not "zero".
        deadline = time.time() + 5.0
        remembered = None
        while time.time() < deadline:
            note = (read_settings(settings_path) or {}).get("quick_note") or {}
            if note.get("width") not in (None, 400):
                remembered = (note.get("width"), note.get("height"))
                break
            time.sleep(0.1)
        current = box(window)
        report.check(
            "Q3 the size the resize ends at is the size C5 remembers",
            remembered is not None
            and abs(remembered[0] - round(current[2] / scale)) <= 2
            and abs(remembered[1] - round(current[3] / scale)) <= 2,
            f"file={remembered} client={current[2]}x{current[3]} scale={scale}",
        )

        # ------------------------------------------------- the log lines agree
        begins = [m for m in session.trace.all(RESIZE_LINE) if m.group(2) == "begin"]
        ends = [m for m in session.trace.all(RESIZE_LINE) if m.group(2) == "end"]
        grips = {m.group(3) for m in begins}
        report.check(
            "Q3 every gesture was started and ended, with the edges it held",
            len(begins) == len(ends) and len(begins) >= 6,
            f"begin={len(begins)} end={len(ends)} grips={sorted(grips)}",
        )
        report.check(
            "Q3 the log names all four grips the gate used",
            {"right", "left", "bottom", "bottom-right"} <= grips,
            f"grips={sorted(grips)}",
        )
    finally:
        session.stop()

    return report.finish()


if __name__ == "__main__":
    sys.exit(main())
