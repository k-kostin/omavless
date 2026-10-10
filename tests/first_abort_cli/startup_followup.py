#!/usr/bin/python3
"""Fixed follow-up of reviewed 0b92f4a capture. No package/generator execution."""
import base64
import hashlib
import json
import re
import os
from pathlib import Path
import stat
import sys
import time

OUTPUT = Path('/run/ov-t4-user-startup-followup-v1')
ROOTS = tuple(Path(path) for path in (
    '/usr/lib/pam.d', '/etc/pam.conf', '/usr/lib/pam.conf',
    '/usr/bin/systemctl', '/usr/bin/dbus-broker-launch',
    '/usr/lib/systemd/systemd', '/usr/lib/systemd/systemd-user-runtime-dir',
    '/usr/lib/systemd/user-generators/systemd-xdg-autostart-generator',
    '/usr/lib/systemd/user-environment-generators/30-systemd-environment-d-generator',
    '/usr/share/man/man5/environment.d.5.gz',
    '/usr/share/man/man8/systemd-xdg-autostart-generator.8.gz',
    '/usr/share/man/man8/systemd-environment-d-generator.8.gz',
    '/usr/share/man/man5/systemd.unit.5.gz',
    '/usr/share/man/man8/pam_systemd.8.gz',
))
# Finite cross product, never traverse unrelated root-manager units.
SYSTEM_ROOTS = (
    '/etc/systemd/system.control', '/run/systemd/system.control',
    '/run/systemd/transient', '/run/systemd/generator.early',
    '/run/systemd/generator', '/run/systemd/generator.late',
    '/etc/systemd/system.attached', '/run/systemd/system.attached',
)
UNIT_NAMES = (
    'user@48044.service', 'user-runtime-dir@48044.service',
    'user@.service', 'user-runtime-dir@.service',
)
DROPINS = tuple(name + '.d' for name in UNIT_NAMES) + (
    'service.d', 'user-.service.d', 'user-runtime-.service.d',
)
ROOTS += tuple(Path(root) / name for root in SYSTEM_ROOTS for name in (*UNIT_NAMES, *DROPINS))
ROOTS += tuple(Path(root) / name for root in (
    '/etc/systemd/system', '/run/systemd/system',
    '/usr/local/lib/systemd/system', '/usr/lib/systemd/system',
) for name in ('user-.service.d', 'user-runtime-.service.d'))
PACKAGES = ('systemd', 'systemd-libs', 'pam', 'pambase', 'dbus-broker', 'dbus-broker-units')
PACKAGE_ROOT = Path('/var/lib/pacman/local')
EXECUTABLES = (
    '/usr/bin/systemctl', '/usr/bin/dbus-broker-launch',
    '/usr/lib/systemd/systemd', '/usr/lib/systemd/systemd-user-runtime-dir',
    '/usr/lib/systemd/user-generators/systemd-xdg-autostart-generator',
    '/usr/lib/systemd/user-environment-generators/30-systemd-environment-d-generator',
)
# Follow only explicit source links into these package/config namespaces, never
# recursively inventory an entire target prefix just because one link used it.
TARGETS = tuple(Path(path) for path in (
    '/etc/systemd', '/run/systemd', '/usr/local/lib/systemd', '/usr/lib/systemd',
    '/etc/xdg/systemd', '/usr/share/systemd', '/usr/local/share/systemd',
    '/etc/environment', '/etc/environment.d', '/run/environment.d',
    '/usr/local/lib/environment.d', '/usr/lib/environment.d', '/usr/bin', '/dev/null',
    '/usr/lib/pam.d', '/usr/share/man',
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
        if hasattr(self, 'package_names'):
            require(self.children(self.directories[PACKAGE_ROOT][0]) == self.package_names)
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


    def package_inputs(self):
        """Select fixed package names, not arbitrary discovered source paths."""
        self.available()
        try:
            parent = self.parent(PACKAGE_ROOT / 'placeholder')
            require(parent is not None)
            names = self.children(parent)
            selected = {}
            for package in PACKAGES:
                candidates = [name for name in names if re.fullmatch(
                    re.escape(package) + r'-[0-9][A-Za-z0-9_.+:-]*', name)]
                # systemd and systemd-libs cannot alias: version starts numeric.
                require(len(candidates) == 1)
                directory = PACKAGE_ROOT / candidates[0]
                self.visit(directory / 'desc')
                self.visit(directory / 'files')
                row = self.records[directory / 'desc']
                require(row['type'] == 'file')
                text = base64.b64decode(row['content_b64']).decode('utf-8')
                sections = {}
                key = None
                for line in text.splitlines():
                    if line.startswith('%') and line.endswith('%'):
                        require(line not in sections)
                        key = line
                        sections[key] = []
                    elif line:
                        require(key is not None)
                        sections[key].append(line)
                require(sections.get('%NAME%') == [package]
                        and len(sections.get('%VERSION%', [])) == 1)
                require(candidates[0] == package + '-' + sections['%VERSION%'][0])
                selected[package] = str(directory)
            self.package_selection = selected
            self.package_names = names
        except BaseException:
            self.sealed = True
            raise

    def package_bindings(self):
        self.available()
        try:
            lists = {}
            for package, directory in self.package_selection.items():
                row = self.records[Path(directory) / 'files']
                require(row['type'] == 'file')
                text = base64.b64decode(row['content_b64']).decode('utf-8')
                lines = text.splitlines()
                require(lines.count('%FILES%') == 1)
                start = lines.index('%FILES%') + 1
                entries = []
                for line in lines[start:]:
                    if not line or line.startswith('%'):
                        break
                    require(len(line.encode()) <= 4096 and not line.startswith('/')
                            and '..' not in Path(line).parts)
                    entries.append(line)
                require(len(entries) <= 16384 and len(entries) == len(set(entries)))
                lists[package] = set(entries)
            bindings = {}
            for executable in EXECUTABLES:
                row = self.records[Path(executable)]
                require(row['type'] == 'file' and row['identity'][2] & 0o111 != 0
                        and base64.b64decode(row['content_b64']).startswith(b'\x7fELF'))
                owners = [name for name, entries in lists.items() if executable[1:] in entries]
                require(len(owners) == 1)
                bindings[executable] = owners[0]
            return bindings
        except BaseException:
            self.sealed = True
            raise

    def capture(self):
        self.available()
        try:
            for root in self.roots:
                self.visit(root)
            self.recheck()
            return {'schema': 't4-startup-followup-inventory-v1', 'semantic_admission': False,
                    'roots': [str(path) for path in self.roots], 'bytes': self.total,
                    'package_selection': getattr(self, 'package_selection', {}),
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
            and __file__ == '<stdin>' and sys.argv == ['-', '--capture-t4-user-startup-followup-v1'])
    run = os.open('/run', os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC)
    before = os.fstat(run)
    require(before.st_uid == before.st_gid == 0 and before.st_mode & 0o022 == 0)
    os.mkdir(OUTPUT.name, mode=0o700, dir_fd=run)
    out = os.open(OUTPUT.name, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC, dir_fd=run)
    original = os.fstat(out)
    require(original.st_uid == original.st_gid == 0 and stat.S_IMODE(original.st_mode) == 0o700)
    exclusive(out, 'invocation.json', b'{"schema":1,"read_only_sources":true,"semantic_admission":false}\n')
    inventory = Inventory(ROOTS, TARGETS)
    inventory.package_inputs()
    value = inventory.capture()
    value['package_bindings'] = inventory.package_bindings()
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
    print('T4_STARTUP_FOLLOWUP_CAPTURE_ONLY_NOT_ADMISSION ' + digest)


if __name__ == '__main__':
    os.umask(0o077)
    try:
        main()
    except BaseException:
        print('T4_STARTUP_FOLLOWUP_CAPTURE_NONPASS_RETAINED', file=sys.stderr)
        sys.exit(2)
