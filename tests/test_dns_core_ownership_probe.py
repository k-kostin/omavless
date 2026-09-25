"""Offline safety contracts: no core, namespaces, DNS or authorization."""
import contextlib
import hashlib
import io
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import Mock, patch

sys.path.insert(0, str(Path(__file__).resolve().parent))
import dns_core_ownership_probe as probe


class OwnershipProbeTests(unittest.TestCase):
    def report(self):
        return {'facts': dict.fromkeys(probe.FACTS, True), 'coreSha256': 'a' * 64}

    def parent(self, uid=1000, argv=None, result=None, error=None):
        stream = io.StringIO()
        if result is None:
            result = subprocess.CompletedProcess([], 0, json.dumps(self.report()).encode(), b'')
        with patch.object(sys, 'argv', argv or ['probe', '/candidate', 'a' * 64]), \
             patch.object(probe.os, 'geteuid', return_value=uid), \
             patch.object(probe, 'validate_source') as validate, \
             patch.object(probe.base.ns, 'namespace', side_effect=lambda name: 'old-' + name), \
             patch.object(probe.subprocess, 'run', return_value=result, side_effect=error) as run, \
             contextlib.redirect_stdout(stream):
            code = probe.main()
        return code, stream.getvalue(), run, validate

    def test_parent_refuses_root_and_wrong_arguments_before_source_access(self):
        for uid, argv in [(0, None), (1000, ['probe']), (1000, ['probe', '--host'])]:
            code, _, run, validate = self.parent(uid, argv)
            self.assertEqual(code, 2)
            run.assert_not_called()
            validate.assert_not_called()

    def test_namespace_containment_and_bounded_wait(self):
        code, _, run, validate = self.parent()
        self.assertEqual(code, 0)
        validate.assert_called_once_with(Path('/candidate'), 'a' * 64)
        self.assertEqual(run.call_args.args[0][:8], [
            '/usr/bin/unshare', '--user', '--map-root-user', '--net', '--pid',
            '--fork', '--kill-child=SIGKILL', '--'])
        self.assertEqual(run.call_args.kwargs['timeout'], 60)
        self.assertEqual(run.call_args.kwargs['stdin'], subprocess.DEVNULL)

    def test_child_guard_precedes_copy_and_fixture_work(self):
        with patch.object(probe.base, 'guard', side_effect=RuntimeError), \
             patch.object(probe.tempfile, 'TemporaryDirectory') as temporary, \
             patch.object(probe, 'experiment') as experiment:
            with self.assertRaises(RuntimeError): probe.child('net', 'user', 'pid', '/candidate', 'a' * 64)
            temporary.assert_not_called()
            experiment.assert_not_called()

    def test_source_digest_mode_and_symlink(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'candidate'
            path.write_bytes(b'synthetic')
            path.chmod(0o700)
            digest = hashlib.sha256(b'synthetic').hexdigest()
            probe.validate_source(path, digest)
            with self.assertRaises(RuntimeError): probe.validate_source(path, 'a' * 64)
            path.chmod(0o722)
            with self.assertRaises(RuntimeError): probe.validate_source(path, digest)
            link = path.with_name('symlink')
            link.symlink_to(path)
            with self.assertRaises(RuntimeError): probe.validate_source(link, digest)

    def test_bad_owner_size_or_type_refuses_before_read(self):
        for mode, uid, size in [(0o100755, os.getuid() + 100, 1),
                                (0o100755, 0, 129 * 1024 * 1024), (0o040700, 0, 1)]:
            source = Mock()
            source.is_absolute.return_value = True
            source.lstat.return_value = Mock(st_mode=mode, st_uid=uid, st_size=size)
            with self.assertRaises(RuntimeError): probe.validate_source(source, 'a' * 64)
            source.read_bytes.assert_not_called()

    def test_copy_digest_mismatch_refuses_before_launch(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / 'source'
            source.write_bytes(b'synthetic')
            with patch.object(probe, 'launch') as launch:
                with self.assertRaises(RuntimeError): probe.experiment(root, source, 'a' * 64)
                launch.assert_not_called()

    def test_failures_do_not_echo_source_or_child_output(self):
        for result, error in [(subprocess.CompletedProcess([], 1, b'PRIVATE', b'PRIVATE'), None),
                              (None, subprocess.TimeoutExpired('PRIVATE', 60))]:
            code, text, _, _ = self.parent(result=result, error=error)
            self.assertEqual(code, 1)
            self.assertNotIn('PRIVATE', text)

    def test_unknown_flag_cannot_pass_as_false(self):
        for value in [{}, {'tun': {}}, {'tun': {'disable-system-dns': 0}},
                      {'tun': {'disable-system-dns': 'false'}}]:
            with patch.object(probe.base, 'request', return_value=json.dumps(value)):
                with self.assertRaises(RuntimeError): probe.flag(Path('/synthetic'))

    def test_typed_flags_and_omitted_legacy_config(self):
        root = Path('/synthetic')
        self.assertNotIn('disable-system-dns', probe.config(root, 'ovdnsprobe0', None))
        for disabled in [True, False]:
            text = probe.config(root, 'ovdnsprobe0', disabled)
            self.assertIn('disable-system-dns: ' + str(disabled).lower(), text)
            self.assertIn('auto-route: false', text)
            self.assertIn('dns-hijack: []', text)
            self.assertNotIn('external-controller:', text)
        for bad in [0, 1, 'true', [], {}]:
            with self.assertRaises(RuntimeError): probe.config(root, 'ovdnsprobe0', bad)

    def test_strict_projection_and_exact_candidate(self):
        value = self.report()
        self.assertEqual(probe.project(json.dumps(value).encode(), 'a' * 64), value)
        with self.assertRaises(RuntimeError): probe.project(json.dumps(value).encode(), 'b' * 64)
        for raw in [b'{}', b'[]', b' ' * 2049, b'{"facts":{},"facts":{}}']:
            with self.assertRaises((ValueError, RuntimeError)): probe.project(raw, 'a' * 64)

    def test_false_fact_is_not_success(self):
        value = self.report()
        value['facts']['true_suppresses_setup_and_close'] = False
        code, _, _, _ = self.parent(result=subprocess.CompletedProcess([], 0, json.dumps(value).encode(), b''))
        self.assertEqual(code, 1)

    def test_stock_core_has_safe_specific_refusal(self):
        code, text, _, _ = self.parent(result=subprocess.CompletedProcess(
            [], 1, b'{"unsupported_dns_ownership_capability":true}\n', b''))
        self.assertEqual(code, 1)
        self.assertEqual(json.loads(text), {'unsupported_dns_ownership_capability': True})

    def test_forced_exit_does_not_prove_close_path(self):
        for code in [-9, -15, 1, 0]:
            child = Mock(returncode=code)
            with patch.object(probe.base, 'stop', return_value=True):
                self.assertEqual(probe.stop_cleanly(child), code == 0)

    def test_existing_link_and_controller_do_not_prove_fd_readiness(self):
        for tun, expected in [({}, False), ({'enable': False, 'file-descriptor': 42}, False),
                              ({'enable': True, 'file-descriptor': 7}, False),
                              ({'enable': True, 'file-descriptor': 42}, True)]:
            with patch.object(probe.base, 'request', side_effect=[b'{}', json.dumps({'tun': tun})]), \
                 patch.object(probe.base, 'index', return_value=1), \
                 patch.object(probe.base, 'wait_for', side_effect=lambda predicate, child:
                     self.assertEqual(predicate(), expected)):
                probe.wait_ready(Path('/synthetic'), Mock(), 42)

    def test_fd_handoff_is_explicit_and_does_not_change_packet_dns(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            with patch.object(probe.subprocess, 'Popen') as launch:
                probe.launch(root, root / 'candidate', True, fd=42)
            self.assertEqual(launch.call_args.kwargs['pass_fds'], (42,))
            text = (root / 'config.yaml').read_text()
            self.assertIn('file-descriptor: 42', text)
            self.assertIn('disable-system-dns: true', text)
            self.assertIn('dns-hijack: []', text)
            for invalid in [0, 1, 2, True, '/private']:
                with self.assertRaises(RuntimeError): probe.launch(root, root / 'candidate', True, fd=invalid)

    def test_launch_has_no_host_environment_and_no_real_resolvectl(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            with patch.object(probe.subprocess, 'Popen') as launch:
                probe.launch(root, root / 'candidate', True)
            kwargs = launch.call_args.kwargs
            self.assertEqual(kwargs['env']['PATH'], str(root))
            self.assertEqual(kwargs['env']['DBUS_SYSTEM_BUS_ADDRESS'], 'unix:path=' + str(root / 'absent-system-bus'))
            self.assertEqual(kwargs['stdout'], subprocess.DEVNULL)
            self.assertNotIn('subprocess', (root / 'resolvectl').read_text())


if __name__ == '__main__':
    unittest.main()
