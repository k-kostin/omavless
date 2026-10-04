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


guard = load('response_diagnostic_guard')
stage = load('response_diagnostic_stage')
query = load('response_diagnostic_guest_guard')


class Flow(unittest.TestCase):
    def test_negative_witness_is_exact_non_authorizing_and_bound_to_original_stage(self):
        value = {'schema': 1, 'fixture_unit': guard.UNIT, 'stage_device': 2, 'stage_inode': 3,
                 'namespace_device': 5, 'namespace_inode': 9,
                 'negative_witness_only': True, 'canonical_authority': False}
        self.assertEqual(guard.validate_negative_witness(value, (2, 3)), (5, 9))
        for key in value:
            missing = dict(value); del missing[key]
            with self.assertRaises(RuntimeError): guard.validate_negative_witness(missing, (2, 3))
            wrong = dict(value); wrong[key] = []
            with self.assertRaises(RuntimeError): guard.validate_negative_witness(wrong, (2, 3))
        for key, bad in [('namespace_inode', 0), ('namespace_device', True), ('namespace_inode', 2**64),
                         ('canonical_authority', True), ('negative_witness_only', False),
                         ('fixture_unit', 'other.service'), ('stage_inode', 4), ('schema', True),
                         ('extra', 0)]:
            wrong = dict(value); wrong[key] = bad
            with self.assertRaises(RuntimeError): guard.validate_negative_witness(wrong, (2, 3))

    def test_retained_615_pins_are_the_original_four_not_current_candidate(self):
        self.assertEqual(set(query.FAILED_PINS),{'guard.py','query-guard.py','fixture.service','probe'})
        for name, path in {
            'guard.py': SUPPORT/'private_admission_guard.py',
            'query-guard.py': SUPPORT/'private_admission_guest_guard.py',
            'fixture.service': SUPPORT.parent/'fixtures'/'omavless-k1-private-lifecycle-admission.service',
        }.items():
            self.assertEqual(query.FAILED_PINS[name][2],hashlib.sha256(path.read_bytes()).hexdigest())
        self.assertEqual(query.FAILED_PINS['probe'][2],'803f7959df873660d262da02b19a7676cb00f36ef16ec0709120ff03582ad242')
        self.assertNotEqual(query.FAILED_PINS['probe'][2],guard.PROBE_SHA)
        self.assertNotIn(str(query.FAILED_STAGE),query.TARGET_ROOTS)

    def test_retained_exact_files_no_query_and_same_byte_swaps_refuse(self):
        # Real FDs/inodes; only ownership is normalized for ordinary CI UID.
        original_fstat, original_stat = os.fstat, os.stat
        original_lstat = Path.lstat
        def root_meta(info):
            return SimpleNamespace(**{name: (0 if name in ('st_uid','st_gid') else getattr(info,name))
                for name in ('st_dev','st_ino','st_mode','st_uid','st_gid','st_nlink','st_size','st_mtime_ns','st_ctime_ns')})
        for generation, swap in ((g, s) for g in (615,621)
                                 for s in ('none','link','fragment','source','metadata','retarget','hash')):
            with tempfile.TemporaryDirectory(dir=Path.home()) as temp:
                root = Path(temp); stage_dir = root / 'stage'; stage_dir.mkdir(mode=0o700)
                links = root / 'links'; links.mkdir(mode=0o700)
                link = links / 'fixture.service'; link.symlink_to(stage_dir / 'fixture.service')
                pins = {}
                for name in query.FAILED_PINS:
                    raw = ('synthetic-'+name).encode(); file = stage_dir / name
                    file.write_bytes(raw); file.chmod(0o600)
                    pins[name] = (0o600,4096,hashlib.sha256(raw).hexdigest())
                held = None
                prefix = 'FAILED' if generation == 615 else 'RESPONSE'
                with patch.object(query,prefix+'_STAGE',stage_dir), patch.object(query,prefix+'_LINK',link), \
                     patch.object(query,prefix+'_PINS',pins), patch.object(query,'command',side_effect=AssertionError('query')) as command, \
                     patch.object(query.os,'fstat',side_effect=lambda fd:root_meta(original_fstat(fd))), \
                     patch.object(query.os,'stat',side_effect=lambda *a,**kw:root_meta(original_stat(*a,**kw))), \
                     patch.object(Path,'lstat',lambda p:root_meta(original_lstat(p))):
                    held = query.RetainedFailedActivation(generation)
                    self.assertTrue(held.admits(link,stage_dir/'fixture.service'))
                    self.assertFalse(held.admits(links/'other.service',stage_dir/'fixture.service'))
                    self.assertFalse(held.admits(link,stage_dir/'guard.py'))
                    if swap == 'link':
                        link.unlink(); link.symlink_to(stage_dir/'fixture.service')
                    elif swap in ('fragment','source'):
                        file=stage_dir/('fixture.service' if swap=='fragment' else 'guard.py')
                        raw=file.read_bytes(); file.unlink(); file.write_bytes(raw);file.chmod(0o600)
                    elif swap == 'metadata':
                        (stage_dir/'probe').chmod(0o400)
                    elif swap == 'retarget':
                        link.unlink(); link.symlink_to(stage_dir/'guard.py')
                    elif swap == 'hash':
                        fd, info, _ = held.files['probe']
                        held.files['probe'] = (fd, info, '0'*64)
                    if swap == 'none':
                        held.recheck()
                        self.assertEqual(held.file_record(stage_dir/'fixture.service',root_meta(original_stat(stage_dir/'fixture.service'))),
                                         ['file',pins['fixture.service'][2]])
                    else:
                        with self.assertRaises(query.Refused): held.recheck()
                        self.assertTrue(held.sealed)
                        with patch.object(query.os,'pread',side_effect=AssertionError('late read')) as read:
                            with self.assertRaises(query.Refused): held.recheck()
                            read.assert_not_called()
                    command.assert_not_called()
                # No process/manager owns these synthetic local files.
                if held is not None:
                    for _,fd,_ in held.parents: os.close(fd)
                    os.close(held.link_fd)
                    for fd,_,_ in held.files.values(): os.close(fd)

    def test_retained_621_fixed_pins_and_generation_are_not_dispatch(self):
        self.assertEqual(query.RESPONSE_PINS['fixture.service'][2],
                         'a3b03103bbd8c43f6e6ca6755c063a7e851f40a006303c93028be80aec411e58')
        self.assertEqual(query.RESPONSE_PINS['probe'][2],
                         'a2b8dd3b7cc1658c536fd81bd25a74ae55255cb2159624cfe8f548c162ad0e9a')
        self.assertNotIn(str(query.RESPONSE_STAGE),query.TARGET_ROOTS)
        for unknown in (True, '621', 622, None, '/run/other'):
            with patch.object(query.os,'open') as opened:
                with self.assertRaises(query.Refused): query.RetainedFailedActivation(unknown)
                opened.assert_not_called()
        with patch.object(query,'ACTIVATION_REFUSED',False), patch.object(query,'RESPONSE_ACTIVATION',None), \
             patch.object(query,'RetainedFailedActivation',side_effect=query.Refused) as constructor:
            for _ in range(2):
                with self.assertRaises(query.Refused): query.retained_response_activation()
            constructor.assert_called_once_with(621)
            with self.assertRaises(query.Refused): query.retained_failed_activation()

    def test_supported_socket_alias_fields_required_before_and_after(self):
        self.assertNotIn('Sockets',guard.expected_permissions()['service'])
        for field, wrong in [('Names',[guard.UNIT,'alias.service']),
                             ('Names',[guard.UNIT,guard.UNIT]),
                             ('Following','other.service'),
                             ('TriggeredBy',['synthetic.socket']),('Wants',['synthetic.socket'])]:
            for missing in (False,True):
                value=guard.expected_permissions()
                if missing: del value['unit'][field]
                else: value['unit'][field]['value']=wrong
                with self.assertRaises(RuntimeError): guard.validate_permissions(value)

    def test_failed_activation_refusal_latches_before_second_constructor(self):
        with patch.object(query,'ACTIVATION_REFUSED',False), patch.object(query,'FAILED_ACTIVATION',None), \
             patch.object(query,'RetainedFailedActivation',side_effect=query.Refused) as constructor:
            for _ in range(2):
                with self.assertRaises(query.Refused): query.retained_failed_activation()
            constructor.assert_called_once()

    def test_inventory_allows_only_exact_retained_615_pair_without_prefix_exception(self):
        with tempfile.TemporaryDirectory(dir=Path.home()) as temp:
            root=Path(temp); links=root/'links'; links.mkdir()
            old_stage=root/'old609'; old_stage.mkdir(); new_stage=root/'old615'; new_stage.mkdir()
            old_fragment=old_stage/'fixture.service'; old_fragment.write_bytes(b'old609')
            new_fragment=new_stage/'fixture.service'; new_fragment.write_bytes(b'old615')
            old_link=links/'old609.service'; old_link.symlink_to(old_fragment)
            new_link=links/'old615.service'; new_link.symlink_to(new_fragment)
            response_stage=root/'old621'; response_stage.mkdir()
            response_fragment=response_stage/'fixture.service'; response_fragment.write_bytes(b'old621')
            response_link=links/'old621.service'; response_link.symlink_to(response_fragment)
            unrelated=new_stage/'other.service'; unrelated.write_bytes(b'old615')
            old=SimpleNamespace(fragment=old_fragment.stat(),recheck=Mock(return_value=b'old609'),
                admits=lambda link,target:link==old_link and target==old_fragment)
            failed=SimpleNamespace(recheck=Mock(),admits=lambda link,target:link==new_link and target==new_fragment,
                file_record=lambda path,info:['file',hashlib.sha256(b'old615').hexdigest()])
            response=SimpleNamespace(recheck=Mock(),admits=lambda link,target:link==response_link and target==response_fragment,
                file_record=lambda path,info:['file',hashlib.sha256(b'old621').hexdigest()])
            with patch.object(query,'ACTIVATION_ROOTS',(str(links),)), patch.object(query,'TARGET_ROOTS',()), \
                 patch.object(query,'RETAINED_LINK',old_link),patch.object(query,'RETAINED_FRAGMENT',old_fragment), \
                 patch.object(query,'FAILED_LINK',new_link),patch.object(query,'FAILED_STAGE',new_stage), \
                 patch.object(query,'retained_activation',return_value=old), \
                 patch.object(query,'retained_failed_activation',return_value=failed), \
                 patch.object(query,'RESPONSE_LINK',response_link),patch.object(query,'RESPONSE_STAGE',response_stage), \
                 patch.object(query,'retained_response_activation',return_value=response), \
                 patch.object(query,'command',side_effect=AssertionError('query')) as command:
                records=query._inventory()
                self.assertIn(str(new_fragment),records)
                self.assertIn(str(response_fragment),records)
                self.assertNotIn(str(unrelated),records)
                failed.recheck.assert_called()
                # Same stage directory is not a general target exception.
                (links/'unrelated.service').symlink_to(unrelated)
                with patch.object(query,'ACTIVATION_REFUSED',False):
                    with self.assertRaises(query.Refused):query.inventory()
                    self.assertTrue(query.ACTIVATION_REFUSED)
                    with patch.object(query,'_inventory',side_effect=AssertionError('late inventory')) as again:
                        with self.assertRaises(query.Refused):query.inventory()
                        again.assert_not_called()
                command.assert_not_called()

    def test_complete_rpc_receipts_are_diagnostic_and_exact_typed(self):
        before = {'schema': 1, 'diagnostic': True, 'boundary': 'before-rpc', 'admission': False}
        version = {'unique_owner': ':1.77'}
        data = {'dump': {'data': ['synthetic-private-text']}}
        def selected(side):
            fields = guard.expected_permissions()[side]
            if side == 'unit': fields['Job'] = {'signature': '(uo)', 'value': [0, '/']}
            return {'selected': {key: {'present': True, 'variant': value} for key,value in fields.items()},
                    'mismatch_fields': []}
        facts = [{'owner': ':1.77'}, {'version': '261.2-1-arch'}, {'empty_ack': True},
                 selected('unit'), selected('service'), {'dump': 'synthetic-private-text', 'mismatch_fields': []},
                 {'version': '261.2-1-arch'}, {'empty_ack': True},
                 {'unit': selected('unit'), 'service': selected('service')}]
        for index, row in enumerate(facts):
            value = {'schema': 1, 'diagnostic': True, 'admission': False, 'validation_passed': True, 'facts': row}
            guard.validate_rpc(index,before,value,version,data)
            for key, wrong in [('schema', True), ('admission', 0), ('diagnostic', 1), ('validation_passed', False)]:
                changed = dict(value); changed[key] = wrong
                with self.subTest(index=index,key=key), self.assertRaises(RuntimeError):
                    guard.validate_rpc(index,before,changed,version,data)
        value = {'schema': 1, 'diagnostic': True, 'admission': False, 'validation_passed': True, 'facts': selected('service')}
        value['facts']['selected']['PrivateNetwork']['variant']['value'] = 1
        with self.assertRaises(RuntimeError): guard.validate_rpc(4,before,value,version,data)

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

    def test_writer_provenance_delta_and_start_only_in_fixed_native_adapter(self):
        src = ROOT / 'crates/omavless-netguard/src'
        new = (src / 'kernel_response_diagnostic_fixture.rs').read_text()
        self.assertNotIn('File::open("/proc/1/ns/net")', new)
        self.assertIn('Self::capture_using(Witness::read)', new)
        self.assertIn('let witness = read(host_id)', new)
        self.assertIn('Self::capture_using(Witness::read_lease_regression)', new)
        self.assertIn('self.witness.recheck(self.host_id)', new)
        run = new.split('fn run(held: &mut Held)')[1]
        self.assertLess(run.index('held.isolation.recheck()?'),run.index('FixtureCreator::open'))
        self.assertIn('setns(&null, CloneFlags::empty())', new)
        witness = (src / 'manager_negative_witness.rs').read_text()
        self.assertIn('canonical_authority:false', witness.replace(' ', ''))
        self.assertIn('OFlags::EXCL', witness)
        self.assertIn('deny_unknown_fields', witness)
        self.assertIn(f'const STAGE: &str = "{guard.STAGE}";', witness)
        self.assertIn(f'const UNIT: &str = "{guard.UNIT}";', witness)
        old = (src / 'kernel_manager_private_fixture.rs').read_text()
        self.assertEqual(new.split('fn run(held: &mut Held)')[1].split('#[test]')[0],
                         old.split('fn run(held: &mut Held)')[1].split('#[test]')[0])
        outer = (SUPPORT / 'response_diagnostic_guard.py').read_text()
        native = (src / 'manager_response_diagnostic_fixture.rs').read_text()
        for forbidden in ('StartUnit', 'StopUnit', 'SetProperties', 'ReloadUnit'):
            self.assertNotIn(forbidden, native)
        self.assertNotIn("'/usr/bin/systemctl', 'start'", outer)
        self.assertNotIn("'/usr/bin/systemctl', 'stop'", outer)
        self.assertIn("manager_retained_lifecycle::adapter::run_private_lifecycle", outer)
        adapter = (src / 'manager_retained_lifecycle_adapter.rs').read_text()
        self.assertIn('"StartUnit"', adapter)
        self.assertIn('"StopUnit"', adapter)
        for forbidden in ('SetProperties', 'StartTransientUnit', 'RestartUnit', 'KillUnit'):
            self.assertNotIn(forbidden, adapter)
        self.assertIn('&(self.fixture.unit(), "fail")', adapter)
        identities = (src / 'manager_fixture_identity.rs').read_text()
        self.assertIn('pub(crate) enum Fixture', identities)
        self.assertIn('PrivateLifecycle,', identities)
        self.assertIn('RetainedLease,', identities)
        self.assertNotIn('Deserialize', identities)

    def fixture(self):
        obj = object.__new__(guard.Observer)
        obj.directory_fd, obj.parent_fd = 17, 18
        obj.sealed, obj.lifecycle_validated, obj.helper_known_zero = False, False, False
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
            obj.lifecycle_validated = True

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
        self.assertTrue(obj.helper_known_zero and obj.lifecycle_validated)
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
            obj.ns['await_child'].assert_called_once_with(child, seconds=180)

    def test_cleanup_requires_both_known_child_and_receipts_before_unlink(self):
        for known, evidence in ((False, True), (True, False), (False, False)):
            obj = self.fixture()
            obj.helper_known_zero, obj.lifecycle_validated = known, evidence
            obj.phase = 'validate-evidence'
            obj.write = Mock()
            obj.recheck = Mock()
            with patch.object(guard.os, 'unlink') as unlink, self.assertRaises(RuntimeError):
                obj.cleanup_known_success()
            obj.recheck.assert_not_called()
            unlink.assert_not_called()

    def test_no_global_process_scan_and_no_old_fixture_change(self):
        source = (SUPPORT / 'response_diagnostic_guard.py').read_text()
        self.assertNotIn("Path('/proc')", source)
        self.assertNotIn('exact_inode_absent', source)
        self.assertEqual(hashlib.sha256((SUPPORT / 'version_reference_guard.py').read_bytes()).hexdigest(),
                         'e35cfe4d7aac64d71158357806dfbcf09135ba57fa8a6a8b4986e9b2c2e1f9e7')

    def test_actual_cleanup_checks_pinned_unit_and_cgroup_before_exact_unlink(self):
        for cgroup in (False, True):
            obj = self.fixture()
            obj.helper_known_zero = obj.lifecycle_validated = True
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
        for name, path in {'guard.py': SUPPORT / 'response_diagnostic_guard.py',
                          'query-guard.py': SUPPORT / 'response_diagnostic_guest_guard.py',
                          'fixture.service': SUPPORT.parent / 'fixtures' / guard.UNIT}.items():
            self.assertEqual(stage.MEMBERS[name][0], hashlib.sha256(path.read_bytes()).hexdigest())
        self.assertEqual(stage.MEMBERS['probe'][0], guard.PROBE_SHA)
        self.assertEqual(guard.PROBE_SHA,
                         'b9b07d98dfcfdbe13b9bbedc033769d5590327b4ea51770f46c165eb4385e5f5')
        self.assertEqual(guard.NATIVE_SOURCE, '4c8e0c9f3b7aab2b7387f58c711220d9dd4a20f6')
        self.assertEqual(stage.DESTINATION, guard.STAGE)

    def lifecycle(self):
        execution = dict(invocation=[7]*16, pid=123, start=100, exit=200,
                         command_start=100, command_exit=200)
        result = dict(schema=1, unit=guard.UNIT, unique_owner=':1.77',
            start_job={'id':17, 'path':'/org/freedesktop/systemd1/job/17'},
            stop_job={'id':23, 'path':'/org/freedesktop/systemd1/job/23'},
            execution=execution, effects=2, closed_retired=True, absent=True,
            stopped=True, unref_acknowledged=True, synthetic_epoch=True, production_admission=False)
        native = dict(schema=1, execution=execution, effects=2, closed_retired=True,
                      absent=True, synthetic_epoch=True)
        stopped = dict(schema=1, execution=execution, inactive_dead=True,
                       zero_pids=True, no_job=True, empty_cgroup=True)
        return result, native, stopped

    def test_lifecycle_receipts_bind_same_execution_and_exact_jobs_without_product_claim(self):
        records = self.lifecycle()
        guard.validate_lifecycle(*records, ':1.77')
        for index, key in [(0,'unit'),(0,'unique_owner'),(0,'closed_retired'),(0,'stopped'),
                           (0,'unref_acknowledged'),(0,'production_admission'),
                           (1,'closed_retired'),(1,'absent'),(2,'zero_pids'),(2,'empty_cgroup')]:
            changed = json.loads(json.dumps(records))
            changed[index][key] = 'wrong'
            with self.assertRaises(RuntimeError):
                guard.validate_lifecycle(*changed, ':1.77')
        for index in range(3):
            changed = json.loads(json.dumps(records))
            changed[index]['schema'] = True
            with self.assertRaises(RuntimeError):
                guard.validate_lifecycle(*changed, ':1.77')

    def test_lifecycle_pid_time_invocation_job_types_and_cross_proof_change_refuse(self):
        records = self.lifecycle()
        for key, value in [('pid',True),('pid',0),('start',True),('start',0),('exit',2**64-1),
                           ('invocation',[0]*16),('invocation',[True]*16),('command_exit',99)]:
            changed = json.loads(json.dumps(records))
            changed[0]['execution'][key] = value
            with self.assertRaises(RuntimeError):
                guard.validate_lifecycle(*changed, ':1.77')
        for value in [{'id':True,'path':'/org/freedesktop/systemd1/job/1'},
                      {'id':17,'path':'/org/freedesktop/systemd1/job/017'}, records[0]['start_job']]:
            changed = json.loads(json.dumps(records))
            changed[0]['stop_job'] = value
            with self.assertRaises(RuntimeError):
                guard.validate_lifecycle(*changed, ':1.77')
        changed = json.loads(json.dumps(records))
        changed[2]['execution']['invocation'][0] = 8
        with self.assertRaises(RuntimeError):
            guard.validate_lifecycle(*changed, ':1.77')

    def test_no_inert_post_unref_or_nine_rpc_cleanup_path_in_actual_evidence(self):
        import inspect
        evidence = inspect.getsource(guard.Observer.evidence)
        self.assertIn('range(7)', evidence)
        self.assertIn('validate_lifecycle', evidence)
        self.assertIn('self.native_evidence()', evidence)
        self.assertNotIn('private-admission-post-unref-state.json', evidence)
        self.assertNotIn('private-admission-unref-ack.json', evidence)


if __name__ == '__main__':
    unittest.main()
