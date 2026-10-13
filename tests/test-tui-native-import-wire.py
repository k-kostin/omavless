#!/usr/bin/env python3
"""Hermetic real `omavless tui` -> fixed synthetic Unix peer -> owned PTY.

Build with --no-default-features --features tui ONLY, then freeze/hash the ELF.
This tests native client wiring, not a daemon, canonical import or live owner.
No provider/core/service/network/clipboard/store access. Raw input is invented.
"""
import argparse
import codecs
import fcntl
import hashlib
import json
import os
from pathlib import Path
import pty
import re
import select
import socket
import stat
import struct
import subprocess
import tempfile
import termios
import threading
import time


INSTANCE = "native-wire-fixture"
PROFILE = "trojan://synthetic-not-a-real-key@fixture.invalid:443?security=tls#Synthetic"
INPUTS = {"profile": PROFILE, "subscription": "https://fixture.invalid/feed",
          "bundle": PROFILE + "\n" + PROFILE.replace("fixture.invalid", "second.invalid")}
PRIVATE_MARKERS = ["trojan://", "https://", "synthetic-not-a-real-key", "fixture.invalid", "second.invalid"]
FINGERPRINT = "a" * 64
METHODS = ["system.hello", "capabilities.get", "ui.snapshot", "runtime.observation",
           "imports.classify", "plugin.action"]
LOCALES = json.loads((Path(__file__).resolve().parents[1] /
                     "crates/omavless-tui/locales.json").read_text())


def terminal_text(output, *, bordered=True):
    """Fixed 70x24 fixture projection, not a general terminal emulator.

    Ratatui paints word runs at absolute positions and reuses old cells. Raw
    substring checks do not establish a visible sentence. These fixtures use
    only one-cell EN/RU/box glyphs; CSI colors/modes do not change text cells.
    """
    cells = [[" "] * 70 for _ in range(24)]
    row = col = 0
    end = len(output)
    # A PTY read may split a CSI too. Retain the prefix for the next cumulative
    # projection rather than inventing cells or rejecting an incomplete command.
    last_escape = output.rfind("\x1b")
    if last_escape >= 0 and re.fullmatch(r"\x1b(?:\[[0-9;?]*)?", output[last_escape:]):
        require(end - last_escape <= 32, "partial CSI bound")
        end = last_escape
    for token in re.findall(r"\x1b\[[0-9;?]*[ -/]*[@-~]|.", output[:end], re.S):
        if token.startswith("\x1b["):
            params, command = token[2:-1], token[-1]
            if command in ("H", "f"):
                parts = [int(p or "1") for p in params.split(";")]
                target_col = parts[1] if len(parts) > 1 else 1
                require(len(parts) <= 2 and 1 <= parts[0] <= 24 and 1 <= target_col <= 70,
                        "fixture cursor outside fixed cells")
                row, col = parts[0] - 1, target_col - 1
            elif command == "J" and params in ("2", "3"):
                cells = [[" "] * 70 for _ in range(24)]
            elif command == "K":
                if params == "2":
                    cells[row] = [" "] * 70
                elif params in ("", "0"):
                    cells[row][col:] = [" "] * (70 - col)
                else:
                    require(False, "unsupported fixture erase command")
            elif command == "m":
                require(re.fullmatch(r"[0-9;]*", params) is not None, "fixture style command")
            elif command in ("h", "l"):
                require(params in ("?25", "?1049", "?2004"), "unsupported fixture terminal mode")
            elif command == "n":
                require(params == "6", "unsupported fixture terminal query")
            else:
                require(False, "unsupported fixture CSI command")
            continue
        if token == "\r":
            col = 0
        elif token == "\n":
            row = min(23, row + 1)
        elif token >= " " and token != "\x7f":
            cells[row][col] = token
            col = min(69, col + 1)
        else:
            require(False, "unsupported fixture output control")
    # Import frames have two gutter cells; the ordinary Profiles footer is
    # unbordered and must retain its edge columns. A main-screen assertion must
    # not accidentally discard the first letter of its notice.
    return " ".join("".join(line[1:-1] if bordered else line).strip() for line in cells)


def require(condition, category):
    # Never include request data or terminal output in a failing assertion.
    if not condition:
        raise AssertionError(category)


def exact_object(pairs):
    obj = {}
    for key, value in pairs:
        require(key not in obj, "duplicate wire key")
        obj[key] = value
    return obj


def private_parent():
    """Same checked HOME/.cache/ovt policy as tests/support/temp.rs."""
    home = Path.home()
    require(home.is_absolute(), "absolute test HOME")
    parent = home
    for component in (None, ".cache", "ovt"):
        if component is not None:
            parent = parent / component
            parent.mkdir(mode=0o700, exist_ok=True)
        metadata = parent.lstat()
        require(stat.S_ISDIR(metadata.st_mode) and metadata.st_uid == os.getuid()
                and stat.S_IMODE(metadata.st_mode) & 0o022 == 0, "unsafe private test parent")
        if component == "ovt":
            require(stat.S_IMODE(metadata.st_mode) == 0o700, "private test parent mode")
    return parent


class Peer:
    """One fixed same-UID listener, bounded frames, original child only."""
    def __init__(self, directory, kind, fault):
        directory.mkdir(mode=0o700)
        self.path = directory / "control.sock"
        require(len(os.fsencode(self.path)) < 108, "Unix fixture path bound")
        self.listener = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        self.listener.bind(str(self.path))
        self.path.chmod(0o600)
        self.listener.listen(4)
        self.listener.settimeout(0.1)
        require(stat.S_IMODE(directory.stat().st_mode) == 0o700, "directory mode")
        require(stat.S_IMODE(self.path.stat().st_mode) == 0o600, "socket mode")
        self.kind, self.fault = kind, fault
        self.pid = None
        self.ready = threading.Event()
        self.stop = threading.Event()
        self.lock = threading.Lock()
        self.calls = []  # Methods only: never retained private params.
        self.failure = None
        self.revision = 7
        self.armed = None
        self.fault_sent = False
        self.post_pending = False
        self.thread = threading.Thread(target=self.serve, name="native-wire-peer")
        self.thread.start()

    def methods(self):
        with self.lock:
            return list(self.calls)

    def result(self, method, params):
        require(method in METHODS, "unexpected native method")
        if method == "imports.classify":
            require(params == {"input": INPUTS[self.kind]}, "exact classify input")
            self.post_pending = True
            if self.kind == "profile":
                return {"version": 1, "kind": "profile", "profile": {
                    "version": 1, "protocol": "trojan", "suggestedName": "Synthetic import fixture",
                    "insecure": False, "experimental": False}}
            if self.kind == "subscription":
                return {"version": 1, "kind": "subscription",
                        "suggestedName": "Synthetic feed fixture", "duplicate": False}
            return {"version": 1, "kind": "bundle", "counts": {
                "accepted": 0 if self.fault == "no-new" else 2, "duplicates": 1,
                "skipped": 0}, "previewFingerprint": FINGERPRINT}
        if method == "plugin.action":
            if self.fault == "cancel-connect":
                require(set(params) == {"instanceId", "expectedRevision", "operationId",
                                       "action", "profileId", "mode"}, "positive Connect fields")
                require(params["instanceId"] == INSTANCE and params["expectedRevision"] == 7
                        and params["action"] == "connect" and params["mode"] == "rule"
                        and params["profileId"] == "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
                        "positive Connect target")
                require(isinstance(params["operationId"], str) and
                        1 <= len(params["operationId"]) <= 64, "positive Connect operation")
                self.revision = 8
                return {"schemaVersion": 1, "applied": True, "instanceId": INSTANCE,
                        "operationId": params["operationId"], "action": "connect"}
            fields = {"instanceId", "expectedRevision", "operationId", "action"}
            extra = {"input", "previewFingerprint"} if self.kind == "bundle" else (
                {"url", "name"} if self.kind == "subscription" else {"input", "name"})
            require(set(params) == fields | extra, "exact Save fields")
            require(params["instanceId"] == INSTANCE and params["expectedRevision"] == 7,
                    "Save owner/revision fencing")
            action = {"profile": "profile-import", "subscription": "subscription-add",
                      "bundle": "profile-bundle-import"}[self.kind]
            require(params["action"] == action, "fixed Save action")
            require(params.get("input", params.get("url")) == INPUTS[self.kind], "exact Save input")
            operation = params["operationId"]
            require(isinstance(operation, str) and 1 <= len(operation) <= 64 and
                    operation.isascii(), "bounded original operation")
            if self.kind == "bundle":
                require(params["previewFingerprint"] == FINGERPRINT, "Save preview binding")
            else:
                require(params["name"] == ("Synthetic feed fixture" if self.kind == "subscription"
                                           else "Synthetic import fixture"), "Save preview name")
            if self.fault == "unknown":
                return None  # Original peer closes without any response: no resend.
            self.revision = 8
            return {"schemaVersion": 1, "applied": True, "instanceId": INSTANCE,
                    "operationId": operation, "action": action}
        require(params == ({"versions": [1]} if method == "system.hello" else {}),
                "fixed read params")
        if method == "system.hello":
            owner, revision = INSTANCE, self.revision
            fault_due = self.armed == "before" or (self.armed == "after" and self.post_pending)
            if fault_due and not self.fault_sent:
                owner = "changed-fixture" if "owner" in self.fault else owner
                revision += int("revision" in self.fault)
                self.fault_sent = True
            self.post_pending = False
            return {"instanceId": owner, "version": 1, "runtimeOwnership": True}, revision
        if method == "capabilities.get":
            # Real ordinary action eligibility must be present in the Unknown
            # negative row, not vacuously absent. Any unexpected action reaches
            # the strict peer and fails rather than invoking a host operation.
            return {"runtimeOwnership": True, "methods": METHODS +
                    ["connection.connect", "routing.set_mode"]}
        desired = {"connected": False, "profileId": "", "mode": "rule", "generation": 5}
        common = {"schemaVersion": 1, "instanceId": INSTANCE, "transition": None,
                  "desired": desired, "lastKnownActual": "disconnected"}
        if method == "ui.snapshot":
            return {**common, "scope": "private_ui_metadata", "healthFresh": False,
                    "liveHealth": "unavailable", "profiles": [{"id": "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
                    "name": "Native wire fixture", "subscriptionId": None,
                    "missing": False, "favorite": False}], "subscriptions": []}
        return {**common, "scope": "local_runtime_observation", "availability": "observed",
                "manualRecoveryRequired": False, "facts": {"ownedCoreRunning": False,
                "visibleMihomoCount": 0, "ownedAuxiliaryMihomoCount": 0,
                "visibleTunCount": 0, "managedTunCount": 0,
                "ownedControllerConfigVerified": False, "desiredProfileMatchesOwned": False}}

    def serve(self):
        try:
            while not self.stop.is_set():
                try:
                    connection, _ = self.listener.accept()
                except socket.timeout:
                    continue
                with connection:
                    require(self.ready.wait(1), "original child identity unavailable")
                    pid, uid, _ = struct.unpack("3i", connection.getsockopt(
                        socket.SOL_SOCKET, socket.SO_PEERCRED, struct.calcsize("3i")))
                    require((pid, uid) == (self.pid, os.getuid()), "original same-UID peer")
                    connection.settimeout(1)
                    frame = bytearray()
                    while True:
                        part = connection.recv(4096)
                        if not part:
                            break  # Native client half-closes after its unary frame.
                        frame.extend(part)
                        require(len(frame) <= 65536, "request frame bound")
                    require(frame.endswith(b"\n") and frame.count(b"\n") == 1,
                            "single newline-terminated request")
                    request = json.loads(frame.decode("utf-8"), object_pairs_hook=exact_object)
                    require(set(request) == {"api", "version", "id", "method", "params"},
                            "exact request envelope")
                    require(request["api"] == "omavless.control" and request["version"] == 1
                            and request["id"] == f"cli-{self.pid}", "native request correlation")
                    method = request["method"]
                    with self.lock:
                        require(len(self.calls) < 80, "fixture request count bound")
                        result = self.result(method, request["params"])
                        self.calls.append(method)
                        revision = self.revision
                    if result is None:
                        continue
                    if isinstance(result, tuple):
                        result, revision = result
                    response = {"api": "omavless.control", "version": 1, "id": request["id"],
                                "ok": True, "revision": revision, "result": result}
                    encoded = json.dumps(response, separators=(",", ":")).encode() + b"\n"
                    require(len(encoded) < 32768, "synthetic response bound")
                    connection.sendall(encoded)
                    connection.shutdown(socket.SHUT_WR)
        except Exception as error:
            self.failure = str(error) if isinstance(error, AssertionError) else "synthetic peer failure"

    def close(self):
        self.stop.set()
        self.thread.join(timeout=2)
        require(not self.thread.is_alive(), "owned listener did not settle")
        self.listener.close()
        self.path.unlink()
        require(self.failure is None, self.failure or "synthetic peer failure")


def check(binary, locale, kind, fault):
    with tempfile.TemporaryDirectory(prefix="tnw-", dir=private_parent()) as name:
        home = Path(name)
        home.chmod(0o700)
        runtime = home / "run"
        runtime.mkdir(mode=0o700)
        peer = Peer(runtime / "omavless", kind, fault)
        master, slave = pty.openpty()
        original = termios.tcgetattr(slave)
        fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 24, 70, 0, 0))
        child = subprocess.Popen([str(binary), "tui"], stdin=slave, stdout=slave,
            stderr=slave, start_new_session=True, env={"HOME": str(home),
            "XDG_RUNTIME_DIR": str(runtime), "XDG_CONFIG_HOME": str(home / "config"),
            "XDG_STATE_HOME": str(home / "state"), "XDG_CACHE_HOME": str(home / "cache"),
            "TERM": "xterm-256color", "OMAVLESS_LOCALE": locale, "PATH": "/usr/bin:/bin"})
        peer.pid = child.pid
        peer.ready.set()
        output = bytearray()
        decoder = codecs.getincrementaldecoder("utf-8")("strict")
        terminal_stream = ""
        case_deadline = time.monotonic() + 20

        def drain():
            nonlocal terminal_stream
            require(time.monotonic() < case_deadline, "whole native case deadline")
            if select.select([master], [], [], 0.03)[0]:
                part = os.read(master, 65536)
                output.extend(part)
                terminal_stream += decoder.decode(part)
                require(len(output) <= 2 * 1024 * 1024, "terminal output bound")
                require(all(marker.encode() not in output for marker in PRIVATE_MARKERS),
                        "private draft rendered")
                visible = terminal_text(terminal_stream)
                require(all(marker not in visible for marker in PRIVATE_MARKERS),
                        "private draft visible across cursor runs")
            require(peer.failure is None, peer.failure or "synthetic peer failure")

        def wait(ready):
            deadline = min(case_deadline, time.monotonic() + 6)
            while not ready() and time.monotonic() < deadline:
                drain()
            require(ready(), "native terminal phase not reached")

        def text(key, *, bordered=True):
            phrase = LOCALES[key][locale]
            return " ".join(phrase.split()) in " ".join(terminal_text(terminal_stream, bordered=bordered).split())

        def settle_for(seconds):
            deadline = time.monotonic() + seconds
            while time.monotonic() < deadline:
                drain()

        try:
            wait(lambda: b"fixture" in output and b"F3" in output)
            require(peer.methods() == METHODS[:4], "initial fixed read sequence")
            os.write(master, b"\x1bOR")  # Traditional F3, not a synthetic KeyEvent.
            wait(lambda: b"Ctrl+U" in output)
            parts = INPUTS[kind].split("\n")
            os.write(master, b"\x1b[200~" + parts[0].encode() + b"\x1b[201~")
            if kind == "bundle":
                os.write(master, b"\x0e")  # Actual Ctrl+N adds exactly one newline.
                os.write(master, b"\x1b[200~" + parts[1].encode() + b"\x1b[201~")
            wait(lambda: "●".encode() in output)
            if fault.startswith("before-"):
                peer.armed = "before"
            elif fault.startswith("after-"):
                peer.armed = "after"
            boundary = len(peer.methods())
            os.write(master, b"\r")
            if fault.startswith(("before-", "after-")):
                wait(lambda: text("tui.import_rejected"))
                relevant = peer.methods()[boundary:]
                expected = ["system.hello"] if fault.startswith("before-") else [
                    "system.hello", "imports.classify", "system.hello"]
                require(relevant[:len(expected)] == expected and peer.fault_sent,
                        "changed context admission sequence")
                require(relevant.count("imports.classify") == int(fault.startswith("after-")),
                        "private classify refusal count")
                os.write(master, b"\r\r")
                settle_for(0.35)
                require("plugin.action" not in peer.methods(), "stale preview must not Save")
            else:
                key = "tui.import_no_new" if fault == "no-new" else "tui.import_confirm_keys"
                wait(lambda: text(key))
                relevant = peer.methods()[boundary:]
                require(relevant[:3] == ["system.hello", "imports.classify", "system.hello"],
                        "native classify pre/post hello sequence")
                require(relevant.count("imports.classify") == 1, "one original classify")
                if fault in ("cancel", "cancel-connect"):
                    os.write(master, b"\x1b")
                    settle_for(0.35)
                    require("plugin.action" not in peer.methods(), "Cancel must not Save")
                    if fault == "cancel-connect":
                        require(not text("tui.import_title"), "positive Cancel left modal")
                        os.write(master, b"\x1b[B")
                        settle_for(0.15)
                        os.write(master, b"c")
                        wait(lambda: text("tui.confirm"))
                        os.write(master, b"\r")
                        wait(lambda: peer.methods().count("plugin.action") == 1)
                        wait(lambda: peer.methods()[-4:] == METHODS[:4])
                elif fault == "no-new":
                    os.write(master, b"\r\r")
                    settle_for(0.35)
                    require("plugin.action" not in peer.methods(), "zero-new must not Save")
                else:
                    if fault.startswith("save-"):
                        # Classification completion triggers a new normal read.
                        # Earn that completed phase before arming the next Hello,
                        # so a polling Hello cannot consume this negative fixture.
                        wait(lambda: peer.methods()[-4:] == METHODS[:4])
                        boundary = len(peer.methods())
                        with peer.lock:
                            peer.armed = "before"
                    os.write(master, b"\r")
                    if fault.startswith("save-"):
                        wait(lambda: text("tui.import_rejected"))
                        require(peer.methods()[boundary] == "system.hello" and peer.fault_sent,
                                "Save fixed pre-hello refusal")
                        os.write(master, b"\r\r")
                        settle_for(0.35)
                        require("plugin.action" not in peer.methods(), "changed Save owner/revision admission")
                    else:
                        wait(lambda: peer.methods().count("plugin.action") == 1)
                        calls = peer.methods()
                        index = calls.index("plugin.action")
                        require(calls[index - 1] == "system.hello", "Save fixed pre-hello")
                        key = "tui.import_unknown" if fault == "unknown" else "tui.import_saved"
                        wait(lambda: text(key))
                        if fault == "unknown":
                            # Separate standalone Escape from following bytes:
                            # ESC+Enter in one write can decode as Alt+Enter.
                            # Earn the visibly closed modal before exercising
                            # global Connect/confirm/mode keys, then visibly
                            # reopen the retained Unknown context with F3.
                            os.write(master, b"\r")
                            settle_for(0.15)
                            require(text("tui.import_title"), "Unknown modal remains open")
                            os.write(master, b"\x1b")
                            # Clearing the modal can arrive in an earlier PTY
                            # chunk than painting the new footer. Earn both.
                            wait(lambda: not text("tui.import_title") and
                                 text("tui.import_unknown_footer", bordered=False))
                            require(text("tui.import_unknown_footer", bordered=False),
                                    "whole narrow Unknown footer with review/close keys")
                            # The complete short footer is observed above;
                            # ordinary keys cannot resurrect a new import.
                            for keys in (b"\x1b[B", b"c", b"\r", b"\r", b"1", b"2", b"3"):
                                os.write(master, keys)
                                settle_for(0.15)
                                require(not text("tui.import_title"), "global keys do not reopen import")
                            os.write(master, b"\x1bOR")
                            wait(lambda: text("tui.import_title") and text("tui.import_unknown"))
                            os.write(master, b"\r")
                            settle_for(0.15)
                            require(peer.methods().count("plugin.action") == 1,
                                    "unknown Save must not retry")
                            require(peer.methods().count("imports.classify") == 1,
                                    "unknown must retain original context")
            require(all(marker.encode() not in output for marker in PRIVATE_MARKERS),
                    "private draft rendered")
            os.write(master, b"\x03")
            require(child.wait(timeout=2) == 0, "original native client exit")
            require(termios.tcgetattr(slave) == original, "original terminal modes restored")
            require(not (home / "config").exists() and not (home / "state").exists(),
                    "native client created persistent configuration")
            calls = peer.methods()
            classifications = int(not fault.startswith("before-"))
            saves = int(fault in ("save", "unknown"))
            connect_control = int(fault == "cancel-connect")
            # Import has a guarded pre-Hello; the existing ordinary action
            # adapter dispatches its fenced plugin.action without that guard.
            guards = (2 if classifications else 1) + int(saves or fault.startswith("save-"))
            cycles = calls.count("capabilities.get")
            require(calls.count("imports.classify") == classifications and
                    calls.count("plugin.action") == saves + connect_control, "final exact classify/action counts")
            require(calls.count("ui.snapshot") == cycles == calls.count("runtime.observation") and
                    calls.count("system.hello") == cycles + guards,
                    "exact native read cycles and import Hello guards")
            return {"classify": calls.count("imports.classify"),
                    "Save": saves, "ConnectControl": connect_control,
                    "HelloGuards": guards, "readCycles": cycles}
        finally:
            if child.poll() is None:
                # Only this original known synthetic client, never daemon/core.
                child.kill()
                child.wait(timeout=2)
            os.close(master)
            os.close(slave)
            peer.close()


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=Path)
    parser.add_argument("--sha256", required=True)
    args = parser.parse_args()
    require("Enter: save" in terminal_text("\x1b[2J\x1b[2;2HEnter:\x1b[2;9Hsave"),
            "fixed cell projection spacing")
    require("Rejected" in terminal_text("\x1b[2;2HPreview\x1b[2;2HRejected"),
            "fixed cell projection replacement")
    require("first second" in terminal_text("\x1b[2;2Hfirst\x1b[2;70H│\x1b[3;1H│second"),
            "fixed bordered wrap projection")
    require("edge" in terminal_text("\x1b[2;1Hedge", bordered=False),
            "ordinary footer retains edge columns")
    split = codecs.getincrementaldecoder("utf-8")("strict")
    encoded = "Сохранено".encode()
    require(split.decode(encoded[:1]) == "" and split.decode(encoded[1:]) == "Сохранено",
            "split Cyrillic acquisition")
    try:
        terminal_text("\x1b[3D")
    except AssertionError:
        pass
    else:
        require(False, "unexpected CSI must not silently pass")
    binary = args.binary.resolve(strict=True)
    metadata = binary.stat()
    require(stat.S_ISREG(metadata.st_mode) and metadata.st_uid == os.getuid() and
            metadata.st_size <= 128 * 1024 * 1024 and stat.S_IMODE(metadata.st_mode) & 0o222 == 0,
            "bounded immutable owned ELF")
    executable = binary.read_bytes()
    require(executable.startswith(b"\x7fELF") and hashlib.sha256(executable).hexdigest() == args.sha256,
            "frozen native ELF identity")
    require(subprocess.run([str(binary), "tui", "--available"], capture_output=True,
            timeout=2, check=True).stdout == b"omavless.tui.v1\n", "normal TUI entry probe")
    cases = [(kind, outcome) for kind in ("bundle", "profile", "subscription")
             for outcome in ("cancel", "save")]
    cases += [("bundle", fault) for fault in ("before-owner", "before-revision",
              "after-owner", "after-revision", "save-owner", "save-revision", "unknown", "no-new", "cancel-connect")]
    for locale in ("en", "ru"):
        for kind, fault in cases:
            counts = check(binary, locale, kind, fault)
            print(f"PASS: native client wire / {locale} / {kind} / {fault} / {counts}")
    print("Hermetic native CLIENT ONLY: real tui entry, Unix framing/peer, PTY keys/masking/counts/teardown PASS")
