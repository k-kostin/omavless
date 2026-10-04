"""Harmless syscall counterexample; not a diagnosis of the historic Rust failure."""
import errno
import fcntl
import os
from pathlib import Path
import select
import tempfile
import time
import unittest
from types import SimpleNamespace
from unittest.mock import Mock, patch


def known_exit(pid, seconds=5):
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        seen = os.waitid(os.P_PID, pid, os.WEXITED | os.WNOHANG | os.WNOWAIT)
        if seen is None:
            time.sleep(0.001)
            continue
        if seen.si_pid != pid or seen.si_code not in (os.CLD_EXITED, os.CLD_KILLED, os.CLD_DUMPED):
            raise RuntimeError('unknown child: preserve without signal/reap/retry')
        actual, status = os.waitpid(pid, os.WNOHANG)
        if actual != pid or not (os.WIFEXITED(status) or os.WIFSIGNALED(status)):
            raise RuntimeError('unknown reap: preserve')
        expected = seen.si_status if seen.si_code == os.CLD_EXITED else -seen.si_status
        if (os.waitstatus_to_exitcode(status) != expected
                or bool(os.WCOREDUMP(status)) != (seen.si_code == os.CLD_DUMPED)
                or expected != 0):
            raise RuntimeError('non-success: preserve')
        return
    raise RuntimeError('unknown timeout: preserve without signal/reap/retry')


class ForkLock(unittest.TestCase):
    def test_unknown_wait_never_reaps_or_retries(self):
        for result in (ChildProcessError(), InterruptedError(),
                       SimpleNamespace(si_pid=8, si_code=os.CLD_EXITED, si_status=0),
                       SimpleNamespace(si_pid=7, si_code=os.CLD_STOPPED, si_status=0)):
            wait = Mock(side_effect=result) if isinstance(result, BaseException) else Mock(return_value=result)
            with patch.object(os, 'waitid', wait), patch.object(os, 'waitpid') as reap:
                with self.assertRaises((RuntimeError, OSError)):
                    known_exit(7)
                wait.assert_called_once()
                reap.assert_not_called()

    def test_cloexec_does_not_release_inherited_flock_until_exec(self):
        root = Path(tempfile.mkdtemp(prefix='ov-journal-fork-', dir=os.environ['HOME']))
        root.chmod(0o700)
        held = os.open(root, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC)
        original = os.fstat(held)
        self.assertFalse(os.get_inheritable(held))
        fcntl.flock(held, fcntl.LOCK_EX | fcntl.LOCK_NB)
        ready_read, ready_write = os.pipe2(os.O_CLOEXEC)
        finish_read, finish_write = os.pipe2(os.O_CLOEXEC)
        pid = os.fork()
        if pid == 0:
            # EOF/error is not permission to release the inherited descriptor.
            try:
                os.close(ready_read)
                os.close(finish_write)
                if os.write(ready_write, b'r') != 1 or os.read(finish_read, 1) != b'e':
                    raise RuntimeError()
                os.execve('/usr/bin/true', ['/usr/bin/true'], {})
            except BaseException:
                while True:
                    time.sleep(60)
        os.close(ready_write)
        os.close(finish_read)
        self.assertEqual(select.select([ready_read], [], [], 5)[0], [ready_read])
        self.assertEqual(os.read(ready_read, 1), b'r')
        self.assertIsNone(os.waitid(os.P_PID, pid, os.WEXITED | os.WNOHANG | os.WNOWAIT))
        os.close(held)
        probe = os.open(root, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC)
        self.assertEqual((os.fstat(probe).st_dev, os.fstat(probe).st_ino),
                         (original.st_dev, original.st_ino))
        with self.assertRaises(BlockingIOError) as blocked:
            fcntl.flock(probe, fcntl.LOCK_EX | fcntl.LOCK_NB)
        self.assertIn(blocked.exception.errno, (errno.EAGAIN, errno.EWOULDBLOCK))
        self.assertEqual(os.write(finish_write, b'e'), 1)
        known_exit(pid)
        # One attempt only: no eventual-success retry or explicit unlock.
        fcntl.flock(probe, fcntl.LOCK_EX | fcntl.LOCK_NB)
        self.assertEqual((root.lstat().st_dev, root.lstat().st_ino),
                         (original.st_dev, original.st_ino))
        for fd in (probe, ready_read, finish_write):
            os.close(fd)
        root.rmdir()


if __name__ == '__main__':
    unittest.main()
