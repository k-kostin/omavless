"""No-effect guard tests for the opt-in actual-core namespace fixture."""
import contextlib
import io
import json
from pathlib import Path
import subprocess
import sys
import unittest
from unittest.mock import Mock, patch

sys.path.insert(0, str(Path(__file__).resolve().parent))
import dns_core_broker_probe as probe


class BrokerProbeTests(unittest.TestCase):
    def test_invalid_parent_has_no_file_service_or_namespace_effects(self):
        for uid, args in [(0, ['probe', '/core', 'a'*64, '/fixture', 'b'*64]),
                          (1000, ['probe']), (1000, ['probe', '--host'])]:
            with patch.object(sys, 'argv', args), patch.object(probe.os, 'geteuid', return_value=uid), \
                 patch.object(probe.ownership, 'validate_source') as validate, \
                 patch.object(probe.subprocess, 'run') as run, contextlib.redirect_stdout(io.StringIO()):
                self.assertEqual(probe.main(), 1)
                validate.assert_not_called()
                run.assert_not_called()

    def test_rejected_binary_never_starts_namespace(self):
        with patch.object(sys, 'argv', ['probe', '/core', 'a'*64, '/fixture', 'b'*64]), \
             patch.object(probe.os, 'geteuid', return_value=1000), \
             patch.object(probe.ownership, 'validate_source', side_effect=RuntimeError), \
             patch.object(probe.subprocess, 'run') as run, contextlib.redirect_stdout(io.StringIO()):
            self.assertEqual(probe.main(), 1)
            run.assert_not_called()

    def test_child_guard_precedes_scratch_creation(self):
        with patch.object(probe, 'guard', side_effect=RuntimeError), \
             patch.object(probe.tempfile, 'TemporaryDirectory') as temporary:
            with self.assertRaises(RuntimeError):
                probe.child(['net', 'user', 'pid', 'mnt', '/core', 'a'*64, '/fixture', 'b'*64])
            temporary.assert_not_called()

    def test_experiment_rechecks_before_mount_or_copy(self):
        with patch.object(probe, 'guard', side_effect=RuntimeError), \
             patch.object(probe.subprocess, 'run') as mount, patch.object(probe, 'copied') as copy:
            with self.assertRaises(RuntimeError):
                probe.experiment(Path('/fixture'), Path('/core'), 'a'*64, Path('/fixture-bin'), 'b'*64,
                                 ['net', 'user', 'pid', 'mnt'])
            mount.assert_not_called()
            copy.assert_not_called()

    def test_private_mount_namespace_required(self):
        with patch.object(probe.base, 'guard') as base_guard, \
             patch.object(probe.base.ns, 'namespace', return_value='same'):
            with self.assertRaises(RuntimeError): probe.guard(['n', 'u', 'p', 'same'])
            base_guard.assert_called_once_with('n', 'u', 'p')

    def test_parent_bounds_projection_and_contains_mount(self):
        valid = json.dumps({'coreSha256': 'a'*64, 'facts': dict.fromkeys(probe.FACTS, True)}).encode()
        for output, code, expected in [(valid, 0, 0), (valid, 1, 1), (b'private sentinel', 1, 1),
                                       (b'{}', 0, 1), (b' '*2049, 0, 1)]:
            stream = io.StringIO()
            with patch.object(sys, 'argv', ['probe', '/core', 'a'*64, '/fixture', 'b'*64]), \
                 patch.object(probe.os, 'geteuid', return_value=1000), \
                 patch.object(probe.ownership, 'validate_source'), \
                 patch.object(probe.base.ns, 'namespace', side_effect=lambda kind: kind), \
                 patch.object(probe.subprocess, 'run', return_value=subprocess.CompletedProcess([], code, output, b'private sentinel')) as run, \
                 contextlib.redirect_stdout(stream):
                self.assertEqual(probe.main(), expected)
            args = run.call_args.args[0]
            for required in ('--mount', '--net', '--user', '--pid', '--kill-child=SIGKILL'):
                self.assertIn(required, args)
            self.assertEqual(args[args.index('--propagation')+1], 'private')
            self.assertEqual(run.call_args.kwargs['timeout'], 60)
            self.assertNotIn('private sentinel', stream.getvalue())

    def test_fixture_commands_are_fixed(self):
        child = Mock()
        for invalid in ('connect', '../private', 'ready\ndrop_proof', 'password'):
            with self.assertRaises(RuntimeError): probe.command(child, invalid)
        child.stdin.write.assert_not_called()
        probe.command(child, 'ready')
        child.stdin.write.assert_called_once_with(b'ready\n')

    def test_controller_liveness_does_not_imply_ready(self):
        for value in ({'tun': {'enable': True}}, {'tun': {'omavless-dns-ready': 1}},
                      {'tun': {'omavless-dns-ready': False}}):
            with patch.object(probe.base, 'request', return_value=json.dumps(value).encode()):
                self.assertFalse(probe.readiness(Path('/synthetic')))
        with patch.object(probe.base, 'request', return_value=b'{"tun":{"omavless-dns-ready":true}}'):
            self.assertTrue(probe.readiness(Path('/synthetic')))


if __name__ == '__main__':
    unittest.main()
