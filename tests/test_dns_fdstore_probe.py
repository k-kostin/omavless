#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""Deterministic guards for the opt-in user-unit probe; no service started."""
import contextlib
import importlib.util
import io
import json
import os
from pathlib import Path
import subprocess
import unittest
from unittest.mock import Mock, patch

SPEC = importlib.util.spec_from_file_location('fdstore_probe', Path(__file__).with_name('dns_fdstore_probe.py'))
probe = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(probe)
UNIT = 'omavless-dns-fdstore-probe-' + 'a' * 32 + '.service'


def result(output=b'', code=0, error=b''):
    return subprocess.CompletedProcess([], code, output, error)


class ProbeTests(unittest.TestCase):
    def test_generated_name_only(self):
        self.assertEqual(probe.unit_name(UNIT), UNIT)
        for value in ('omavless.service', '*', '../' + UNIT, UNIT + '\n', UNIT.upper(), None):
            with self.subTest(value=value), self.assertRaises(RuntimeError):
                probe.unit_name(value)

    def test_control_cannot_target_installed_unit(self):
        with patch.object(probe, 'command') as command:
            with self.assertRaises(RuntimeError):
                probe.control('omavless.service', 'stop')
            command.assert_not_called()

    def test_control_user_manager_only(self):
        with patch.object(probe, 'command') as command:
            probe.control(UNIT, 'stop')
            command.assert_called_once_with(['/usr/bin/systemctl', '--user', 'stop', UNIT], check=True)

    def test_control_refuses_unlisted_action(self):
        with patch.object(probe, 'command') as command:
            with self.assertRaises(RuntimeError):
                probe.control(UNIT, 'enable')
            command.assert_not_called()

    def test_command_is_bounded_and_never_echoes_stderr(self):
        with patch.object(probe.subprocess, 'run', return_value=result(code=1, error=b'private-token')) as run:
            with self.assertRaisesRegex(RuntimeError, '^fdstore_probe_refused$'):
                probe.command(['synthetic'])
            self.assertEqual(run.call_args.kwargs['timeout'], 9)
            self.assertTrue(run.call_args.kwargs['capture_output'])

    def test_command_refuses_oversized_output(self):
        with patch.object(probe.subprocess, 'run', return_value=result(b'x' * 4097)):
            with self.assertRaises(RuntimeError):
                probe.command(['synthetic'], check=False)

    def test_count_requires_strict_properties(self):
        for text in (b'LoadState=loaded\nNFileDescriptorStore=0\n', b'LoadState=loaded\nNFileDescriptorStore=1\n'):
            with patch.object(probe, 'control', return_value=result(text)):
                self.assertIn(probe.count(UNIT), (0, 1))
        with patch.object(probe, 'control', return_value=result(b'LoadState=not-found\n', 1)):
            self.assertEqual(probe.count(UNIT), 0)

    def test_count_does_not_turn_errors_into_zero(self):
        for text in (b'', b'LoadState=loaded\n', b'LoadState=loaded\nNFileDescriptorStore=2\n',
                     b'LoadState=loaded\nNFileDescriptorStore=0\nNFileDescriptorStore=0\n',
                     b'LoadState=loaded\nEnvironment=private\n'):
            with self.subTest(text=text), patch.object(probe, 'control', return_value=result(text)):
                with self.assertRaises((RuntimeError, ValueError)):
                    probe.count(UNIT)

    def test_notify_refuses_arbitrary_payload(self):
        channel = Mock()
        with self.assertRaises(RuntimeError):
            probe.notify(channel, b'EXEC=arbitrary')
        channel.sendmsg.assert_not_called()

    def test_barrier_requires_eof(self):
        for data in (b'x',):
            with patch.object(probe.os, 'pipe2', return_value=(100, 101)), \
                 patch.object(probe.os, 'close') as close, patch.object(probe, 'notify'), \
                 patch.object(probe.select, 'select', return_value=([100], [], [])), \
                 patch.object(probe.os, 'read', return_value=data):
                with self.assertRaises(RuntimeError):
                    probe.barrier(Mock())
                self.assertCountEqual([call.args[0] for call in close.call_args_list], [100, 101])

    def test_barrier_closes_descriptors_on_send_failure(self):
        with patch.object(probe.os, 'pipe2', return_value=(100, 101)), \
             patch.object(probe.os, 'close') as close, \
             patch.object(probe, 'notify', side_effect=OSError('private')):
            with self.assertRaises(OSError):
                probe.barrier(Mock())
            self.assertCountEqual([call.args[0] for call in close.call_args_list], [100, 101])

    def test_barrier_closes_descriptors_on_timeout(self):
        with patch.object(probe.os, 'pipe2', return_value=(100, 101)), \
             patch.object(probe.os, 'close') as close, patch.object(probe, 'notify'), \
             patch.object(probe.select, 'select', return_value=([], [], [])):
            with self.assertRaises(RuntimeError):
                probe.barrier(Mock())
            self.assertCountEqual([call.args[0] for call in close.call_args_list], [100, 101])

    def test_cleanup_checks_process_and_descriptors(self):
        with patch.object(probe, 'count', return_value=0), \
             patch.object(probe, 'control', return_value=result(b'LoadState=loaded\nActiveState=inactive\nMainPID=0\n')) as control:
            probe.cleanup(UNIT)
            self.assertEqual([call.args[1] for call in control.call_args_list], ['stop', 'clean', 'reset-failed', 'show'])
            self.assertTrue(all(call.args[0] == UNIT for call in control.call_args_list))

    def test_cleanup_does_not_claim_success_with_live_worker(self):
        with patch.object(probe, 'count', return_value=0), \
             patch.object(probe, 'control', return_value=result(b'LoadState=loaded\nActiveState=active\nMainPID=123\n')):
            with self.assertRaises(RuntimeError):
                probe.cleanup(UNIT)

    def test_failed_launch_still_cleans_exact_generated_unit(self):
        with patch.object(probe, 'command', side_effect=RuntimeError('synthetic')), \
             patch.object(probe, 'cleanup') as cleanup:
            with self.assertRaises(RuntimeError):
                probe.one_case('yes', 1)
            self.assertEqual(cleanup.call_count, 1)
            probe.unit_name(cleanup.call_args.args[0])

    def test_launch_is_user_only_bounded_no_privilege(self):
        with patch.object(probe, 'command') as command, patch.object(probe, 'cleanup'), \
             patch.object(probe, 'await_record', return_value={'barrier': True}), \
             patch.object(probe, 'count', return_value=0):
            self.assertEqual(probe.one_case('yes', 0), {'barrier_without_capacity_not_acceptance': True})
            args = command.call_args.args[0]
            self.assertEqual(args[:2], ['/usr/bin/systemd-run', '--user'])
            for option in ('--property=NotifyAccess=main', '--property=NoNewPrivileges=yes',
                           '--property=RuntimeMaxSec=25s', '--property=StartLimitBurst=5'):
                self.assertIn(option, args)
            self.assertNotIn('sudo', args)

    def test_worker_rejects_root_before_environment(self):
        with patch.object(probe.os, 'geteuid', return_value=0), patch.object(probe.socket, 'socket') as socket:
            with self.assertRaises(RuntimeError):
                probe.worker('/tmp/omavless-fdstore.fake')
            socket.assert_not_called()

    def test_main_root_or_unknown_argument_never_runs_unit(self):
        for argv, uid in ([['probe'], 0], [['probe', '--arbitrary'], 1000]):
            with patch.object(probe.sys, 'argv', argv), patch.object(probe.os, 'geteuid', return_value=uid), \
                 patch.object(probe, 'one_case') as run, contextlib.redirect_stdout(io.StringIO()):
                self.assertEqual(probe.main(), 2)
                run.assert_not_called()

    def test_public_error_has_no_raw_detail(self):
        output = io.StringIO()
        with patch.object(probe.sys, 'argv', ['probe']), patch.object(probe.os, 'geteuid', return_value=1000), \
             patch.object(probe, 'one_case', side_effect=RuntimeError('password=private-token')), \
             contextlib.redirect_stdout(output):
            self.assertEqual(probe.main(), 1)
        self.assertEqual(json.loads(output.getvalue()), {'user_fdstore_probe_failed': True})

    def test_record_roundtrip_is_private_and_bounded(self):
        with probe.tempfile.TemporaryDirectory(prefix='omavless-fdstore.') as directory:
            root = Path(directory)
            value = {'generation': 1, 'device': 1, 'inode': 2, 'same': True, 'barrier': True, 'removed': False}
            probe.record(root, value)
            self.assertEqual(probe.read_record(root), value)
            self.assertEqual(os.stat(root / 'record').st_mode & 0o777, 0o600)
            with self.assertRaises(RuntimeError):
                probe.record(root, {'oversized': 'x' * 513})


if __name__ == '__main__':
    unittest.main()
