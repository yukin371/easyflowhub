"""Phase 2 gate: the tools panel closes the way Q7 says it does.

    T0  the `•••` button opens the panel with a real click
    Q7  the panel pushes the body down (the island moves, it is not just painted)
    Q7  the same button closes it again (toggle, not "open only")
    Q7  a click in the body closes it -- the click is reported by the island's
        subclass proc, because egui never sees a press on a child window

Two facts decided the shape of this script, both measured rather than assumed.

**The order is not cosmetic.** The body-click case runs last. A synthetic click
released over the RichEdit leaves the control's mouse capture in a state where
winit reports the parent's next cursor position relative to the *child* window -- a
click at client (300, 176) arrived as (182, 31) and started a window drag. A real
release clears that capture, so a user never sees it; a gate that clicks the body
first would silently get wrong coordinates and blame the product.

**The click must be real.** The `•••` button is an egui widget and egui's pointer
events come from winit, so a posted `WM_LBUTTONDOWN` goes to a window procedure
nobody is listening to. The body is the opposite case: it is a RichEdit child
window, so the press for Q7's second half never reaches the parent at all -- which
is exactly why `hub-editor` reports it.

The button's vertical position is read from the log instead of assumed: the island's
top edge *is* the strip height, and the button sits at half of it. At 150% that is
y=24; hard-coding 16 or 24 would click the wrong row on any other scale.

Usage:
    python scripts/check-native-tools.py --exe native/target/release/easyflowhub-native.exe
"""

import argparse
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
except (ImportError, OSError, AttributeError):  # not on Windows: this gate is Windows-only
    print("RESULT: SKIP (this gate drives a Windows window)")
    sys.exit(0)

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "lib"))
import wininput  # noqa: E402  (path first, then the import)

NOTE_PREFIX = "easyflowhub-note"

# `island_created` carries the first layout as `rect=WxH+X+Y`; later moves come from
# `island_rect` as `WxH+X+Y`. Only the top edge is wanted: it is the strip's height.
ISLAND_TOP = re.compile(r"island_(?:created|rect) id=0 (?:rect=)?\d+x\d+\+\d+\+(-?\d+)")


def note_window(index: int) -> int:
    return user32.FindWindowW(None, f"{NOTE_PREFIX}#{index}")


def island(index: int) -> int:
    window = note_window(index)
    return user32.FindWindowExW(window, None, "RICHEDIT50W", None) if window else 0


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

    def start(self) -> tuple:
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


def click_tools_button(session: Session, window: int, strip_height: int) -> str:
    """Click the centre slot; retried only while the log says nothing happened.

    Two things make the retry safe. It is guarded by the log, so a click that did
    land is never sent twice (a second one would close what the first opened). And
    the x coordinate shifts by a pixel per attempt, because two clicks at the same
    spot inside `GetDoubleClickTime` (500 ms here) can be synthesised as a
    double-click, which egui does not turn into a second `clicked()`.
    """
    _, _, width, _ = wininput.client_box(window)
    for attempt in range(4):
        before = session.trace.count("note_tools id=0")
        wininput.force(window)
        wininput.click_client(window, width / 2 + attempt, strip_height / 2)
        line = session.trace.later("note_tools id=0", before, 1.2)
        if line:
            return line
        print(f"  note: tools click attempt {attempt + 1} produced nothing; retrying")
    return ""


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--exe", default="native/target/release/easyflowhub-native.exe")
    parser.add_argument("--work", default=os.path.join(tempfile.gettempdir(), "p2-tools-gate"))
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
    log_path = os.path.join(work, "native-tools.log")

    report = Report()
    session = Session(exe, root, log_path)
    try:
        window, control = session.start()
        if not window or not control:
            print("the shell never produced a text island; see " + log_path)
            return 2

        tops = wininput.wait_for(lambda: island_tops(session.trace.text()) or None, 10) or []
        strip_height = tops[-1] if tops else 0
        _, _, width, height = wininput.client_box(window)
        report.check(
            "T0 the island starts below the strip",
            0 < strip_height < height,
            f"island top={strip_height}px client={width}x{height}",
        )

        # ---------------------------------------------------------------- open
        opened = click_tools_button(session, window, strip_height)
        report.check(
            "Q7 the tools button opened the panel",
            "open=true reason=button" in opened,
            opened.strip() or "no note_tools line",
        )

        moved = wininput.wait_for(
            lambda: [top for top in island_tops(session.trace.text()) if top > strip_height]
            or None,
            4,
        )
        report.check(
            "Q7 the body moved down instead of being painted over",
            bool(moved),
            f"island tops={island_tops(session.trace.text())} strip={strip_height}",
        )

        # ------------------------------------------------- the same button closes it
        closed_again = click_tools_button(session, window, strip_height)
        report.check(
            "Q7 the same button closes it (toggle, not open-only)",
            "open=false reason=button" in closed_again,
            closed_again.strip() or "no note_tools line",
        )
        back = wininput.wait_for(
            lambda: (island_tops(session.trace.text()) or [None])[-1] == strip_height or None,
            4,
        )
        report.check(
            "Q7 closing the panel gave the space back to the body",
            bool(back),
            f"island tops={island_tops(session.trace.text())}",
        )

        # ------------------------------------- a click in the body, last of all (see header)
        before_body = session.trace.count("note_tools id=0 open=false")
        reopened = click_tools_button(session, window, strip_height)
        if "open=true" not in reopened:
            report.check(
                "Q7 the panel reopened for the body-click case",
                False,
                reopened.strip() or "no note_tools line",
            )
        body_top = (
            wininput.wait_for(
                lambda: [
                    top for top in island_tops(session.trace.text()) if top > strip_height
                ]
                or None,
                4,
            )
            or [strip_height]
        )[-1]
        wininput.force(window)
        wininput.click_client(window, width / 2, body_top + 20)
        closed = session.trace.later("note_tools id=0 open=false", before_body, 3)
        report.check(
            "Q7 a click in the body closed the panel",
            "reason=body" in closed,
            closed.strip() or "no note_tools line (the click was never reported)",
        )
        report.check(
            "Q7 the click did not close the note",
            session.process.poll() is None and session.trace.count("note_close") == 0,
            f"process alive={session.process.poll() is None}",
        )
        restored = wininput.wait_for(
            lambda: (island_tops(session.trace.text()) or [None])[-1] == strip_height or None,
            4,
        )
        report.check(
            "Q7 the body came back up to the strip",
            bool(restored),
            f"island tops={island_tops(session.trace.text())}",
        )
    finally:
        session.stop()

    return report.finish()


if __name__ == "__main__":
    sys.exit(main())
