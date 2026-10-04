#!/usr/bin/python3
"""Fixed create-only VM staging; not an installed or generic privileged API."""
import hashlib
import os
from pathlib import Path
import pwd
import stat
import sys

SOURCE = Path('/home/kdk_vm/.cache/k1-manager-private-28bdba2-stage-1')
DESTINATION = Path('/run/omavless-k1-manager-private-lifecycle')
MEMBERS = {
    'probe': ('a3f224c7f9e0e288a2e93a64b428c65c4b03207388368cfc6e13572bc0cffa28', 0o500, 0o500),
    'query-guard.py': ('67e541ea5c764a05b669267b248d3df77ca3402569df5116bfff9346ff9ea0bd', 0o400, 0o600),
    'fixture.service': ('76f137c03314769af114b06ce4d9f4684e96893d1b12c6f15812a0082b24d8be', 0o400, 0o600),
    'guard.py': ('472643e1b497bdc7cdb36ca9c3c380811d03143fdaaf5385018f3f082dbe063d', 0o400, 0o500),
}


def require(value):
    if not value:
        raise RuntimeError()


def identity(meta):
    return (meta.st_dev, meta.st_ino, meta.st_mode, meta.st_uid, meta.st_gid,
            meta.st_nlink, meta.st_size, meta.st_mtime_ns, meta.st_ctime_ns)


def retained(directory_fd, name, expected, mode):
    fd = os.open(name, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC,
                 dir_fd=directory_fd)
    with os.fdopen(fd, 'rb') as stream:
        before = os.fstat(stream.fileno())
        require(stat.S_ISREG(before.st_mode) and before.st_uid == before.st_gid == 1000
                and stat.S_IMODE(before.st_mode) == mode and before.st_nlink == 1
                and 0 < before.st_size <= 32 * 1024 * 1024 and not os.listxattr(stream.fileno()))
        data = stream.read(32 * 1024 * 1024 + 1)
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
    # Admit every byte before creating or publishing any root artifact.
    data = {name: retained(source_fd, name, expected, source_mode)
            for name, (expected, source_mode, _) in MEMBERS.items()}
    require(identity(source) == identity(os.fstat(source_fd)) == identity(SOURCE.lstat())
            and set(os.listdir(source_fd)) == set(MEMBERS) | {'root-stage.py'})
    os.mkdir(DESTINATION, 0o700)
    destination_fd = os.open(DESTINATION, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC)
    for name, (_, _, mode) in MEMBERS.items():
        fd = os.open(name, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW | os.O_CLOEXEC,
                     mode, dir_fd=destination_fd)
        with os.fdopen(fd, 'wb') as stream:
            stream.write(data[name])
            stream.flush()
            os.fsync(stream.fileno())
    os.fsync(destination_fd)
    os.execve('/usr/bin/python3', ['/usr/bin/python3', '-I', '-B', str(DESTINATION / 'guard.py')],
              {'PATH': '/usr/bin', 'LC_ALL': 'C', 'OMAVLESS_K1_MANAGER_PRIVATE_GUARD': '1'})


if __name__ == '__main__':
    os.umask(0o077)
    try:
        main()
    except BaseException:
        print('K1_MANAGER_PRIVATE_STAGING_NONPASS_RETAINED', file=sys.stderr)
        sys.exit(2)
