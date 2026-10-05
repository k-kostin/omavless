"""Pure source/flow controls; never connect to a manager or run the ignored ELF."""
import importlib.util
import json
import hashlib
import os
from pathlib import Path
import tempfile
import tomllib
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, patch

ROOT = Path(__file__).resolve().parents[1]
SUPPORT = ROOT / 'crates/omavless-netguard/tests/support'
spec = importlib.util.spec_from_file_location('k1_version_reference_guard', SUPPORT / 'version_reference_guard.py')
guard = importlib.util.module_from_spec(spec)
spec.loader.exec_module(guard)
stage_spec = importlib.util.spec_from_file_location('k1_version_reference_stage', SUPPORT / 'version_reference_stage.py')
stage = importlib.util.module_from_spec(stage_spec)
stage_spec.loader.exec_module(stage)


def observed():
    return {'schema': 2, 'marker': guard.MARKER,
            'unit': {'Id': guard.UNIT, 'LoadState': 'loaded', 'FragmentPath': str(guard.LINK),
                     'ActiveState': 'inactive', 'SubState': 'dead', 'DropInPaths': [],
                     'Job': {'type': '(uo)', 'data': [0, '/']}},
            'service': {'StandardOutput': 'append', 'StandardError': 'append', 'Type': 'oneshot',
                        'User': 'root', 'Group': 'root', 'WatchdogUSec': 2**64 - 1,
                        'MainPID': 0, 'ControlPID': 0, 'ExecMainPID': 0,
                        'ExecMainStartTimestampMonotonic': 0, 'ControlGroup': ''},
            'dump': {'type': 's', 'data': ['SYNTHETIC_OPAQUE_CAPTURE_NOT_GRAMMAR']}}


ACK = {'schema': 2, 'unref_acknowledged': True, 'admission': False}


class Metadata(unittest.TestCase):
    def test_exact_version_capture_is_not_version_admission(self):
        def observed_version():
            return {'schema': 2, 'marker': 'OBSERVED_VERSION_DATA_NOT_ADMISSION',
                    'unique_owner': ':1.77', 'unit': guard.UNIT, 'admission': False,
                    'before_ref': {'type': 's', 'data': ['UNKNOWN-full-version-λ']},
                    'while_ref_after_dump': {'type': 's', 'data': ['UNKNOWN-full-version-λ']}}
        guard.validate_version(observed_version())
        for edit in [
                lambda v: v.update(schema=True), lambda v: v.update(unit='other.service'),
                lambda v: v.update(unique_owner='org.freedesktop.systemd1'),
                lambda v: v.update(unique_owner=':1.'), lambda v: v.update(admission=True),
                lambda v: v['before_ref'].update(type='v'),
                lambda v: v['before_ref'].update(data=['other']),
                lambda v: v['before_ref'].update(data=['a', 'b']),
                lambda v: v['before_ref'].update(data=[True]),
                lambda v: v['before_ref'].update(data=['']),
                lambda v: v['before_ref'].update(data=['x' * 257]),
                lambda v: v['before_ref'].update(data=['a\n']),
                lambda v: v['before_ref'].update(data=['a\u0085'])]:
            value = observed_version()
            edit(value)
            with self.assertRaises(RuntimeError):
                guard.validate_version(value)

    def test_post_state_is_required_typed_and_not_an_absence_fallback(self):
        def post():
            data = observed()
            return {'schema': 2, 'phase': 'post-unref-single-getall', 'unit': data['unit'],
                    'service': data['service'], 'admission': False}
        guard.validate_post_state(post())
        for edit in [lambda v: v.update(schema=True),
                     lambda v: v['unit'].update(LoadState='not-found'),
                     lambda v: v['unit']['Job'].update(data=[False, '/']),
                     lambda v: v['unit']['Job'].update(data=[1, '/']),
                     lambda v: v['service'].update(ControlGroup='/system.slice/other.service'),
                     lambda v: v['service'].update(ExecMainStartTimestampMonotonic=1),
                     lambda v: v.update(phase='before-unref')]:
            value = post()
            edit(value)
            with self.assertRaises(RuntimeError):
                guard.validate_post_state(value)
        obj = object.__new__(guard.Observer)
        obj.post_validated = False
        obj.call = Mock(side_effect=AssertionError('unexpected query'))
        with self.assertRaises(RuntimeError):
            obj.cleanup_known_success()
        obj.call.assert_not_called()

    def test_strict_capture_envelope_not_grammar(self):
        guard.validate_capture(observed(), ACK)
        for edit in [lambda v: v.update(schema=True),
                     lambda v: v['unit'].update(Id='other.service'),
                     lambda v: v['service'].update(MainPID=False),
                     lambda v: v['service'].update(WatchdogUSec=-1),
                     lambda v: v['service'].update(WatchdogUSec=2**64),
                     lambda v: v['dump'].update(data=['']),
                     lambda v: v['dump'].update(data=['x', 'x']),
                     lambda v: v['dump'].update(data=['x\0']),
                     lambda v: v['dump'].update(data=['x' * (1024 * 1024 + 1)])]:
            value = observed()
            edit(value)
            with self.assertRaises(RuntimeError):
                guard.validate_capture(value, ACK)
        for ack in [{}, {**ACK, 'schema': True}, {**ACK, 'unref_acknowledged': 1},
                    {**ACK, 'admission': 0}]:
            with self.assertRaises(RuntimeError):
                guard.validate_capture(observed(), ack)
        with self.assertRaises(RuntimeError):
            json.loads('{"schema":1,"schema":1}', object_pairs_hook=guard.pairs)

    def test_old_artifacts_unchanged_and_explicit_dependency_boundary(self):
        import hashlib
        self.assertEqual(hashlib.sha256((SUPPORT / 'manager_private_guard.py').read_bytes()).hexdigest(),
                         '472643e1b497bdc7cdb36ca9c3c380811d03143fdaaf5385018f3f082dbe063d')
        unit = (SUPPORT.parent / 'fixtures' / guard.UNIT).read_text()
        self.assertIn('ExecStart=/usr/bin/false\n', unit)
        self.assertIn('WatchdogSec=0\n', unit)
        self.assertNotIn('[Install]', unit)
        cargo = (ROOT / 'crates/omavless-netguard/Cargo.toml').read_text()
        self.assertIn('[dev-dependencies]\nzbus = "=5.19.0"', cargo)
        manifest = tomllib.loads(cargo)
        self.assertEqual(manifest['dependencies']['zbus'], {'version': '=5.19.0', 'optional': True})
        self.assertEqual(manifest['features'], {'netguard-service-core': ['dep:nix-netguard', 'dep:zbus']})
        self.assertEqual(manifest['bin'], [{'name': 'omavless-netguard', 'path': 'src/main.rs',
                                           'required-features': ['netguard-service-core']}])


class ActualObserverFlow(unittest.TestCase):
    def test_missing_or_malformed_version_cannot_validate_evidence(self):
        for missing in (True, False):
            obj = object.__new__(guard.Observer)
            obj.pins, obj.post_validated = [], False
            def pin(path, *args):
                if path.name == 'version-reference-manager-version.json':
                    if missing:
                        raise FileNotFoundError()
                    return SimpleNamespace(data=lambda: b'{}')
                return SimpleNamespace(data=lambda: b'{}')
            with patch.object(guard, 'Pin', side_effect=pin):
                with self.assertRaises((FileNotFoundError, RuntimeError)):
                    obj.evidence()
            self.assertFalse(obj.post_validated)
            self.assertEqual(obj.pins, [])

    def run_flow(self, failure=None):
        calls = []
        obj = object.__new__(guard.Observer)
        obj.directory_fd, obj.parent_fd, obj.sealed = 17, 18, False
        obj.pins = []

        def event(name, result=None):
            def call(*args, **kwargs):
                calls.append(name)
                if name == failure:
                    raise RuntimeError()
                return result
            return call

        snapshots = iter(['before', 'after'])

        def snapshot():
            return event(next(snapshots), {})()

        obj.ns = {'UNCERTAIN': False, 'snapshot': snapshot, 'preserve': lambda a, b: True}
        obj.recheck = event('recheck')
        obj.call = event('vm', b'kvm\n')
        obj.properties = event('notfound', {'LoadState': 'not-found'})
        obj.write = lambda name, value: event(name)()
        obj.helper = event('helper')
        obj.evidence = event('evidence')
        obj.cleanup_known_success = event('cleanup')
        meta = SimpleNamespace(st_dev=1, st_ino=2, st_uid=0, st_gid=0, st_mode=0o120777,
                               st_nlink=1, st_size=1, st_mtime_ns=1, st_ctime_ns=1)
        with patch.object(Path, 'exists', return_value=False), patch.object(Path, 'is_symlink', return_value=False), \
             patch.object(guard.os, 'listdir', return_value=['guard.py', 'query-guard.py', 'fixture.service', 'probe']), \
             patch.object(guard.os, 'symlink', side_effect=event('publish')), \
             patch.object(guard.os, 'stat', return_value=meta), patch.object(guard.os, 'fsync'):
            if failure:
                with self.assertRaises(RuntimeError):
                    obj.execute()
                self.assertTrue(obj.sealed)
                with self.assertRaises(RuntimeError):
                    obj.available()
            else:
                obj.execute()
        return calls

    def test_known_success_order(self):
        calls = self.run_flow()
        self.assertLess(calls.index('before'), calls.index('publish'))
        self.assertLess(calls.index('helper'), calls.index('evidence'))
        self.assertLess(calls.index('evidence'), calls.index('cleanup'))
        self.assertLess(calls.index('cleanup'), calls.index('after'))
        self.assertEqual(calls[-1], 'result.json')

    def test_every_refusal_stops_without_later_queries_or_cleanup(self):
        for phase in ('before', 'baseline-before.json', 'publish', 'helper', 'evidence', 'cleanup', 'after'):
            calls = self.run_flow(phase)
            self.assertEqual(calls[-1], phase)
            self.assertNotIn('result.json', calls)
            if phase in ('helper', 'evidence'):
                self.assertNotIn('cleanup', calls)
                self.assertNotIn('after', calls)

    def test_helper_uses_original_fd_and_raw_owned_wait_nonzero_refuses(self):
        obj = object.__new__(guard.Observer)
        child = object()
        spawn, wait = Mock(return_value=child), Mock(return_value=7)
        obj.ns = {'UNCERTAIN': False, 'spawn': spawn, 'await_child': wait}
        obj.sealed, obj.directory_fd = False, 17
        obj.pins = [None, None, SimpleNamespace(fd=25)]
        with patch.object(guard.os, 'open', side_effect=[30, 31]), patch.object(guard.os, 'close'):
            with self.assertRaises(RuntimeError):
                obj.helper()
        self.assertEqual(spawn.call_count, 1)
        kwargs = spawn.call_args.kwargs
        self.assertEqual(kwargs['executable'], '/proc/self/fd/25')
        self.assertEqual(kwargs['pass_fds'], (25,))
        self.assertEqual(kwargs['extra_groups'], [])
        self.assertEqual(set(kwargs['env']), {'PATH', 'LC_ALL', 'OMAVLESS_K1_VERSION_REFERENCE'})
        wait.assert_called_once_with(child, seconds=45)

    def test_cleanup_has_no_manager_query_before_exact_own_unlink(self):
        for fail_absence in (False, True):
            trace = []
            obj = object.__new__(guard.Observer)
            obj.post_validated, obj.parent_fd = True, 18
            obj.pins = [None, None, object()]
            obj.recheck = lambda **kwargs: trace.append('pin-check')
            obj.call = lambda argv: trace.append(tuple(argv))
            obj.properties = lambda names: (trace.append('not-found') or {'LoadState': 'not-found'})

            def absence(probe):
                trace.append('inode-absence')
                if fail_absence:
                    raise RuntimeError()

            with patch.object(Path, 'exists', return_value=False), \
                 patch.object(Path, 'is_symlink', return_value=False), \
                 patch.object(guard, 'exact_inode_absent', side_effect=absence), \
                 patch.object(guard.os, 'unlink', side_effect=lambda *a, **k: trace.append('unlink')), \
                 patch.object(guard.os, 'fsync'):
                if fail_absence:
                    with self.assertRaises(RuntimeError):
                        obj.cleanup_known_success()
                    self.assertEqual(trace, ['pin-check', 'inode-absence'])
                else:
                    obj.cleanup_known_success()
                    self.assertEqual(trace, ['pin-check', 'inode-absence', 'pin-check', 'unlink',
                                            ('/usr/bin/systemctl', 'daemon-reload'), 'not-found'])


class ExactInode(unittest.TestCase):
    def test_matching_executable_and_unstable_epoch_refuse(self):
        path = Path('/proc/77')
        metadata = ('77', 'S', 0, 123, (0, 0, 0, 0))
        probe = SimpleNamespace(meta=SimpleNamespace(st_dev=26, st_ino=4070))
        with patch.object(Path, 'iterdir', return_value=iter([path])), \
             patch.object(guard, 'process_metadata', return_value=metadata), \
             patch.object(guard.os, 'open', return_value=45), patch.object(guard.os, 'close'), \
             patch.object(guard.os, 'fstat', return_value=probe.meta):
            with self.assertRaises(RuntimeError):
                guard.exact_inode_absent(probe)

    def test_noexe_only_stable_zombie_or_root_kernel_thread(self):
        path = Path('/proc/77')
        probe = SimpleNamespace(meta=SimpleNamespace(st_dev=26, st_ino=4070))
        for state, flags, uids, permitted in [('Z', 0, (1000,) * 4, True),
                ('S', 0x00200000, (0,) * 4, True), ('S', 0, (0,) * 4, False),
                ('S', 0x00200000, (1000,) * 4, False)]:
            metadata = ('77', state, flags, 123, uids)
            with patch.object(Path, 'iterdir', return_value=iter([path])), \
                 patch.object(guard, 'process_metadata', return_value=metadata), \
                 patch.object(guard.os, 'open', side_effect=FileNotFoundError):
                if permitted:
                    guard.exact_inode_absent(probe)
                else:
                    with self.assertRaises(RuntimeError):
                        guard.exact_inode_absent(probe)


class Loader(unittest.TestCase):
    def test_acyclic_exact_source_pins(self):
        for name, path in {'guard.py': SUPPORT / 'version_reference_guard.py',
                           'query-guard.py': SUPPORT / 'generator_filter_guest_guard.py',
                           'fixture.service': SUPPORT.parent / 'fixtures' / guard.UNIT}.items():
            self.assertEqual(stage.MEMBERS[name][0], hashlib.sha256(path.read_bytes()).hexdigest())
        self.assertEqual(stage.MEMBERS['probe'][0], guard.PROBE_SHA)
        self.assertEqual(stage.DESTINATION, guard.STAGE)
        self.assertEqual(stage.MEMBERS['probe'][3], 128 * 1024 * 1024)

    @unittest.skipUnless(os.getuid() == os.getgid() == 1000, 'actual ordinary source-owner FD control')
    def test_retained_original_fd_rejects_hash_mode_size_hardlink_symlink(self):
        with tempfile.TemporaryDirectory(prefix='k1-ref-loader-') as directory:
            root = Path(directory)
            source = root / 'source'
            source.write_bytes(b'pinned')
            source.chmod(0o400)
            fd = os.open(root, os.O_RDONLY | os.O_DIRECTORY)
            expected = hashlib.sha256(b'pinned').hexdigest()
            try:
                self.assertEqual(stage.retained(fd, 'source', expected, 0o400, 16), b'pinned')
                for sha, mode, bound in [('0' * 64, 0o400, 16), (expected, 0o500, 16),
                                          (expected, 0o400, 2)]:
                    with self.assertRaises(RuntimeError):
                        stage.retained(fd, 'source', sha, mode, bound)
                os.link(source, root / 'hard')
                with self.assertRaises(RuntimeError):
                    stage.retained(fd, 'source', expected, 0o400, 16)
                (root / 'soft').symlink_to(source)
                with self.assertRaises(OSError):
                    stage.retained(fd, 'soft', expected, 0o400, 16)
            finally:
                os.close(fd)


if __name__ == '__main__':
    unittest.main()
