#!/usr/bin/python3
"""Fixed create-only fresh delivery. Original inputs stay held through exec."""
import hashlib
import os
from pathlib import Path
import pwd
import stat
import sys
import time

SOURCE = Path('/home/kdk_vm/.cache/k1-retained-lease-stage-v1')
DESTINATION = Path('/run/omavless-k1-retained-lease-regression')
MEMBERS = {
    'probe': ('0000000000000000000000000000000000000000000000000000000000000000', 0o500, 0o500, 128*1024*1024),
    'query-guard.py': ('8485879b68937c15908add274d61edcd569a4940685cc9ff201cb718f3f7d85a', 0o400, 0o600, 256*1024),
    'fixture.service': ('f5d461381beeaf4846cc6113b2d81d2131ef28441cb33f6ee15362725527c78e', 0o400, 0o600, 16384),
    'guard.py': ('9bd1f44bca9592880c912bfe22eeb90e92783095e300ddc5e84c8e47b33c5413', 0o400, 0o500, 256*1024),
}
HELD = []
DEADLINE = float('inf')
TERMINAL = False


def require(value):
    if not value or time.monotonic() >= DEADLINE:
        raise RuntimeError('fixed_retained_lease_staging_refused')


def identity(s):
    return (s.st_dev, s.st_ino, s.st_mode, s.st_uid, s.st_gid,
            s.st_nlink, s.st_size, s.st_mtime_ns, s.st_ctime_ns)


def checked(operation, *args, **kwargs):
    require(True)
    value = operation(*args, **kwargs)
    require(True)
    return value


def opened(name, flags, mode=0o600, *, dir_fd=None):
    require(True)
    fd = os.open(name, flags, mode, dir_fd=dir_fd)
    HELD.append(fd)
    require(True)
    return fd


def catalog(fd):
    names = set()
    checked(os.lseek, fd, 0, os.SEEK_SET)
    with checked(os.scandir, fd) as entries:
        for entry in entries:
            require(entry.name in set(MEMBERS)|{'root-stage.py'} and entry.name not in names)
            names.add(entry.name)
    require(names == set(MEMBERS)|{'root-stage.py'})


class Delivery:
    def __init__(self):
        self.sealed = False
        self.parents, self.files = [], {}
        try:
            parent = None
            for path in reversed((SOURCE, *SOURCE.parents)):
                fd = opened('/' if parent is None else path.name,
                    os.O_RDONLY|os.O_DIRECTORY|os.O_NOFOLLOW|os.O_CLOEXEC, dir_fd=parent)
                info = checked(os.fstat, fd)
                require(stat.S_ISDIR(info.st_mode) and info.st_uid == info.st_gid
                    and info.st_uid in (0,1000) and not info.st_mode & 0o6022)
                self.parents.append((path,fd,info,parent))
                parent = fd
            require(info.st_uid == 1000 and stat.S_IMODE(info.st_mode) == 0o700)
            self.source = parent
            self.run = opened('/run', os.O_RDONLY|os.O_DIRECTORY|os.O_NOFOLLOW|os.O_CLOEXEC)
            run = checked(os.fstat,self.run)
            require(run.st_uid == run.st_gid == 0 and not run.st_mode & 0o6022)
            self.run_meta = run
            catalog(self.source)
            for name, (sha,mode,_,cap) in MEMBERS.items():
                fd = opened(name,os.O_RDONLY|os.O_NOFOLLOW|os.O_NONBLOCK|os.O_CLOEXEC,dir_fd=self.source)
                info = checked(os.fstat,fd)
                require(stat.S_ISREG(info.st_mode) and info.st_uid == info.st_gid == 1000
                    and stat.S_IMODE(info.st_mode) == mode and info.st_nlink == 1
                    and 0 < info.st_size <= cap and not checked(os.listxattr,fd))
                raw = checked(os.pread,fd,info.st_size+1,0)
                require(type(raw) is bytes and len(raw) == info.st_size and hashlib.sha256(raw).hexdigest() == sha)
                self.files[name]=(fd,info,raw)
            self.recheck()
        except BaseException:
            self.sealed = True
            raise

    def available(self):
        try:
            require(not self.sealed)
        except BaseException:
            self.sealed = True
            raise

    def recheck(self):
        self.available()
        try:
            for path,fd,info,parent in self.parents:
                require(identity(info)==identity(checked(os.fstat,fd))==identity(checked(os.stat,
                    '/' if parent is None else path.name,dir_fd=parent,follow_symlinks=False)))
            require(identity(self.run_meta)[:5]==identity(checked(os.fstat,self.run))[:5]
                ==identity(checked(os.stat,'/run',follow_symlinks=False))[:5])
            catalog(self.source)
            for name,(fd,info,raw) in self.files.items():
                require(identity(info)==identity(checked(os.fstat,fd))==identity(checked(os.stat,name,dir_fd=self.source,follow_symlinks=False))
                    and not checked(os.listxattr,fd) and checked(os.pread,fd,info.st_size+1,0)==raw)
                require(identity(info)==identity(checked(os.fstat,fd))==identity(checked(os.stat,name,dir_fd=self.source,follow_symlinks=False)))
        except BaseException:
            self.sealed = True
            raise

    def publish(self):
        self.available()
        try:
            self.recheck()
            # mkdir is exclusive: presence, including a dangling symlink, refuses.
            checked(os.mkdir,DESTINATION.name,0o700,dir_fd=self.run)
            dest=opened(DESTINATION.name,os.O_RDONLY|os.O_DIRECTORY|os.O_NOFOLLOW|os.O_CLOEXEC,dir_fd=self.run)
            original=checked(os.fstat,dest)
            require(original.st_uid==original.st_gid==0 and stat.S_IMODE(original.st_mode)==0o700)
            published=[]
            for name,(_,_,mode,_) in MEMBERS.items():
                self.recheck()
                fd=opened(name,os.O_RDWR|os.O_CREAT|os.O_EXCL|os.O_NOFOLLOW|os.O_CLOEXEC,mode,dir_fd=dest)
                count=checked(os.write,fd,self.files[name][2])
                require(type(count) is int and count==len(self.files[name][2]))
                checked(os.fsync,fd)
                info=checked(os.fstat,fd)
                require(stat.S_ISREG(info.st_mode) and info.st_uid==info.st_gid==0 and info.st_nlink==1
                    and stat.S_IMODE(info.st_mode)==mode and info.st_size==len(self.files[name][2])
                    and not checked(os.listxattr,fd))
                published.append((name,fd,info))
            checked(os.fsync,dest)
            self.recheck()
            for name,fd,info in published:
                require(identity(info)==identity(checked(os.fstat,fd))==identity(checked(os.stat,name,dir_fd=dest,follow_symlinks=False))
                    and checked(os.pread,fd,info.st_size+1,0)==self.files[name][2]
                    and not checked(os.listxattr,fd))
                require(identity(info)==identity(checked(os.fstat,fd))==identity(checked(os.stat,name,dir_fd=dest,follow_symlinks=False)))
            require(identity(original)[:5]==identity(checked(os.fstat,dest))[:5]
                ==identity(checked(os.stat,DESTINATION.name,dir_fd=self.run,follow_symlinks=False))[:5])
            self.sealed=True  # One publication/exec attempt, including failure.
            require(True)
            os.execve('/usr/bin/python3',['/usr/bin/python3','-I','-B',str(DESTINATION/'guard.py')],
                {'PATH':'/usr/bin','LC_ALL':'C','OMAVLESS_K1_RETAINED_LEASE_GUARD':'1'})
            raise RuntimeError('exec_returned')
        finally:
            self.sealed=True


def main():
    global DEADLINE
    DEADLINE=time.monotonic()+180
    require(os.getresuid()==os.getresgid()==(0,0,0) and len(sys.argv)==1
        and pwd.getpwuid(1000).pw_name=='kdk_vm')
    Delivery().publish()


if __name__=='__main__':
    os.umask(0o077)
    try:
        main()
    except BaseException:
        try:
            require(not TERMINAL)
            TERMINAL=True
            raw=b'K1_RETAINED_LEASE_STAGING_NONPASS_RETAINED\n'
            count=os.write(2,raw)
            require(type(count) is int and count==len(raw))
        except BaseException:
            pass
        raise SystemExit(2) from None
