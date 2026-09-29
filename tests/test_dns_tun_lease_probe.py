"""Offline admission/containment tests; no host device or network operations."""
import array
import contextlib
import errno
import io
import json
from pathlib import Path
import socket
import stat
import struct
import subprocess
import sys
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, patch

sys.path.insert(0, str(Path(__file__).resolve().parent))
import dns_tun_lease_probe as probe


def info(mode=stat.S_IFCHR, rdev=None, dev=7, ino=9):
    return SimpleNamespace(st_mode=mode,
        st_rdev=probe.os.makedev(10, 200) if rdev is None else rdev,
        st_dev=dev, st_ino=ino)


class LeaseAdmissionTests(unittest.TestCase):
    def test_valid_fd_checks_real_namespace_and_closes_namespace_fd(self):
        with patch.object(probe.os, 'fstat', side_effect=[info(), info()]), \
             patch.object(probe.fcntl, 'ioctl', side_effect=[struct.pack('16sH22x', probe.tun.NAME, probe.tun.FLAGS), 42]) as ioctl, \
             patch.object(probe.os, 'close') as close:
            self.assertTrue(probe.admit(12, (7, 9)))
            self.assertEqual(ioctl.call_args_list[1].args, (12, probe.TUNGETDEVNETNS))
            close.assert_called_once_with(42)

    def test_regular_socket_and_wrong_character_device_refuse_before_ioctl(self):
        for metadata in [info(stat.S_IFREG), info(stat.S_IFSOCK), info(rdev=0)]:
            with patch.object(probe.os, 'fstat', return_value=metadata), \
                 patch.object(probe.fcntl, 'ioctl') as ioctl:
                self.assertTrue(probe.refused(12, (7, 9)))
                ioctl.assert_not_called()

    def test_unsupported_flags_and_name_refuse_before_namespace_lookup(self):
        cases = [(b'wrong', probe.tun.FLAGS), (probe.tun.NAME, 1),
                 (probe.tun.NAME, probe.IFF_TAP | 0x1000)]
        cases += [(probe.tun.NAME, probe.tun.FLAGS | flag) for flag in
                  [probe.IFF_MULTI_QUEUE, probe.IFF_PERSIST, probe.IFF_DETACH_QUEUE]]
        for name, flags in cases:
            with patch.object(probe.os, 'fstat', return_value=info()), \
                 patch.object(probe.fcntl, 'ioctl', return_value=struct.pack('16sH22x', name, flags)) as ioctl:
                self.assertTrue(probe.refused(12, (7, 9)))
                self.assertEqual(ioctl.call_count, 1)

    def test_detached_refusal_is_not_an_unsupported_kernel_pass(self):
        for number in [errno.EBADFD, errno.EINVAL, errno.EPERM, errno.ENOTTY]:
            with patch.object(probe.os, 'fstat', return_value=info()), \
                 patch.object(probe.fcntl, 'ioctl', side_effect=OSError(number, 'private')):
                if number == errno.EBADFD:
                    self.assertTrue(probe.refused(12, (7, 9)))
                else:
                    with self.assertRaises(OSError): probe.refused(12, (7, 9))

    def test_foreign_namespace_refuses_and_closes_fd_even_if_stat_fails(self):
        for outcome in [info(ino=10), OSError(errno.EIO, 'private')]:
            with patch.object(probe.os, 'fstat', side_effect=[info(), outcome]), \
                 patch.object(probe.fcntl, 'ioctl', side_effect=[struct.pack('16sH22x', probe.tun.NAME, probe.tun.FLAGS), 42]), \
                 patch.object(probe.os, 'close') as close:
                with self.assertRaises((probe.Rejected, OSError)): probe.admit(12, (7, 9))
                close.assert_called_once_with(42)

    def test_namespace_ioctl_unavailable_is_failure(self):
        with patch.object(probe.os, 'fstat', return_value=info()), \
             patch.object(probe.fcntl, 'ioctl', side_effect=[struct.pack('16sH22x', probe.tun.NAME, probe.tun.FLAGS), OSError(errno.ENOTTY, 'private')]):
            with self.assertRaises(OSError): probe.refused(12, (7, 9))

    def test_device_create_and_cleanup_errors_still_close_original_fd(self):
        for effects, kwargs in [([OSError()], {}),
                                ([0, 0, OSError()], {'persistent': True})]:
            with patch.object(probe.os, 'open', return_value=42), \
                 patch.object(probe.fcntl, 'ioctl', side_effect=effects), \
                 patch.object(probe.os, 'close') as close:
                with self.assertRaises(OSError):
                    with probe.device(**kwargs): pass
                close.assert_called_once_with(42)

    def test_persistence_negative_fixture_is_reverted_via_held_fd(self):
        with patch.object(probe.os, 'open', return_value=42), \
             patch.object(probe.fcntl, 'ioctl') as ioctl, patch.object(probe.os, 'close') as close:
            with probe.device(persistent=True): pass
            self.assertEqual(ioctl.call_args_list[-1].args, (42, probe.TUNSETPERSIST, 0))
            close.assert_called_once_with(42)

    def test_retention_closes_holder_on_failed_readback(self):
        with patch.object(probe, 'device', return_value=contextlib.nullcontext(12)), \
             patch.object(probe, 'admit', side_effect=[True, RuntimeError()]), \
             patch.object(probe.os, 'dup', return_value=42), \
             patch.object(probe.tun, 'tun_index', return_value=7), \
             patch.object(probe.os, 'close') as close:
            with self.assertRaises(RuntimeError): probe.retention((7, 9))
            close.assert_called_once_with(42)


class LeaseTransportTests(unittest.TestCase):
    def test_real_local_rights_transfer_is_cloexec_and_same_object(self):
        # Ordinary local file descriptor, not TUN or privileged transport.
        sender, recipient = socket.socketpair(socket.AF_UNIX, socket.SOCK_DGRAM)
        with sender, recipient:
            rights = array.array('i', [sender.fileno()])
            sender.sendmsg([b'x'], [(socket.SOL_SOCKET, socket.SCM_RIGHTS, rights)])
            fd = probe.receive_fd(recipient)
            try:
                self.assertFalse(probe.os.get_inheritable(fd))
                self.assertEqual(probe.identity(fd), probe.identity(sender.fileno()))
            finally:
                probe.os.close(fd)

    def test_bad_ancillary_closes_every_received_descriptor(self):
        rights = array.array('i', [40, 41]).tobytes()
        cases = [(b'x', [(socket.SOL_SOCKET, socket.SCM_RIGHTS, rights)], 0),
                 (b'x', [(socket.SOL_SOCKET, socket.SCM_RIGHTS, rights[:4])], socket.MSG_CTRUNC),
                 (b'bad', [(socket.SOL_SOCKET, socket.SCM_RIGHTS, rights[:4])], 0),
                 (b'x', [(socket.SOL_SOCKET, socket.SCM_RIGHTS, rights[:4] + b'!')], 0)]
        for data, ancillary, flags in cases:
            channel = Mock()
            channel.recvmsg.return_value = data, ancillary, flags, None
            with patch.object(probe.os, 'close') as close:
                with self.assertRaises(RuntimeError): probe.receive_fd(channel)
                self.assertEqual([call.args[0] for call in close.call_args_list],
                                 [40, 41] if len(ancillary[0][2]) == 8 else [40])

    def test_missing_or_unknown_ancillary_refused(self):
        for ancillary in [[], [(999, 999, b'')]]:
            channel = Mock()
            channel.recvmsg.return_value = b'x', ancillary, 0, None
            with self.assertRaises(RuntimeError): probe.receive_fd(channel)

    def test_foreign_child_timeout_killed_and_joined(self):
        process = Mock()
        process.poll.return_value = None
        with patch.object(probe.subprocess, 'Popen', return_value=process) as start, \
             patch.object(probe, 'receive_fd', side_effect=TimeoutError):
            with self.assertRaises(TimeoutError): probe.foreign_case('net', 'user', 'pid', (7, 9))
            process.kill.assert_called_once()
            process.wait.assert_called_once_with(timeout=4)
            self.assertEqual(start.call_args.args[0][:3], ['/usr/bin/unshare', '--net', '--'])
            self.assertEqual(len(start.call_args.kwargs['pass_fds']), 1)

    def test_received_fd_closed_even_if_foreign_child_join_fails(self):
        process = Mock()
        process.poll.return_value = None
        process.wait.side_effect = subprocess.TimeoutExpired('child', 4)
        with patch.object(probe.subprocess, 'Popen', return_value=process), \
             patch.object(probe, 'receive_fd', return_value=42), \
             patch.object(probe.os, 'close') as close:
            with self.assertRaises(subprocess.TimeoutExpired): probe.foreign_case('net', 'user', 'pid', (7, 9))
            close.assert_called_once_with(42)


class LeaseContainmentTests(unittest.TestCase):
    def test_guards_precede_devices_scratch_and_namespace_fd(self):
        with patch.object(probe.base, 'guard', side_effect=RuntimeError), \
             patch.object(probe, 'device') as device, patch.object(probe.os, 'open') as opened, \
             patch.object(probe.tempfile, 'TemporaryFile') as temporary:
            with self.assertRaises(RuntimeError): probe.owner('net', 'user', 'pid')
            device.assert_not_called()
            opened.assert_not_called()
            temporary.assert_not_called()

    def test_foreign_worker_guard_before_inherited_socket_or_device(self):
        with patch.object(probe.base, 'guard', side_effect=RuntimeError), \
             patch.object(probe.socket, 'socket') as channel, patch.object(probe, 'device') as device:
            with self.assertRaises(RuntimeError): probe.foreign_worker('net', 'user', 'pid', '3')
            channel.assert_not_called()
            device.assert_not_called()

    def test_projection_is_exact_bounded_boolean_and_duplicate_free(self):
        valid = dict.fromkeys(probe.FACTS, True)
        self.assertEqual(probe.project(json.dumps(valid).encode()), valid)
        for raw in [b'{}', b'[]', b'\xff', b' ' * 2049,
                    b'{"isolated":true,"isolated":true}',
                    json.dumps({**valid, 'extra': True}).encode(),
                    json.dumps({**valid, 'isolated': 1}).encode()]:
            with self.assertRaises((ValueError, RuntimeError)): probe.project(raw)

    def test_root_parent_and_extra_args_cannot_launch(self):
        for uid, args in [(0, ['probe']), (1000, ['probe', '--host'])]:
            with patch.object(probe.os, 'geteuid', return_value=uid), \
                 patch.object(sys, 'argv', args), patch.object(probe.subprocess, 'run') as run, \
                 contextlib.redirect_stdout(io.StringIO()):
                self.assertEqual(probe.main(), 2)
                run.assert_not_called()

    def test_parent_uses_all_namespaces_and_never_forwards_raw_errors(self):
        good = json.dumps(dict.fromkeys(probe.FACTS, True)).encode()
        for data, code, expected in [(good, 0, 0), (good, 1, 1), (b'private sentinel', 1, 1)]:
            output = io.StringIO()
            with patch.object(probe.os, 'geteuid', return_value=1000), \
                 patch.object(sys, 'argv', ['probe']), \
                 patch.object(probe.tun, 'namespace', side_effect=lambda kind: kind), \
                 patch.object(probe.subprocess, 'run', return_value=subprocess.CompletedProcess([], code, data, b'private sentinel')) as run, \
                 contextlib.redirect_stdout(output):
                self.assertEqual(probe.main(), expected)
            self.assertEqual(run.call_args.args[0][:8], ['/usr/bin/unshare', '--user', '--map-root-user',
                '--net', '--pid', '--fork', '--kill-child=SIGKILL', '--'])
            self.assertEqual(run.call_args.kwargs['timeout'], 30)
            self.assertNotIn('private sentinel', output.getvalue())


if __name__ == '__main__':
    unittest.main()
