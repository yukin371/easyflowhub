"""Send `Ctrl+wheel` to a note's text island, the way a real wheel notch arrives.

Why not a real mouse: the sandbox has none, and `WM_MOUSEWHEEL` with `MK_CONTROL`
in the low word is exactly what Windows posts when Ctrl is held -- the shell reads
the modifier from the message, not from `GetKeyState`, so an injected notch takes
the same path as a physical one.

Usage:
    python poke-wheel.py --down 5          # five notches towards the user
    python poke-wheel.py --up 2 --gap 60   # slower, so each notch passes the gate
    python poke-wheel.py --raw 120         # one notch with no Ctrl (a plain scroll)
    python poke-wheel.py --down 1 --egui   # the title-strip path (see below)

Why `--egui` differs: the island reads `MK_CONTROL` out of the message, so a posted
`WM_MOUSEWHEEL` is enough. The title strip goes through winit, which reads the
*modifier state* and turns the message into an egui scroll event -- so the chord has
to be held for real with `SendInput` around the post, or egui sees a plain scroll.
"""

import argparse
import ctypes
import sys
import time
from ctypes import wintypes

user32 = ctypes.WinDLL("user32", use_last_error=True)

WM_MOUSEWHEEL = 0x020A
MK_CONTROL = 0x0008
WHEEL_DELTA = 120
VK_CONTROL = 0x11

INPUT_KEYBOARD = 1
KEYEVENTF_KEYUP = 0x0002


class KEYBDINPUT(ctypes.Structure):
    _fields_ = [
        ("wVk", wintypes.WORD),
        ("wScan", wintypes.WORD),
        ("dwFlags", wintypes.DWORD),
        ("time", wintypes.DWORD),
        ("dwExtraInfo", ctypes.POINTER(ctypes.c_ulong)),
    ]


class KEYINPUT(ctypes.Structure):
    class _U(ctypes.Union):
        _fields_ = [("ki", KEYBDINPUT), ("padding", ctypes.c_byte * 32)]

    _anonymous_ = ("u",)
    _fields_ = [("type", wintypes.DWORD), ("u", _U)]


def hold_control(up):
    item = KEYINPUT(type=INPUT_KEYBOARD)
    item.ki.wVk = VK_CONTROL
    item.ki.dwFlags = KEYEVENTF_KEYUP if up else 0
    user32.SendInput(1, ctypes.byref(item), ctypes.sizeof(KEYINPUT))

find_child = user32.FindWindowExW
find_child.argtypes = [wintypes.HWND, wintypes.HWND, wintypes.LPCWSTR, wintypes.LPCWSTR]
find_child.restype = wintypes.HWND


def find_note(needle="easyflowhub-note"):
    found = []

    @ctypes.WINFUNCTYPE(wintypes.BOOL, wintypes.HWND, wintypes.LPARAM)
    def callback(hwnd, _lparam):
        if not user32.IsWindowVisible(hwnd):
            return True
        length = user32.GetWindowTextLengthW(hwnd)
        if length <= 0:
            return True
        buffer = ctypes.create_unicode_buffer(length + 1)
        user32.GetWindowTextW(hwnd, buffer, length + 1)
        if needle in buffer.value:
            found.append((hwnd, buffer.value))
        return True

    user32.EnumWindows(callback, 0)
    return found[0] if found else (None, None)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--down", type=int, default=0, help="notches towards the user")
    parser.add_argument("--up", type=int, default=0, help="notches away from the user")
    parser.add_argument("--raw", type=int, default=None, help="one raw delta, no Ctrl")
    parser.add_argument("--gap", type=int, default=60, help="ms between notches")
    parser.add_argument("--parent", action="store_true", help="post to the top-level window")
    parser.add_argument(
        "--egui",
        action="store_true",
        help="title-strip path: post to the top-level window with Ctrl held for real",
    )
    args = parser.parse_args()

    hwnd, title = find_note()
    if hwnd is None:
        print("no note window found", file=sys.stderr)
        return 1
    target = hwnd
    if not (args.parent or args.egui):
        # The island is the focused child; RichEdit is the only RICHEDIT50W child.
        island = find_child(hwnd, None, "RICHEDIT50W", None)
        if island:
            target = island
    print(f"note=0x{hwnd:X} {title!r} target=0x{target:X} path={'egui' if args.egui else 'island'}")

    notches = [(WHEEL_DELTA, n) for n in range(args.up)]
    notches += [(-WHEEL_DELTA, n) for n in range(args.down)]
    if args.raw is not None:
        notches = [(args.raw, 0)]

    for delta, index in notches:
        control = 0 if args.raw is not None else MK_CONTROL
        wparam = ((delta & 0xFFFF) << 16) | control
        if args.egui:
            # winit derives modifiers from `GetKeyState`, not from the message.
            hold_control(up=False)
            time.sleep(0.03)
            user32.PostMessageW(target, WM_MOUSEWHEEL, wparam, 0)
            time.sleep(0.03)
            hold_control(up=True)
        else:
            user32.PostMessageW(target, WM_MOUSEWHEEL, wparam, 0)
        print(f"  notch {index + 1}: delta={delta} control={bool(control)}")
        time.sleep(args.gap / 1000.0)
    return 0


if __name__ == "__main__":
    sys.exit(main())
