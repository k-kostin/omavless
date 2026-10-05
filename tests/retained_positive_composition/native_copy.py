"""Developer-only native byte copies into a fresh retained executable tmpfs.

No entry point or arbitrary input/utility selector. The fixed launcher supplies
all four already-admitted source originals AFTER the packaged ELF Bridge is
ready. Copying changes filesystem identity, not bytes or mapping predicates.
Every first uncertainty retains all originals, destinations and partial writers.
"""
import fcntl
import functools
import hashlib
import math
import os
import re
import stat

DIRECTORY = '/artifacts'
FLAGS = os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC


class Refused(RuntimeError):
    def __init__(self):super().__init__('fixed_native_tmpfs_copy_refused')


def require(value):
    if not value:raise Refused()


def guarded(method):
    @functools.wraps(method)
    def call(self,*args,**kwargs):
        try:
            self.available();value=method(self,*args,**kwargs);self.available();return value
        except BaseException:
            self.refuse();raise Refused() from None
    return call


class NativeStore:
    def __init__(self, owner, ownership, originals, artifacts):
        self.owner,self.ownership,self.artifacts=owner,ownership,artifacts
        self.sealed=True;self.ready=False;self.held=[];self.iterators=[]
        self.originals=originals;self.files={};self.pending=set()
        try:
            require(type(owner) is ownership.Session and owner.kind=='inner' and owner.isolated
                and os.getpid()==1 and os.getresuid()==os.getresgid()==(0,0,0)
                and any(value is self for value in owner.retained)
                and set(originals)==set(artifacts.TABLE) and len(originals)==4
                and sum(row[0] for row in artifacts.TABLE.values())<=128*1024*1024)
            self.deadline=owner.local_deadline(20)
            self.sealed=False
            self.source_check()  # All originals BEFORE the first new mount.
            self.covered=self.opened(DIRECTORY,FLAGS|os.O_DIRECTORY)
            covered=self.call(os.fstat,self.covered)
            self.covered_identity=artifacts.identity(covered)
            require(stat.S_ISDIR(covered.st_mode) and covered.st_uid==covered.st_gid==0
                and stat.S_IMODE(covered.st_mode)==0o755 and not self.call(os.listxattr,self.covered)
                and self.covered_identity==artifacts.identity(self.call(os.stat,DIRECTORY,follow_symlinks=False)))
            self.directory=self.covered
            require(self.members()==set(artifacts.TABLE))
            for name,(fd,before) in originals.items():
                require(artifacts.identity(before)==artifacts.identity(self.call(
                    os.stat,name,dir_fd=self.covered,follow_symlinks=False)))
            rows=self.mount_rows();require(len(rows)==1)
            self.covered_row=rows[0]
            split=self.covered_row.index('-')
            require(split>=6 and len(self.covered_row)==split+4
                and {'ro','nosuid','nodev'}<=set(self.covered_row[5].split(','))
                and 'rw' not in self.covered_row[5].split(',')
                and self.call(os.fstatvfs,self.covered).f_flag&os.ST_RDONLY)
            self.call(owner.command,['/usr/bin/mount','-t','tmpfs','-o',
                'size=128m,mode=0755,nosuid,nodev','tmpfs',DIRECTORY])
            self.directory=self.opened(DIRECTORY,FLAGS|os.O_DIRECTORY)
            directory=self.call(os.fstat,self.directory)
            require(stat.S_ISDIR(directory.st_mode) and directory.st_uid==directory.st_gid==0
                and stat.S_IMODE(directory.st_mode)==0o755 and not self.call(os.listxattr,self.directory))
            self.device=directory.st_dev
            self.mount_policy(False)
            require(self.members()==set())
            for name in sorted(artifacts.TABLE):self.copy(name)
            self.directory_identity=artifacts.identity(self.call(os.fstat,self.directory))
            self.source_check()
            self.no_writers()
            self.call(owner.command,['/usr/bin/mount','-o','remount,ro,nosuid,nodev',DIRECTORY])
            self.verify()
            self.available();self.ready=True
            # The20s stage cap is not a lifetime for later admitted reads.
            # No absolute Session cap is extended: later verification uses it
            # plus a fresh15s LOCAL cap and any stricter caller cap supplied.
            self.owner.available();self.deadline=self.owner.deadline;self.available()
        except BaseException:
            self.refuse();raise Refused() from None

    def refuse(self):self.sealed=self.owner.sealed=True

    def available(self):
        try:
            require(not self.sealed and type(self.deadline) is float and math.isfinite(self.deadline))
            self.owner.within(self.deadline)
        except BaseException:
            self.refuse();raise Refused() from None

    def call(self,operation,*args,**kwargs):
        try:
            self.available();value=operation(*args,**kwargs);self.available();return value
        except BaseException:
            self.refuse();raise Refused() from None

    def opened(self,*args,**kwargs):
        try:
            self.available();fd=os.open(*args,**kwargs);self.held.append(fd)
            self.available();require(type(fd) is int and 3<=fd<512);return fd
        except BaseException:
            self.refuse();raise Refused() from None

    @guarded
    def digest(self,fd,size):
        require(type(size) is int and 0<size<=64*1024*1024)
        sha=hashlib.sha256();offset=0
        while offset<size:
            raw=self.call(os.pread,fd,min(65536,size-offset),offset)
            require(type(raw) is bytes and 0<len(raw)<=min(65536,size-offset))
            sha.update(raw);offset+=len(raw);self.available()
        require(self.call(os.pread,fd,1,size)==b'')
        return sha.hexdigest()

    @guarded
    def source_check(self):
        for name,(fd,before) in self.originals.items():
            size,mode,sha=self.artifacts.TABLE[name]
            require(stat.S_ISREG(before.st_mode) and before.st_uid==before.st_gid==0
                and before.st_nlink==1 and before.st_size==size
                and stat.S_IMODE(before.st_mode)==mode
                and self.artifacts.identity(before)==self.artifacts.identity(self.call(os.fstat,fd))
                and not self.call(os.listxattr,fd) and self.digest(fd,size)==sha
                and self.artifacts.identity(before)==self.artifacts.identity(self.call(os.fstat,fd)))

    @guarded
    def members(self):
        self.call(os.lseek,self.directory,0,os.SEEK_SET)
        self.available();entries=os.scandir(self.directory);self.iterators.append(entries);self.available()
        seen=set();sentinel=object()
        while True:
            entry=self.call(next,entries,sentinel)
            if entry is sentinel:break
            require(entry.name in self.artifacts.TABLE and entry.name not in seen and len(seen)<4)
            seen.add(entry.name)
        return seen

    @guarded
    def copy(self,name):
        size,mode,sha=self.artifacts.TABLE[name];source,before=self.originals[name]
        writer=self.opened(name,os.O_WRONLY|os.O_CREAT|os.O_EXCL|os.O_NOFOLLOW|os.O_CLOEXEC,
            0o600,dir_fd=self.directory)
        self.pending.add(writer)
        offset=0
        while offset<size:
            raw=self.call(os.pread,source,min(65536,size-offset),offset)
            require(type(raw) is bytes and 0<len(raw)<=min(65536,size-offset))
            written=self.call(os.write,writer,raw)
            require(type(written) is int and written==len(raw));offset+=len(raw)
        require(self.call(os.pread,source,1,size)==b''
            and self.artifacts.identity(before)==self.artifacts.identity(self.call(os.fstat,source)))
        self.call(os.fchmod,writer,mode);self.call(os.fsync,writer)
        value=self.call(os.fstat,writer)
        require(stat.S_ISREG(value.st_mode) and value.st_uid==value.st_gid==0
            and value.st_nlink==1 and value.st_size==size and stat.S_IMODE(value.st_mode)==mode
            and value.st_dev==self.device and not self.call(os.listxattr,writer))
        # Only known positive completion authorizes this writer close. A late
        # or unknown close seals before readonly reopen or any later mutation.
        require(self.call(os.close,writer) is None)
        self.pending.remove(writer);self.held.remove(writer)
        fd=self.opened(name,FLAGS,dir_fd=self.directory)
        self.files[name]=(fd,value)
        require(self.artifacts.identity(value)==self.artifacts.identity(self.call(os.fstat,fd))
            ==self.artifacts.identity(self.call(os.stat,name,dir_fd=self.directory,follow_symlinks=False))
            and self.digest(fd,size)==sha
            and self.artifacts.identity(value)==self.artifacts.identity(self.call(os.fstat,fd)))

    @guarded
    def mount_rows(self):
        fd=self.opened('/proc/self/mountinfo',FLAGS);raw=b''
        while True:
            part=self.call(os.read,fd,min(65536,1024*1024+1-len(raw)))
            require(type(part) is bytes);raw+=part;require(len(raw)<=1024*1024)
            if not part:break
        rows=[]
        text=raw.decode('ascii','strict')
        require(text.endswith('\n'))
        for line in text[:-1].split('\n'):
            require(line)
            row=line.split();require(len(row)>=10)
            if row[4]==DIRECTORY:
                require(len(rows)<2);rows.append(row)
        return rows

    @guarded
    def mount_policy(self,readonly):
        require(type(readonly) is bool)
        rows=self.mount_rows()
        # Only the exact previously admitted covered bind plus the one fresh
        # tmpfs is permitted. No arbitrary additional /artifacts mount row.
        require(len(rows)==2 and rows.count(self.covered_row)==1)
        current=[row for row in rows if row!=self.covered_row]
        require(len(current)==1);row=current[0];split=row.index('-')
        require(split>=6 and len(row)==split+4 and row[split+1]=='tmpfs'
            and row[3]=='/' and row[2]==str(os.major(self.device))+':'+str(os.minor(self.device)))
        flags=set(row[5].split(','));superflags=set(row[split+3].split(','))
        expected,opposite=('ro','rw') if readonly else ('rw','ro')
        require({'nosuid','nodev',expected}<=flags and 'noexec' not in flags
            and opposite not in flags and expected in superflags and opposite not in superflags
            and bool(self.call(os.fstatvfs,self.directory).f_flag&os.ST_RDONLY)==readonly)

    @guarded
    def no_writers(self):
        directory=self.opened('/proc/self/fd',FLAGS|os.O_DIRECTORY)
        self.available();entries=os.scandir(directory);self.iterators.append(entries);self.available()
        seen=set();sentinel=object()
        while True:
            entry=self.call(next,entries,sentinel)
            if entry is sentinel:break
            name=entry.name
            require(type(name) is str and re.fullmatch(r'0|[1-9][0-9]{0,2}',name)
                and int(name)<512 and name not in seen and len(seen)<512)
            seen.add(name);fd=int(name);value=self.call(os.fstat,fd)
            if value.st_dev==self.device:
                require(self.call(fcntl.fcntl,fd,fcntl.F_GETFL)&os.O_ACCMODE==os.O_RDONLY)
        require(str(directory) in seen and not self.pending)

    @guarded
    def verify(self,enclosing_deadline=None):
        previous=self.deadline
        local=self.owner.local_deadline(15)
        require(type(local) is float and math.isfinite(local))
        if enclosing_deadline is not None:
            require(type(enclosing_deadline) is float and math.isfinite(enclosing_deadline)
                and enclosing_deadline<=previous)
            local=min(local,enclosing_deadline)
        self.deadline=min(previous,local);self.available()
        self.available();require(set(self.files)==set(self.artifacts.TABLE) and not self.pending)
        require(self.directory_identity==self.artifacts.identity(self.call(os.fstat,self.directory))
            ==self.artifacts.identity(self.call(os.stat,DIRECTORY,follow_symlinks=False))
            and self.members()==set(self.artifacts.TABLE))
        self.mount_policy(True);self.no_writers();self.source_check()
        require(self.covered_identity==self.artifacts.identity(self.call(os.fstat,self.covered)))
        for name,(fd,before) in self.files.items():
            require(self.artifacts.identity(before)==self.artifacts.identity(self.call(os.fstat,fd))
                ==self.artifacts.identity(self.call(os.stat,name,dir_fd=self.directory,follow_symlinks=False))
                and not self.call(os.listxattr,fd)
                and self.digest(fd,before.st_size)==self.artifacts.TABLE[name][2]
                and self.artifacts.identity(before)==self.artifacts.identity(self.call(os.fstat,fd)))
        require(self.members()==set(self.artifacts.TABLE));self.available()
        self.deadline=previous
