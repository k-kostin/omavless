#!/usr/bin/env python3
"""Strict finite public receipt validation; inert import, no candidate admission."""
import hashlib
import json
import os
from pathlib import Path
import posixpath
import re
import stat
import sys

STAGE = Path("/home/kdk_vm/.cache/t3-static-elf-empty-record-review-1")
INVENTORY_SHA = "4f1b92aeaff78f5f376576fc1da722cf9077735ff5c36d8674ff92581342ada4"
READELF_SHA = "a72f12f3dd8c560554a3ba818bcecbda9178db5befbcc64d91bbb6a241fc21bc"
SEARCH = ["/usr/lib", "/usr/lib/systemd"]
CANDIDATE = "/usr/lib/libbrotlicommon.so.1.2.0"


def need(value):
    if not value:
        raise ValueError("static_receipt_refused")


def keys(value, expected):
    need(type(value) is dict and set(value) == set(expected.split()))


def number(value, minimum=0, maximum=2**64 - 1):
    need(type(value) is int and minimum <= value <= maximum)


def text(value, pattern):
    need(type(value) is str and re.fullmatch(pattern, value) is not None)


def digest(value):
    text(value, r"[0-9a-f]{64}")


def basename(value):
    text(value, r"[A-Za-z0-9_+.-]{1,160}")
    need(value not in (".", ".."))


def public(value, canonical=False):
    need(type(value) is str)
    roots = ("/usr/bin", "/usr/lib", "/usr/lib/systemd") if canonical else ("/usr/bin", "/usr/lib", "/usr/lib/systemd", "/usr/lib64", "/lib64")
    if not canonical and value in ("/lib64", "/usr/lib64"):
        return
    need(any(value.startswith(root + "/") and re.fullmatch(r"[A-Za-z0-9_+.-]{1,160}", value[len(root) + 1:]) for root in roots))
    need(posixpath.normpath(value) == value)


def package(value):
    keys(value, "name version file_list_sha256 description_sha256")
    for field in ("name", "version"):
        text(value[field], r"[A-Za-z0-9_+.@:-]{1,160}")
    digest(value["file_list_sha256"])
    digest(value["description_sha256"])


def links(value, logical, resolved):
    need(type(value) is list and len(value) <= 8)
    current = logical
    for link in value:
        keys(link, "path target identity")
        public(link["path"])
        text(link["target"], r"[A-Za-z0-9_./+-]{1,256}")
        identity = link["identity"]
        need(type(identity) is list and len(identity) == 9)
        for item in identity:
            number(item)
        dev, ino, size, uid, gid, mode, nlink, mtime, ctime = identity
        need(ino > 0 and nlink > 0 and uid == gid == 0 and stat.S_ISLNK(mode)
             and mode <= 0o177777 and size == len(link["target"].encode()))
        need(current == link["path"] or current.startswith(link["path"] + "/"))
        tail = current[len(link["path"]):].lstrip("/")
        current = posixpath.normpath(posixpath.join(posixpath.dirname(link["path"]), link["target"]))
        if tail:
            current = posixpath.join(current, tail)
        public(current)
    need(current == resolved)


def validate(value, owned, inventory):
    keys(owned, "schema outcome returncode")
    need(owned["schema"] == "elf-static-owned-child-v1" and owned["outcome"] == "KNOWN_COMPLETED")
    number(owned["returncode"], 0, 0)
    keys(value, "schema outcome candidate_elf_executed allowlist_adoption loaded_elf_identity_proven compatibility_acceptance search_policy records readelf aliases")
    need(value["schema"] == "public-static-elf-provenance-v1" and value["outcome"] == "OBSERVED_STATIC_CANDIDATE_CLOSURE")
    for field in ("candidate_elf_executed", "allowlist_adoption", "loaded_elf_identity_proven", "compatibility_acceptance"):
        need(value[field] is False)
    need(type(value["search_policy"]) is list and value["search_policy"] == SEARCH)
    tool = value["readelf"]
    keys(tool, "path sha256 package")
    need(tool["path"] == "/usr/bin/readelf" and tool["sha256"] == READELF_SHA)
    package(tool["package"])
    need(tool["package"]["name"] == "binutils" and tool["package"]["version"] == "2.47-4")
    records, aliases = value["records"], value["aliases"]
    need(type(records) is list and 16 <= len(records) <= 64)
    need(type(aliases) is list and 17 <= len(aliases) <= 512)
    alias_index = {}
    for alias in aliases:
        keys(alias, "path resolved_path links")
        public(alias["path"])
        public(alias["resolved_path"], True)
        links(alias["links"], alias["path"], alias["resolved_path"])
        need(alias["path"] not in alias_index)
        alias_index[alias["path"]] = alias
    known = {row["resolved_path"]: row for row in inventory["elfs"].values()}
    expected_aliases = set(inventory["elfs"]) | {CANDIDATE}
    canonical_records, total, edge_count = {}, 0, 0
    for row in records:
        keys(row, "path resolved_path links device inode uid gid mode nlink size sha256 package source depth dynamic dependencies matches_original_manifest")
        public(row["path"])
        public(row["resolved_path"], True)
        links(row["links"], row["path"], row["resolved_path"])
        need(alias_index.get(row["path"]) == {key: row[key] for key in ("path", "resolved_path", "links")})
        need(row["resolved_path"] not in canonical_records)
        canonical_records[row["resolved_path"]] = row
        for field in ("device", "inode", "uid", "gid", "mode", "nlink", "size", "depth"):
            number(row[field])
        need(row["inode"] > 0 and row["nlink"] > 0 and row["uid"] == row["gid"] == 0)
        need(stat.S_ISREG(row["mode"]) and row["mode"] <= 0o177777 and row["mode"] & 0o022 == 0)
        need(0 < row["size"] <= 32 * 1024 * 1024 and row["depth"] <= 8)
        digest(row["sha256"])
        package(row["package"])
        need(type(row["matches_original_manifest"]) is bool)
        source = row["source"]
        need(source in ("original_manifest", "explicit_observation_candidate", "static_needed_candidate", "fixed_interpreter_candidate"))
        if source == "original_manifest":
            need(row["path"] in inventory["elfs"] and row["depth"] == 0)
        elif source == "explicit_observation_candidate":
            need(row["path"] == CANDIDATE and row["depth"] == 0)
        else:
            need(1 <= row["depth"] <= 8)
        original = known.get(row["resolved_path"])
        need(row["matches_original_manifest"] is (original is not None))
        if original is not None:
            need(row["sha256"] == original["sha256"] and stat.S_IMODE(row["mode"]) == int(original["mode"], 8))
        dynamic = row["dynamic"]
        keys(dynamic, "needed interpreter declared_search_tokens")
        need(type(dynamic["needed"]) is list and len(dynamic["needed"]) <= 64)
        for name in dynamic["needed"]:
            basename(name)
        need(len(set(dynamic["needed"])) == len(dynamic["needed"]))
        need(dynamic["interpreter"] is None or dynamic["interpreter"] in ("/lib64/ld-linux-x86-64.so.2", "/usr/lib/ld-linux-x86-64.so.2"))
        tokens = dynamic["declared_search_tokens"]
        need(type(tokens) is list and len(tokens) <= 131072 and all(type(token) is str and token in (*SEARCH, "$ORIGIN", "${ORIGIN}") for token in tokens))
        deps = row["dependencies"]
        need(type(deps) is list and len(deps) == len(dynamic["needed"]))
        for name, dep in zip(dynamic["needed"], deps):
            keys(dep, "name logical_path resolved_path links")
            need(dep["name"] == name and dep["logical_path"] in [root + "/" + name for root in SEARCH])
            public(dep["resolved_path"], True)
            links(dep["links"], dep["logical_path"], dep["resolved_path"])
            need(alias_index.get(dep["logical_path"]) == {"path": dep["logical_path"], "resolved_path": dep["resolved_path"], "links": dep["links"]})
            expected_aliases.add(dep["logical_path"])
        if dynamic["interpreter"]:
            expected_aliases.add(dynamic["interpreter"])
        edge_count += len(deps) + (dynamic["interpreter"] is not None)
        total += row["size"]
    need(total <= 128 * 1024 * 1024 and edge_count + 17 <= 512)
    need(set(alias_index) == expected_aliases)
    need(set(canonical_records) == {alias["resolved_path"] for alias in aliases})
    for path, original in inventory["elfs"].items():
        need(alias_index[path]["resolved_path"] == original["resolved_path"])
    for row in records:
        if row["source"] == "static_needed_candidate":
            need(any(parent["depth"] + 1 == row["depth"] and any(dep["logical_path"] == row["path"]
                     for dep in parent["dependencies"]) for parent in records))
        if row["source"] == "fixed_interpreter_candidate":
            need(any(parent["depth"] + 1 == row["depth"] and parent["dynamic"]["interpreter"] == row["path"]
                     for parent in records))


def pairs(items):
    result = {}
    for key, value in items:
        need(key not in result)
        result[key] = value
    return result


def decode(raw):
    need(type(raw) is bytes and len(raw) <= 4 * 1024 * 1024)
    return json.loads(raw.decode("utf-8", "strict"), object_pairs_hook=pairs,
                      parse_constant=lambda value: need(False))


def identity(value):
    return (value.st_dev, value.st_ino, value.st_size, value.st_uid, value.st_gid,
            value.st_mode, value.st_nlink, value.st_mtime_ns, value.st_ctime_ns)


def read(path, maximum):
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC)
    try:
        before = os.fstat(fd)
        need(stat.S_ISREG(before.st_mode) and before.st_uid == before.st_gid == 1000
             and stat.S_IMODE(before.st_mode) == 0o600 and before.st_nlink == 1
             and 0 < before.st_size <= maximum)
        with os.fdopen(os.dup(fd), "rb") as stream:
            raw = stream.read(maximum + 1)
        need(len(raw) == before.st_size and identity(before) == identity(os.fstat(fd)) == identity(path.lstat()))
        return raw
    finally:
        os.close(fd)


def main():
    need(Path(__file__) == STAGE / "validator.py" and sys.argv[1:] == ["--validate-static-receipt"])
    need(os.getuid() == os.geteuid() == 1000)
    before = STAGE.lstat()
    need(stat.S_ISDIR(before.st_mode) and before.st_uid == before.st_gid == 1000 and stat.S_IMODE(before.st_mode) == 0o700)
    inventory = read(STAGE / "guest-inventory.json", 32768)
    need(hashlib.sha256(inventory).hexdigest() == INVENTORY_SHA)
    validate(decode(read(STAGE / "result.json", 4 * 1024 * 1024)),
             decode(read(STAGE / "supervisor-receipt.json", 4096)), decode(inventory))
    need(identity(before) == identity(STAGE.lstat()))
    print("STATIC_ELF_CANDIDATE_CLOSURE_OBSERVED_NOT_ALLOWLIST_OR_ACCEPTANCE")


if __name__ == "__main__":
    try:
        main()
    except Exception:
        print("STATIC_ELF_RECEIPT_NONPASS", file=sys.stderr)
        sys.exit(1)
