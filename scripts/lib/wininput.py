"""Real keyboard and window input for the native gates.

The notes gate could get away with `WM_CHAR` posted at the RichEdit child: the text
path only needs the message. Anything that goes through the *frame* -- the switcher
panel, the workbench, a shortcut -- needs more than a message:

* `hub-editor` reads the modifier flags with `GetKeyState` at message time, and a
  posted message does not touch the key state, so `Ctrl+K` posted is just `k`.
* egui builds its key and text events from winit's input, and winit is told about a
  keystroke by the message queue, not by a window procedure someone calls directly.

So the gates that exercise a chord use `SendInput`, which is what a keyboard does.
That in turn needs the target window to be foreground, and `SetForegroundWindow` is
refused for a process that does not own the foreground -- hence [`force`], which
attaches to the foreground thread for the duration of the call.

Measured behaviour worth knowing before writing a gate against this:

* `INPUT` must be 40 bytes on x64 (4 for `type`, 4 padding, 32 for the union). A
  wrong size makes `SendInput` answer 0 with `ERROR_INVALID_PARAMETER` (87) and
  deliver nothing -- silently, if the return value is ignored.
* Chinese has no keyboard layout here, so text is typed with
  `KEYEVENTF_UNICODE`: the character travels in `wScan` and needs no layout.
* The first chord after launch is unreliable until `editor_focus` reaches the log.
  Wait for that line, do not sleep a fixed time.
* A click has to be real for the same reason a chord does: egui builds its pointer
  events from winit, so a posted `WM_LBUTTONDOWN` would be seen by the control and
  never by the frame -- and `WM_LBUTTONDOWN` *is* the message the island's subclass
  proc watches (Q7). Absolute coordinates are in 0..65535 across the desktop, not
  pixels.
"""

import ctypes
import time
from ctypes import wintypes

user32 = ctypes.WinDLL("user32", use_last_error=True)
# Per-monitor DPI: a virtualised rectangle puts the injected click or the measured
# window on a scaled rectangle, which is the wrong pixel.
user32.SetProcessDpiAwarenessContext(ctypes.c_void_p(-4))
user32.FindWindowW.restype = wintypes.HWND
user32.FindWindowW.argtypes = [wintypes.LPCWSTR, wintypes.LPCWSTR]
user32.FindWindowExW.restype = wintypes.HWND
user32.FindWindowExW.argtypes = [wintypes.HWND, wintypes.HWND, wintypes.LPCWSTR, wintypes.LPCWSTR]
user32.GetForegroundWindow.restype = wintypes.HWND
user32.GetWindowThreadProcessId.argtypes = [wintypes.HWND, ctypes.POINTER(wintypes.DWORD)]
user32.GetWindowThreadProcessId.restype = wintypes.DWORD
user32.SendMessageW.restype = wintypes.LPARAM
user32.SendMessageW.argtypes = [wintypes.HWND, wintypes.UINT, wintypes.WPARAM, wintypes.LPARAM]
user32.PostMessageW.restype = wintypes.BOOL
user32.PostMessageW.argtypes = [wintypes.HWND, wintypes.UINT, wintypes.WPARAM, wintypes.LPARAM]
user32.IsWindowVisible.argtypes = [wintypes.HWND]
user32.GetSystemMetrics.argtypes = [ctypes.c_int]
user32.GetClientRect.argtypes = [wintypes.HWND, ctypes.POINTER(wintypes.RECT)]
user32.ClientToScreen.argtypes = [wintypes.HWND, ctypes.POINTER(wintypes.POINT)]

WM_CHAR = 0x0102
WM_KEYDOWN = 0x0100
WM_KEYUP = 0x0101

VK_CONTROL = 0x11
VK_MENU = 0x12
VK_SHIFT = 0x10
VK_ESCAPE = 0x1B
VK_RETURN = 0x0D
VK_DOWN = 0x28
VK_UP = 0x26

INPUT_MOUSE = 0
INPUT_KEYBOARD = 1
KEYEVENTF_KEYUP = 0x0002
KEYEVENTF_UNICODE = 0x0004

MOUSEEVENTF_MOVE = 0x0001
MOUSEEVENTF_LEFTDOWN = 0x0002
MOUSEEVENTF_LEFTUP = 0x0004
MOUSEEVENTF_ABSOLUTE = 0x8000


class KEYBDINPUT(ctypes.Structure):
    _fields_ = [
        ("wVk", wintypes.WORD),
        ("wScan", wintypes.WORD),
        ("dwFlags", wintypes.DWORD),
        ("time", wintypes.DWORD),
        ("dwExtraInfo", ctypes.POINTER(ctypes.c_ulong)),
    ]


class MOUSEINPUT(ctypes.Structure):
    _fields_ = [
        ("dx", wintypes.LONG),
        ("dy", wintypes.LONG),
        ("mouseData", wintypes.DWORD),
        ("dwFlags", wintypes.DWORD),
        ("time", wintypes.DWORD),
        ("dwExtraInfo", ctypes.POINTER(ctypes.c_ulong)),
    ]


class _INPUTUNION(ctypes.Union):
    # `MOUSEINPUT` is the largest member and it is 32 bytes on x64, which makes
    # `INPUT` 40. See the module header for what a wrong size does. `padding`
    # stays as the statement of that size, and the assertion below checks it.
    _fields_ = [("ki", KEYBDINPUT), ("mi", MOUSEINPUT), ("padding", ctypes.c_byte * 32)]


class INPUT(ctypes.Structure):
    _anonymous_ = ("u",)
    _fields_ = [("type", wintypes.DWORD), ("u", _INPUTUNION)]

# The size is asserted rather than commented: a wrong one makes SendInput answer 0
# with `ERROR_INVALID_PARAMETER` and deliver nothing, which reads as "the click did
# nothing" in a gate.
if ctypes.sizeof(INPUT) != 40:  # pragma: no cover - wrong ctypes layout
    raise RuntimeError(f"INPUT is {ctypes.sizeof(INPUT)} bytes; SendInput needs 40 on x64")


def _send(vk, scan, flags):
    item = INPUT()
    item.type = INPUT_KEYBOARD
    item.ki.wVk = vk
    item.ki.wScan = scan
    item.ki.dwFlags = flags
    if user32.SendInput(1, ctypes.byref(item), ctypes.sizeof(INPUT)) != 1:
        raise RuntimeError(
            f"SendInput(vk=0x{vk:02X}, flags=0x{flags:X}) failed: {ctypes.get_last_error()}"
        )


def tap(vk):
    """One key, down and up."""
    _send(vk, 0, 0)
    _send(vk, 0, KEYEVENTF_KEYUP)


def chord(vk, ctrl=False, alt=False, shift=False):
    """Press `vk` with the modifiers held *for real*.

    The modifiers go down and up around the key rather than being passed as flags,
    because Windows derives the state `GetKeyState` reports from the queue.
    """
    mods = [mod for mod, on in ((VK_CONTROL, ctrl), (VK_MENU, alt), (VK_SHIFT, shift)) if on]
    for mod in mods:
        _send(mod, 0, 0)
        time.sleep(0.01)
    tap(vk)
    for mod in reversed(mods):
        _send(mod, 0, KEYEVENTF_KEYUP)
        time.sleep(0.01)


def type_text(text, gap=0.01):
    """Type `text` as Unicode keystrokes, no keyboard layout required."""
    for char in text:
        raw = char.encode("utf-16-le")
        for offset in range(0, len(raw), 2):
            unit = int.from_bytes(raw[offset : offset + 2], "little")
            _send(0, unit, KEYEVENTF_UNICODE)
            _send(0, unit, KEYEVENTF_UNICODE | KEYEVENTF_KEYUP)
        time.sleep(gap)


def _mouse(dx, dy, flags):
    item = INPUT()
    item.type = INPUT_MOUSE
    item.mi.dx = dx
    item.mi.dy = dy
    item.mi.dwFlags = flags
    if user32.SendInput(1, ctypes.byref(item), ctypes.sizeof(INPUT)) != 1:
        raise RuntimeError(f"SendInput(mouse flags=0x{flags:X}) failed: {ctypes.get_last_error()}")


def click(x, y):
    """A real left click at a **screen** point, in physical pixels.

    `MOUSEEVENTF_ABSOLUTE` coordinates are normalised to 0..65535 over the whole
    desktop, so the conversion is done here rather than at every call site. The
    move-then-press pair matters: some controls react to the move, and a press that
    arrives while the cursor is still somewhere else opens the wrong thing.
    """
    width = user32.GetSystemMetrics(0)
    height = user32.GetSystemMetrics(1)
    ax = int(round(x * 65535 / max(width - 1, 1)))
    ay = int(round(y * 65535 / max(height - 1, 1)))
    _mouse(ax, ay, MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE)
    time.sleep(0.02)
    _mouse(ax, ay, MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_LEFTDOWN)
    time.sleep(0.05)
    _mouse(ax, ay, MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_LEFTUP)
    time.sleep(0.12)


def client_box(hwnd):
    """`(left, top, width, height)` of a window's client area, in screen pixels."""
    rect = wintypes.RECT()
    origin = wintypes.POINT(0, 0)
    user32.GetClientRect(hwnd, ctypes.byref(rect))
    user32.ClientToScreen(hwnd, ctypes.byref(origin))
    return (origin.x, origin.y, rect.right - rect.left, rect.bottom - rect.top)


def click_client(hwnd, x, y):
    """Click at `(x, y)` inside `hwnd`'s client area, whatever its screen position."""
    left, top, width, height = client_box(hwnd)
    x = max(0, min(width - 1, int(x)))
    y = max(0, min(height - 1, int(y)))
    click(left + x, top + y)


def drag(x, y, dx, dy, steps=8, hold=0.02):
    """Press at a screen point, move by `(dx, dy)` in steps, release.

    The steps are not politeness. The shell moves the window by one delta per frame
    (`app/src/note/drag.rs`), so a single jump would be one frame carrying the whole
    travel: it would pass while a real drag lagged, stuttered, or dropped every
    intermediate position. A dozen small moves produce a dozen frames, which is what
    a hand does and what the window has to keep up with.
    """
    width = user32.GetSystemMetrics(0)
    height = user32.GetSystemMetrics(1)

    def at(px, py, flags):
        ax = int(round(px * 65535 / max(width - 1, 1)))
        ay = int(round(py * 65535 / max(height - 1, 1)))
        _mouse(ax, ay, flags | MOUSEEVENTF_ABSOLUTE)

    at(x, y, MOUSEEVENTF_MOVE)
    time.sleep(0.05)
    at(x, y, MOUSEEVENTF_LEFTDOWN)
    time.sleep(0.1)
    for step in range(1, steps + 1):
        at(x + dx * step / steps, y + dy * step / steps, MOUSEEVENTF_MOVE)
        time.sleep(hold)
    time.sleep(0.1)
    at(x + dx, y + dy, MOUSEEVENTF_LEFTUP)
    time.sleep(0.25)


def drag_client(hwnd, x, y, dx, dy, steps=8):
    """Drag starting at a client point; `(dx, dy)` is a screen-space delta."""
    left, top, width, height = client_box(hwnd)
    x = max(0, min(width - 1, int(x)))
    y = max(0, min(height - 1, int(y)))
    drag(left + x, top + y, dx, dy, steps)


def force(hwnd):
    """Make `hwnd` the foreground window and say whether it worked.

    `SetForegroundWindow` is refused unless the calling process owns the
    foreground; `AttachThreadInput` borrows the right for the length of the call,
    which is the documented way a test tool takes the keyboard.
    """
    foreground = user32.GetForegroundWindow()
    if foreground == hwnd:
        return True
    target_thread = user32.GetWindowThreadProcessId(hwnd, None)
    holder = user32.GetWindowThreadProcessId(foreground, None) if foreground else 0
    attached = False
    if holder and target_thread and holder != target_thread:
        attached = bool(user32.AttachThreadInput(holder, target_thread, True))
    user32.SetForegroundWindow(hwnd)
    if attached:
        user32.AttachThreadInput(holder, target_thread, False)
    time.sleep(0.2)
    return user32.GetForegroundWindow() == hwnd


def type_into_island(hwnd, text):
    """Type into a RichEdit island with `WM_CHAR`, the path the notes gate uses.

    A line break is `keydown + char + keyup`: RichEdit ignores a bare `WM_CHAR 0x0D`
    and the whole note arrives as one paragraph.
    """
    raw = text.encode("utf-16-le")
    sent = 0
    for offset in range(0, len(raw), 2):
        unit = int.from_bytes(raw[offset : offset + 2], "little")
        if unit == 0x0A:
            user32.SendMessageW(hwnd, WM_KEYDOWN, VK_RETURN, 0x001C0001)
            user32.SendMessageW(hwnd, WM_CHAR, 0x0D, 1)
            user32.SendMessageW(hwnd, WM_KEYUP, VK_RETURN, 0xC01C0001)
        else:
            user32.SendMessageW(hwnd, WM_CHAR, unit, 1)
        sent += 1
    return sent


def wait_for(predicate, timeout, step=0.05):
    deadline = time.time() + timeout
    while time.time() < deadline:
        value = predicate()
        if value:
            return value
        time.sleep(step)
    return predicate()
