#!/usr/bin/env python3
"""Strict typed fixed-size diagnosis receipts only; no imports of staged code."""
import json
import os
from pathlib import Path
import re
import stat
import sys

STAGE = Path("/home/kdk_vm/.cache/t3-package-file-bound-review-1")


def need(value):
    if not value:
        raise ValueError("package_bound_receipt_refused")


def keys(value, fields):
    need(type(value) is dict and set(value) == set(fields.split()))


def number(value, lower, upper):
    need(type(value) is int and lower <= value <= upper)


def metadata(value, lower, upper):
    keys(value, "device inode size uid gid mode nlink mtime_ns ctime_ns sha256")
    for name in ("device", "inode", "size", "uid", "gid", "mode", "nlink", "mtime_ns", "ctime_ns"):
        number(value[name], 0, 2**64 - 1)
    need(value["uid"] == value["gid"] == 0 and value["inode"] > 0 and value["nlink"] > 0)
    need(stat.S_ISREG(value["mode"]) and value["mode"] <= 0o177777 and value["mode"] & 0o022 == 0)
    number(value["size"], lower, upper)
    need(type(value["sha256"]) is str and re.fullmatch(r"[0-9a-f]{64}", value["sha256"]))


def validate(value, owned):
    keys(owned, "schema outcome returncode")
    need(owned["schema"] == "package-bound-owned-child-v1" and owned["outcome"] == "KNOWN_COMPLETED")
    number(owned["returncode"], 0, 0)
    keys(value, "schema outcome readelf_executed candidate_elf_executed allowlist_adoption static_limit_changed package_directory package original_open_fd description_open_fd legacy_bound_bytes diagnostic_bound_bytes legacy_regular legacy_size_within_bound scanned_count scanned_bytes read_bytes_including_rechecks")
    need(value["schema"] == "public-package-file-bound-v1" and value["outcome"] == "OBSERVED_PACKAGE_FILE_BOUND")
    for name in ("readelf_executed", "candidate_elf_executed", "allowlist_adoption", "static_limit_changed", "legacy_size_within_bound"):
        need(value[name] is False)
    need(value["legacy_regular"] is True)
    keys(value["package"], "name version")
    for name in (value["package_directory"], value["package"]["name"], value["package"]["version"]):
        need(type(name) is str and re.fullmatch(r"[A-Za-z0-9_+.@:-]{1,160}", name))
    number(value["legacy_bound_bytes"], 2 * 1024 * 1024, 2 * 1024 * 1024)
    number(value["diagnostic_bound_bytes"], 8 * 1024 * 1024, 8 * 1024 * 1024)
    metadata(value["original_open_fd"], 2 * 1024 * 1024 + 1, 8 * 1024 * 1024)
    metadata(value["description_open_fd"], 1, 65536)
    number(value["scanned_count"], 1, 4096)
    number(value["scanned_bytes"], value["original_open_fd"]["size"], 64 * 1024 * 1024)
    number(value["read_bytes_including_rechecks"], 0, 64 * 1024 * 1024)
    need(value["read_bytes_including_rechecks"] == value["scanned_bytes"] + value["original_open_fd"]["size"] + 2 * value["description_open_fd"]["size"])


def pairs(items):
    result = {}
    for key, value in items:
        need(key not in result)
        result[key] = value
    return result


def decode(raw):
    need(type(raw) is bytes and len(raw) <= 16384)
    return json.loads(raw.decode("utf-8", "strict"), object_pairs_hook=pairs,
                      parse_constant=lambda value: need(False))


def identity(m):
    return (m.st_dev, m.st_ino, m.st_uid, m.st_gid, m.st_mode, m.st_nlink, m.st_size, m.st_mtime_ns, m.st_ctime_ns)


def read(path):
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC)
    try:
        before = os.fstat(fd)
        need(stat.S_ISREG(before.st_mode) and before.st_uid == before.st_gid == 1000
             and stat.S_IMODE(before.st_mode) == 0o600 and before.st_nlink == 1 and 0 < before.st_size <= 16384)
        raw = os.pread(fd, 16385, 0)
        need(len(raw) == before.st_size and identity(before) == identity(os.fstat(fd)) == identity(path.lstat()))
        return decode(raw)
    finally:
        os.close(fd)


def main():
    need(Path(__file__) == STAGE / "validator.py" and sys.argv[1:] == ["--validate-package-bound"])
    need(os.getuid() == os.geteuid() == 1000)
    before = STAGE.lstat()
    need(stat.S_ISDIR(before.st_mode) and before.st_uid == before.st_gid == 1000 and stat.S_IMODE(before.st_mode) == 0o700)
    validate(read(STAGE / "result.json"), read(STAGE / "supervisor-receipt.json"))
    need(identity(before) == identity(STAGE.lstat()))
    print("PACKAGE_BOUND_METADATA_OBSERVED_NOT_STATIC_LIMIT_CHANGE")


if __name__ == "__main__":
    try:
        main()
    except Exception:
        print("PACKAGE_BOUND_RECEIPT_NONPASS", file=sys.stderr)
        sys.exit(1)
