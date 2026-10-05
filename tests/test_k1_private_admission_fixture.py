"""Pure phase/known-child controls. No guest or manager connection."""
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, patch

ROOT = Path(__file__).resolve().parents[1]
SUPPORT = ROOT / 'crates/omavless-netguard/tests/support'


def load(name):
    spec = importlib.util.spec_from_file_location(name, SUPPORT / (name + '.py'))
    value = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(value)
    return value


guard = load('private_admission_guard')
stage = load('private_admission_stage')
query = load('private_admission_guest_guard')


class Flow(unittest.TestCase):
    def test_waitable_nonzero_signal_malformed_and_unknown_never_reap_or_query_again(self):
        observations = [
            SimpleNamespace(si_pid=913, si_code=os.CLD_EXITED, si_status=7),
            SimpleNamespace(si_pid=913, si_code=os.CLD_KILLED, si_status=9),
            SimpleNamespace(si_pid=913, si_code=os.CLD_DUMPED, si_status=11),
            SimpleNamespace(si_pid=914, si_code=os.CLD_EXITED, si_status=0),
            SimpleNamespace(si_pid=913, si_code=True, si_status=0),
            SimpleNamespace(si_pid=913, si_code=os.CLD_EXITED, si_status=False),
            SimpleNamespace(si_pid=913, si_code=os.CLD_EXITED),
            ChildProcessError(), OSError(),
        ]
        for seen in observations:
            child = SimpleNamespace(pid=913, returncode=None)
            with patch.object(query, 'UNCERTAIN', False), patch.object(query, 'RETAINED', []), \
                 patch.object(query.os, 'waitid', side_effect=seen if isinstance(seen, BaseException) else None,
                              return_value=seen) as waitid, \
                 patch.object(query.os, 'waitpid') as reap, \
                 patch.object(query, 'OwnedProcess') as spawn, \
                 patch.object(query.tempfile, 'TemporaryFile') as output:
                with self.assertRaises(query.Refused):
                    query.await_child(child, seconds=1)
                self.assertTrue(query.UNCERTAIN)
                self.assertEqual(query.RETAINED, [child])
                self.assertIsNone(child.returncode)
                for action in (lambda: query.await_child(child, 1),
                               lambda: query.await_child_once(child, 1),
                               lambda: query.command(['/usr/bin/true']),
                               lambda: query.spawn(['/usr/bin/true'])):
                    with self.assertRaises(query.Refused):
                        action()
                waitid.assert_called_once()
                reap.assert_not_called(); spawn.assert_not_called(); output.assert_not_called()

    def test_only_typed_waitable_zero_allows_one_exact_zero_reap(self):
        for result in [(913, 0), (913, False), (913, 7), (0, 0), None]:
            child = SimpleNamespace(pid=913, returncode=None)
            seen = SimpleNamespace(si_pid=913, si_code=os.CLD_EXITED, si_status=0)
            with patch.object(query, 'UNCERTAIN', False), patch.object(query, 'RETAINED', []), \
                 patch.object(query.os, 'waitid', return_value=seen), \
                 patch.object(query.os, 'waitpid', return_value=result) as reap:
                if result == (913, 0) and type(result[1]) is int:
                    self.assertEqual(query.await_child(child, 1), 0)
                    self.assertFalse(query.UNCERTAIN)
                    self.assertEqual(child.returncode, 0)
                else:
                    with self.assertRaises(query.Refused):
                        query.await_child(child, 1)
                    self.assertTrue(query.UNCERTAIN)
                reap.assert_called_once_with(913, os.WNOHANG)

    def test_wait_deadline_preserves_child_without_reap(self):
        for clock in ([0, 2], [0, 0, 2]):
            child = SimpleNamespace(pid=913, returncode=None)
            with patch.object(query, 'UNCERTAIN', False), patch.object(query, 'RETAINED', []), \
                 patch.object(query.time, 'monotonic', side_effect=clock), patch.object(query.time, 'sleep'), \
                 patch.object(query.os, 'waitid', return_value=None) as waitid, \
                 patch.object(query.os, 'waitpid') as reap:
                with self.assertRaises(query.Refused):
                    query.await_child(child, 1)
                self.assertTrue(query.UNCERTAIN)
                self.assertEqual(waitid.call_count, len(clock) - 2)
                reap.assert_not_called()

    def test_actual_command_nonzero_never_reads_output_or_reaps(self):
        child = SimpleNamespace(pid=913, returncode=None)
        seen = SimpleNamespace(si_pid=913, si_code=os.CLD_EXITED, si_status=7)
        with patch.object(query, 'UNCERTAIN', False), patch.object(query, 'RETAINED', []), \
             patch.object(query, 'OwnedProcess', return_value=child), \
             patch.object(query.tempfile, 'TemporaryFile') as files, \
             patch.object(query.os, 'waitid', return_value=seen), patch.object(query.os, 'waitpid') as reap:
            with self.assertRaises(query.Refused):
                query.command(['/usr/bin/true'])
            self.assertTrue(query.UNCERTAIN)
            output = files.return_value.__enter__.return_value
            output.tell.assert_not_called(); output.seek.assert_not_called(); output.read.assert_not_called()
            reap.assert_not_called()

    def test_loader_real_retained_fd_refuses_hash_mode_bounds_and_aliases(self):
        if os.getuid() != 1000 or os.getgid() != 1000:
            self.skipTest('ordinary uid1000 source-file control only')
        with tempfile.TemporaryDirectory() as name:
            directory = Path(name)
            artifact = directory / 'probe'
            artifact.write_bytes(b'SYNTHETIC')
            artifact.chmod(0o500)
            digest = hashlib.sha256(b'SYNTHETIC').hexdigest()
            fd = os.open(directory, os.O_RDONLY | os.O_DIRECTORY)
            try:
                self.assertEqual(stage.retained(fd, 'probe', digest, 0o500, 9), b'SYNTHETIC')
                for sha, mode, maximum in [('0'*64, 0o500, 9), (digest, 0o400, 9), (digest, 0o500, 8)]:
                    with self.assertRaises(RuntimeError):
                        stage.retained(fd, 'probe', sha, mode, maximum)
                (directory / 'alias').symlink_to(artifact)
                with self.assertRaises(OSError):
                    stage.retained(fd, 'alias', digest, 0o500, 9)
                os.link(artifact, directory / 'hardlink')
                with self.assertRaises(RuntimeError):
                    stage.retained(fd, 'probe', digest, 0o500, 9)
            finally:
                os.close(fd)

    def test_typed_permission_receipt_strict_values_types_and_scope(self):
        expected = guard.expected_permissions()
        guard.validate_permissions(expected)
        expected['unit']['Requires']['value'].reverse()
        guard.validate_permissions(expected)
        for group, rows in guard.expected_permissions().items():
            for field in rows:
                value = guard.expected_permissions()
                del value[group][field]
                with self.assertRaises(RuntimeError):
                    guard.validate_permissions(value)
                value = guard.expected_permissions()
                value[group][field]['signature'] = 'v'
                with self.assertRaises(RuntimeError):
                    guard.validate_permissions(value)
        for key, wrong in [('PrivateNetwork', False), ('NoNewPrivileges', 1),
                           ('WatchdogUSec', 0), ('CapabilityBoundingSet', 0x201000),
                           ('ExecStartPre', [['/usr/bin/true']]), ('EnvironmentFiles', [['/secret', False]])]:
            value = guard.expected_permissions()
            value['service'][key]['value'] = wrong
            with self.assertRaises(RuntimeError):
                guard.validate_permissions(value)

    def test_old_writer_is_only_rebound_not_reimplemented_and_no_start_dispatch(self):
        src = ROOT / 'crates/omavless-netguard/src'
        old = (src / 'kernel_manager_private_fixture.rs').read_text()
        new = (src / 'kernel_private_admission_fixture.rs').read_text()
        expected = old.replace('omavless-k1-manager-private-lifecycle',
            'omavless-k1-private-lifecycle-admission').replace('::manager_private::',
            '::private_admission::').replace('OMAVLESS_K1_MANAGER_PRIVATE',
            'OMAVLESS_K1_PRIVATE_ADMISSION_WRITER')
        self.assertEqual(new, expected.replace('const TEST: &str = "', 'const TEST: &str =\n    "'))
        outer = (SUPPORT / 'private_admission_guard.py').read_text()
        native = (src / 'manager_lifecycle_admission_fixture.rs').read_text()
        for forbidden in ('StartUnit', 'StopUnit', 'SetProperties', 'ReloadUnit'):
            self.assertNotIn(forbidden, native)
        self.assertNotIn("'start'", outer)
        self.assertNotIn("'stop'", outer)
        self.assertIn("manager_lifecycle_admission_fixture::capture_effective_config", outer)

    def fixture(self):
        obj = object.__new__(guard.Observer)
        obj.directory_fd, obj.parent_fd = 17, 18
        obj.sealed, obj.post_validated, obj.helper_known_zero = False, False, False
        obj.phase, obj.pins = 'not-entered', []
        obj.ns = {'UNCERTAIN': False, 'ACTIVATION_REFUSED': False}
        return obj

    def flow(self, failure=None, uncertain=False):
        obj = self.fixture()
        trace = []

        def event(name, result=None):
            trace.append(name)
            if name == failure:
                obj.ns['UNCERTAIN'] = uncertain
                raise RuntimeError('PRIVATE_ERROR_NEVER_PUBLISHED')
            return result

        snapshots = iter(('before', 'after'))
        obj.ns.update(snapshot=lambda: event(next(snapshots), {}),
                      preserve=lambda a, b: event('preserve', True))
        obj.recheck = lambda **kw: event('pins')
        obj.call = lambda argv: event('vm', b'kvm\n')
        obj.properties = lambda names: event('notfound', {'LoadState': 'not-found'})
        obj.persist = lambda name, value: event((name, value))
        obj.write = lambda name, value: event(name)

        def helper():
            event('helper')
            obj.helper_known_zero = True

        def evidence():
            event('evidence')
            obj.post_validated = True

        def cleanup():
            for phase in guard.PHASES[5:9]:
                obj.before(phase)
                event(phase)

        obj.helper, obj.evidence, obj.cleanup_known_success = helper, evidence, cleanup
        meta = SimpleNamespace(st_dev=1, st_ino=2, st_uid=0, st_gid=0,
            st_mode=0o120777, st_nlink=1, st_size=1, st_mtime_ns=1, st_ctime_ns=1)
        with patch.object(Path, 'exists', return_value=False), \
             patch.object(Path, 'is_symlink', return_value=False), \
             patch.object(guard.os, 'listdir', return_value=['guard.py', 'query-guard.py', 'fixture.service', 'probe']), \
             patch.object(guard.os, 'symlink', side_effect=lambda *a, **k: event('publish')), \
             patch.object(guard.os, 'stat', return_value=meta), patch.object(guard.os, 'fsync'):
            if failure:
                with self.assertRaises(RuntimeError):
                    obj.execute()
            else:
                obj.execute()
        return obj, trace

    def test_complete_order_has_all_before_receipts_and_one_result(self):
        obj, trace = self.flow()
        phases = [x for x in trace if isinstance(x, str) and x.startswith('phase-')]
        self.assertEqual(phases, [f'phase-{i:02d}-{p}.json' for i, p in enumerate(guard.PHASES)])
        self.assertEqual(trace[-1], 'result.json')
        self.assertTrue(obj.helper_known_zero and obj.post_validated)
        self.assertLess(trace.index('before'), trace.index('publish'))
        self.assertLess(trace.index('helper'), trace.index('evidence'))

    def test_all_failures_have_only_fixed_refusal_after_first_error(self):
        for failure in ['vm', 'before', 'baseline-before.json', 'publish', 'helper', 'evidence',
                        *guard.PHASES[5:9], 'after', 'preserve', 'result.json',
                        *[f'phase-{i:02d}-{p}.json' for i, p in enumerate(guard.PHASES)]]:
            for uncertain in (False, True):
                obj, trace = self.flow(failure, uncertain)
                index = trace.index(failure)
                self.assertEqual(len(trace[index + 1:]), 1)
                name, receipt = trace[-1]
                self.assertEqual(name, 'refusal.json')
                self.assertEqual(receipt['phase'], obj.phase)
                self.assertEqual(receipt['reason'], 'BOUNDARY_REFUSED_NO_RETRY')
                self.assertFalse(receipt['cleanup_authorized'])
                self.assertNotIn('PRIVATE_ERROR', json.dumps(receipt))
                with self.assertRaises(RuntimeError):
                    obj.available()

    def test_phase_reorder_and_duplicate_refuse_without_write(self):
        obj = self.fixture()
        obj.write = Mock()
        with self.assertRaises(RuntimeError):
            obj.before('native-helper')
        obj.write.assert_not_called()
        obj.before('preflight')
        with self.assertRaises(RuntimeError):
            obj.before('preflight')
        self.assertEqual(obj.write.call_count, 1)

    def test_direct_helper_unknown_nonzero_boolean_cannot_enable_cleanup(self):
        for code in (7, None, False):
            obj = self.fixture()
            obj.pins = [None, None, SimpleNamespace(fd=25)]
            child = object()
            obj.ns.update(spawn=Mock(return_value=child), await_child=Mock(return_value=code))
            with patch.object(guard.os, 'open', side_effect=[30, 31]), patch.object(guard.os, 'close'):
                with self.assertRaises(RuntimeError):
                    obj.helper()
            self.assertFalse(obj.helper_known_zero)
            self.assertEqual(obj.ns['spawn'].call_args.kwargs['executable'], '/proc/self/fd/25')
            obj.ns['await_child'].assert_called_once_with(child, seconds=45)

    def test_cleanup_requires_both_known_child_and_receipts_before_unlink(self):
        for known, evidence in ((False, True), (True, False), (False, False)):
            obj = self.fixture()
            obj.helper_known_zero, obj.post_validated = known, evidence
            obj.phase = 'validate-evidence'
            obj.write = Mock()
            obj.recheck = Mock()
            with patch.object(guard.os, 'unlink') as unlink, self.assertRaises(RuntimeError):
                obj.cleanup_known_success()
            obj.recheck.assert_not_called()
            unlink.assert_not_called()

    def test_no_global_process_scan_and_no_old_fixture_change(self):
        source = (SUPPORT / 'private_admission_guard.py').read_text()
        self.assertNotIn("Path('/proc')", source)
        self.assertNotIn('exact_inode_absent', source)
        self.assertEqual(hashlib.sha256((SUPPORT / 'version_reference_guard.py').read_bytes()).hexdigest(),
                         'e35cfe4d7aac64d71158357806dfbcf09135ba57fa8a6a8b4986e9b2c2e1f9e7')

    def test_actual_cleanup_checks_pinned_unit_and_cgroup_before_exact_unlink(self):
        for cgroup in (False, True):
            obj = self.fixture()
            obj.helper_known_zero = obj.post_validated = True
            obj.phase = 'validate-evidence'
            trace = []
            obj.write = lambda name, value: trace.append(name)
            obj.recheck = lambda **kw: trace.append(('retained-pins', kw))
            obj.call = lambda argv: trace.append(tuple(argv))
            obj.properties = lambda names: {'LoadState': 'not-found'}
            with patch.object(Path, 'exists', return_value=cgroup), \
                 patch.object(Path, 'is_symlink', return_value=False), \
                 patch.object(guard.os, 'unlink') as unlink, patch.object(guard.os, 'fsync'):
                if cgroup:
                    with self.assertRaises(RuntimeError):
                        obj.cleanup_known_success()
                    unlink.assert_not_called()
                else:
                    obj.cleanup_known_success()
                    unlink.assert_called_once_with(guard.UNIT, dir_fd=18)
                    self.assertEqual(sum(x == ('retained-pins', {'linked': True}) for x in trace), 2)
                    self.assertIn(('/usr/bin/systemctl', 'daemon-reload'), trace)

    def test_exact_version_gate_and_phase_specific_watchdog(self):
        value = {'schema': 2, 'marker': 'OBSERVED_VERSION_DATA_NOT_ADMISSION',
                 'unique_owner': ':1.77', 'unit': guard.UNIT, 'admission': False,
                 'before_ref': {'type': 's', 'data': ['261.2-1-arch']},
                 'while_ref_after_dump': {'type': 's', 'data': ['261.2-1-arch']}}
        guard.validate_version(value)
        for wrong in ('261.2', '261.2-1-arch ', '261.2-1-arch-extra', '261.2-1-arch\0'):
            value['before_ref']['data'] = value['while_ref_after_dump']['data'] = [wrong]
            with self.assertRaises(RuntimeError):
                guard.validate_version(value)
        unit = {'Id': guard.UNIT, 'LoadState': 'loaded', 'FragmentPath': str(guard.LINK),
                'ActiveState': 'inactive', 'SubState': 'dead', 'DropInPaths': [],
                'Job': {'type': '(uo)', 'data': [0, '/']}}
        service = dict(StandardOutput='append', StandardError='append', Type='oneshot',
            User='root', Group='root', WatchdogUSec=2**64-1, MainPID=0, ControlPID=0,
            ExecMainPID=0, ExecMainStartTimestampMonotonic=0, ControlGroup='')
        guard.validate_state(unit, service)
        for wrong in (0, False, -1, 2**64):
            service['WatchdogUSec'] = wrong
            with self.assertRaises(RuntimeError):
                guard.validate_state(unit, service)

    def test_pins_are_acyclic_and_distinct(self):
        for name, path in {'guard.py': SUPPORT / 'private_admission_guard.py',
                          'query-guard.py': SUPPORT / 'private_admission_guest_guard.py',
                          'fixture.service': SUPPORT.parent / 'fixtures' / guard.UNIT}.items():
            self.assertEqual(stage.MEMBERS[name][0], hashlib.sha256(path.read_bytes()).hexdigest())
        self.assertEqual(stage.MEMBERS['probe'][0], guard.PROBE_SHA)
        self.assertEqual(stage.DESTINATION, guard.STAGE)


if __name__ == '__main__':
    unittest.main()
