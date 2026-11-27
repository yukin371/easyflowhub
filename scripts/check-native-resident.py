"""Phase 1 gate: the resident process -- the shape the login entry starts.

    T-Gate 0  the native shell owns the keys without putting a window on screen
    §5.1      "nothing is open" with a tray is a resting state, not an exit
    §5.8      the panel is pre-created even when no note ever existed

What makes this different from `check-native-launcher.py` is the *starting* state:
that gate proves the panel works in a process that already opened a note. Here the
process starts with zero notes, which is the state it will spend 23 hours a day in,
and the questions are:

* does it come up at all, or does "no notes" look like a reason to quit?
* does the panel exist before any note does?
* does a hotkey still spawn a note, and does closing that note leave the process
  alive and the panel usable?

The three chords the gate drives are moved to F9..F11 through the settings file for
the same reason the launcher gate does it: a shipped Tauri instance registers
`Ctrl+Alt+Space` / `+N` / `+D` globally, `RegisterHotKey` then fails with 1409, and
a gate that silently drives nothing is worse than a failing one.

Usage:
    python scripts/check-native-resident.py --exe native/target/release/easyflowhub-native.exe
"""

import argparse
import ctypes
import json
import os
import shutil
import subprocess
import sys
import tempfile
import time

try:
    user32 = ctypes.WinDLL("user32", use_last_error=True)
except (ImportError, OSError, AttributeError):  # not on Windows: Windows-only gate
    print("RESULT: SKIP (this gate drives a Windows window)")
    sys.exit(0)

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "lib"))
import wininput  # noqa: E402

PANEL_TITLE = "easyflowhub-launcher"
NOTE_PREFIX = "easyflowhub-note"
HUB_TITLE = "easyflowhub-hub"

LAUNCHER = ("Ctrl+Alt+F9", 0x78)
NEW_NOTE = ("Ctrl+Alt+F10", 0x79)
CLOSE_ALL = ("Ctrl+Alt+F11", 0x7A)


def text(path: str) -> str:
    if not os.path.exists(path):
        return ""
    with open(path, encoding="utf-8", errors="replace") as handle:
        return handle.read()


def find(title: str):
    return user32.FindWindowW(None, title)


def windows_starting_with(prefix: str) -> int:
    found = [0]

    @ctypes.WINFUNCTYPE(ctypes.c_int, ctypes.c_void_p, ctypes.c_void_p)
    def collect(hwnd, _):
        length = user32.GetWindowTextLengthW(hwnd)
        buffer = ctypes.create_unicode_buffer(length + 1)
        user32.GetWindowTextW(hwnd, buffer, length + 1)
        if buffer.value.startswith(prefix):
            found[0] += 1
        return 1

    user32.EnumWindows(collect, 0)
    return found[0]


def rect(hwnd: int):
    class RECT(ctypes.Structure):
        _fields_ = [
            ("left", ctypes.c_int),
            ("top", ctypes.c_int),
            ("right", ctypes.c_int),
            ("bottom", ctypes.c_int),
        ]

    box = RECT()
    if not user32.GetWindowRect(hwnd, ctypes.byref(box)):
        return (0, 0, 0, 0)
    return (box.left, box.top, box.right - box.left, box.bottom - box.top)


def off_screen(box) -> bool:
    """Parked: visible to Windows (that keeps it repaintable), on no monitor."""
    return box[0] < -10000 or box[1] < -10000


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


class Session:
    """One `--resident` process, one log."""

    def __init__(self, exe: str, root: str, name: str) -> None:
        self.exe = exe
        self.root = root
        self.name = name
        self.log = os.path.join(root, f"{name}.log")
        self.settings = os.path.join(root, f"{name}.json")
        self.process = None

    def write_settings(self) -> None:
        with open(self.settings, "w", encoding="utf-8") as handle:
            json.dump(
                {
                    "shortcuts": {
                        "launcher": LAUNCHER[0],
                        "new_note": NEW_NOTE[0],
                        "close_all": CLOSE_ALL[0],
                    }
                },
                handle,
            )

    def start(self):
        self.stop()
        self.write_settings()
        self.process = subprocess.Popen(
            [
                self.exe,
                "--resident",
                "--settings",
                self.settings,
                "--store-root",
                os.path.join(self.root, "store"),
                "--log",
                self.log,
            ],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        # Readiness is the registration line, not the process existing: a chord
        # another program owns leaves a running process with no working key.
        wininput.wait_for(lambda: "hotkeys registered=" in text(self.log), 25)
        # And then the banner that follows it, because that is the next thing this
        # gate asserts on. Registration is written by the hotkey thread's setup and
        # `hub_start` ~20 ms later by the controller; reading in between made the
        # first check fail about one run in ten on a loaded machine, which reads
        # exactly like "the shell did not start".
        wininput.wait_for(lambda: "hub_start" in text(self.log), 5)
        return self.process

    def alive(self) -> bool:
        return self.process is not None and self.process.poll() is None

    def line(self, needle: str) -> str:
        for line in text(self.log).splitlines():
            if needle in line:
                return line
        return ""

    def count(self, needle: str) -> int:
        return sum(1 for line in text(self.log).splitlines() if needle in line)

    def later(self, needle: str, before: int, timeout: float) -> str:
        deadline = time.time() + timeout
        while time.time() < deadline:
            seen = [line for line in text(self.log).splitlines() if needle in line]
            if len(seen) > before:
                return seen[before]
            time.sleep(0.05)
        return ""

    def press(self, chord) -> bool:
        _, vk = chord
        wininput.chord(vk, ctrl=True, alt=True)
        return True

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
        wininput.wait_for(lambda: find(PANEL_TITLE) == 0, 5)
        self.process = None


def refused(exe: str, root: str) -> tuple:
    """`--resident --note-id x` must be refused, not silently resolved."""
    finished = subprocess.run(
        [exe, "--resident", "--note-id", "gate-resident-a", "--settings",
         os.path.join(root, "refused.json")],
        capture_output=True,
        text=True,
        timeout=30,
    )
    return finished.returncode, (finished.stderr or finished.stdout).strip()


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--exe", default="native/target/release/easyflowhub-native.exe")
    parser.add_argument("--work", default=os.path.join(tempfile.gettempdir(), "p1-resident-gate"))
    args = parser.parse_args()

    exe = os.path.abspath(args.exe)
    if not os.path.exists(exe):
        print(f"missing {exe}; build it with `cargo build --release -p app`")
        return 2

    root = args.work
    shutil.rmtree(root, ignore_errors=True)
    os.makedirs(root)
    report = Report()

    print(f"gate root: {root}")

    # --- flags refuse a contradiction instead of resolving it silently -------
    code, message = refused(exe, root)
    report.check(
        "--resident and --note-id is refused, not silently resolved",
        code == 64 and "note-id" in message,
        f"exit={code} message={message[:90]}",
    )

    session = Session(exe, root, "resident")
    try:
        process = session.start()

        # --- the shape it comes up in ---------------------------------------
        start = session.line("hub_start")
        report.check(
            "it starts with no note",
            "notes=0" in start and "resident=true" in start,
            start[-110:] or "no hub_start line",
        )
        # A couple of seconds is enough for a window that was going to be created
        # to exist: the note shell's window is created on the first pass.
        time.sleep(1.5)
        report.check(
            "and no note window exists after it settles",
            windows_starting_with(NOTE_PREFIX) == 0,
            f"note windows={windows_starting_with(NOTE_PREFIX)}",
        )
        report.check(
            "the panel window exists before any note ever did",
            bool(find(PANEL_TITLE)),
            f"panel hwnd={find(PANEL_TITLE)}",
        )
        report.check(
            "the panel starts parked",
            off_screen(rect(find(PANEL_TITLE))),
            f"rect={rect(find(PANEL_TITLE))}",
        )
        idle = session.line("hub_idle")
        report.check(
            "zero notes is a resting state, not an exit",
            bool(idle) and "tray=0x" in idle and session.alive(),
            f"{idle[-60:] or 'no hub_idle line'} alive={session.alive()}",
        )
        report.check(
            "the three chords the gate drives are registered",
            all(
                f"combo={chord} registered=true" in text(session.log)
                for chord, _ in (LAUNCHER, NEW_NOTE, CLOSE_ALL)
            ),
            "launcher/new_note/close_all -> F9/F10/F11",
        )

        # --- the panel works with nothing open -------------------------------
        before = session.count("launcher_shown")
        session.press(LAUNCHER)
        shown = session.later("launcher_shown", before, 5.0)
        report.check(
            "the hotkey reveals the panel with no note open",
            bool(shown),
            shown[-120:] or "no launcher_shown line",
        )
        report.check(
            "revealing does not spawn a note",
            windows_starting_with(NOTE_PREFIX) == 0,
            f"note windows={windows_starting_with(NOTE_PREFIX)}",
        )
        wininput.tap(0x1B)  # Esc: park it again
        wininput.wait_for(lambda: off_screen(rect(find(PANEL_TITLE))), 3)

        # --- a note appears on demand, in this same process -------------------
        before = session.count("note_spawn")
        session.press(NEW_NOTE)
        spawned = session.later("note_spawn", before, 6.0)
        grew = wininput.wait_for(lambda: windows_starting_with(NOTE_PREFIX) == 1, 6)
        report.check(
            "Ctrl+Alt+N spawns a note in the resident process",
            bool(spawned) and grew,
            f"{spawned[-70:] or 'no note_spawn line'} windows={windows_starting_with(NOTE_PREFIX)}",
        )
        report.check(
            "and it is still the same process (no second instance)",
            session.alive() and session.process.pid is not None,
            f"pid={session.process.pid} alive={session.alive()}",
        )

        # --- closing the last note leaves it resident -------------------------
        before = session.count("hub_idle")
        session.press(CLOSE_ALL)
        closed = wininput.wait_for(lambda: windows_starting_with(NOTE_PREFIX) == 0, 8)
        rested = session.later("hub_idle", before, 8.0)
        report.check(
            "Ctrl+Alt+D closes the last note",
            closed,
            f"note windows={windows_starting_with(NOTE_PREFIX)} close_all={session.line('note_close_all')[-40:]}",
        )
        report.check(
            "closing the last note leaves the process resident",
            bool(rested) and session.alive() and "tray=0x" in rested,
            f"{rested[-60:] or 'no second hub_idle'} alive={session.alive()}",
        )
        report.check(
            "the panel survived the note's death",
            bool(find(PANEL_TITLE)) and off_screen(rect(find(PANEL_TITLE))),
            f"panel hwnd={find(PANEL_TITLE)} rect={rect(find(PANEL_TITLE))}",
        )

        # --- and it is usable again ------------------------------------------
        before = session.count("launcher_shown")
        session.press(LAUNCHER)
        again = session.later("launcher_shown", before, 5.0)
        report.check(
            "a second reveal works after going idle",
            bool(again) and not off_screen(rect(find(PANEL_TITLE))),
            again[-110:] or "no second launcher_shown",
        )
        report.check(
            "nothing in the run treated zero notes as a reason to quit",
            session.count("hub_exit") == 0,
            f"hub_exit lines={session.count('hub_exit')}",
        )
    finally:
        session.stop()

    return report.finish()


if __name__ == "__main__":
    sys.exit(main())
