"""T-Gate 0 gate: the Tauri build stops creating the webviews it no longer owns.

    T-Gate 0  the Tauri build must stop creating the Launcher / quick-note
              webviews and keep one on-demand Manager instance
    §Phase 1  transition rules: every phase ends with panels moving out of
              Tauri, and "temporary" must not become permanent

What this gate is *not*: a screenshot test. The claims here are structural --
which windows the config declares, which chords the registration loop will
register, which tray entries exist -- so they can be judged without a GUI and
without stealing focus from the copy the user is running.

That is what `--shell-report` is for. It answers "what would this build do"
before `tauri::Builder` is even constructed: no single-instance mutex, no
WebView, no hotkey. The report is produced by the same functions the real
startup path uses (`shell::shortcut_plan`, `shell::tray_plan`,
`shell::declared_windows`), so "the report says three chords are gone" and "the
process registers three fewer chords" are the same statement rather than two
lists that happen to agree.

`--legacy-ui` is checked just as carefully as the default. A rollback nobody
tests is a rollback that quietly rots, and the transition rules only allow the
old shell to survive as a real, working fallback.

Usage:
    python scripts/check-tauri-takeover.py --exe dist/easyflowhub.exe
"""

import argparse
import json
import os
import subprocess
import sys

# The chords T-Gate 0 hands to the native shell. Listed literally: a gate that
# derives its expectation from the code under test would pass on any edit.
#
# `manager` is in the list because the first handover run left `Ctrl+Alt+M` on
# this side and the native shell answered `RegisterHotKey failed 1409` for it --
# the conflict is measured, not predicted.
HANDED_OVER = ["manager", "new_note", "close_all_notes", "toggle_notes"]
TAURI_CONF = os.path.join("src-tauri", "tauri.conf.json")


class Report:
    def __init__(self) -> None:
        self.results: list[tuple[str, bool, str]] = []

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


def report_for(exe: str, extra: list[str], env_override: dict[str, str] | None = None):
    """Run `--shell-report` and return (parsed json or None, raw stdout, stderr)."""
    env = dict(os.environ)
    env.pop("EFH_LEGACY_UI", None)
    if env_override:
        env.update(env_override)
    finished = subprocess.run(
        [exe, "--shell-report", *extra],
        capture_output=True,
        text=True,
        timeout=60,
        env=env,
    )
    raw = finished.stdout.strip()
    try:
        parsed = json.loads(raw)
    except json.JSONDecodeError:
        parsed = None
    return finished.returncode, parsed, raw, finished.stderr.strip()


def actions(report: dict) -> list[str]:
    return [entry.get("action", "") for entry in report.get("shortcuts", [])]


def chords(report: dict) -> list[str]:
    return [entry.get("chord", "") for entry in report.get("shortcuts", [])]


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--exe",
        default=os.path.join("easyflowhub-app", "src-tauri", "target", "release", "easyflowhub.exe"),
        help="the Tauri build to judge (release build, not the installed copy)",
    )
    parser.add_argument(
        "--conf",
        default=os.path.join("easyflowhub-app", TAURI_CONF),
        help="tauri.conf.json to read declared windows from, independently of the report",
    )
    args = parser.parse_args()

    exe = os.path.abspath(args.exe)
    if not os.path.exists(exe):
        print(f"missing {exe}; build it with `cargo build --release` in easyflowhub-app/src-tauri")
        return 2

    print(f"tauri takeover gate: exe={exe}")
    report = Report()

    code, taken, raw, stderr = report_for(exe, [])
    report.check(
        "the report is one JSON document on stdout",
        code == 0 and taken is not None,
        f"exit={code} stderr={stderr[:80]!r}" if taken is None else f"exit={code} bytes={len(raw)}",
    )
    if taken is None:
        return report.finish()

    report.check(
        "the default is handover, not the old shell",
        taken.get("takeover") is True and taken.get("source") == "default",
        f"takeover={taken.get('takeover')} source={taken.get('source')!r}",
    )
    report.check(
        "startup declares no launcher window",
        taken.get("declaredWindows") == [],
        f"declaredWindows={taken.get('declaredWindows')}",
    )
    report.check(
        "the launcher, quick note and todo card are all native",
        taken.get("launcher") == "native"
        and taken.get("quickNote") == "native"
        and taken.get("todoCard") == "native",
        f"launcher={taken.get('launcher')} quickNote={taken.get('quickNote')} todoCard={taken.get('todoCard')}",
    )
    report.check(
        "no global chord is registered at all",
        actions(taken) == [] and chords(taken) == [],
        f"shortcuts={taken.get('shortcuts')}",
    )
    for action in HANDED_OVER:
        report.check(
            f"{action} is not registered any more",
            action not in actions(taken),
            "absent" if action not in actions(taken) else f"still there: {actions(taken)}",
        )
    report.check(
        "the tray keeps the Manager and drops the quick-note entry",
        taken.get("tray") == ["show_manager", "quit"],
        f"tray={taken.get('tray')}",
    )

    # ---- independent read of the same config file -----------------------
    conf_path = os.path.abspath(args.conf)
    try:
        with open(conf_path, encoding="utf-8") as handle:
            conf = json.load(handle)
        declared = [window.get("label") for window in conf.get("app", {}).get("windows", [])]
        conf_error = ""
    except (OSError, json.JSONDecodeError) as error:
        declared = None
        conf_error = str(error)
    report.check(
        "tauri.conf.json itself declares no window that startup would build",
        declared == [],
        conf_error or f"windows={declared}",
    )

    # ---- the rollback has to work, or it is not a rollback --------------
    code_legacy, legacy, raw_legacy, stderr_legacy = report_for(exe, ["--legacy-ui"])
    report.check(
        "--legacy-ui brings the webview launcher back",
        code_legacy == 0
        and legacy is not None
        and legacy.get("takeover") is False
        and legacy.get("source") == "flag"
        and legacy.get("launcher") == "webview",
        f"exit={code_legacy} takeover={legacy and legacy.get('takeover')} "
        f"source={legacy and legacy.get('source')} launcher={legacy and legacy.get('launcher')}",
    )
    report.check(
        "--legacy-ui registers every chord the old shell owned",
        legacy is not None
        and sorted(actions(legacy)) == sorted(HANDED_OVER)
        and legacy.get("tray") == ["show_manager", "new_note", "quit"],
        f"shortcuts={legacy and actions(legacy)} tray={legacy and legacy.get('tray')}",
    )

    code_env, from_env, _, _ = report_for(exe, [], {"EFH_LEGACY_UI": "1"})
    report.check(
        "EFH_LEGACY_UI=1 does the same without changing argv",
        code_env == 0 and from_env is not None and from_env.get("takeover") is False
        and from_env.get("source") == "env",
        f"exit={code_env} takeover={from_env and from_env.get('takeover')} source={from_env and from_env.get('source')}",
    )
    code_off, off, _, _ = report_for(exe, [], {"EFH_LEGACY_UI": "0"})
    report.check(
        "EFH_LEGACY_UI=0 does not flip a shell script's 'off' into 'on'",
        code_off == 0 and off is not None and off.get("takeover") is True,
        f"exit={code_off} takeover={off and off.get('takeover')}",
    )
    report.check(
        "the two modes disagree on every surface, not just one",
        legacy is not None
        and len(taken.get("shortcuts", [])) + 4 == len(legacy.get("shortcuts", [])),
        f"handover={len(taken.get('shortcuts', []))} legacy={legacy and len(legacy.get('shortcuts', []))}",
    )

    return report.finish()


if __name__ == "__main__":
    sys.exit(main())
