"""Type into a note's text island, and read back what it holds.

Why `WM_CHAR` instead of synthesising keystrokes: `WM_CHAR` is the message the
control actually edits from, and it carries the character directly, so a CJK body
can be typed without a keyboard layout. It also takes the same path a real key
takes inside the shell -- the edit signal fires, the autosave debounce starts --
which is the part under test.

The three ways in are deliberately different, because they exercise different
paths:

    --type    WM_CHAR per character    the normal path: signal -> debounce -> save
    --set     SetWindowTextW           NO message the shell watches, so only the
                                       2 s scan can notice. This is the "a signal
                                       was missed" case, on purpose.
    --esc     WM_KEYDOWN VK_ESCAPE     the close path, which must flush

Usage:
    python poke-note.py --type "买菜清单\\n鸡蛋"
    python poke-note.py --read
    python poke-note.py --set "整段替换"
    python poke-note.py --esc
    python poke-note.py --info
"""

import argparse
import ctypes
import sys
from ctypes import wintypes

user32 = ctypes.WinDLL("user32", use_last_error=True)
user32.SetProcessDpiAwarenessContext(ctypes.c_void_p(-4))
user32.FindWindowW.restype = wintypes.HWND
user32.FindWindowW.argtypes = [wintypes.LPCWSTR, wintypes.LPCWSTR]
user32.FindWindowExW.restype = wintypes.HWND
user32.FindWindowExW.argtypes = [wintypes.HWND, wintypes.HWND, wintypes.LPCWSTR, wintypes.LPCWSTR]

WM_CHAR = 0x0102
WM_KEYDOWN = 0x0100
VK_ESCAPE = 0x1B
EM_SETSEL = 0x00B1


def island(title: str) -> int:
    """The note's RichEdit child, or exit 2 with the window count for context."""
    note = user32.FindWindowW(None, title)
    if not note:
        sys.exit(f"no window titled {title!r} (is the shell running with --note?)")
    child = user32.FindWindowExW(note, None, "RICHEDIT50W", None)
    if not child:
        sys.exit(f"window {title!r} has no RICHEDIT50W child yet")
    return child


def read(hwnd: int) -> str:
    length = user32.GetWindowTextLengthW(hwnd)
    if length <= 0:
        return ""
    buffer = ctypes.create_unicode_buffer(length + 1)
    copied = user32.GetWindowTextW(hwnd, buffer, length + 1)
    return buffer[: max(copied, 0)]


def type_text(hwnd: int, text: str) -> int:
    """One `WM_CHAR` per UTF-16 code unit.

    Two details that decide whether this looks like typing at all:

    * a line break is `0x0D` (CR), not `0x0A`. RichEdit's paragraph mark is CR, and
      a bare LF is inserted as a character it then has to normalise -- the note ends
      up with different breaks than a person pressing Enter would produce.
    * an astral character (emoji) is two code units, so the string is encoded to
      UTF-16 first rather than iterated as code points.
    """
    raw = text.replace("\n", "\r").encode("utf-16-le")
    sent = 0
    for offset in range(0, len(raw), 2):
        unit = int.from_bytes(raw[offset : offset + 2], "little")
        user32.SendMessageW(hwnd, WM_CHAR, unit, 1)
        sent += 1
    return sent


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--title", default="easyflowhub-note#0")
    group = parser.add_mutually_exclusive_group(required=True)
    group.add_argument("--type", dest="typed")
    group.add_argument("--set", dest="replaced")
    group.add_argument("--read", action="store_true")
    group.add_argument("--esc", action="store_true")
    group.add_argument("--info", action="store_true")
    parser.add_argument(
        "--append",
        action="store_true",
        help="move the caret to the end before typing (default for --type)",
    )
    args = parser.parse_args()

    hwnd = island(args.title)

    if args.info:
        before = read(hwnd)
        print(f"island=0x{hwnd:X} chars={len(before)}")
        return 0

    if args.read:
        text = read(hwnd)
        # Escaped on one line so a probe log line can be diffed byte for byte;
        # CRLF shows up as `\r\n` here, which is the whole point of looking.
        print(f"chars={len(text)} text={text!r}")
        return 0

    if args.typed is not None:
        if args.append:
            end = user32.GetWindowTextLengthW(hwnd)
            user32.SendMessageW(hwnd, EM_SETSEL, end, end)
        print(f"typed={type_text(hwnd, args.typed)} chars={len(read(hwnd))}")
        return 0

    if args.replaced is not None:
        # Straight into the control: no `WM_CHAR`, so no edit signal. The shell can
        # only find this through the periodic scan.
        user32.SetWindowTextW(hwnd, args.replaced)
        print(f"set chars={len(read(hwnd))} signal=none")
        return 0

    if args.esc:
        user32.PostMessageW(hwnd, WM_KEYDOWN, VK_ESCAPE, 0)
        print("esc posted")
        return 0

    return 0


if __name__ == "__main__":
    sys.exit(main())
