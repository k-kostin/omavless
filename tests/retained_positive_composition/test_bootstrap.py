"""Inert fixed original-socket permission controls; no chmod/socket/VM occurs."""
import ast
import importlib.util
from pathlib import Path
import stat
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, patch

SOURCE=Path(__file__).with_name('bootstrap.py')
spec=importlib.util.spec_from_file_location('retained_bootstrap',SOURCE)
b=importlib.util.module_from_spec(spec);spec.loader.exec_module(b)

class Session:pass
class Child:pass
class Images:pass


class Controls(unittest.TestCase):
    def fixture(self):
        owner=Session();owner.kind='inner';owner.isolated=True;owner.sealed=False;owner.retained=[]
        core=Child();owner.children=[core];owner.roles={id(core):'core'}
        owner.anchors={'core':{'child':core,'state':'spawned'}};clock=[0.0]
        owner.available=Mock();owner.live=Mock();owner.local_deadline=Mock(return_value=5.0)
        def within(deadline):
            if clock[0]>=deadline:raise RuntimeError('private late')
        owner.within=Mock(side_effect=within)
        ownership=SimpleNamespace(Session=Session,OwnedProcess=Child,clock=Mock(side_effect=lambda:clock[0]))
        images=Images();images.owner=owner;images.executable=Mock()
        def info(inode,mode,uid):
            return SimpleNamespace(st_dev=1,st_ino=inode,st_mode=mode,st_uid=uid,st_gid=uid,st_nlink=1 if stat.S_ISSOCK(mode) else 2)
        values={71:info(71,stat.S_IFDIR|0o700,1000),72:info(72,stat.S_IFSOCK|0o666,1000),
                73:info(73,stat.S_IFDIR|0o500,0)}
        def named(path,**kwargs):return values[{b.DIRECTORY:71,'controller.sock':72,'/proc/1/fd':73,'72':72}[path]]
        def chmod(path,mode,**kwargs):
            self.assertEqual((path,mode,kwargs),('72',0o600,{'dir_fd':73}))
            values[72].st_mode=stat.S_IFSOCK|mode
        return owner,ownership,images,values,named,chmod,clock

    def context(self,fixture,**overrides):
        owner,ownership,images,values,named,chmod,clock=fixture
        return (patch.object(b.os,'getpid',return_value=1),patch.object(b.os,'geteuid',return_value=0),
                patch.object(b.os,'getegid',return_value=0),
                patch.object(b.os,'open',**overrides.get('opened',{'side_effect':[71,72,73]})),
                patch.object(b.os,'fstat',side_effect=lambda fd:values[fd]),
                patch.object(b.os,'stat',side_effect=overrides.get('named',named)),
                patch.object(b.os,'chmod',side_effect=overrides.get('chmod',chmod)))

    def construct(self,fixture):
        owner,ownership,images,*_=fixture
        return b.Bootstrap(owner,ownership,images,SimpleNamespace(Images=Images))

    def test_known_original_mode_only_chmods_kernel_alias_and_repeats_all_checks(self):
        fixture=self.fixture();contexts=self.context(fixture)
        with contexts[0],contexts[1],contexts[2],contexts[3] as opened,contexts[4],contexts[5],contexts[6] as chmod:
            value=self.construct(fixture);receipt=value.receipt()
        self.assertEqual(value.held,[71,72,73]);self.assertTrue(value.done);self.assertFalse(value.sealed)
        self.assertEqual(receipt,{'fresh_original_controller_socket_mode_0600':True,'production_effect_authority':False})
        self.assertEqual(opened.call_args_list[1].args,('controller.sock',b.FLAGS))
        self.assertEqual(opened.call_args_list[1].kwargs,{'dir_fd':71})
        chmod.assert_called_once_with('72',0o600,dir_fd=73)
        self.assertIs(fixture[0].controller_bootstrap,value);self.assertIn(value,fixture[0].retained)
        self.assertGreaterEqual(fixture[2].executable.call_count,3)

    def test_wrong_private_parent_precedes_socket_open_and_chmod(self):
        for variant in ('uid','gid','mode','kind','alias_bool'):
            fixture=self.fixture();values=fixture[3]
            if variant=='uid':values[71].st_uid=0
            if variant=='gid':values[71].st_gid=0
            if variant=='mode':values[71].st_mode=stat.S_IFDIR|0o755
            if variant=='kind':values[71].st_mode=stat.S_IFREG|0o700
            if variant=='alias_bool':values[71].st_uid=True
            contexts=self.context(fixture)
            with contexts[0],contexts[1],contexts[2],contexts[3] as opened,contexts[4],contexts[5],contexts[6] as chmod:
                with self.assertRaises(b.Refused):self.construct(fixture)
                self.assertEqual(opened.call_count,1);chmod.assert_not_called()
            self.assertTrue(fixture[0].sealed)

    def test_wrong_socket_owner_link_mode_or_kernel_alias_before_chmod(self):
        for variant in ('uid','gid','links','mode','kind','alias'):
            fixture=self.fixture();values=fixture[3];original=fixture[4]
            if variant=='uid':values[72].st_uid=0
            if variant=='gid':values[72].st_gid=0
            if variant=='links':values[72].st_nlink=2
            if variant=='mode':values[72].st_mode=stat.S_IFSOCK|0o600
            if variant=='kind':values[72].st_mode=stat.S_IFREG|0o666
            def named(path,**kwargs):
                value=original(path,**kwargs)
                if variant=='alias' and path=='72':return SimpleNamespace(**(vars(value)|{'st_ino':99}))
                return value
            contexts=self.context(fixture,named=named)
            with contexts[0],contexts[1],contexts[2],contexts[3],contexts[4],contexts[5],contexts[6] as chmod:
                with self.assertRaises(b.Refused):self.construct(fixture)
                chmod.assert_not_called()
            self.assertTrue(fixture[0].sealed)

    def test_named_aba_before_or_after_chmod_never_accepts_replacement(self):
        for variant in ('before','after'):
            fixture=self.fixture();original=fixture[4];mutated=[variant=='before']
            def named(path,**kwargs):
                value=original(path,**kwargs)
                if path=='controller.sock' and mutated[0]:return SimpleNamespace(**(vars(value)|{'st_ino':99}))
                return value
            def chmod(*args,**kwargs):fixture[5](*args,**kwargs);mutated[0]=True
            contexts=self.context(fixture,named=named,chmod=chmod)
            with contexts[0],contexts[1],contexts[2],contexts[3],contexts[4],contexts[5],contexts[6] as op:
                with self.assertRaises(b.Refused):self.construct(fixture)
                self.assertEqual(op.call_count,0 if variant=='before' else 1)
            value=fixture[0].controller_bootstrap
            self.assertFalse(value.done);self.assertTrue(value.sealed and fixture[0].sealed)

    def test_late_throw_non_none_or_no_effect_chmod_has_no_followup_or_retry(self):
        for variant in ('late','throw','alias','unchanged'):
            fixture=self.fixture();clock=fixture[-1];calls=[];original=fixture[4]
            def named(*args,**kwargs):calls.append('stat');return original(*args,**kwargs)
            def chmod(*args,**kwargs):
                calls.append('chmod')
                if variant=='throw':raise RuntimeError('private')
                if variant!='unchanged':fixture[5](*args,**kwargs)
                if variant=='late':clock[0]=5.0
                return True if variant=='alias' else None
            contexts=self.context(fixture,named=named,chmod=chmod)
            with contexts[0],contexts[1],contexts[2],contexts[3] as opened,contexts[4],contexts[5],contexts[6] as op:
                with self.assertRaises(b.Refused):self.construct(fixture)
                if variant in ('late','throw','alias'):self.assertEqual(calls[-1],'chmod')
                count=len(calls);clock[0]=0.0
                value=fixture[0].controller_bootstrap
                with self.assertRaises(b.Refused):value.check(0o600)
                self.assertEqual(len(calls),count);op.assert_called_once()
                with self.assertRaises(b.Refused):self.construct(fixture)
                self.assertEqual(opened.call_count,3)
            self.assertFalse(value.done);self.assertTrue(fixture[0].sealed)

    def test_late_open_retained_before_any_metadata_or_chmod(self):
        fixture=self.fixture();clock=fixture[-1]
        def opened(*args,**kwargs):clock[0]=5.0;return 71
        contexts=self.context(fixture,opened={'side_effect':opened})
        with contexts[0],contexts[1],contexts[2],contexts[3],contexts[4] as metadata,contexts[5],contexts[6] as chmod:
            with self.assertRaises(b.Refused):self.construct(fixture)
            metadata.assert_not_called();chmod.assert_not_called()
        self.assertEqual(fixture[0].controller_bootstrap.held,[71]);self.assertTrue(fixture[0].sealed)

    def test_wrong_concrete_owner_core_phase_or_image_before_any_socket_open(self):
        for variant in ('isolation','kind','core','phase','role','image','already_used'):
            fixture=self.fixture();owner=fixture[0]
            if variant=='isolation':owner.isolated=False
            if variant=='kind':owner.kind='outer'
            if variant=='core':owner.anchors['core']['child']=object()
            if variant=='phase':owner.anchors['core']['state']='mapped'
            if variant=='role':owner.roles={}
            if variant=='image':fixture[2].executable.side_effect=RuntimeError('private')
            if variant=='already_used':owner.controller_bootstrap=object()
            contexts=self.context(fixture)
            with contexts[0],contexts[1],contexts[2],contexts[3] as opened,contexts[4],contexts[5],contexts[6] as chmod:
                with self.assertRaises(b.Refused):self.construct(fixture)
                opened.assert_not_called();chmod.assert_not_called()

    def test_invalid_internal_clock_before_next_effect_and_source_no_failure_close(self):
        for bad in (True,0,float('inf'),float('nan')):
            fixture=self.fixture();fixture[1].clock=Mock(return_value=bad);contexts=self.context(fixture)
            with contexts[0],contexts[1],contexts[2],contexts[3] as opened,contexts[4],contexts[5],contexts[6] as chmod:
                with self.assertRaises(b.Refused):self.construct(fixture)
                opened.assert_not_called();chmod.assert_not_called()
        for node in ast.walk(ast.parse(SOURCE.read_text())):
            if isinstance(node,ast.Try):self.assertEqual(node.finalbody,[])
            if isinstance(node,ast.Call):
                name=node.func.id if isinstance(node.func,ast.Name) else node.func.attr if isinstance(node.func,ast.Attribute) else ''
                self.assertNotIn(name,{'close','unlink','kill','waitid','waitpid','system','exec','eval','readlink'})


if __name__=='__main__':unittest.main()
