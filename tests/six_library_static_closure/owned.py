"""Fixed read-only capture ownership: first uncertainty/nonzero is terminal."""
import os
import math
import subprocess
import tempfile
import time


def child_status(base, child):
    base.require(not base.UNSETTLED and type(child.pid) is int and child.pid > 0
                 and child.returncode is None, 'fixed_raw_wait_scope')
    try:
        status = os.waitid(os.P_PID, child.pid, os.WEXITED | os.WNOHANG | os.WNOWAIT)
        if status is None:
            return None
        base.require(type(status.si_pid) is int and status.si_pid == child.pid
                     and type(status.si_code) is int
                     and status.si_code in (os.CLD_EXITED, os.CLD_KILLED, os.CLD_DUMPED)
                     and type(status.si_status) is int and 0 <= status.si_status <= 255,
                     'fixed_raw_wait_shape')
        base.require(status.si_code == os.CLD_EXITED or 1 <= status.si_status <= 64,
                     'fixed_raw_signal_shape')
        return status.si_status if status.si_code == os.CLD_EXITED else -status.si_status
    except BaseException:
        base.quarantine(child, 'fixed_raw_wait_unknown_preserve')


def settle(base, child, seconds):
    base.require(seconds in (5, 140) and not base.UNSETTLED, 'fixed_owned_scope')
    # A reused/pre-set Popen status is not a fresh WNOWAIT observation.
    if child.returncode is not None:
        base.quarantine(child, 'owned_preset_status')
    deadline = time.monotonic() + seconds
    try:
        while True:
            if time.monotonic() >= deadline:
                base.quarantine(child, 'owned_deadline_terminal')
            code = child_status(base, child)
            if code is None:
                time.sleep(0.02)
                continue
            if type(code) is not int or code != 0:
                base.quarantine(child, 'owned_nonzero_or_invalid_terminal')
            if time.monotonic() >= deadline:
                base.quarantine(child, 'owned_late_zero_terminal')
            # Only the exact observed-zero child may be reaped once. No group
            # inventory, signal, cancellation cleanup, poll or wait fallback.
            pid, status = os.waitpid(child.pid, os.WNOHANG)
            if (type(pid) is not int or type(status) is not int or pid != child.pid or status != 0
                    or not os.WIFEXITED(status) or os.WEXITSTATUS(status) != 0):
                base.quarantine(child, 'owned_exact_zero_reap_unknown')
            if time.monotonic() >= deadline:
                base.quarantine(child, 'owned_late_reap_terminal')
            child.returncode = 0
            return
    except BaseException:
        if not any(item is child for item in base.UNSETTLED):
            base.quarantine(child, 'owned_observation_unknown_terminal')
        raise


def command(base, args, *, pass_fds, env, deadline):
    base.require(not base.UNSETTLED and len(pass_fds) == 2
                 and all(type(fd) is int and fd >= 0 for fd in pass_fds)
                 and args == [f'/proc/self/fd/{pass_fds[0]}', '--wide', '--dynamic',
                              '--program-headers', f'/proc/self/fd/{pass_fds[1]}']
                 and env == {'PATH':'/usr/bin','LANG':'C','LC_ALL':'C'}
                 and type(deadline) is float and math.isfinite(deadline), 'fixed_readelf_scope')
    # The fixed wrapper supplies only its fresh private scratch directory.
    # No historical global /tmp fallback is used by this new generation.
    base.require(os.environ.get('TMPDIR') == '/home/kdk_vm/.cache/t3-six-library-static-closure-review-1/scratch',
                 'fixed_readelf_scratch')
    base.require(time.monotonic() < deadline, 'fixed_readelf_source_deadline')
    with tempfile.TemporaryFile(dir=os.environ['TMPDIR']) as output, tempfile.TemporaryFile(dir=os.environ['TMPDIR']) as errors:
        # Scratch opens can block: recheck immediately before the first process effect.
        base.require(time.monotonic() < deadline, 'fixed_readelf_source_deadline')
        child = base.OwnedProcess(args, stdin=subprocess.DEVNULL, stdout=output, stderr=errors,
                                  start_new_session=True, pass_fds=pass_fds, env=env)
        settle(base, child, 5)
        output.seek(0)
        errors.seek(0)
        stdout, stderr = output.read(131073), errors.read(131073)
        base.require(len(stdout) <= 131072 and len(stderr) <= 131072, 'fixed_readelf_output_bound')
        return subprocess.CompletedProcess(args, 0, stdout, stderr)
