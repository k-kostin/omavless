"""Fixed read-only capture ownership: first uncertainty/nonzero is terminal."""
import os
import subprocess
import tempfile
import time


def settle(base, child, seconds):
    base.require(seconds in (5, 65) and not base.UNSETTLED, 'fixed_owned_scope')
    # A reused/pre-set Popen status is not a fresh WNOWAIT observation.
    if child.returncode is not None:
        base.quarantine(child, 'owned_preset_status')
    deadline = time.monotonic() + seconds
    try:
        while True:
            if time.monotonic() >= deadline:
                base.quarantine(child, 'owned_deadline_terminal')
            code = base.child_status(child)
            if code is None:
                time.sleep(0.02)
                continue
            if type(code) is not int or code != 0:
                base.quarantine(child, 'owned_nonzero_or_invalid_terminal')
            # Only the exact observed-zero child may be reaped once. No group
            # inventory, signal, cancellation cleanup, poll or wait fallback.
            pid, status = os.waitpid(child.pid, os.WNOHANG)
            if (type(pid) is not int or type(status) is not int or pid != child.pid
                    or not os.WIFEXITED(status) or os.WEXITSTATUS(status) != 0):
                base.quarantine(child, 'owned_exact_zero_reap_unknown')
            child.returncode = 0
            return
    except BaseException:
        if not any(item is child for item in base.UNSETTLED):
            base.quarantine(child, 'owned_observation_unknown_terminal')
        raise


def command(base, args, *, pass_fds, env):
    base.require(not base.UNSETTLED and len(pass_fds) == 2
                 and all(type(fd) is int and fd >= 0 for fd in pass_fds)
                 and args == [f'/proc/self/fd/{pass_fds[0]}', '--wide', '--dynamic',
                              '--program-headers', f'/proc/self/fd/{pass_fds[1]}']
                 and env == {'PATH':'/usr/bin','LANG':'C','LC_ALL':'C'}, 'fixed_readelf_scope')
    with tempfile.TemporaryFile(dir='/tmp') as output, tempfile.TemporaryFile(dir='/tmp') as errors:
        child = base.OwnedProcess(args, stdin=subprocess.DEVNULL, stdout=output, stderr=errors,
                                  start_new_session=True, pass_fds=pass_fds, env=env)
        settle(base, child, 5)
        output.seek(0)
        errors.seek(0)
        stdout, stderr = output.read(131073), errors.read(131073)
        base.require(len(stdout) <= 131072 and len(stderr) <= 131072, 'fixed_readelf_output_bound')
        return subprocess.CompletedProcess(args, 0, stdout, stderr)
