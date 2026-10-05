#!/usr/bin/env python3
"""Fresh developer-only retained positive entry; NEVER a production fallback.

Needs separately reviewed fixed HOST originals/builder, parent canonical guard
and source graph pins. No archive/exercise/global argv cleanup path is loaded.
All actual invocation authority remains with ROOT's exclusive disposable VM.
"""
import ast
import functools
import hashlib
import json
import math
import os
from pathlib import Path
import re
import resource
import signal
import stat
import subprocess
import sys
import time
import types

STAGE='/home/kdk_vm/.cache/t3-retained-positive-composition-review-2'
ROOT=STAGE+'/scratch/inventory/root'
NATIVE=STAGE+'/native'
GRAPH='61d1cd2527a32ba83ebf7d29b94c6567f79fdbcd3719b04aa5d8aa32e35066d9'
VALIDATOR='8acc602d2d6abfc56fd2e0f6d2d2cc35d00d046e4e1b0acddc55f2217def7f1e'
SOURCE_PINS={'graph.py':GRAPH,'validate_receipt.py':VALIDATOR}
RUN=['--run','--ack-retained-positive-disposable-vm']
NS=('user','net','mnt','pid','uts')
HELD=[]
FLAGS=os.O_RDONLY|os.O_NOFOLLOW|os.O_NONBLOCK|os.O_CLOEXEC
WRITE=os.O_RDWR|os.O_CREAT|os.O_EXCL|os.O_NOFOLLOW|os.O_CLOEXEC


class Refused(RuntimeError):
    def __init__(self):super().__init__('retained_positive_entry_refused')


def require(value):
    if not value:raise Refused()


def identity(value):
    return tuple(getattr(value,key) for key in ('st_dev','st_ino','st_mode','st_uid',
        'st_gid','st_nlink','st_size','st_mtime_ns','st_ctime_ns'))


def guarded(method):
    @functools.wraps(method)
    def call(entry,*args,**kwargs):
        try:
            entry.available();value=method(entry,*args,**kwargs);entry.available();return value
        except BaseException:
            entry.sealed=True;raise Refused() from None
    return call


class Entry:
    def __init__(self):
        self.sealed=True;self.output_attempted=False
        start=time.monotonic()
        require(type(start) is float and math.isfinite(start))
        self.deadline=start+90.0
        require(math.isfinite(self.deadline) and start<self.deadline)
        self.sealed=False

    def available(self):
        try:
            now=time.monotonic()
            require(not self.sealed and type(now) is float and math.isfinite(now)
                and type(self.deadline) is float and math.isfinite(self.deadline) and now<self.deadline)
        except BaseException:
            self.sealed=True;raise Refused() from None

    def call(self,operation,*args,**kwargs):
        try:
            self.available();value=operation(*args,**kwargs);self.available();return value
        except BaseException:
            self.sealed=True;raise Refused() from None

    def opened(self,*args,**kwargs):
        try:
            self.available();fd=os.open(*args,**kwargs);HELD.append(fd);self.available();return fd
        except BaseException:
            self.sealed=True;raise Refused() from None

    def output(self,raw):
        try:
            self.available();require(not self.output_attempted and type(raw) is bytes
                and 0<len(raw)<=4*1024*1024)
            self.output_attempted=True
            result=os.write(1,raw)
            require(type(result) is int and result==len(raw))
            self.available();self.sealed=True  # One terminal unbuffered output, no flush/retry.
        except BaseException:
            self.sealed=True;raise Refused() from None


@guarded
def pinned_module(entry,name):
    require(name in SOURCE_PINS)
    path=STAGE+'/'+name;fd=entry.opened(path,FLAGS)
    before=entry.call(os.fstat,fd)
    require(stat.S_ISREG(before.st_mode) and before.st_uid==before.st_gid==os.getuid()
        and stat.S_IMODE(before.st_mode)==0o600 and before.st_nlink==1
        and 0<before.st_size<=64*1024 and not entry.call(os.listxattr,fd))
    raw=b''
    while len(raw)<before.st_size:
        part=entry.call(os.pread,fd,min(65536,before.st_size-len(raw)),len(raw))
        require(part);raw+=part
    require(entry.call(os.pread,fd,1,len(raw))==b''
        and hashlib.sha256(raw).hexdigest()==SOURCE_PINS[name]
        and identity(before)==identity(entry.call(os.fstat,fd))
        ==identity(entry.call(os.stat,path,follow_symlinks=False)))
    module=types.ModuleType('fixed_positive_'+name[:-3]);HELD.append(module)
    entry.call(exec,compile(ast.parse(raw),'fixed-positive-'+name,'exec'),module.__dict__)
    return module


@guarded
def modules(entry,kind):
    reader=pinned_module(entry,'graph.py');validator=pinned_module(entry,'validate_receipt.py')
    graph=reader.Graph.__new__(reader.Graph);HELD.append(graph)
    entry.call(graph.__init__,entry.deadline)
    loaded=entry.call(graph.load)
    ownership=loaded['lifecycle.py']
    entry.available();owner=ownership.Session(kind);HELD.append(owner);entry.available()
    owner.deadline=min(owner.deadline,entry.deadline);owner.available()
    base=loaded['containment.py']
    # These exact retained hooks precede ANY selected containment invocation.
    base.command=owner.command
    base.child_status=owner.live
    base.no_directory_fds=owner.no_directory_fds
    return graph,loaded,ownership,owner,base,validator


def namespace_frame(value):
    require(type(value) is dict and set(value)==set(NS))
    for name,text in value.items():
        require(type(text) is str and re.fullmatch(re.escape(name)+r':\[[1-9][0-9]{0,19}\]',text))
    return value


@guarded
def native_originals(entry,artifact_module):
    """ALL four fixed originals before bind/execution; no receipt path selector.

    The separate HOST builder proves its original-to-staging transfer. These
    guest originals and later read-only /artifacts originals must still agree.
    """
    directory=entry.opened(NATIVE,FLAGS|os.O_DIRECTORY)
    before=entry.call(os.fstat,directory)
    require(stat.S_ISDIR(before.st_mode) and before.st_uid==before.st_gid==os.getuid()
        and stat.S_IMODE(before.st_mode)==0o755)
    def catalog():
        entry.call(os.lseek,directory,0,os.SEEK_SET)
        entry.available();stream=os.scandir(directory);HELD.append(stream);entry.available()
        names=set();sentinel=object()
        while True:
            item=entry.call(next,stream,sentinel)
            if item is sentinel:break
            require(type(item.name) is str and item.name in artifact_module.TABLE
                and item.name not in names and len(names)<4)
            names.add(item.name)
        require(names==set(artifact_module.TABLE))
    catalog();records={};total=0
    for name,(size,mode,sha) in sorted(artifact_module.TABLE.items()):
        fd=entry.opened(name,FLAGS,dir_fd=directory);value=entry.call(os.fstat,fd)
        require(stat.S_ISREG(value.st_mode) and value.st_uid==value.st_gid==os.getuid()
            and value.st_nlink==1 and value.st_size==size and stat.S_IMODE(value.st_mode)==mode
            and not entry.call(os.listxattr,fd))
        total+=size;require(total<=128*1024*1024)
        digest=hashlib.sha256();offset=0
        while offset<size:
            part=entry.call(os.pread,fd,min(65536,size-offset),offset)
            require(part);digest.update(part);offset+=len(part)
        require(entry.call(os.pread,fd,1,size)==b'' and digest.hexdigest()==sha
            and identity(value)==identity(entry.call(os.fstat,fd))
            ==identity(entry.call(os.stat,name,dir_fd=directory,follow_symlinks=False)))
        records[name]=(fd,value)
    catalog()
    require(identity(before)==identity(entry.call(os.fstat,directory))
        ==identity(entry.call(os.stat,NATIVE,follow_symlinks=False)))
    # A known positive catalog/readback permits only this directory close,
    # BEFORE chroot; regular native originals remain held on every outcome.
    require(entry.call(os.close,directory) is None);HELD.remove(directory)
    return records


@guarded
def directories(entry):
    # Fresh constructor only. The reviewed transport has created scratch,
    # but these two entries must still be absent; never reuse an older scope.
    for path in (STAGE+'/scratch/inventory',ROOT):entry.call(os.mkdir,path,0o700)


def child(entry,frame):
    require(os.getpid()==1 and os.getresuid()==os.getresgid()==(0,0,0))
    require(type(frame) is dict and set(frame)=={'original_ns','absolute_deadline'})
    cap=frame['absolute_deadline']
    require(type(cap) is float and math.isfinite(cap) and cap<=entry.deadline)
    # No time namespace is created: both processes use the same monotonic
    # clock. This private fixed own-spawn frame is not an effect permit.
    entry.deadline=min(entry.deadline,cap);entry.available()
    original=namespace_frame(frame['original_ns'])
    graph,loaded,ownership,owner,base,validator=modules(entry,'inner')
    # PID1 owns all retained descriptors; actual role processes still receive
    # independent128 limits from the unchanged containment pre-exec function.
    entry.call(resource.setrlimit,resource.RLIMIT_NOFILE,(512,512))
    originals=native_originals(entry,loaded['artifacts.py'])
    entry.call(graph.release_directories_positive)
    owner.perform(base.isolate,original,Path(ROOT),Path(NATIVE));entry.available()
    copies=loaded['bridge.py'].Bridge.__new__(loaded['bridge.py'].Bridge)
    owner.retained.append(copies)
    owner.perform(copies.__init__,base,loaded['admission.py'],graph.raw['copy-manifest.json']);entry.available()
    owner.phase('before_copy_prepare');entry.available()
    owner.perform(copies.prepare,original);entry.available()
    owner.phase('after_copy_prepare');entry.available()
    artifacts=loaded['artifacts.py'].Sources.__new__(loaded['artifacts.py'].Sources)
    owner.retained.append(artifacts)
    owner.phase('before_artifact_admission');entry.available()
    owner.perform(artifacts.__init__);entry.available()
    owner.phase('after_artifact_admission');entry.available()
    owner.phase('before_artifact_crosscheck');entry.available()
    for name,(fd,original) in originals.items():
        current=entry.call(os.fstat,fd);_,bound,_=artifacts.files[name]
        require(identity(original)==identity(current)==identity(bound))
    owner.phase('after_artifact_crosscheck');entry.available()
    owner.phase('before_case_constructor');entry.available()
    case=loaded['positive.py'].Case(owner,ownership,base,copies,loaded['bridge.py'],
        artifacts,loaded['artifacts.py'],loaded['images.py'],loaded['controller.py'],
        loaded['helper.py'],loaded['streams.py'],loaded['bootstrap.py'])
    owner.retained.append(case);entry.available()
    owner.phase('after_case_constructor');entry.available()
    owner.phase('before_case_run');entry.available()
    result=owner.perform(case.run);entry.available()
    owner.phase('after_case_run');entry.available()
    owner.phase('before_case_receipt_validation');entry.available()
    entry.call(validator.validate_case,result,graph.raw['copy-manifest.json'])
    owner.phase('after_case_receipt_validation');entry.available()
    record={'schema':'retained-positive-inner-record-v1','namespace_boundary_checked':True,
            'case':result,'parent_whole_known_zero':False,'production_effect_authority':False}
    raw=entry.call(json.dumps,record,sort_keys=True,separators=(',',':'),allow_nan=False).encode('ascii')+b'\n'
    owner.phase('before_inner_record_output');entry.available()
    owner.available();entry.output(raw)


@guarded
def result_bytes(entry,fd):
    before=entry.call(os.fstat,fd)
    require(stat.S_ISREG(before.st_mode) and before.st_uid==before.st_gid==1000
        and stat.S_IMODE(before.st_mode)==0o600 and before.st_nlink==1
        and 0<before.st_size<=4*1024*1024)
    data=b''
    while len(data)<before.st_size:
        part=entry.call(os.pread,fd,min(65536,before.st_size-len(data)),len(data))
        require(part);data+=part
    require(entry.call(os.pread,fd,1,len(data))==b''
        and identity(before)==identity(entry.call(os.fstat,fd))
        ==identity(entry.call(os.stat,STAGE+'/scratch/inventory/child.result',follow_symlinks=False)))
    return data


def parent(entry):
    require(os.getresuid()==os.getresgid()==(1000,1000,1000))
    graph,loaded,ownership,owner,base,validator=modules(entry,'outer')
    native_originals(entry,loaded['artifacts.py'])
    owner.perform(base.verify_subordinates);entry.available()
    original=namespace_frame({name:entry.call(base.namespace,name) for name in NS})
    entry.call(resource.setrlimit,resource.RLIMIT_NOFILE,(512,512))
    directories(entry)
    result=entry.opened(STAGE+'/scratch/inventory/child.result',WRITE,0o600)
    error=entry.opened(STAGE+'/scratch/inventory/child.stderr',WRITE,0o600)
    env={'PATH':'/usr/bin','HOME':'/tmp','LANG':'C','LC_ALL':'C',
         'TMPDIR':STAGE+'/scratch','GOMAXPROCS':'2'}
    namespace_deadline=owner.local_deadline(65)
    owner.within(namespace_deadline);entry.available()
    owner.deadline=min(owner.deadline,namespace_deadline)
    entry.deadline=min(entry.deadline,namespace_deadline)
    owner.available();entry.available()
    frame={'original_ns':original,'absolute_deadline':namespace_deadline}
    argv=['/usr/bin/unshare','--user','--map-root-user','--map-users=974:100001:1',
        '--map-groups=974:100001:1','--map-users=1000:100000:1','--map-groups=1000:100000:1',
        '--net','--mount','--pid','--uts','--fork','/usr/bin/python3','-I','-B',
        STAGE+'/launcher.py','--isolated-child',json.dumps(frame,sort_keys=True,separators=(',',':'))]
    spawn_deadline=owner.local_deadline(5)
    owner.within(spawn_deadline);entry.available()
    namespace=owner.spawn(argv,role='namespace',stdin=subprocess.DEVNULL,stdout=result,stderr=error,
                          env=env,close_fds=True)
    owner.within(spawn_deadline)
    entry.available()
    owner.settle_zero(namespace,65);entry.available()
    zero=owner.complete();entry.available()
    # No result/file/baseline followup before independently exact raw-zero child.
    record=entry.call(validator.decode,result_bytes(entry,result))
    require(type(record) is dict and set(record)=={'schema','namespace_boundary_checked','case',
        'parent_whole_known_zero','production_effect_authority'}
        and record['schema']=='retained-positive-inner-record-v1'
        and record['namespace_boundary_checked'] is True and type(record['namespace_boundary_checked']) is bool
        and record['parent_whole_known_zero'] is False and record['production_effect_authority'] is False)
    entry.call(validator.validate_case,record['case'],graph.raw['copy-manifest.json'])
    entry.call(validator.ledger,zero,'outer')
    whole={'schema':'retained-positive-private-record-v1','inner':record,'outer_owned_zero':zero,
        'namespace_child_known_zero':True,'parent_whole_known_zero':False,
        'canonical_baseline_proven':False,'production_effect_authority':False,
        'installed_compatibility':False,'normal_owner_adoption':False}
    raw=entry.call(json.dumps,whole,sort_keys=True,separators=(',',':'),allow_nan=False).encode('ascii')+b'\n'
    output=entry.opened(STAGE+'/result.json',WRITE,0o600)
    written=entry.call(os.write,output,raw);require(type(written) is int and written==len(raw))
    entry.call(os.fsync,output)
    value=entry.call(os.fstat,output)
    require(stat.S_ISREG(value.st_mode) and value.st_uid==value.st_gid==1000
        and value.st_nlink==1 and stat.S_IMODE(value.st_mode)==0o600 and value.st_size==len(raw))
    owner.available();entry.output(b'RETAINED_POSITIVE_RECORDED_ONLY\n')


def main(argv):
    isolated=os.getpid()==1
    entry=Entry.__new__(Entry);HELD.append(entry)
    try:
        entry.__init__()
        require(type(argv) is list and len(argv)<=2
            and all(type(argument) is str and len(argument)<=1024 for argument in argv))
        if argv==RUN:parent(entry)
        elif len(argv)==2 and argv[0]=='--isolated-child':
            require(len(argv[1])<=1024)
            def pairs(items):
                value={}
                for key,item in items:require(key not in value);value[key]=item
                return value
            child(entry,json.loads(argv[1],object_pairs_hook=pairs))
        else:raise Refused()
        return 0
    except BaseException:
        entry.sealed=True
        if isolated:
            # Preserve every owned role/FD on first uncertainty. No destructor,
            # output, query, signal, re-exec or cleanup is allowed from here.
            while True:signal.pause()
        return 1


if __name__=='__main__':raise SystemExit(main(sys.argv[1:]))
