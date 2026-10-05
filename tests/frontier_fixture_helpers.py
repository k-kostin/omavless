"""Inert fixed-VM metadata facade; never used by a guest/product transport.

Real O_EXCL IO, identity, modes, timestamps and bytes stay intact. Only UID/GID
observations are modeled as the literal test VM account, so a cloud runner's
own account cannot accidentally decide this synthetic fixture's admission.
The facade is module-local: the interpreter's shared os/types modules are not
patched, and no actual ownership or host security policy is changed.
"""
import os
from types import SimpleNamespace

FIELDS = ("st_dev", "st_ino", "st_mode", "st_nlink", "st_size",
          "st_mtime_ns", "st_ctime_ns")


def fixed_vm_process_os(original):
    """Model the fixed supervisor account without patching shared os.getuid."""
    return SimpleNamespace(**{**vars(original), "getuid": lambda: 1000,
                              "geteuid": lambda: 1000})


def fixed_vm_os(root):
    def modeled(value):
        return SimpleNamespace(**{name: getattr(value, name) for name in FIELDS},
                               st_uid=1000, st_gid=1000)

    def opened(name, *args, **kwargs):
        return os.open(root if name == "/" else name, *args, **kwargs)

    def observed(name, *args, **kwargs):
        return modeled(os.stat(root if name == "/" else name, *args, **kwargs))

    return SimpleNamespace(**{**vars(os), "open": opened, "stat": observed,
                              "fstat": lambda fd: modeled(os.fstat(fd))})
