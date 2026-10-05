"""Inert mapping originals/identity controls; no process or candidate execution."""
import ast
import importlib.util
import os
import stat
from pathlib import Path
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, patch

SOURCE=Path(__file__).with_name('images.py')
spec=importlib.util.spec_from_file_location('retained_images',SOURCE)
i=importlib.util.module_from_spec(spec);spec.loader.exec_module(i)


class Session:
    def __init__(self):
        self.kind='inner';self.sealed=False;self.children=[];self.anchors={}
        self.available=Mock();self.live=Mock()


class Child:pass
class Bridge:pass
class Artifacts:pass


def mapping(paths):
    return ''.join(f'{index*4096:x}-{(index+1)*4096:x} r-xp 00000000 00:1f {inode} {path}\n'
                   for index,(path,inode) in enumerate(paths,1))


class Controls(unittest.TestCase):
    def setUp(self):
        self.clock=patch.object(i.time,'monotonic',return_value=0.0);self.clock.start()
        self.values=[]

    def tearDown(self):
        for value in self.values:
            for fd in value.held:os.close(fd)
        self.clock.stop()

    def fixture(self,role='core'):
        owner=Session();child=Child();owner.children=[child]
        owner.anchors[role]={'child':child,'proc_fd':71}
        ownership=SimpleNamespace(Session=Session,OwnedProcess=Child)
        copies=Bridge();copies.state='ready';copies.verify=Mock();copies._verify_target=Mock()
        names=['/usr/bin/dbus-daemon','/usr/lib/systemd/systemd-resolved',
               '/usr/lib/libc.so.6','/usr/lib/ld-linux-x86-64.so.2']
        names.extend(f'/usr/lib/fixed{index}.so' for index in range(21))
        copies.records={path:{'device':31,'inode':index,'size':64,'sha256':'a'*64}
                        for index,path in enumerate(names,1)}
        copies.fds={path:100+index for index,path in enumerate(names)}
        artifacts=Artifacts();artifacts.sealed=False;artifacts.recheck=Mock()
        artifacts.files={name:(55,SimpleNamespace(st_dev=31,st_ino=90),'b'*64)
                         for name in ('developer-manifest.json','mihomo','omavless-dns-broker','host-fixture')}
        def native(name,device,inode):
            self.assertEqual((device,inode),(31,90))
            return {'path':'/artifacts/'+name,'device':device,'inode':inode,'size':64,'sha256':'b'*64}
        artifacts.mapped_identity=Mock(side_effect=native)
        value=i.Images(owner,ownership,copies,SimpleNamespace(Bridge=Bridge),artifacts,
                       SimpleNamespace(Sources=Artifacts,TABLE=artifacts.files))
        value.executable=Mock(return_value=(31,90 if role in ('core','broker','host')
                                           else copies.records[i.ROLES[role]]['inode']))
        self.values.append(value)
        return value,child

    def maps(self,value,role):
        paths=[(i.ROLES[role],90)] if role in ('core','broker','host') else []
        paths.extend((path,row['inode']) for path,row in value.copies.records.items()
                     if path==i.ROLES[role] or path in ('/usr/lib/libc.so.6','/usr/lib/ld-linux-x86-64.so.2'))
        return mapping(paths)

    def sealed(self,value):
        self.assertTrue(value.sealed and value.owner.sealed and value.artifacts.sealed)
        self.assertEqual(value.copies.state,'refused')

    def test_all_five_role_loaded_objects_match_then_complete_reread(self):
        for role in i.ROLES:
            value,child=self.fixture(role);raw=self.maps(value,role)
            with patch.object(value,'text',side_effect=[raw,raw]) as read:
                result=value.inventory(child,5.0,'initial_'+role)
            self.assertEqual(read.call_count,2)
            self.assertIn(i.ROLES[role],{row['path'] for row in result})
            self.assertFalse(value.sealed)
            self.assertEqual(value.artifacts.mapped_identity.call_count,int(role in ('core','broker','host')))

    def test_unknown_path_wrong_copy_identity_or_other_native_never_hashes(self):
        for path,inode in (('/usr/lib/unadmitted.so',5),('/usr/lib/libc.so.6',999),
                           ('/artifacts/host-fixture',90),('/artifacts/mihomo',91)):
            value,child=self.fixture();raw=mapping([(path,inode)])
            if path=='/artifacts/mihomo':value.artifacts.mapped_identity.side_effect=i.Refused()
            with patch.object(value,'text',return_value=raw):
                with self.assertRaises(i.Refused):value.inventory(child,5.0)
            value.copies._verify_target.assert_not_called()
            if path!='/artifacts/mihomo':value.artifacts.mapped_identity.assert_not_called()
            self.sealed(value)

    def test_changed_complete_mapping_or_live_anchor_permanently_seals(self):
        for variant in ('map','live'):
            value,child=self.fixture();raw=self.maps(value,'core')
            after=raw.replace(' 90 /artifacts',' 91 /artifacts') if variant=='map' else raw
            if variant=='live':value.owner.live.side_effect=RuntimeError('private must not escape')
            with patch.object(value,'text',side_effect=[raw,after]):
                with self.assertRaisesRegex(i.Refused,'^fixed_loaded_image_refused$'):value.inventory(child,5.0)
            self.sealed(value)
            with patch.object(value,'text') as read:
                with self.assertRaises(i.Refused):value.inventory(child,5.0)
                read.assert_not_called()

    def test_concrete_child_membership_role_and_context_precede_maps_read(self):
        for variant in ('type','role','duplicate','context'):
            value,child=self.fixture()
            if variant=='type':child=SimpleNamespace()
            if variant=='role':value.owner.anchors={'wrong':value.owner.anchors['core']}
            if variant=='duplicate':value.owner.anchors['host']=value.owner.anchors['core']
            with patch.object(value,'text') as read:
                with self.assertRaises(i.Refused):value.inventory(child,5.0,'final_host' if variant=='context' else None)
                read.assert_not_called()
            self.sealed(value)

    def test_original_proc_fd_literal_maps_two_reads_no_candidate_open(self):
        value,child=self.fixture();raw=self.maps(value,'core').encode()
        with tempfile.TemporaryDirectory() as temp:
            path=Path(temp)/'maps';path.write_bytes(raw)
            directory=os.open(temp,os.O_RDONLY|os.O_DIRECTORY)
            try:
                value.owner.anchors['core']['proc_fd']=directory
                real_open=os.open
                def opened(name,flags,**kwargs):
                    self.assertEqual(name,'maps');self.assertEqual(kwargs,{'dir_fd':directory})
                    self.assertEqual(flags,i.FLAGS);return real_open(name,flags,**kwargs)
                with patch.object(i.os,'open',side_effect=opened) as read:
                    result=value.inventory(child,5.0)
                self.assertEqual(read.call_count,2);self.assertEqual(len(value.held),2)
                self.assertEqual(len(result),3)
            finally:os.close(directory)

    def test_maps_open_throw_or_late_retains_descriptor_and_never_fstats(self):
        for variant in ('throw','late'):
            value,child=self.fixture();clock=[0.0]
            with tempfile.TemporaryFile() as temp:
                fd=os.dup(temp.fileno())
                def opened(*args,**kwargs):
                    if variant=='throw':raise RuntimeError('private')
                    clock[0]=5.0;return fd
                with patch.object(i.time,'monotonic',side_effect=lambda:clock[0]), \
                     patch.object(i.os,'open',side_effect=opened) as opened_mock,patch.object(i.os,'fstat') as metadata:
                    with self.assertRaises(i.Refused):value.text(child,5.0)
                    metadata.assert_not_called();clock[0]=0.0
                    with self.assertRaises(i.Refused):value.text(child,5.0)
                    self.assertEqual(opened_mock.call_count,1)
                if variant=='throw':os.close(fd)
                else:self.assertIn(fd,value.held)
            self.sealed(value)

    def test_nonfinite_wrong_type_deadline_or_clock_precedes_maps_open(self):
        for bad in (True,1,float('nan'),float('inf')):
            for variant in ('deadline','clock'):
                value,child=self.fixture()
                with patch.object(i.time,'monotonic',return_value=bad if variant=='clock' else 0.0), \
                     patch.object(i.os,'open') as opened:
                    with self.assertRaises(i.Refused):value.text(child,bad if variant=='deadline' else 5.0)
                    opened.assert_not_called()
                self.sealed(value)

    def test_late_read_or_exception_prevents_next_io_after_clock_recovers(self):
        for variant in ('late','throw'):
            value,child=self.fixture();clock=[0.0];operation=Mock()
            def read():
                if variant=='throw':raise RuntimeError('private')
                clock[0]=5.0;return b'private'
            with patch.object(i.time,'monotonic',side_effect=lambda:clock[0]):
                with self.assertRaises(i.Refused):value.io(5.0,read)
                clock[0]=0.0
                with self.assertRaises(i.Refused):value.io(5.0,operation)
                operation.assert_not_called()
            self.sealed(value)

    def test_missing_executable_or_daemon_required_loader_refuses(self):
        for role in ('core','bus','resolved'):
            value,child=self.fixture(role)
            raw=mapping([('/usr/lib/libc.so.6',3)]) if role=='core' else mapping([(i.ROLES[role],1 if role=='bus' else 2)])
            with patch.object(value,'text',return_value=raw):
                with self.assertRaises(i.Refused):value.inventory(child,5.0)
            value.copies._verify_target.assert_not_called();value.artifacts.mapped_identity.assert_not_called()
            self.sealed(value)

    def test_whole_batch_bad_late_member_or_identity_precedes_any_target_hash(self):
        for bad,inode in (('/usr/lib/zz-unadmitted.so',90),('/usr/lib/libc.so.6',999),
                          ('/artifacts/host-fixture',90)):
            value,child=self.fixture()
            raw=mapping([('/artifacts/mihomo',90),('/usr/lib/ld-linux-x86-64.so.2',4),(bad,inode)])
            with patch.object(value,'text',return_value=raw):
                with self.assertRaises(i.Refused):value.inventory(child,5.0)
            value.copies._verify_target.assert_not_called();value.artifacts.mapped_identity.assert_not_called()
            self.sealed(value)

    def test_verify_late_or_throw_preserves_all_originals_and_no_followup(self):
        for variant in ('late','throw'):
            value,child=self.fixture();clock=[0.0]
            def verified(*args):
                if variant=='throw':raise RuntimeError('private')
                clock[0]=5.0
            value.copies.verify.side_effect=verified
            with patch.object(i.time,'monotonic',side_effect=lambda:clock[0]):
                with self.assertRaises(i.Refused):value.verify(5.0)
            value.artifacts.recheck.assert_not_called();self.sealed(value)

    def test_maps_parser_refuses_private_deleted_conflict_overflow_and_excess(self):
        bad=[mapping([('/home/private',1)]),mapping([('/usr/lib/libc.so.6 (deleted)',1)]),
             mapping([('/usr/lib/libc.so.6',1),('/usr/lib/libc.so.6',2)]),
             mapping([('/usr/lib/libc.so.6',2**64)]),
             mapping([(f'/usr/lib/f{index}',index+1) for index in range(65)]),
             '0-10000000000000000 r-xp 0 00:1f 1 /usr/lib/libc.so.6\n',
             '0-1000 rw-p 1 00:00 0 [heap]\n', '', 'x'*(1024*1024+1)]
        for raw in bad:
            with self.assertRaisesRegex(i.Refused,'^fixed_loaded_image_refused$'):i.map_objects(raw)
        self.assertEqual(i.map_objects(mapping([('/usr/lib/libc.so.6',1)])),{'/usr/lib/libc.so.6':(31,1)})

    def test_constructor_only_exact_pinned_classes_before_any_fs_read(self):
        with patch.object(i.os,'open') as opened:
            with self.assertRaises(i.Refused):i.Images(SimpleNamespace(sealed=False),SimpleNamespace(Session=Session),
                SimpleNamespace(state='ready'),SimpleNamespace(Bridge=Bridge),
                SimpleNamespace(sealed=False),SimpleNamespace(Sources=Artifacts))
            opened.assert_not_called()

    def test_actual_kernel_exe_original_fd_identity_before_any_maps_hash(self):
        for role in i.ROLES:
            value,child=self.fixture(role)
            del value.executable
            inode=90 if role in ('core','broker','host') else value.copies.records[i.ROLES[role]]['inode']
            info=SimpleNamespace(st_dev=31,st_ino=inode,st_size=64,st_mode=stat.S_IFREG|0o555,
                                 st_uid=0,st_gid=0,st_nlink=1)
            if role in ('core','broker','host'):
                value.artifacts.files[i.ROLES[role].rsplit('/',1)[1]]=(55,info,'b'*64)
            else:value.copies.records[i.ROLES[role]].update(mode=info.st_mode,uid=0,gid=0,nlink=1)
            with tempfile.TemporaryFile() as temp:
                fd=os.dup(temp.fileno())
                with patch.object(i.os,'open',return_value=fd) as opened,patch.object(i.os,'fstat',return_value=info):
                    self.assertEqual(value.executable(child,role,5.0),(31,inode))
                opened.assert_called_once_with('exe',i.EXE_FLAGS,dir_fd=71)
                self.assertIn(fd,value.held)
            value.copies._verify_target.assert_not_called();value.artifacts.recheck.assert_not_called()

    def test_wrong_kernel_executable_identity_precedes_maps_and_hash(self):
        value,child=self.fixture();del value.executable
        original=SimpleNamespace(st_dev=31,st_ino=90,st_size=64,st_mode=stat.S_IFREG|0o555,
                                 st_uid=0,st_gid=0,st_nlink=1)
        value.artifacts.files['mihomo']=(55,original,'b'*64)
        wrong=SimpleNamespace(**vars(original));wrong.st_ino=91
        with tempfile.TemporaryFile() as temp:
            fd=os.dup(temp.fileno())
            with patch.object(i.os,'open',return_value=fd),patch.object(i.os,'fstat',return_value=wrong), \
                 patch.object(value,'text') as maps_read:
                with self.assertRaises(i.Refused):value.inventory(child,5.0)
                maps_read.assert_not_called();value.copies._verify_target.assert_not_called()
                value.artifacts.mapped_identity.assert_not_called()
        self.sealed(value)

    def test_no_execution_signal_reap_write_cleanup_or_dynamic_proc_path(self):
        forbidden={'exec','eval','spawn','kill','waitid','waitpid','system','write','unlink','close','rmdir','mkdir','chmod','chown'}
        tree=ast.parse(SOURCE.read_text())
        for node in ast.walk(tree):
            if isinstance(node,ast.Call):
                name=node.func.id if isinstance(node.func,ast.Name) else node.func.attr if isinstance(node.func,ast.Attribute) else ''
                self.assertNotIn(name,forbidden)
        self.assertNotIn('/proc/',SOURCE.read_text())


if __name__=='__main__':unittest.main()
