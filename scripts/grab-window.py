"""Screenshot a window (or the whole screen) with plain GDI, no third-party deps.

Why this exists: the sandbox blocks `Add-Type`, so the PowerShell one-liner that
would normally grab a frame cannot run, and "look at the window yourself" is not
an option when a 1 px caret needs judging. `ctypes` + `gdi32!BitBlt` needs no
compiled helper and writes a BMP that any viewer opens.

Usage:
    python grab-window.py                     # first window whose title contains easyflowhub-note
    python grab-window.py --title spike       # substring match
    python grab-window.py --screen            # whole virtual desktop
    python grab-window.py --out shot.bmp

BMP rather than PNG on purpose: encoding a PNG by hand is a lot of code for a
debugging aid, and the image viewers we care about read BMP.
"""

import argparse
import ctypes
import os
import struct
import sys
from ctypes import wintypes

user32 = ctypes.WinDLL("user32", use_last_error=True)
gdi32 = ctypes.WinDLL("gdi32", use_last_error=True)

SRCCOPY = 0x00CC0020
DIB_RGB_COLORS = 0
BI_RGB = 0

# Without this the coordinates below are virtualised at 150% and the grab is
# silently a quarter of the window.
try:
    user32.SetProcessDpiAwarenessContext(ctypes.c_void_p(-4))  # PER_MONITOR_AWARE_V2
except Exception:
    user32.SetProcessDPIAware()


class BITMAPINFOHEADER(ctypes.Structure):
    _fields_ = [
        ("biSize", wintypes.DWORD),
        ("biWidth", wintypes.LONG),
        ("biHeight", wintypes.LONG),
        ("biPlanes", wintypes.WORD),
        ("biBitCount", wintypes.WORD),
        ("biCompression", wintypes.DWORD),
        ("biSizeImage", wintypes.DWORD),
        ("biXPelsPerMeter", wintypes.LONG),
        ("biYPelsPerMeter", wintypes.LONG),
        ("biClrUsed", wintypes.DWORD),
        ("biClrImportant", wintypes.DWORD),
    ]


class BITMAPINFO(ctypes.Structure):
    _fields_ = [("bmiHeader", BITMAPINFOHEADER), ("bmiColors", wintypes.DWORD * 3)]


def find_window(needle):
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
        title = buffer.value
        if needle.lower() in title.lower():
            rect = wintypes.RECT()
            user32.GetWindowRect(hwnd, ctypes.byref(rect))
            found.append((hwnd, title, rect))
        return True

    user32.EnumWindows(callback, 0)
    return found[0] if found else None


def virtual_screen():
    """Whole desktop, including monitors with negative coordinates."""
    left = user32.GetSystemMetrics(76)   # SM_XVIRTUALSCREEN
    top = user32.GetSystemMetrics(77)    # SM_YVIRTUALSCREEN
    width = user32.GetSystemMetrics(78)  # SM_CXVIRTUALSCREEN
    height = user32.GetSystemMetrics(79)  # SM_CYVIRTUALSCREEN
    rect = wintypes.RECT(left, top, left + width, top + height)
    return rect


def grab(rect, out_path):
    width = rect.right - rect.left
    height = rect.bottom - rect.top
    if width <= 0 or height <= 0:
        raise SystemExit(f"empty rect {width}x{height}")

    screen_dc = user32.GetDC(None)
    memory_dc = gdi32.CreateCompatibleDC(screen_dc)
    bitmap = gdi32.CreateCompatibleBitmap(screen_dc, width, height)
    gdi32.SelectObject(memory_dc, bitmap)
    ok = gdi32.BitBlt(
        memory_dc, 0, 0, width, height, screen_dc, rect.left, rect.top, SRCCOPY
    )
    if not ok:
        raise SystemExit(f"BitBlt failed: {ctypes.get_last_error()}")

    info = BITMAPINFO()
    info.bmiHeader.biSize = ctypes.sizeof(BITMAPINFOHEADER)
    info.bmiHeader.biWidth = width
    info.bmiHeader.biHeight = -height  # top-down: no row flip when writing
    info.bmiHeader.biPlanes = 1
    info.bmiHeader.biBitCount = 32
    info.bmiHeader.biCompression = BI_RGB

    stride = width * 4
    pixels = ctypes.create_string_buffer(stride * height)
    lines = gdi32.GetDIBits(
        memory_dc, bitmap, 0, height, pixels, ctypes.byref(info), DIB_RGB_COLORS
    )
    if lines != height:
        raise SystemExit(f"GetDIBits returned {lines} of {height} lines")

    header = struct.pack("<2sIHHI", b"BM", 14 + 40 + len(pixels), 0, 0, 14 + 40)
    dib = struct.pack(
        "<IiiHHIIiiII",
        40, width, -height, 1, 32, BI_RGB, len(pixels), 2835, 2835, 0, 0
    )
    with open(out_path, "wb") as handle:
        handle.write(header)
        handle.write(dib)
        handle.write(pixels.raw)

    gdi32.DeleteObject(bitmap)
    gdi32.DeleteDC(memory_dc)
    user32.ReleaseDC(None, screen_dc)
    return width, height


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--title", default="easyflowhub-note")
    parser.add_argument("--screen", action="store_true")
    parser.add_argument("--out", default=None)
    args = parser.parse_args()

    if args.screen:
        rect = virtual_screen()
        out = args.out or "screen.bmp"
    else:
        match = find_window(args.title)
        if match is None:
            print(f"no visible window matching {args.title!r}", file=sys.stderr)
            return 1
        hwnd, title, rect = match
        out = args.out or "window.bmp"
        print(f"hwnd={hwnd} title={title!r} rect={rect.left},{rect.top} {rect.right-rect.left}x{rect.bottom-rect.top}")

    out = os.path.abspath(out)
    width, height = grab(rect, out)
    print(f"wrote {out} ({width}x{height})")
    return 0


if __name__ == "__main__":
    sys.exit(main())
