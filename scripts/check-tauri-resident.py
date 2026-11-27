"""T-Gate 1 gate: the build that says it creates nothing, watched creating nothing.

    T-Gate 0  the Tauri build stops creating the launcher / quick-note webviews
              and keeps one on-demand Manager instance
    §Phase 1  "0 webviews" is the number the whole migration exists to reach

`check-tauri-takeover.py` judges the *plan*: `--shell-report` answers which windows
a build would declare and which chords it would register, before `tauri::Builder`
is even constructed. This gate is the other half. It starts the real process and
watches Windows:

    resting    one process, zero WebView2 descendants, zero windows
    quiet      a second launch with `--autostart` shows nothing and exits
    on demand  a plain second launch creates exactly one Manager -- and a
               second one does not create a second
    closed     closing it returns to resting without taking the process with it

The process's own log cannot answer this: this build keeps its log in a bounded
in-memory buffer unless the user turned debugging on, so "0 webviews" has to come
from outside. A toolhelp snapshot gives the process tree (WebView2 runs as
children), `EnumWindows` gives the surfaces, and `GetProcessMemoryInfo` gives the
commit as a corroborating number rather than the claim itself.

Two things this gate deliberately does *not* do:

* it does not run `--shell-report` again; the structural gate owns that, and a
  second copy would drift.
* it does not touch `tauri.conf.json`; a gate that edits the thing it judges is
  measuring its own edit.

The single-instance mutex is keyed by the app identifier, so this gate needs the
session to itself. If any `easyflowhub.exe` is already running it refuses
(exit 2) instead of measuring the user's copy: their instance is not a fixture,
and closing *their* Manager would be this gate's side effect, not its evidence.

Side effect, stated up front: the "on demand" state opens the real Manager window
(and therefore a real webview) for about a second, then closes it. That window
reads the same database the installed build reads. Nothing is written except the
window-state plugin's usual record on close, and the process this gate started is
the only one it kills.

Usage:
    python scripts/check-tauri-resident.py --exe easyflowhub-app/src-tauri/target/release/easyflowhub.exe
"""

import argparse
import ctypes
import os
import subprocess
import sys
import time

try:
    user32 = ctypes.WinDLL("user32", use_last_error=True)
    kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)
    psapi = ctypes.WinDLL("psapi", use_last_error=True)
except (ImportError, OSError, AttributeError):  # not on Windows: Windows-only gate
    print("RESULT: SKIP (this gate drives Windows processes and windows)")
    sys.exit(0)

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "lib"))
import wininput  # noqa: E402

IMAGE = "easyflowhub.exe"
WEBVIEW = "msedgewebview2.exe"
MANAGER_TITLE = "EasyFlowHub"
LAUNCHER_CHORD = 0x20  # VK_SPACE: Ctrl+Alt+Space, the native shell's key
WM_CLOSE = 0x0010
TH32CS_SNAPPROCESS = 0x00000002


class PROCESSENTRY32(ctypes.Structure):
    _fields_ = [
        ("dwSize", ctypes.c_ulong),
        ("cntUsage", ctypes.c_ulong),
        ("th32ProcessID", ctypes.c_ulong),
        ("th32DefaultHeapID", ctypes.c_void_p),
        ("th32ModuleID", ctypes.c_ulong),
        ("cntThreads", ctypes.c_ulong),
        ("th32ParentProcessID", ctypes.c_ulong),
        ("pcPriClassBase", ctypes.c_long),
        ("dwFlags", ctypes.c_ulong),
        ("szExeFile", ctypes.c_char * 260),
    ]


class PROCESS_MEMORY_COUNTERS_EX(ctypes.Structure):
    _fields_ = [
        ("cb", ctypes.c_ulong),
        ("PageFaultCount", ctypes.c_ulong),
        ("PeakWorkingSetSize", ctypes.c_size_t),
        ("WorkingSetSize", ctypes.c_size_t),
        ("QuotaPeakPagedPoolUsage", ctypes.c_size_t),
        ("QuotaPagedPoolUsage", ctypes.c_size_t),
        ("QuotaPeakNonPagedPoolUsage", ctypes.c_size_t),
        ("QuotaNonPagedPoolUsage", ctypes.c_size_t),
        ("PagefileUsage", ctypes.c_size_t),
        ("PeakPagefileUsage", ctypes.c_size_t),
        ("PrivateUsage", ctypes.c_size_t),
    ]


kernel32.CreateToolhelp32Snapshot.restype = ctypes.c_void_p
kernel32.CreateToolhelp32Snapshot.argtypes = [ctypes.c_ulong, ctypes.c_ulong]
kernel32.Process32First.argtypes = [ctypes.c_void_p, ctypes.POINTER(PROCESSENTRY32)]
kernel32.Process32Next.argtypes = [ctypes.c_void_p, ctypes.POINTER(PROCESSENTRY32)]
kernel32.CloseHandle.argtypes = [ctypes.c_void_p]
kernel32.OpenProcess.restype = ctypes.c_void_p
kernel32.OpenProcess.argtypes = [ctypes.c_ulong, ctypes.c_int, ctypes.c_ulong]
psapi.GetProcessMemoryInfo.argtypes = [
    ctypes.c_void_p,
    ctypes.POINTER(PROCESS_MEMORY_COUNTERS_EX),
    ctypes.c_ulong,
]
user32.EnumWindows.argtypes = [ctypes.c_void_p, ctypes.c_size_t]
user32.GetWindowThreadProcessId.argtypes = [ctypes.c_void_p, ctypes.POINTER(ctypes.c_ulong)]
user32.IsWindowVisible.argtypes = [ctypes.c_void_p]
user32.GetWindowTextLengthW.argtypes = [ctypes.c_void_p]
user32.GetWindowTextW.argtypes = [ctypes.c_void_p, ctypes.c_wchar_p, ctypes.c_int]
user32.GetClassNameW.argtypes = [ctypes.c_void_p, ctypes.c_wchar_p, ctypes.c_int]
user32.PostMessageW.argtypes = [ctypes.c_void_p, ctypes.c_uint, ctypes.c_size_t, ctypes.c_size_t]


def snapshot() -> dict:
    """`{pid: (ppid, exe)}` from one toolhelp snapshot."""
    handle = kernel32.CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0)
    if not handle or handle == ctypes.c_void_p(-1).value:
        return {}
    table = {}
    entry = PROCESSENTRY32()
    entry.dwSize = ctypes.sizeof(entry)
    ok = kernel32.Process32First(handle, ctypes.byref(entry))
    while ok:
        name = entry.szExeFile.decode("mbcs", "replace").lower()
        table[int(entry.th32ProcessID)] = (int(entry.th32ParentProcessID), name)
        ok = kernel32.Process32Next(handle, ctypes.byref(entry))
    kernel32.CloseHandle(handle)
    return table


def descendants(table: dict, root: int) -> list:
    children = {}
    for pid, (parent, _) in table.items():
        children.setdefault(parent, []).append(pid)
    found, stack = [], [root]
    while stack:
        for child in children.get(stack.pop(), []):
            found.append(child)
            stack.append(child)
    return found


def webviews(pid: int) -> list:
    """Every WebView2 process under `pid`. Zero is the whole point of T-Gate 0."""
    table = snapshot()
    return [child for child in descendants(table, pid) if table[child][1] == WEBVIEW]


def image_pids(name: str = IMAGE) -> list:
    return [pid for pid, (_, exe) in snapshot().items() if exe == name.lower()]


def visible_windows(pid: int) -> list:
    """Visible top-level windows owned by `pid`: `(hwnd, title, class)`.

    The single-instance plugin keeps a hidden message window
    (`com.yukin.easyflowhub-siw`); it never becomes visible and must not be
    mistaken for "something opened".
    """
    found = []

    @ctypes.WINFUNCTYPE(ctypes.c_int, ctypes.c_void_p, ctypes.c_void_p)
    def collect(hwnd, _):
        owner = ctypes.c_ulong()
        user32.GetWindowThreadProcessId(hwnd, ctypes.byref(owner))
        if owner.value != pid or not user32.IsWindowVisible(hwnd):
            return 1
        length = user32.GetWindowTextLengthW(hwnd)
        title = ctypes.create_unicode_buffer(length + 1)
        user32.GetWindowTextW(hwnd, title, length + 1)
        cls = ctypes.create_unicode_buffer(256)
        user32.GetClassNameW(hwnd, cls, 256)
        found.append((hwnd, title.value, cls.value))
        return 1

    user32.EnumWindows(collect, 0)
    return found


def titled(pid: int) -> list:
    """Visible windows that have a title -- "opened" in the sense the user means.

    Minus the single-instance rendezvous: `single-instance` keeps a window named
    `{identifier}-siw` in class `{identifier}-sic` as the mailbox a second launch
    writes into. Windows reports it as visible, and it is not a surface --
    counting it would make "no window" a claim no correct build could satisfy.
    """
    return [
        window
        for window in visible_windows(pid)
        if window[1] and not window[2].endswith("-sic")
    ]


def manager_window(pid: int):
    for hwnd, title, _ in titled(pid):
        if title == MANAGER_TITLE:
            return hwnd
    return 0


def private_mb(pid: int):
    """Commit charge, printed as corroboration -- not asserted on."""
    handle = kernel32.OpenProcess(0x1000 | 0x0010, False, pid)  # LIMITED | VM_READ
    if not handle:
        return None
    try:
        counters = PROCESS_MEMORY_COUNTERS_EX()
        counters.cb = ctypes.sizeof(counters)
        if not psapi.GetProcessMemoryInfo(handle, ctypes.byref(counters), counters.cb):
            return None
        return counters.PrivateUsage / (1024.0 * 1024.0)
    finally:
        kernel32.CloseHandle(handle)


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


class Host:
    """The one process this gate starts, and the only one it may kill."""

    def __init__(self, exe: str) -> None:
        self.exe = exe
        self.process = None

    def start(self):
        self.process = subprocess.Popen(
            [self.exe], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL
        )
        return self.process

    def pid(self) -> int:
        return self.process.pid if self.process is not None else 0

    def alive(self) -> bool:
        return self.process is not None and self.process.poll() is None

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
        self.process = None


def second_launch(exe: str, autostart: bool = False, timeout: float = 20.0) -> tuple:
    """Run the exe again and time how long the forward takes.

    A second instance that does *not* exit is the bug the mutex exists for, so
    "it eventually finished" is not a pass: three seconds is an order of
    magnitude past the measured 0.1 s handshake. A timeout is therefore an
    answer (`code=None`), not a reason to crash -- and the process it leaves
    behind is killed here, because it became a second host and this gate started
    it.
    """
    argv = [exe] + (["--autostart"] if autostart else [])
    at = time.time()
    try:
        finished = subprocess.run(argv, capture_output=True, text=True, timeout=timeout)
    except subprocess.TimeoutExpired:
        return None, time.time() - at, "did not exit: this launch kept the session"
    return finished.returncode, time.time() - at, (finished.stderr or finished.stdout).strip()


def close(hwnd: int) -> None:
    user32.PostMessageW(hwnd, WM_CLOSE, 0, 0)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--exe",
        default=os.path.join("easyflowhub-app", "src-tauri", "target", "release", "easyflowhub.exe"),
        help="the Tauri build to observe (release build, not the installed copy)",
    )
    args = parser.parse_args()

    exe = os.path.abspath(args.exe)
    if not os.path.exists(exe):
        print(f"missing {exe}; build it with `cargo build --release` in easyflowhub-app/src-tauri")
        return 2

    print(f"tauri resident gate: exe={exe}")

    # --- the session must be ours -------------------------------------------
    others = image_pids()
    if others:
        print(f"cannot run: easyflowhub.exe already running (pid={others})")
        print("this gate measures a process it started; quit the running copy from its tray first")
        print("RESULT: FAIL")
        return 2

    report = Report()
    host = Host(exe)
    measuring = False
    try:
        host.start()
        # A forwarded launch exits immediately. Anything still alive after the
        # window-toolkit has had time to initialise is ours to measure.
        settled = wininput.wait_for(lambda: host.alive(), 3)
        if not settled:
            print(f"cannot run: the process exited at once (exit={host.process.returncode})")
            print("another EasyFlowHub owns the single-instance mutex, or the build crashed")
            print("RESULT: FAIL")
            return 2
        measuring = True
        time.sleep(2.5)

        pid = host.pid()
        commit = private_mb(pid)
        report.check(
            "the first launch is still resident after it settles",
            host.alive(),
            f"pid={pid} alive={host.alive()}"
            + (f" private={commit:.1f} MB" if commit is not None else ""),
        )
        report.check(
            "it opened no window at all",
            len(titled(pid)) == 0,
            f"visible titled windows={[(hwnd, title) for hwnd, title, _ in titled(pid)]}",
        )
        report.check(
            "and not one WebView2 process",
            len(webviews(pid)) == 0,
            f"webview descendants={webviews(pid)} (T-Gate 0's whole number)",
        )
        # Absence measured once could be "not yet". Measure it again after the
        # slowest thing a launch could still be doing.
        time.sleep(2.0)
        report.check(
            "still zero two seconds later, so it is absence and not latency",
            len(webviews(pid)) == 0 and len(titled(pid)) == 0 and host.alive(),
            f"webviews={len(webviews(pid))} windows={len(titled(pid))} alive={host.alive()}",
        )

        # --- a login-time second launch holds its tongue --------------------
        code, seconds, output = second_launch(exe, autostart=True)
        time.sleep(0.8)
        report.check(
            "a second launch with --autostart exits 0",
            code == 0,
            f"exit={code} in {seconds:.2f}s {output[:60]!r}",
        )
        report.check(
            "and it leaves in well under three seconds",
            seconds < 3.0,
            f"{seconds:.2f}s",
        )
        report.check(
            "it summoned nothing: the same process, still no window",
            host.alive()
            and sorted(image_pids()) == [pid]
            and len(titled(pid)) == 0
            and len(webviews(pid)) == 0,
            f"pids={image_pids()} windows={len(titled(pid))} webviews={len(webviews(pid))}",
        )

        # --- a plain second launch is a request for the Manager ------------
        code, seconds, output = second_launch(exe)
        report.check(
            "a plain second launch also exits 0",
            code == 0,
            f"exit={code} in {seconds:.2f}s {output[:60]!r}",
        )
        appeared = wininput.wait_for(lambda: bool(manager_window(pid)), 25)
        report.check(
            "and the Manager appears, on demand",
            appeared,
            f"hwnd={manager_window(pid)} windows={[(t, c) for _, t, c in titled(pid)]}",
        )
        # The window is created first and the WebView2 processes follow it, so
        # this waits for the children rather than sampling them once: measured
        # at one instant the same correct build reads 0 or 1 depending on load.
        grown = wininput.wait_for(lambda: len(webviews(pid)) > 0, 15)
        baked = webviews(pid)
        report.check(
            "which is a real webview, created for it",
            grown,
            f"webview descendants={len(baked)}",
        )
        report.check(
            "the Manager belongs to the process that was already running",
            host.alive() and sorted(image_pids()) == [pid],
            f"pids={image_pids()} host_alive={host.alive()}",
        )

        # "One on-demand instance" means asking again reuses it. The window is the
        # claim; the WebView2 count is printed but not asserted -- a webview comes
        # up as browser, gpu, renderer and crashpad over the first second or two,
        # so the count grows to six whether or not a second window exists.
        code, seconds, _ = second_launch(exe)
        time.sleep(1.5)
        managers = [title for _, title, _ in titled(pid) if title == MANAGER_TITLE]
        report.check(
            "asking again does not create a second Manager",
            code == 0 and len(managers) == 1,
            f"exit={code} managers={len(managers)} windows={len(titled(pid))}"
            f" webviews={len(webviews(pid))} (was {len(baked)}, not asserted)",
        )

        # --- closing it is a resting state, not an exit ---------------------
        close(manager_window(pid))
        gone = wininput.wait_for(
            lambda: len(titled(pid)) == 0 and len(webviews(pid)) == 0, 15
        )
        report.check(
            "closing the Manager releases its webview",
            gone,
            f"windows={len(titled(pid))} webviews={len(webviews(pid))}",
        )
        resting = host.alive() and sorted(image_pids()) == [pid]
        report.check(
            "and the tray process is still there to be asked again",
            resting,
            f"alive={host.alive()} pids={image_pids()} exit={host.process.returncode}",
        )

        # --- and it can be asked again --------------------------------------
        # Only meaningful while it is still there. Without this guard the next
        # launch becomes the host itself, waits out its timeout and is killed --
        # which would report "a launch timed out" instead of the exit that
        # caused it, and bury the finding under its own symptom.
        if resting:
            code, seconds, _ = second_launch(exe)
            again = wininput.wait_for(lambda: bool(manager_window(pid)), 25)
            report.check(
                "a later second launch brings it back",
                code == 0 and again,
                f"exit={code} hwnd={manager_window(pid)} in {seconds:.2f}s",
            )
            close(manager_window(pid))
            # Wait for both, as with the first close: WM_CLOSE tears the webview
            # down first and destroys the HWND a moment later, so waiting on the
            # webview alone would read the window's last breath as "it stayed".
            wininput.wait_for(
                lambda: len(webviews(pid)) == 0 and len(titled(pid)) == 0, 15
            )
            report.check(
                "and closing it again still does not end the process",
                host.alive() and len(webviews(pid)) == 0 and len(titled(pid)) == 0,
                f"alive={host.alive()} webviews={len(webviews(pid))} windows={len(titled(pid))}",
            )

            # --- the chord it gave away does not secretly still listen ------
            wininput.chord(LAUNCHER_CHORD, ctrl=True, alt=True)
            time.sleep(1.2)
            report.check(
                "the launcher chord creates nothing here (it belongs to the native shell)",
                host.alive() and len(webviews(pid)) == 0 and len(titled(pid)) == 0,
                f"webviews={len(webviews(pid))} windows={len(titled(pid))}",
            )
        else:
            for name in (
                "a later second launch brings it back",
                "and closing it again still does not end the process",
                "the launcher chord creates nothing here (it belongs to the native shell)",
            ):
                report.check(
                    name, False, "not reached: the process was gone (see the check above)"
                )
    finally:
        pid = host.pid()
        host.stop()
        if measuring:
            # WebView2 outlives its host for a moment; a leak here would leave
            # the next gate measuring somebody else's browser processes.
            drained = wininput.wait_for(lambda: not webviews(pid), 15)
            report.check(
                "no WebView2 process outlives the host",
                drained,
                f"pid={pid} remaining={webviews(pid)}",
            )

    return report.finish()


if __name__ == "__main__":
    sys.exit(main())
