"""Inert synthetic FD controls; no candidate execution or actual guest input."""
import ast
from contextlib import ExitStack, contextmanager
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

SOURCE=Path(__file__).with_name('artifacts.py')
spec=importlib.util.spec_from_file_location('native_artifacts',SOURCE)
a=importlib.util.module_from_spec(spec);spec.loader.exec_module(a)


class Controls(unittest.TestCase):
    def setUp(self):
        self.clock=patch.object(a.time,'monotonic',return_value=0.0);self.clock.start()
        self.instances=[]

    def tearDown(self):
        for value in self.instances:
            for iterator in value.iterators:iterator.close()
            for fd in reversed(value.held):os.close(fd)
        self.clock.stop()

    def fixture(self, directory):
        root=Path(directory);root.chmod(0o755);artifacts=root/'artifacts';artifacts.mkdir(mode=0o755)
        artifacts.chmod(0o755)  # Explicit namespace staging mode, even under private umask077.
        elf=bytearray(64);elf[:6]=b'\x7fELF\x02\x01';elf[18:20]=b'\x3e\x00'
        core=bytes(elf);broker=core+b'broker';helper=core+b'helper'
        sha={name:hashlib.sha256(raw).hexdigest() for name,raw in
             (('mihomo',core),('omavless-dns-broker',broker),('host-fixture',helper))}
        manifest={'schema':'omavless-composed-developer-artifacts-v1',
            'builder_source':'8c038e76c8407eebd7afdd6e0389fc2bbc28cab9',
            'dns_source':'c4e800425243c1b02165f82153e4bf418fe465e6',
            'architecture':'x86_64','broker_feature':'release-package','sha256':sha,
            'broker_executed':False,'installed_compatibility':False,'package_attestation':False,'effect_authority':False}
        raw=json.dumps(manifest).encode();raw+=b' '*(4181-len(raw))
        contents={'developer-manifest.json':raw,'mihomo':core,'omavless-dns-broker':broker,'host-fixture':helper}
        table={}
        for name,raw in contents.items():
            mode=0o600 if name.endswith('.json') else 0o555
            path=artifacts/name;path.write_bytes(raw);path.chmod(mode)
            table[name]=(len(raw),mode,hashlib.sha256(raw).hexdigest())
        real_open,real_stat=os.open,os.stat
        def opened(name,*args,**kwargs):
            self.assertIn(str(name),('/', 'artifacts', *contents))
            return real_open(root if str(name)=='/' else name,*args,**kwargs)
        def named(name,*args,**kwargs):
            return real_stat(root if str(name)=='/' else name,*args,**kwargs)
        @contextmanager
        def patched():
            with ExitStack() as stack:
                stack.enter_context(patch.object(a,'OWNER',os.getuid()))
                stack.enter_context(patch.object(a,'TABLE',table))
                stack.enter_context(patch.object(a.os,'open',side_effect=opened))
                stack.enter_context(patch.object(a.os,'stat',side_effect=named))
                yield
        return artifacts,patched()

    def construct(self):
        class NativeStore:
            ready=True;sealed=False;artifacts=a;deadline=65.0
            def verify(self,deadline):pass  # Explicit inert custody adapter, not tmpfs evidence.
        value=a.Sources.__new__(a.Sources);self.instances.append(value)
        value.__init__(NativeStore(),SimpleNamespace(NativeStore=NativeStore));return value

    def test_fixed_table_matches_exact_retained_provenance_and_bounds(self):
        base=ast.parse((SOURCE.parents[1]/'real_resolved_binary/probe.py').read_text())
        values={node.targets[0].id:ast.literal_eval(node.value) for node in base.body
                if isinstance(node,ast.Assign) and isinstance(node.targets[0],ast.Name)
                and node.targets[0].id in ('MANIFEST','CORE','BROKER')}
        self.assertEqual(a.TABLE['developer-manifest.json'][2],values['MANIFEST'])
        self.assertEqual(a.TABLE['mihomo'][2],values['CORE'])
        self.assertEqual(a.TABLE['omavless-dns-broker'][2],values['BROKER'])
        self.assertEqual(a.TABLE['host-fixture'][2],'fbd19fc83f5d6548ff8f1fe89d4d66ab9032d85cb551a2364f5719a5cad234f7')
        self.assertEqual(len(a.TABLE),4);self.assertEqual(len(a.ROLES),3)
        self.assertLess(sum(row[0] for row in a.TABLE.values()),128*1024*1024)

    def test_full_original_fd_constructor_catalog_rescan_and_loaded_identity(self):
        with tempfile.TemporaryDirectory() as temp:
            directory,stack=self.fixture(temp)
            with stack:
                value=self.construct();self.assertEqual(value.members(),set(a.TABLE));value.recheck()
                fd,info,sha=value.files['mihomo']
                record=value.mapped_identity('mihomo',info.st_dev,info.st_ino)
                self.assertEqual(record['path'],'/artifacts/mihomo');self.assertEqual(record['sha256'],sha)
                self.assertFalse(value.sealed)

    def test_unknown_native_store_refuses_before_any_artifact_open(self):
        value=a.Sources.__new__(a.Sources);self.instances.append(value)
        with patch.object(a.os,'open') as opened:
            with self.assertRaises(a.Refused):value.__init__(SimpleNamespace(),SimpleNamespace(NativeStore=type))
            opened.assert_not_called()

    def test_native_seal_refusal_precedes_artifact_metadata_and_hash(self):
        with tempfile.TemporaryDirectory() as temp:
            _,stack=self.fixture(temp)
            with stack:
                value=self.construct()
                with patch.object(value.native,'verify',side_effect=RuntimeError('synthetic')), \
                     patch.object(a.os,'stat') as named,patch.object(a.os,'pread') as hashed:
                    with self.assertRaises(a.Refused):value.recheck()
                    named.assert_not_called();hashed.assert_not_called()
                self.assertTrue(value.sealed)

    def test_mapping_caller_deadline_is_shared_through_native_seal(self):
        with tempfile.TemporaryDirectory() as temp:
            _,stack=self.fixture(temp)
            with stack:
                value=self.construct()
                with patch.object(value.native,'verify') as seal:
                    value.recheck(5.0)
                    seal.assert_called_once_with(5.0)
                for invalid in (True,1,float('nan'),float('inf'),66.0,0.0):
                    # Independent bounded adapter objects, never retry refusal.
                    fresh=self.construct()
                    with patch.object(fresh.native,'verify') as seal:
                        with self.assertRaises(a.Refused):fresh.recheck(invalid)
                        seal.assert_not_called()

    def test_inert_namespace_staging_explicit_modes_under_private_umask(self):
        previous=os.umask(0o077)
        try:
            with tempfile.TemporaryDirectory() as temp:
                directory,stack=self.fixture(temp)
                self.assertEqual(directory.stat().st_mode&0o777,0o755)
                with stack:value=self.construct();value.recheck();self.assertFalse(value.sealed)
        finally:os.umask(previous)

    def test_member_ambiguity_prevents_any_elf_open(self):
        with tempfile.TemporaryDirectory() as temp:
            directory,stack=self.fixture(temp);(directory/'unexpected').touch()
            with stack,patch.object(a.os,'pread') as read:
                with self.assertRaises(a.Refused):value=self.construct()
                read.assert_not_called()
            self.assertTrue(self.instances[-1].sealed)

    def test_same_bytes_replacement_recheck_permanently_seals(self):
        with tempfile.TemporaryDirectory() as temp:
            directory,stack=self.fixture(temp)
            with stack:
                value=self.construct();path=directory/'mihomo';raw=path.read_bytes()
                path.unlink();path.write_bytes(raw);path.chmod(0o555)
                with self.assertRaises(a.Refused):value.recheck()
                with patch.object(a.os,'pread') as read:
                    with self.assertRaises(a.Refused):value.recheck()
                    read.assert_not_called()
                self.assertTrue(value.sealed)

    def test_wrong_mapped_device_inode_membership_or_type_precedes_hash(self):
        for name,device,inode in (('mihomo',1,2),('unknown',1,2),('mihomo',True,2)):
            with tempfile.TemporaryDirectory() as temp:
                directory,stack=self.fixture(temp)
                with stack:
                    value=self.construct()
                    with patch.object(value,'recheck') as hashed:
                        with self.assertRaises(a.Refused):value.mapped_identity(name,device,inode)
                        hashed.assert_not_called()
                    self.assertTrue(value.sealed)

    def test_fifo_and_mismatched_pin_refuse_without_content_read(self):
        for variant in ('fifo','pin'):
            with tempfile.TemporaryDirectory() as temp:
                directory,stack=self.fixture(temp)
                if variant=='fifo':
                    path=directory/'developer-manifest.json';path.unlink();os.mkfifo(path,0o600)
                with stack:
                    if variant=='pin':a.TABLE['developer-manifest.json']=(4181,0o600,'0'*64)
                    with self.assertRaises(a.Refused):self.construct()
                self.assertTrue(self.instances[-1].sealed)

    def test_invalid_initial_clock_never_opens_or_late_read_follows(self):
        for clock in (float('inf'),float('nan'),True,1,1e308):
            with patch.object(a.time,'monotonic',return_value=clock),patch.object(a.os,'open') as opened:
                with self.assertRaises(a.Refused):self.construct()
                opened.assert_not_called()
            self.assertTrue(self.instances[-1].sealed)

    def test_late_read_closes_no_original_and_never_retries(self):
        value=a.Sources.__new__(a.Sources);value.sealed=False;value.deadline=90.0;value.local_budget=15.0
        clock=[0.0]
        def late(*args):clock[0]=15.0;return b'x'
        with patch.object(a.time,'monotonic',side_effect=lambda:clock[0]),patch.object(a.os,'pread',side_effect=late) as read:
            with self.assertRaises(a.Refused):value.digest(17,2)
            self.assertEqual(read.call_count,1);clock[0]=0.0
            with self.assertRaises(a.Refused):value.digest(17,2)
            self.assertEqual(read.call_count,1)

    def test_no_candidate_exec_write_cleanup_or_arbitrary_path_surface(self):
        forbidden={'exec','eval','spawn','kill','waitid','waitpid','system','write','unlink','rmdir','mkdir','chmod','chown'}
        for node in ast.walk(ast.parse(SOURCE.read_text())):
            if isinstance(node,ast.Call):
                name=node.func.id if isinstance(node.func,ast.Name) else node.func.attr if isinstance(node.func,ast.Attribute) else ''
                self.assertNotIn(name,forbidden)


if __name__=='__main__':unittest.main()
