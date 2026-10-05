"""Inert fixed-source graph controls; no fixture/native/process/namespace call."""
import ast
from contextlib import ExitStack
import importlib.util
import os
from pathlib import Path
import stat
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import Mock,patch

HERE=Path(__file__).parent
spec=importlib.util.spec_from_file_location('retained_graph',HERE/'graph.py')
g=importlib.util.module_from_spec(spec);spec.loader.exec_module(g)
SOURCES={name:HERE/name for name in ('lifecycle.py','artifacts.py','images.py','controller.py',
    'helper.py','positive.py','streams.py','bootstrap.py','native_copy.py')}
SOURCES.update({'bridge.py':HERE.parent/'six_library_live_mapping/bridge.py',
    'admission.py':HERE.parent/'six_library_copy_admission/admission.py',
    'copy-manifest.json':HERE.parent/'six_library_copy_admission/copy-manifest.json',
    'containment.py':HERE.parent/'real_resolved_binary/probe.py',
    'guest-inventory.json':HERE.parent/'real_resolved_binary/guest-inventory.json'})


class Controls(unittest.TestCase):
    def setUp(self):
        # /tmp is correctly rejected by the production original-ancestor
        # contract. Keep inert fixtures in safe owned HOME ancestry even when
        # CI has no TMPDIR; never translate away its writable-ancestor check.
        cache=Path.home()/'.cache';cache.mkdir(mode=0o700,exist_ok=True)
        self.temp=tempfile.TemporaryDirectory(prefix='t3graph.',dir=cache);self.stage=Path(self.temp.name)
        self.stage.chmod(0o700)
        for name,path in SOURCES.items():
            target=self.stage/name;target.write_bytes(path.read_bytes());target.chmod(0o600)
        for name in g.EXTRA:
            target=self.stage/name
            if name in ('native','scratch'):target.mkdir(mode=0o700)
            else:target.write_bytes(b'fixed outside-chain metadata only\n');target.chmod(0o600)
        self.values=[];self.stack=ExitStack();self.clock=[0.0]
        self.stack.enter_context(patch.object(g,'STAGE',str(self.stage)))
        self.stack.enter_context(patch.object(g.time,'monotonic',side_effect=lambda:self.clock[0]))
        # Translate only current HOST fixture ownership to the fixed VM user.
        # CI UID is not VM authority; no production predicate is loosened.
        host_uid,host_gid=os.getuid(),os.getgid();real_fstat=os.fstat;real_stat=os.stat
        root=real_stat('/');root_identity=(root.st_dev,root.st_ino)
        def metadata(value):
            # os is shared with unittest/linecache. Preserve its COMPLETE stat
            # projection, including float timestamps used by Python3.12 error
            # reporting; only fixture ownership is translated.
            fields={key:getattr(value,key) for key in dir(value) if key.startswith('st_')}
            if (fields['st_dev'],fields['st_ino'])!=root_identity:
                if fields['st_uid']==host_uid:fields['st_uid']=1000
                if fields['st_gid']==host_gid:fields['st_gid']=1000
            return SimpleNamespace(**fields)
        self.stack.enter_context(patch.object(g.os,'getresuid',return_value=(1000,1000,1000)))
        self.stack.enter_context(patch.object(g.os,'getresgid',return_value=(1000,1000,1000)))
        self.stack.enter_context(patch.object(g.os,'getuid',return_value=1000))
        self.stack.enter_context(patch.object(g.os,'fstat',side_effect=lambda fd:metadata(real_fstat(fd))))
        self.stack.enter_context(patch.object(g.os,'stat',side_effect=lambda *a,**k:metadata(real_stat(*a,**k))))

    def tearDown(self):
        self.stack.close()
        for value in self.values:
            for stream in value.iterators:stream.close()
            for fd in value.held:os.close(fd)
        self.temp.cleanup()

    def fixture(self,deadline=90.0):
        value=g.Graph.__new__(g.Graph);self.values.append(value);value.__init__(deadline);return value

    def sealed(self,value):
        self.assertTrue(value.sealed)
        with patch.object(g.os,'open') as opened,patch.object(g.os,'stat') as named:
            with self.assertRaises(g.Refused):value.recheck()
            opened.assert_not_called();named.assert_not_called()

    def test_exact_fourteen_pins_originals_and_three_rescans_then_fixed_definitions(self):
        value=self.fixture()
        self.assertEqual(set(value.raw),set(g.PINS));self.assertEqual(len(value.records),14)
        value.recheck();modules=value.load()
        self.assertEqual(set(modules),{name for name in g.PINS if name.endswith('.py')})
        base=modules['containment.py']
        for name in ('exercise','stop','supervise','quarantine','main','OwnedProcess','command',
                     'child_status','no_directory_fds','controller','echo_server','snapshot'):
            self.assertFalse(hasattr(base,name),name)
        self.assertEqual({name for name in g.BASE_FUNCTIONS if hasattr(base,name)},g.BASE_FUNCTIONS)
        self.assertEqual(base.ENV['TMPDIR'],'/tmp')
        self.assertFalse(value.sealed)

    def test_changed_pin_missing_extra_unknown_catalog_fifo_and_hardlink_refuse_before_load(self):
        for variant in ('changed','missing','unknown','fifo','hardlink'):
            with self.subTest(variant=variant):
                target=self.stage/'images.py';original=target.read_bytes()
                if variant=='changed':target.write_bytes(original+b'\n')
                elif variant=='missing':(self.stage/'vm-guard.sh').unlink()
                elif variant=='unknown':(self.stage/'private-name').write_bytes(b'private')
                elif variant=='fifo':target.unlink();os.mkfifo(target,0o600)
                else:os.link(target,self.stage/'extra-link')
                with patch.object(g,'containment_tree') as compile_source:
                    with self.assertRaisesRegex(g.Refused,'^fixed_positive_graph_refused$'):self.fixture()
                    compile_source.assert_not_called()
                if variant in ('changed','fifo'):
                    target.unlink();target.write_bytes(original);target.chmod(0o600)
                elif variant=='missing':(self.stage/'vm-guard.sh').write_bytes(b'fixed');(self.stage/'vm-guard.sh').chmod(0o600)
                elif variant=='unknown':(self.stage/'private-name').unlink()
                else:(self.stage/'extra-link').unlink()

    def test_complete_recheck_detects_replaced_original_before_any_definition_exec(self):
        value=self.fixture();target=self.stage/'images.py';target.rename(self.stage/'parked')
        target.write_bytes(SOURCES['images.py'].read_bytes());target.chmod(0o600)
        with patch.object(g,'containment_tree') as compile_source:
            with self.assertRaises(g.Refused):value.load()
            compile_source.assert_not_called()
        self.sealed(value)

    def test_stat_mock_retains_reporting_fields_and_safe_fixture_ancestry(self):
        value=os.stat(self.stage)
        for name in ('st_mtime','st_mtime_ns','st_ctime','st_atime','st_size'):
            self.assertTrue(hasattr(value,name),name)
        self.assertEqual(self.stage.parent,Path.home()/'.cache')
        self.assertEqual(stat.S_IMODE(value.st_mode),0o700)

    def test_mutated_saved_bytes_refuse_before_definition_loading(self):
        value=self.fixture();value.raw['images.py']=b'raise RuntimeError("private")'
        with patch.object(g,'containment_tree') as compile_source:
            with self.assertRaises(g.Refused):value.load()
            compile_source.assert_not_called()
        self.sealed(value)

    def test_nonfinite_alias_initial_clock_precedes_any_source_open(self):
        for bad in (True,1,float('nan'),float('inf')):
            self.clock[0]=bad
            with patch.object(g.os,'open') as opened:
                with self.assertRaises(g.Refused):self.fixture()
                opened.assert_not_called()

    def test_late_first_open_retains_fd_before_metadata_and_never_retries(self):
        value=g.Graph.__new__(g.Graph);self.values.append(value);real_open=os.open
        def opened(*args,**kwargs):
            fd=real_open(*args,**kwargs);self.clock[0]=20.0;return fd
        with patch.object(g.os,'open',side_effect=opened) as opening,patch.object(g.os,'fstat') as info:
            with self.assertRaises(g.Refused):value.__init__(90.0)
            info.assert_not_called();self.assertEqual(len(value.held),1)
            self.clock[0]=0.0
            with self.assertRaises(g.Refused):value.load()
            self.assertEqual(opening.call_count,1)

    def test_positive_directory_release_closes_only_known_dirs_and_retains_fourteen_files(self):
        value=self.fixture();value.load();originals={fd for fd,_ in value.records.values()}
        value.release_directories_positive()
        self.assertEqual(set(value.held),originals);self.assertEqual(len(value.held),14)
        for fd in value.held:self.assertTrue(stat.S_ISREG(os.fstat(fd).st_mode))
        with patch.object(g.os,'close') as closing:
            with self.assertRaises(g.Refused):value.release_directories_positive()
            closing.assert_not_called()

    def test_late_or_throwing_positive_close_prevents_next_close_and_retry(self):
        for variant in ('late','throw'):
            value=self.fixture();value.load();real_close=os.close;closed=[]
            def closing(fd):
                if variant=='throw':raise OSError('private')
                real_close(fd);closed.append(fd);self.clock[0]=20.0
            with patch.object(g.os,'close',side_effect=closing) as close:
                with self.assertRaises(g.Refused):value.release_directories_positive()
                self.assertEqual(close.call_count,1);self.clock[0]=0.0
                with self.assertRaises(g.Refused):value.release_directories_positive()
                self.assertEqual(close.call_count,1)
            for fd in closed:value.held.remove(fd)

    def test_whole_original_containment_hash_and_exact_whitelist_precede_compile(self):
        raw=SOURCES['containment.py'].read_bytes();tree=g.containment_tree(raw)
        names={node.name for node in tree.body if isinstance(node,(ast.ClassDef,ast.FunctionDef))}
        self.assertEqual(names,g.BASE_FUNCTIONS|{'Refused'})
        with self.assertRaises(g.Refused):g.containment_tree(raw+b'\n')
        with patch.object(g,'BASE_FUNCTIONS',g.BASE_FUNCTIONS|{'invented'}):
            with self.assertRaises(g.Refused):g.containment_tree(raw)

    def test_enclosing_deadline_aliases_and_expiry_precede_any_source_open(self):
        for bad in (True,1,float('nan'),float('inf'),0.0,-1.0):
            with patch.object(g.os,'open') as opened:
                with self.assertRaises(g.Refused):self.fixture(bad)
                opened.assert_not_called()

    def test_enclosing_deadline_caps_internal_read_and_permanently_stops_next_io(self):
        real_read=os.pread;value=g.Graph.__new__(g.Graph);self.values.append(value)
        def reading(*args):
            data=real_read(*args);self.clock[0]=3.0;return data
        with patch.object(g.os,'pread',side_effect=reading) as read:
            with self.assertRaises(g.Refused):value.__init__(3.0)
            self.assertEqual(value.deadline,3.0);self.assertEqual(read.call_count,1)
            self.clock[0]=0.0
            with patch.object(g.os,'open') as opened:
                with self.assertRaises(g.Refused):value.load()
                opened.assert_not_called();self.assertEqual(read.call_count,1)

    def test_no_candidate_process_namespace_write_or_failure_cleanup_entry(self):
        tree=ast.parse((HERE/'graph.py').read_bytes())
        calls={node.func.attr if isinstance(node.func,ast.Attribute) else node.func.id
            for node in ast.walk(tree) if isinstance(node,ast.Call)
            and isinstance(node.func,(ast.Name,ast.Attribute))}
        self.assertFalse(calls & {'spawn','Popen','kill','waitid','waitpid','chroot','mkdir',
            'write','unlink','rmdir','chmod','chown','system','run'})
        self.assertNotIn('__main__',(HERE/'graph.py').read_text())


if __name__=='__main__':unittest.main()
