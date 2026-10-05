"""Real local procfs controls; no namespace, mount, daemon or ELF execution."""
import errno
import os
import tempfile
import time
import unittest
from types import SimpleNamespace
from unittest.mock import Mock, patch

from tests.live_fd_tmpfs import bridge
from tests import test_live_fd_bridge as controls


class LiveEnumerationTests(unittest.TestCase):
    def test_breadcrumb_is_finite_bounded_and_first_failure_seals(self):
        for cause in ("short", "unknown", "label", "bound"):
            with patch.object(bridge, "_breadcrumb_count", 128 if cause == "bound" else 0), \
                 patch.object(bridge, "_breadcrumb_refused", False), \
                 patch.object(bridge.os, "write") as write:
                write.return_value = 0
                if cause == "unknown":
                    write.side_effect = OSError(errno.EIO, "synthetic")
                with self.assertRaises((OSError, bridge.Refused)):
                    bridge.breadcrumb("private arbitrary message" if cause == "label" else "before_copy")
                write.reset_mock()
                with self.assertRaisesRegex(bridge.Refused, "breadcrumb_sealed"):
                    bridge.breadcrumb("before_copy")
                write.assert_not_called()
        with patch.object(bridge, "_breadcrumb_count", 0), \
             patch.object(bridge, "_breadcrumb_refused", False), \
             patch.object(bridge.os, "write", side_effect=lambda _, data: len(data)) as write:
            bridge.breadcrumb("before_copy")
        write.assert_called_once_with(2, b"T3_LIVE_FD_PHASE_V1 before_copy\n")

    def test_failed_before_effect_breadcrumb_permanently_stops_prepare(self):
        obj = controls.fixture()
        with patch.object(obj, "_boundary"), patch.object(bridge.os, "mkdir") as mkdir, \
             patch.object(bridge, "breadcrumb", side_effect=OSError(errno.EIO, "synthetic")):
            with self.assertRaises(OSError):
                obj.prepare({})
            controls.LiveFdBridgeTests().assert_terminal(obj)
        mkdir.assert_not_called()
        obj.base.command.assert_not_called()

    def test_actual_stream_checks_internal_descriptor_while_live(self):
        actual = os.fstat
        checked = []
        def check(fd):
            value = actual(fd)
            checked.append(fd)
            return value
        with patch.object(bridge.os, "fstat", side_effect=check):
            bridge.no_writable_fds(os.stat("/proc/self/fd").st_dev)
        self.assertGreaterEqual(len(checked), 3)
        # Both owned directory FDs have gone away only AFTER successful checks.
        ended = []
        for fd in checked:
            try:
                actual(fd)
            except OSError as error:
                self.assertEqual(error.errno, errno.EBADF)
                ended.append(fd)
        self.assertEqual(len(ended), 2)

    def test_real_writable_fd_of_each_access_mode_refuses(self):
        with tempfile.TemporaryDirectory() as directory:
            path = directory + "/owned"
            for flags in (os.O_WRONLY, os.O_RDWR):
                fd = os.open(path, flags | os.O_CREAT | os.O_EXCL, 0o600)
                try:
                    with self.assertRaisesRegex(bridge.Refused, "copy_writable_fd"):
                        bridge.no_writable_fds(os.fstat(fd).st_dev)
                finally:
                    os.close(fd)
                    os.unlink(path)

    def test_real_unknown_fd_is_not_ignored_and_bridge_stays_sealed(self):
        obj = controls.fixture()
        obj.state = "ready"
        actual = os.fstat
        with tempfile.TemporaryFile() as owned:
            fd = owned.fileno()
            def check(number):
                if number == fd:
                    raise OSError(errno.EBADF, "injected actual owned FD uncertainty")
                return actual(number)
            obj._verify_all = lambda _: bridge.no_writable_fds(-1)
            with patch.object(bridge.os, "fstat", side_effect=check), self.assertRaises(OSError):
                obj.verify(time.monotonic() + 1)
            with patch.object(bridge.os, "scandir") as reopened:
                controls.LiveFdBridgeTests().assert_terminal(obj)
            reopened.assert_not_called()

    def test_malformed_duplicate_overflow_and_missing_directory_refuse(self):
        cases = [[""], ["01"], ["-1"], ["١"], ["2147483648"], ["9", "9"],
                 [str(i) for i in range(129)], ["10"]]
        for names in cases:
            stream = Mock()
            stream.__enter__ = Mock(return_value=iter(SimpleNamespace(name=n) for n in names))
            stream.__exit__ = Mock(return_value=False)
            with patch.object(bridge.os, "open", return_value=9), \
                 patch.object(bridge.os, "close"), \
                 patch.object(bridge.os, "scandir", return_value=stream), \
                 patch.object(bridge.os, "fstat", return_value=SimpleNamespace(st_dev=44)), \
                 self.assertRaises(bridge.Refused):
                bridge.no_writable_fds(45)
            stream.__exit__.assert_called_once()

    def test_every_yielded_same_device_fd_is_checked_before_next_entry(self):
        events = []
        def entries():
            for number in (9, 10, 11):
                events.append(("yield", number))
                yield SimpleNamespace(name=str(number))
        stream = Mock()
        stream.__enter__ = Mock(return_value=entries())
        stream.__exit__ = Mock(return_value=False)
        with patch.object(bridge.os, "open", return_value=9), patch.object(bridge.os, "close"), \
             patch.object(bridge.os, "scandir", return_value=stream), \
             patch.object(bridge.os, "fstat", return_value=SimpleNamespace(st_dev=44)), \
             patch.object(bridge.fcntl, "fcntl", side_effect=lambda fd, _: events.append(("check", fd)) or os.O_RDONLY):
            bridge.no_writable_fds(44)
        self.assertEqual(events, [item for fd in (9, 10, 11) for item in (("yield", fd), ("check", fd))])

    def test_exact_entry_bound_and_iterator_error(self):
        def failing():
            yield SimpleNamespace(name="9")
            raise OSError(errno.EIO, "synthetic enumeration uncertainty")
        for failed in (False, True):
            stream = Mock()
            stream.__enter__ = Mock(return_value=failing() if failed else
                                    iter(SimpleNamespace(name=str(n)) for n in range(128)))
            stream.__exit__ = Mock(return_value=False)
            with patch.object(bridge.os, "open", return_value=9), \
                 patch.object(bridge.os, "close") as closed, \
                 patch.object(bridge.os, "scandir", return_value=stream) as opened, \
                 patch.object(bridge.os, "fstat", return_value=SimpleNamespace(st_dev=44)):
                if failed:
                    with self.assertRaises(OSError):
                        bridge.no_writable_fds(45)
                else:
                    bridge.no_writable_fds(45)
            opened.assert_called_once_with(9)
            closed.assert_called_once_with(9)
            stream.__exit__.assert_called_once()


if __name__ == "__main__":
    unittest.main()
