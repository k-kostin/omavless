"""Synthetic only: never invoke readelf, candidate, guest or namespace."""
import copy
import hashlib
import json
import os
from pathlib import Path
import struct
import subprocess
import tempfile
import time
import types
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, patch
from tests.frontier_fixture_helpers import fixed_vm_process_os

from tests.encoder_static_boundaries import probe, validator, supervisor, transport
from tests.static_elf_provenance import probe_four_mib as helpers

ROOT = Path(__file__).parent
MANIFEST_RAW = (ROOT/'decoder_copy_admission/copy-manifest.json').read_bytes()
MANIFEST = json.loads(MANIFEST_RAW)
ELF = struct.pack('<16sHHIQQQIHHHHHH', b'\x7fELF\x02\x01\x01'+b'\0'*9,
                  3,62,1,0,0,0,0,64,56,0,64,0,0)


def metadata(ino=1, size=64):
    return SimpleNamespace(st_dev=31,st_ino=ino,st_mode=0o100755,st_uid=0,st_gid=0,
                           st_nlink=1,st_size=size,st_mtime_ns=1,st_ctime_ns=1)


def capture_fixture():
    sources = Mock()
    sources.files = {probe.PACKAGE+n:(9,metadata(),b'',10) for n in probe.PACKAGE_PINS}
    def take(path):
        raw = ELF
        value = metadata()
        if path == probe.PACKAGE+'desc': raw = b'%NAME%\nbrotli\n\n%VERSION%\n1.2.0-1\n'
        if path == probe.PACKAGE+'files': raw = b'%FILES%\nusr/lib/libbrotlienc.so.1.2.0\n'
        if path == probe.READELF: value = metadata(29149,810072)
        return 9,value,raw,10
    sources.take.side_effect = take
    base = Mock(UNSETTLED=[])
    base.command.return_value = SimpleNamespace(stderr=b'',stdout=b'Dynamic section at offset 0x0 contains 0 entries:\n')
    helper = Mock()
    helper.canonical_public.side_effect = lambda path:(path,[])
    helper.decode_readelf.side_effect = helpers.decode_readelf
    helper.owned = SimpleNamespace(command=lambda _base,*a,**kw:base.command(*a,**kw))
    return sources,base,helper


def receipt():
    sources,base,helper = capture_fixture()
    with patch.object(probe,'Sources',return_value=sources):
        return probe.capture(base,helper,helper.owned,MANIFEST)


class EncoderTests(unittest.TestCase):
    def test_exact_seventeen_known_objects_no_encoder_or_alias_admission(self):
        known = MANIFEST['source_provenance']
        self.assertEqual(len(known), 17)
        self.assertIn('/usr/lib/libbrotlidec.so.1.2.0', known)
        self.assertNotIn(probe.CANDIDATE, known)
        self.assertNotIn('/usr/lib/libbrotlienc.so.1', known)
        self.assertEqual(probe.CANDIDATE, '/usr/lib/libbrotlienc.so.1.2.0')
        self.assertEqual(hashlib.sha256(MANIFEST_RAW).hexdigest(), validator.MANIFEST_SHA)
        for count in (16, 18):
            altered = copy.deepcopy(MANIFEST)
            if count == 16:
                del altered['source_provenance']['/usr/lib/libbrotlidec.so.1.2.0']
            else:
                altered['source_provenance']['/usr/lib/unknown.so'] = {}
            with patch.object(probe, 'Sources') as sources, self.assertRaises(RuntimeError):
                probe.capture(Mock(), Mock(), Mock(), altered)
            sources.assert_not_called()

    def test_package_must_list_exact_canonical_encoder_before_tool(self):
        for content in (b'%FILES%\nusr/lib/libbrotlidec.so.1.2.0\n',
                        b'%FILES%\nusr/lib/libbrotlienc.so.1\n',
                        b'%FILES%\nusr/lib/libbrotlienc.so.1.2.0\nusr/lib/libbrotlienc.so.1.2.0\n',
                        b'%FILES%\nusr/lib/libbrotlienc.so.1.2.0\n../private\n'):
            sources, base, helper = capture_fixture()
            original = sources.take.side_effect
            sources.take.side_effect = lambda path: (9, metadata(), content, 10) if path == probe.PACKAGE+'files' else original(path)
            with patch.object(probe, 'Sources', return_value=sources), self.assertRaises(RuntimeError):
                probe.capture(base, helper, helper.owned, MANIFEST)
            base.command.assert_not_called()
            self.assertNotIn(probe.CANDIDATE, [c.args[0] for c in sources.take.call_args_list])

    def test_encoder_alias_is_not_the_fixed_candidate(self):
        sources, base, helper = capture_fixture()
        helper.canonical_public.return_value = (probe.CANDIDATE, ['alias'])
        helper.canonical_public.side_effect = None
        with patch.object(probe, 'Sources', return_value=sources), self.assertRaises(RuntimeError):
            probe.capture(base, helper, helper.owned, MANIFEST)
        base.command.assert_not_called()
        self.assertNotIn(probe.CANDIDATE, [c.args[0] for c in sources.take.call_args_list])

    def test_entire_exact_dependency_graph_and_no_old_main(self):
        folder = ROOT/'encoder_static_boundaries'
        for name,digest in transport.PINS.items():
            path = folder/name
            if name == 'containment.py': path = ROOT/'real_resolved_binary/probe.py'
            if name == 'helpers.py': path = ROOT/'static_elf_provenance/probe_four_mib.py'
            if name == 'copy-manifest.json': path = ROOT/'decoder_copy_admission/copy-manifest.json'
            self.assertEqual(hashlib.sha256(path.read_bytes()).hexdigest(),digest)
        self.assertEqual(supervisor.PINS['probe.py'],transport.PINS['probe.py'])
        source = (folder/'probe.py').read_text()
        self.assertNotIn('package_index(',source)
        self.assertNotIn('helpers.capture(',source)
        self.assertNotIn('base.exercise(',source)

    def test_valid_static_data_schema_and_no_candidate_execution(self):
        sources,base,helper = capture_fixture()
        with patch.object(probe,'Sources',return_value=sources):
            value = probe.capture(base,helper,helper.owned,MANIFEST)
        self.assertEqual(validator.validate(value,MANIFEST),value)
        base.command.assert_called_once()
        argv = base.command.call_args.args[0]
        self.assertEqual(argv[1:4],['--wide','--dynamic','--program-headers'])
        self.assertTrue(argv[0].startswith('/proc/self/fd/'))
        self.assertFalse(value['candidate_elf_executed'])
        self.assertFalse(value['allowlist_adoption'])

    def test_unknown_edge_refuses_before_object_open_or_next_tool(self):
        sources,base,helper = capture_fixture()
        helper.decode_readelf.side_effect = None
        helper.decode_readelf.return_value = dict(needed=['unknown.so'],interpreter=None,declared_search_tokens=[])
        helper.resolve_needed.return_value = ('/usr/lib/unknown.so','/usr/lib/unknown.so',[])
        with patch.object(probe,'Sources',return_value=sources), self.assertRaises(RuntimeError):
            probe.capture(base,helper,helper.owned,MANIFEST)
        self.assertEqual(base.command.call_count,1)
        self.assertNotIn('/usr/lib/unknown.so',[c.args[0] for c in sources.take.call_args_list])

    def test_queued_edge_change_refuses_before_next_object_or_command(self):
        sources,base,helper = capture_fixture()
        target = '/usr/lib/libc.so.6'
        helper.decode_readelf.side_effect = None
        helper.decode_readelf.return_value = dict(needed=['libc.so.6'],interpreter=None,declared_search_tokens=[])
        helper.resolve_needed.return_value = (target,target,[])
        helper.canonical_public.side_effect = lambda path:(path, [] if path == probe.CANDIDATE else ['changed'])
        with patch.object(probe,'Sources',return_value=sources), self.assertRaises(RuntimeError):
            probe.capture(base,helper,helper.owned,MANIFEST)
        self.assertEqual(base.command.call_count,1)
        self.assertNotIn(target,[c.args[0] for c in sources.take.call_args_list])

    def test_package_mismatch_unknown_wait_and_bad_header_stop(self):
        sources,base,helper = capture_fixture()
        base.UNSETTLED.append('unknown')
        with patch.object(probe,'Sources',return_value=sources), self.assertRaises(RuntimeError):
            probe.capture(base,helper,helper.owned,MANIFEST)
        base.command.assert_not_called()
        for raw in (b'',b'notELF'*20,ELF[:4]+b'\x01'+ELF[5:],ELF[:18]+b'\x03\x00'+ELF[20:]):
            with self.assertRaises(RuntimeError):
                probe.header(raw)

    def test_strict_nested_receipt_and_json_counterexamples(self):
        good = receipt()
        variants = []
        for key,value in (('records','x'),('aliases','x'),('candidate_elf_executed',0),
                          ('readelf_identity',[31,29149,0o100755,False,0,1,810072,1,1])):
            bad = copy.deepcopy(good); bad[key] = value; variants.append(bad)
        bad = copy.deepcopy(good); bad['records'][0]['identity'][5] = True; variants.append(bad)
        bad = copy.deepcopy(good); bad['records'][0]['path'] = '/private/object'; variants.append(bad)
        bad = copy.deepcopy(good); bad['aliases'][0]['resolved_path'] = '/usr/lib/unknown.so'; variants.append(bad)
        for bad in variants:
            with self.assertRaises((ValueError,TypeError)):
                validator.validate(bad,MANIFEST)
        for raw in (b'{}x',b'NaN',b'{"x":1,"x":2}',b'x'*262145):
            with self.assertRaises(ValueError):
                validator.decode(raw)

    def test_sources_unknown_and_deadline_are_permanent_no_reads(self):
        for failure in ('deadline','unknown'):
            obj = probe.Sources(MANIFEST['source_provenance'])
            if failure == 'deadline': obj.deadline = 0
            with patch.object(probe.os,'open',side_effect=OSError('unknown')):
                with self.assertRaises((RuntimeError,OSError)):
                    obj.take(probe.CANDIDATE)
            obj.deadline = time.monotonic()+100
            with patch.object(probe.os,'open') as opened, patch.object(probe.os,'pread') as read:
                for action in (lambda:obj.take(probe.CANDIDATE),obj.recheck):
                    with self.assertRaises(RuntimeError): action()
                opened.assert_not_called(); read.assert_not_called()

    def test_actual_terminal_wrapper_fragment_no_after_on_failure(self):
        source = (ROOT/'encoder_static_boundaries/vm-guard.sh').read_text()
        fragment = source.split('task_failed=0\n',1)[1].split('check_category() {',1)[0]
        for failed in ('supervisor','validator'):
            with tempfile.TemporaryDirectory(dir='/var/tmp') as directory:
                stage = Path(directory); (stage/'scratch').mkdir()
                for name in ('supervisor','validator'):
                    (stage/(name+'.py')).write_text('raise SystemExit('+('1' if failed==name else '0')+')\n')
                result = subprocess.run(['/bin/bash','-c',fragment+'printf AFTER_FORBIDDEN'],
                                        env={'PATH':'/usr/bin','task_stage':directory},
                                        capture_output=True,timeout=5)
                self.assertNotEqual(result.returncode,0)
                self.assertNotIn(b'AFTER_FORBIDDEN',result.stdout)

    def test_actual_original_fd_replacement_and_change_reversion_seal(self):
        with tempfile.TemporaryDirectory(dir='/var/tmp') as directory:
            root = Path(directory); (root/'usr/lib').mkdir(parents=True)
            target = root/'usr/lib/libbrotlienc.so.1.2.0'
            original_open, original_stat, original_fstat = os.open, os.stat, os.fstat
            def public(value):
                names = ('st_dev','st_ino','st_mode','st_nlink','st_size','st_mtime_ns','st_ctime_ns')
                return SimpleNamespace(**{n:getattr(value,n) for n in names},st_uid=0,st_gid=0)
            with patch.object(probe.os,'open',side_effect=lambda p,*a,**k: original_open(root if p=='/' else p,*a,**k)), \
                 patch.object(probe.os,'stat',side_effect=lambda p,*a,**k: public(original_stat(root if p=='/' else p,*a,**k))), \
                 patch.object(probe.os,'fstat',side_effect=lambda fd:public(original_fstat(fd))):
                for replace in (True,False):
                    target.write_bytes(ELF); target.chmod(0o755)
                    obj = probe.Sources({})
                    try:
                        obj.take(probe.CANDIDATE); obj.recheck()
                        if replace:
                            newer = root/'usr/lib/new'; newer.write_bytes(ELF); newer.chmod(0o755); newer.replace(target)
                        else:
                            target.write_bytes(b'changed'); target.write_bytes(ELF)
                        with self.assertRaises(RuntimeError): obj.recheck()
                        with patch.object(probe.os,'pread') as read:
                            with self.assertRaises(RuntimeError): obj.recheck()
                            read.assert_not_called()
                    finally:
                        for fd in {v[0] for v in (*obj.files.values(),*obj.parents.values())}:
                            os.close(fd)

    def test_supervisor_unknown_nonzero_and_boolean_status_never_follow_up(self):
        s = SimpleNamespace(st_dev=31,st_ino=1,st_mode=0o100500,st_uid=1000,st_gid=1000,
                            st_nlink=1,st_size=4,st_mtime_ns=1,st_ctime_ns=1)
        directory = SimpleNamespace(**dict(vars(s),st_mode=0o40700))
        for code in (1,False,'unknown'):
            child = SimpleNamespace(returncode=code)
            base = Mock(OwnedProcess=Mock(return_value=child))
            base.__dict__['__name__'] = 'synthetic'
            owned = Mock()
            if code == 'unknown': owned.settle.side_effect = RuntimeError('unknown')
            with patch.object(supervisor,'__file__',str(supervisor.STAGE/'supervisor.py')), \
                 patch.object(supervisor,'os',fixed_vm_process_os(supervisor.os)), \
                 patch.object(supervisor.sys,'argv',['supervisor.py','--run-fixed-encoder']), \
                 patch.object(supervisor.os,'open',return_value=9), patch.object(supervisor.os,'close'), \
                 patch.object(supervisor.os,'fstat',return_value=s), patch.object(supervisor.os,'listxattr',return_value=[]), \
                 patch.object(supervisor.os,'pread',return_value=b'pass'), \
                 patch.object(supervisor,'PINS',{n:hashlib.sha256(b'pass').hexdigest() for n in ('containment.py','owned.py')}), \
                 patch.object(supervisor,'types',SimpleNamespace(ModuleType=Mock(side_effect=[base,owned]))), \
                 patch.object(supervisor.Path,'open'), \
                self.assertRaises(RuntimeError):
                self.assertIsInstance(types.ModuleType, type)
                # lstat file must match source, ancestors must be directories.
                with patch.object(supervisor.Path,'lstat',autospec=True,
                                  side_effect=lambda p:s if p.name in ('containment.py','owned.py') else directory):
                    supervisor.run()
            owned.settle.assert_called_once_with(base,child,65)


if __name__ == '__main__':
    unittest.main()
