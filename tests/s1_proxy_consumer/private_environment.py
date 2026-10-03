#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""Opt-in installed default GIO consumer gate; child contexts and owned loopback only."""
import argparse
import contextlib
import hashlib
import json
import os
from pathlib import Path
import re
import secrets
import socket
import subprocess
import tempfile
import threading
import time

CORE_SHA256 = "aef9a6f24bde8101f59afbe06c67e159b36e3fcddb24ae6e88c667d4a8b987ff"
SCRIPT = Path(__file__).with_name("default_gio.js")
KEYS = tuple(key for lower in ("http_proxy", "https_proxy", "ftp_proxy", "all_proxy", "no_proxy")
             for key in (lower, lower.upper()))


def selected(values=None):
    result = dict.fromkeys(KEYS)
    if values:
        if set(values) - set(KEYS):
            raise ValueError("Unknown child environment key")
        result.update(values)
    return result


def child_environment(root, values):
    # Only these public desktop selectors are read. Parent proxy variables and
    # the manager environment are neither read, copied nor changed.
    desktop = {key: os.environ.get(key, "") for key in
               ("XDG_CURRENT_DESKTOP", "XDG_SESSION_DESKTOP", "DESKTOP_SESSION")}
    if desktop != {"XDG_CURRENT_DESKTOP": "Hyprland", "XDG_SESSION_DESKTOP": "Hyprland", "DESKTOP_SESSION": "omarchy"}:
        raise ValueError("Actual Hyprland context required")
    env = {"PATH": "/usr/bin:/bin", "LC_ALL": "C", "HOME": str(root), "TMPDIR": str(root),
           "XDG_CONFIG_HOME": str(root / "config"), "XDG_CACHE_HOME": str(root / "cache"),
           "XDG_DATA_HOME": str(root / "data"), "XDG_RUNTIME_DIR": str(root / "runtime"),
           "XDG_CONFIG_DIRS": str(root / "config"), "XDG_DATA_DIRS": "/usr/share",
           "DBUS_SESSION_BUS_ADDRESS": "unix:path=" + str(root / "absent-user-bus"),
           "DBUS_SYSTEM_BUS_ADDRESS": "unix:path=" + str(root / "absent-system-bus"), "GIO_USE_VFS": "local", **desktop}
    if set(values) != set(KEYS):
        raise ValueError("Complete child context required")
    for key, value in values.items():
        if value is not None:
            if not isinstance(value, str) or "\x00" in value or len(value.encode()) > 1024:
                raise ValueError("Invalid private child value")
            env[key] = value
    return env


def exact_pairs(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError("Duplicate fixture receipt")
        result[key] = value
    return result


def consumer(root, values, target, proxy, scheme, challenge, *, send=True, require_failure=None):
    payload = json.dumps({"target": target, "proxy": proxy, "scheme": scheme,
                          "environment": values, "challenge": challenge, "send": send}).encode()
    if len(payload) > 8192:
        raise ValueError("Oversized private child context")
    with tempfile.TemporaryFile(dir=root) as output:
        process = subprocess.Popen(["/usr/bin/gjs", str(SCRIPT)], stdin=subprocess.PIPE, stdout=output,
                                   stderr=subprocess.DEVNULL, env=child_environment(root, values))
        try:
            process.communicate(payload, timeout=8)
        except BaseException:
            # Retain/reap only this parent-owned child; no signal groups or PID reopen.
            process.kill()
            process.wait(timeout=3)
            raise
        if process.returncode or output.tell() > 4096:
            raise ValueError("Private GIO child failed")
        output.seek(0)
        report = json.loads(output.read(4097), object_pairs_hook=exact_pairs)
    if (isinstance(report, dict) and type(report.get("ok")) is bool and type(report.get("sent")) is bool
            and report == {"ok": True, "resolver": "GLibproxyResolver", "selection": scheme, "sent": send}):
        return True
    if isinstance(report, dict) and set(report) == {"ok", "phase"} and report["ok"] is False:
        if report["phase"] in {"input", "environment", "resolver", "connection", "dial", "http"}:
            if require_failure is not None and report["phase"] != require_failure:
                raise ValueError("Expected private consumer failure stage missing")
            return False
    raise ValueError("Ambiguous private GIO receipt")


def read_header(connection):
    result = b""
    while not result.endswith(b"\r\n\r\n"):
        data = connection.recv(1)
        if not data or len(result) >= 2048:
            raise ValueError("Bounded synthetic HTTP header required")
        result += data
    return result


class Target:
    def __init__(self):
        self.listener = socket.socket()
        self.listener.bind(("127.0.0.1", 0))
        self.listener.listen(4)
        self.listener.settimeout(0.1)
        self.port = self.listener.getsockname()[1]
        self.stopped = threading.Event()
        self.failed = threading.Event()
        self.receipts = []
        self.thread = threading.Thread(target=self.run, daemon=True)

    def __enter__(self):
        self.thread.start()
        return self

    def run(self):
        try:
            while not self.stopped.is_set():
                try:
                    connection, peer = self.listener.accept()
                except socket.timeout:
                    continue
                with connection:
                    connection.settimeout(1)
                    raw = read_header(connection)
                    match = re.fullmatch(rb"GET /([a-z0-9-]{1,80}) HTTP/1.0\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n", raw)
                    if peer[0] != "127.0.0.1" or not match:
                        raise ValueError("Unexpected synthetic HTTP target request")
                    challenge = match[1]
                    self.receipts.append(challenge.decode("ascii"))
                    connection.sendall(b"HTTP/1.0 200 OK\r\nContent-Length: " + str(len(challenge)).encode()
                                       + b"\r\nConnection: close\r\n\r\n" + challenge)
        except (OSError, ValueError):
            self.failed.set()

    def __exit__(self, *unused):
        self.stopped.set()
        self.thread.join(timeout=2)
        self.listener.close()
        if self.thread.is_alive() or self.failed.is_set():
            raise ValueError("Synthetic target cleanup or worker failed")


def http_receipt(connection, challenge):
    connection.sendall(f"GET /{challenge} HTTP/1.0\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n".encode())
    expected = f"HTTP/1.0 200 OK\r\nContent-Length: {len(challenge)}\r\nConnection: close\r\n\r\n{challenge}".encode()
    result = b""
    while True:
        data = connection.recv(4097)
        if not data:
            break
        result += data
        if len(result) > 4096:
            raise ValueError("Oversized synthetic response")
    if result != expected:
        raise ValueError("Synthetic target receipt missing")


def receive_exact(connection, size):
    result = b""
    while len(result) < size:
        data = connection.recv(size - len(result))
        if not data:
            raise ValueError("Synthetic handshake ended early")
        result += data
    return result


def readiness(proxy, target):
    try:
        with socket.create_connection(("127.0.0.1", proxy), timeout=0.3) as connection:
            connection.sendall(f"CONNECT 127.0.0.1:{target} HTTP/1.1\r\nHost: 127.0.0.1:{target}\r\n\r\n".encode())
            if not read_header(connection).startswith(b"HTTP/1.1 200 "):
                return False
            http_receipt(connection, "readiness-http")
        with socket.create_connection(("127.0.0.1", proxy), timeout=0.3) as connection:
            connection.sendall(b"\x05\x01\x00")
            if receive_exact(connection, 2) != b"\x05\x00":
                return False
            connection.sendall(b"\x05\x01\x00\x01\x7f\x00\x00\x01" + target.to_bytes(2, "big"))
            if receive_exact(connection, 4) != b"\x05\x00\x00\x01":
                return False
            receive_exact(connection, 6)  # Bound address; only target is fixed above.
            http_receipt(connection, "readiness-socks")
        return True
    except (OSError, ValueError):
        return False


def free_port():
    with socket.socket() as listener:
        listener.bind(("127.0.0.1", 0))
        return listener.getsockname()[1]


@contextlib.contextmanager
def owned_core(core, root):
    proxy = free_port()
    with tempfile.TemporaryDirectory(prefix="core-", dir=root) as name:
        config = Path(name) / "config.yaml"
        config.write_text(f"mixed-port: {proxy}\nexternal-controller: ''\nallow-lan: false\nbind-address: 127.0.0.1\n"
                          "mode: direct\nlog-level: silent\nipv6: false\nfind-process-mode: off\n"
                          "tun:\n  enable: false\ndns:\n  enable: false\nrules:\n  - MATCH,DIRECT\n")
        config.chmod(0o600)
        process = subprocess.Popen([str(core), "-d", name, "-f", str(config)], stdin=subprocess.DEVNULL,
                                   stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                                   env={"PATH": "/usr/bin:/bin", "HOME": name, "TMPDIR": name, "LC_ALL": "C"})
        try:
            deadline = time.monotonic() + 5
            while True:
                if process.poll() is not None or time.monotonic() >= deadline:
                    raise ValueError("Owned fixture core unavailable")
                try:
                    with socket.create_connection(("127.0.0.1", proxy), timeout=0.1):
                        break
                except OSError:
                    time.sleep(0.02)
            yield proxy
        finally:
            process.terminate()
            try:
                process.wait(timeout=3)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait(timeout=3)


def admit(root, values, target, proxy, scheme, challenge):
    if not readiness(proxy, target.port):
        return False
    before = len(target.receipts)
    if not consumer(root, values, target.port, proxy, scheme, challenge):
        return False
    if target.receipts[before:] != [challenge]:
        raise ValueError("Real HTTP application receipt required")
    return True


def exercise(core, scratch):
    if not core.is_absolute() or core.is_symlink() or not core.is_file():
        raise ValueError("Absolute regular fixture core required")
    if hashlib.sha256(core.read_bytes()).hexdigest() != CORE_SHA256:
        raise ValueError("Exact standalone core identity required")
    if not scratch.is_absolute() or scratch.is_symlink() or not scratch.is_dir():
        raise ValueError("Private existing scratch required")
    st = scratch.stat()
    if st.st_uid != os.getuid() or st.st_mode & 0o077:
        raise ValueError("Private scratch ownership required")
    with tempfile.TemporaryDirectory(prefix="s1-child-", dir=scratch) as name:
        root = Path(name)
        for directory in ("config", "cache", "data", "runtime"):
            (root / directory).mkdir(mode=0o700)
        child_environment(root, selected())  # Refuse wrong desktop before core/request.
        with Target() as target:
            with owned_core(core, root) as prior, owned_core(core, root) as intended:
                for baseline in (selected(), selected({"http_proxy": "", "HTTP_PROXY": ""}),
                                 selected({"http_proxy": f"http://127.0.0.1:{prior}", "no_proxy": ""})):
                    saved = dict(baseline)
                    scheme = "http" if baseline["http_proxy"] else "none"
                    challenge = "original-" + secrets.token_hex(8)
                    before = len(target.receipts)
                    if not consumer(root, baseline, target.port, prior, scheme, challenge) or target.receipts[before:] != [challenge]:
                        raise ValueError("Original child context consumption refused")
                    for protocol in ("http", "socks5"):
                        values = selected({"http_proxy": f"{protocol}://127.0.0.1:{intended}", "no_proxy": ""})
                        if not admit(root, values, target, intended, protocol, "application-" + secrets.token_hex(8)):
                            raise ValueError("Default GIO proxy consumer refused")
                    if baseline != saved:
                        raise ValueError("Original child context changed")
                    challenge = "restored-" + secrets.token_hex(8)
                    before = len(target.receipts)
                    if not consumer(root, saved, target.port, prior, scheme, challenge) or target.receipts[before:] != [challenge]:
                        raise ValueError("Exact child launch context restoration refused")
                lost = intended
            # Both owned cores have stopped; no direct fallback is acceptable.
            values = selected({"http_proxy": f"http://127.0.0.1:{lost}", "no_proxy": ""})
            before = len(target.receipts)
            if admit(root, values, target, lost, "http", "lost-readiness"):
                raise ValueError("Lost listener admitted")
            if consumer(root, values, target.port, lost, "http", "lost-application", require_failure="dial"):
                raise ValueError("Lost proxy fell back directly")
            with socket.socket() as bound:
                bound.bind(("127.0.0.1", 0))
                bound.listen(1)  # Bound/listening, but deliberately no protocol worker.
                port = bound.getsockname()[1]
                dummy = selected({"http_proxy": f"http://127.0.0.1:{port}", "no_proxy": ""})
                if admit(root, dummy, target, port, "http", "bound-only"):
                    raise ValueError("Bound-only listener admitted")
            missing = free_port()
            dummy = selected({"http_proxy": f"http://127.0.0.1:{missing}", "no_proxy": ""})
            if admit(root, dummy, target, missing, "http", "missing") or len(target.receipts) != before:
                raise ValueError("Refusal delivered unexpected application request")
    return CORE_SHA256


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--core", type=Path, required=True)
    parser.add_argument("--scratch-parent", type=Path, required=True)
    args = parser.parse_args()
    exercise(args.core, args.scratch_parent)
    print("s1_child_gio: passed; default GLibproxyResolver; HTTP/SOCKS real HTTP receipts; child-context restoration")
    print("readiness=HTTP_and_SOCKS_target_receipts; missing_lost_bound_only=refused; direct_fallback=refused")
    print("core_sha256=" + CORE_SHA256)
    print("scope=private_child_context; no manager/UWSM/settings effects; no existing-app claim")


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, TypeError, KeyError, subprocess.SubprocessError):
        raise SystemExit("s1_child_gio: refused; no host effects; no raw private output") from None
