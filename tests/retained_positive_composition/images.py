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
# Linux show_vma_header_prefix emits ONE trailing ASCII separator even for an
# unnamed VMA. The absent-path alternative permits precisely that separator;
# never strip row/path bytes or accept arbitrary trailing whitespace.
MAP_LINE = re.compile(r'([0-9a-f]{1,16})-([0-9a-f]{1,16}) ([r-][w-][x-][ps]) ([0-9a-f]{1,16}) ([0-9a-f]{1,8}):([0-9a-f]{1,8}) ([0-9]{1,20})(?:[ \t]+([^\r\n]+)| )?')
ROLES = {'bus':'/usr/bin/dbus-daemon','resolved':'/usr/lib/systemd/systemd-resolved',
         'core':'/artifacts/mihomo','broker':'/artifacts/omavless-dns-broker','host':'/artifacts/host-fixture'}
INVENTORY_ROLES = ('bus','host')
# Go's Linux runtime places a five-byte " Go: " prefix in a 79-byte
# NUL-terminated VMA name. Only zero-identity anonymous rows may use this
# bounded annotation; it is discarded, never an eligible file/object path.
ANONYMOUS = re.compile(r'(?:\[[A-Za-z0-9_:.-]+\]|\[anon: Go: [A-Za-z][A-Za-z0-9 _.:-]{0,73}\])\Z')
INVENTORY_STEPS = ('executable','first_text','first_parse','required_members',
    'whole_membership','targets','second_text','second_parse','maps_equal',
    'final_executable','final_live')
ANONYMOUS_CLASSES = ('unnamed','plain_bracket','go','glibc_malloc',
    'glibc_malloc_arena','glibc_loader_malloc','foreign_bracket','invalid_zero_identity')
REQUIRED_CLASSES = ('present','absent','identity_equal','identity_different')

def anonymous_class(path, identity, offset):
    # Fixed category only, BEFORE the unchanged predicate. No raw row/value
    # escapes; a category is neither acceptance nor a diagnosed cause.
    if identity != (0, 0) or offset != 0:
        return 'invalid_zero_identity'
    if not path:
        return 'unnamed'
    if re.fullmatch(r'\[[A-Za-z0-9_:.-]+\]', path):
        return 'plain_bracket'
    if path.startswith('[anon: Go: '):
        return 'go'
    return {'[anon: glibc: malloc]':'glibc_malloc',
            '[anon: glibc: malloc arena]':'glibc_malloc_arena',
            '[anon: glibc: loader malloc]':'glibc_loader_malloc'}.get(path,'foreign_bracket')

class Refused(RuntimeError):
    def __init__(self):
        super().__init__('fixed_loaded_image_refused')

def require(value, reason=None):
    if not value:
        raise Refused()

def map_objects(text, before_anonymous=None):
    require(type(text) is str and len(text) <= 1024 * 1024, "mapping_bound")
    objects = {}
    previous_end = 0
    seen = set()
    # The proc producer uses LF, not Python's broader line-separator grammar.
    # Preserve CR/VT and every path byte for the predicates below; discard only
    # one optional terminal LF, never embedded or repeated empty rows.
    lines = text.split('\n')
    if lines[-1] == '':
        lines.pop()
    for line in lines:
        match = MAP_LINE.fullmatch(line)
        require(match is not None, "mapping_shape")
        start, end, _, offset, major, minor, inode, path = match.groups()
        start, end, offset = int(start, 16), int(end, 16), int(offset, 16)
        require(previous_end <= start < end <= 2**64 - 1 and offset <= 2**64 - 1,
                "mapping_range")
        previous_end = end
        identity = os.makedev(int(major, 16), int(minor, 16)), int(inode)
        if path is None or path == "" or path.startswith("["):
            if before_anonymous is not None:
                category = anonymous_class(path, identity, offset)
                require(category in ANONYMOUS_CLASSES)
                if category not in seen:
                    seen.add(category)  # At most eight labels per whole parse.
                    before_anonymous(category)
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
        self.executables = {}
        self.initial_bus_observed = False
        self.initial_host_observed = False
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
            self.artifacts.recheck(deadline)
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
        """Retain one original per role; reread current kernel exe on EVERY call.

        This bounds descriptor retention in fragmented helper/stream loops. A
        cached FD alone is never current-image evidence: both its original
        metadata and the current kernel link must still match admission.
        """
        try:
            self.available(deadline)
            require(type(child) is self.ownership.OwnedProcess and name in ROLES
                    and self.owner.anchors[name]['child'] is child
                    and any(child is held for held in self.owner.children))
            self.owner.live(child)
            self.available(deadline)
            proc_fd = self.owner.anchors[name]['proc_fd']
            if name not in self.executables:
                fd = os.open('exe', EXE_FLAGS, dir_fd=proc_fd)
                self.held.append(fd)  # Retain before even the late-return gate.
                self.executables[name] = (child, proc_fd, fd)
                self.available(deadline)
            cached_child, cached_proc, fd = self.executables[name]
            require(cached_child is child and type(cached_proc) is int
                    and type(proc_fd) is int and cached_proc == proc_fd
                    and type(fd) is int and fd >= 0 and fd in self.held)
            value = self.io(deadline, os.fstat, fd)
            if name in ('core','broker','host'):
                _, original, _ = self.artifacts.files[ROLES[name].rsplit('/',1)[1]]
                expected = tuple(getattr(original, key) for key in
                                 ('st_dev','st_ino','st_size','st_mode','st_uid','st_gid','st_nlink'))
            else:
                row = self.copies.records[ROLES[name]]
                expected = tuple(row[key] for key in ('device','inode','size','mode','uid','gid','nlink'))
            keys = ('st_dev','st_ino','st_size','st_mode','st_uid','st_gid','st_nlink')
            require(all(type(item) is int for item in expected))
            require(stat.S_ISREG(value.st_mode)
                    and all(type(getattr(value, key)) is int for key in keys)
                    and tuple(getattr(value, key) for key in keys) == expected)
            current = self.io(deadline, os.stat, 'exe', dir_fd=proc_fd, follow_symlinks=True)
            require(all(type(getattr(current, key)) is int for key in keys)
                    and tuple(getattr(current, key) for key in keys) == expected)
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
            observed = name in INVENTORY_ROLES and context == 'initial_' + name
            if observed:
                if name == 'bus':
                    require(self.initial_bus_observed is False)
                    self.initial_bus_observed = True
                else:
                    require(self.initial_host_observed is False)
                    self.initial_host_observed = True
                # Separate fixed role latches; neither diagnostic can replay.
            def mark(side, step):
                if observed:
                    self.available(deadline)
                    require(side in ('before','after') and step in INVENTORY_STEPS)
                    self.owner.phase(side+'_'+name+'_initial_inventory_'+step, deadline)
                    self.available(deadline)
            def parsed(raw, step):
                def category(label):
                    self.available(deadline)
                    require(label in ANONYMOUS_CLASSES and step in ('first_parse','second_parse'))
                    self.owner.phase('before_'+name+'_initial_inventory_'+step+'_'+label, deadline)
                    self.available(deadline)
                return map_objects(raw, category if observed else None)
            mark('before','executable')
            executable = self.executable(child, name, deadline)
            mark('after','executable')
            mark('before','first_text')
            raw = self.text(child, deadline)
            mark('after','first_text')
            mark('before','first_parse')
            first = parsed(raw, 'first_parse')
            mark('after','first_parse')
            self.available(deadline)
            mark('before','required_members')
            if observed and name == 'host':
                self.available(deadline)
                # Exactly two fixed Boolean observations, on this one attempt.
                # Neither category authorizes a later effect or explains absence.
                presence = 'present' if ROLES[name] in first else 'absent'
                equality = 'identity_equal' if first.get(ROLES[name]) == executable else 'identity_different'
                for label in (presence, equality):
                    self.available(deadline)
                    require(label in REQUIRED_CLASSES)
                    self.owner.phase('before_host_initial_inventory_required_members_'+label, deadline)
                    self.available(deadline)
            require(first.get(ROLES[name]) == executable)
            if name in ('bus','resolved'):
                require('/usr/lib/libc.so.6' in first and '/usr/lib/ld-linux-x86-64.so.2' in first)
            mark('after','required_members')
            # WHOLE batch membership/identity before any mapped-target open/hash.
            # These are retained admission records, not paths discovered/opened from data.
            mark('before','whole_membership')
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
            mark('after','whole_membership')
            result = []
            mark('before','targets')
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
                    result.append(self.artifacts.mapped_identity(path.rsplit('/',1)[1],*identity,deadline))
                    self.available(deadline)
                else:
                    raise Refused()  # No unknown/other role's path opened or echoed.
            mark('after','targets')
            mark('before','second_text')
            raw = self.text(child, deadline)
            mark('after','second_text')
            mark('before','second_parse')
            after = parsed(raw, 'second_parse')
            mark('after','second_parse')
            self.available(deadline)
            mark('before','maps_equal')
            require(first == after)
            mark('after','maps_equal')
            mark('before','final_executable')
            require(self.executable(child, name, deadline) == executable)
            mark('after','final_executable')
            mark('before','final_live')
            self.owner.live(child)
            self.available(deadline)
            mark('after','final_live')
            return result
        except BaseException:
            self.refuse()
            raise Refused() from None
