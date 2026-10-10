#!/usr/bin/python3
"""Trusted-stdin, capture-only startup source inventory. Never execute a source."""
import base64
import hashlib
import json
import os
from pathlib import Path
import stat
import sys
import time

OUTPUT = Path('/run/ov-t4-user-startup-inventory-v1')
ROOTS = tuple(Path(path) for path in (
    '/etc/systemd/user', '/etc/xdg/systemd/user', '/run/systemd/user',
    '/usr/local/lib/systemd/user', '/usr/local/share/systemd/user',
    '/usr/lib/systemd/user', '/usr/share/systemd/user',
    '/etc/systemd/user-generators', '/run/systemd/user-generators',
    '/usr/local/lib/systemd/user-generators', '/usr/lib/systemd/user-generators',
    '/etc/systemd/user-environment-generators', '/run/systemd/user-environment-generators',
    '/usr/local/lib/systemd/user-environment-generators', '/usr/lib/systemd/user-environment-generators',
    '/etc/environment', '/etc/environment.d', '/run/environment.d',
    '/usr/local/lib/environment.d', '/usr/lib/environment.d',
    '/etc/xdg/autostart', '/usr/local/share/xdg/autostart', '/usr/share/xdg/autostart',
    '/etc/systemd/system/user@.service', '/run/systemd/system/user@.service',
    '/usr/local/lib/systemd/system/user@.service', '/usr/lib/systemd/system/user@.service',
    '/etc/systemd/system/user-runtime-dir@.service', '/run/systemd/system/user-runtime-dir@.service',
    '/usr/local/lib/systemd/system/user-runtime-dir@.service', '/usr/lib/systemd/system/user-runtime-dir@.service',
    '/etc/systemd/system/user@.service.d', '/run/systemd/system/user@.service.d',
    '/usr/local/lib/systemd/system/user@.service.d', '/usr/lib/systemd/system/user@.service.d',
    '/etc/systemd/system/user-runtime-dir@.service.d', '/run/systemd/system/user-runtime-dir@.service.d',
    '/usr/local/lib/systemd/system/user-runtime-dir@.service.d', '/usr/lib/systemd/system/user-runtime-dir@.service.d',
    '/etc/systemd/system/user@48044.service', '/run/systemd/system/user@48044.service',
    '/usr/local/lib/systemd/system/user@48044.service', '/usr/lib/systemd/system/user@48044.service',
    '/etc/systemd/system/user-runtime-dir@48044.service', '/run/systemd/system/user-runtime-dir@48044.service',
    '/usr/local/lib/systemd/system/user-runtime-dir@48044.service', '/usr/lib/systemd/system/user-runtime-dir@48044.service',
    '/etc/systemd/system/user@48044.service.d', '/run/systemd/system/user@48044.service.d',
    '/usr/local/lib/systemd/system/user@48044.service.d', '/usr/lib/systemd/system/user@48044.service.d',
    '/etc/systemd/system/user-runtime-dir@48044.service.d', '/run/systemd/system/user-runtime-dir@48044.service.d',
    '/usr/local/lib/systemd/system/user-runtime-dir@48044.service.d', '/usr/lib/systemd/system/user-runtime-dir@48044.service.d',
    '/etc/systemd/system/service.d', '/run/systemd/system/service.d', '/usr/lib/systemd/system/service.d',
    '/usr/local/lib/systemd/system/service.d',
    '/etc/systemd/user.conf', '/etc/systemd/user.conf.d', '/run/systemd/user.conf.d',
    '/usr/local/lib/systemd/user.conf.d', '/usr/lib/systemd/user.conf.d',
    '/etc/pam.d', '/etc/security/pam_env.conf', '/etc/security/limits.conf', '/etc/security/limits.d',
    '/usr/lib/systemd/systemd', '/usr/lib/systemd/systemd-user-runtime-dir',
))
# Follow only explicit source links into these package/config namespaces, never
# recursively inventory an entire target prefix just because one link used it.
TARGETS = tuple(Path(path) for path in (
    '/etc/systemd', '/run/systemd', '/usr/local/lib/systemd', '/usr/lib/systemd',
    '/etc/xdg/systemd', '/usr/share/systemd', '/usr/local/share/systemd',
    '/etc/environment', '/etc/environment.d', '/run/environment.d',
    '/usr/local/lib/environment.d', '/usr/lib/environment.d', '/usr/bin', '/dev/null',
))
MAX_FILES = 4096
MAX_FILE = 8 * 1024 * 1024
MAX_TOTAL = 32 * 1024 * 1024
MAX_OUTPUT = 48 * 1024 * 1024


class Refused(Exception):
    pass


def require(value):
    if not value:
        raise Refused()


def identity(m):
    return (m.st_dev, m.st_ino, m.st_mode, m.st_uid, m.st_gid, m.st_nlink,
            m.st_size, m.st_mtime_ns, m.st_ctime_ns)


def directory_identity(m):
    return (m.st_dev, m.st_ino, m.st_mode, m.st_uid, m.st_gid)


def normalized(path):
    raw = os.fspath(path)
    require(path.is_absolute() and len(os.fsencode(raw)) <= 4096
            and all(ord(c) >= 32 and ord(c) != 127 and not 0xd800 <= ord(c) <= 0xdfff for c in raw))
    require(raw == os.path.normpath(raw))


class Inventory:
    def __init__(self, roots, targets, uid=0):
        self.roots, self.targets, self.uid = tuple(roots), tuple(targets), uid
        self.deadline = time.monotonic() + 45
        self.directories, self.nodes, self.absent, self.records = {}, {}, {}, {}
        self.total, self.sealed = 0, False

    def available(self):
        if self.sealed or time.monotonic() >= self.deadline:
            self.sealed = True
            raise Refused()

    def parent(self, path):
        self.available()
        normalized(path)
        for directory in reversed(path.parent.parents):
            if directory not in self.directories:
                if not self.add_directory(directory):
                    return None
        if path.parent not in self.directories and not self.add_directory(path.parent):
            return None
        return self.directories[path.parent][0]

    def add_directory(self, path):
        self.available()
        if path in self.directories:
            fd, before = self.directories[path]
            require(directory_identity(before) == directory_identity(os.fstat(fd)))
            return True
        if path == Path('/'):
            parent, name = None, '/'
        else:
            parent = self.directories[path.parent][0]
            name = path.name
        try:
            before = os.stat(name, dir_fd=parent, follow_symlinks=False)
        except FileNotFoundError:
            self.absent[path] = (parent, name)
            return False
        require(stat.S_ISDIR(before.st_mode) and before.st_uid in (0, self.uid)
                and before.st_mode & 0o022 == 0)
        fd = os.open(name, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC, dir_fd=parent)
        self.directories[path] = (fd, before)
        require(directory_identity(before) == directory_identity(os.fstat(fd)))
        return True

    def children(self, fd):
        names = []
        with os.scandir(fd) as stream:
            for entry in stream:
                self.available()
                require(len(names) < MAX_FILES)
                names.append(entry.name)
        require(len(names) == len(set(names)))
        return sorted(names)

    def attributes(self, fd):
        names = sorted(os.listxattr(fd))
        require(len(names) <= 32)
        result = {}
        for name in names:
            value = os.getxattr(fd, name)
            require(len(name) <= 256 and len(value) <= 4096)
            result[name] = base64.b64encode(value).decode('ascii')
        return result

    def read(self, fd, before):
        self.available()
        require(before.st_size <= MAX_FILE)
        data, offset = [], 0
        while offset < before.st_size:
            self.available()
            block = os.pread(fd, min(65536, before.st_size - offset), offset)
            require(block)
            data.append(block)
            offset += len(block)
        require(os.pread(fd, 1, offset) == b'' and identity(before) == identity(os.fstat(fd)))
        return b''.join(data)

    def visit(self, path, depth=0, chain=()):
        self.available()
        normalized(path)
        require(depth <= 32 and path not in chain)
        if path in self.records:
            return
        require(len(self.records) < MAX_FILES)
        parent = self.parent(path)
        if parent is None:
            self.records[path] = {'type': 'absent-ancestor'}
            return
        try:
            before = os.stat(path.name, dir_fd=parent, follow_symlinks=False)
        except FileNotFoundError:
            self.absent[path] = (parent, path.name)
            self.records[path] = {'type': 'absent'}
            return
        require(before.st_uid == self.uid)
        record = {'identity': identity(before)}
        self.records[path] = record
        if stat.S_ISLNK(before.st_mode):
            target = os.readlink(path.name, dir_fd=parent)
            require(len(os.fsencode(target)) <= 4096)
            resolved = Path(os.path.normpath(target if target.startswith('/') else str(path.parent / target)))
            normalized(resolved)
            require(any(resolved == root or resolved.is_relative_to(root) for root in self.targets))
            record.update(type='link', target=target, resolved=str(resolved))
            self.nodes[path] = (parent, before, None, None)
            self.visit(resolved, depth + 1, (*chain, path))
        elif stat.S_ISDIR(before.st_mode):
            require(before.st_mode & 0o022 == 0)
            require(self.add_directory(path))
            fd = self.directories[path][0]
            children = self.children(fd)
            record.update(type='directory', children=children, xattrs=self.attributes(fd))
            self.nodes[path] = (parent, before, fd, children)
            for name in children:
                self.visit(path / name, depth + 1, chain)
        elif stat.S_ISREG(before.st_mode):
            require(before.st_mode & 0o022 == 0 and before.st_nlink == 1 and before.st_size <= MAX_FILE)
            self.total += before.st_size
            require(self.total <= MAX_TOTAL)
            fd = os.open(path.name, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC, dir_fd=parent)
            self.nodes[path] = (parent, before, fd, None)
            require(identity(before) == identity(os.fstat(fd)))
            data = self.read(fd, before)
            record.update(type='file', sha256=hashlib.sha256(data).hexdigest(),
                          content_b64=base64.b64encode(data).decode('ascii'), xattrs=self.attributes(fd))
            self.nodes[path] = (parent, before, fd, None)
        elif path == Path('/dev/null') and stat.S_ISCHR(before.st_mode):
            require(os.major(before.st_rdev) == 1 and os.minor(before.st_rdev) == 3)
            record.update(type='null-mask')
            self.nodes[path] = (parent, before, None, None)
        else:
            raise Refused()
        require(identity(before) == identity(os.stat(path.name, dir_fd=parent, follow_symlinks=False)))

    def recheck(self):
        self.available()
        try:
            self._recheck()
        except BaseException:
            self.sealed = True
            raise

    def _recheck(self):
        for path, (fd, before) in self.directories.items():
            self.available()
            require(directory_identity(before) == directory_identity(os.fstat(fd))
                    == directory_identity(path.lstat()))
        for parent, name in self.absent.values():
            self.available()
            try:
                os.stat(name, dir_fd=parent, follow_symlinks=False)
            except FileNotFoundError:
                pass
            else:
                raise Refused()
        for path, (parent, before, fd, children) in self.nodes.items():
            self.available()
            require(identity(before) == identity(os.stat(path.name, dir_fd=parent, follow_symlinks=False)))
            record = self.records[path]
            if fd is not None:
                require(identity(before) == identity(os.fstat(fd)) and self.attributes(fd) == record['xattrs'])
                if children is None:
                    require(hashlib.sha256(self.read(fd, before)).hexdigest() == record['sha256'])
                else:
                    require(self.children(fd) == children)
            elif record['type'] == 'link':
                require(os.readlink(path.name, dir_fd=parent) == record['target'])

    def capture(self):
        self.available()
        try:
            for root in self.roots:
                self.visit(root)
            self.recheck()
            return {'schema': 't4-startup-source-inventory-v1', 'semantic_admission': False,
                    'roots': [str(path) for path in self.roots], 'bytes': self.total,
                    'records': {str(path): record for path, record in sorted(self.records.items())}}
        except BaseException:
            self.sealed = True
            raise


def exclusive(directory, name, data):
    require(len(data) <= MAX_OUTPUT)
    fd = os.open(name, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW | os.O_CLOEXEC,
                 0o600, dir_fd=directory)
    with os.fdopen(fd, 'wb') as stream:
        stream.write(data)
        stream.flush()
        os.fsync(stream.fileno())
    os.fsync(directory)


def main():
    require(os.getresuid() == os.getresgid() == (0, 0, 0)
            and sys.flags.isolated == 1 and sys.dont_write_bytecode
            and __file__ == '<stdin>' and sys.argv == ['-', '--capture-t4-user-startup-v1'])
    run = os.open('/run', os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC)
    before = os.fstat(run)
    require(before.st_uid == before.st_gid == 0 and before.st_mode & 0o022 == 0)
    os.mkdir(OUTPUT.name, mode=0o700, dir_fd=run)
    out = os.open(OUTPUT.name, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC, dir_fd=run)
    original = os.fstat(out)
    require(original.st_uid == original.st_gid == 0 and stat.S_IMODE(original.st_mode) == 0o700)
    exclusive(out, 'invocation.json', b'{"schema":1,"read_only_sources":true,"semantic_admission":false}\n')
    inventory = Inventory(ROOTS, TARGETS)
    value = inventory.capture()
    data = json.dumps(value, sort_keys=True, separators=(',', ':')).encode()
    require(len(data) <= MAX_OUTPUT)
    inventory.recheck()
    exclusive(out, 'inventory.json', data)
    inventory.recheck()
    require(directory_identity(before) == directory_identity(os.fstat(run)) == directory_identity(Path('/run').lstat())
            and directory_identity(original) == directory_identity(os.fstat(out)) == directory_identity(OUTPUT.lstat()))
    digest = hashlib.sha256(data).hexdigest()
    exclusive(out, 'result.json', json.dumps({'schema':1, 'inventory_sha256':digest,
              'records':len(value['records']), 'semantic_admission':False}, sort_keys=True).encode())
    print('T4_STARTUP_CAPTURE_ONLY_NOT_ADMISSION ' + digest)


if __name__ == '__main__':
    os.umask(0o077)
    try:
        main()
    except BaseException:
        print('T4_STARTUP_CAPTURE_NONPASS_RETAINED', file=sys.stderr)
        sys.exit(2)
