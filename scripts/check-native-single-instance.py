"""T-Gate 0 gate: one shell per session, and a second launch becomes a request.

    §7.2     a second launch must not become a second shell
    T-Gate 0 the login entry may point at the native shell only once this holds

What goes wrong without it, measured rather than imagined: the login entry starts the
shell, the user also clicks the icon, and the desktop ends up with two tray icons and a
round of `RegisterHotKey failed (1409)` -- the second process owns none of the chords it
promised, and nothing on screen says which icon is the live one.

The gate drives three processes against one another:

    A   `--resident`                 the shell; owns the mutex and the panel
    B   `--resident`  (second)       must forward, print one line, and exit 0
    C   `--autostart` (second)       must exit 0 *without* summoning anything

Three checks here are about ordering, not features, and each has a failure mode that a
feature test would miss:

* **B wrote no log.** `Log::new` truncates its file. If the claim happened after the log
  was opened, a second launch would erase the first instance's evidence and only then
  find out it had nothing to do -- and every other gate would report a boot that never
  happened. So `--log b.log` must not exist afterwards.
* **A forwarded while the panel was already up.** The answer must be "refocus", not a
  toggle: the user double-clicked because they could not see it, and a toggle would read
  as the double-click having closed it.
* **The autostart second instance is silent.** Windows starts the login entry while the
  desktop is still settling; a panel appearing over the login is the wrong first
  impression, so that forward rides the same event and is deliberately not answered.

Usage:
    python scripts/check-native-single-instance.py --exe native/target/release/easyflowhub-native.exe
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
    print("RESULT: SKIP (this gate drives Windows processes and windows)")
    sys.exit(0)

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "lib"))
import wininput  # noqa: E402

PANEL_TITLE = "easyflowhub-launcher"
NOTE_PREFIX = "easyflowhub-note"

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


def image_count(exe: str) -> int:
    """How many processes of this image are running. -1 when `tasklist` is absent."""
    name = os.path.basename(exe)
    try:
        done = subprocess.run(
            ["tasklist", "/FI", f"IMAGENAME eq {name}", "/NH"],
            capture_output=True,
            text=True,
            timeout=20,
        )
    except (OSError, subprocess.SubprocessError):
        return -1
    if done.returncode != 0:
        return -1
    return done.stdout.lower().count(name.lower())


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


class Shell:
    """The first instance: one `--resident` process, one log."""

    def __init__(self, exe: str, root: str) -> None:
        self.exe = exe
        self.root = root
        self.log = os.path.join(root, "shell.log")
        self.settings = os.path.join(root, "shell.json")
        self.store = os.path.join(root, "store")
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
        self.write_settings()
        self.process = subprocess.Popen(
            [
                self.exe,
                "--resident",
                "--settings",
                self.settings,
                "--store-root",
                self.store,
                "--log",
                self.log,
            ],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        wininput.wait_for(lambda: "hotkeys registered=" in text(self.log), 25)
        return self.process

    def alive(self) -> bool:
        return self.process is not None and self.process.poll() is None

    def count(self, needle: str) -> int:
        return sum(1 for line in text(self.log).splitlines() if needle in line)

    def line(self, needle: str) -> str:
        for line in text(self.log).splitlines():
            if needle in line:
                return line
        return ""

    def later(self, needle: str, before: int, timeout: float) -> str:
        deadline = time.time() + timeout
        while time.time() < deadline:
            seen = [line for line in text(self.log).splitlines() if needle in line]
            if len(seen) > before:
                return seen[before]
            time.sleep(0.05)
        return ""

    def press(self, chord) -> None:
        _, vk = chord
        wininput.chord(vk, ctrl=True, alt=True)

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


def second_launch(exe: str, shell: Shell, log: str, autostart: bool = False) -> tuple:
    """Spawn a second instance and wait for it to leave.

    Returns `(exit code, stderr, seconds)`. Bounded on purpose: a second instance that
    does not exit is the bug this gate exists for, so "it eventually finished" is not a
    pass -- three seconds is an order of magnitude past the claim and the ring.
    """
    argv = [
        exe,
        "--settings",
        shell.settings,
        "--store-root",
        shell.store,
        "--log",
        log,
    ]
    argv.append("--autostart" if autostart else "--resident")
    at = time.time()
    done = subprocess.run(argv, capture_output=True, text=True, timeout=30)
    return done.returncode, (done.stderr or done.stdout).strip(), time.time() - at


def registered_driven(log: str) -> bool:
    """The three chords this gate moves must have won their registration.

    Not "no failures at all": on a real machine another program owning `Ctrl+Alt+M`
    is the user's problem to see, and a gate that fails for it would be reporting
    somebody else's product.
    """
    body = text(log)
    return all(f"combo={chord} registered=true" in body for chord, _ in (LAUNCHER, NEW_NOTE, CLOSE_ALL))


def park(shell: "Shell", timeout: float = 5.0) -> bool:
    """Leave the panel parked, whichever state it is in.

    A press alone is not enough: whether it shows or hides depends on where the panel
    started, and a gate that assumes "it is up, so a press parks it" fails exactly when
    the thing it is testing went wrong.
    """
    if not off_screen(rect(find(PANEL_TITLE))):
        shell.press(LAUNCHER)
    return wininput.wait_for(lambda: off_screen(rect(find(PANEL_TITLE))), timeout)


def on_screen() -> bool:
    hwnd = find(PANEL_TITLE)
    return bool(hwnd) and not off_screen(rect(hwnd))


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--exe", default="native/target/release/easyflowhub-native.exe")
    parser.add_argument("--work", default=os.path.join(tempfile.gettempdir(), "tg0-instance-gate"))
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

    shell = Shell(exe, root)
    try:
        shell.start()

        # --- the shell owns the session --------------------------------------
        claim = shell.line("instance claimed=")
        if "exclusive=true" not in claim:
            # Another shell already owns this session: every check below would be
            # measuring *that* process, so say so instead of reporting a broken build.
            print(f"cannot run: {claim or 'no instance line'}")
            print("another native shell is already resident; quit it from the tray first")
            print("RESULT: FAIL")
            return 2
        report.check(
            "the first launch owns the session",
            "exclusive=true" in claim,
            claim[-90:],
        )
        report.check(
            "and it registered the three chords this gate drives",
            registered_driven(shell.log),
            f"combo lines={text(shell.log).count('combo=')}"
            f" failures={text(shell.log).count('registered=false')} (unrelated chords may lose theirs)",
        )

        # --- a second launch is a request, not a rival -----------------------
        b_log = os.path.join(root, "second.log")
        code, message, seconds = second_launch(exe, shell, b_log)
        report.check(
            "a second launch exits 0",
            code == 0,
            f"exit={code} in {seconds:.2f}s stderr={message[:80]}",
        )
        report.check(
            "and leaves in well under three seconds",
            seconds < 3.0,
            f"{seconds:.2f}s",
        )
        report.check(
            "it says it forwarded, rather than failing silently",
            message.startswith("instance action=forward"),
            message[:90] or "no stderr line",
        )
        # The ordering check. `Log::new` truncates; if the claim came after it, this
        # file would exist and the shell's own log would be gone.
        report.check(
            "it wrote no log at all (the claim precedes the log)",
            not os.path.exists(b_log),
            f"{b_log} exists={os.path.exists(b_log)}",
        )

        # --- the shell answers ------------------------------------------------
        reveal = shell.later("instance action=reveal", 0, 3.0)
        report.check(
            "the shell answers the forward",
            bool(reveal),
            reveal[-90:] or "no reveal line",
        )
        # The reveal line is written after the move, so the panel is already there --
        # but a frame or two of slack costs nothing and keeps a loaded machine honest.
        wininput.wait_for(on_screen, 2)
        report.check(
            "and the panel is on screen",
            on_screen(),
            f"hwnd={find(PANEL_TITLE)} rect={rect(find(PANEL_TITLE))} {reveal[-60:]}",
        )
        report.check(
            "the forward did not become a second shell",
            shell.count("hub_start") == 1
            and shell.count("tray installed=true") == 1
            and shell.count("instance claimed=") == 1,
            f"hub_start={shell.count('hub_start')} tray={shell.count('tray installed=true')}",
        )
        report.check(
            "and no note was spawned by it",
            shell.count("note_spawn") == 0,
            f"note_spawn={shell.count('note_spawn')}",
        )

        # --- a forward while it is already up must not close it ---------------
        # This is why the answer is `reveal` and not `toggle`: the double-click that
        # could not see the panel must not be the double-click that hides it.
        hides = shell.count("launcher_toggle action=hide")
        code, message, _ = second_launch(exe, shell, os.path.join(root, "third.log"))
        refocus = shell.later("launcher_toggle action=refocus", 0, 3.0)
        report.check(
            "a forward while the panel is up refocuses instead of hiding",
            code == 0 and bool(refocus)
            and shell.count("launcher_toggle action=hide") == hides,
            f"exit={code} {refocus[-70:] or 'no refocus line'} hides={shell.count('launcher_toggle action=hide')}",
        )
        report.check(
            "and the panel is still on screen",
            on_screen(),
            f"rect={rect(find(PANEL_TITLE))}",
        )

        # --- the autostart second instance stays silent -----------------------
        report.check(
            "the panel can be parked again for the next case",
            park(shell),
            f"rect={rect(find(PANEL_TITLE))}",
        )
        reveals = shell.count("instance action=reveal")
        code, message, _ = second_launch(exe, shell, os.path.join(root, "login.log"), autostart=True)
        time.sleep(0.8)
        report.check(
            "--autostart as a second instance exits 0 without summoning",
            code == 0
            and message.startswith("instance action=quiet")
            and shell.count("instance action=reveal") == reveals,
            f"exit={code} stderr={message[:70]} reveals={shell.count('instance action=reveal')}",
        )
        report.check(
            "and the panel stayed parked",
            off_screen(rect(find(PANEL_TITLE))),
            f"rect={rect(find(PANEL_TITLE))}",
        )

        # --- the shell is still a shell ---------------------------------------
        park(shell)
        before = shell.count("launcher_shown")
        shell.press(LAUNCHER)
        shown = shell.later("launcher_shown", before, 5.0)
        report.check(
            "its own hotkey still works after two forwards",
            bool(shown) and shell.alive(),
            f"{shown[-70:] or 'no launcher_shown line'} alive={shell.alive()}",
        )
        park(shell)

        # --- and there is exactly one ------------------------------------------
        alive = image_count(exe)
        report.check(
            "exactly one process of this image is running",
            alive == 1 or alive == -1,
            f"processes={alive}" + (" (tasklist unavailable)" if alive == -1 else ""),
        )
    finally:
        shell.stop()

    return report.finish()


if __name__ == "__main__":
    sys.exit(main())
