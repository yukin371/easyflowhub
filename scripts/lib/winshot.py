"""Read a window's pixels through GDI, so a gate can judge what was drawn.

Why `PrintWindow` and not `BitBlt` from the screen DC: a screen grab only sees the
window while nothing covers it, and one of the things the pin gate has to prove is
that the window is *still there*. Measured 2026-11-26: after the pin was clicked
with the old code, `BitBlt` returned the desktop and `PrintWindow` returned black,
because winit had cleared `WS_VISIBLE` -- the window really was gone. A grab that
silently reads the desktop is worse than no grab.

`PW_RENDERFULLCONTENT` (2) is what makes it work for a composited (DWM/Mica)
surface; the plain call paints nothing for this window. The returned buffer is the
usual 32-bit bottom-up-negative DIB, so `rgb()` is the only place that knows the
byte order.

No third-party dependency on purpose: the gates run on a bare Windows box, and
`Add-Type` is blocked, so a compiled helper is not an option either.
"""

import ctypes
import sys
from ctypes import wintypes

try:
    user32 = ctypes.WinDLL("user32", use_last_error=True)
    gdi32 = ctypes.WinDLL("gdi32", use_last_error=True)
except (ImportError, OSError, AttributeError):  # not on Windows
    print("RESULT: SKIP (pixel checks need a Windows window)")
    sys.exit(0)

# Without this the coordinates are virtualised at 150% and every rectangle is a
# third of the size the window reports.
try:
    user32.SetProcessDpiAwarenessContext(ctypes.c_void_p(-4))  # PER_MONITOR_AWARE_V2
except Exception:  # pragma: no cover - older Windows
    user32.SetProcessDPIAware()

PW_RENDERFULLCONTENT = 2


class _BITMAPINFOHEADER(ctypes.Structure):
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


class _BITMAPINFO(ctypes.Structure):
    _fields_ = [("bmiHeader", _BITMAPINFOHEADER), ("bmiColors", wintypes.DWORD * 3)]


def client_size(hwnd):
    rect = wintypes.RECT()
    user32.GetClientRect(hwnd, ctypes.byref(rect))
    return rect.right - rect.left, rect.bottom - rect.top


def client_pixels(hwnd):
    """`(width, height, bytes)` for the client area, top row first."""
    width, height = client_size(hwnd)
    if width <= 0 or height <= 0:
        return 0, 0, b""

    source = user32.GetDC(hwnd)
    memory = gdi32.CreateCompatibleDC(source)
    bitmap = gdi32.CreateCompatibleBitmap(source, width, height)
    gdi32.SelectObject(memory, bitmap)
    ok = user32.PrintWindow(hwnd, memory, PW_RENDERFULLCONTENT)
    user32.ReleaseDC(hwnd, source)

    info = _BITMAPINFO()
    info.bmiHeader.biSize = ctypes.sizeof(_BITMAPINFOHEADER)
    info.bmiHeader.biWidth = width
    info.bmiHeader.biHeight = -height  # top-down
    info.bmiHeader.biPlanes = 1
    info.bmiHeader.biBitCount = 32
    info.bmiHeader.biCompression = 0  # BI_RGB

    buffer = ctypes.create_string_buffer(width * height * 4)
    lines = gdi32.GetDIBits(memory, bitmap, 0, height, buffer, ctypes.byref(info), 0)
    gdi32.DeleteObject(bitmap)
    gdi32.DeleteDC(memory)
    if not ok or lines != height:
        return width, height, b""
    return width, height, buffer.raw


def rgb(raw, width, x, y):
    """One pixel as `(r, g, b)`; DIBSection rows arrive as B, G, R, A."""
    offset = (y * width + x) * 4
    return raw[offset + 2], raw[offset + 1], raw[offset]


def ink_count(raw, width, x, y, half, threshold=110):
    """Pixels brighter than `threshold` per channel in a square around `(x, y)`.

    Used to tell a **filled** glyph from an outline: the same 16 px bookmark is
    solid in the middle when pinned and hollow when not, so a small box centred on
    the icon separates the two states without naming a colour. Measured at 150%:
    121 pixels filled vs 6 hollow in an 11 px box.
    """
    total = 0
    for row in range(y - half, y + half + 1):
        for column in range(x - half, x + half + 1):
            r, g, b = rgb(raw, width, column, row)
            if min(r, g, b) >= threshold:
                total += 1
    return total
