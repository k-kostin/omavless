#!/usr/bin/python3
# SPDX-License-Identifier: MIT
"""Reviewed stdin-only root staging; no subprocess, execution, overwrite or IPC."""
import hashlib
import json
import os
from pathlib import Path
import re
import stat
import sys

LEGACY_SHA = "3631492302b3e87beb0b2a801ded9ee5df1235a5c125117b0f4728596067049d"
LIMIT = 8 * 1024 * 1024
RETAINED = []


def require(value):
    if not value:
        raise RuntimeError("staging refused")


def identity(m):
    return (m.st_dev, m.st_ino, m.st_uid, m.st_gid, m.st_mode,
            m.st_nlink, m.st_size, m.st_mtime_ns, m.st_ctime_ns)


def directory_identity(m):
    return (m.st_dev, m.st_ino, m.st_uid, m.st_gid, m.st_mode)


def directory(path, uid, mode):
    for ancestor in (path, *path.parents):
        m = ancestor.lstat()
        require(stat.S_ISDIR(m.st_mode) and m.st_uid in (0, uid) and m.st_mode & 0o6022 == 0)
        try:
            (ancestor / ".git").lstat()
        except FileNotFoundError:
            pass
        else:
            raise RuntimeError("Git ancestry refused")
    fd = os.open(path, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC)
    RETAINED.append(fd)
    before = os.fstat(fd)
    require(before.st_uid == before.st_gid == uid and stat.S_IMODE(before.st_mode) == mode
            and directory_identity(before) == directory_identity(path.lstat()))
    return fd, before


def recheck_directory(path, fd, before):
    require(directory_identity(before) == directory_identity(os.fstat(fd)) == directory_identity(path.lstat()))


class Source:
    def __init__(self, directory_fd, name, limit, mode, uid=1000, capture=True):
        require(name in ("fixture", "root_vm_guard.py", "vm_guard.py", "receipt.json"))
        self.directory_fd, self.name = directory_fd, name
        self.fd = os.open(name, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC, dir_fd=directory_fd)
        RETAINED.append(self.fd)
        self.before = os.fstat(self.fd)
        require(stat.S_ISREG(self.before.st_mode) and self.before.st_uid == self.before.st_gid == uid
                and stat.S_IMODE(self.before.st_mode) == mode and self.before.st_nlink == 1
                and 0 < self.before.st_size <= limit and not os.listxattr(self.fd))
        h = hashlib.sha256()
        data = []
        offset = 0
        while offset < self.before.st_size:
            part = os.pread(self.fd, min(65536, self.before.st_size - offset), offset)
            require(part)
            h.update(part)
            if capture:
                data.append(part)
            offset += len(part)
        require(os.pread(self.fd, 1, offset) == b"")
        self.sha, self.data = h.hexdigest(), b"".join(data)
        self.recheck()

    def recheck(self):
        require(identity(self.before) == identity(os.fstat(self.fd))
                == identity(os.stat(self.name, dir_fd=self.directory_fd, follow_symlinks=False))
                and not os.listxattr(self.fd))


def pairs(values):
    result = {}
    for key, value in values:
        require(key not in result)
        result[key] = value
    return result


def receipt_hashes(data):
    value = json.loads(data, object_pairs_hook=pairs)
    require(type(value) is dict and set(value) == {"schema", "head", "host_build_target", "host_original",
                                                "host_frozen", "elf_sha256", "guard_sha256"})
    require(value["schema"] == "t4-abort-vm-copy-v1")
    for name, length in (("head", 40), ("elf_sha256", 64), ("guard_sha256", 64)):
        require(type(value[name]) is str and re.fullmatch("[0-9a-f]{" + str(length) + "}", value[name]))
    # This loader copies authenticated bytes, not claims of build provenance.
    # The sealed root guard applies the complete host-provenance schema later.
    return value["elf_sha256"], value["guard_sha256"]


def copy_new(destination_fd, source, mode):
    require(source.name in ("root_vm_guard.py", "vm_guard.py", "receipt.json"))
    require(mode == (0o600 if source.name == "receipt.json" else 0o500))
    source.recheck()
    fd = os.open(source.name, os.O_RDWR | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW | os.O_CLOEXEC,
                 mode, dir_fd=destination_fd)
    RETAINED.append(fd)
    offset = 0
    while offset < len(source.data):
        count = os.write(fd, source.data[offset:])
        require(count > 0)
        offset += count
    os.fsync(fd)
    after = os.fstat(fd)
    require(stat.S_ISREG(after.st_mode) and after.st_uid == after.st_gid == 0
            and stat.S_IMODE(after.st_mode) == mode and after.st_nlink == 1
            and after.st_size == len(source.data) and not os.listxattr(fd)
            and identity(after) == identity(os.stat(source.name, dir_fd=destination_fd, follow_symlinks=False)))
    copied = os.pread(fd, len(source.data) + 1, 0)
    require(len(copied) == len(source.data) and hashlib.sha256(copied).hexdigest() == source.sha
            and identity(after) == identity(os.fstat(fd))
            == identity(os.stat(source.name, dir_fd=destination_fd, follow_symlinks=False)))
    source.recheck()


def main():
    require(os.getresuid() == os.getresgid() == (0, 0, 0))
    require(__file__ == "<stdin>" and sys.flags.isolated == 1 and sys.dont_write_bytecode)
    require(len(sys.argv) == 4 and sys.argv[:2] == ["-", "--stage-reviewed-root-observed-abort"])
    nonce, digest = sys.argv[2:]
    require(re.fullmatch(r"[0-9a-f]{32}", nonce) and re.fullmatch(r"[0-9a-f]{64}", digest))
    name = "ov-abort-root-" + nonce
    source_path = Path("/run/user/1000") / name
    source_fd, source_before = directory(source_path, 1000, 0o700)
    run = Path("/run")
    run_fd, run_before = directory(run, 0, 0o755)
    receipt = Source(source_fd, "receipt.json", 8192, 0o600)
    require(receipt.sha == digest)
    elf_sha, guard_sha = receipt_hashes(receipt.data)
    legacy = Source(source_fd, "vm_guard.py", LIMIT, 0o500)
    guard = Source(source_fd, "root_vm_guard.py", LIMIT, 0o500)
    elf = Source(source_fd, "fixture", 1024**3, 0o500, capture=False)
    require(legacy.sha == LEGACY_SHA and guard.sha == guard_sha and elf.sha == elf_sha
            and os.pread(elf.fd, 4, 0) == b"\x7fELF")
    for item in (receipt, legacy, guard, elf):
        item.recheck()
    recheck_directory(source_path, source_fd, source_before)
    recheck_directory(run, run_fd, run_before)
    # Create-only atomic name admission; never adopt or overwrite an old stage.
    os.mkdir(name, mode=0o700, dir_fd=run_fd)
    destination = run / name
    destination_fd, destination_before = directory(destination, 0, 0o700)
    for item, mode in ((receipt, 0o600), (legacy, 0o500), (guard, 0o500)):
        copy_new(destination_fd, item, mode)
        recheck_directory(destination, destination_fd, destination_before)
    for item in (receipt, legacy, guard, elf):
        item.recheck()
    recheck_directory(source_path, source_fd, source_before)
    recheck_directory(run, run_fd, run_before)
    require(set(os.listdir(destination_fd)) == {"receipt.json", "vm_guard.py", "root_vm_guard.py"})
    os.fsync(destination_fd)
    os.fsync(run_fd)
    print("T4_ROOT_STAGE_CREATED_ONLY_NOT_EXECUTED")


if __name__ == "__main__":
    os.umask(0o077)
    try:
        main()
    except BaseException:
        # No cleanup, query, retry, exec or privileged write after uncertainty.
        print("T4_ROOT_STAGE_NONPASS_RETAINED", file=sys.stderr)
        sys.exit(2)
