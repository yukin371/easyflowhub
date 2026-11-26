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

INPUT_KEYBOARD = 1
KEYEVENTF_KEYUP = 0x0002
KEYEVENTF_UNICODE = 0x0004


class KEYBDINPUT(ctypes.Structure):
    _fields_ = [
        ("wVk", wintypes.WORD),
        ("wScan", wintypes.WORD),
        ("dwFlags", wintypes.DWORD),
        ("time", wintypes.DWORD),
        ("dwExtraInfo", ctypes.POINTER(ctypes.c_ulong)),
    ]


class _INPUTUNION(ctypes.Union):
    # `MOUSEINPUT` is the largest member and it is 32 bytes on x64, which makes
    # `INPUT` 40. See the module header for what a wrong size does.
    _fields_ = [("ki", KEYBDINPUT), ("padding", ctypes.c_byte * 32)]


class INPUT(ctypes.Structure):
    _anonymous_ = ("u",)
    _fields_ = [("type", wintypes.DWORD), ("u", _INPUTUNION)]


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
