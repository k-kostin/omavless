#!/usr/bin/env python3
"""Fixed ALPM file-size diagnosis only; no file-list output or ELF execution."""
import hashlib
import json
import os
from pathlib import Path
import re
import stat
import sys
import time

ROOT = Path("/var/lib/pacman/local")
STAGE = Path("/home/kdk_vm/.cache/t3-package-file-bound-review-1")
PACKAGE = re.compile(r"[A-Za-z0-9_+.@:-]{1,160}\Z")
LEGACY_LIMIT = 2 * 1024 * 1024
DIAGNOSTIC_LIMIT = 8 * 1024 * 1024
TOTAL_LIMIT = 64 * 1024 * 1024


class Refused(Exception):
    pass


def need(value, reason="package_bound_refused"):
    if not value:
        raise Refused(reason)


def identity(value):
    return (value.st_dev, value.st_ino, value.st_size, value.st_uid, value.st_gid,
            value.st_mode, value.st_nlink, value.st_mtime_ns, value.st_ctime_ns)


def root_owned(value):
    return value.st_uid == value.st_gid == 0 and value.st_mode & 0o022 == 0


def check(record):
    fd, path, before, parent = record
    need(identity(before) == identity(os.fstat(fd)) == identity(path.lstat())
         and identity(parent) == identity(path.parent.lstat()), "original_fd_path_or_parent_changed")


def open_record(path, maximum):
    parent = path.parent.lstat()
    need(stat.S_ISDIR(parent.st_mode) and root_owned(parent), "package_directory_shape")
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC)
    try:
        before = os.fstat(fd)
        need(stat.S_ISREG(before.st_mode) and root_owned(before) and before.st_nlink > 0,
             "package_nonregular_or_unsafe")
        need(0 <= before.st_size <= maximum, "diagnostic_size_bound")
        record = fd, path, before, parent
        check(record)
        return record
    except BaseException:
        os.close(fd)
        raise


def read_record(record, deadline, budget, capture=False):
    check(record)
    fd, path, before, parent = record
    need(budget[0] + before.st_size <= TOTAL_LIMIT, "metadata_read_bound")
    os.lseek(fd, 0, os.SEEK_SET)
    digest, chunks, length = hashlib.sha256(), [], 0
    while True:
        need(time.monotonic() < deadline, "metadata_deadline")
        chunk = os.read(fd, 65536)
        if not chunk:
            break
        length += len(chunk)
        budget[0] += len(chunk)
        need(length <= before.st_size and budget[0] <= TOTAL_LIMIT, "metadata_read_bound")
        digest.update(chunk)
        if capture:
            chunks.append(chunk)
    need(length == before.st_size, "metadata_short_read")
    check(record)
    metadata = dict(zip(("device", "inode", "size", "uid", "gid", "mode", "nlink", "mtime_ns", "ctime_ns"), identity(before)))
    metadata["sha256"] = digest.hexdigest()
    return b"".join(chunks), metadata


def package_name_version(raw):
    rows = raw.decode("utf-8", "strict").splitlines()
    result = {}
    for key in ("NAME", "VERSION"):
        marker = "%" + key + "%"
        need(rows.count(marker) == 1, "package_description_shape")
        index = rows.index(marker) + 1
        need(index < len(rows) and PACKAGE.fullmatch(rows[index]), "package_description_value")
        result[key.lower()] = rows[index]
    return result


def capture():
    result = {"schema": "public-package-file-bound-v1", "outcome": "NONPASS",
              "readelf_executed": False, "candidate_elf_executed": False,
              "allowlist_adoption": False, "static_limit_changed": False}
    try:
        need(os.getuid() == os.geteuid() == 1000, "metadata_uid")
        root = ROOT.lstat()
        need(stat.S_ISDIR(root.st_mode) and root_owned(root), "database_root_shape")
        names = sorted(os.listdir(ROOT))
        need(len(names) <= 4096, "package_count_bound")
        deadline, budget, scanned, scanned_bytes = time.monotonic() + 15, [0], 0, 0
        for name in names:
            if name == "ALPM_DB_VERSION":
                continue
            need(PACKAGE.fullmatch(name), "package_directory_name")
            record = open_record(ROOT / name / "files", DIAGNOSTIC_LIMIT)
            try:
                _, metadata = read_record(record, deadline, budget)
                scanned += 1
                scanned_bytes += metadata["size"]
                if metadata["size"] <= LEGACY_LIMIT:
                    continue
                desc = open_record(ROOT / name / "desc", 65536)
                try:
                    raw, desc_metadata = read_record(desc, deadline, budget, True)
                    package = package_name_version(raw)
                    _, final = read_record(record, deadline, budget)
                    _, desc_final = read_record(desc, deadline, budget)
                    need(final == metadata and desc_final == desc_metadata, "retained_input_changed")
                    check(record)
                    check(desc)
                    need(identity(root) == identity(ROOT.lstat()), "database_root_changed")
                    result.update({"outcome": "OBSERVED_PACKAGE_FILE_BOUND", "package_directory": name,
                                   "package": package, "original_open_fd": metadata,
                                   "description_open_fd": desc_metadata,
                                   "legacy_bound_bytes": LEGACY_LIMIT, "diagnostic_bound_bytes": DIAGNOSTIC_LIMIT,
                                   "legacy_regular": True, "legacy_size_within_bound": False,
                                   "scanned_count": scanned, "scanned_bytes": scanned_bytes,
                                   "read_bytes_including_rechecks": budget[0]})
                    break
                finally:
                    os.close(desc[0])
            finally:
                os.close(record[0])
        else:
            raise Refused("original_size_predicate_failure_not_reproduced")
    except Exception as error:
        result["reason"] = str(error) if isinstance(error, Refused) else type(error).__name__
    return result


if __name__ == "__main__":
    need(Path(__file__) == STAGE / "probe.py" and sys.argv[1:] == ["--capture-package-bound"], "fixed_metadata_entry")
    os.umask(0o077)
    receipt = capture()
    print(json.dumps(receipt, sort_keys=True))
    sys.exit(0 if receipt["outcome"] == "OBSERVED_PACKAGE_FILE_BOUND" else 1)
