"""Private fixture lineage; no entry point, process API or production caller."""
import hashlib
import os
from pathlib import Path
import stat
import time

UID = 48049
HOME = Path('/home/ov-t4-abort-v6')
CONFIG = HOME / '.config/omavless'
STATE = HOME / '.local/state/omavless'
RUNTIME = Path('/run/user/48049/omavless')
ARTIFACTS = HOME / '.t4-first-abort'
STAGE = STATE / 'restore-pair.pending'
LIMIT = 64 * 1024 * 1024
MAX_TOTAL = 128 * 1024 * 1024
GATE = lambda: None  # The fixed outer guard supplies its permanent/deadline gate.
FLAGS = os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC


class Refused(Exception):
    pass


def require(value):
    if not value:
        raise Refused()


def identity(s):
    return (s.st_dev, s.st_ino, s.st_mode, s.st_uid, s.st_gid,
            s.st_nlink, s.st_size, s.st_mtime_ns, s.st_ctime_ns)


def directory_identity(s):
    return (s.st_dev, s.st_ino, s.st_mode, s.st_uid, s.st_gid)


class Lineage:
    """Continuous original immutable pins plus ONE explicit live/terminal change.

    The outer guard must establish exact known-zero normal CLI completion AND
    native typed verification before calling first_completed(). This object
    does not mint child-completion authority. The replaced predecessor FDs stay
    held; after the legitimate lower rename they are not relabeled current.
    """
    def __init__(self):
        self.phase = 'admitting'
        self.directories, self.files, self.predecessors = {}, {}, {}
        self.held = []
        self.total = 0
        self.deadline = time.monotonic() + 600
        try:
            for directory in (CONFIG, STATE, RUNTIME, ARTIFACTS, STAGE):
                self.directory(directory)
            immutable = [ARTIFACTS / name for name in ('archive.ovb', 'request.json', 'setup.json')]
            immutable += [STATE / name for name in ('ownership.json', 'desired.json', 'restore-decision.intent')]
            immutable += [RUNTIME / 'owner.lock']
            immutable += [STAGE / name for name in ('old-profiles.json', 'old-route-template.yaml',
                          'new-profiles.json', 'new-route-template.yaml', 'ready.bin')]
            for path in immutable:
                self.files[path] = self.pin(path)
            for name in ('profiles.json', 'route-template.yaml'):
                path = CONFIG / name
                self.predecessors[path] = self.pin(path)
            require(self.predecessors[CONFIG / 'profiles.json'][2] == self.files[STAGE / 'new-profiles.json'][2]
                    and self.predecessors[CONFIG / 'route-template.yaml'][2] == self.files[STAGE / 'old-route-template.yaml'][2])
            self.absent_terminal()
            self.phase = 'mixed'
            self.before_first()
        except BaseException:
            self.phase = 'refused'
            raise

    def directory(self, path):
        if path in self.directories:
            return self.directories[path][0]
        if path == Path('/'):
            parent, name = None, '/'
        else:
            parent, name = self.directory(path.parent), path.name
        before = os.stat(name, dir_fd=parent, follow_symlinks=False)
        require(stat.S_ISDIR(before.st_mode) and before.st_uid in (0, UID)
                and before.st_mode & 0o6022 == 0)
        fd = os.open(name, FLAGS | os.O_DIRECTORY, dir_fd=parent)
        self.held.append(fd)
        self.directories[path] = (fd, before)
        require(directory_identity(before) == directory_identity(os.fstat(fd)))
        try:
            os.stat('.git', dir_fd=fd, follow_symlinks=False)
        except FileNotFoundError:
            pass
        else:
            raise Refused()
        if path in (CONFIG, STATE, RUNTIME, ARTIFACTS, STAGE):
            require(before.st_uid == before.st_gid == UID and stat.S_IMODE(before.st_mode) == 0o700)
        return fd

    def data(self, fd, before):
        GATE()
        require(time.monotonic() < self.deadline)
        require(before.st_size <= LIMIT)
        value, offset = bytearray(), 0
        while offset < before.st_size:
            GATE()
            require(time.monotonic() < self.deadline)
            chunk = os.pread(fd, min(65536, before.st_size - offset), offset)
            require(chunk)
            value.extend(chunk)
            offset += len(chunk)
        require(os.pread(fd, 1, offset) == b'')
        return bytes(value)

    def pin(self, path):
        parent = self.directory(path.parent)
        fd = os.open(path.name, FLAGS, dir_fd=parent)
        self.held.append(fd)
        before = os.fstat(fd)
        require(stat.S_ISREG(before.st_mode) and before.st_uid == before.st_gid == UID
                and stat.S_IMODE(before.st_mode) == 0o600 and before.st_nlink == 1
                and not os.listxattr(fd))
        self.total += before.st_size
        require(self.total <= MAX_TOTAL)
        value = self.data(fd, before)
        require(identity(before) == identity(os.fstat(fd))
                == identity(os.stat(path.name, dir_fd=parent, follow_symlinks=False)))
        return fd, before, value

    def check(self, path, pin):
        fd, before, value = pin
        require(identity(before) == identity(os.fstat(fd))
                == identity(os.stat(path.name, dir_fd=self.directories[path.parent][0], follow_symlinks=False))
                and not os.listxattr(fd) and self.data(fd, before) == value)
        require(identity(before) == identity(os.fstat(fd)))

    def originals(self):
        GATE()
        require(time.monotonic() < self.deadline)
        for path, (fd, before) in self.directories.items():
            named = os.stat('/', follow_symlinks=False) if path == Path('/') else os.stat(
                path.name, dir_fd=self.directories[path.parent][0], follow_symlinks=False)
            require(directory_identity(before) == directory_identity(os.fstat(fd)) == directory_identity(named))
            if path == STAGE:
                require(identity(before) == identity(os.fstat(fd)) == identity(named))
        for path, pin in self.files.items():
            self.check(path, pin)
        directory = self.directories[STAGE][0]
        os.lseek(directory, 0, os.SEEK_SET)
        names = []
        with os.scandir(directory) as stream:
            for entry in stream:
                require(len(names) < 5)
                names.append(entry.name)
        require(sorted(names) == sorted(
            ('old-profiles.json', 'old-route-template.yaml', 'new-profiles.json', 'new-route-template.yaml', 'ready.bin')))

    def absent_terminal(self):
        try:
            os.stat('restore-decision.terminal', dir_fd=self.directories[STATE][0], follow_symlinks=False)
        except FileNotFoundError:
            return
        raise Refused()

    def enter(self, expected):
        previous, self.phase = self.phase, 'refused'
        require(previous == expected)

    def before_first(self):
        self.enter('mixed')
        self.originals()
        for path, pin in self.predecessors.items():
            self.check(path, pin)
        self.absent_terminal()
        self.phase = 'mixed'

    def first_completed(self):
        self.enter('mixed')
        self.originals()
        for name, old in (('profiles.json', 'old-profiles.json'), ('route-template.yaml', 'old-route-template.yaml')):
            path = CONFIG / name
            current = self.pin(path)
            require(current[2] == self.files[STAGE / old][2])
            self.files[path] = current
        terminal = STATE / 'restore-decision.terminal'
        self.files[terminal] = self.pin(terminal)
        require(self.files[terminal][2])
        self.originals()
        self.phase = 'aborted'

    def reentry_boundary(self):
        self.enter('aborted')
        self.originals()
        self.phase = 'aborted'

    def receipt(self):
        self.reentry_boundary()
        return {str(path.relative_to(HOME)) if path.is_relative_to(HOME) else 'runtime/owner.lock':
                {'identity': identity(pin[1]), 'sha256': hashlib.sha256(pin[2]).hexdigest()}
                for path, pin in self.files.items()}

    def mixed_receipt(self):
        self.before_first()
        return {'phase': 'mixed-first-intent', 'originals': {
            str(path.relative_to(HOME)) if path.is_relative_to(HOME) else 'runtime/owner.lock':
            {'identity': identity(pin[1]), 'sha256': hashlib.sha256(pin[2]).hexdigest()}
            for path, pin in (*self.files.items(), *self.predecessors.items())}}
