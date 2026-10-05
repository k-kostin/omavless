"""Fixed public source graph for the future fresh positive launcher.

No entry point, process/namespace operation or caller-selected source path.
The outer guard must independently pin THIS reader and the launcher. Loading
known source definitions is not native execution, current identity or adoption.
"""
import ast
import hashlib
import math
import os
from pathlib import Path
import stat
import time
import types

STAGE = '/home/kdk_vm/.cache/t3-retained-native-tmpfs-review-1'
PINS = {
    'lifecycle.py': 'cc855656a31b7681ad45ec28a040f8b3fd9a4cfbf6cd93269960d9e440543708',
    'native_copy.py': '87c2c7fb9267b6f8aad34f93d184ad2b492158e54926c964cadbab41f3d03b74',
    'artifacts.py': '6ae8a1293afcf7a3385f19a0451af8f96016d358f357796208344d74da7ffd4e',
    'images.py': 'd1e6fabdfe0487485c6b130490bf11c6519684ef553c98d99b09e95cbc1095f1',
    'controller.py': 'ffc849e9554e9cdf3f3e9fc2a9b5d60066c72d56ca634275b2c33e2f87673a8a',
    'helper.py': 'f369f888d64aeb3c8d9fd2e57354d8185547aeed21e19c86ef6f91c0bddaebc4',
    'positive.py': '74a2f2d20bc418a10bc0b21eadfb3e6304b77725cd3ed85ae935122679ee8f44',
    'streams.py': '1f0b676610f4f5d6f599a1d73ff9ea6ea2d469c320cf789fbe3e88ee56ae9384',
    'bootstrap.py': 'ae86bfae0bbacc3bc5f3c9db99c1c75ec04f79b50f55ccea575b1ba3fb7c3a73',
    'bridge.py': 'aeb863f81b3b250091020e7b7610e10346da65fd2dba58d50b4c9e63a8e79f45',
    'admission.py': 'ab4da7a70494283a1dac12aec385383ef5232d1699a21f2ed0d0b1bca56825da',
    'copy-manifest.json': '3caa3d2bfdace10e97617b1e222192bf0c78f8ac194f51ee0a2f90f9defdd896',
    'containment.py': '2b9980266bd467c0684ee489167aadb4b53495340d26c6389aed723d67736592',
    'guest-inventory.json': '4f1b92aeaff78f5f376576fc1da722cf9077735ff5c36d8674ff92581342ada4',
}
# Metadata/catalog only here: their actual bytes are pinned by the outer guard.
EXTRA = {'graph.py','launcher.py','validate_receipt.py','vm-guard.sh','native','scratch'}
FLAGS = os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC
BASE_FUNCTIONS = {'require','pairs','decode','namespace','validate_maps','links',
    'no_usr_submounts','public_directory','public_file','isolate','verify_subordinates',
    'verify_system_inputs','verify_child','prepare_resolved_root','bus_config',
    'resolved_exec','cap_exec','effects','active','clean','limits'}
BASE_CONSTANTS = {'RELEASE_ENROLLMENT','CASES','NS','LIMIT','UNSETTLED',
                  'BOOTSTRAP_DIRECT_ONLY','RESOLVER_CAPS','RESOLVED_CONFIG','ENV','CAP'}
BASE_IMPORTS = {'hashlib','json','os','pathlib','resource','socket','stat'}


class Refused(RuntimeError):
    def __init__(self): super().__init__('fixed_positive_graph_refused')


def require(value):
    if not value: raise Refused()


def identity(value):
    return tuple(getattr(value,key) for key in ('st_dev','st_ino','st_mode','st_uid',
        'st_gid','st_nlink','st_size','st_mtime_ns','st_ctime_ns'))


def containment_tree(raw):
    """Compile only explicit definitions; no legacy exercise/stop/main graph.

    Hash of the whole original source is checked BEFORE this function is used.
    Fixed command/live/directory-FD hooks must be supplied by the retained owner
    before any returned containment function is invoked.
    """
    require(type(raw) is bytes and hashlib.sha256(raw).hexdigest()==PINS['containment.py'])
    tree=ast.parse(raw,filename='fixed-reviewed-containment')
    chosen=[];functions=set();constants=set();imports=set();classes=set()
    for node in tree.body:
        if isinstance(node,ast.Import):
            if all(alias.name in BASE_IMPORTS for alias in node.names):
                chosen.append(node);imports.update(alias.name for alias in node.names)
        elif isinstance(node,ast.ImportFrom) and node.module=='pathlib':
            require([(alias.name,alias.asname) for alias in node.names]==[('Path',None)])
            chosen.append(node);imports.add('pathlib')
        elif isinstance(node,ast.ClassDef) and node.name=='Refused':
            require(node.name not in classes);classes.add(node.name);chosen.append(node)
        elif isinstance(node,ast.FunctionDef) and node.name in BASE_FUNCTIONS:
            require(node.name not in functions);functions.add(node.name);chosen.append(node)
        elif isinstance(node,ast.Assign):
            names={target.id for target in node.targets if isinstance(target,ast.Name)}
            if names and names <= BASE_CONSTANTS:
                require(not names & constants);constants.update(names);chosen.append(node)
    require(functions==BASE_FUNCTIONS and constants==BASE_CONSTANTS
            and imports==BASE_IMPORTS and classes=={'Refused'})
    return ast.fix_missing_locations(ast.Module(body=chosen,type_ignores=[]))


class Graph:
    def __init__(self, enclosing_deadline):
        self.sealed=True;self.held=[];self.directories=[];self.iterators=[]
        self.records={};self.raw={};self.modules={};self.directory_closed=False
        self.loaded=False
        start=time.monotonic()
        require(type(start) is float and math.isfinite(start)
                and type(enclosing_deadline) is float and math.isfinite(enclosing_deadline)
                and start<enclosing_deadline)
        local=start+20.0
        require(math.isfinite(local) and start<local)
        self.deadline=min(local,enclosing_deadline)
        self.sealed=False
        try:
            self.available()
            require(os.getresuid()==os.getresgid() and os.getresuid() in ((0,0,0),(1000,1000,1000)))
            self.uid=os.getuid()
            parent=self.open('/',FLAGS|os.O_DIRECTORY,directory=True)
            info=self.io(os.fstat,parent)
            require(stat.S_ISDIR(info.st_mode) and info.st_uid==info.st_gid
                    and info.st_uid in (0,65534) and info.st_mode&0o022==0)
            self.parents=[('/',parent,identity(info))]
            for part in Path(STAGE).parts[1:]:
                parent=self.open(part,FLAGS|os.O_DIRECTORY,dir_fd=parent,directory=True)
                info=self.io(os.fstat,parent)
                require(stat.S_ISDIR(info.st_mode) and info.st_uid==info.st_gid
                        and info.st_uid in (0,self.uid,65534) and info.st_mode&0o022==0)
                self.parents.append((part,parent,identity(info)))
            self.directory=parent
            info=self.io(os.fstat,parent)
            require(info.st_uid==info.st_gid==self.uid and stat.S_IMODE(info.st_mode)==0o700)
            self.catalog()
            total=0
            for name in sorted(PINS):
                fd=self.open(name,FLAGS,dir_fd=parent)
                info=self.io(os.fstat,fd)
                require(stat.S_ISREG(info.st_mode) and info.st_uid==info.st_gid==self.uid
                        and stat.S_IMODE(info.st_mode)==0o600 and info.st_nlink==1
                        and 0<info.st_size<=512*1024 and not self.io(os.listxattr,fd))
                total+=info.st_size;require(total<=2*1024*1024)
                raw=self.read(fd,info.st_size)
                require(hashlib.sha256(raw).hexdigest()==PINS[name])
                self.records[name]=(fd,identity(info));self.raw[name]=raw
            self.recheck()
        except BaseException:
            self.sealed=True;raise Refused() from None

    def available(self):
        try:
            now=time.monotonic()
            require(not self.sealed and type(now) is float and math.isfinite(now)
                    and type(self.deadline) is float and math.isfinite(self.deadline) and now<self.deadline)
        except BaseException:
            self.sealed=True;raise Refused() from None

    def io(self,operation,*args,**kwargs):
        try:
            self.available();value=operation(*args,**kwargs);self.available();return value
        except BaseException:
            self.sealed=True;raise Refused() from None

    def open(self,*args,directory=False,**kwargs):
        try:
            self.available();fd=os.open(*args,**kwargs)
            self.held.append(fd)
            if directory:self.directories.append(fd)
            self.available();return fd
        except BaseException:
            self.sealed=True;raise Refused() from None

    def read(self,fd,size):
        data=b''
        while len(data)<size:
            part=self.io(os.pread,fd,min(65536,size-len(data)),len(data))
            require(part);data+=part
        require(self.io(os.pread,fd,1,size)==b'')
        return data

    def catalog(self):
        self.io(os.lseek,self.directory,0,os.SEEK_SET)
        self.available();stream=os.scandir(self.directory);self.iterators.append(stream);self.available()
        seen=set();sentinel=object()
        while True:
            item=self.io(next,stream,sentinel)
            if item is sentinel:break
            require(type(item.name) is str and item.name in set(PINS)|EXTRA
                    and item.name not in seen and len(seen)<20)
            seen.add(item.name)
        require(seen==set(PINS)|EXTRA)

    def recheck(self):
        try:
            require(not self.directory_closed)
            self.catalog()
            for index,(name,fd,before) in enumerate(self.parents):
                current=self.io(os.stat,'/',follow_symlinks=False) if index==0 else self.io(
                    os.stat,name,dir_fd=self.parents[index-1][1],follow_symlinks=False)
                require(before==identity(self.io(os.fstat,fd))==identity(current))
            require(set(self.records)==set(PINS))
            for name,(fd,before) in self.records.items():
                require(before==identity(self.io(os.fstat,fd))==identity(self.io(
                    os.stat,name,dir_fd=self.directory,follow_symlinks=False))
                    and not self.io(os.listxattr,fd))
                require(hashlib.sha256(self.read(fd,before[6])).hexdigest()==PINS[name])
                require(before==identity(self.io(os.fstat,fd))==identity(self.io(
                    os.stat,name,dir_fd=self.directory,follow_symlinks=False)))
            self.catalog();self.available()
        except BaseException:
            self.sealed=True;raise Refused() from None

    def load(self):
        try:
            self.available();require(not self.loaded)
            self.loaded=True  # No retry of partial definition loading.
            require(set(self.raw)==set(PINS) and all(type(raw) is bytes
                    and hashlib.sha256(raw).hexdigest()==PINS[name]
                    for name,raw in self.raw.items()))
            self.recheck()  # WHOLE graph before even the first definition exec.
            for name in sorted(PINS):
                if not name.endswith('.py'):continue
                tree=containment_tree(self.raw[name]) if name=='containment.py' else ast.parse(self.raw[name])
                module=types.ModuleType('fixed_positive_'+name[:-3])
                self.available();exec(compile(tree,'fixed-public-'+name,'exec'),module.__dict__);self.available()
                self.modules[name]=module
            self.recheck();return self.modules
        except BaseException:
            self.sealed=True;raise Refused() from None

    def release_directories_positive(self):
        """Only a fully known source readback permits closes before chroot.

        Regular source originals remain held. An ambiguous/late close seals
        before the next close and is never retried. No failure cleanup exists.
        """
        try:
            require(self.loaded and not self.directory_closed)
            self.recheck()
            for fd in reversed(self.directories):
                self.io(os.close,fd)
                self.held.remove(fd)
            self.directory_closed=True;self.available()
        except BaseException:
            self.sealed=True;raise Refused() from None
