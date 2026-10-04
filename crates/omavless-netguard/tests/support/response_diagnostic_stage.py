#!/usr/bin/python3
"""Trusted-delivery, create-only root staging for one fixed metadata capture."""
import hashlib
import os
from pathlib import Path
import pwd
import stat
import sys

SOURCE = Path('/home/kdk_vm/.cache/k1-response-diagnostic-stage-v1')
DESTINATION = Path('/run/omavless-k1-admission-response-diagnostic')
MEMBERS = {
    'probe': ('0000000000000000000000000000000000000000000000000000000000000000', 0o500, 0o500, 128 * 1024 * 1024),
    'query-guard.py': ('660f55e20b2c4e3b3bb1b4f9616b6176c739db7798bc6013bd936f032843665e', 0o400, 0o600, 256 * 1024),
    'fixture.service': ('a3b03103bbd8c43f6e6ca6755c063a7e851f40a006303c93028be80aec411e58', 0o400, 0o600, 16384),
    'guard.py': ('9fac76d08df39d20bd85c27807157214f5af08ccf34aeb80b4b04b66a15c0521', 0o400, 0o500, 256 * 1024),
}


def require(value):
    if not value:
        raise RuntimeError()


def identity(meta):
    return (meta.st_dev, meta.st_ino, meta.st_mode, meta.st_uid, meta.st_gid,
            meta.st_nlink, meta.st_size, meta.st_mtime_ns, meta.st_ctime_ns)


def retained(directory_fd, name, expected, mode, maximum):
    fd = os.open(name, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC,
                 dir_fd=directory_fd)
    with os.fdopen(fd, 'rb') as stream:
        before = os.fstat(stream.fileno())
        require(stat.S_ISREG(before.st_mode) and before.st_uid == before.st_gid == 1000
                and stat.S_IMODE(before.st_mode) == mode and before.st_nlink == 1
                and 0 < before.st_size <= maximum and not os.listxattr(stream.fileno()))
        data = stream.read(maximum + 1)
        require(identity(before) == identity(os.fstat(stream.fileno()))
                == identity(os.stat(name, dir_fd=directory_fd, follow_symlinks=False))
                and len(data) == before.st_size and hashlib.sha256(data).hexdigest() == expected)
        return data


def main():
    require(os.getuid() == os.geteuid() == os.getgid() == os.getegid() == 0
            and len(sys.argv) == 1 and pwd.getpwuid(1000).pw_name == 'kdk_vm')
    run = Path('/run').lstat()
    require(stat.S_ISDIR(run.st_mode) and run.st_uid == run.st_gid == 0 and run.st_mode & 0o022 == 0)
    source_fd = os.open(SOURCE, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC)
    source = os.fstat(source_fd)
    require(source.st_uid == source.st_gid == 1000 and stat.S_IMODE(source.st_mode) == 0o700
            and identity(source) == identity(SOURCE.lstat())
            and set(os.listdir(source_fd)) == set(MEMBERS) | {'root-stage.py'})
    require(not DESTINATION.exists() and not DESTINATION.is_symlink())
    # Every artifact is admitted from a stable original FD before publication.
    # root-stage.py in user staging is NOT executed: the reviewed loader must be
    # delivered through the trusted host/root channel with python -I -B.
    data = {name: retained(source_fd, name, expected, source_mode, maximum)
            for name, (expected, source_mode, _, maximum) in MEMBERS.items()}
    require(identity(source) == identity(os.fstat(source_fd)) == identity(SOURCE.lstat())
            and set(os.listdir(source_fd)) == set(MEMBERS) | {'root-stage.py'})
    os.mkdir(DESTINATION, 0o700)
    destination_fd = os.open(DESTINATION, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC)
    for name, (_, _, mode, _) in MEMBERS.items():
        fd = os.open(name, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW | os.O_CLOEXEC,
                     mode, dir_fd=destination_fd)
        with os.fdopen(fd, 'wb') as stream:
            stream.write(data[name])
            stream.flush()
            os.fsync(stream.fileno())
    os.fsync(destination_fd)
    os.execve('/usr/bin/python3', ['/usr/bin/python3', '-I', '-B', str(DESTINATION / 'guard.py')],
              {'PATH': '/usr/bin', 'LC_ALL': 'C', 'OMAVLESS_K1_RESPONSE_DIAGNOSTIC_GUARD': '1'})


if __name__ == '__main__':
    os.umask(0o077)
    try:
        main()
    except BaseException:
        print('K1_RESPONSE_DIAGNOSTIC_STAGING_NONPASS_RETAINED', file=sys.stderr)
        sys.exit(2)
