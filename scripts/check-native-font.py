"""C1 gate: the CJK font must not be charged to the process, and must still render.

    C1  `mode=mapped` and `bytes=` equal to the font file on disk
    C1  commit (PagefileUsage) -- the metric G0 fixed at "<= 100 MB"
    C1  the font costs the process ~nothing privately: mapped - no-font is small
    C1  the heap path really costs what we removed: heap - mapped is large
    C1  the same face is painted either way: mapped and heap differ in < 1% of
        pixels
    C1  the font is doing the work: no-font differs from mapped in > 2% of pixels

## Why six checks

The tempting gate is one number: "commit is under 100 MB". It is also the one that
can pass for the wrong reason. If the font silently failed to load, commit would
drop *and* the number would improve -- and the panel would render tofu, which a
memory probe cannot see.

So the gate is built as a **controlled comparison** of three runs that differ in
exactly one thing, the loading path:

| run | flag | what it means |
|---|---|---|
| mapped | (none) | the product path: `ab_glyph::FontRef` over a file mapping |
| heap | `--font-heap` | the previous path: owned `Vec<u8>` + epaint's clone |
| none | `--no-font` | no CJK font at all -- egui's default stack, tofu |

Memory is judged by *deltas* between runs rather than by an absolute number,
because an absolute number only says "this machine". The 100 MB line is still
asserted, but as the C1 target it is, with a flag to say so out loud.

Rendering is judged by pixels. `PrintWindow` of the same note window with the
switcher open (its first row is 新建笔记, drawn by egui, so it needs CJK glyphs)
gives an array per run:

* mapped vs heap must be nearly identical -- same face, same glyphs, only the
  bytes' home changed. A font that loaded but resolved to the *fallback* face
  would show up here.
* mapped vs none must differ clearly -- otherwise "no delta" would be
  indistinguishable from "the font was never used", which is the failure mode
  this whole batch exists to avoid.

Usage:
    python scripts/check-native-font.py --exe <path to easyflowhub-native.exe>
"""

import argparse
import json
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

NOTE_TITLE = "easyflowhub-note#0"
VK_K = 0x4B

# Medians of three runs: one run has a 8 MB spread on this machine, so a single
# sample would make the thresholds below a coin toss.
SAMPLES = 3
# What `mapped - none` is allowed to be: **drawing the panel's text**, not the
# mapping. `--no-font` renders tofu, so it pays for no glyphs at all, and the
# difference between the two runs is the glyph cache plus the 0.5 MB atlas.
#
# This threshold used to be 6.0, under the belief that the atlas was the only
# thing the two runs differed by. That was true of the spike panel, which drew a
# title and a list; it stopped being true when the panel became a real surface
# (theme tokens: six cards, chips, a footer, note rows). Measured 2026-11-27 on
# two binaries built from different commits, both 6.7-6.9 MB -- so it is the UI,
# not a regression in one batch.
#
# The claim "the 19.7 MB file is not charged to commit" is not answered here: it
# is answered by `heap - mapped`, where the same glyphs are painted from a
# different loader and the pixels differ by 0.00%. This number only has to stay
# far below the font size, which it does: if the mapping were ever charged, the
# difference would jump to ~26 MB.
TEXT_AND_ATLAS_MB = 12.0
# Measured 37.7 MB. Anything under 25 means the heap run did not actually take the
# expensive path, and then this gate proves nothing about what was removed.
HEAP_EXTRA_MB = 25.0
# Same face through a different buffer: only the caret's phase can differ.
SAME_PIXELS = 0.01
# Glyphs vs tofu: measured far above this; the point is that it is not zero.
DIFFERENT_PIXELS = 0.02


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


def work_root(name: str) -> str:
    """A scratch root per run: notes written by a gate must never touch the user's."""
    root = os.path.join(tempfile.gettempdir(), f"efh-c1-{name}")
    shutil.rmtree(root, ignore_errors=True)
    os.makedirs(root, exist_ok=True)
    return root


def probe_once(exe: str, name: str, extra: list) -> dict:
    """One `--probe` run: its JSON line, its `fonts` line, and its log path."""
    root = work_root(name)
    log = os.path.join(root, "note.log")
    finished = subprocess.run(
        [exe, "--note", *extra, "--probe", "--idle-seconds", "5",
         "--store-root", root, "--log", log],
        capture_output=True,
        text=True,
        timeout=120,
    )
    payload = None
    for line in finished.stdout.splitlines():
        if '"kind":"native-s1-probe"' in line:
            payload = json.loads(line)
    if payload is None:
        raise SystemExit(f"{name}: no probe line\n{finished.stdout}\n{finished.stderr}")

    fonts = ""
    for line in read(log).splitlines():
        if " fonts " in line:
            fonts = line.strip()
    payload["fonts"] = fonts
    payload["log"] = log
    return payload


def median(values: list) -> float:
    ordered = sorted(values)
    return ordered[len(ordered) // 2]


def probe(exe: str, name: str, extra: list, samples: int = SAMPLES) -> dict:
    """Median of `samples` runs.

    A single run is not enough: the same binary measured 91.5, 98.0 and 99.3 MB
    commit across three consecutive starts (glyph cache, GL driver scratch), i.e.
    a spread as wide as the effect being measured. Medians make the delta stable;
    the spread is printed so a future regression cannot hide inside noise.
    """
    runs = [probe_once(exe, f"{name}{index}", extra) for index in range(samples)]
    payload = {
        "private_mb": median([run["private_mb"] for run in runs]),
        "private_ws_mb": median([run["private_ws_mb"] for run in runs]),
        "working_set_mb": median([run["working_set_mb"] for run in runs]),
        "atlas": runs[0].get("atlas", "?"),
        "fonts": runs[0]["fonts"],
        "log": runs[0]["log"],
        "spread": (
            min(run["private_mb"] for run in runs),
            max(run["private_mb"] for run in runs),
        ),
    }
    return payload


def capture(exe: str, name: str, extra: list) -> tuple:
    """Window pixels with the switcher open, so egui has CJK text to draw.

    `extra` is what makes the three runs comparable: without it every capture is
    the mapped path and the pixel checks compare a run against itself -- which is
    exactly the bug the first draft had (three identical images, judged a pass).
    """
    root = work_root(name)
    log = os.path.join(root, "note.log")
    process = subprocess.Popen(
        [exe, "--note", *extra, "--store-root", root, "--log", log],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    try:
        window = wininput.wait_for(lambda: user32.FindWindowW(None, NOTE_TITLE), 25)
        if not window:
            raise SystemExit(f"{name}: the note window never appeared")
        wininput.wait_for(lambda: "editor_focus" in read(log), 10)
        if not open_switcher(window, log):
            raise SystemExit(f"{name}: the switcher never opened")
        # Logged on open, painted on the next pass.
        time.sleep(0.25)
        width, height, raw = winshot.client_pixels(window)
        if not raw:
            raise SystemExit(f"{name}: PrintWindow returned no pixels")
        return width, height, raw
    finally:
        process.terminate()
        try:
            process.wait(timeout=10)
        except subprocess.TimeoutExpired:
            process.kill()
        wininput.wait_for(lambda: user32.FindWindowW(None, NOTE_TITLE) == 0, 5)


def read(path: str) -> str:
    if not os.path.exists(path):
        return ""
    with open(path, encoding="utf-8", errors="replace") as handle:
        return handle.read()


# A stray `Ctrl+K` is the flakiest part of this gate: the first press can land
# before the window is foreground, and the log then shows nothing at all. Retrying
# is safe because a press that *did* open the switcher is logged, and the retry
# only happens when no `open=true` ever appeared -- pressing again after a
# successful open would toggle it closed.
ATTEMPTS = 5


def open_switcher(window: int, log: str, attempts: int = 5) -> bool:
    for _ in range(attempts):
        wininput.force(window)
        time.sleep(0.15)
        wininput.chord(VK_K, ctrl=True)
        deadline = time.time() + 0.6
        while time.time() < deadline:
            if "open=true" in read(log):
                return True
            time.sleep(0.05)
    return False


def ink_share(raw: bytes, width: int, height: int, threshold: int = 110) -> float:
    """Share of pixels whose every channel is brighter than `threshold`."""
    table = bytes(1 if value >= threshold else 0 for value in range(256))
    total = int.from_bytes(raw[0::4].translate(table), "big")
    for channel in (1, 2):
        total &= int.from_bytes(raw[channel::4].translate(table), "big")
    return total.bit_count() / float(width * height)


def differs(left: bytes, right: bytes) -> float:
    """Share of bytes that differ, counted exactly.

    Four bytes per pixel (B, G, R, A), so the number bounds the pixel share from
    above -- enough for "same face" vs "tofu", and it needs no threshold guess. An
    LSB-mask trick was tried first and undercounted by 3x (a byte that changes by
    an even amount keeps its low bit), which is exactly the kind of quiet error a
    gate must not have.
    """
    if len(left) != len(right):
        return 1.0
    return sum(1 for a, b in zip(left, right) if a != b) / float(len(left))


def mode_of(line: str) -> str:
    match = re.search(r"mode=([a-z]+)", line)
    return match.group(1) if match else ""


def size_of(line: str) -> int:
    match = re.search(r"bytes=(\d+)", line)
    return int(match.group(1)) if match else 0


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--exe", required=True)
    parser.add_argument("--font", default=r"C:\Windows\Fonts\msyh.ttc")
    parser.add_argument("--budget-commit-mb", type=float, default=100.0)
    parser.add_argument("--budget-pws-mb", type=float, default=100.0)
    args = parser.parse_args()

    report = Report()

    mapped = probe(args.exe, "mapped", [])
    heap = probe(args.exe, "heap", ["--font-heap"])
    none = probe(args.exe, "none", ["--no-font"])

    print("run      commit (min-max)      private-ws  working-set  atlas")
    for name, run in (("mapped", mapped), ("heap", heap), ("none", none)):
        low, high = run["spread"]
        print(
            f"{name:<8} {run['private_mb']:>6.1f} ({low:.1f}-{high:.1f})"
            f"   {run['private_ws_mb']:>8.1f}   {run['working_set_mb']:>9.1f}"
            f"   {run.get('atlas', '?')}"
        )
    print()

    # 1. The product path is the mapped one, and it mapped the whole file: a
    #    subsetted font would show a smaller `bytes=` and would have dropped
    #    coverage, which is the thing this design deliberately did not do.
    on_disk = os.path.getsize(args.font)
    report.check(
        "mapped mode is in force",
        mode_of(mapped["fonts"]) == "mapped",
        mapped["fonts"] or "no `fonts` line",
    )
    report.check(
        "the whole font file is mapped",
        size_of(mapped["fonts"]) == on_disk,
        f"log bytes={size_of(mapped['fonts'])} file={on_disk}",
    )

    # 2. The heap run must be the old path, otherwise check 3 is vacuous.
    report.check(
        "the heap run took the heap path",
        mode_of(heap["fonts"]) == "heap",
        heap["fonts"] or "no `fonts` line",
    )
    report.check(
        "the no-font run really skipped it",
        "skipped" in none["fonts"],
        none["fonts"] or "no `fonts` line",
    )

    # 3. What the file mapping bought, in the metric G0 chose.
    heap_extra = heap["private_mb"] - mapped["private_mb"]
    mapped_extra = mapped["private_mb"] - none["private_mb"]
    report.check(
        "the heap path costs what was removed",
        heap_extra >= HEAP_EXTRA_MB,
        f"heap - mapped = {heap_extra:.1f} MB (>= {HEAP_EXTRA_MB}) "
        f"-- same glyphs, same pixels, different loader",
    )
    report.check(
        "drawing the panel's text is not the font walking into commit",
        mapped_extra <= TEXT_AND_ATLAS_MB,
        f"mapped - none = {mapped_extra:.1f} MB (<= {TEXT_AND_ATLAS_MB}), "
        f"glyph cache + atlas (tofu costs nothing, so this is text, not the mapping)",
    )
    report.check(
        f"C1 target: commit <= {args.budget_commit_mb:.0f} MB",
        mapped["private_mb"] <= args.budget_commit_mb,
        f"{mapped['private_mb']:.1f} MB (was 130.1 on S1, 129.2 on the heap path)",
    )
    report.check(
        "private working set stays under the same line",
        mapped["private_ws_mb"] <= args.budget_pws_mb,
        f"{mapped['private_ws_mb']:.1f} MB (heap path {heap['private_ws_mb']:.1f})",
    )

    # 4. Pixels: same face either way, and the font is actually on screen.
    width, height, mapped_px = capture(args.exe, "mapped-px", [])
    _, _, heap_px = capture(args.exe, "heap-px", ["--font-heap"])
    _, _, none_px = capture(args.exe, "none-px", ["--no-font"])

    same = differs(mapped_px, heap_px)
    other = differs(mapped_px, none_px)
    ink = ink_share(mapped_px, width, height)
    report.check(
        "the mapped and heap runs paint the same face",
        same <= SAME_PIXELS,
        f"{same * 100:.2f}% of bytes differ (<= {SAME_PIXELS * 100:.0f}%)",
    )
    report.check(
        "the font is on screen, not skipped",
        ink > 0.005,
        f"{ink * 100:.2f}% of the window is text",
    )
    report.check(
        "the no-font run looks different, i.e. the pixels can tell",
        other >= DIFFERENT_PIXELS,
        f"{other * 100:.2f}% of bytes differ from mapped (>= {DIFFERENT_PIXELS * 100:.0f}%)",
    )

    return report.finish()


if __name__ == "__main__":
    sys.exit(main())
