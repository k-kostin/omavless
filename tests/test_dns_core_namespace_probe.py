"""Effect-free guard/projection tests; never launch a core or namespace."""
import contextlib
import io
import json
from pathlib import Path
import subprocess
import sys
import unittest
from unittest.mock import patch, Mock

sys.path.insert(0, str(Path(__file__).resolve().parent))
import dns_core_namespace_probe as probe


def report():
    return {'facts': dict.fromkeys(probe.FACTS, True), 'coreSha256': 'a' * 64}


class CoreNamespaceTests(unittest.TestCase):
    def parent(self, uid=1000, argv=None, result=None, failure=None):
        stream = io.StringIO()
        if result is None:
            result = subprocess.CompletedProcess([], 0, json.dumps(report()).encode(), b'')
        with patch.object(probe.sys, 'argv', argv or ['probe']), \
             patch.object(probe.os, 'geteuid', return_value=uid), \
             patch.object(probe, 'core_digest', return_value='a' * 64) as digest, \
             patch.object(probe.ns, 'namespace', side_effect=lambda key: 'original-' + key), \
             patch.object(probe.subprocess, 'run', return_value=result, side_effect=failure) as run, \
             contextlib.redirect_stdout(stream):
            code = probe.main()
        return code, stream.getvalue(), run, digest

    def test_root_parent_refuses_without_reading_core(self):
        code, _, run, digest = self.parent(uid=0)
        self.assertEqual(code, 2)
        run.assert_not_called()
        digest.assert_not_called()

    def test_extra_argument_refuses_without_work(self):
        code, _, run, digest = self.parent(argv=['probe', '--host'])
        self.assertEqual(code, 2)
        run.assert_not_called()
        digest.assert_not_called()

    def test_three_namespaces_kill_child_and_deadline_required(self):
        code, text, run, _ = self.parent()
        self.assertEqual(code, 0)
        self.assertEqual(json.loads(text), report())
        argv = run.call_args.args[0]
        self.assertEqual(argv[:8], ['/usr/bin/unshare', '--user', '--map-root-user',
                         '--net', '--pid', '--fork', '--kill-child=SIGKILL', '--'])
        self.assertEqual(argv[-4:], ['original-net', 'original-user', 'original-pid', 'a' * 64])
        self.assertEqual(run.call_args.kwargs['timeout'], 30)
        self.assertEqual(run.call_args.kwargs['stdin'], subprocess.DEVNULL)

    def test_any_unchanged_namespace_refuses_before_inventory(self):
        for unchanged in ['net', 'user', 'pid']:
            with self.subTest(namespace=unchanged), \
                 patch.object(probe.ns, 'namespace', side_effect=lambda key:
                     ('original-' if key == unchanged else 'new-') + key), \
                 patch.object(probe.ns, 'links') as inventory:
                with self.assertRaises(RuntimeError):
                    probe.guard('original-net', 'original-user', 'original-pid')
                inventory.assert_not_called()

    def test_nonfresh_namespace_and_nonroot_child_refuse(self):
        for uid, links in [(1000, [{'ifname': 'lo'}]),
                           (0, [{'ifname': 'lo'}, {'ifname': 'foreign'}]), (0, [])]:
            with patch.object(probe.ns, 'namespace', side_effect=lambda key: 'new-' + key), \
                 patch.object(probe.os, 'geteuid', return_value=uid), \
                 patch.object(probe.ns, 'links', return_value=links):
                with self.assertRaises(RuntimeError):
                    probe.guard('original-net', 'original-user', 'original-pid')

    def test_child_guard_precedes_all_fixture_and_core_work(self):
        with patch.object(probe, 'guard', side_effect=RuntimeError), \
             patch.object(probe.tempfile, 'TemporaryDirectory') as temporary, \
             patch.object(probe, 'experiment') as experiment:
            with self.assertRaises(RuntimeError): probe.child('net', 'user', 'pid', 'a' * 64)
            temporary.assert_not_called()
            experiment.assert_not_called()

    def test_projection_rejects_unknown_keys_values_missing_facts_and_duplicate_keys(self):
        cases = [b'\xff', b' ' * 2049, b'[]', b'{}',
                 b'{"facts":{},"facts":{},"coreSha256":"private-sentinel"}']
        for change in ['extra', 'missing', 'string', 'integer', 'hash']:
            value = report()
            if change == 'extra': value['private-sentinel'] = True
            if change == 'missing': value['facts'].pop('isolated')
            if change == 'string': value['facts']['isolated'] = 'private-sentinel'
            if change == 'integer': value['facts']['isolated'] = 1
            if change == 'hash': value['coreSha256'] = 'private-sentinel'
            cases.append(json.dumps(value).encode())
        for raw in cases:
            with self.subTest(raw_type=type(raw)):
                with self.assertRaises((ValueError, RuntimeError)): probe.project(raw)

    def test_failed_child_and_timeout_never_echo_output(self):
        for failure in [OSError('private-sentinel'), subprocess.TimeoutExpired('private-sentinel', 30)]:
            code, text, _, _ = self.parent(failure=failure)
            self.assertEqual(code, 1)
            self.assertNotIn('sentinel', text)
        code, text, _, _ = self.parent(result=subprocess.CompletedProcess(
            [], 1, b'private-sentinel', b'private-sentinel'))
        self.assertEqual(code, 1)
        self.assertNotIn('sentinel', text)

    def test_false_fact_is_not_pass(self):
        value = report()
        value['facts']['same_tun_reload_no_dns_calls'] = False
        code, text, _, _ = self.parent(result=subprocess.CompletedProcess(
            [], 0, json.dumps(value).encode(), b''))
        self.assertEqual(code, 1)
        self.assertFalse(json.loads(text)['facts']['same_tun_reload_no_dns_calls'])

    def test_config_uses_only_synthetic_direct_and_fixed_device_names(self):
        root = Path('/tmp/synthetic fixture')
        text = probe.config(root, 'ovdnsprobe0', 'rule')
        self.assertIn('auto-route: false', text)
        self.assertIn('dns-hijack: []', text)
        self.assertIn('MATCH,DIRECT', text)
        self.assertNotIn('external-controller:', text)
        for device, mode in [('foreign', 'direct'), ('ovdnsprobe0', 'unknown')]:
            with self.assertRaises(RuntimeError): probe.config(root, device, mode)

    def test_cleanup_waits_then_kills_only_owned_child(self):
        child = Mock()
        child.poll.side_effect = [None, 0]
        child.wait.side_effect = [subprocess.TimeoutExpired('core', 4), 0]
        self.assertTrue(probe.stop(child))
        child.send_signal.assert_called_once_with(probe.signal.SIGTERM)
        child.kill.assert_called_once_with()
        self.assertEqual(child.wait.call_count, 2)

    def test_copy_mismatch_refuses_before_launch(self):
        with probe.tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / 'source'
            source.write_bytes(b'synthetic binary')
            with patch.object(probe, 'CORE', source), \
                 patch.object(probe.subprocess, 'Popen') as launch:
                with self.assertRaises(RuntimeError): probe.experiment(root, 'a' * 64)
                launch.assert_not_called()

    def test_unsafe_core_permissions_owner_or_symlink_refuse(self):
        for mode, uid in [(0o100777, 0), (0o100755, 1000), (0o120777, 0)]:
            core = Mock()
            core.lstat.return_value = Mock(st_mode=mode, st_uid=uid, st_size=1)
            with patch.object(probe, 'CORE', core):
                with self.assertRaises(RuntimeError): probe.core_digest()
                core.read_bytes.assert_not_called()


if __name__ == '__main__':
    unittest.main()
