"""Phase 1 gate: the panel's colours come from the theme, not from call sites.

    §5.4      the launcher is a dark action surface in both themes, so its colours
              are fixed tokens rather than the OS palette
    §7.1      the reveal budget is unchanged by any of this (asserted by the
              launcher gate; this one only reads pixels)

Why pixels and not unit tests: the theme's own tests prove that `surface_soft`
sits between `canvas` and `surface`, and that a lift moves towards the text colour.
They cannot prove that the panel *draws* those values -- a call site that passed
`Color32::from_rgb(30, 30, 30)` would pass every unit test in the crate and paint a
flat grey panel. So this gate asks the window what it put on screen.

The geometry it samples is not re-derived here. `launcher_ready` carries the
rectangles the frame actually used (`canvas=`, `selected_card=`, ...), so moving a
constant in `hub-ui` moves the gate with it. A gate that recomputed the layout from
its own copy of the spacing constants would keep sampling the old coordinates and
pass on a panel that draws somewhere else.

Run:
    python scripts/check-native-theme.py --exe native/target/release/easyflowhub-native.exe
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
except (ImportError, OSError, AttributeError):  # not on Windows
    print("RESULT: SKIP (this gate reads a Windows window's pixels)")
    sys.exit(0)

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "lib"))
import wininput  # noqa: E402
import winshot  # noqa: E402

PANEL_TITLE = "easyflowhub-launcher"

# The shipped default is Ctrl+Alt+Space, but the Tauri build owns it while it is
# running. F9..F11 are unused by both builds; the first one Windows accepts wins.
CHORDS = [("Ctrl+Alt+F9", 0x78), ("Ctrl+Alt+F10", 0x79), ("Ctrl+Alt+F11", 0x7A)]

STAMP = "2026-11-26T02:31:00Z"
SEEDED = [
    ("gate-theme-a", "周末计划", "周末计划\n买螺丝刀和膨胀管"),
    ("gate-theme-b", "接口设计", "接口设计\nlauncher 的动作契约"),
    ("gate-theme-c", "面板笔记", "面板笔记\n面板要能在光标处出现"),
]

# `Theme::standard().canvas` and `surface`, written out. This is the one place the
# gate knows a colour, and it knows it so that "the fill of the page" can be
# checked against the theme file rather than against a relation that any dark grey
# would satisfy. If the theme changes, this line changes with it -- it is an
# expectation, not a copy of the implementation.
CANVAS_RGB = (24, 27, 33)

# Tolerance per channel. DIB channels are reached through a colour-managed path on
# some systems, and a 1-2 step difference is invisible; a fill that came from the
# wrong token is off by 10-40.
TOL = 4

# How many VK_DOWNs the notes scenario will spend looking for the note rows. Not
# the expected count -- a ceiling, so a cursor that never arrives fails the check
# instead of hanging the gate.
NOTE_WALK_LIMIT = 24


def text(path: str) -> str:
    if not os.path.exists(path):
        return ""
    with open(path, encoding="utf-8", errors="replace") as handle:
        return handle.read()


def find(title: str):
    return user32.FindWindowW(None, title)


def window_rect(hwnd: int):
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
    """Parked: still visible (so it stays repaintable), but no monitor holds it."""
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


def parse_layout(line: str) -> dict:
    """Pull the `name=x,y,w,h` pairs out of a `launcher_ready` line."""
    out = {}
    for token in line.split():
        if "=" not in token:
            continue
        key, value = token.split("=", 1)
        if value == "-":
            out[key] = None
        elif value.count(",") == 3:
            out[key] = tuple(float(part) for part in value.split(","))
        else:
            out[key] = value
    return out


def near(actual, expected, tol: int = TOL) -> bool:
    return actual is not None and all(
        abs(a - b) <= tol for a, b in zip(actual, expected)
    )


def luminance(c) -> float:
    """Rough luma, 0..255. Only ever used to compare two colours on one screen."""
    if c is None:
        return -1.0
    return 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2]


def show(c) -> str:
    return "none" if c is None else "#%02X%02X%02X" % c


def save_bmp(path: str, shot: "Shot") -> None:
    """Write the captured frame as a 24-bit BMP.

    BMP rather than PNG because the gates run on a bare Windows box with no
    third-party modules, and a BMP writer is forty lines of `struct`. Same reason
    `winshot` talks to GDI directly.
    """
    import struct

    row = shot.width * 3
    pad = (4 - row % 4) % 4
    size = (row + pad) * shot.height
    header = struct.pack("<2sIHHI", b"BM", 14 + 40 + size, 0, 0, 14 + 40)
    info = struct.pack(
        "<IiiHHIIiiII", 40, shot.width, shot.height, 1, 24, 0, size, 2835, 2835, 0, 0
    )
    with open(path, "wb") as handle:
        handle.write(header)
        handle.write(info)
        # Bottom-up, BGR: `winshot` gives top-down BGRA, so walk the rows backwards
        # and keep the first three bytes of each pixel.
        for y in range(shot.height - 1, -1, -1):
            line = bytearray(row)
            base = y * shot.width * 4
            for x in range(shot.width):
                offset = base + x * 4
                line[x * 3 : x * 3 + 3] = shot.raw[offset : offset + 3]
            handle.write(bytes(line))
            handle.write(b"\0" * pad)


class Shot:
    """One printed frame plus the mapping from layout points to its pixels."""

    def __init__(self, hwnd: int, layout: dict) -> None:
        width, height, raw = winshot.client_pixels(hwnd)
        if not raw:
            raise SystemExit("PrintWindow returned no pixels; is the panel on screen?")
        self.width = width
        self.height = height
        self.raw = raw
        # The window is 440 logical points wide by construction, but `place` may
        # narrow it on a small work area, so the scale is solved from the canvas
        # the frame reported rather than assumed to be the one at startup.
        canvas = layout.get("canvas")
        if not canvas:
            raise SystemExit("no canvas rect in launcher_ready")
        self.scale = width / (canvas[2] + 2 * 16.0)
        self.layout = layout

    def at(self, slot: str, dx: float, dy: float):
        """One pixel, `dx`/`dy` in **logical points** from the slot's rect."""
        rect = self.layout.get(slot)
        if not rect:
            return None
        x = int(round((rect[0] + dx) * self.scale))
        y = int(round((rect[1] + dy) * self.scale))
        if not (0 <= x < self.width and 0 <= y < self.height):
            return None
        return winshot.rgb(self.raw, self.width, x, y)

    def bluest(self, slot: str, dx: float, dy: float, span: float, step: float = 0.25):
        """The bluest pixel down a short vertical span, with its offset.

        A one-point border is thinner than two pixels at the 125% scaling this
        machine runs at, and `chrome` fills a field before it strokes the ring,
        so one sample can land on the fill just inside the border and report a
        grey where the accent is. Measured: the search field's top border is one
        row at `#7EA5D6`, the next row blends, and the row after that is the
        surface -- `dy=1.0` read the surface and the ring looked absent.

        Scanning `span` points instead of picking a `dy` keeps the gate from
        depending on which side of a rounding the border fell, and on this
        machine's scaling factor.
        """
        best, best_offset, offset = None, None, 0.0
        while offset <= span + 1e-9:
            pixel = self.at(slot, dx, dy + offset)
            if pixel is not None and (best is None or pixel[2] - pixel[0] > best[2] - best[0]):
                best, best_offset = pixel, offset
            offset += step
        return best_offset, best


class Session:
    def __init__(self, exe: str, root: str, name: str) -> None:
        self.exe = exe
        self.root = root
        self.name = name
        self.log = os.path.join(root, f"{name}.log")
        self.settings = os.path.join(root, f"{name}.json")
        self.process = None
        self.vk = None
        self.chord = None

    def start(self, chord: str, vk: int, extra=None) -> bool:
        self.stop()
        self.chord, self.vk = chord, vk
        with open(self.settings, "w", encoding="utf-8") as handle:
            json.dump({"shortcuts": {"launcher": chord}}, handle)
        command = [
            self.exe,
            "--note",
            "--settings",
            self.settings,
            "--store-root",
            os.path.join(self.root, "store"),
            "--log",
            self.log,
        ]
        command.extend(extra or [])
        self.process = subprocess.Popen(
            command, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL
        )
        wininput.wait_for(lambda: "hotkeys registered=" in text(self.log), 25)
        return f"combo={chord} registered=true" in text(self.log)

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

    def press(self) -> None:
        wininput.chord(self.vk, ctrl=True, alt=True)

    def wait_parked(self, timeout: float = 10.0) -> bool:
        return wininput.wait_for(
            lambda: "launcher_window parked=true" in text(self.log), timeout
        )

    def reveal(self, timeout: float = 6.0) -> bool:
        """Show the panel and wait for the first frame that is what the user sees.

        `launcher_ready` is the frame the query box took the keyboard, which is
        also the first frame with the focus ring on it -- sampling earlier would
        judge a panel whose outline had not settled.
        """
        hwnd = find(PANEL_TITLE)
        if hwnd and off_screen(window_rect(hwnd)):
            time.sleep(0.25)
        # Both baselines are taken *before* the press. `launcher_shown` and
        # `launcher_ready` are 17 ms apart on this machine, so counting the ready
        # lines after the show was seen reads a file that already contains the
        # ready line -- and then waits for a second one that never comes. Measured
        # the hard way: the gate failed with both lines sitting in the log.
        shown_before = self.count("launcher_shown")
        ready_before = self.count("launcher_ready")
        self.press()
        if not self.later("launcher_shown", shown_before, timeout):
            # The OS dropped the injected chord: it raced the foreground change the
            # park made. A hand would press again; so does the gate, once.
            time.sleep(0.4)
            self.press()
            if not self.later("launcher_shown", shown_before, timeout):
                return False
        return bool(self.later("launcher_ready", ready_before, timeout))

    def layout(self) -> dict:
        """The rectangles of the most recent frame that reported any."""
        lines = [x for x in text(self.log).splitlines() if "launcher_layout" in x]
        if not lines:
            raise SystemExit("no launcher_layout line: the panel never reported its frame")
        return parse_layout(lines[-1])

    def shot(self, attempts: int = 3) -> Shot:
        """A frame whose layout line matches the pixels that came back."""
        hwnd = find(PANEL_TITLE)
        if not hwnd:
            raise SystemExit("the panel window is gone")
        last = None
        for _ in range(attempts):
            time.sleep(0.12)
            last = Shot(hwnd, self.layout())
        return last

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
    """Let the app create its schema, then add the rows the recent list shows.

    The schema is not duplicated here: a gate that re-declares the tables it tests
    starts passing against a schema the product no longer has.
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


def register(session: Session, report: Report, label: str = "", extra=None) -> bool:
    name = "the hotkey is registered, so the gate is driving the real path"
    for chord, vk in CHORDS:
        if session.start(chord, vk, extra):
            report.check(f"{label}{name}", True, f"combo={chord}")
            return session.wait_parked()
        session.stop()
    report.check(
        f"{label}{name}",
        False,
        "every candidate chord was refused; something else owns all of them",
    )
    return False


def scenario_standard(session: Session, shot, report: Report) -> None:
    """The shipped theme, with the cursor on the first command card."""
    box = window_rect(find(PANEL_TITLE))
    report.check("the panel is on screen, not parked", not off_screen(box), f"rect={box}")
    layout = shot.layout
    report.check(
        "the frame reports the rectangles it used",
        layout.get("canvas") and layout.get("search") and layout.get("selected_card")
        and layout.get("rest_card"),
        f"canvas={layout.get('canvas')} search={layout.get('search')} "
        f"cards={layout.get('selected_card')}/{layout.get('rest_card')}",
    )

    scale_line = session.line("launcher_window parked=true")
    declared = 0.0
    for token in scale_line.split():
        if token.startswith("scale="):
            declared = float(token.split("=", 1)[1])
    report.check(
        "the pixels map to the layout at the window's own scale",
        declared > 0 and abs(shot.scale - declared) < 0.03,
        f"measured={shot.scale:.3f} declared={declared:.3f}",
    )

    # -- page ------------------------------------------------------------------
    canvas = shot.at("canvas", 6, 6)
    report.check(
        "the page is the theme's canvas, not whatever was to hand",
        near(canvas, CANVAS_RGB),
        f"sampled={show(canvas)} expected={show(CANVAS_RGB)}",
    )

    # -- search field ----------------------------------------------------------
    field = shot.at("search", 8, layout["search"][3] / 2.0)
    report.check(
        "the query field is one step above the page",
        field is not None and luminance(field) > luminance(canvas) + 4,
        f"field={show(field)} page={show(canvas)}",
    )

    # The ring is one logical point wide; sample across it rather than hoping a
    # half-point lands on the right row. See `Shot.bluest`.
    ring_at, ring = shot.bluest("search", layout["search"][2] / 2.0, 0.0, 3.0)
    report.check(
        "a focused field is outlined in the accent, so the caret has a home",
        ring is not None and ring[2] - ring[0] > 20,
        f"top border={show(ring)} at dy={ring_at} "
        f"(accent is blue: b > r by a wide margin)",
    )

    # -- cards -----------------------------------------------------------------
    selected = shot.at(
        "selected_card", layout["selected_card"][2] - 14, layout["selected_card"][3] / 2.0
    )
    rest = shot.at(
        "rest_card", layout["rest_card"][2] - 14, layout["rest_card"][3] / 2.0
    )
    report.check(
        "the selected card and an unselected card are different colours",
        selected is not None and rest is not None and not near(selected, rest, 2),
        f"selected={show(selected)} rest={show(rest)}",
    )
    report.check(
        "the selected card is the accent wash: bluer than it is red",
        selected is not None and selected[2] > selected[0] + 8,
        f"selected={show(selected)}",
    )
    report.check(
        "an unselected card still sits above the page",
        rest is not None and luminance(rest) > luminance(canvas) + 2,
        f"rest={show(rest)} page={show(canvas)}",
    )


def scenario_hover(session: Session, report: Report, rest_rgb) -> None:
    """Point at the card the cursor is not on; it must lift out of the page."""
    layout = session.layout()
    card = layout.get("rest_card")
    if not card:
        report.check("hover lifts a card", False, "no unselected card to hover")
        return

    shot = session.shot()
    # Move to the middle of the unselected card, in screen pixels.
    box = window_rect(find(PANEL_TITLE))
    x = int(round(box[0] + (card[0] + card[2] / 2.0) * shot.scale))
    y = int(round(box[1] + (card[1] + card[3] / 2.0) * shot.scale))
    user32.SetCursorPos(x, y)
    time.sleep(0.35)

    hovered = session.shot().at("rest_card", card[2] - 14, card[3] / 2.0)
    report.check(
        "hovering lifts the card the cursor is not on",
        hovered is not None and rest_rgb is not None
        and luminance(hovered) > luminance(rest_rgb) + 3,
        f"hover={show(hovered)} rest={show(rest_rgb)}",
    )

    # Back to a blank part of the page: leaving the pointer on a card would make
    # the next screenshot's "rest" card a hovered one.
    user32.SetCursorPos(
        int(round(box[0] + (layout["canvas"][0] + 20) * shot.scale)),
        int(round(box[1] + (layout["canvas"][1] + layout["canvas"][3] - 60) * shot.scale)),
    )
    time.sleep(0.2)


def scenario_notes(session: Session, report: Report, report_name: str) -> None:
    """Walk down to the note rows: the same three states, on the same tokens."""
    # How far the cursor walks is a property of the command list, and that list
    # grows whenever a command is added: five presses were right until the sixth
    # card landed, then they stopped one short of the notes and three checks
    # failed with `selected_note=None`. Press until the frame says the cursor is
    # on a note instead of counting cards, and report the count so a cursor that
    # is stuck or too slow is visible in the detail, not only in the verdict.
    presses = 0
    while presses < NOTE_WALK_LIMIT and not session.layout().get("selected_note"):
        wininput.tap(0x28)  # VK_DOWN
        presses += 1
        time.sleep(0.08)
    time.sleep(0.25)

    shot = session.shot()
    layout = shot.layout
    selected = shot.at("selected_note", layout["selected_note"][2] / 2.0, layout["selected_note"][3] / 2.0) if layout.get(
        "selected_note"
    ) else None
    rest = shot.at("rest_note", layout["rest_note"][2] / 2.0, layout["rest_note"][3] / 2.0) if layout.get(
        "rest_note"
    ) else None
    report.check(
        f"{report_name}: the cursor reaches the note rows",
        layout.get("selected_note") is not None,
        f"selected_note={layout.get('selected_note')} after {presses} presses",
    )
    report.check(
        f"{report_name}: a selected note and an unselected note differ",
        selected is not None and rest is not None and not near(selected, rest, 2),
        f"selected={show(selected)} rest={show(rest)}",
    )
    report.check(
        f"{report_name}: the selected note carries the accent wash",
        selected is not None and selected[2] > selected[0] + 8,
        f"selected={show(selected)}",
    )


def scenario_lite(root: str, exe: str, report: Report, standard_canvas) -> None:
    """`--lite` keeps one step of separation; it does not flatten the panel."""
    session = Session(exe, root, "lite")
    try:
        if not register(session, report, "lite: ", ["--lite"]):
            return
        if not session.reveal():
            report.check("lite: the panel reveals", False, "no launcher_ready")
            return
        shot = session.shot()
        layout = shot.layout
        canvas = shot.at("canvas", 6, 6)
        rest = shot.at("rest_card", layout["rest_card"][2] - 14, layout["rest_card"][3] / 2.0) if layout.get(
            "rest_card"
        ) else None
        report.check(
            "lite: the page is darker than the standard theme's",
            canvas is not None
            and standard_canvas is not None
            and luminance(canvas) < luminance(standard_canvas),
            f"lite={show(canvas)} standard={show(standard_canvas)}",
        )
        report.check(
            "lite: cards still sit above the page",
            rest is not None and canvas is not None
            and luminance(rest) > luminance(canvas) + 1,
            f"rest={show(rest)} page={show(canvas)}",
        )
    finally:
        session.stop()


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--exe", required=True, help="path to easyflowhub-native.exe")
    parser.add_argument("--keep", action="store_true", help="keep the temp directory")
    parser.add_argument("--save", help="also write the standard frame here, as a BMP")
    args = parser.parse_args()

    exe = os.path.abspath(args.exe)
    if not os.path.exists(exe):
        raise SystemExit(f"no such executable: {exe}")

    root = tempfile.mkdtemp(prefix="p1-theme-gate-")
    report = Report()
    try:
        seed(root, exe)
        session = Session(exe, root, "standard")
        try:
            if not register(session, report) or not session.reveal():
                tail = " | ".join(text(session.log).splitlines()[-4:])
                report.check("the panel reveals", False, f"log tail: {tail}")
                return report.finish()
            shot = session.shot()
            if args.save:
                save_bmp(args.save, shot)
                print(f"saved {args.save} ({shot.width}x{shot.height})")
            scenario_standard(session, shot, report)
            rest = shot.layout
            rest_rgb = (
                shot.at("rest_card", rest["rest_card"][2] - 14, rest["rest_card"][3] / 2.0)
                if rest.get("rest_card")
                else None
            )
            standard_canvas = shot.at("canvas", 6, 6)
            scenario_hover(session, report, rest_rgb)
            scenario_notes(session, report, "notes")
        finally:
            session.stop()
        scenario_lite(root, exe, report, standard_canvas)
        return report.finish()
    finally:
        if not args.keep:
            shutil.rmtree(root, ignore_errors=True)


if __name__ == "__main__":
    sys.exit(main())
