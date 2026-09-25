"""No-effect safety contracts for the opt-in isolated packet gate."""
import contextlib
import io
import json
from pathlib import Path
import subprocess
import sys
import unittest
from unittest.mock import Mock, patch

sys.path.insert(0, str(Path(__file__).resolve().parent))
import dns_core_packet_probe as probe


class PacketProbeTests(unittest.TestCase):
    def test_root_parent_and_extra_arguments_refuse_without_access(self):
        for uid, args in [(0, ['probe', '/synthetic/core', 'a' * 64]),
                          (1000, ['probe', '--host']), (1000, ['probe'])]:
            with patch.object(sys, 'argv', args), patch.object(probe.os, 'geteuid', return_value=uid), \
                 patch.object(probe.ownership, 'validate_source') as validate, \
                 patch.object(probe.subprocess, 'run') as run, contextlib.redirect_stdout(io.StringIO()):
                self.assertEqual(probe.main(), 2)
                validate.assert_not_called()
                run.assert_not_called()

    def test_parent_validates_before_namespace_launch(self):
        with patch.object(sys, 'argv', ['probe', '/synthetic/core', 'bad']), \
             patch.object(probe.os, 'geteuid', return_value=1000), \
             patch.object(probe.ownership, 'validate_source', side_effect=RuntimeError), \
             patch.object(probe.subprocess, 'run') as run, contextlib.redirect_stdout(io.StringIO()):
            self.assertEqual(probe.main(), 1)
            run.assert_not_called()

    def test_child_guard_precedes_source_and_fixture_access(self):
        args = ['probe', '--isolated', 'net', 'user', 'pid', '/synthetic/core', 'a' * 64]
        with patch.object(sys, 'argv', args), \
             patch.object(probe.base, 'guard', side_effect=RuntimeError), \
             patch.object(probe.ownership, 'validate_source') as validate, \
             patch.object(probe.tempfile, 'TemporaryDirectory') as temporary, \
             contextlib.redirect_stdout(io.StringIO()):
            self.assertEqual(probe.main(), 1)
            validate.assert_not_called()
            temporary.assert_not_called()

    def test_experiment_itself_checks_namespace_before_copy_device_or_routes(self):
        with patch.object(probe.base, 'guard', side_effect=RuntimeError), \
             patch.object(probe.shutil, 'copyfile') as copy, \
             patch.object(probe.os, 'open') as device, patch.object(probe, 'ip') as ip:
            with self.assertRaises(RuntimeError):
                probe.experiment(Path('/synthetic'), Path('/synthetic/core'), 'a' * 64, 'net', 'user', 'pid')
            copy.assert_not_called()
            device.assert_not_called()
            ip.assert_not_called()

    def test_parent_contains_all_namespaces_and_only_projects_fixed_facts(self):
        valid = json.dumps(dict.fromkeys(probe.FACTS, True)).encode()
        for output, code, expected in [(valid, 0, 0), (b'private sentinel', 1, 1),
                                       (valid, 2, 1), (b'{}', 0, 1),
                                       (b' ' * 2049, 0, 1)]:
            stream = io.StringIO()
            with patch.object(sys, 'argv', ['probe', '/synthetic/core', 'a' * 64]), \
                 patch.object(probe.os, 'geteuid', return_value=1000), \
                 patch.object(probe.ownership, 'validate_source'), \
                 patch.object(probe.base.ns, 'namespace', side_effect=lambda item: item), \
                 patch.object(probe.subprocess, 'run', return_value=subprocess.CompletedProcess([], code, output, b'private sentinel')) as run, \
                 contextlib.redirect_stdout(stream):
                self.assertEqual(probe.main(), expected)
            self.assertEqual(run.call_args.args[0][:8], ['/usr/bin/unshare', '--user', '--map-root-user',
                             '--net', '--pid', '--fork', '--kill-child=SIGKILL', '--'])
            self.assertEqual(run.call_args.kwargs['timeout'], 40)
            self.assertNotIn('private sentinel', stream.getvalue())

    def test_route_query_bounds_and_command_shape(self):
        with patch.object(probe.subprocess, 'run', return_value=subprocess.CompletedProcess([], 0, b'[]', b'')) as run:
            self.assertEqual(probe.ip_query('route', 'get', probe.PEER), [])
            self.assertEqual(run.call_args.args[0], ['/usr/bin/ip', '-j', 'route', 'get', '192.0.2.1'])
            self.assertEqual(run.call_args.kwargs['timeout'], 3)
        for output in [b' ' * 8193, b'private sentinel', b'\xff']:
            with patch.object(probe.subprocess, 'run', return_value=subprocess.CompletedProcess([], 0, output, b'')):
                with self.assertRaises((RuntimeError, ValueError)):
                    probe.ip_query('link')

    def test_tun_counters_require_exact_target_and_numeric_counts(self):
        valid = {'ifname': 'ovdnsprobe0', 'stats64': {'rx': {'bytes': 1}, 'tx': {'bytes': 2}}}
        with patch.object(probe, 'ip_query', return_value=[valid]):
            self.assertEqual(probe.tun_counters(), (1, 2))
        for item in [[], [valid, valid], [{**valid, 'ifname': 'foreign'}],
                     [{**valid, 'stats64': {'rx': {'bytes': True}, 'tx': {'bytes': 2}}}]]:
            with patch.object(probe, 'ip_query', return_value=item):
                with self.assertRaises(RuntimeError): probe.tun_counters()

    def test_echo_failure_is_not_success_and_raw_errors_stay_private(self):
        for stream in (True, False):
            server = Mock()
            server.accept.side_effect = OSError('private sentinel')
            server.recvfrom.side_effect = OSError('private sentinel')
            self.assertFalse(probe.echo_once(server, stream))


if __name__ == '__main__':
    unittest.main()
