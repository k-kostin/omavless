#!/usr/bin/env python3
"""Opt-in kernel hypothesis probe. Never enters the host network namespace.

No sudo, system bus, installed runtime or private store. The unprivileged parent
creates NEW user AND network namespaces; the child refuses anything but a fresh
loopback-only namespace. All interfaces disappear when that child exits.
Output is fixed booleans, not host inventory. Not run by the normal test suite.
"""
import errno
import fcntl
import json
import os
from pathlib import Path
import struct
import subprocess
import sys

NAME = b"ovdnstest0"
TUNSETIFF = 0x400454CA
TUNGETIFF = 0x800454D2
TUNSETIFINDEX = 0x400454DA
FLAGS = 0x0001 | 0x1000  # IFF_TUN | IFF_NO_PI, nonpersistent


def namespace(kind):
    return os.readlink(f"/proc/self/ns/{kind}")


def links():
    # /sys may still be a host mount even inside a new network namespace.
    # Netlink follows the calling process's network namespace instead.
    result = subprocess.run(
        ["/usr/bin/ip", "-j", "link", "show"], stdin=subprocess.DEVNULL,
        capture_output=True, timeout=3, check=True,
    )
    if len(result.stdout) > 16384:
        raise RuntimeError("inventory_bound")
    return json.loads(result.stdout)


def tun_index():
    inventory = links()
    candidates = [item["ifindex"] for item in inventory if item["ifname"] == NAME.decode("ascii")]
    if len(candidates) != 1 or type(candidates[0]) is not int or candidates[0] <= 0:
        raise RuntimeError("isolated_tun_missing")
    return candidates[0]


def open_tun(index=None):
    fd = os.open("/dev/net/tun", os.O_RDWR | os.O_CLOEXEC)
    try:
        if index is not None:
            fcntl.ioctl(fd, TUNSETIFINDEX, struct.pack("I", index))
        fcntl.ioctl(fd, TUNSETIFF, struct.pack("16sH22x", NAME, FLAGS))
        return fd
    except BaseException:
        os.close(fd)
        raise


def fd_attached(fd):
    try:
        result = fcntl.ioctl(fd, TUNGETIFF, bytes(40))
        return result[:16].split(b"\0", 1)[0] == NAME
    except OSError as error:
        if error.errno in (errno.EBADFD, errno.ENODEV, errno.EINVAL):
            return False
        raise


def child(original_net, original_user):
    if namespace("net") == original_net or namespace("user") == original_user:
        raise RuntimeError("namespace_isolation_missing")
    if os.geteuid() != 0 or {item["ifname"] for item in links()} != {"lo"}:
        raise RuntimeError("fresh_namespace_required")
    fd = open_tun()
    replacement = None
    try:
        index = tun_index()
        attached_before = fd_attached(fd)
        deleted = subprocess.run(
            ["/usr/bin/ip", "link", "delete", "dev", NAME.decode("ascii")],
            stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL, timeout=3, check=False,
        ).returncode == 0
        if not deleted:
            return {"isolated": True, "attached_before": attached_before,
                    "delete_while_fd_open": False, "replacement_tested": False}
        old_attached_after = fd_attached(fd)
        replacement = open_tun(index)
        new_index = tun_index()
        return {
            "isolated": True, "attached_before": attached_before,
            "delete_while_fd_open": True, "replacement_tested": True,
            "same_name_and_index_reused": new_index == index,
            "old_fd_attached_after_delete": old_attached_after,
            "old_fd_attached_after_replacement": fd_attached(fd),
            "new_fd_attached": fd_attached(replacement),
        }
    finally:
        if replacement is not None:
            os.close(replacement)
        os.close(fd)


def main():
    if len(sys.argv) == 4 and sys.argv[1] == "--isolated-child":
        try:
            print(json.dumps(child(sys.argv[2], sys.argv[3]), sort_keys=True))
            return 0
        except Exception:
            print('{"isolated_probe_failed":true}')
            return 1
    if len(sys.argv) != 1 or os.geteuid() == 0:
        print('{"unprivileged_parent_required":true}')
        return 2
    try:
        result = subprocess.run(
            ["/usr/bin/unshare", "--user", "--map-root-user", "--net", "--",
             sys.executable, str(Path(__file__).resolve()), "--isolated-child",
             namespace("net"), namespace("user")],
            stdin=subprocess.DEVNULL, capture_output=True, timeout=10, check=False,
        )
    except (OSError, subprocess.TimeoutExpired):
        print('{"namespace_probe_unavailable":true}')
        return 1
    # Child emits only fixed keys/booleans; never forward raw errors.
    allowed = {"isolated", "attached_before", "delete_while_fd_open", "replacement_tested",
               "same_name_and_index_reused", "old_fd_attached_after_delete",
               "old_fd_attached_after_replacement", "new_fd_attached", "isolated_probe_failed"}
    try:
        data = json.loads(result.stdout) if len(result.stdout) <= 1024 else None
        if not isinstance(data, dict) or not data or not set(data) <= allowed:
            raise ValueError()
        if not all(type(value) is bool for value in data.values()):
            raise ValueError()
    except (ValueError, UnicodeError):
        print('{"namespace_probe_unavailable":true}')
        return 1
    print(json.dumps(data, sort_keys=True))
    return 0 if result.returncode == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
