"""Enhancement C5: the size a note is dragged to is the size the next one opens at.

    C5  a resize writes `quick_note.width` (and keeps the other keys)
    C5  the file gains a `schema` number when it is written
    C5  a launch that is never resized leaves the file byte-identical
    C5  the next process opens the note at the remembered width
    C5  the window is not snapped back to the startup size after the drag

One size for every note, per parity C5: the point is not that *this* window
remembers, it is that the *next* one inherits it. So the second process is the
real assertion -- the file could be written correctly and never read back.

## Why the drag is attempted for real, with a fallback

The behaviour under test is the application's: observe the client area, wait for
it to settle, persist it. Dragging the window's edge with `SendInput` is the
honest input, and the gate tries it first. Whether the edge of an *undecorated*
winit window is a resize hit-test area is the OS/winit's business, and it has
already been wrong once on this machine (the title-strip drag does not start an
OS move loop). If the real drag does not change the size, `SetWindowPos` produces
the same `WM_SIZE` and the same new client area, and the log records which route
was used (`drag=real|programmatic`) so the evidence is never overstated.

## Why the settings file is written by this script

`--settings` points at a temp file, so the live `%APPDATA%` settings are never
touched -- and the file can be seeded with no `schema` key, which is exactly the
shape every settings file had before this change. The first write is then the
one that proves a missing version is treated as version 1 rather than refused.

Usage:
    python scripts/check-native-size.py --exe native/target/release/easyflowhub-native.exe
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

# How long the window must hold one size before it is written (`note/size.rs`),
# plus the time a frame needs to notice. The gate waits on the file, not on this.
QUIET = 0.4
SETTLE = 5.0

SIZE_LINE = re.compile(
    r"note_size id=(\d+) logical=(\d+)x(\d+) from=(\d+)x(\d+) saved=(true|false) schema=(\d+)"
)

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


class Session:
    def __init__(self, exe: str, root: str, settings: str, log: str) -> None:
        self.exe = exe
        self.root = root
        self.settings = settings
        self.trace = Trace(log)
        self.process = None

    def start(self) -> int:
        # The log belongs to one process: `Log::new` truncates. Keeping the old one
        # around would make a line from the previous run answer for this one.
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
        window = wininput.wait_for(note_window, 25)
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
    """Client area as `(x, y, w, h)` in physical pixels, or `None`."""
    return wininput.client_box(window) if window else None


def read_settings(path: str):
    try:
        with open(path, encoding="utf-8") as handle:
            return json.load(handle)
    except (OSError, ValueError):
        return None


def size_of(settings):
    """`(width, height)` from a settings document, or `None`."""
    if not isinstance(settings, dict):
        return None
    note = settings.get("quick_note") or {}
    width, height = note.get("width"), note.get("height")
    if isinstance(width, int) and isinstance(height, int):
        return (width, height)
    return None


def try_drag(window: int, dx: int) -> bool:
    """Drag the right edge by `dx` physical pixels. True if the size changed."""
    before = box(window)
    if not before:
        return False
    x, y, w, h = before
    # Three pixels in from the nominal edge: an undecorated window's resize border
    # is a few pixels wide and the outermost one can belong to the frame itself.
    wininput.force(window)
    wininput.drag(x + w - 3, y + h / 2, dx, 0, steps=12, hold=0.02)
    deadline = time.time() + 1.5
    while time.time() < deadline:
        now = box(window)
        if now and now[2] != w:
            return True
        time.sleep(0.05)
    return False


def set_client(window: int, width: int, height: int) -> bool:
    """Resize from outside, the way an edge drag ends: same `WM_SIZE`, new client."""
    before = box(window)
    if not before:
        return False
    x, y, _, _ = before
    # `AdjustWindowRectEx` would be exact; this window has no frame, so the client
    # rectangle and the window rectangle are the same shape.
    return bool(
        user32.SetWindowPos(
            window,
            0,
            x,
            y,
            width,
            height,
            0x0002 | 0x0004 | 0x0010,  # SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE
        )
    )


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--exe", default="native/target/release/easyflowhub-native.exe")
    parser.add_argument("--work", default=os.path.join("native", "target", "c5-size-gate"))
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

    # Deliberately version-free, and with two keys a resize must not disturb: a
    # file written by any build before this change looks exactly like this.
    with open(settings_path, "w", encoding="utf-8") as handle:
        json.dump(
            {
                "margin": 12,
                "quick_note": {
                    "width": 400,
                    "height": 300,
                    "appearance": {"font_pt": 12.5},
                },
                "shortcuts": {"switcher": "Ctrl+Shift+K"},
            },
            handle,
            ensure_ascii=False,
            indent=2,
        )
    seeded = read_settings(settings_path)
    with open(settings_path, "rb") as handle:
        seeded_bytes = handle.read()

    report = Report()
    session = Session(exe, root, settings_path, os.path.join(work, "size-1.log"))
    try:
        # ------------------------------------------------------- the untouched run
        window = session.start()
        if not window:
            print("the shell never produced a note window; see " + session.trace.path)
            return 2
        startup = box(window)
        if not startup:
            print("the note window has no client rectangle")
            return 2
        # 400 logical is what the seeded file says; the ratio is the monitor scale,
        # measured rather than assumed so the gate runs at 100% and 150% alike.
        scale = round(startup[2] / 400.0, 2) or 1.0
        report.check(
            "C5 the note opens at the size in the settings file",
            abs(startup[2] - round(400 * scale)) <= 2
            and abs(startup[3] - round(300 * scale)) <= 2,
            f"client={startup[2]}x{startup[3]} scale={scale} expected~{round(400 * scale)}"
            f"x{round(300 * scale)}",
        )
        time.sleep(QUIET + 1.0)
        with open(settings_path, "rb") as handle:
            untouched = handle.read()
        report.check(
            "C5 an unresized launch does not rewrite the settings file",
            untouched == seeded_bytes,
            "file byte-identical after startup",
        )

        # ------------------------------------------------------------- the resize
        step = round(160 * scale)
        drag = try_drag(window, -step)
        dragged = box(window)
        if not drag:
            target = (max(round(150 * scale), dragged[2] - step), dragged[3])
            set_client(window, target[0], target[1])
            wininput.wait_for(lambda: box(window)[2] != dragged[2], 2)
            dragged = box(window)
        # Wait for the write rather than for a fixed time: the assertion is about a
        # file, and the quiet period plus a frame is what produces it. A read that
        # lands in the middle of the rename returns nothing -- the settings write is
        # atomic (temp file, then rename), so a miss means "try again", not "the
        # value is zero".
        deadline = time.time() + SETTLE
        written = None
        while time.time() < deadline:
            size = size_of(read_settings(settings_path))
            if size and size != (400, 300):
                written = size
                break
            time.sleep(0.1)
        after = box(window)
        # The file is renamed into place and the line is appended right after, in
        # that order, so the gate can see the new size before the line that explains
        # it. Waiting for the line is what makes the two assertions independent.
        line = wininput.wait_for(lambda: session.trace.last(SIZE_LINE), 3)

        report.check(
            "C5 the drag actually changed the window",
            bool(dragged) and dragged[2] < startup[2] - 8,
            f"drag={'real' if drag else 'programmatic'} "
            f"{startup[2]} -> {dragged[2] if dragged else '?'}",
        )
        report.check(
            "C5 the new width is written to quick_note.width",
            written is not None and abs(written[0] - round(after[2] / scale)) <= 2,
            f"file={written} client={after[2]} scale={scale}",
        )
        report.check(
            "C5 the height is left where it was",
            written is not None and written[1] == 300,
            f"file={written}",
        )
        report.check(
            "C5 the write is logged with the schema it wrote",
            line is not None and line.group(6) == "true" and line.group(7) == "1",
            line.group(0) if line else "no note_size line",
        )
        reloaded = read_settings(settings_path) or {}
        note = reloaded.get("quick_note") or {}
        report.check(
            "C5 the version field is stamped on a file that had none",
            note.get("schema") == 1,
            f"schema={note.get('schema')!r}",
        )
        report.check(
            "C5 the keys a resize has no opinion about survive",
            (note.get("appearance") or {}).get("font_pt") == 12.5
            and (reloaded.get("shortcuts") or {}).get("switcher") == "Ctrl+Shift+K"
            and reloaded.get("margin") == 12,
            f"font_pt={(note.get('appearance') or {}).get('font_pt')} "
            f"switcher={(reloaded.get('shortcuts') or {}).get('switcher')} "
            f"margin={reloaded.get('margin')}",
        )
        report.check(
            "C5 the window stays where it was dragged",
            after is not None and after[2] == dragged[2],
            f"after write={after[2] if after else '?'} dragged={dragged[2]}",
        )
        remembered = written
        session.stop()

        # ------------------------------------------------- the size has to travel
        again = Session(exe, root, settings_path, os.path.join(work, "size-2.log"))
        window = again.start()
        if not window:
            print("the second launch produced no note window; see " + again.trace.path)
            return 2
        reopened = box(window)
        report.check(
            "C5 the next process opens at the remembered width",
            reopened is not None and abs(reopened[2] - round(remembered[0] * scale)) <= 2,
            f"client={reopened[2] if reopened else '?'} remembered={remembered[0]} "
            f"scale={scale}",
        )
        report.check(
            "C5 the second launch does not touch the file either",
            read_settings(settings_path) and size_of(read_settings(settings_path)) == remembered,
            f"file={size_of(read_settings(settings_path))}",
        )
        again.stop()
    finally:
        session.stop()

    return report.finish()


if __name__ == "__main__":
    sys.exit(main())
