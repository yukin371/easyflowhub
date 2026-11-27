"""Phase 1 gate: the launcher, driven the way a user drives it.

    T-Gate 0  the panel is a pre-created, parked window -- not something created
              on the keystroke (§5.8)
    §5.1      it opens at the cursor, clamped into the work area, 440x860 logical
    §5.8      the reveal is a move: hotkey -> window on screen, no window creation
    §7.1      p95 <= 60 ms, and the content refresh is reported separately

What this gate adds over the S1 latency bench is *the product path*: the same
binary that ships the note window, the same hotkey table, and the actions actually
doing something (a note spawns, a workbench tab opens, Esc gives the keyboard
back).

Two environment facts shape the script:

* The shipped Tauri build registers `Ctrl+Alt+Space` globally. If it is running,
  `RegisterHotKey` fails with 1409 and no keystroke would ever arrive. So the gate
  writes a settings file that moves the launcher to the first free chord of a
  small list, and it **requires the registration to succeed** -- a bench that
  silently tests nothing is worse than a failing one.
* `IsWindowVisible` stays true for a parked window (that is what keeps it
  repaintable), so "closed" is judged by the rectangle being off every monitor,
  never by visibility.

Usage:
    python scripts/check-native-launcher.py --exe native/target/release/easyflowhub-native.exe
"""

import argparse
import ctypes
import json
import os
import shutil
import sqlite3
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
import winshot  # noqa: E402
import wininput  # noqa: E402

PANEL_TITLE = "easyflowhub-launcher"
NOTE_PREFIX = "easyflowhub-note"
MANAGER_TITLE = "easyflowhub-manager"

# `Ctrl+Alt+Space` is the shipped default, but the Tauri build owns it while it is
# running. F9..F11 are unused by both builds; the first one Windows accepts is the
# one the gate drives.
CHORDS = [("Ctrl+Alt+F9", 0x78), ("Ctrl+Alt+F10", 0x79), ("Ctrl+Alt+F11", 0x7A)]

STAMP = "2026-11-26T02:31:00Z"
SEEDED = [
    ("gate-launcher-a", "周末计划", "周末计划\n买螺丝刀和膨胀管"),
    ("gate-launcher-b", "接口设计", "接口设计\nlauncher 的动作契约"),
    ("gate-launcher-c", "", "随手记：面板要能在光标处出现"),
]


def text(path: str) -> str:
    if not os.path.exists(path):
        return ""
    with open(path, encoding="utf-8", errors="replace") as handle:
        return handle.read()


def find(title: str):
    return user32.FindWindowW(None, title)


def note_windows() -> int:
    found = [0]

    @ctypes.WINFUNCTYPE(ctypes.c_int, ctypes.c_void_p, ctypes.c_void_p)
    def collect(hwnd, _):
        length = user32.GetWindowTextLengthW(hwnd)
        buffer = ctypes.create_unicode_buffer(length + 1)
        user32.GetWindowTextW(hwnd, buffer, length + 1)
        if buffer.value.startswith(NOTE_PREFIX):
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
    """Parked: the window is still visible, but no monitor contains its origin."""
    x, y = box[0], box[1]
    return x < -10000 or y < -10000


def cursor_work_area():
    class POINT(ctypes.Structure):
        _fields_ = [("x", ctypes.c_long), ("y", ctypes.c_long)]

    class MONITORINFO(ctypes.Structure):
        _fields_ = [
            ("cbSize", ctypes.c_ulong),
            ("rcMonitor", ctypes.c_long * 4),
            ("rcWork", ctypes.c_long * 4),
            ("dwFlags", ctypes.c_ulong),
        ]

    point = POINT()
    user32.GetCursorPos(ctypes.byref(point))
    monitor = user32.MonitorFromPoint(point, 2)  # MONITOR_DEFAULTTONEAREST
    info = MONITORINFO()
    info.cbSize = ctypes.sizeof(MONITORINFO)
    user32.GetMonitorInfoW(monitor, ctypes.byref(info))
    work = info.rcWork
    return (work[0], work[1], work[2] - work[0], work[3] - work[1])


def diff_share(left: bytes, right: bytes) -> float:
    """Share of bytes that differ. Four bytes per pixel, so this bounds pixels."""
    if len(left) != len(right):
        return 1.0
    return sum(1 for a, b in zip(left, right) if a != b) / float(len(left))


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
    """One process, one log. `Log::new` truncates, so each scenario restarts."""

    def __init__(self, exe: str, root: str, name: str) -> None:
        self.exe = exe
        self.root = root
        self.name = name
        self.log = os.path.join(root, f"{name}.log")
        self.settings = os.path.join(root, f"{name}.json")
        self.process = None
        self.chord = None
        self.vk = None

    def write_settings(self, chord: str) -> None:
        with open(self.settings, "w", encoding="utf-8") as handle:
            json.dump({"shortcuts": {"launcher": chord}}, handle)

    def start(self, chord: str, vk: int) -> bool:
        self.stop()
        self.chord = chord
        self.vk = vk
        self.write_settings(chord)
        self.process = subprocess.Popen(
            [self.exe, "--note", "--settings", self.settings,
             "--store-root", os.path.join(self.root, "store"), "--log", self.log],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        # Readiness is the registration line, not the window: a chord that another
        # program owns leaves the process running with no working key.
        wininput.wait_for(lambda: "hotkeys registered=" in text(self.log), 25)
        return f"combo={chord} registered=true" in text(self.log)

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

    def count(self, needle: str) -> int:
        return sum(1 for line in text(self.log).splitlines() if needle in line)

    def press(self) -> None:
        wininput.chord(self.vk, ctrl=True, alt=True)

    def wait_parked(self, timeout: float = 10.0) -> str:
        """Wait for the panel window to exist and be parked.

        Registration is logged by the hotkey thread; the window is created by the
        first UI frame, ~100 ms later. Pressing in between reaches
        `launcher_toggle ready=false reason=no_window` -- a real state, but not the
        one this gate is about, so every scenario waits for the window first.
        """
        wininput.wait_for(lambda: "launcher_window parked=true" in text(self.log), timeout)
        return self.line("launcher_window parked=true")

    def query(self, expect: str, timeout: float = 3.0) -> str:
        """Wait until the panel's last `launcher_query` line carries `expect`.

        One keystroke is one line, so typing 回收 produces `text="回"` and then
        `text="回收"`. The settled state is the *last* line, not the first new one:
        typing is fast enough that both lines usually land before this is called.
        """
        deadline = time.time() + timeout
        last = ""
        while time.time() < deadline:
            seen = [
                line
                for line in text(self.log).splitlines()
                if "launcher_query" in line
            ]
            if seen:
                last = seen[-1]
                if not expect or f'text="{expect}"' in last:
                    return last
            time.sleep(0.05)
        return last

    def reveal(self, timeout: float = 5.0) -> bool:
        # An injected chord pressed within ~50 ms of the panel parking is dropped
        # by the OS: the foreground changed under the input, the key never becomes
        # a `WM_HOTKEY`, and the log shows no `launcher_toggle` line at all. It was
        # measured identically on this build and on the one before it, so it is the
        # injection racing the hide rather than the product -- and a hand presses
        # again seconds later, not 50 ms later. Settle first, then measure.
        if off_screen(rect(self.panel())):
            wininput.wait_for(lambda: off_screen(rect(self.panel())), 2)
            time.sleep(0.25)
        before = self.count("launcher_shown")
        self.press()
        if not self.later("launcher_shown", before, timeout):
            # Same race as above, one step worse: the deadline is short and the
            # press can still be swallowed after the settle. A hand presses again;
            # so does this, once, and the second press is the measurement.
            time.sleep(0.4)
            self.press()
            if not self.later("launcher_shown", before, timeout):
                return False
        # The window is on screen before the query box has the keyboard -- ~80 ms
        # of paint separate them. Typing inside that gap is swallowed by egui, so
        # every reveal waits for `launcher_ready` before the next keystroke.
        ready = self.count("launcher_ready")
        self.later("launcher_ready", ready, timeout)
        return True

    def panel(self):
        return find(PANEL_TITLE)

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


def seed(root: str, exe: str) -> None:
    """Create the schema with the app, then add rows the recent list can show.

    The schema is not duplicated here on purpose: a gate that re-declares the
    tables it is testing starts passing against a schema the product no longer
    has. `notes.db` is created by the app, then filled.
    """
    store = os.path.join(root, "store")
    log = os.path.join(root, "seed.log")
    settings = os.path.join(root, "seed.json")
    probe = subprocess.Popen(
        [exe, "--note", "--settings", settings, "--store-root", store, "--log", log],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    try:
        if not wininput.wait_for(lambda: "store open=true" in text(log), 25):
            print("the seed process never opened the store; see " + log)
            print(text(log)[-600:])
            raise SystemExit(2)
        time.sleep(0.4)
    finally:
        probe.terminate()
        try:
            probe.wait(timeout=10)
        except subprocess.TimeoutExpired:
            probe.kill()

    connection = sqlite3.connect(os.path.join(store, "notes.db"))
    try:
        for note_id, title, content in SEEDED:
            connection.execute(
                "INSERT OR REPLACE INTO notes"
                " (id, title, content, tags, created_at, updated_at, is_pinned, deleted_at)"
                " VALUES (?, ?, ?, '', ?, ?, 0, NULL)",
                (note_id, title, content, STAMP, STAMP),
            )
        connection.commit()
    finally:
        connection.close()


def run_bench(exe: str, root: str, cycles: int) -> dict:
    """`--launcher-bench`: the product's own reveal loop, medians and all."""
    work = os.path.join(root, f"bench-{cycles}")
    shutil.rmtree(work, ignore_errors=True)
    os.makedirs(work)
    settings = os.path.join(work, "settings.json")
    with open(settings, "w", encoding="utf-8") as handle:
        json.dump({"shortcuts": {"launcher": "Ctrl+Alt+F9"}}, handle)
    finished = subprocess.run(
        [exe, "--note", "--launcher-bench", str(cycles), "--settings", settings,
         "--store-root", os.path.join(work, "store"), "--log", os.path.join(work, "hub.log")],
        capture_output=True,
        text=True,
        timeout=180,
    )
    out = finished.stdout
    if "---JSON---" not in out or "---LEAK---" not in out:
        raise SystemExit(f"bench {cycles}: no report\n{out[-800:]}\n{finished.stderr[-800:]}")
    latency = json.loads(out.split("---JSON---")[1].split("---LEAK---")[0])
    leak = json.loads(out.split("---LEAK---")[1].strip().splitlines()[0])
    latency["leak"] = leak
    latency["exit"] = finished.returncode
    return latency


def scenario_a(session: Session, report: Report) -> None:
    """One process: parked -> reveal -> typing -> actions -> Esc -> blur."""
    session.write_settings(session.chord)
    ok = session.start(session.chord, session.vk)
    report.check(
        "the launcher hotkey is registered",
        ok,
        f"{session.chord} registered=true"
        if ok
        else f"{session.chord} was refused (another program owns it); tried {[c for c, _ in CHORDS]}",
    )
    if not ok:
        return

    parked = session.wait_parked()
    report.check(
        "the window exists before any keystroke",
        bool(parked),
        parked[-110:] or "no launcher_window line",
    )
    panel = session.panel()
    box = rect(panel) if panel else (0, 0, 0, 0)
    report.check(
        "it is parked off every monitor at startup",
        bool(panel) and off_screen(box),
        f"rect={box}",
    )

    # --- reveal -------------------------------------------------------------
    shown = session.reveal()
    line = session.line("launcher_shown")
    report.check("the hotkey reveals the panel", shown, line[-150:] or "no launcher_shown")
    if not shown:
        return

    panel = session.panel()
    box = rect(panel)
    work = cursor_work_area()
    inside = (
        box[0] >= work[0] - 1
        and box[1] >= work[1] - 1
        and box[0] + box[2] <= work[0] + work[2] + 1
        and box[1] + box[3] <= work[1] + work[3] + 1
    )
    report.check(
        "it lands inside the work area of the cursor's monitor",
        inside,
        f"rect={box} work={work}",
    )
    # The log records what the handler intended; the rectangle is what Windows
    # did. A mismatch here is the DPI bug class §5.1.1 warns about.
    intended = ""
    for token in line.split():
        if token.startswith("at=+") and "size=" in line:
            intended = token[3:]  # "+x+y"
    size = ""
    for token in line.split():
        if token.startswith("size="):
            size = token[5:]
    want_origin = intended.lstrip("+").split("+")
    want_size = size.split("x")
    matches = (
        len(want_origin) == 2
        and len(want_size) == 2
        and (box[0], box[1]) == (int(want_origin[0]), int(want_origin[1]))
        and (box[2], box[3]) == (int(want_size[0]), int(want_size[1]))
    )
    report.check(
        "the window is exactly where the handler said it put it",
        matches,
        f"rect={box} log=at={intended} size={size}",
    )

    focused = wininput.wait_for(
        lambda: user32.GetForegroundWindow() == session.panel(), 1.5
    )
    report.check("the panel takes the keyboard", focused, "GetForegroundWindow == launcher")

    idle = winshot.client_pixels(session.panel())[2]

    # --- typing filters, and the pixels follow -------------------------------
    wininput.type_text("回收")
    hit = session.query("回收")
    report.check(
        "typing filters the panel",
        'text="回收"' in hit and "matches=1" in hit,
        hit[-110:] or "no launcher_query line",
    )
    filtered = winshot.client_pixels(session.panel())[2]
    report.check(
        "the filtered panel is actually different pixels",
        diff_share(idle, filtered) > 0.02,
        f"diff={diff_share(idle, filtered):.1%}",
    )

    # --- an empty result is inert -------------------------------------------
    wininput.tap(0x08)  # Backspace x2 removes 回收
    wininput.tap(0x08)
    wininput.type_text("zzz")
    empty = session.query("zzz")
    empty_px = winshot.client_pixels(session.panel())[2]
    notes_before = note_windows()
    wininput.tap(0x0D)  # Enter
    time.sleep(0.6)
    report.check(
        "Enter on an empty result does nothing",
        "matches=0" in empty
        and not session.line("launcher_action")
        and note_windows() == notes_before
        and not off_screen(rect(session.panel())),
        f"matches0={'matches=0' in empty} note_windows={note_windows()} visible={not off_screen(rect(session.panel()))}",
    )
    report.check(
        "the empty state is its own pixels",
        diff_share(idle, empty_px) > 0.02 and diff_share(filtered, empty_px) > 0.01,
        f"idle={diff_share(idle, empty_px):.1%} filtered={diff_share(filtered, empty_px):.1%}",
    )

    # --- a card runs the real action ----------------------------------------
    for _ in range(5):
        wininput.tap(0x08)
    wininput.type_text("回收")
    session.query("回收")
    wininput.tap(0x0D)
    opened = bool(session.later("manager_tab", 0, 3.0))
    report.check(
        "Enter on 回收站 opens the workbench on that tab",
        opened and "tab=trash" in session.line("manager_tab"),
        session.line("manager_tab")[-80:] or "no manager_tab line",
    )
    report.check(
        "the panel hides before the action's window appears",
        off_screen(rect(session.panel())) and "reason=action" in session.line("launcher_hide"),
        f"rect={rect(session.panel())} {session.line('launcher_hide')[-70:]}",
    )

    # --- Esc gives the keyboard back ----------------------------------------
    report.check("it reopens", session.reveal(), "second launcher_shown")
    wininput.wait_for(lambda: user32.GetForegroundWindow() == session.panel(), 1.5)
    before = session.count("launcher_hide reason=esc")
    wininput.tap(0x1B)
    esc = session.later("launcher_hide reason=esc", before, 3.0)
    # `focus_after=0` used to be the assertion here: the panel let go of the
    # keyboard and nothing took it. That is now only half the story -- a dismissal
    # also hands the keyboard back to the window the panel was summoned over, so
    # the honest statement is "the keyboard is on that window, not on the panel".
    refocus = 0
    for part in esc.split():
        if part.startswith("refocus="):
            value, _, ok = part.partition("=")[2].partition(":")
            if ok == "true" and value.isdigit():
                refocus = int(value)
    report.check(
        "Esc hides it and hands the keyboard to the window underneath",
        bool(esc)
        and off_screen(rect(session.panel()))
        and refocus != 0
        and user32.GetForegroundWindow() == refocus,
        f"{esc[-110:] or 'no launcher_hide reason=esc'} foreground={user32.GetForegroundWindow()}",
    )

    # --- a click outside does the same --------------------------------------
    report.check("it reopens again", session.reveal(), "third launcher_shown")
    wininput.wait_for(lambda: user32.GetForegroundWindow() == session.panel(), 1.5)
    note = wininput.wait_for(lambda: find(f"{NOTE_PREFIX}#0"), 2)
    if note:
        wininput.force(note)
    blurred = bool(session.later("launcher_hide reason=blur", 0, 3.0))
    report.check(
        "clicking another window dismisses it",
        blurred and off_screen(rect(session.panel())),
        session.line("launcher_hide reason=blur")[-90:] or "no blur hide",
    )

    # --- and the key that opened it closes it -------------------------------
    # The three checks above cover the panel's own doors -- action, Esc, blur -- and
    # all three run on the UI thread. The hotkey door is the only one that *queues*
    # its hide, and it queued a request the UI thread then declined to act on:
    # `toggle` had already cleared `visible` on the hotkey thread (that is how it
    # chooses a direction), so `Shell::hide`'s "only hide what is shown" guard
    # answered "not shown" and the panel stayed on screen with its flag off. Nothing
    # in this gate pressed the key twice, so nothing here saw it.
    report.check("it opens for the hotkey case", session.reveal(), "hotkey case shown")
    wininput.wait_for(lambda: user32.GetForegroundWindow() == session.panel(), 1.5)
    before = session.count("launcher_hide reason=hotkey")
    session.press()
    queued = session.later("launcher_hide reason=hotkey", before, 3.0)
    report.check(
        "the same hotkey hides it, and the window actually moves",
        bool(queued) and off_screen(rect(session.panel())),
        queued[-100:] or "no launcher_hide reason=hotkey",
    )

    # --- recent notes, opened from the panel --------------------------------
    report.check("it opens for the recent list", session.reveal(), "fourth launcher_shown")
    wininput.wait_for(lambda: user32.GetForegroundWindow() == session.panel(), 1.5)
    seeded = session.line("launcher_recent")
    report.check(
        "the recent list is a snapshot of the store",
        "notes=" in seeded and int(seeded.split("notes=")[1].strip()) >= len(SEEDED),
        seeded[-60:] or "no launcher_recent line",
    )
    notes_before = note_windows()
    wininput.type_text("螺丝")
    session.query("螺丝")
    wininput.tap(0x0D)
    opened = bool(session.later("note_open id=gate-launcher-a", 0, 3.0))
    report.check(
        "Enter on a recent note opens that note",
        opened and note_windows() > notes_before,
        session.line("note_open")[-90:] or "no note_open line",
    )


def scenario_b(session: Session, report: Report) -> None:
    """A card that spawns a note -- on a store with no seeded rows."""
    session.write_settings(session.chord)
    if not session.start(session.chord, session.vk):
        report.check("second process starts", False, f"{session.chord} not registered")
        return
    session.wait_parked()
    if not session.reveal():
        report.check("the hotkey reveals the panel", False, "no launcher_shown")
        return
    before = note_windows()
    wininput.type_text("新建")
    session.query("新建")
    wininput.tap(0x0D)
    grew = wininput.wait_for(lambda: note_windows() > before, 5)
    report.check(
        "Enter on 新建笔记 spawns a note window",
        grew,
        f"notes {before} -> {note_windows()}",
    )
    report.check(
        "and the panel is gone before the note takes the keyboard",
        off_screen(rect(session.panel())),
        f"rect={rect(session.panel())}",
    )


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--exe", default="native/target/release/easyflowhub-native.exe")
    parser.add_argument("--work", default=os.path.join(tempfile.gettempdir(), "p1-launcher-gate"))
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
    print("preparing a store with three notes ...")
    seed(root, exe)

    # Pick the first chord Windows will actually register; the session tells us by
    # logging `registered=true`. Trying them in order keeps the gate runnable on a
    # machine where the shipped build is also running.
    chord, vk = CHORDS[0]
    probe = Session(exe, root, "probe")
    for candidate, code in CHORDS:
        if probe.start(candidate, code):
            chord, vk = candidate, code
            break
        print(f"  note: {candidate} is taken; trying the next one")
    probe.stop()

    session = Session(exe, root, "a")
    session.chord, session.vk = chord, vk
    try:
        scenario_a(session, report)
    finally:
        session.stop()

    session = Session(exe, root, "b")
    session.chord, session.vk = chord, vk
    try:
        scenario_b(session, report)
    finally:
        session.stop()

    # --- the latency budget, on the product's own reveal loop ---------------
    print("bench: 20 cycles ...")
    short = run_bench(exe, root, 20)
    print("bench: 40 cycles ...")
    long = run_bench(exe, root, 40)
    shown = long["shown"]
    report.check(
        "p95 <= 60 ms (the gate) and p99 <= 100 ms",
        long["budget"]["p95_ok"] and long["budget"]["p99_ok"],
        f"n={long['n']} p50={shown['p50']} p95={shown['p95']} p99={shown['p99']} ms",
    )
    leak_short = short["leak"]
    leak_long = long["leak"]
    growth = leak_long["private_delta_mb"] - leak_short["private_delta_mb"]
    # The absolute handle count is not the question: the first reveal allocates a
    # fixed block of DWM/window handles (~66 here, measured with both 20 and 40
    # cycles) and stays there. What a leak looks like is the *difference* between
    # the two runs growing with the cycles, so that difference is what is checked.
    handle_growth = leak_long["handles_delta"] - leak_short["handles_delta"]
    report.check(
        "no per-cycle leak: doubling the cycles does not double the growth",
        growth <= 6.0 and handle_growth <= 40,
        f"private {leak_short['private_delta_mb']}MB@{short['n']} vs "
        f"{leak_long['private_delta_mb']}MB@{long['n']}, "
        f"handles {leak_short['handles_delta']}@{short['n']} vs "
        f"{leak_long['handles_delta']}@{long['n']}",
    )
    report.check(
        "the refresh is reported separately and is not the gate",
        "p95" in long["refresh"] and long["refresh"]["p95"] >= shown["p95"],
        f"refresh p95={long['refresh']['p95']} ms vs shown p95={shown['p95']} ms",
    )
    print(
        "  leak: private {} -> {} MB, handles {} -> {}".format(
            leak_long["start_private_mb"],
            leak_long["end_private_mb"],
            leak_long["start_handles"],
            leak_long["end_handles"],
        )
    )

    return report.finish()


if __name__ == "__main__":
    sys.exit(main())
