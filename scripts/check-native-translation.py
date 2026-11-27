"""Translation end to end: a real selection, a stub Relay, and the text written back.

    T-Gate 0  §5  the last item of the gate-0 list

What is actually under test
---------------------------
Not "does the model translate well" -- that is the provider's job and cannot be a
gate. What is under test is the chain this shell owns:

    a selection in another application
      -> a completion request at the right path, with the right body, and no credential
      -> a card the user can read
      -> the translation written back over the text it came from

Each link can fail in a way the others cannot see. A request to the wrong path
returns 404 and looks like "Relay is down". A body that splices the text into the
instruction translates the instruction. A replace that pastes into the panel
instead of the note leaves the note untouched while every log line says `ok`.

Why a stub and not the real Relay
---------------------------------
The Relay is a separate service with providers, keys and a network. A gate that
needs it can only run on a machine where all of that is configured, and it can only
assert what some particular model happened to answer. The stub answers in fixed
sentences, so the assertions are about *this* code: the request shape, the status
mapping, the card, the paste. `translation::status_problem` and `selection` are
covered against the real thing elsewhere.

The refusal scenario is the one that would otherwise go untested: a 502 whose body
mentions a key must never reach the user's screen, and only an end-to-end run can
prove that the sentence on screen came from this process rather than from upstream.

Usage:
    python scripts/check-native-translation.py --exe native/target/release/easyflowhub-native.exe
"""

import argparse
import json
import os
import shutil
import sys
import tempfile
import threading
import time

try:
    import ctypes

    user32 = ctypes.WinDLL("user32", use_last_error=True)
except (ImportError, OSError, AttributeError):  # not on Windows: Windows-only gate
    print("RESULT: SKIP (this gate drives Windows processes and windows)")
    sys.exit(0)

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "lib"))
import wininput  # noqa: E402

# The window plumbing is the selection gate's, imported rather than copied: both
# gates drive the same shell through the same windows, and a second copy would drift
# the first time either one learned something about focus. Nothing imported here is
# a shortcut past the code under test -- this gate's subject starts *after* the
# selection row appears.
import importlib.util  # noqa: E402


def _load_selection_gate():
    path = os.path.join(HERE, "check-native-selection.py")
    spec = importlib.util.spec_from_file_location("selection_gate", path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


sg = _load_selection_gate()

import http.server  # noqa: E402

VK_A = 0x41
VK_C = 0x43
VK_RETURN = 0x0D

MODEL = "gate-model"
TARGET = "zh-CN"
# What the stub answers, and therefore what must appear in the note afterwards.
# Chinese on purpose: the replace path has to survive the clipboard round trip with
# multibyte text, which is the case a `String`-to-`char` bug breaks.
TRANSLATION = "敏捷的棕色狐狸跃过懒狗。"
# The translation the panel is told to run, typed the way a user would search for it.
COMMAND_QUERY = "翻译"
# What the upstream body says, and what must never be shown.
SECRET = "sk-live-gate-4f2a"


# --- the stub Relay ---------------------------------------------------------


class Stub:
    """A Relay that answers in fixed sentences and remembers what it was asked."""

    def __init__(self) -> None:
        self.mode = "ok"
        self.requests = []
        self.server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), self._handler())
        self.server.daemon_threads = True
        self.port = self.server.server_address[1]
        threading.Thread(target=self.server.serve_forever, daemon=True).start()

    def url(self) -> str:
        return f"http://127.0.0.1:{self.port}"

    def answer(self):
        if self.mode == "denied":
            return 401, {"error": {"message": "invalid api key"}}
        if self.mode == "broken":
            # A provider body that carries a key. The panel must never show it.
            return 502, {"error": {"message": f"upstream provider exploded ({SECRET})"}}
        return 200, {
            "choices": [{"message": {"role": "assistant", "content": TRANSLATION}}]
        }

    def _handler(stub_self):
        stub = stub_self

        class Handler(http.server.BaseHTTPRequestHandler):
            protocol_version = "HTTP/1.1"

            def do_POST(self):  # noqa: N802 - the name is the framework's
                length = int(self.headers.get("Content-Length") or 0)
                raw = self.rfile.read(length) if length else b""
                stub.requests.append(
                    {
                        "path": self.path,
                        "headers": {k: v for k, v in self.headers.items()},
                        "body": raw.decode("utf-8", "replace"),
                    }
                )
                status, payload = stub.answer()
                data = json.dumps(payload, ensure_ascii=False).encode("utf-8")
                self.send_response(status)
                self.send_header("Content-Type", "application/json")
                self.send_header("Content-Length", str(len(data)))
                self.end_headers()
                self.wfile.write(data)

            def log_message(self, *args):
                # The gate owns stdout; one line per request would drown the report.
                pass

        return Handler

    def stop(self) -> None:
        self.server.shutdown()
        self.server.server_close()


class Shell(sg.Shell):
    """The selection gate's process, plus the translation settings block."""

    def __init__(self, exe: str, root: str, relay: str) -> None:
        super().__init__(exe, root)
        self.relay = relay

    def write_settings(self) -> None:
        with open(self.settings, "w", encoding="utf-8") as handle:
            json.dump(
                {
                    "shortcuts": {
                        "launcher": sg.LAUNCHER[0],
                        "new_note": sg.NEW_NOTE[0],
                        "close_all": sg.CLOSE_ALL[0],
                    },
                    # The block this gate exists to exercise. `relay_url` is the
                    # stub's port, which is why it is a setting and not a constant.
                    "translation": {
                        "model": MODEL,
                        "target_language": TARGET,
                        "relay_url": self.relay,
                    },
                },
                handle,
                ensure_ascii=False,
            )


# --- driving the panel ------------------------------------------------------


def arm(note: int, expected: str, leave):
    """Select everything in the note and prove a `Ctrl+C` would copy it.

    Same method as the selection gate, but against whatever the note holds *now*:
    after the first scenario the note contains the translation, and a check pinned
    to the original sentence would report "no selection" for a note that is
    perfectly selected.
    """
    trace = sg.take_keyboard(note)
    wininput.chord(VK_A, ctrl=True)
    time.sleep(0.1)
    sg.clipboard_set(None)
    wininput.chord(VK_C, ctrl=True)
    time.sleep(0.2)
    live = sg.clipboard_text() == expected
    sg.clipboard_set(leave)
    return live, f"{trace} live={live}"


def reveal_ready(shell: Shell, timeout: float = 8.0):
    """Reveal and wait until the query box owns the keyboard.

    Typing before that frame lands nowhere: the panel is on screen but egui has not
    given the text box the focus yet, and the keystrokes are dropped. The line is
    logged by the shell for exactly this reason.
    """
    before = shell.count("launcher_ready")
    line = sg.reveal(shell)
    ready = shell.later("launcher_ready", before, timeout)
    return line, ready


def run_translate(shell: Shell):
    """Search for the 翻译 card, press Enter, and return (start, done) log lines."""
    before = shell.count("launcher_translate")
    wininput.type_text(COMMAND_QUERY)
    time.sleep(0.35)
    wininput.tap(VK_RETURN)
    start = shell.later("launcher_translate", before, 8.0)
    if not start:
        return "", ""
    finish = shell.later("launcher_translate", before + 1, 25.0)
    return start, finish


def translation_layout(shell: Shell, after: str = "", timeout: float = 8.0):
    """The first translation-card layout line logged **after** `after`.

    Anchored to the finish line rather than to "the last layout line": the shell
    logs a layout only when it changes, so the *running* card and the *finished*
    card are two lines in order, and reading the newest without an anchor would let
    a check for "a failure offers no 替换选区" pass on the running card -- which also
    has no button -- without ever seeing the failure.
    """
    deadline = time.time() + timeout
    while time.time() < deadline:
        lines = sg.text(shell.log).splitlines()
        start = 0
        if after:
            for index, line in enumerate(lines):
                if line == after:
                    start = index + 1
                    break
            else:
                start = len(lines)  # the marker has not been written yet
        for line in lines[start:]:
            if "launcher_layout" not in line:
                continue
            slots = sg.parse_layout(line)
            if slots.get("translation") is not None:
                return slots
        time.sleep(0.05)
    return None


def click_layout(panel: int, slots, box) -> bool:
    """Click the centre of a layout slot, converting points to pixels."""
    canvas = slots["canvas"]
    logical_width = canvas[2] + 2 * canvas[0]
    if not logical_width:
        return False
    # The window has no decorations, so its rect width is the client width the
    # layout's logical points have to be scaled against. Derived rather than
    # assumed: the machine this runs on is at 125%.
    scale = sg.rect(panel)[2] / logical_width
    wininput.click_client(
        panel,
        (box[0] + box[2] / 2.0) * scale,
        (box[1] + box[3] / 2.0) * scale,
    )
    return True


# --- scenarios --------------------------------------------------------------


def scenario_success(report, shell: Shell, note: int, stub: Stub) -> None:
    """A translation that works: request, card, and the text written back."""
    live, trace = arm(note, sg.SAMPLE, "sentinel-before-translate")
    if not report.check("the note holds a live selection to translate", live, trace):
        return

    captured, ready = reveal_ready(shell)
    if not report.check("the panel takes the keyboard before anything is typed", bool(ready), ready or "no launcher_ready"):
        return
    report.check(
        "the reveal captured the selection, not the panel's own focus",
        sg.field(captured, "chars") == str(len(sg.SAMPLE))
        and sg.field(captured, "reason", "none") == "none",
        captured or "no launcher_selection line",
    )

    start, finish = run_translate(shell)
    report.check("Enter on 翻译 asks the Relay", sg.field(start, "mode") == "start", start or "no start line")
    report.check(
        "the request carries the captured selection",
        sg.field(start, "chars") == str(len(sg.SAMPLE)) and sg.field(start, "model") == f'"{MODEL}"',
        start,
    )
    report.check(
        "the finished card reports the translation",
        sg.field(finish, "mode") == "done" and sg.field(finish, "chars") == str(len(TRANSLATION)),
        finish or "no done line",
    )
    ms = sg.field(finish, "ms", "0")
    report.check(
        "the answer came back over the socket, not from a cache",
        ms.isdigit() and 0 <= int(ms) < 5000,
        f"ms={ms}",
    )

    report.check("the stub was asked exactly once", len(stub.requests) == 1, f"requests={len(stub.requests)}")
    if not stub.requests:
        return
    request = stub.requests[-1]
    report.check(
        "it posted to the completion path",
        request["path"] == "/v1/chat/completions",
        f"path={request['path']}",
    )
    report.check(
        "no credential was sent: Relay owns the keys",
        not any(name.lower() == "authorization" for name in request["headers"]),
        f"headers={sorted(request['headers'])}",
    )
    try:
        body = json.loads(request["body"])
    except ValueError as error:
        report.check("the body is JSON", False, str(error))
        return
    report.check(
        "the body is a non-streaming completion for the configured model",
        body.get("stream") is False and body.get("model") == MODEL,
        f"model={body.get('model')} stream={body.get('stream')}",
    )
    messages = body.get("messages") or []
    report.check(
        "the selection is its own message, not spliced into the instruction",
        len(messages) == 2 and messages[1].get("content") == sg.SAMPLE,
        f"messages={len(messages)} user={messages[1].get('content')!r}" if len(messages) > 1 else "no user message",
    )
    report.check(
        "the instruction names the configured target language",
        len(messages) > 0 and TARGET in (messages[0].get("content") or ""),
        (messages[0].get("content") or "")[:40] if messages else "no system message",
    )

    slots = translation_layout(shell, after=finish)
    if not report.check(
        "the frame draws a translation card",
        slots is not None,
        f"translation={slots.get('translation') if slots else '-'}",
    ):
        return
    report.check("the card offers 替换选区", slots.get("replace") is not None, f"replace={slots.get('replace')}")

    panel = sg.find(sg.PANEL_TITLE)
    if not report.check("the panel is still on screen to click", bool(panel), f"hwnd={panel}"):
        return
    before_replace = shell.count("launcher_translate_replace")
    click_layout(panel, slots, slots["replace"])
    line = shell.later("launcher_translate_replace", before_replace, 10.0)
    report.check("替换选区 reports the paste it performed", sg.field(line, "ok") == "true", line or "no replace line")
    wrote = wininput.wait_for(
        lambda: sg.note_text(note).replace("\r", "").strip() == TRANSLATION, 6
    )
    report.check(
        "the note now holds the translation, in place of the selection",
        bool(wrote),
        f"note={sg.note_text(note).replace(chr(13), '')!r}",
    )


def scenario_refused(report, shell: Shell, note: int, stub: Stub) -> None:
    """A 502 whose body carries a key: the panel shows our sentence, not theirs."""
    stub.mode = "broken"
    expected = sg.note_text(note).replace("\r", "").strip()
    live, trace = arm(note, expected, "sentinel-before-refusal")
    if not report.check("the note holds a live selection for the refusal", live, trace):
        return
    _, ready = reveal_ready(shell)
    if not report.check("the panel takes the keyboard again", bool(ready), ready or "no launcher_ready"):
        return

    before = len(stub.requests)
    start, finish = run_translate(shell)
    report.check("the refused run still started", sg.field(start, "mode") == "start", start or "no start line")
    report.check(
        "a 502 is reported as the provider being unavailable",
        sg.field(finish, "mode") == "failed" and sg.field(finish, "code") == "provider_unavailable",
        finish or "no finish line",
    )
    report.check("the request really reached the stub", len(stub.requests) == before + 1, f"requests={len(stub.requests)}")
    report.check(
        "the upstream body never reaches the log",
        SECRET not in sg.text(shell.log),
        "the key is in shell.log" if SECRET in sg.text(shell.log) else "clean",
    )

    slots = translation_layout(shell, after=finish)
    if not report.check(
        "a failure still draws a card",
        slots is not None,
        f"translation={slots.get('translation') if slots else '-'}",
    ):
        return
    report.check(
        "a failure offers no 替换选区",
        slots.get("replace") is None,
        f"replace={slots.get('replace')}",
    )
    report.check(
        "the note was not touched by a failed translation",
        sg.note_text(note).replace("\r", "").strip() == expected,
        f"note={sg.note_text(note).replace(chr(13), '')!r}",
    )
    stub.mode = "ok"


def scenario_nothing(report, shell: Shell, note: int, stub: Stub) -> None:
    """Nothing selected: a sentence, not a spinner and not a request."""
    sg.collapse_selection(note)
    _, ready = reveal_ready(shell)
    if not report.check("the panel opens with nothing selected", bool(ready), ready or "no launcher_ready"):
        return
    before = len(stub.requests)
    start, finish = run_translate(shell)
    # `finish` is empty here and that is the point: a preflight refusal logs one
    # line, not a start and then a failure, because no request was ever built.
    line = finish or start
    report.check(
        "an empty selection fails before a request is built",
        sg.field(line, "mode") == "failed"
        and sg.field(line, "code") == "empty_input"
        and sg.field(line, "stage") == "preflight",
        line or "no translate line",
    )
    report.check("nothing was sent to the Relay", len(stub.requests) == before, f"requests={len(stub.requests)}")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--exe", required=True)
    parser.add_argument("--keep", action="store_true", help="keep the temp dir")
    args = parser.parse_args()

    exe = os.path.abspath(args.exe)
    if not os.path.exists(exe):
        print(f"RESULT: FAIL (no exe at {exe})")
        return 1

    root = tempfile.mkdtemp(prefix="tg0-translation-gate-")
    report = sg.Report()
    stub = Stub()
    shell = Shell(exe, root, stub.url())
    print(f"# {root}\n# stub relay on {stub.url()}")
    try:
        shell.start()
        note, typed = sg.open_note_with(shell, sg.SAMPLE)
        if not report.check(
            "the note holds the text before the panel opens", typed, sg.note_probe(note)
        ):
            return report.finish()

        scenario_success(report, shell, note, stub)
        scenario_refused(report, shell, note, stub)
        scenario_nothing(report, shell, note, stub)
        return report.finish()
    finally:
        sg.close_panel(shell)
        shell.stop()
        stub.stop()
        if not args.keep and not os.environ.get("KEEP_GATE_DIR"):
            shutil.rmtree(root, ignore_errors=True)


if __name__ == "__main__":
    sys.exit(main())
