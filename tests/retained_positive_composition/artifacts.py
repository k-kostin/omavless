"""Fixed unsigned developer artifacts: original-FD admission, never execution.

Used only AFTER the pinned launcher creates and seals its private /artifacts
native byte-copy tmpfs. These destination originals are independent of the
25 copied packaged objects; native source provenance is checked separately.
HOST-to-guest delivery provenance and current loaded process identity remain
separate proof steps. No path/hash from a receipt can select another input.
"""
import hashlib
import json
import math
import os
import stat
import time

DIRECTORY = '/artifacts'
OWNER = 0
TABLE = {
    'developer-manifest.json': (1235, 0o600, '65925070cd83b2af177bbfa4fbb7b821cdc65e855db53671528b2da03bb621cd'),
    'mihomo': (61083808, 0o555, '3b1da75d3c9fd8440216f9c256c6c59da812faae88debc936f3c72fef9724544'),
    'omavless-dns-broker': (5126984, 0o555, 'ea958302d745b901294df6164c624a431a7493b67457a255306ec8216545eb9d'),
    'host-fixture': (49630768, 0o555, 'fbd19fc83f5d6548ff8f1fe89d4d66ab9032d85cb551a2364f5719a5cad234f7'),
}
ROLES = {'core':'mihomo', 'broker':'omavless-dns-broker', 'host':'host-fixture'}
FLAGS = os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC


class Refused(RuntimeError):
    def __init__(self):
        super().__init__('fixed_developer_artifacts_refused')


def require(value):
    if not value:
        raise Refused()


def identity(value):
    return (value.st_dev,value.st_ino,value.st_mode,value.st_uid,value.st_gid,
            value.st_nlink,value.st_size,value.st_mtime_ns,value.st_ctime_ns)


def pairs(items):
    result={}
    for key,value in items:
        require(key not in result)
        result[key]=value
    return result


class Sources:
    def __init__(self,native,native_module):
        self.sealed=True
        self.held,self.iterators,self.files=[],[],{}
        start=time.monotonic()
        require(type(start) is float and math.isfinite(start))
        self.deadline=start+90.0
        self.local_budget=start+20.0
        self.sealed=False
        try:
            self.available()
            require(type(native) is native_module.NativeStore and native.ready is True
                    and not native.sealed and native.artifacts.TABLE is TABLE)
            self.native=native
            require(type(native.deadline) is float and math.isfinite(native.deadline))
            self.deadline=min(self.deadline,native.deadline)
            self.local_budget=min(self.local_budget,self.deadline)
            require(os.getresuid()==os.getresgid()==(OWNER,OWNER,OWNER))
            self.root=self.open('/',FLAGS|os.O_DIRECTORY)
            self.root_identity=identity(self.io(os.fstat,self.root))
            root=self.io(os.fstat,self.root)
            require(stat.S_ISDIR(root.st_mode) and root.st_uid==root.st_gid==OWNER
                    and not root.st_mode&0o6022)
            self.directory=self.open('artifacts',FLAGS|os.O_DIRECTORY,dir_fd=self.root)
            info=self.io(os.fstat,self.directory)
            require(stat.S_ISDIR(info.st_mode) and info.st_uid==info.st_gid==OWNER
                    and stat.S_IMODE(info.st_mode)==0o755)
            self.directory_identity=identity(info)
            require(self.members()==set(TABLE))
            require(sum(row[0] for row in TABLE.values())<=128*1024*1024)
            for name,(size,mode,expected) in TABLE.items():
                fd=self.open(name,FLAGS,dir_fd=self.directory)
                value=self.io(os.fstat,fd)
                require(stat.S_ISREG(value.st_mode) and value.st_uid==value.st_gid==OWNER
                        and value.st_nlink==1 and value.st_size==size and 0<size<=64*1024*1024
                        and stat.S_IMODE(value.st_mode)==mode and not self.io(os.listxattr,fd))
                require(self.digest(fd,size)==expected)
                if name!='developer-manifest.json':
                    header=self.io(os.pread,fd,64,0)
                    require(len(header)==64 and header[:6]==b'\x7fELF\x02\x01'
                            and header[18:20]==b'\x3e\x00')
                self.files[name]=(fd,value,expected)
            manifest=self.io(os.pread,self.files['developer-manifest.json'][0],
                             TABLE['developer-manifest.json'][0]+1,0)
            require(len(manifest)==TABLE['developer-manifest.json'][0]
                    and hashlib.sha256(manifest).hexdigest()==TABLE['developer-manifest.json'][2])
            value=json.loads(manifest,object_pairs_hook=pairs,
                             parse_constant=lambda _:require(False))
            require(value['schema']=='omavless-composed-developer-artifacts-v2'
                    and value['builder_source']=='8c038e76c8407eebd7afdd6e0389fc2bbc28cab9'
                    and value['dns_source']=='c4e800425243c1b02165f82153e4bf418fe465e6'
                    and value['broker_source']=='aff0c38075338d51d979acc9f10dab1ae6dbba6f'
                    and value['architecture']=='x86_64' and value['broker_feature']=='release-package')
            require(all(value[key] is False for key in
                    ('broker_executed','installed_compatibility','package_attestation','effect_authority')))
            require(value['sha256']['mihomo']==TABLE['mihomo'][2]
                    and value['sha256']['omavless-dns-broker']==TABLE['omavless-dns-broker'][2]
                    and value['sha256']['host-fixture']==TABLE['host-fixture'][2])
            self.recheck()
            self.available()
            self.local_budget=None
        except BaseException:
            self.sealed=True
            raise Refused() from None

    def available(self):
        try:
            now=time.monotonic()
            require(not self.sealed and type(now) is float and math.isfinite(now)
                    and type(self.deadline) is float and math.isfinite(self.deadline)
                    and now<self.deadline)
            if self.local_budget is not None:
                require(type(self.local_budget) is float and math.isfinite(self.local_budget)
                        and now<self.local_budget)
        except BaseException:
            self.sealed=True
            raise Refused() from None

    def io(self,operation,*args,**kwargs):
        try:
            self.available()
            value=operation(*args,**kwargs)
            self.available()
            return value
        except BaseException:
            self.sealed=True
            raise Refused() from None

    def open(self,*args,**kwargs):
        try:
            self.available()
            fd=os.open(*args,**kwargs)
            self.held.append(fd)
            self.available()
            return fd
        except BaseException:
            self.sealed=True
            raise Refused() from None

    def members(self):
        try:
            self.io(os.lseek,self.directory,0,os.SEEK_SET)
            self.available();entries=os.scandir(self.directory);self.iterators.append(entries);self.available()
            result=set();sentinel=object()
            while True:
                entry=self.io(next,entries,sentinel)
                if entry is sentinel:break
                require(entry.name in TABLE and entry.name not in result and len(result)<4)
                result.add(entry.name)
            require(result==set(TABLE))
            return result
        except BaseException:
            self.sealed=True
            raise Refused() from None

    def digest(self,fd,size):
        try:
            sha=hashlib.sha256();offset=0
            while offset<size:
                raw=self.io(os.pread,fd,min(65536,size-offset),offset)
                require(raw);sha.update(raw);offset+=len(raw)
            require(self.io(os.pread,fd,1,offset)==b'')
            return sha.hexdigest()
        except BaseException:
            self.sealed=True
            raise Refused() from None

    def recheck(self,enclosing_deadline=None):
        try:
            self.available()
            old_budget=self.local_budget
            start=time.monotonic()
            require(type(start) is float and math.isfinite(start))
            local=start+15.0
            require(math.isfinite(local) and start<local)
            if enclosing_deadline is not None:
                require(type(enclosing_deadline) is float and math.isfinite(enclosing_deadline)
                        and enclosing_deadline<=self.deadline)
                local=min(local,enclosing_deadline)
            self.local_budget=min(local,self.deadline) if old_budget is None else min(local,self.deadline,old_budget)
            self.available()
            self.io(self.native.verify,self.local_budget)
            require(identity(self.io(os.fstat,self.root))==self.root_identity
                    ==identity(self.io(os.stat,'/',follow_symlinks=False)))
            require(identity(self.io(os.fstat,self.directory))==self.directory_identity
                    ==identity(self.io(os.stat,'artifacts',dir_fd=self.root,follow_symlinks=False)))
            require(self.members()==set(TABLE) and set(self.files)==set(TABLE))
            for name,(fd,before,sha) in self.files.items():
                require(identity(before)==identity(self.io(os.fstat,fd))
                        ==identity(self.io(os.stat,name,dir_fd=self.directory,follow_symlinks=False))
                        and not self.io(os.listxattr,fd) and self.digest(fd,before.st_size)==sha)
                require(identity(before)==identity(self.io(os.fstat,fd))
                        ==identity(self.io(os.stat,name,dir_fd=self.directory,follow_symlinks=False)))
            require(self.members()==set(TABLE))
            self.available()
            self.local_budget=old_budget
        except BaseException:
            self.sealed=True
            raise Refused() from None

    def mapped_identity(self,name,device,inode,enclosing_deadline=None):
        """Membership/device/inode BEFORE hash; caller owns live/map reread proof."""
        try:
            self.available()
            require(name in ROLES.values() and type(device) is int and type(inode) is int)
            fd,before,sha=self.files[name]
            require((device,inode)==(before.st_dev,before.st_ino))
            self.recheck(enclosing_deadline)
            return {'path':DIRECTORY+'/'+name,'device':device,'inode':inode,
                    'size':before.st_size,'sha256':sha}
        except BaseException:
            self.sealed=True
            raise Refused() from None
