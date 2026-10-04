#!/usr/bin/env python3
"""Fixed dbus FD metadata/self-mmap diagnostic. No mapped bytes are accessed."""
import ctypes
import json
import os
from pathlib import Path
import re
import stat
import sys

TARGET = "/usr/bin/dbus-daemon"
LENGTH = 4096


class Refused(Exception):
    pass


def require(condition, reason):
    if not condition:
        raise Refused(reason)


def bounded_text(path, limit):
    with open(path, "r", encoding="ascii", errors="strict") as source:
        text = source.read(limit + 1)
    require(len(text) <= limit, "metadata_bound")
    return text


class Statfs(ctypes.Structure):
    # Linux x86_64 glibc struct statfs; refuse every other ABI before calling libc.
    _fields_ = [(name, ctypes.c_long) for name in ("type", "bsize")]
    _fields_ += [(name, ctypes.c_ulong) for name in ("blocks", "bfree", "bavail", "files", "ffree")]
    _fields_ += [("fsid", ctypes.c_int * 2)]
    _fields_ += [(name, ctypes.c_long) for name in ("namelen", "frsize", "flags")]
    _fields_ += [("spare", ctypes.c_long * 4)]


def libc_calls():
    require(sys.platform == "linux" and os.uname().machine == "x86_64"
            and (os.confstr("CS_GNU_LIBC_VERSION") or "").startswith("glibc ")
            and ctypes.sizeof(ctypes.c_void_p) == 8 and ctypes.sizeof(ctypes.c_long) == 8
            and ctypes.sizeof(Statfs) == 120 and os.sysconf("SC_PAGE_SIZE") == LENGTH,
            "unsupported_abi")
    libc = ctypes.CDLL(None, use_errno=True)
    libc.fstatfs.argtypes = [ctypes.c_int, ctypes.POINTER(Statfs)]
    libc.fstatfs.restype = ctypes.c_int
    libc.mmap.argtypes = [ctypes.c_void_p, ctypes.c_size_t, ctypes.c_int,
                          ctypes.c_int, ctypes.c_int, ctypes.c_long]
    libc.mmap.restype = ctypes.c_void_p
    libc.munmap.argtypes = [ctypes.c_void_p, ctypes.c_size_t]
    libc.munmap.restype = ctypes.c_int
    return libc


def fd_metadata(value):
    return {"device": value.st_dev, "inode": value.st_ino, "uid": value.st_uid,
            "gid": value.st_gid, "mode": value.st_mode, "nlink": value.st_nlink,
            "size": value.st_size}


def stable_identity(value):
    return (fd_metadata(value), value.st_mtime_ns, value.st_ctime_ns)


def mount_metadata(fdinfo, mountinfo):
    ids = re.findall(r"^mnt_id:\s*([0-9]{1,10})$", fdinfo, re.MULTILINE)
    require(len(ids) == 1, "fd_mount_id_shape")
    selected = []
    for line in mountinfo.splitlines():
        row = line.split()
        if row and row[0] == ids[0]:
            require(len(row) >= 10 and row.count("-") == 1, "mountinfo_shape")
            separator = row.index("-")
            require(separator >= 6 and len(row) == separator + 4, "mountinfo_shape")
            require(re.fullmatch(r"[0-9]{1,10}", row[1]) is not None
                    and re.fullmatch(r"[0-9]{1,10}:[0-9]{1,10}", row[2]) is not None,
                    "mountinfo_numeric_shape")
            major, minor = (int(value) for value in row[2].split(":"))
            kind = row[separator + 1]
            # Never retain root, mountpoint, source, options, optional fields or unknown text.
            selected.append({"mount_id": int(ids[0]), "parent_mount_id": int(row[1]),
                             "device_major": major, "device_minor": minor,
                             "filesystem_type": kind if kind in ("btrfs", "ext4", "xfs", "tmpfs", "overlay") else "other"})
    require(len(selected) == 1, "fd_mount_not_unique")
    return selected[0]


def mapped_metadata(text, address):
    selected = []
    for line in text.splitlines():
        row = line.split(maxsplit=5)
        require(len(row) >= 5 and re.fullmatch(r"[0-9a-f]+-[0-9a-f]+", row[0]), "self_maps_shape")
        start, end = (int(value, 16) for value in row[0].split("-"))
        if start <= address < end:
            require(start == address and end == address + LENGTH and row[1] == "r--p"
                    and row[2] == "00000000" and len(row) == 6 and row[5] == TARGET,
                    "self_mapping_shape")
            require(re.fullmatch(r"[0-9a-f]{1,8}:[0-9a-f]{1,8}", row[3])
                    and re.fullmatch(r"[0-9]{1,20}", row[4]), "self_mapping_identity_shape")
            major, minor = (int(value, 16) for value in row[3].split(":"))
            selected.append({"device": os.makedev(major, minor), "inode": int(row[4]),
                             "length": LENGTH, "permissions": "r--p", "offset": 0})
    require(len(selected) == 1, "self_mapping_not_unique")
    return selected[0]


def observe():
    result = {"schema": "fixed-fd-filesystem-diagnostic-v1", "outcome": "NONPASS",
              "path": TARGET, "mapped_bytes_accessed": False, "loaded_elf_identity_proven": False,
              "allowlist_adoption": False, "compatibility_acceptance": False}
    fd, address, libc = None, None, None
    try:
        require(os.getuid() == os.geteuid() == 1000, "diagnostic_uid")
        libc = libc_calls()
        kernel = os.uname().release
        require(re.fullmatch(r"[A-Za-z0-9._+-]{1,128}", kernel), "kernel_release_shape")
        result["kernel_release"] = kernel
        fd = os.open(TARGET, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC)
        before = os.fstat(fd)
        result["original_open_fd"] = fd_metadata(before)
        require(stat.S_ISREG(before.st_mode) and before.st_uid == before.st_gid == 0
                and before.st_mode == stat.S_IFREG | 0o755 and before.st_nlink == 1
                and (before.st_dev, before.st_ino, before.st_size) == (31, 26297, 199176),
                "fixed_fd_identity_changed")
        fs = Statfs()
        require(libc.fstatfs(fd, ctypes.byref(fs)) == 0, "fstatfs_failed")
        result["filesystem_magic"] = ctypes.c_ulong(fs.type).value
        mount = mount_metadata(bounded_text(f"/proc/self/fdinfo/{fd}", 4096),
                               bounded_text("/proc/self/mountinfo", 2 * 1024 * 1024))
        result["mount"] = mount
        # PROT_READ=1, MAP_PRIVATE=2, offset=0. No PROT_EXEC/WRITE, MAP_FIXED or pointer reads.
        pointer = libc.mmap(None, LENGTH, 1, 2, fd, 0)
        require(pointer is not None and pointer != ctypes.c_void_p(-1).value, "mmap_failed")
        address = pointer
        mapped = mapped_metadata(bounded_text("/proc/self/maps", 1024 * 1024), address)
        require(mapped == mapped_metadata(bounded_text("/proc/self/maps", 1024 * 1024), address),
                "self_mapping_changed")
        require(mount == mount_metadata(bounded_text(f"/proc/self/fdinfo/{fd}", 4096),
                                       bounded_text("/proc/self/mountinfo", 2 * 1024 * 1024)),
                "fd_mount_changed")
        require(stable_identity(before) == stable_identity(os.fstat(fd)), "fixed_fd_changed")
        result["self_mapping"] = mapped
        result["device_equal"] = mapped["device"] == before.st_dev
        result["inode_equal"] = mapped["inode"] == before.st_ino
        result["outcome"] = "OBSERVED_READONLY_METADATA"
    except Exception as error:
        result["reason"] = str(error) if isinstance(error, Refused) else type(error).__name__
    finally:
        if address is not None:
            try:
                require(libc.munmap(address, LENGTH) == 0, "munmap_failed")
            except Exception:
                result["outcome"] = "NONPASS"
                result["cleanup_reason"] = "munmap_failed"
        if fd is not None:
            try:
                os.close(fd)
            except OSError:
                result["outcome"] = "NONPASS"
                result["cleanup_reason"] = "close_failed"
    return result


if __name__ == "__main__":
    require(sys.argv[1:] == ["--run-fixed-readonly-diagnostic"], "explicit_opt_in")
    receipt = observe()
    print(json.dumps(receipt, sort_keys=True))
    sys.exit(0 if receipt["outcome"] == "OBSERVED_READONLY_METADATA" else 1)
