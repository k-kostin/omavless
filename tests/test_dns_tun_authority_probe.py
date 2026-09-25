"""Offline contracts for namespace-only FD authority research. No host effects."""
import contextlib
import errno
import io
import json
from pathlib import Path
import struct
import subprocess
import sys
import unittest
from unittest.mock import Mock, patch

sys.path.insert(0, str(Path(__file__).resolve().parent))
import dns_tun_authority_probe as probe
import dns_tun_fd_policy as policy


def evaluate(machine, arch, number, command=0):
    # Interpret serialized BPF independently of the builder, using seccomp_data.
    raw = policy.encode(machine)
    program = list(struct.iter_unpack('=HBBI', raw))
    data = struct.pack('=IIQQQQQQQ', number, arch, 0, 99, command, 0, 0, 0, 0)
    pc, accumulator = 0, 0
    for _ in range(64):
        code, yes, no, value = program[pc]
        pc += 1
        if code == 0x20:
            accumulator = struct.unpack_from('=I', data, value)[0]
        elif code == 0x15:
            pc += yes if accumulator == value else no
        elif code == 0x45:
            pc += yes if accumulator & value else no
        elif code == 0x06:
            return value
        else:
            raise AssertionError('unsupported instruction')
    raise AssertionError('unterminated filter')


class FdPolicyTests(unittest.TestCase):
    def test_native_ioctl_read_and_libc_allowed(self):
        for machine, arch, number in [('aarch64', 0xC00000B7, 29), ('x86_64', 0xC000003E, 16)]:
            for request in [0x800454D2, 0x5401, 0x5451]:
                self.assertEqual(evaluate(machine, arch, number, request), 0x7FFF0000)

    def test_tun_mutators_and_unknown_ioctl_refused_on_both_architectures(self):
        for machine, arch, number in [('aarch64', 0xC00000B7, 29), ('x86_64', 0xC000003E, 16)]:
            for request in [0x400454CA, 0x400454CB, 0x400454CC, 0x400454CE,
                            0x400454CD, 0x400454D9, 0x400454DA, 0, 0xFFFFFFFF]:
                self.assertEqual(evaluate(machine, arch, number, request), 0x00050001)

    def test_other_syscalls_allowed_but_uring_and_x32_refused(self):
        for machine, arch, ioctl in [('aarch64', 0xC00000B7, 29), ('x86_64', 0xC000003E, 16)]:
            for number in [0, 1, 56, 57, 63, 64, 93]:
                if number != ioctl:
                    self.assertEqual(evaluate(machine, arch, number), 0x7FFF0000)
            for number in [425, 426, 427, 0x40000000 + ioctl, 0x40000202]:
                self.assertEqual(evaluate(machine, arch, number), 0x00050001)

    def test_foreign_abi_killed_and_unsupported_machine_refused(self):
        for machine, arch in [('aarch64', 0xC000003E), ('x86_64', 0x40000003)]:
            self.assertEqual(evaluate(machine, arch, 1), 0x80000000)
        for machine in ['armv7l', 'i686', 'unknown']:
            with self.assertRaises(ValueError): policy.encode(machine)

    def test_fd_export_syscalls_refused_on_both_architectures(self):
        for machine, arch, numbers in [('aarch64', 0xC00000B7, (211, 269)),
                                        ('x86_64', 0xC000003E, (46, 307))]:
            for number in numbers:
                self.assertEqual(evaluate(machine, arch, number), 0x00050001)

    def test_ioctl_high_bits_cannot_relabel_mutating_request(self):
        for machine, arch, number in [('aarch64', 0xC00000B7, 29), ('x86_64', 0xC000003E, 16)]:
            self.assertEqual(evaluate(machine, arch, number, 0x12345678400454CB), 0x00050001)


class AuthorityProbeTests(unittest.TestCase):
    def test_owner_guard_precedes_policy_files_and_device_access(self):
        with patch.object(probe.base, 'guard', side_effect=RuntimeError), \
             patch.object(probe.policy, 'encode') as encode, \
             patch.object(probe.tempfile, 'TemporaryDirectory') as temporary, \
             patch.object(probe.tun, 'open_tun') as device:
            with self.assertRaises(RuntimeError): probe.owner('net', 'user', 'pid')
            encode.assert_not_called()
            temporary.assert_not_called()
            device.assert_not_called()

    def test_worker_refuses_before_any_ioctl_if_capabilities_remain(self):
        with patch.object(probe.tun, 'namespace', return_value='new'), \
             patch.object(probe, 'dropped_capabilities', side_effect=RuntimeError), \
             patch.object(probe.tun, 'links') as links, \
             patch.object(probe.fcntl, 'ioctl') as ioctl:
            with self.assertRaises(RuntimeError): probe.worker('net', 'user', 'pid', '3')
            links.assert_not_called()
            ioctl.assert_not_called()

    def test_worker_refuses_original_namespace_before_privilege_checks(self):
        with patch.object(probe.tun, 'namespace', return_value='net'), \
             patch.object(probe, 'dropped_capabilities') as capabilities:
            with self.assertRaises(RuntimeError): probe.worker('net', 'user', 'pid', '3')
            capabilities.assert_not_called()

    def test_capability_and_nnp_checks_require_every_field(self):
        valid = 'CapInh: 0\nCapPrm: 0\nCapEff: 0\nCapBnd: 0\nCapAmb: 0\nNoNewPrivs: 1\n'
        with patch('builtins.open', return_value=io.BytesIO(valid.encode())):
            probe.dropped_capabilities()
        for field in ['CapInh', 'CapPrm', 'CapEff', 'CapBnd', 'CapAmb']:
            for text in [valid.replace(field + ': 0', field + ': 1000'), valid.replace(field + ': 0\n', '')]:
                with patch('builtins.open', return_value=io.BytesIO(text.encode())):
                    with self.assertRaises((RuntimeError, KeyError)): probe.dropped_capabilities()
        with patch('builtins.open', return_value=io.BytesIO(valid.replace('NoNewPrivs: 1', 'NoNewPrivs: 0').encode())):
            with self.assertRaises(RuntimeError): probe.dropped_capabilities()

    def test_permission_refusal_is_not_any_failure(self):
        self.assertFalse(probe.denied(lambda: None))
        for value in [errno.EPERM, errno.EINVAL, errno.EBADF]:
            def operation(): raise OSError(value, 'private sentinel')
            if value == errno.EPERM:
                self.assertTrue(probe.denied(operation))
            else:
                with self.assertRaises(RuntimeError): probe.denied(operation)
        for code, error, expected in [(2, b'RTNETLINK answers: Operation not permitted', True),
                                     (1, b'Cannot talk to rtnetlink: Operation not permitted', True),
                                     (2, b'No such device', False), (0, b'', False)]:
            with patch.object(probe.subprocess, 'run', return_value=subprocess.CompletedProcess([], code, b'', error)):
                self.assertEqual(probe.link_denied('delete', 'dev', 'ovdnstest0'), expected)

    def test_owner_dispatch_drops_all_caps_and_only_passes_owned_fd(self):
        response = subprocess.CompletedProcess([], 0, json.dumps(dict.fromkeys(probe.WORKER_FACTS, True)).encode(), b'')
        with patch.object(probe.base, 'guard'), \
             patch.object(probe.tun, 'open_tun', return_value=42), \
             patch.object(probe.tun, 'tun_index', return_value=7), \
             patch.object(probe.tun, 'fd_attached', return_value=True), \
             patch.object(probe.tun, 'links', return_value=[{'ifname': 'lo'}]), \
             patch.object(probe.subprocess, 'run', return_value=response) as run, \
             patch.object(probe.fcntl, 'ioctl') as ioctl, patch.object(probe.os, 'close') as close:
            self.assertTrue(all(probe.one_case('net', 'user', 'pid', Path('/synthetic-filter')).values()))
            args = run.call_args.args[0]
            self.assertIn('--bounding-set=-all', args)
            self.assertIn('--inh-caps=-all', args)
            self.assertIn('--ambient-caps=-all', args)
            self.assertIn('--no-new-privs', args)
            self.assertIn('--seccomp-filter', args)
            self.assertEqual(run.call_args.kwargs['pass_fds'], (42,))
            ioctl.assert_called_once_with(42, probe.TUNSETPERSIST, 0)
            close.assert_called_once_with(42)

    def test_owner_closes_held_fd_on_worker_timeout(self):
        with patch.object(probe.base, 'guard'), \
             patch.object(probe.tun, 'open_tun', return_value=42), \
             patch.object(probe.tun, 'tun_index', return_value=7), \
             patch.object(probe.subprocess, 'run', side_effect=subprocess.TimeoutExpired('worker', 15)), \
             patch.object(probe.fcntl, 'ioctl'), patch.object(probe.os, 'close') as close:
            with self.assertRaises(subprocess.TimeoutExpired): probe.one_case('net', 'user', 'pid')
            close.assert_called_once_with(42)

    def test_owner_closes_held_fd_even_if_persistence_reset_fails(self):
        with patch.object(probe.base, 'guard'), \
             patch.object(probe.tun, 'open_tun', return_value=42), \
             patch.object(probe.tun, 'tun_index', side_effect=RuntimeError), \
             patch.object(probe.fcntl, 'ioctl', side_effect=OSError), \
             patch.object(probe.os, 'close') as close:
            with self.assertRaises(OSError): probe.one_case('net', 'user', 'pid')
            close.assert_called_once_with(42)

    def test_projection_rejects_extra_missing_duplicate_and_nonboolean(self):
        valid = dict.fromkeys(probe.RESULT_FACTS, True)
        self.assertEqual(probe.project(json.dumps(valid).encode(), probe.RESULT_FACTS), valid)
        for raw in [b'{}', b'[]', b'\xff', b' ' * 2049, b'{"isolated":true,"isolated":true}',
                    json.dumps({**valid, 'extra': True}).encode(),
                    json.dumps({**valid, 'isolated': 1}).encode()]:
            with self.assertRaises((ValueError, RuntimeError)): probe.project(raw, probe.RESULT_FACTS)

    def test_root_parent_and_extra_args_refuse_before_effects(self):
        for uid, argv in [(0, ['probe']), (1000, ['probe', '--host'])]:
            with patch.object(probe.os, 'geteuid', return_value=uid), \
                 patch.object(sys, 'argv', argv), patch.object(probe.subprocess, 'run') as run, \
                 contextlib.redirect_stdout(io.StringIO()):
                self.assertEqual(probe.main(), 2)
                run.assert_not_called()

    def test_parent_uses_three_namespaces_and_only_safe_projection(self):
        good = json.dumps(dict.fromkeys(probe.RESULT_FACTS, True)).encode()
        for output, returncode, expected in [(good, 0, 0), (b'private sentinel', 1, 1)]:
            stream = io.StringIO()
            with patch.object(probe.os, 'geteuid', return_value=1000), \
                 patch.object(sys, 'argv', ['probe']), \
                 patch.object(probe.tun, 'namespace', side_effect=lambda name: name), \
                 patch.object(probe.subprocess, 'run', return_value=subprocess.CompletedProcess([], returncode, output, b'')) as run, \
                 contextlib.redirect_stdout(stream):
                self.assertEqual(probe.main(), expected)
            self.assertEqual(run.call_args.args[0][:8], ['/usr/bin/unshare', '--user', '--map-root-user',
                             '--net', '--pid', '--fork', '--kill-child=SIGKILL', '--'])
            self.assertNotIn('private sentinel', stream.getvalue())


if __name__ == '__main__':
    unittest.main()
