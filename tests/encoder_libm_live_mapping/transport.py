#!/usr/bin/python3
"""Fixed trusted-stdin staging only; never execute transferred bytes."""
import hashlib
import os
import stat
import sys

PARTS = ("home", "kdk_vm", ".cache", "t3-encoder-libm-mapping-review-1")
PINS = {
    "probe.py": "2c05c470257877d685931f468d4c54cf37027f76fe31a08bd59a8d7d4b938511",
    "lifecycle.py": "deb8836caf1f31e94ab91a3ccba7eefd219678c9343acd68803986ad46e2df3a",
    "containment.py": "2b9980266bd467c0684ee489167aadb4b53495340d26c6389aed723d67736592",
    "admission.py": "e6b0e8386e14e96d1111e8de22733b16990140e6b4f67497f18463436c4fb36a",
    "bridge.py": "7f0d95a5c75b84367421c860dc8b437a70ed92f7611e9e181e0315614f6a46dc",
    "copy-manifest.json": "b914dece6cb3c58f74bb4cdea8b19ade7b3b032e1b12d112d7524a2c47ca6c87",
    "guest-inventory.json": "4f1b92aeaff78f5f376576fc1da722cf9077735ff5c36d8674ff92581342ada4",
    "validate_receipt.py": "f0b19ad69b26806f36d3a81d860a004e56215138737d591d55fc80f19e17f43d",
    "vm-guard.sh": "610a36e4f3575dd906954c2bdaf25950eacf415c96da965dac6757c97ed50764"
}
FLAGS = os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC


def require(value):
    if not value:
        raise RuntimeError("fixed_transfer_refused")


def identity(s):
    return (s.st_dev, s.st_ino, s.st_mode, s.st_uid, s.st_gid, s.st_nlink,
            s.st_size, s.st_mtime_ns, s.st_ctime_ns)


def stage(action, name, data):
    # Validation before any mkdir/open-for-write. No arbitrary basename/path.
    require(action in ("create", "put"))
    require((action == "create" and name is None and data == b"")
            or (action == "put" and name in PINS and type(data) is bytes
                and 0 < len(data) <= 131072 and hashlib.sha256(data).hexdigest() == PINS[name]))
    directories, written = [], None
    try:
        def directory(parent, child, private=False):
            fd = os.open(child, FLAGS, dir_fd=parent)
            directories.append((fd, None, parent, child))
            s = os.fstat(fd)
            require(stat.S_ISDIR(s.st_mode) and s.st_uid in (0, 1000)
                    and s.st_gid in (0, 1000) and not s.st_mode & 0o022)
            if private:
                require(s.st_uid == s.st_gid == 1000 and stat.S_IMODE(s.st_mode) == 0o700)
            directories[-1] = (fd, s, parent, child)
            return fd
        parent = directory(None, "/")
        for child in PARTS[:-1]:
            parent = directory(parent, child)
        if action == "create":
            os.mkdir(PARTS[-1], mode=0o700, dir_fd=parent)
        target = directory(parent, PARTS[-1], True)
        # Bound the enumeration; unknown names immediately refuse.
        with os.scandir(target) as entries:
            seen = set()
            for entry in entries:
                require(entry.name in PINS and entry.name not in seen and len(seen) < 9)
                seen.add(entry.name)
        require(action != "create" or not seen)
        if action == "put":
            written = os.open(name, os.O_RDWR | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW
                              | os.O_CLOEXEC, 0o600, dir_fd=target)
            # A short/unknown write is terminal, leaving only this new partial
            # file. Do not loop, truncate, unlink, resume or reuse it.
            require(os.write(written, data) == len(data))
            os.fchmod(written, 0o500 if name.endswith((".py", ".sh")) else 0o600)
            os.fsync(written)
            s = os.fstat(written)
            require(stat.S_ISREG(s.st_mode) and s.st_uid == s.st_gid == 1000
                    and s.st_nlink == 1 and s.st_size == len(data) and not os.listxattr(written)
                    and stat.S_IMODE(s.st_mode) == (0o500 if name.endswith((".py", ".sh")) else 0o600))
            require(hashlib.sha256(os.pread(written, 131073, 0)).hexdigest() == PINS[name])
            require(identity(s) == identity(os.fstat(written))
                    == identity(os.stat(name, dir_fd=target, follow_symlinks=False)))
        # Parent entry creation legitimately changes directory timestamps/size:
        # only stable directory identity/ownership/mode are compared here.
        for fd, before, parent, child in directories:
            key = lambda s: identity(s)[:5]
            require(key(before) == key(os.fstat(fd))
                    == key(os.stat(child, dir_fd=parent, follow_symlinks=False)))
    finally:
        failed = False
        for fd in ([] if written is None else [written]) + [r[0] for r in reversed(directories)]:
            try:
                os.close(fd)
            except OSError:
                failed = True
        require(not failed)  # Single-attempt owned FD close, never retry.


def main():
    os.umask(0o077)
    require(os.getuid() == os.geteuid() == os.getgid() == os.getegid() == 1000)
    args = sys.argv[1:]
    require(args == ["--create-stage"] or
            (len(args) == 2 and args[0] == "--put" and args[1] in PINS))
    data = sys.stdin.buffer.read(131073)
    stage("create" if args == ["--create-stage"] else "put",
          None if args == ["--create-stage"] else args[1], data)
    print("FIXED_CREATE_ONLY_TRANSFER_PASS")


if __name__ == "__main__":
    try:
        main()
    except BaseException:
        os.write(2, b"FIXED_CREATE_ONLY_TRANSFER_REFUSED\n")
        raise SystemExit(1) from None
