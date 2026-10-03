#!/usr/bin/env python3
"""Private rekey-only owned commands; no CLI, production caller or networking.

Derived from sealed #585 dcec0ed supervisor. Command/private-directory/save
bodies retain their fixed-fixture behavior, except first main-loop wait-state
uncertainty is irrevocably latched before any further ownership operation.
"""
import importlib.util
import os
from pathlib import Path
import selectors
import signal
import stat
import subprocess
import time

HERE = Path(__file__).resolve().parent
HELPER_BYTES = (HERE / "run_cookie_overlay.py").read_bytes()
spec = importlib.util.spec_from_file_location("p4_rekey_export", HERE / "run_cookie_overlay.py")
helpers = importlib.util.module_from_spec(spec)
exec(compile(HELPER_BYTES, str(HERE / "run_cookie_overlay.py"), "exec"), helpers.__dict__)
UNSETTLED = []


class Unsettled(RuntimeError):
    """Preserve scratch and the unreaped owned anchor; never infer cleanup."""


def members(leader):
    found = []
    for entry in Path("/proc").iterdir():
        if entry.name.isdecimal() and int(entry.name) != leader:
            try:
                fields = (entry / "stat").read_text().rsplit(")", 1)[1].split()
                if int(fields[2]) == leader:
                    found.append(int(entry.name))
            except FileNotFoundError:
                pass
    return found


def reap(p, timeout, known_exited=False):
    """Exact raw child status only; never Popen's ECHILD-to-zero fallback."""
    deadline = time.monotonic() + timeout
    try:
        while True:
            pid, status = os.waitpid(p.pid, os.WNOHANG)
            if pid == p.pid:
                code = os.waitstatus_to_exitcode(status)
                p.returncode = code
                return code
            if pid != 0 or known_exited or time.monotonic() >= deadline:
                raise ValueError("exact_owned_reap_missing")
            time.sleep(0.01)
    except BaseException as exc:
        UNSETTLED.append(p)
        raise Unsettled("owned_final_reap_unknown_preserve") from exc


def command(args, cwd, env, timeout):
    """Fixed trusted children; retain WNOWAIT anchor until group and EOF settle."""
    if UNSETTLED:
        raise Unsettled("prior_owned_group_unsettled")
    p = subprocess.Popen(args, cwd=cwd, env=env, stdin=subprocess.DEVNULL,
                         stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                         start_new_session=True, bufsize=0)
    selector = None
    output = {"out": bytearray(), "err": bytearray()}
    deadline = time.monotonic() + timeout
    exited_at = None
    quiescent = False
    try:
        # Setup is part of owned-child cancellation too.
        selector = selectors.DefaultSelector()
        for stream, label in ((p.stdout, "out"), (p.stderr, "err")):
            os.set_blocking(stream.fileno(), False)
            selector.register(stream, selectors.EVENT_READ, label)
        while True:
            if time.monotonic() >= deadline:
                raise ValueError("fixed_command_timeout")
            for event, _ in selector.select(0.02):
                block = os.read(event.fileobj.fileno(), 65536)
                if not block:
                    selector.unregister(event.fileobj)
                else:
                    output[event.data].extend(block)
                    if sum(map(len, output.values())) > 2 * 1024 * 1024:
                        raise ValueError("fixed_command_output_bound")
            try:
                status = os.waitid(os.P_PID, p.pid, os.WEXITED | os.WNOHANG | os.WNOWAIT)
            except BaseException as exc:
                UNSETTLED.append(p)
                raise Unsettled("main_wait_anchor_unknown_preserve") from exc
            if status is not None:
                exited_at = exited_at or time.monotonic()
                remaining = members(p.pid)
                if not remaining and not selector.get_map():
                    quiescent = True
                    code = reap(p, 1, known_exited=True)
                    return code, bytes(output["out"]), bytes(output["err"])
                if time.monotonic() - exited_at >= 2:
                    raise ValueError("fixed_command_descendant_survives")
    except BaseException:
        # Never retry a wait/query, signal or reap after the first unknown
        # ownership observation. A later apparent success cannot restore it.
        if any(anchor is p for anchor in UNSETTLED):
            raise
        try:
            os.waitid(os.P_PID, p.pid, os.WEXITED | os.WNOHANG | os.WNOWAIT)
        except ChildProcessError:
            if quiescent:
                raise
            UNSETTLED.append(p)
            raise Unsettled("owned_anchor_lost_preserve")
        except BaseException as exc:
            UNSETTLED.append(p)
            raise Unsettled("owned_anchor_state_unknown") from exc
        try:
            try:
                os.killpg(p.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            until = time.monotonic() + 5
            while members(p.pid) and time.monotonic() < until:
                time.sleep(0.02)
            if members(p.pid):
                raise Unsettled("cancelled_group_not_quiescent")
            reap(p, 2)
        except BaseException as exc:
            if not any(anchor is p for anchor in UNSETTLED):
                UNSETTLED.append(p)
            raise Unsettled("owned_group_preserved") from exc
        raise
    finally:
        if selector is not None:
            selector.close()
        p.stdout.close()
        p.stderr.close()


def private_directory(path, home):
    resolved = path.resolve(strict=True)
    if not resolved.is_relative_to(home) or resolved == home:
        raise ValueError("owned_home_subdirectory_required")
    for ancestor in (resolved, *resolved.parents):
        if ancestor == home:
            break
        m = ancestor.lstat()
        if not stat.S_ISDIR(m.st_mode) or m.st_uid != os.getuid() or m.st_mode & 0o7022 or (ancestor == resolved and stat.S_IMODE(m.st_mode) != 0o700):
            raise ValueError("private_owned_ancestors_required")
    return resolved


def save(directory, name, data, mode=0o600):
    fd = os.open(directory / name, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, mode)
    try:
        os.fchmod(fd, mode)
        with os.fdopen(fd, "wb", closefd=False) as stream:
            stream.write(data)
            stream.flush()
            os.fsync(fd)
    finally:
        os.close(fd)
