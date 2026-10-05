"""Inert byte-copy/FD controls: no mount, namespace, ELF or guest invocation."""
from contextlib import ExitStack
import hashlib
import importlib.util
import os
from pathlib import Path
import stat
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import Mock,patch

HERE=Path(__file__).parent
def module(name):
    spec=importlib.util.spec_from_file_location('copy_test_'+name,HERE/(name+'.py'))
    value=importlib.util.module_from_spec(spec);spec.loader.exec_module(value);return value
n=module('native_copy');a=module('artifacts')


class Owner:
    def __init__(self):self.kind='inner';self.isolated=True;self.retained=[];self.sealed=False;self.now=0.0;self.deadline=65.0
    def local_deadline(self,seconds):return min(self.deadline,self.now+float(seconds))
    def available(self):self.within(self.deadline)
    def within(self,deadline):
        if self.sealed or self.now>=deadline:raise RuntimeError('synthetic fence')


class Controls(unittest.TestCase):
    def setUp(self):
        cache=Path.home()/'.cache';cache.mkdir(mode=0o700,exist_ok=True)
        self.temp=tempfile.TemporaryDirectory(prefix='t3-native-copy.',dir=cache)
        self.root=Path(self.temp.name);self.old=self.root/'old';self.new=self.root/'new'
        self.old.mkdir(mode=0o755);self.new.mkdir(mode=0o755)
        self.old.chmod(0o755);self.new.chmod(0o755)
        self.originals={};self.table={};self.fds=[];self.instances=[];self.owner=Owner()
        self.state={'mounted':False,'readonly':True};self.commands=[]
        for name in a.TABLE:
            raw=b'PUBLIC INERT NONEXECUTED '+name.encode();mode=a.TABLE[name][1]
            path=self.old/name;path.write_bytes(raw);path.chmod(mode)
            fd=os.open(path,n.FLAGS);self.fds.append(fd)
            self.table[name]=(len(raw),mode,hashlib.sha256(raw).hexdigest())
            self.originals[name]=(fd,self.metadata(os.fstat(fd)))
        self.artifacts=SimpleNamespace(TABLE=self.table,identity=a.identity)
        real_open,real_stat,real_fstat=os.open,os.stat,os.fstat
        def opened(path,*args,**kwargs):
            if path=='/artifacts':path=self.new if self.state['mounted'] else self.old
            return real_open(path,*args,**kwargs)
        def named(path,*args,**kwargs):
            if path=='/artifacts':path=self.new if self.state['mounted'] else self.old
            return self.metadata(real_stat(path,*args,**kwargs))
        def command(argv):
            self.commands.append(argv)
            if len(self.commands)==1:self.state.update(mounted=True,readonly=False)
            else:self.state['readonly']=True
        self.owner.command=command
        self.covered=['11','1','1:1','/fixed/native','/artifacts','ro,nosuid,nodev','-','btrfs','/dev/public','rw']
        device=self.new.stat().st_dev
        def rows(value):
            if not self.state['mounted']:return [self.covered[:]]
            flag='ro' if self.state['readonly'] else 'rw'
            return [self.covered[:],['12','1',f'{os.major(device)}:{os.minor(device)}','/',
                '/artifacts',flag+',nosuid,nodev','-','tmpfs','tmpfs',flag]]
        self.stack=ExitStack()
        for obj,name,value in ((n.os,'open',opened),(n.os,'stat',named),
            (n.os,'fstat',lambda fd:self.metadata(real_fstat(fd))),
            (n.os,'getpid',lambda:1),(n.os,'getresuid',lambda:(0,0,0)),
            (n.os,'getresgid',lambda:(0,0,0)),
            (n.os,'fstatvfs',lambda fd:SimpleNamespace(f_flag=os.ST_RDONLY if self.state['readonly'] else 0)),
            (n.NativeStore,'mount_rows',rows),(n.NativeStore,'no_writers',lambda value:None)):
            self.stack.enter_context(patch.object(obj,name,value))

    def metadata(self,value):
        fields={key:getattr(value,key) for key in dir(value) if key.startswith('st_')}
        fields.update(st_uid=0,st_gid=0);return SimpleNamespace(**fields)

    def tearDown(self):
        self.stack.close()
        for value in self.instances:
            for stream in value.iterators:stream.close()
            self.fds.extend(value.held)
        for fd in set(self.fds):
            try:os.close(fd)
            except OSError:pass  # Test-owned positive-close cuts can have closed it.
        self.temp.cleanup()

    def construct(self):
        value=n.NativeStore.__new__(n.NativeStore);self.instances.append(value)
        self.owner.retained.append(value)
        value.__init__(self.owner,SimpleNamespace(Session=Owner),self.originals,self.artifacts)
        return value

    def test_full_inert_copy_new_inodes_same_bytes_and_sealed_destination(self):
        value=self.construct();self.assertTrue(value.ready);self.assertFalse(value.sealed)
        self.assertEqual(len(self.commands),2);self.assertEqual(set(value.files),set(self.table))
        self.assertEqual(self.commands[0],['/usr/bin/mount','-t','tmpfs','-o',
            'size=128m,mode=0755,nosuid,nodev','tmpfs','/artifacts'])
        self.assertEqual(self.commands[1],['/usr/bin/mount','-o','remount,ro,nosuid,nodev','/artifacts'])
        for name,(fd,destination) in value.files.items():
            self.assertNotEqual(destination.st_ino,self.originals[name][1].st_ino)
            self.assertEqual(os.pread(fd,1000,0),os.pread(self.originals[name][0],1000,0))
            self.assertEqual(stat.S_IMODE(destination.st_mode),self.table[name][1])
        value.verify();self.assertFalse(value.pending)

    def test_source_digest_drift_refuses_before_new_mount_or_destination(self):
        key=next(iter(self.table));size,mode,_=self.table[key];self.table[key]=(size,mode,'0'*64)
        with patch.object(n.os,'write') as write:
            with self.assertRaises(n.Refused):self.construct()
            write.assert_not_called()
        self.assertEqual(self.commands,[]);self.assertTrue(self.owner.sealed)

    def test_source_same_bytes_new_inode_is_not_provenance(self):
        key=next(iter(self.originals));fd,before=self.originals[key]
        wrong=SimpleNamespace(**vars(before));wrong.st_ino+=1;self.originals[key]=(fd,wrong)
        with self.assertRaises(n.Refused):self.construct()
        self.assertEqual(self.commands,[])

    def test_short_write_preserves_writer_and_no_close_seal_or_next_copy(self):
        with patch.object(n.os,'write',return_value=0) as write,patch.object(n.os,'close') as close:
            with self.assertRaises(n.Refused):self.construct()
            write.assert_called_once();close.assert_not_called()
        value=self.instances[-1];self.assertEqual(len(value.pending),1)
        self.assertEqual(value.files,{});self.assertEqual(len(self.commands),1)
        self.assertTrue(value.sealed);self.assertTrue(self.owner.sealed)

    def test_late_destination_open_retained_before_any_write(self):
        real=n.os.open
        def opened(path,*args,**kwargs):
            fd=real(path,*args,**kwargs)
            if path in self.table and kwargs.get('dir_fd')!=None:self.owner.now=20.0
            return fd
        with patch.object(n.os,'open',side_effect=opened),patch.object(n.os,'write') as write:
            with self.assertRaises(n.Refused):self.construct()
            write.assert_not_called()
        value=self.instances[-1];self.assertGreaterEqual(len(value.held),2)
        self.assertEqual(value.files,{})

    def test_late_positive_writer_close_never_reopens_or_advances(self):
        real=n.os.close
        def closing(fd):real(fd);self.owner.now=20.0
        with patch.object(n.os,'close',side_effect=closing) as close:
            with self.assertRaises(n.Refused):self.construct()
            close.assert_called_once()
        value=self.instances[-1];self.assertEqual(value.files,{})
        self.assertEqual(len(value.pending),1);self.assertEqual(len(self.commands),1)

    def test_late_readonly_open_retains_destination_before_recheck(self):
        real=n.os.open
        def opened(path,flags,*args,**kwargs):
            fd=real(path,flags,*args,**kwargs)
            if path in self.table and flags==n.FLAGS:self.owner.now=20.0
            return fd
        with patch.object(n.os,'open',side_effect=opened):
            with self.assertRaises(n.Refused):self.construct()
        value=self.instances[-1]
        self.assertEqual(value.files,{});self.assertFalse(value.pending)
        self.assertTrue(any(stat.S_ISREG(os.fstat(fd).st_mode) for fd in value.held))
        self.assertEqual(len(self.commands),1)

    def test_seal_refusal_retains_four_destinations_never_ready_or_retries(self):
        real=self.owner.command
        def command(argv):
            if len(self.commands)==1:raise RuntimeError('synthetic seal refusal')
            return real(argv)
        self.owner.command=command
        with self.assertRaises(n.Refused):self.construct()
        value=self.instances[-1];self.assertEqual(len(value.files),4);self.assertFalse(value.ready)
        with patch.object(n.os,'pread') as read,patch.object(n.os,'open') as opened:
            with self.assertRaises(n.Refused):value.verify()
            read.assert_not_called();opened.assert_not_called()

    def test_shared_lifetime_and_caller_cap_are_not_expanded(self):
        value=self.construct();self.assertEqual(value.deadline,65.0)
        self.owner.now=21.0;value.verify(26.0);self.assertEqual(value.deadline,65.0)
        self.owner.now=26.0
        with patch.object(n.os,'fstat') as metadata:
            with self.assertRaises(n.Refused):value.verify(26.0)
            metadata.assert_not_called()
        self.assertTrue(value.sealed)

    def test_mount_text_parser_has_finite_exact_rows_and_no_raw_diagnostic(self):
        actual=module('native_copy')
        good=b'11 1 1:1 /fixed/native /artifacts ro,nosuid,nodev - btrfs /dev/public rw\n'
        for raw in (good,good+b'\n',good[:-1],b'bad\n'):
            value=n.NativeStore.__new__(n.NativeStore);value.owner=Owner();value.sealed=False
            value.deadline=20.0;value.held=[];value.iterators=[]
            with patch.object(value,'opened',return_value=81),patch.object(n.os,'read',side_effect=[raw,b'']):
                if raw==good:self.assertEqual(actual.NativeStore.mount_rows(value),[self.covered])
                else:
                    with self.assertRaises((n.Refused,actual.Refused)):actual.NativeStore.mount_rows(value)
                    self.assertTrue(value.owner.sealed)

    def test_destination_replacement_equal_bytes_never_accepted(self):
        value=self.construct();key=next(iter(self.table));path=self.new/key
        raw=path.read_bytes();path.unlink();path.write_bytes(raw);path.chmod(self.table[key][1])
        with self.assertRaises(n.Refused):value.verify()
        with patch.object(n.os,'pread') as read,patch.object(n.os,'stat') as named:
            with self.assertRaises(n.Refused):value.verify()
            read.assert_not_called();named.assert_not_called()

    def test_wrong_covered_or_extra_mount_and_writable_seal_refuse(self):
        base=self.construct()
        device=base.device
        correct=['12','1',f'{os.major(device)}:{os.minor(device)}','/',
            '/artifacts','ro,nosuid,nodev','-','tmpfs','tmpfs','ro']
        for variant in ('changed_covered','extra','writable','wrong_device','noexec'):
            # Fresh synthetic owner/object per case, no retry of any refusal.
            value=n.NativeStore.__new__(n.NativeStore);value.__dict__.update(base.__dict__)
            value.owner=Owner();value.sealed=False;rows=[base.covered_row[:],correct[:]]
            if variant=='changed_covered':rows[0][3]='/different/native'
            elif variant=='extra':rows.append(correct[:])
            elif variant=='writable':rows[1][5]='rw,nosuid,nodev';rows[1][-1]='rw'
            elif variant=='wrong_device':rows[1][2]='999:999'
            else:rows[1][5]+=',noexec'
            with self.subTest(variant=variant),patch.object(value,'mount_rows',return_value=rows):
                with self.assertRaises(n.Refused):value.mount_policy(True)
            self.assertTrue(value.owner.sealed)

    def test_live_fd_inventory_readonly_exact_bound_and_refusals(self):
        # Restore the real method only for these synthetic per-next controls.
        # The patch's saved original is obtainable from the class source module
        # reloaded separately; no namespace/FD discovery occurs in this test.
        original=module('native_copy').NativeStore.no_writers
        for names,mode in ((['81','82'],os.O_RDONLY),(['81','82'],os.O_WRONLY),
            (['81','82'],os.O_RDWR),(['81','81'],os.O_RDONLY),
            (['81','512'],os.O_RDONLY),(['81','01'],os.O_RDONLY),
            (['82'],os.O_RDONLY)):
            value=n.NativeStore.__new__(n.NativeStore);value.owner=Owner();value.sealed=False
            value.deadline=20.0;value.device=7;value.pending=set();value.iterators=[]
            entries=iter(SimpleNamespace(name=name) for name in names)
            with self.subTest(names=names,mode=mode),patch.object(value,'opened',return_value=81), \
                 patch.object(n.os,'scandir',return_value=entries), \
                 patch.object(n.os,'fstat',return_value=SimpleNamespace(st_dev=7)), \
                 patch.object(n.fcntl,'fcntl',return_value=mode):
                if names==['81','82'] and mode==os.O_RDONLY:original(value)
                else:
                    with self.assertRaises(Exception):original(value)
                    self.assertTrue(value.owner.sealed)

    def test_unknown_context_precedes_all_native_copy_effects(self):
        self.owner.isolated=False
        with patch.object(n.os,'open') as opened:
            with self.assertRaises(n.Refused):self.construct()
            opened.assert_not_called()
        self.assertEqual(self.commands,[])


if __name__=='__main__':unittest.main()
