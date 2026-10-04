#!/usr/bin/env python3
"""Read-only ALPM files shape evidence; no filenames/content or ELF execution."""
import hashlib
import json
import os
from pathlib import Path
import re
import stat
import sys
import time

PACKAGE = re.compile(r"[A-Za-z0-9_+.@:-]{1,160}\Z")
ROOT = Path("/var/lib/pacman/local")
MAX_FILE = 2 * 1024 * 1024


class Refused(Exception):
    pass


def require(value, reason):
    if not value:
        raise Refused(reason)


def identity(value):
    return (value.st_dev, value.st_ino, value.st_size, value.st_uid, value.st_gid,
            value.st_mode, value.st_nlink, value.st_mtime_ns, value.st_ctime_ns)


def root_owned(value):
    return value.st_uid == value.st_gid == 0 and value.st_mode & 0o022 == 0


def read_record(record, deadline):
    fd, path, before, parent = record
    require(identity(os.fstat(fd)) == identity(before), "selected_original_fd_changed")
    os.lseek(fd, 0, os.SEEK_SET)
    chunks, size, digest = [], 0, hashlib.sha256()
    while True:
        require(time.monotonic() < deadline, "metadata_deadline")
        chunk = os.read(fd, 65536)
        if not chunk:
            break
        size += len(chunk)
        require(size <= before.st_size, "package_file_read_bound")
        chunks.append(chunk)
        digest.update(chunk)
    require(size == before.st_size and identity(before) == identity(os.fstat(fd))
            == identity(path.lstat()) and identity(parent) == identity(path.parent.lstat()),
            "package_file_or_directory_changed")
    return b"".join(chunks), {"device": before.st_dev, "inode": before.st_ino,
            "size": before.st_size, "uid": before.st_uid, "gid": before.st_gid,
            "mode": before.st_mode, "nlink": before.st_nlink,
            "mtime_ns": before.st_mtime_ns, "ctime_ns": before.st_ctime_ns,
            "sha256": digest.hexdigest()}


def open_record(path):
    parent = path.parent.lstat()
    require(stat.S_ISDIR(parent.st_mode) and root_owned(parent), "package_directory_shape")
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC)
    try:
        before = os.fstat(fd)
        require(stat.S_ISREG(before.st_mode) and root_owned(before) and before.st_nlink > 0
                and 0 <= before.st_size <= MAX_FILE, "package_file_shape")
        return fd, path, before, parent
    except BaseException:
        os.close(fd)
        raise


def measure(path, deadline):
    record = open_record(path)
    try:
        return read_record(record, deadline)
    finally:
        os.close(record[0])


def proposed_file_entries(raw):
    """Primary-format parser proposal, NOT wired into ELF capture or admission."""
    rows = raw.decode("utf-8", "strict").split("\n")
    rows = [row for row in rows if row != ""]
    if not rows:
        return []
    section, seen, files, file_set, backups = None, set(), [], set(), set()
    for row in rows:
        if row.startswith("%") and row.endswith("%"):
            require(row in ("%FILES%", "%BACKUP%") and row not in seen, "unsupported_or_duplicate_section")
            require(row != "%BACKUP%" or section == "%FILES%" and files, "backup_without_files")
            require(row != "%FILES%" or not seen, "files_section_order")
            seen.add(row)
            section = row
        elif section == "%FILES%":
            require(not row.startswith("/") and "\x00" not in row
                    and not any(part in (".", "..") for part in row.split("/"))
                    and row not in file_set, "file_entry_shape")
            files.append(row)
            file_set.add(row)
        elif section == "%BACKUP%":
            path, separator, checksum = row.rpartition("\t")
            require(separator == "\t" and re.fullmatch(r"[0-9a-f]{32}", checksum)
                    and path in file_set and not path.endswith("/") and path not in backups,
                    "backup_entry_shape")
            backups.add(path)
        else:
            raise Refused("data_without_section")
    require(files and ("%BACKUP%" not in seen or backups), "empty_section")
    return files


def shape(raw):
    rows = raw.decode("utf-8", "strict").splitlines()
    headers = [row for row in rows if row.startswith("%") and row.endswith("%")]
    result = {"zero_bytes": raw == b"", "blank_lines_only": all(row == "" for row in raw.decode("utf-8").split("\n")),
              "line_count": len(rows), "files_marker_count": rows.count("%FILES%"),
              "backup_marker_count": rows.count("%BACKUP%"),
              "other_header_count": sum(row not in ("%FILES%", "%BACKUP%") for row in headers),
              "legacy_exactly_one_files_marker": rows.count("%FILES%") == 1}
    try:
        parsed = proposed_file_entries(raw)
        result.update({"proposed_format": "VALID", "proposed_entry_count": len(parsed)})
    except (Refused, UnicodeError) as error:
        result.update({"proposed_format": "NONPASS", "proposed_reason": str(error) if isinstance(error, Refused) else "encoding"})
    return result


def package_name_version(raw):
    rows = raw.decode("utf-8", "strict").splitlines()
    result = {}
    for key in ("NAME", "VERSION"):
        marker = "%" + key + "%"
        require(rows.count(marker) == 1, "package_description_shape")
        index = rows.index(marker) + 1
        require(index < len(rows) and PACKAGE.fullmatch(rows[index]), "package_description_value")
        result[key.lower()] = rows[index]
    return result


def capture():
    result = {"schema": "public-alpm-files-shape-v1", "outcome": "NONPASS",
              "readelf_executed": False, "candidate_elf_executed": False, "allowlist_adoption": False}
    try:
        require(os.getuid() == os.geteuid() == 1000, "metadata_uid")
        value = ROOT.lstat()
        require(stat.S_ISDIR(value.st_mode) and root_owned(value), "database_root_shape")
        names = sorted(os.listdir(ROOT))
        require(len(names) <= 4096, "package_count_bound")
        deadline, total, scanned = time.monotonic() + 10, 0, 0
        for name in names:
            if name == "ALPM_DB_VERSION":
                continue
            require(PACKAGE.fullmatch(name), "package_directory_name")
            record = open_record(ROOT / name / "files")
            try:
                raw, metadata = read_record(record, deadline)
                total += len(raw)
                scanned += 1
                require(total <= 64 * 1024 * 1024, "package_bytes_bound")
                if raw.decode("utf-8", "strict").splitlines().count("%FILES%") == 1:
                    continue
                summary = shape(raw)
                description, desc_metadata = measure(ROOT / name / "desc", deadline)
                # Re-read the STILL-OPEN original FD across the description read.
                # Any failed observation is terminal, never retried.
                again, final_metadata = read_record(record, deadline)
                require(again == raw and final_metadata == metadata, "selected_file_changed")
                result.update({"outcome": "OBSERVED_ALPM_FILELIST_SHAPE", "package_directory": name,
                               "package": package_name_version(description), "original_open_fd": metadata,
                               "description_sha256": desc_metadata["sha256"], "shape": summary,
                               "scanned_count": scanned, "scanned_bytes": total})
                break
            finally:
                os.close(record[0])
        else:
            raise Refused("original_predicate_failure_not_reproduced")
    except Exception as error:
        result["reason"] = str(error) if isinstance(error, Refused) else type(error).__name__
    return result


if __name__ == "__main__":
    require(sys.argv[1:] == ["--capture-alpm-shape"], "explicit_metadata_opt_in")
    os.umask(0o077)
    result = capture()
    print(json.dumps(result, sort_keys=True))
    sys.exit(0 if result["outcome"] == "OBSERVED_ALPM_FILELIST_SHAPE" else 1)
