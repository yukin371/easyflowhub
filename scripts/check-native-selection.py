"""Selection capture: the panel takes the text under the user's cursor, and gives the clipboard back.

    T-Gate 0  §5  "selection -> translate" was the cost of stopping the webviews

What this gate is protecting is not the feature, it is the *side effect*. Asking an
application for its selection is done the only way that works everywhere: press
Ctrl+C in its face and read the clipboard. That means every reveal writes to the
user's clipboard, and a bug here loses something the user copied -- silently, and
before the panel even appears. So the checks below are mostly about what the
clipboard looks like afterwards.

Three scenarios, each with a different thing to get wrong:

    text     clipboard holds a sentinel, note has a selection
             -> the selection is taken, the sentinel comes back
    empty    clipboard empty, note has a selection
             -> the selection is taken, the clipboard is empty again
             (not left holding our copy of it)
    nothing  nothing is selected
             -> nothing is taken, and the clipboard is *not* cleared

The clipboard helpers here are written from scratch on `ctypes` rather than reusing
the product's: a gate that shares the code path under test agrees with every bug it
is meant to catch.

Usage:
    python scripts/check-native-selection.py --exe native/target/release/easyflowhub-native.exe
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
    kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)
except (ImportError, OSError, AttributeError):  # not on Windows: Windows-only gate
    print("RESULT: SKIP (this gate drives Windows processes and windows)")
    sys.exit(0)

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "lib"))
import wininput  # noqa: E402
import winshot  # noqa: E402

# Declared up front: a `HANDLE` is a pointer, and ctypes defaults to `int`, which
# turns a 64-bit clipboard block into "int too long to convert" at the first call.
kernel32.GlobalAlloc.restype = ctypes.c_void_p
kernel32.GlobalAlloc.argtypes = [ctypes.c_uint, ctypes.c_size_t]
kernel32.GlobalLock.restype = ctypes.c_void_p
kernel32.GlobalLock.argtypes = [ctypes.c_void_p]
kernel32.GlobalUnlock.argtypes = [ctypes.c_void_p]
user32.OpenClipboard.argtypes = [ctypes.c_void_p]
user32.GetClipboardData.restype = ctypes.c_void_p
user32.GetClipboardData.argtypes = [ctypes.c_uint]
user32.SetClipboardData.restype = ctypes.c_void_p
user32.SetClipboardData.argtypes = [ctypes.c_uint, ctypes.c_void_p]
user32.EnumClipboardFormats.argtypes = [ctypes.c_uint]

PANEL_TITLE = "easyflowhub-launcher"
NOTE_PREFIX = "easyflowhub-note"

LAUNCHER = ("Ctrl+Alt+F9", 0x78)
NEW_NOTE = ("Ctrl+Alt+F10", 0x79)
CLOSE_ALL = ("Ctrl+Alt+F11", 0x7A)

WM_GETTEXT = 0x000D

# A second handle on `user32`, with its own prototypes: `wininput` declares
# `SendMessageW` with integer arguments, and a text buffer does not fit those.
_plain_user32 = ctypes.WinDLL("user32")
_plain_user32.SendMessageW.restype = ctypes.c_ssize_t
_plain_user32.SendMessageW.argtypes = [
    ctypes.c_void_p,
    ctypes.c_uint,
    ctypes.c_size_t,
    ctypes.c_void_p,
]

VK_A = 0x41
VK_C = 0x43
VK_DELETE = 0x2E
VK_RIGHT = 0x27
VK_ESCAPE = 0x1B

CF_UNICODETEXT = 13
GMEM_MOVEABLE = 0x0002

# The text typed into the note. Fixed so the character count is a fact the gate can
# assert: 43 characters, and nothing in the test depends on the words.
SAMPLE = "The quick brown fox jumps over the lazy dog"
SENTINEL = "SENTINEL-7412 keep me"


# --- the clipboard, independently of the product ----------------------------


def clipboard_sequence() -> int:
    return user32.GetClipboardSequenceNumber()


def _open():
    for _ in range(20):
        if user32.OpenClipboard(None):
            return True
        time.sleep(0.01)
    return False


def clipboard_text():
    """The clipboard's text, or `None` when it holds none."""
    if not _open():
        raise RuntimeError("another program held the clipboard for 200 ms")
    try:
        handle = user32.GetClipboardData(CF_UNICODETEXT)
        if not handle:
            return None
        kernel32.GlobalLock.restype = ctypes.c_void_p
        pointer = kernel32.GlobalLock(handle)
        if not pointer:
            return None
        try:
            return ctypes.wstring_at(pointer)
        finally:
            kernel32.GlobalUnlock(handle)
    finally:
        user32.CloseClipboard()


def clipboard_set(value) -> None:
    """Replace the clipboard with `value`, or empty it when `value` is `None`."""
    if not _open():
        raise RuntimeError("another program held the clipboard for 200 ms")
    try:
        user32.EmptyClipboard()
        if value is None:
            return
        data = value.encode("utf-16-le") + b"\x00\x00"
        kernel32.GlobalAlloc.restype = ctypes.c_void_p
        block = kernel32.GlobalAlloc(GMEM_MOVEABLE, len(data))
        kernel32.GlobalLock.restype = ctypes.c_void_p
        target = kernel32.GlobalLock(block)
        ctypes.memmove(target, data, len(data))
        kernel32.GlobalUnlock(block)
        if not user32.SetClipboardData(CF_UNICODETEXT, ctypes.c_void_p(block)):
            raise RuntimeError("SetClipboardData failed")
    finally:
        user32.CloseClipboard()


def clipboard_has_anything() -> bool:
    if not _open():
        return False
    try:
        return bool(user32.EnumClipboardFormats(0))
    finally:
        user32.CloseClipboard()


# --- plumbing ---------------------------------------------------------------


def text(path: str) -> str:
    if not os.path.exists(path):
        return ""
    with open(path, encoding="utf-8", errors="replace") as handle:
        return handle.read()


def find(title: str):
    return user32.FindWindowW(None, title)


_ENUM = ctypes.WINFUNCTYPE(ctypes.c_bool, ctypes.c_void_p, ctypes.c_void_p)
user32.EnumWindows.argtypes = [_ENUM, ctypes.c_void_p]
user32.GetWindowTextLengthW.argtypes = [ctypes.c_void_p]
user32.GetWindowTextW.argtypes = [ctypes.c_void_p, ctypes.c_wchar_p, ctypes.c_int]


def find_note() -> int:
    """The note window, by title prefix.

    Not by number: the suffix is the note's viewport index inside *this* process,
    so a gate that assumes `#1` misses a shell whose first note is `#0` -- and then
    types into whatever happens to be in front, which is a false pass waiting to
    happen rather than a failure.
    """
    hits = []

    def visit(hwnd, _):
        length = user32.GetWindowTextLengthW(hwnd)
        if length:
            buffer = ctypes.create_unicode_buffer(length + 1)
            user32.GetWindowTextW(hwnd, buffer, length + 1)
            if buffer.value.startswith(NOTE_PREFIX):
                hits.append(int(hwnd))
        return True

    user32.EnumWindows(_ENUM(visit), None)
    return hits[0] if hits else 0


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
    return box[0] < -10000 or box[1] < -10000


def parse_layout(line: str) -> dict:
    """`canvas=16,16,408,828 search=… selection=-` -> dict of tuples or None."""
    slots = {}
    for field in line.split():
        if "=" not in field:
            continue
        name, _, value = field.partition("=")
        if value == "-":
            slots[name] = None
            continue
        parts = value.split(",")
        if len(parts) != 4:
            continue
        try:
            slots[name] = tuple(float(part) for part in parts)
        except ValueError:
            continue
    return slots


def pixel_at(raw, width: int, height: int, x: float, y: float):
    """One pixel, clamped: a rounded layout rect can name the pixel past the edge."""
    return winshot.rgb(
        raw,
        width,
        min(max(int(x), 0), width - 1),
        min(max(int(y), 0), height - 1),
    )


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
    """One `--resident` process, one log, one store."""

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
        # Readiness is the registration line, not the process existing. The copy
        # below is driven by a real keystroke, and a chord another program owns
        # never arrives.
        wininput.wait_for(lambda: "hotkeys registered=" in text(self.log), 25)
        return self.process

    def alive(self) -> bool:
        return self.process is not None and self.process.poll() is None

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


def note_text(note: int) -> str:
    """What the note's editor holds right now."""
    edit = user32.FindWindowExW(note, None, "RICHEDIT50W", None)
    if not edit:
        return ""
    buffer = ctypes.create_unicode_buffer(4096)
    _plain_user32.SendMessageW(
        ctypes.c_void_p(edit), WM_GETTEXT, 4096, ctypes.cast(buffer, ctypes.c_void_p)
    )
    return buffer.value


def note_probe(note: int) -> str:
    """Diagnostics for the precondition: which window, and what it answered."""
    edit = user32.FindWindowExW(note, None, "RICHEDIT50W", None)
    buffer = ctypes.create_unicode_buffer(4096)
    copied = 0
    if edit:
        copied = _plain_user32.SendMessageW(
            ctypes.c_void_p(edit), WM_GETTEXT, 4096, ctypes.cast(buffer, ctypes.c_void_p)
        )
    return f"note={note} edit={edit} copied={copied} text={buffer.value!r}"


def open_note_with(shell: Shell, body: str):
    """Create a note and type `body` into it. Returns `(hwnd, typed)`.

    `typed` is read back rather than assumed. A keystroke with the wrong window in
    front lands nowhere and leaves no trace -- the note is empty, `Ctrl+A` selects
    nothing, and every later check fails with "no copy" while the actual fault is
    two steps earlier.
    """
    before = shell.count("note_spawned")
    shell.press(NEW_NOTE)
    wininput.wait_for(lambda: find_note() != 0, 10)
    note = find_note()
    # The first chord after launch is unreliable until the editor has the focus
    # (wininput's own note says so); wait for the line the shell logs, not a sleep.
    shell.later("editor_focus", 0, 5)
    wininput.wait_for(lambda: shell.count("note_spawned") > before, 5)

    for _ in range(3):
        wininput.force(note)
        time.sleep(0.1)
        wininput.type_text(body)
        # Polled rather than slept on: the keystrokes go through the shell's own
        # event loop, and the first ones after a spawn are the ones that can be
        # lost. A fixed sleep is either too short (flaky gate) or too long (every
        # run pays for the worst case).
        deadline = time.time() + 2.0
        while time.time() < deadline:
            if note_text(note).replace("\r", "").strip() == body:
                return note, True
            time.sleep(0.1)
        # Whatever partial text arrived has to go, or the retry appends to it and
        # the comparison can never come out equal.
        wininput.chord(VK_A, ctrl=True)
        wininput.tap(VK_DELETE)
        time.sleep(0.15)
    return note, False


def take_keyboard(note: int) -> str:
    """Put the caret back in the note, the way a user would.

    `force` makes the *window* foreground, which is not the same as giving the
    RichEdit the keyboard: after the panel is parked, the note is in front with no
    focused control, so `Ctrl+A` reaches nothing and the capture reports "no copy".
    A click is what actually hands the keyboard to the control under it.
    """
    before = user32.GetForegroundWindow()
    forced = wininput.force(note)
    time.sleep(0.05)
    wininput.click_client(note, 40, 40)
    time.sleep(0.15)
    return (
        f"note={note} fg_before={before} force={forced} "
        f"fg_after={user32.GetForegroundWindow()} text_len={len(note_text(note))}"
    )


def selection_is_live(note: int) -> bool:
    """Whether a `Ctrl+C` right now would copy the note's selection.

    Proved rather than assumed, and with the product's own method: clear the
    clipboard, press `Ctrl+C`, see what arrives. `EM_GETSEL` would answer the
    editor's opinion, and the thing under test is what the *clipboard* does.
    """
    clipboard_set(None)
    wininput.chord(VK_C, ctrl=True)
    time.sleep(0.2)
    return clipboard_text() == SAMPLE


def arm_selection(note: int, leave):
    """Select everything in the note and hand back the clipboard the caller wants.

    Returns whether the selection is actually live. Every scenario starts here, so
    a note that lost its keyboard reads as "the setup did not hold" instead of
    "the capture is broken" -- two very different bugs, and the log line for both
    of them is the same `reason=no_copy`.
    """
    trace = take_keyboard(note)
    wininput.chord(VK_A, ctrl=True)
    time.sleep(0.1)
    live = selection_is_live(note)
    clipboard_set(leave)
    return live, f"{trace} live={live}"


def collapse_selection(note: int) -> None:
    """Leave the caret with nothing selected: the 'no selection' case."""
    take_keyboard(note)
    wininput.tap(VK_RIGHT)
    time.sleep(0.1)


def reveal(shell: Shell) -> str:
    """Press the launcher chord and return the `launcher_selection` line it produced."""
    before = shell.count("launcher_selection")
    shell.press(LAUNCHER)
    return shell.later("launcher_selection", before, 6.0)


def field(line: str, key: str, default: str = "") -> str:
    """`chars=43 ms=12.3 prior=text` -> `field(line, "prior") == "text"`."""
    for part in line.split():
        name, _, value = part.partition("=")
        if name == key:
            return value
    return default


def close_panel(shell: Shell) -> str:
    """Esc out of the panel and return the `launcher_hide` line it produced."""
    if off_screen(rect(find(PANEL_TITLE))):
        return ""
    before = shell.count("launcher_hide")
    wininput.tap(VK_ESCAPE)
    wininput.wait_for(lambda: off_screen(rect(find(PANEL_TITLE))), 5)
    return shell.later("launcher_hide", before, 3.0)


def layout_with_selection(shell: Shell, timeout: float = 4.0):
    """The first layout line that reports a selection row.

    Reading the last line instead is a race, and one that looks like a product bug:
    the reveal returns as soon as the window is on screen, while the frame that
    draws the row is a message away -- and on a cold process that frame can be
    half a second late (the first content refresh is where the font atlas and the
    recent list are built).
    """
    deadline = time.time() + timeout
    while time.time() < deadline:
        lines = [line for line in text(shell.log).splitlines() if "launcher_layout" in line]
        for line in reversed(lines):
            slots = parse_layout(line)
            if slots.get("selection") is not None:
                return slots
        time.sleep(0.05)
    return None


def scenario_text(report: Report, shell: Shell, note: int) -> None:
    live, trace = arm_selection(note, SENTINEL)
    report.check("the note's selection is live before the panel opens", live, trace)
    line = reveal(shell)

    report.check(
        "a selection under the panel is taken",
        field(line, "chars") == str(len(SAMPLE)),
        line or "no launcher_selection line",
    )
    report.check(
        "and the clipboard is named for what it was",
        field(line, "prior") == "text" and field(line, "restored") == "true",
        f"prior={field(line, 'prior')} restored={field(line, 'restored')}",
    )
    report.check(
        "the sentinel is back, character for character",
        clipboard_text() == SENTINEL,
        repr(clipboard_text()),
    )
    # The budget is the ceiling on "the panel is up but ignores the keyboard", so a
    # capture that takes the whole of it is a reveal that took 100 ms.
    try:
        millis = float(field(line, "ms", "9999"))
    except ValueError:
        millis = 9999.0
    report.check(
        "and it does not spend the whole budget waiting",
        millis < 90.0,
        f"ms={millis:.1f}",
    )
    # The §7.1 budget, on a real key press rather than on the bench loop: the bench
    # runs with the capture switched off (a hundred synthetic reveals would type a
    # hundred Ctrl+C into whatever window is in front), so this is the only place
    # the shipped reveal-with-capture is timed end to end.
    shown = ""
    for line in text(shell.log).splitlines():
        if "launcher_shown" in line:
            shown = line
    try:
        shown_ms = float(field(shown, "shown_ms", "9999"))
    except ValueError:
        shown_ms = 9999.0
    report.check(
        "the reveal with a capture still fits the 60 ms budget",
        shown_ms < 60.0,
        f"shown_ms={shown_ms:.2f}",
    )


def scenario_empty(report: Report, shell: Shell, note: int) -> None:
    live, trace = arm_selection(note, None)
    report.check(
        "the selection is live again after the panel closed",
        live,
        trace + " (a false here is the focus path, not the capture)",
    )
    line = reveal(shell)

    report.check(
        "an empty clipboard is still an empty clipboard afterwards",
        field(line, "prior") == "empty" and clipboard_text() is None,
        f"prior={field(line, 'prior')} clipboard={clipboard_text()!r}",
    )
    report.check(
        "and the selection is taken anyway",
        field(line, "chars") == str(len(SAMPLE)),
        line or "no launcher_selection line",
    )


def scenario_nothing(report: Report, shell: Shell, note: int) -> None:
    clipboard_set(SENTINEL)
    collapse_selection(note)
    line = reveal(shell)

    report.check(
        "nothing selected is a miss, not an empty transcription",
        field(line, "chars") == "0" and field(line, "reason") == "no_copy",
        line or "no launcher_selection line",
    )
    # The Tauri build cleared the clipboard on this path when the previous value
    # held no text. A miss means the clipboard was never touched, so nothing has to
    # be put back -- and putting back "empty" would delete a screenshot the user
    # took between the key press and the copy.
    report.check(
        "and the clipboard it did not touch is untouched",
        clipboard_text() == SENTINEL,
        repr(clipboard_text()),
    )


def scenario_pixels(report: Report, shell: Shell) -> None:
    """The capture reached the panel, not just the log."""
    hwnd = find(PANEL_TITLE)
    if not hwnd:
        report.check("the panel is on screen", False, "no window")
        return
    report.check("the panel is on screen", not off_screen(rect(hwnd)), f"rect={rect(hwnd)}")

    slots = layout_with_selection(shell)
    if slots is None:
        report.check("the frame reports a selection row", False, "no layout line with a selection")
        return
    report.check("the frame reports a selection row", True, f"selection={slots['selection']}")

    width, height, raw = winshot.client_pixels(hwnd)
    if not raw:
        report.check("the panel's pixels can be read", False, "PrintWindow returned nothing")
        return
    # Logical points to pixels through the panel's own width: the layout line is in
    # points and the bitmap is in pixels, and the scale between them is whatever
    # this monitor has (125% here). Deriving it from the canvas avoids asserting on
    # the machine's DPI.
    canvas = slots["canvas"]
    logical_width = canvas[2] + 2 * canvas[0]
    scale = width / logical_width if logical_width else 1.0

    row = slots["selection"]
    inside = pixel_at(
        raw,
        width,
        height,
        (row[0] + 5.0) * scale,
        (row[1] + row[3] / 2.0) * scale,
    )
    page = pixel_at(
        raw,
        width,
        height,
        (canvas[0] + 3.0) * scale,
        (canvas[1] + 3.0) * scale,
    )

    report.check(
        "the selection row is painted, and it is not the page colour",
        inside != page,
        f"row={inside} page={page}",
    )
    # An accent wash, not a grey band: the theme's `accent_soft` is the page moved
    # towards the accent, so blue gains more than red does.
    report.check(
        "the row reads as the accent wash, not as a second search box",
        inside[2] - inside[0] >= 10,
        f"row={inside} (b-r={inside[2] - inside[0]})",
    )


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--exe", required=True)
    parser.add_argument("--keep", action="store_true", help="keep the temp dir")
    args = parser.parse_args()

    exe = os.path.abspath(args.exe)
    if not os.path.exists(exe):
        print(f"RESULT: FAIL (no exe at {exe})")
        return 1

    root = tempfile.mkdtemp(prefix="tg0-selection-gate-")
    report = Report()
    shell = Shell(exe, root)
    print(f"# {root}")
    try:
        shell.start()
        note, typed = open_note_with(shell, SAMPLE)
        if not report.check(
            "the note holds the text before the panel opens",
            typed,
            note_probe(note),
        ):
            return report.finish()

        scenario_text(report, shell, note)
        scenario_pixels(report, shell)
        hide = close_panel(shell)
        report.check(
            "Esc puts the keyboard back where it came from",
            field(hide, "refocus").endswith(":true"),
            hide or "no launcher_hide line",
        )

        scenario_empty(report, shell, note)
        close_panel(shell)

        scenario_nothing(report, shell, note)
        close_panel(shell)

        report.check(
            "the shell is still running after three captures",
            shell.alive(),
            f"alive={shell.alive()}",
        )
    finally:
        shell.stop()
        if args.keep:
            print(f"# kept {root}")
        else:
            shutil.rmtree(root, ignore_errors=True)

    return report.finish()


if __name__ == "__main__":
    sys.exit(main())
