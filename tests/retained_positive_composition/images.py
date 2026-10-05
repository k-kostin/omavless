"""Developer-only retained copied/native loaded-image witness.

No unknown path is opened. The kernel maps text is read only through the live
retained direct child's original proc FD; membership and device/inode precede
all object hash checks. Complete second maps read and live anchor must agree.
"""
import math
import os
import re
import stat
import time

FLAGS = os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC
EXE_FLAGS = os.O_RDONLY | os.O_NONBLOCK | os.O_CLOEXEC
PUBLIC = re.compile(r'(?:/usr/(?:lib|bin)/[A-Za-z0-9_./+:-]+|/artifacts/(?:mihomo|omavless-dns-broker|host-fixture))\Z')
MAP_LINE = re.compile(r'([0-9a-f]{1,16})-([0-9a-f]{1,16}) ([r-][w-][x-][ps]) ([0-9a-f]{1,16}) ([0-9a-f]{1,8}):([0-9a-f]{1,8}) ([0-9]{1,20})(?:[ \t]+([^\r\n]+))?')
ROLES = {'bus':'/usr/bin/dbus-daemon','resolved':'/usr/lib/systemd/systemd-resolved',
         'core':'/artifacts/mihomo','broker':'/artifacts/omavless-dns-broker','host':'/artifacts/host-fixture'}
# Go's Linux runtime places a five-byte " Go: " prefix in a 79-byte
# NUL-terminated VMA name. Only zero-identity anonymous rows may use this
# bounded annotation; it is discarded, never an eligible file/object path.
ANONYMOUS = re.compile(r'(?:\[[A-Za-z0-9_:.-]+\]|\[anon: Go: [A-Za-z][A-Za-z0-9 _.:-]{0,73}\])\Z')

class Refused(RuntimeError):
    def __init__(self):
        super().__init__('fixed_loaded_image_refused')

def require(value, reason=None):
    if not value:
        raise Refused()

def map_objects(text):
    require(type(text) is str and len(text) <= 1024 * 1024, "mapping_bound")
    objects = {}
    previous_end = 0
    for line in text.splitlines():
        match = MAP_LINE.fullmatch(line)
        require(match is not None, "mapping_shape")
        start, end, _, offset, major, minor, inode, path = match.groups()
        start, end, offset = int(start, 16), int(end, 16), int(offset, 16)
        require(previous_end <= start < end <= 2**64 - 1 and offset <= 2**64 - 1,
                "mapping_range")
        previous_end = end
        identity = os.makedev(int(major, 16), int(minor, 16)), int(inode)
        if path is None or path == "" or path.startswith("["):
            require(identity == (0, 0) and offset == 0
                    and (not path or ANONYMOUS.fullmatch(path)),
                    "anonymous_mapping_shape")
            continue
        require(len(path) <= 4096 and PUBLIC.fullmatch(path)
                and all(p not in ("", ".", "..") for p in path.split("/")[1:]),
                "mapping_nonpublic_or_deleted")
        require(0 < identity[1] <= 2**64 - 1 and (path not in objects or objects[path] == identity),
                "mapping_identity_conflict")
        objects[path] = identity
        require(len(objects) <= 64, "mapping_count")
    require(objects, "mapping_empty")
    return objects

class Images:
    def __init__(self, owner, ownership, copies, copy_module, artifacts, artifact_module):
        self.owner, self.ownership, self.copies, self.artifacts = owner, ownership, copies, artifacts
        self.sealed = True
        self.held = []
        try:
            require(type(owner) is ownership.Session and owner.kind == 'inner'
                    and type(copies) is copy_module.Bridge and type(artifacts) is artifact_module.Sources)
            owner.available()
            require(len(copies.records) == len(copies.fds) == 25 and copies.state == 'ready'
                    and set(artifacts.files) == set(artifact_module.TABLE) and not artifacts.sealed)
            self.sealed = False
        except BaseException:
            self.refuse()
            raise Refused() from None

    def refuse(self):
        self.sealed = self.owner.sealed = self.artifacts.sealed = True
        self.copies.state = 'refused'

    def available(self, deadline):
        try:
            require(not self.sealed)
            self.owner.available()
            now = time.monotonic()
            require(type(now) is float and math.isfinite(now)
                    and type(deadline) is float and math.isfinite(deadline) and now < deadline)
        except BaseException:
            self.refuse()
            raise Refused() from None

    def io(self, deadline, operation, *args, **kwargs):
        try:
            self.available(deadline)
            value = operation(*args, **kwargs)
            self.available(deadline)
            return value
        except BaseException:
            self.refuse()
            raise Refused() from None

    def verify(self, deadline):
        try:
            self.available(deadline)
            self.copies.verify(deadline)
            self.available(deadline)
            self.artifacts.recheck()
            self.available(deadline)
        except BaseException:
            self.refuse()
            raise Refused() from None

    def text(self, child, deadline):
        try:
            return self._text(child, deadline)
        except BaseException:
            self.refuse()
            raise Refused() from None

    def _text(self, child, deadline):
        self.available(deadline)
        require(type(child) is self.ownership.OwnedProcess
                and any(child is held for held in self.owner.children))
        self.owner.live(child)
        self.available(deadline)
        rows = [row for row in self.owner.anchors.values() if row['child'] is child]
        require(len(rows) == 1)
        row = rows[0]
        self.available(deadline)
        fd = os.open('maps', FLAGS, dir_fd=row['proc_fd'])
        self.held.append(fd)  # Preserve even a late descriptor, never failure close.
        self.available(deadline)
        before = self.io(deadline, os.fstat, fd)
        require(stat.S_ISREG(before.st_mode))
        raw = b''
        while True:
            part = self.io(deadline, os.read, fd, min(65536, 1024*1024 + 1 - len(raw)))
            if not part:
                break
            raw += part
            require(len(raw) <= 1024*1024)
        after = self.io(deadline, os.fstat, fd)
        require((before.st_dev,before.st_ino,before.st_mode,before.st_uid,before.st_gid)
                == (after.st_dev,after.st_ino,after.st_mode,after.st_uid,after.st_gid))
        self.owner.live(child)
        self.available(deadline)
        return raw.decode('ascii','strict')

    def executable(self, child, name, deadline):
        """Follow only kernel exe through the original live child's proc FD."""
        try:
            self.available(deadline)
            require(type(child) is self.ownership.OwnedProcess and name in ROLES
                    and self.owner.anchors[name]['child'] is child
                    and any(child is held for held in self.owner.children))
            self.owner.live(child)
            self.available(deadline)
            fd = os.open('exe', EXE_FLAGS, dir_fd=self.owner.anchors[name]['proc_fd'])
            self.held.append(fd)
            self.available(deadline)
            value = self.io(deadline, os.fstat, fd)
            require(stat.S_ISREG(value.st_mode))
            if name in ('core','broker','host'):
                _, original, _ = self.artifacts.files[ROLES[name].rsplit('/',1)[1]]
                expected = tuple(getattr(original, key) for key in
                                 ('st_dev','st_ino','st_size','st_mode','st_uid','st_gid','st_nlink'))
            else:
                row = self.copies.records[ROLES[name]]
                expected = tuple(row[key] for key in ('device','inode','size','mode','uid','gid','nlink'))
            require(tuple(getattr(value, key) for key in
                          ('st_dev','st_ino','st_size','st_mode','st_uid','st_gid','st_nlink')) == expected)
            self.owner.live(child)
            self.available(deadline)
            return value.st_dev, value.st_ino
        except BaseException:
            self.refuse()
            raise Refused() from None

    def inventory(self, child, deadline, context=None):
        try:
            self.available(deadline)
            require(type(child) is self.ownership.OwnedProcess)
            names = [name for name,row in self.owner.anchors.items() if row['child'] is child]
            require(len(names) == 1 and names[0] in ROLES)
            name = names[0]
            require(context is None or context in ('initial_' + name, 'final_' + name))
            executable = self.executable(child, name, deadline)
            first = map_objects(self.text(child, deadline))
            self.available(deadline)
            require(first.get(ROLES[name]) == executable)
            if name in ('bus','resolved'):
                require('/usr/lib/libc.so.6' in first and '/usr/lib/ld-linux-x86-64.so.2' in first)
            # WHOLE batch membership/identity before any mapped-target open/hash.
            # These are retained admission records, not paths discovered/opened from data.
            for path, identity in first.items():
                if path in self.copies.records:
                    row = self.copies.records[path]
                    require(identity == (row['device'],row['inode']))
                elif path == ROLES[name] and name in ('core','broker','host'):
                    _, original, _ = self.artifacts.files[path.rsplit('/',1)[1]]
                    require(identity == (original.st_dev,original.st_ino))
                else:
                    raise Refused()
            self.available(deadline)
            result = []
            for path, identity in sorted(first.items()):
                if path in self.copies.records:
                    row = self.copies.records[path]
                    require(identity == (row['device'],row['inode']))
                    # Membership+copied identity BEFORE any target open/hash.
                    self.copies._verify_target(path, deadline)
                    self.available(deadline)
                    result.append({'path':path,'device':row['device'],'inode':row['inode'],
                                   'size':row['size'],'sha256':row['sha256']})
                elif path == ROLES[name] and name in ('core','broker','host'):
                    result.append(self.artifacts.mapped_identity(path.rsplit('/',1)[1],*identity))
                    self.available(deadline)
                else:
                    raise Refused()  # No unknown/other role's path opened or echoed.
            after = map_objects(self.text(child, deadline))
            self.available(deadline)
            require(first == after)
            require(self.executable(child, name, deadline) == executable)
            self.owner.live(child)
            self.available(deadline)
            return result
        except BaseException:
            self.refuse()
            raise Refused() from None
