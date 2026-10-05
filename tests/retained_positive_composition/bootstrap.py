"""Fixed fresh-fixture Unix socket permission bootstrap, not production IPC.

Pinned upstream listens with0666. Retain its ORIGINAL pathname-socket O_PATH
inode and change only that original through this PID1's held kernel FD alias.
Named replacement/late chmod is terminal; normal Controller must separately
repeat0600 admission and exact core SO_PEERCRED before every request write.
"""
import math
import os
import stat

FLAGS=os.O_PATH|os.O_NOFOLLOW|os.O_CLOEXEC
KEYS=('st_dev','st_ino','st_mode','st_uid','st_gid','st_nlink')
DIRECTORY='/home/core'


class Refused(RuntimeError):
    def __init__(self):super().__init__('fixed_controller_bootstrap_refused')


def require(value):
    if not value:raise Refused()


def identity(value):
    require(all(type(getattr(value,key)) is int for key in KEYS))
    return tuple(getattr(value,key) for key in KEYS)


class Bootstrap:
    def __init__(self, owner, ownership, images, image_module):
        self.owner,self.ownership,self.images=owner,ownership,images
        self.sealed=True;self.done=False;self.held=[]
        try:
            require(type(owner) is ownership.Session and owner.kind=='inner' and owner.isolated
                    and os.getpid()==1 and os.geteuid()==os.getegid()==0
                    and type(images) is image_module.Images and images.owner is owner
                    and not hasattr(owner,'controller_bootstrap'))
            self.core=owner.anchors['core']['child']
            require(type(self.core) is ownership.OwnedProcess and owner.roles.get(id(self.core))=='core'
                    and any(child is self.core for child in owner.children)
                    and owner.anchors['core']['state']=='spawned')
            owner.controller_bootstrap=self;owner.retained.append(self);owner.available()
            self.deadline=owner.local_deadline(5);self.sealed=False
            self.call(images.executable,self.core,'core',self.deadline)
            self.directory=self.opened(DIRECTORY,FLAGS|os.O_DIRECTORY)
            parent=self.call(os.fstat,self.directory)
            self.directory_identity=identity(parent)
            require(stat.S_ISDIR(parent.st_mode) and parent.st_uid==parent.st_gid==1000
                    and stat.S_IMODE(parent.st_mode)==0o700
                    and self.directory_identity==identity(self.call(os.stat,DIRECTORY,follow_symlinks=False)))
            self.socket=self.opened('controller.sock',FLAGS,dir_fd=self.directory)
            self.socket_identity=identity(self.call(os.fstat,self.socket))
            # Only the already-private, PID1-owned proc mount made by the pinned
            # containment is reached. No caller FD/path or generic chmod API.
            self.alias_directory=self.opened('/proc/1/fd',FLAGS|os.O_DIRECTORY)
            self.alias_identity=identity(self.call(os.fstat,self.alias_directory))
            self.check(0o666)
            require(self.call(os.chmod,str(self.socket),0o600,dir_fd=self.alias_directory) is None)
            self.check(0o600)
            self.available();self.done=True
        except BaseException:
            self.refuse();raise Refused() from None

    def refuse(self):
        self.sealed=self.owner.sealed=True

    def available(self):
        try:
            require(not self.sealed)
            self.owner.within(self.deadline)
            now=self.ownership.clock()
            require(type(now) is float and math.isfinite(now)
                    and type(self.deadline) is float and math.isfinite(self.deadline) and now < self.deadline)
        except BaseException:
            self.refuse();raise Refused() from None

    def call(self, operation, *args, **kwargs):
        try:
            self.available();result=operation(*args,**kwargs);self.available();return result
        except BaseException:
            self.refuse();raise Refused() from None

    def opened(self, *args, **kwargs):
        try:
            self.available();fd=os.open(*args,**kwargs);self.held.append(fd)
            self.available();require(type(fd) is int and 3 <= fd < 512);return fd
        except BaseException:
            self.refuse();raise Refused() from None

    def check(self, mode):
        try:
            require(type(mode) is int and mode in (0o666,0o600)
                    and self.owner.controller_bootstrap is self
                    and self.owner.anchors['core']['child'] is self.core
                    and self.owner.anchors['core']['state']=='spawned')
            self.call(self.owner.live,self.core)
            self.call(self.images.executable,self.core,'core',self.deadline)
            directory=self.call(os.fstat,self.directory)
            require(stat.S_ISDIR(directory.st_mode) and directory.st_uid==directory.st_gid==1000
                    and stat.S_IMODE(directory.st_mode)==0o700
                    and identity(directory)==self.directory_identity
                    ==identity(self.call(os.stat,DIRECTORY,follow_symlinks=False)))
            aliases=self.call(os.fstat,self.alias_directory)
            require(stat.S_ISDIR(aliases.st_mode) and aliases.st_uid==aliases.st_gid==0
                    and stat.S_IMODE(aliases.st_mode)==0o500
                    and identity(aliases)==self.alias_identity
                    ==identity(self.call(os.stat,'/proc/1/fd',follow_symlinks=False)))
            original=self.call(os.fstat,self.socket)
            require(stat.S_ISSOCK(original.st_mode) and original.st_uid==original.st_gid==1000
                    and original.st_nlink==1 and stat.S_IMODE(original.st_mode)==mode)
            expected=list(self.socket_identity);expected[2]=stat.S_IFSOCK|mode
            require(identity(original)==tuple(expected)
                    ==identity(self.call(os.stat,'controller.sock',dir_fd=self.directory,follow_symlinks=False))
                    ==identity(self.call(os.stat,str(self.socket),dir_fd=self.alias_directory)))
            self.call(self.owner.live,self.core);self.available()
        except BaseException:
            self.refuse();raise Refused() from None

    def receipt(self):
        try:
            self.available();require(self.done is True)
            return {'fresh_original_controller_socket_mode_0600':True,'production_effect_authority':False}
        except BaseException:
            self.refuse();raise Refused() from None
