"""Inert static catalog/closure controls; no actual tool, guest or network."""
import copy
import hashlib
import json
import os
from pathlib import Path
from types import SimpleNamespace
import tempfile
import time
import unittest
from unittest.mock import Mock, patch

from tests.six_library_static_closure import probe, validator, owned, supervisor, transport
from tests.real_resolved_binary import probe as base_module
from tests.test_encoder_boundary_provenance import ELF, metadata
from tests import test_live_fd_transport as old_transport

ROOT = Path(__file__).parent
MANIFEST_RAW = (ROOT/'encoder_libm_copy_admission/copy-manifest.json').read_bytes()
MANIFEST = json.loads(MANIFEST_RAW)


def fixture():
    manifest = copy.deepcopy(MANIFEST)
    for row in manifest['source_provenance'].values():
        row['sha256'] = hashlib.sha256(ELF).hexdigest()
    sources = Mock()
    sources.files, sources.package_files = {}, {}
    sources.packages_validated = False
    sources.deadline = time.monotonic() + 120
    selected = {name: (name+'-1:2.3~rc1-4', '1:2.3~rc1-4') for name in probe.PACKAGES}
    names = tuple(sorted(directory for directory, _ in selected.values()))
    directory = metadata()
    directory.st_mode, directory.st_nlink, directory.st_size = 0o40755, 2, 4096
    sources.catalog = (8, directory, names)
    sources.select_packages.return_value = selected
    for name, (directory, version) in selected.items():
        for leaf in ('desc', 'files'):
            sources.package_files[str(probe.CATALOG/directory/leaf)] = (name, leaf)
    def take(path):
        raw, info = ELF, metadata()
        if path in sources.package_files:
            name, leaf = sources.package_files[path]
            raw = (('%NAME%\n'+name+'\n\n%VERSION%\n'+selected[name][1]+'\n')
                   if leaf == 'desc' else '%FILES%\n'+'\n'.join(p[1:] for p in probe.PACKAGES[name])+'\n').encode()
            info.st_size = len(raw)
        elif path == probe.READELF:
            info = metadata(29149, 810072)
        elif path in manifest['source_provenance']:
            row = manifest['source_provenance'][path]
            info = SimpleNamespace(**{'st_'+a: row[b] for a,b in
                (('dev','device'),('ino','inode'),('mode','mode'),('uid','uid'),
                 ('gid','gid'),('nlink','nlink'),('size','size'))}, st_mtime_ns=1, st_ctime_ns=1)
        if path in probe.CANDIDATES:
            assert sources.packages_validated
        value = (9, info, raw, 10)
        sources.files[path] = value
        return value
    sources.take.side_effect = take
    base = Mock(UNSETTLED=[])
    base.command.return_value = SimpleNamespace(stderr=b'', stdout=b'inert')
    helper = Mock()
    helper.canonical_public.side_effect = lambda p: (p, [])
    helper.resolve_needed.side_effect = lambda n: ('/usr/lib/'+n, '/usr/lib/'+n, [])
    helper.decode_readelf.side_effect = [
        {'needed':['libc.so.6'], 'interpreter':None, 'declared_search_tokens':[]}
        for _ in probe.CANDIDATES] + [
        {'needed':['ld-linux-x86-64.so.2'], 'interpreter':None, 'declared_search_tokens':[]},
        {'needed':[], 'interpreter':None, 'declared_search_tokens':[]}]
    helper.owned = SimpleNamespace(command=lambda _base, *a, **kw: base.command(*a, **kw))
    return sources, base, helper, manifest


def result():
    sources, base, helper, manifest = fixture()
    with patch.object(probe, 'Sources', return_value=sources):
        value = probe.capture(base, helper, helper.owned, manifest)
    return value, manifest


class ClosureTests(unittest.TestCase):
    def test_all_six_roots_complete_known_edges_and_negative_flags(self):
        value, manifest = result()
        self.assertEqual(validator.validate(value, manifest), value)
        self.assertEqual(len(value['records']), 8)
        self.assertEqual(value['candidates'], list(probe.CANDIDATES))
        for key in ('candidate_elf_executed','allowlist_adoption',
                    'loaded_elf_identity_proven','compatibility_acceptance'):
            self.assertIs(value[key], False)
        for package in value['packages']:
            self.assertIs(package['global_package_owner_uniqueness_proven'], False)
            self.assertIs(package['package_signature_verified'], False)

    def test_existing_nineteen_manifest_is_exact_and_unchanged(self):
        self.assertEqual(hashlib.sha256(MANIFEST_RAW).hexdigest(), validator.MANIFEST_SHA)
        self.assertEqual(len(MANIFEST['source_provenance']), 19)
        self.assertFalse(set(probe.CANDIDATES) & set(MANIFEST['source_provenance']))
        for count in (18, 20):
            bad = copy.deepcopy(MANIFEST)
            if count == 18:
                bad['source_provenance'].pop(next(iter(bad['source_provenance'])))
            else:
                bad['source_provenance']['/usr/lib/third.so'] = {}
            with patch.object(probe, 'Sources') as sources, self.assertRaises(RuntimeError):
                probe.capture(Mock(), Mock(), Mock(), bad)
            sources.assert_not_called()

    def test_ten_packages_and_recheck_before_tool_or_any_new_elf(self):
        sources, base, helper, manifest = fixture()
        events, original = [], sources.take.side_effect
        sources.take.side_effect = lambda p: (events.append(p), original(p))[1]
        sources.recheck.side_effect = lambda: events.append('recheck')
        with patch.object(probe, 'Sources', return_value=sources):
            probe.capture(base, helper, helper.owned, manifest)
        self.assertEqual(events[:10], list(sources.package_files))
        self.assertEqual(events[10], 'recheck')
        self.assertEqual(events[11], probe.READELF)
        self.assertEqual(base.command.call_count, 8)
        self.assertEqual(len(sources.files), 19)

    def test_each_package_failure_prevents_tool_and_new_candidates(self):
        reference, _, _, _ = fixture()
        for failed in reference.package_files:
            sources, base, helper, manifest = fixture()
            original = sources.take.side_effect
            def take(p):
                if p == failed:
                    raise OSError('inert')
                return original(p)
            sources.take.side_effect = take
            with patch.object(probe, 'Sources', return_value=sources), self.assertRaises(OSError):
                probe.capture(base, helper, helper.owned, manifest)
            base.command.assert_not_called()
            self.assertFalse(sources.packages_validated)
            self.assertFalse(set(sources.files) & set(probe.CANDIDATES))

    def test_name_version_membership_duplicates_and_private_traversal(self):
        desc = b'%NAME%\nopenssl\n\n%VERSION%\n1:2.3~rc1-4\n'
        members = tuple(p[1:] for p in probe.PACKAGES['openssl'])
        good = ('%FILES%\n'+'\n'.join(members)+'\n').encode()
        probe.package_binding(desc, good, 'openssl', '1:2.3~rc1-4', members)
        for files in (b'%FILES%\nusr/lib/libcrypto.so.3\n', good+members[0].encode()+b'\n',
                      good+b'../private\n', good+b'%FILES%\nx\n', good.replace(b'\n',b'\r\n')):
            with self.assertRaises(RuntimeError):
                probe.package_binding(desc, files, 'openssl', '1:2.3~rc1-4', members)
        for name, version in (('zlib','1:2.3~rc1-4'), ('openssl','wrong')):
            with self.assertRaises(RuntimeError):
                probe.package_binding(desc, good, name, version, members)
        for name in ('openssl-1:2.3~rc1-4', 'openssl-3.4.0-1.2'):
            self.assertEqual(probe.directory_version(name, 'openssl'), name[8:])
        for name in ('openssl-../x-1', 'openssl-x', 'openssl-1\n-2',
                     'openssl--1', 'other-3.4-1', 'openssl-3:4:5-1'):
            with self.assertRaises(RuntimeError):
                probe.directory_version(name, 'openssl')

    def test_candidates_unbound_unknown_package_or_seventh_elf_never_open(self):
        for path, bound in [(p, False) for p in probe.CANDIDATES] + [
                ('/usr/lib/third.so', True), ('/var/lib/pacman/local/private/desc', True)]:
            obj = probe.Sources(MANIFEST['source_provenance'])
            obj.packages_validated = bound
            with patch.object(probe.os, 'open') as opened, self.assertRaises(RuntimeError):
                obj.take(path)
            opened.assert_not_called()
            self.assertEqual(obj.state, 'refused')

    def test_unknown_edge_before_next_open_or_tool(self):
        sources, base, helper, manifest = fixture()
        helper.decode_readelf.side_effect = None
        helper.decode_readelf.return_value = {'needed':['third.so'], 'interpreter':None, 'declared_search_tokens':[]}
        with patch.object(probe, 'Sources', return_value=sources), self.assertRaises(RuntimeError):
            probe.capture(base, helper, helper.owned, manifest)
        self.assertEqual(base.command.call_count, 1)
        self.assertNotIn('/usr/lib/third.so', sources.files)

    def test_expired_final_event_prevents_next_readelf_spawn(self):
        sources, base, helper, manifest = fixture()
        sources.available.side_effect = lambda: probe.require(time.monotonic() < sources.deadline)
        def delayed(phase):
            if phase == 'before_readelf':
                sources.deadline = 0
        with patch.object(probe,'Sources',return_value=sources), patch.object(probe,'boundary',side_effect=delayed), \
             self.assertRaises(RuntimeError):
            probe.capture(base,helper,helper.owned,manifest)
        base.command.assert_not_called()
        self.assertEqual(sources.state,'refused')

    def test_any_new_candidate_alias_and_known_inode_change_refuse(self):
        for path in probe.CANDIDATES:
            sources, base, helper, manifest = fixture()
            helper.canonical_public.side_effect = lambda p: ('/usr/lib/libc.so.6', []) if p == path else (p, [])
            with patch.object(probe, 'Sources', return_value=sources), self.assertRaises(RuntimeError):
                probe.capture(base, helper, helper.owned, manifest)
            self.assertNotIn(path, sources.files)
        good, manifest = result()
        known = next(r for r in good['records'] if r['matches_reviewed_source'])
        known['identity'][1] += 1
        with self.assertRaises(ValueError):
            validator.validate(good, manifest)

    def test_nested_strict_receipt_counterexamples_and_complete_reachability(self):
        good, manifest = result()
        variants = []
        for key, value in (('loaded_elf_identity_proven', 0), ('candidates', list(reversed(probe.CANDIDATES))),
                           ('catalog', {}), ('records', {}), ('unknown', False)):
            bad = copy.deepcopy(good); bad[key] = value; variants.append(bad)
        for key, value in (('version','wrong'), ('directory','openssl-wrong'), ('listed_candidates',[]),
                           ('package_signature_verified',1)):
            bad = copy.deepcopy(good); bad['packages'][0][key] = value; variants.append(bad)
        bad = copy.deepcopy(good); bad['packages'][0]['identities']['desc'][1] = True; variants.append(bad)
        bad = copy.deepcopy(good); bad['records'].pop(); variants.append(bad)
        bad = copy.deepcopy(good); bad['aliases'].append(copy.deepcopy(bad['aliases'][0])); variants.append(bad)
        for bad in variants:
            with self.assertRaises((ValueError, KeyError, TypeError)):
                validator.validate(bad, manifest)
        for raw in (b'{"x":1,"x":2}', b'NaN', b'{}x', b'x'*262145):
            with self.assertRaises(ValueError):
                validator.decode(raw)

    def test_deadline_unknown_and_failed_event_permanent_seal(self):
        for clock in (float('inf'), OSError('inert')):
            obj = probe.Sources({})
            with patch.object(probe.time, 'monotonic', side_effect=clock if isinstance(clock, BaseException) else None,
                              return_value=clock):
                with self.assertRaises((RuntimeError, OSError)):
                    obj.available()
            self.assertEqual(obj.state, 'refused')
            with patch.object(probe.os, 'open') as opened, self.assertRaises(RuntimeError):
                obj.take(probe.CANDIDATES[0])
            opened.assert_not_called()
        for value in (None, False, 1.0, 0, OSError('inert')):
            events = probe.Events()
            with patch.object(probe.os, 'write', side_effect=value if isinstance(value, BaseException) else None,
                              return_value=value) as written:
                with self.assertRaises((RuntimeError, OSError)):
                    events.before('entry')
                calls = written.call_count
                with self.assertRaises(RuntimeError):
                    events.before('entry')
                events.failure()
                self.assertEqual(written.call_count, calls)

    def test_capture_unknown_recheck_or_tool_cannot_resume_events(self):
        for fault in ('recheck', 'tool'):
            sources, base, helper, manifest = fixture()
            if fault == 'recheck':
                sources.recheck.side_effect = [None, OSError('inert')]
            else:
                base.command.side_effect = RuntimeError('inert')
            events = probe.Events()
            with patch.object(probe, 'Sources', return_value=sources), patch.object(probe, 'EVENTS', events), \
                 patch.object(probe.os, 'write', side_effect=lambda _, raw: len(raw)):
                with self.assertRaises((OSError, RuntimeError)):
                    probe.capture(base, helper, helper.owned, manifest)
                calls = sources.take.call_count
                with self.assertRaises(RuntimeError):
                    probe.capture(base, helper, helper.owned, manifest)
                self.assertEqual(sources.take.call_count, calls)
                self.assertEqual(sources.state, 'refused')

    def test_final_full_typed_write_flush_deadline_and_permanent_success_seal(self):
        for fault in ('none','short','float','bool','write','flush','late_write','late','success'):
            obj = probe.Sources({})
            stream = Mock()
            stream.buffer.write.side_effect = lambda raw: len(raw)
            if fault in ('none','short','float','bool'):
                stream.buffer.write.side_effect = None
                stream.buffer.write.return_value = {'none':None,'short':0,'float':3.0,'bool':True}[fault]
            elif fault == 'write':
                stream.buffer.write.side_effect = OSError('inert')
            elif fault == 'flush':
                stream.buffer.flush.side_effect = OSError('inert')
            elif fault == 'late_write':
                def late_write(raw):
                    obj.deadline = 0
                    return len(raw)
                stream.buffer.write.side_effect = late_write
            elif fault == 'late':
                stream.buffer.flush.side_effect = lambda: setattr(obj,'deadline',0)
            events = probe.Events()
            with patch.object(probe.sys,'stdout',stream), patch.object(probe,'EVENTS',events), \
                 patch.object(probe.os,'write',side_effect=lambda _,raw:len(raw)):
                if fault == 'success':
                    probe.emit_result({},obj)
                    self.assertEqual(obj.state,'sealed')
                else:
                    with self.assertRaises((RuntimeError,OSError)):
                        probe.emit_result({},obj)
                    self.assertEqual(obj.state,'refused')
                    if fault == 'late_write':
                        stream.buffer.flush.assert_not_called()
                self.assertTrue(events.sealed)
                calls = stream.buffer.write.call_count
                with self.assertRaises(RuntimeError):
                    probe.emit_result({},obj)
                self.assertEqual(stream.buffer.write.call_count,calls)

    def test_exact_acyclic_source_pins(self):
        folder = ROOT/'six_library_static_closure'
        paths = {name: folder/name for name in transport.PINS}
        paths.update({'containment.py': ROOT/'real_resolved_binary/probe.py',
                      'helpers.py': ROOT/'static_elf_provenance/probe_four_mib.py',
                      'copy-manifest.json': ROOT/'encoder_libm_copy_admission/copy-manifest.json'})
        for name, pin in transport.PINS.items():
            self.assertEqual(hashlib.sha256(paths[name].read_bytes()).hexdigest(), pin, name)
        for module in (probe, supervisor):
            for name, pin in module.PINS.items():
                self.assertEqual(pin, transport.PINS[name])
        wrapper = (folder/'vm-guard.sh').read_text()
        for name, pin in transport.PINS.items():
            if name != 'vm-guard.sh':
                self.assertIn(pin, wrapper)
        self.assertEqual(probe.STAGE, supervisor.STAGE)
        self.assertEqual(probe.STAGE, validator.STAGE)
        self.assertEqual(probe.STAGE, Path('/', *transport.PARTS))

    def test_actual_shell_failure_has_no_after_queries(self):
        import subprocess
        wrapper = (ROOT/'six_library_static_closure/vm-guard.sh').read_text()
        fragment = wrapper.split('task_failed=0\n', 1)[1].split('check_category() {', 1)[0]
        for failed in ('supervisor', 'validator'):
            with tempfile.TemporaryDirectory() as temporary:
                stage = Path(temporary); (stage/'scratch').mkdir()
                for name in ('supervisor', 'validator'):
                    (stage/(name+'.py')).write_text('raise SystemExit('+('1' if name == failed else '0')+')\n')
                outcome = subprocess.run(['/bin/bash','-c',fragment+'printf AFTER_FORBIDDEN'],
                    env={'PATH':'/usr/bin','task_stage':temporary}, capture_output=True, timeout=5)
                self.assertEqual(outcome.returncode, 1)
                self.assertNotIn(b'AFTER_FORBIDDEN', outcome.stdout)


class CatalogTests(unittest.TestCase):
    def synthetic(self):
        temporary = tempfile.TemporaryDirectory()
        root = Path(temporary.name)
        local = root/'var/lib/pacman/local'; local.mkdir(parents=True)
        for name, members in probe.PACKAGES.items():
            folder = local/(name+'-1:2.3~rc1-4'); folder.mkdir()
            (folder/'desc').write_bytes(('%NAME%\n'+name+'\n\n%VERSION%\n1:2.3~rc1-4\n').encode())
            (folder/'files').write_bytes(('%FILES%\n'+'\n'.join(p[1:] for p in members)+'\n').encode())
        return temporary, root, local

    def normalized(self, root):
        original_open, original_stat, original_fstat = os.open, os.stat, os.fstat
        def public(s):
            return SimpleNamespace(**{n:getattr(s,n) for n in
                ('st_dev','st_ino','st_mode','st_nlink','st_size','st_mtime_ns','st_ctime_ns')}, st_uid=0, st_gid=0)
        return (patch.object(probe.os, 'open', side_effect=lambda p,*a,**kw: original_open(root if p == '/' else p,*a,**kw)),
                patch.object(probe.os, 'stat', side_effect=lambda p,*a,**kw: public(original_stat(root if p == '/' else p,*a,**kw))),
                patch.object(probe.os, 'fstat', side_effect=lambda fd: public(original_fstat(fd))))

    def close(self, obj):
        for fd in {row[0] for row in (*obj.files.values(), *obj.parents.values())}:
            os.close(fd)

    def test_original_catalog_and_only_ten_selected_contents(self):
        temp, root, local = self.synthetic()
        with temp:
            (local/'unrelated-5.6-1').mkdir()
            (local/'ALPM_DB_VERSION').write_text('9')
            obj = probe.Sources({})
            a,b,c = self.normalized(root)
            with a,b,c:
                try:
                    selected = obj.select_packages()
                    self.assertEqual(set(selected), set(probe.PACKAGES))
                    self.assertEqual(len(obj.package_files), 10)
                    self.assertFalse(obj.files)
                    for path in obj.package_files:
                        obj.take(path)
                    obj.recheck()
                    self.assertEqual(len(obj.files), 10)
                    with patch.object(probe.os,'open') as opened, self.assertRaises(RuntimeError):
                        obj.take(str(probe.CATALOG/'unrelated-5.6-1/desc'))
                    opened.assert_not_called()
                finally:
                    self.close(obj)

    def test_catalog_missing_duplicate_bad_name_and_4097th_refuse_no_contents(self):
        for fault in ('missing', 'duplicate', 'name', 'version', 'overflow'):
            temp, root, local = self.synthetic()
            with temp:
                if fault == 'missing':
                    (local/'zstd-1:2.3~rc1-4').rename(local/'other-1-1')
                elif fault == 'duplicate':
                    (local/'zstd-2.0-1').mkdir()
                elif fault == 'name':
                    (local/'private space').mkdir()
                elif fault == 'version':
                    (local/'zstd-1:2.3~rc1-4').rename(local/'zstd-notcanonical')
                else:
                    for i in range(4092):
                        (local/('unrelated'+str(i)+'-1-1')).mkdir()
                obj = probe.Sources({})
                a,b,c = self.normalized(root)
                with a,b,c:
                    try:
                        with self.assertRaises(RuntimeError):
                            obj.select_packages()
                        self.assertFalse(obj.files)
                        self.assertEqual(obj.state, 'refused')
                    finally:
                        self.close(obj)

    def test_catalog_added_renamed_replaced_xattr_or_unknown_permanently_seals(self):
        for fault in ('added', 'renamed', 'replaced', 'xattr', 'unknown'):
            temp, root, local = self.synthetic()
            with temp:
                obj = probe.Sources({})
                a,b,c = self.normalized(root)
                with a,b,c:
                    try:
                        obj.select_packages()
                        if fault == 'added':
                            (local/'new-1-1').mkdir()
                        elif fault == 'renamed':
                            (local/'zstd-1:2.3~rc1-4').rename(local/'zstd-2-1')
                        elif fault == 'replaced':
                            local.rename(local.parent/'old'); local.mkdir()
                        with patch.object(probe.os, 'listxattr', return_value=['user.inert'] if fault == 'xattr' else []), \
                             patch.object(probe.os, 'scandir', side_effect=OSError('inert')) if fault == 'unknown' else patch.object(probe, 'boundary'):
                            with self.assertRaises((RuntimeError,OSError)):
                                obj.recheck()
                        self.assertEqual(obj.state, 'refused')
                        with patch.object(probe.os,'scandir') as scan, self.assertRaises(RuntimeError):
                            obj.select_packages()
                        scan.assert_not_called()
                    finally:
                        self.close(obj)

    def test_original_record_swap_inplace_fifo_symlink_hardlink_mode_xattr(self):
        for fault in ('swap','inplace','fifo','symlink','hardlink','mode','xattr'):
            temp, root, local = self.synthetic()
            with temp:
                target = local/'openssl-1:2.3~rc1-4/desc'
                obj = probe.Sources({})
                a,b,c = self.normalized(root)
                with a,b,c:
                    try:
                        obj.select_packages()
                        path = str(probe.CATALOG/'openssl-1:2.3~rc1-4/desc')
                        if fault in ('swap','inplace'):
                            obj.take(path); obj.recheck()
                            if fault == 'swap':
                                new = target.parent/'new'; new.write_bytes(target.read_bytes()); new.replace(target)
                            else:
                                raw = target.read_bytes(); target.write_bytes(raw)
                            with self.assertRaises(RuntimeError):
                                obj.recheck()
                        else:
                            if fault in ('fifo','symlink'):
                                target.unlink()
                                if fault == 'fifo':
                                    os.mkfifo(target,0o600)
                                else:
                                    target.symlink_to('absent')
                            elif fault == 'hardlink':
                                os.link(target,target.parent/'alias')
                            elif fault == 'mode':
                                target.chmod(0o777)
                            with patch.object(probe.os,'listxattr',return_value=['user.inert'] if fault == 'xattr' else []):
                                with self.assertRaises((RuntimeError,OSError)):
                                    obj.take(path)
                        self.assertEqual(obj.state,'refused')
                        with patch.object(probe.os,'open') as opened, patch.object(probe.os,'pread') as read:
                            with self.assertRaises(RuntimeError):
                                obj.take(path)
                            opened.assert_not_called(); read.assert_not_called()
                    finally:
                        self.close(obj)


class OwnershipTests(unittest.TestCase):
    def files(self):
        files = [Mock(),Mock()]
        for item in files:
            item.__enter__ = Mock(return_value=item)
            item.__exit__ = Mock(return_value=False)
            item.read.return_value = b''
        return files

    def test_executed_supervisor_child_environment_reaches_owned_scratch_check(self):
        readelf_base = Mock(UNSETTLED=[],require=base_module.require)
        readelf_base.OwnedProcess.return_value = SimpleNamespace(pid=321,returncode=None)
        files = self.files()
        seen = []
        def capture(*args,**kwargs):
            seen.append(kwargs['env'])
            with patch.dict(owned.os.environ,kwargs['env'],clear=True), \
                 patch.object(owned.tempfile,'TemporaryFile',side_effect=files), \
                 patch.object(owned,'settle',side_effect=lambda b,c,s:setattr(c,'returncode',0)):
                result = owned.command(readelf_base,
                    ['/proc/self/fd/10','--wide','--dynamic','--program-headers','/proc/self/fd/11'],
                    pass_fds=(10,11),env={'PATH':'/usr/bin','LANG':'C','LC_ALL':'C'},
                    deadline=time.monotonic()+120)
                self.assertEqual(result.returncode,0)
                readelf_base.OwnedProcess.assert_called_once()
            return SimpleNamespace(returncode=0)
        outer = Mock()
        outer.OwnedProcess.side_effect = capture
        observed_owned = SimpleNamespace(settle=Mock())
        supervisor.start_capture(outer,observed_owned,Mock(),Mock())
        self.assertEqual(seen,[{'HOME':'/home/kdk_vm','PATH':'/usr/bin','LANG':'C',
                               'TMPDIR':str(probe.STAGE/'scratch')}])
        observed_owned.settle.assert_called_once()
        self.assertEqual(observed_owned.settle.call_args.args[2],140)

    def test_delayed_scratch_open_or_preexpired_source_budget_never_spawns(self):
        for clock, count in (([11],0),([1,11],2)):
            files = self.files()
            base = Mock(UNSETTLED=[],require=base_module.require)
            with patch.dict(owned.os.environ,{'TMPDIR':str(probe.STAGE/'scratch')}), \
                 patch.object(owned.time,'monotonic',side_effect=clock), \
                 patch.object(owned.tempfile,'TemporaryFile',side_effect=files) as opened:
                with self.assertRaises(base_module.Refused):
                    owned.command(base,['/proc/self/fd/10','--wide','--dynamic','--program-headers','/proc/self/fd/11'],
                        pass_fds=(10,11),env={'PATH':'/usr/bin','LANG':'C','LC_ALL':'C'},deadline=10.0)
                base.OwnedProcess.assert_not_called()
                self.assertEqual(opened.call_count,count)
                for item in files:
                    item.seek.assert_not_called(); item.read.assert_not_called()

    def test_raw_waitid_field_aliases_float_bool_unknown_and_signal_are_terminal(self):
        for field in ('si_pid','si_code','si_status'):
            for bad in (False, 0.0, '0'):
                child = SimpleNamespace(pid=123, returncode=None)
                status = SimpleNamespace(si_pid=123, si_code=os.CLD_EXITED, si_status=0)
                setattr(status,field,bad)
                with patch.object(base_module,'UNSETTLED',[]), patch.object(owned.os,'waitid',return_value=status) as query, \
                     patch.object(owned.os,'waitpid') as reap:
                    with self.assertRaises(base_module.Refused):
                        owned.settle(base_module,child,5)
                    with self.assertRaises(base_module.Refused):
                        owned.settle(base_module,child,5)
                    self.assertEqual(query.call_count,1)
                    reap.assert_not_called()
                    self.assertIsNone(child.returncode)
        for status in (SimpleNamespace(si_pid=123, si_code=os.CLD_EXITED, si_status=1),
                       SimpleNamespace(si_pid=123, si_code=os.CLD_KILLED, si_status=15),
                       SimpleNamespace(si_pid=123, si_code=os.CLD_KILLED, si_status=0),
                       SimpleNamespace(si_pid=123, si_code=os.CLD_DUMPED, si_status=0),
                       SimpleNamespace(si_pid=123, si_code=os.CLD_EXITED, si_status=256),
                       OSError('inert')):
            child = SimpleNamespace(pid=123, returncode=None)
            with patch.object(base_module,'UNSETTLED',[]), \
                 patch.object(owned.os,'waitid',side_effect=status if isinstance(status,BaseException) else None,
                              return_value=status) as query, patch.object(owned.os,'waitpid') as reap:
                with self.assertRaises(base_module.Refused):
                    owned.settle(base_module,child,5)
                self.assertEqual(query.call_count,1); reap.assert_not_called()

    def test_known_zero_exact_single_typed_reap_and_no_after_unknown(self):
        for outcome in ((123,0),(123.0,0),(123,False),(124,0),(123,256),
                        (123,65536),(123,-65536),OSError('inert')):
            child = SimpleNamespace(pid=123, returncode=None)
            status = SimpleNamespace(si_pid=123, si_code=os.CLD_EXITED, si_status=0)
            with patch.object(base_module,'UNSETTLED',[]), patch.object(owned.os,'waitid',return_value=status) as query, \
                 patch.object(owned.os,'waitpid',side_effect=outcome if isinstance(outcome,BaseException) else None,
                              return_value=outcome) as reap:
                if outcome == (123,0) and type(outcome[0]) is int and type(outcome[1]) is int:
                    owned.settle(base_module,child,5); self.assertEqual(child.returncode,0)
                else:
                    with self.assertRaises(base_module.Refused):
                        owned.settle(base_module,child,5)
                    with self.assertRaises(base_module.Refused):
                        owned.settle(base_module,child,5)
                    self.assertIsNone(child.returncode)
                query.assert_called_once(); reap.assert_called_once_with(123,os.WNOHANG)

    def test_preset_status_timeout_and_unknown_do_not_query_again_or_signal(self):
        for code in (0,False,1):
            child = SimpleNamespace(pid=123, returncode=code)
            with patch.object(base_module,'UNSETTLED',[]), patch.object(owned.os,'waitid') as query:
                with self.assertRaises(base_module.Refused):
                    owned.settle(base_module,child,5)
                query.assert_not_called()
        child = SimpleNamespace(pid=123, returncode=None)
        with patch.object(base_module,'UNSETTLED',[]), patch.object(owned.time,'monotonic',side_effect=[0,1,6]), \
             patch.object(owned.time,'sleep'), patch.object(owned.os,'waitid',return_value=None) as query, \
             patch.object(owned.os,'waitpid') as reap, patch.object(owned.os,'kill') as kill, patch.object(owned.os,'killpg') as group:
            with self.assertRaises(base_module.Refused):
                owned.settle(base_module,child,5)
            query.assert_called_once(); reap.assert_not_called(); kill.assert_not_called(); group.assert_not_called()

    def test_late_zero_and_late_reap_are_permanent_unknowns(self):
        for clock, count in (([0,1,6],0),([0,1,2,6],1)):
            child = SimpleNamespace(pid=123,returncode=None)
            status = SimpleNamespace(si_pid=123,si_code=os.CLD_EXITED,si_status=0)
            with patch.object(base_module,'UNSETTLED',[]), patch.object(owned.time,'monotonic',side_effect=clock), \
                 patch.object(owned.os,'waitid',return_value=status) as query, \
                 patch.object(owned.os,'waitpid',return_value=(123,0)) as reap:
                with self.assertRaises(base_module.Refused):
                    owned.settle(base_module,child,5)
                with self.assertRaises(base_module.Refused):
                    owned.settle(base_module,child,5)
                self.assertIsNone(child.returncode)
                query.assert_called_once()
                self.assertEqual(reap.call_count,count)


class TransportTests(old_transport.TransportTests):
    def setUp(self):
        original = tempfile.TemporaryDirectory
        def private_temp(*args,**kwargs):
            kwargs['dir'] = os.environ['TMPDIR']
            return original(*args,**kwargs)
        temporary = patch.object(old_transport.tempfile,'TemporaryDirectory',side_effect=private_temp)
        temporary.start(); self.addCleanup(temporary.stop)
        selected = patch.object(old_transport,'transport',transport)
        selected.start(); self.addCleanup(selected.stop)
        self.frozen = dict(transport.PINS)
        super().setUp()

    def test_all_eight_transport_pins_match_exact_source_generation(self):
        with patch.object(transport,'PINS',self.frozen):
            ClosureTests().test_exact_acyclic_source_pins()


if __name__ == '__main__':
    unittest.main()
