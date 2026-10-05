#!/usr/bin/python3
"""Fixed trusted-stdin staging only; never execute transferred bytes."""
import hashlib
import os
import stat
import sys

PARTS = ("home", "kdk_vm", ".cache", "t3-brotli-encoder-provenance-review-1")
PINS = {
    "probe.py": "f2b5f27cd626283f9655a204d5943b4c1c07f9d2f2b42297e07a4ce9533ee0ff",
    "supervisor.py": "8a6d35f17efd7c661d4d52ef12772e34088e9dcd7d811c6d26314cdd0624a5f6",
    "validator.py": "7a33c3e35f1b6edda95c65e89870d0e06c89dfdb0b9b8304cbcc74aeadafeef0",
    "owned.py": "8d9504cff609d703870b4b7f8bec4e4b601d8763b5f69b7df62e3b5da9106258",
    "containment.py": "2b9980266bd467c0684ee489167aadb4b53495340d26c6389aed723d67736592",
    "helpers.py": "cccc171213f4631f54d906652aeaf7954230949a7de40b2093c8ab587f86aa00",
    "copy-manifest.json": "d6ca9cd599adb573f4419f49147677f681db08fcc54a06cb0f34ed950ce19c36",
    "vm-guard.sh": "c00f63d2c87a6cede35fc0f2926428c28219122424a3601788e287acddc519ef"
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
                require(entry.name in PINS and entry.name not in seen and len(seen) < len(PINS))
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
