"""Inert closure controls: no tool, candidate execution, guest or network."""
import ast
import copy
import hashlib
import json
import os
import subprocess
import tempfile
from pathlib import Path
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, patch

from tests.encoder_libm_closure import probe, validator, owned, supervisor, transport
from tests.test_encoder_boundary_provenance import ELF, metadata, MANIFEST
from tests import test_brotli_encoder_owned as old_owned
from tests import test_live_fd_transport as old_transport

ROOT = Path(__file__).parent


def fixture():
    manifest = copy.deepcopy(MANIFEST)
    for row in manifest['source_provenance'].values():
        row['sha256'] = hashlib.sha256(ELF).hexdigest()
    sources = Mock()
    sources.files = {}
    sources.packages_validated = False
    def take(path):
        raw, info = ELF, metadata()
        if path == probe.PACKAGE+'desc': raw = b'%NAME%\nbrotli\n\n%VERSION%\n1.2.0-1\n'
        elif path == probe.PACKAGE+'files': raw = ('%FILES%\n'+probe.CANDIDATE[1:]+'\n').encode()
        elif path == probe.GLIBC_PACKAGE+'desc': raw = ('%NAME%\nglibc\n\n%VERSION%\n'+probe.GLIBC_VERSION+'\n').encode()
        elif path == probe.GLIBC_PACKAGE+'files': raw = b'%FILES%\nusr/lib/libm.so.6\nusr/lib/libc.so.6\n'
        elif path == probe.READELF: info = metadata(29149,810072)
        elif path in manifest['source_provenance']:
            row=manifest['source_provenance'][path]
            info=SimpleNamespace(**{('st_'+a):row[b] for a,b in
                (('dev','device'),('ino','inode'),('mode','mode'),('uid','uid'),('gid','gid'),('nlink','nlink'),('size','size'))},
                st_mtime_ns=1,st_ctime_ns=1)
        if path in (probe.CANDIDATE,probe.LIBM):
            assert sources.packages_validated
        result=(9,info,raw,10);sources.files[path]=result
        return result
    sources.take.side_effect=take
    base=Mock(UNSETTLED=[])
    base.command.return_value=SimpleNamespace(stderr=b'',stdout=b'inert')
    helper=Mock()
    aliases={'/usr/lib/libbrotlicommon.so.1':'/usr/lib/libbrotlicommon.so.1.2.0'}
    def canonical(path):
        if path in aliases:
            target=aliases[path]
            link={'path':path,'target':target.rsplit('/',1)[1],
                  'identity':[31,99,len(target.rsplit('/',1)[1]),0,0,0o120777,1,1,1]}
            return target,[link]
        return path,[]
    helper.canonical_public.side_effect=canonical
    helper.resolve_needed.side_effect=lambda name:('/usr/lib/'+name,*canonical('/usr/lib/'+name))
    helper.decode_readelf.side_effect=[
        {'needed':['libm.so.6','libbrotlicommon.so.1','libc.so.6'],'interpreter':None,'declared_search_tokens':[]},
        {'needed':['libc.so.6'],'interpreter':None,'declared_search_tokens':[]},
        {'needed':['libc.so.6'],'interpreter':None,'declared_search_tokens':[]},
        {'needed':['ld-linux-x86-64.so.2'],'interpreter':None,'declared_search_tokens':[]},
        {'needed':[],'interpreter':None,'declared_search_tokens':[]},
    ]
    helper.owned=SimpleNamespace(command=lambda _base,*a,**kw:base.command(*a,**kw))
    return sources,base,helper,manifest


def result():
    sources,base,helper,manifest=fixture()
    with patch.object(probe,'Sources',return_value=sources):
        value=probe.capture(base,helper,helper.owned,manifest)
    return value,manifest


class ClosureTests(unittest.TestCase):
    def test_full_two_candidate_closure_and_all_false_authority_flags(self):
        value,manifest=result()
        self.assertEqual(validator.validate(value,manifest),value)
        self.assertEqual(len(value['records']),5)
        for key in ('candidate_elf_executed','allowlist_adoption','loaded_elf_identity_proven','compatibility_acceptance'):
            self.assertIs(value[key],False)
        self.assertEqual(value['explicit_additional_candidate'],probe.LIBM)

    def test_fixed_glibc_package_pins_come_from_unchanged_manifest(self):
        raw=(ROOT/'decoder_copy_admission/copy-manifest.json').read_bytes()
        self.assertEqual(hashlib.sha256(raw).hexdigest(),validator.MANIFEST_SHA)
        for path in ('/usr/lib/libc.so.6','/usr/lib/ld-linux-x86-64.so.2'):
            row=MANIFEST['source_provenance'][path]['package']
            self.assertEqual(row['name'],'glibc');self.assertEqual(row['version'],probe.GLIBC_VERSION)
            self.assertEqual(row['description_sha256'],probe.GLIBC_PINS['desc'])
            self.assertEqual(row['file_list_sha256'],probe.GLIBC_PINS['files'])

    def test_four_package_originals_rechecked_before_any_new_elf_or_tool(self):
        sources,base,helper,manifest=fixture()
        events=[]
        original=sources.take.side_effect
        def take(path):events.append(path);return original(path)
        sources.take.side_effect=take
        sources.recheck.side_effect=lambda:events.append('recheck')
        with patch.object(probe,'Sources',return_value=sources):
            probe.capture(base,helper,helper.owned,manifest)
        self.assertEqual(events[:5],[probe.PACKAGE+'desc',probe.PACKAGE+'files',
                                    probe.GLIBC_PACKAGE+'desc',probe.GLIBC_PACKAGE+'files','recheck'])
        self.assertEqual(base.command.call_count,5)
        self.assertEqual(len(sources.files),10)  # 4 package + tool + 5 reachable ELF

    def test_each_package_failure_stops_before_tool_or_candidate(self):
        for failed in probe.PACKAGE_FILES:
            sources,base,helper,manifest=fixture();original=sources.take.side_effect
            def take(path):
                if path==failed:raise OSError('synthetic')
                return original(path)
            sources.take.side_effect=take
            with patch.object(probe,'Sources',return_value=sources),self.assertRaises(OSError):
                probe.capture(base,helper,helper.owned,manifest)
            base.command.assert_not_called()
            self.assertFalse(sources.packages_validated)

    def test_package_missing_member_duplicate_or_wrong_version_refuses(self):
        desc=('%NAME%\nglibc\n\n%VERSION%\n'+probe.GLIBC_VERSION+'\n').encode()
        good=b'%FILES%\nusr/lib/libm.so.6\nusr/lib/libc.so.6\n'
        for raw in (b'%FILES%\nusr/lib/libc.so.6\n',good+b'usr/lib/libm.so.6\n',
                    good+b'../private\n',good+b'%FILES%\nx\n',good.replace(b'\n',b'\r\n')):
            with self.assertRaises(RuntimeError):probe.package_binding(desc,raw,'glibc',probe.GLIBC_VERSION,
                                                        ('usr/lib/libm.so.6','usr/lib/libc.so.6'))
        with self.assertRaises(RuntimeError):probe.package_binding(desc,good,'glibc','wrong',('usr/lib/libm.so.6',))

    def test_actual_selector_refuses_unbound_candidates_and_third_object_before_open(self):
        for path,bound in ((probe.CANDIDATE,False),(probe.LIBM,False),('/usr/lib/third.so',True)):
            obj=probe.Sources(MANIFEST['source_provenance']);obj.packages_validated=bound
            with patch.object(probe.os,'open') as opened,self.assertRaises(RuntimeError):obj.take(path)
            opened.assert_not_called();self.assertEqual(obj.state,'refused')

    def test_third_dependency_refuses_before_next_open_or_tool(self):
        sources,base,helper,manifest=fixture()
        helper.decode_readelf.side_effect=None
        helper.decode_readelf.return_value={'needed':['third.so'],'interpreter':None,'declared_search_tokens':[]}
        with patch.object(probe,'Sources',return_value=sources),self.assertRaises(RuntimeError):
            probe.capture(base,helper,helper.owned,manifest)
        self.assertEqual(base.command.call_count,1)
        self.assertNotIn('/usr/lib/third.so',sources.files)

    def test_libm_alias_or_queued_swap_refuses(self):
        sources,base,helper,manifest=fixture();original=helper.canonical_public.side_effect
        helper.canonical_public.side_effect=lambda path:('/usr/lib/libc.so.6',[]) if path==probe.LIBM else original(path)
        with patch.object(probe,'Sources',return_value=sources),self.assertRaises(RuntimeError):
            probe.capture(base,helper,helper.owned,manifest)
        self.assertEqual(base.command.call_count,1)
        self.assertNotIn(probe.LIBM,sources.files)

    def test_new_final_recheck_or_tool_unknown_permanently_seals_events(self):
        for fault in ('recheck','tool'):
            sources,base,helper,manifest=fixture()
            if fault=='recheck':sources.recheck.side_effect=[None,OSError('synthetic')]
            else:base.command.side_effect=RuntimeError('unknown')
            events=probe.Events()
            with patch.object(probe,'Sources',return_value=sources),patch.object(probe,'EVENTS',events), \
                 patch.object(probe.os,'write',side_effect=lambda _,raw:len(raw)):
                with self.assertRaises((OSError,RuntimeError)):probe.capture(base,helper,helper.owned,manifest)
                calls=sources.take.call_count
                with self.assertRaises(RuntimeError):probe.capture(base,helper,helper.owned,manifest)
                self.assertEqual(sources.take.call_count,calls)

    def test_strict_glibc_alias_closure_and_boolean_receipt_counterexamples(self):
        good,manifest=result()
        variants=[]
        for field,value in (('libm_listed',1),('known_libc_listed',False),('version','wrong')):
            bad=copy.deepcopy(good);bad['glibc_package'][field]=value;variants.append(bad)
        bad=copy.deepcopy(good);bad['glibc_package']['identities']['desc'][1]=True;variants.append(bad)
        bad=copy.deepcopy(good);bad['records']=[r for r in bad['records'] if r['path']!=probe.LIBM];variants.append(bad)
        bad=copy.deepcopy(good)
        next(row for row in bad['records'] if row['path']==probe.CANDIDATE)['dependencies'][0]['resolved_path']='/usr/lib/third.so'
        variants.append(bad)
        bad=copy.deepcopy(good);bad['aliases'].append(copy.deepcopy(bad['aliases'][0]));variants.append(bad)
        for bad in variants:
            with self.assertRaises((ValueError,KeyError,TypeError)):validator.validate(bad,manifest)

    def test_source_caps_and_no_broad_package_or_old_capture(self):
        source=Path(probe.__file__).read_text()
        for forbidden in ('package_index(', 'helpers.capture(', 'base.exercise('):self.assertNotIn(forbidden,source)
        self.assertIn('len(self.files) < 24',source)
        self.assertIn('self.total <= 128 * 1024 * 1024',source)
        self.assertIn('time.monotonic() + 120',source)
        self.assertIn('owned.settle(base,child,140)',Path(supervisor.__file__).read_text())

    def test_owned_zero_only_inherited_controls(self):
        with patch.object(old_owned,'owned',owned):
            for name in unittest.defaultTestLoader.getTestCaseNames(old_owned.OwnedTests):
                getattr(old_owned.OwnedTests(), name)()

    def test_actual_original_fd_mutation_fifo_and_seal(self):
        # Synthetic tree with real descriptors; only ownership is normalized.
        original_open, original_stat, original_fstat = os.open, os.stat, os.fstat
        def public(s):
            return SimpleNamespace(**{n:getattr(s,n) for n in
                ('st_dev','st_ino','st_mode','st_nlink','st_size','st_mtime_ns','st_ctime_ns')},
                st_uid=0,st_gid=0)
        for fault in ('swap','inplace','fifo','symlink','hardlink','mode','xattr'):
            with self.subTest(fault=fault), tempfile.TemporaryDirectory(dir='/var/tmp') as temporary:
                root=Path(temporary); (root/'usr/lib').mkdir(parents=True)
                target=root/'usr/lib/libm.so.6';target.write_bytes(ELF);target.chmod(0o755)
                obj=probe.Sources({});obj.packages_validated=True
                with patch.object(probe.os,'open',side_effect=lambda p,*a,**kw:original_open(root if p=='/' else p,*a,**kw)), \
                     patch.object(probe.os,'stat',side_effect=lambda p,*a,**kw:public(original_stat(root if p=='/' else p,*a,**kw))), \
                     patch.object(probe.os,'fstat',side_effect=lambda fd:public(original_fstat(fd))):
                    try:
                        if fault in ('fifo','symlink'):
                            target.unlink()
                            if fault=='fifo':os.mkfifo(target,0o600)
                            else:target.symlink_to('absent')
                        elif fault=='hardlink':os.link(target,target.parent/'alias')
                        elif fault=='mode':target.chmod(0o777)
                        if fault in ('swap','inplace'):
                            obj.take(probe.LIBM);obj.recheck()
                            if fault=='swap':
                                new=target.parent/'new';new.write_bytes(ELF);new.chmod(0o755);new.replace(target)
                            else:target.write_bytes(b'changed');target.write_bytes(ELF)
                            with self.assertRaises(RuntimeError):obj.recheck()
                        else:
                            with patch.object(probe.os,'listxattr',return_value=['user.synthetic'] if fault=='xattr' else []):
                                with self.assertRaises((RuntimeError,OSError)):obj.take(probe.LIBM)
                        self.assertEqual(obj.state,'refused')
                        with patch.object(probe.os,'open') as opened,patch.object(probe.os,'pread') as read:
                            with self.assertRaises(RuntimeError):obj.take(probe.LIBM)
                            with self.assertRaises(RuntimeError):obj.recheck()
                            opened.assert_not_called();read.assert_not_called()
                    finally:
                        for fd in {row[0] for row in (*obj.files.values(),*obj.parents.values())}:os.close(fd)

    def test_exact_acyclic_source_pins(self):
        folder=ROOT/'encoder_libm_closure'
        paths={name:folder/name for name in transport.PINS}
        paths.update({'containment.py':ROOT/'real_resolved_binary/probe.py',
                      'helpers.py':ROOT/'static_elf_provenance/probe_four_mib.py',
                      'copy-manifest.json':ROOT/'decoder_copy_admission/copy-manifest.json'})
        for name,pin in transport.PINS.items():
            self.assertEqual(hashlib.sha256(paths[name].read_bytes()).hexdigest(),pin,name)
        for module in (probe,supervisor):
            for name,pin in module.PINS.items():self.assertEqual(pin,transport.PINS[name])
        wrapper=(folder/'vm-guard.sh').read_text()
        for name,pin in transport.PINS.items():
            if name!='vm-guard.sh':self.assertIn(pin,wrapper)
        self.assertEqual(probe.STAGE,supervisor.STAGE)
        self.assertEqual(probe.STAGE,validator.STAGE)
        self.assertEqual(probe.STAGE,Path('/',*transport.PARTS))

    def test_actual_shell_invalid_branch_has_no_after_query(self):
        wrapper=(ROOT/'encoder_libm_closure/vm-guard.sh').read_text()
        fragment=wrapper.split('task_failed=0\n',1)[1].split('check_category() {',1)[0]
        for failed in ('supervisor','validator'):
            with tempfile.TemporaryDirectory(dir='/var/tmp') as temporary:
                stage=Path(temporary);(stage/'scratch').mkdir()
                for name in ('supervisor','validator'):
                    (stage/(name+'.py')).write_text('raise SystemExit('+('1' if name==failed else '0')+')\n')
                result=subprocess.run(['/bin/bash','-c',fragment+'printf AFTER_FORBIDDEN'],
                    env={'PATH':'/usr/bin','task_stage':temporary},capture_output=True,timeout=5)
                self.assertNotEqual(result.returncode,0)
                self.assertNotIn(b'AFTER_FORBIDDEN',result.stdout)

    def test_duplicate_json_and_nested_string_or_boolean_refuse(self):
        for raw in (b'{"x":1,"x":2}',b'NaN',b'{}x',b'x'*262145):
            with self.assertRaises(ValueError):validator.decode(raw)
        value,manifest=result()
        for key,badvalue in (('records','x'),('aliases',{}),('candidate_elf_executed',0)):
            bad=copy.deepcopy(value);bad[key]=badvalue
            with self.assertRaises(ValueError):validator.validate(bad,manifest)


class TransportTests(old_transport.TransportTests):
    def setUp(self):
        selected=patch.object(old_transport,'transport',transport)
        selected.start();self.addCleanup(selected.stop)
        self.frozen=dict(transport.PINS)
        super().setUp()

    def test_all_eight_transport_pins_match_exact_source_generation(self):
        with patch.object(transport,'PINS',self.frozen):
            ClosureTests().test_exact_acyclic_source_pins()


if __name__=='__main__':unittest.main()
