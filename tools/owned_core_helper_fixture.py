#!/usr/bin/python3
# SPDX-License-Identifier: MIT
"""TEST ONLY: a fake core whose helper retains an inherited flock after exit.

Materialize this file as an executable and invoke it with Mihomo's fixed
``-d PRIVATE_DIRECTORY -f CONFIG`` arguments. CONFIG is a <=1024-byte JSON
object with exactly these fields (there are no defaults):

    {"spawn": "startup", "helper": "ignore-term", "detach": false,
     "exitAfterReady": false}

``spawn`` is ``startup`` or ``term``. ``helper`` is ``ignore-term`` or
``exit-term``. ``detach`` calls setsid in the helper (an intentional escape
negative case). ``exitAfterReady`` exits the core without waiting for TERM;
it is allowed only for startup spawning.

Files below PRIVATE_DIRECTORY:
  keep-running   caller removes this to request unconditional fixture cleanup
  resource       inherited locked descriptor; independently try flock LOCK_EX
                 | LOCK_NB to observe retained/released resource ownership
  core-ready     core has installed its handler (and startup helper is ready)
  helper-ready   JSON with synthetic helper pid and pgid, published after setup

There is no controller, TUN, network, credential, store or system-service work.
All processes expire after ten seconds even if the caller crashes. The cleanup
marker is independent of TERM handling and group membership. Callers must use
a private fresh directory per case and remove keep-running in a finally/Drop
guard before deleting it. Check resource release before deleting resource:
unlinking a still-locked inode would hide a failed cleanup.

``python3 tools/owned_core_helper_fixture.py --self-test`` runs five fixture
conformance cases. These prove the synthetic failure, NOT production cleanup.
"""

import ctypes
import errno
import fcntl
import json
import os
from pathlib import Path
import signal
import stat
import subprocess
import sys
import tempfile
import time


LIFETIME_SECONDS = 10
POLL_SECONDS = 0.01


def publish(directory, name, payload=b""):
    """Publish a complete small marker; readers never observe partial JSON."""
    staged = directory / (name + ".staged")
    fd = os.open(staged, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
    try:
        os.write(fd, payload)
    finally:
        os.close(fd)
    os.rename(staged, directory / name)


def wait_until(predicate, timeout=2):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if predicate():
            return
        time.sleep(POLL_SECONDS)
    raise RuntimeError("fixture condition timed out")


def retained(directory):
    fd = os.open(directory / "resource", os.O_RDWR | os.O_NOFOLLOW)
    try:
        try:
            fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            return True
        return False
    finally:
        os.close(fd)


def load_config(directory, config):
    metadata = directory.lstat()
    if (not stat.S_ISDIR(metadata.st_mode) or metadata.st_uid != os.getuid()
            or stat.S_IMODE(metadata.st_mode) != 0o700):
        raise ValueError("fixture requires private directory")
    fd = os.open(config, os.O_RDONLY | os.O_NOFOLLOW)
    try:
        if not stat.S_ISREG(os.fstat(fd).st_mode):
            raise ValueError("fixture requires regular config")
        raw = os.read(fd, 1025)
    finally:
        os.close(fd)
    if len(raw) > 1024:
        raise ValueError("fixture config oversized")
    value = json.loads(raw)
    if (not isinstance(value, dict)
            or set(value) != {"spawn", "helper", "detach", "exitAfterReady"}
            or value["spawn"] not in ("startup", "term")
            or value["helper"] not in ("ignore-term", "exit-term")
            or type(value["detach"]) is not bool
            or type(value["exitAfterReady"]) is not bool
            or (value["exitAfterReady"] and value["spawn"] != "startup")):
        raise ValueError("fixture config invalid")
    return value


def fake_core(directory, config):
    options = load_config(directory, config)
    deadline = time.monotonic() + LIFETIME_SECONDS
    publish(directory, "keep-running")
    resource = os.open(directory / "resource",
                       os.O_RDWR | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
    fcntl.flock(resource, fcntl.LOCK_EX | fcntl.LOCK_NB)
    terminated = False

    def terminate(_signal, _frame):
        nonlocal terminated
        terminated = True

    signal.signal(signal.SIGTERM, terminate)

    def spawn_helper():
        pid = os.fork()
        if pid:
            wait_until(lambda: (directory / "helper-ready").exists())
            return
        try:
            signal.signal(signal.SIGTERM,
                          signal.SIG_IGN if options["helper"] == "ignore-term"
                          else signal.SIG_DFL)
            if options["detach"]:
                os.setsid()
            publish(directory, "helper-ready", json.dumps({
                "pid": os.getpid(), "pgid": os.getpgrp(),
            }, separators=(",", ":")).encode("ascii"))
            while (time.monotonic() < deadline
                   and (directory / "keep-running").exists()):
                time.sleep(POLL_SECONDS)
        finally:
            os.close(resource)
            os._exit(0)

    try:
        if options["spawn"] == "startup":
            spawn_helper()
        publish(directory, "core-ready")
        if options["exitAfterReady"]:
            return
        while (not terminated and time.monotonic() < deadline
               and (directory / "keep-running").exists()):
            time.sleep(POLL_SECONDS)
        if terminated and options["spawn"] == "term":
            spawn_helper()
    finally:
        os.close(resource)


def self_test():
    # A dedicated standalone test process adopts only its own orphaned fixture
    # helpers so they can be reaped. This never changes the Rust test runner.
    if ctypes.CDLL(None, use_errno=True).prctl(36, 1, 0, 0, 0) != 0:
        raise RuntimeError("fixture subreaper unavailable")
    cases = [
        ("startup", "ignore-term", False, False),
        ("startup", "exit-term", False, False),
        ("term", "ignore-term", False, False),
        ("term", "ignore-term", True, False),
        ("startup", "ignore-term", False, True),
    ]
    for spawn, helper, detach, early_exit in cases:
        with tempfile.TemporaryDirectory(prefix="omavless-helper-fixture-") as temporary:
            directory = Path(temporary)
            config = directory / "fixture.json"
            config.write_text(json.dumps({
                "spawn": spawn, "helper": helper, "detach": detach,
                "exitAfterReady": early_exit,
            }))
            config.chmod(0o600)
            child = subprocess.Popen([
                "/usr/bin/python3", str(Path(__file__).resolve()),
                "-d", str(directory), "-f", str(config),
            ], stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL, start_new_session=True)
            helper_pidfd = None
            try:
                wait_until(lambda: (directory / "core-ready").exists())
                assert retained(directory)
                if not early_exit:
                    # Popen owns an unreaped child, not an arbitrary remembered
                    # PID. No process group signalling is used in this probe.
                    child.terminate()
                assert child.wait(timeout=2) == 0
                wait_until(lambda: (directory / "helper-ready").exists())
                helper_info = json.loads((directory / "helper-ready").read_bytes())
                assert (helper_info["pgid"] != child.pid) == detach
                assert retained(directory), "parent exit must leave inherited lock"
                helper_pidfd = os.pidfd_open(helper_info["pid"])
                signal.pidfd_send_signal(helper_pidfd, signal.SIGTERM)
                if helper == "exit-term":
                    wait_until(lambda: not retained(directory))
                else:
                    time.sleep(0.03)
                    assert retained(directory), "helper must ignore TERM"
            finally:
                (directory / "keep-running").unlink(missing_ok=True)
                try:
                    child.wait(timeout=2)
                except subprocess.TimeoutExpired:
                    child.kill()
                    child.wait(timeout=2)
                if helper_pidfd is not None:
                    os.close(helper_pidfd)
                if (directory / "resource").exists():
                    wait_until(lambda: not retained(directory), LIFETIME_SECONDS + 1)
                # Adopted helpers can be briefly not-yet-exited after closing
                # the descriptor. Reap every child before deleting the fixture.
                def reaped():
                    try:
                        pid, _ = os.waitpid(-1, os.WNOHANG)
                        return False if pid == 0 else reaped()
                    except OSError as error:
                        if error.errno == errno.ECHILD:
                            return True
                        raise
                wait_until(reaped)
    print("helper fixture: 5 cases PASS (no TUN/network)")


def main():
    if sys.argv[1:] == ["--self-test"]:
        self_test()
    elif len(sys.argv) == 5 and sys.argv[1] == "-d" and sys.argv[3] == "-f":
        fake_core(Path(sys.argv[2]), Path(sys.argv[4]))
    else:
        raise ValueError("fixture arguments invalid")


if __name__ == "__main__":
    try:
        main()
    except Exception:
        # Synthetic input only; still do not teach tests to print raw payloads.
        print("helper fixture failed", file=sys.stderr)
        sys.exit(1)
