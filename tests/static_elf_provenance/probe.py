#!/usr/bin/env python3
"""Read-only packaged ELF static provenance; never executes a candidate ELF."""
import errno
import hashlib
import json
import os
from pathlib import Path
import posixpath
import re
import stat
import sys
import time
import types

CONTAINMENT_SHA = "2b9980266bd467c0684ee489167aadb4b53495340d26c6389aed723d67736592"
INVENTORY_SHA = "4f1b92aeaff78f5f376576fc1da722cf9077735ff5c36d8674ff92581342ada4"
READELF_SHA = "a72f12f3dd8c560554a3ba818bcecbda9178db5befbcc64d91bbb6a241fc21bc"
READELF = "/usr/bin/readelf"
CANDIDATE = "/usr/lib/libbrotlicommon.so.1.2.0"
SEARCH = ("/usr/lib", "/usr/lib/systemd")
BASENAME = re.compile(r"[A-Za-z0-9_+.-]{1,160}\Z")
PACKAGE = re.compile(r"[A-Za-z0-9_+.@:-]{1,160}\Z")
MAX_FILE, MAX_TOTAL, MAX_COUNT, MAX_DEPTH = 32 * 1024 * 1024, 128 * 1024 * 1024, 64, 8


class Refused(Exception):
    pass


def require(value, reason):
    if not value:
        raise Refused(reason)


def identity(value):
    return (value.st_dev, value.st_ino, value.st_size, value.st_uid, value.st_gid,
            value.st_mode, value.st_nlink, value.st_mtime_ns, value.st_ctime_ns)


def bounded_file(path, limit, expected=None):
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC)
    try:
        before = os.fstat(fd)
        require(stat.S_ISREG(before.st_mode) and before.st_size <= limit, "bounded_file_shape")
        require(expected is None or identity(before) == identity(expected), "bounded_file_replaced")
        with os.fdopen(os.dup(fd), "rb") as stream:
            value = stream.read(limit + 1)
        require(len(value) <= limit and identity(before) == identity(os.fstat(fd)), "bounded_file_changed")
        require(expected is None or identity(before) == identity(os.lstat(path)), "bounded_path_replaced")
        return value
    finally:
        os.close(fd)


def load_containment():
    path = Path(__file__).with_name("containment.py")
    if not path.exists():
        path = Path(__file__).parent.parent / "real_resolved_binary/probe.py"
    source = bounded_file(path, 131072)
    require(hashlib.sha256(source).hexdigest() == CONTAINMENT_SHA, "containment_pin")
    module = types.ModuleType("static_closure_containment")
    module.__file__ = str(path)
    exec(compile(source, str(path), "exec"), module.__dict__)
    return module


base = load_containment()


def public_path(path):
    return (path in ("/lib64", "/usr/lib64") or
            any(path.startswith(root + "/") and BASENAME.fullmatch(path[len(root) + 1:])
                for root in ("/usr/bin", "/usr/lib", "/usr/lib/systemd", "/lib64", "/usr/lib64")))


def canonical_public(path):
    require(public_path(path), "nonpublic_candidate_path")
    links, hops = [], 0
    while True:
        parts = Path(path).parts[1:]
        changed = False
        for index in range(len(parts)):
            current = "/" + "/".join(parts[:index + 1])
            value = os.lstat(current)
            require(value.st_uid == value.st_gid == 0 and value.st_mode & 0o022 == 0
                    if not stat.S_ISLNK(value.st_mode) else value.st_uid == value.st_gid == 0,
                    "public_path_owner_mode")
            if stat.S_ISLNK(value.st_mode):
                hops += 1
                require(hops <= 8, "public_symlink_depth")
                target = os.readlink(current)
                require(re.fullmatch(r"[A-Za-z0-9_./+-]{1,256}", target), "public_symlink_target")
                replacement = posixpath.normpath(posixpath.join(posixpath.dirname(current), target))
                tail = parts[index + 1:]
                replacement = posixpath.join(replacement, *tail) if tail else replacement
                require(public_path(replacement), "nonpublic_symlink_target")
                require(identity(value) == identity(os.lstat(current)), "public_symlink_changed")
                links.append({"path": current, "target": target, "identity": list(identity(value))})
                path, changed = replacement, True
                break
            if index + 1 < len(parts):
                require(stat.S_ISDIR(value.st_mode), "public_parent_shape")
            else:
                require(stat.S_ISREG(value.st_mode), "public_elf_shape")
        if not changed:
            require(path.startswith(("/usr/lib/", "/usr/bin/")), "canonical_public_root")
            return path, links


def digest_fd(fd, deadline):
    before = os.fstat(fd)
    require(stat.S_ISREG(before.st_mode) and before.st_uid == before.st_gid == 0
            and before.st_mode & 0o022 == 0 and before.st_nlink > 0
            and 0 < before.st_size <= MAX_FILE, "public_elf_metadata")
    os.lseek(fd, 0, os.SEEK_SET)
    digest, size = hashlib.sha256(), 0
    while True:
        require(time.monotonic() < deadline, "static_capture_deadline")
        block = os.read(fd, 65536)
        if not block:
            break
        if size == 0:
            require(block.startswith(b"\x7fELF"), "public_not_elf")
        size += len(block)
        require(size <= before.st_size, "public_elf_read_bound")
        digest.update(block)
    require(size == before.st_size and identity(before) == identity(os.fstat(fd)), "public_elf_changed")
    return before, digest.hexdigest()


def package_bytes(path):
    parent = path.parent.lstat()
    for item in (path.parent, path):
        value = item.lstat()
        require(value.st_uid == value.st_gid == 0 and value.st_mode & 0o022 == 0
                and not stat.S_ISLNK(value.st_mode), "package_database_shape")
    data = bounded_file(path, 2 * 1024 * 1024, expected=value)
    require(identity(parent) == identity(path.parent.lstat()), "package_directory_changed")
    return data


def package_index(deadline):
    root = Path("/var/lib/pacman/local")
    require(root.is_dir() and not root.is_symlink() and root.stat().st_uid == 0,
            "package_database_root")
    names = sorted(os.listdir(root))
    require(len(names) <= 4096, "package_count_bound")
    owners, databases, total = {}, {}, 0
    for name in names:
        require(time.monotonic() < deadline, "static_capture_deadline")
        if name == "ALPM_DB_VERSION":
            continue
        require(PACKAGE.fullmatch(name), "package_directory_name")
        folder = root / name
        raw = package_bytes(folder / "files")
        total += len(raw)
        require(total <= 64 * 1024 * 1024, "package_database_bound")
        text = raw.decode("utf-8", "strict")
        # ALPM permits an empty file list. Only literal newline-separated
        # empty rows qualify: no whitespace/CR/BOM stripping or owner inference.
        if all(row == "" for row in text.split("\n")):
            databases[name] = hashlib.sha256(raw).hexdigest()
            continue
        rows = text.splitlines()
        require(rows.count("%FILES%") == 1, "package_file_list_shape")
        active = False
        for row in rows:
            if row.startswith("%"):
                active = row == "%FILES%"
                continue
            path = "/" + row
            if active and public_path(path):
                owners.setdefault(path, []).append(name)
        databases[name] = hashlib.sha256(raw).hexdigest()
    require(len(owners) <= 262144, "package_path_count_bound")
    return root, owners, databases


def owner_record(path, index):
    root, owners, databases = index
    names = owners.get(path, [])
    require(len(names) == 1, "package_owner_not_unique")
    name = names[0]
    raw = package_bytes(root / name / "desc")
    rows = raw.decode("utf-8", "strict").splitlines()
    value = {}
    for key in ("NAME", "VERSION"):
        marker = "%" + key + "%"
        require(rows.count(marker) == 1, "package_description_shape")
        position = rows.index(marker) + 1
        require(position < len(rows) and PACKAGE.fullmatch(rows[position]), "package_description_value")
        value[key.lower()] = rows[position]
    value.update({"file_list_sha256": databases[name], "description_sha256": hashlib.sha256(raw).hexdigest()})
    return value, name


def decode_readelf(raw):
    require(len(raw) <= 131072, "readelf_output_bound")
    text = raw.decode("ascii", "strict")
    require(not re.search(r"\((?:FILTER|AUXILIARY|AUDIT|DEPAUDIT)\)", text), "unsupported_dynamic_loader_tag")
    needed = []
    for line in text.splitlines():
        if "(NEEDED)" in line:
            match = re.fullmatch(r"\s*0x[0-9a-f]+\s+\(NEEDED\)\s+Shared library: \[([^\]]+)\]\s*", line)
            require(match is not None and BASENAME.fullmatch(match[1])
                    and match[1] not in (".", ".."), "needed_basename_shape")
            needed.append(match[1])
    require(len(needed) <= 64 and len(set(needed)) == len(needed), "needed_count_or_duplicate")
    interpreters = re.findall(r"\[Requesting program interpreter: ([^\]]+)\]", text)
    require(len(interpreters) <= 1 and all(path in ("/lib64/ld-linux-x86-64.so.2", "/usr/lib/ld-linux-x86-64.so.2")
                                         for path in interpreters), "interpreter_not_fixed")
    search_tokens = []
    for line in text.splitlines():
        if "(RPATH)" in line or "(RUNPATH)" in line:
            match = re.fullmatch(r"\s*0x[0-9a-f]+\s+\((?:RPATH|RUNPATH)\)\s+Library (?:rpath|runpath): \[([^\]]*)\]\s*", line)
            require(match is not None, "search_path_shape")
            tokens = match[1].split(":")
            require(all(token in ("/usr/lib", "/usr/lib/systemd", "$ORIGIN", "${ORIGIN}") for token in tokens),
                    "search_path_not_fixed")
            search_tokens.extend(tokens)
    require("Dynamic section at offset" in text or "There is no dynamic section in this file." in text,
            "readelf_dynamic_missing")
    return {"needed": needed, "interpreter": interpreters[0] if interpreters else None,
            "declared_search_tokens": search_tokens}


def resolve_needed(name):
    require(BASENAME.fullmatch(name) and name not in (".", ".."), "needed_basename_shape")
    matches = []
    for root in SEARCH:
        path = root + "/" + name
        try:
            os.lstat(path)
        except FileNotFoundError:
            continue
        matches.append((path, *canonical_public(path)))
    require(len(matches) == 1, "needed_missing_or_ambiguous")
    return matches[0]


def capture():
    receipt = {"schema": "public-static-elf-provenance-v1", "outcome": "NONPASS",
               "candidate_elf_executed": False, "allowlist_adoption": False,
               "loaded_elf_identity_proven": False, "compatibility_acceptance": False,
               "search_policy": list(SEARCH), "records": []}
    fds, snapshots, package_snapshots = [], [], {}
    try:
        require(os.getuid() == os.geteuid() == 1000, "capture_uid")
        deadline = time.monotonic() + 60
        manifest_path = Path(__file__).with_name("guest-inventory.json")
        manifest_raw = bounded_file(manifest_path, 32768)
        require(hashlib.sha256(manifest_raw).hexdigest() == INVENTORY_SHA, "original_manifest_pin")
        inventory = json.loads(manifest_raw)
        require(len(inventory["elfs"]) == 16, "original_manifest_count")
        index = package_index(deadline)
        tool_path, tool_links = canonical_public(READELF)
        require(tool_path == READELF and not tool_links, "readelf_symlink")
        tool_fd = os.open(READELF, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC)
        fds.append(tool_fd)
        tool_stat, tool_hash = digest_fd(tool_fd, deadline)
        require(tool_hash == READELF_SHA and (tool_stat.st_dev, tool_stat.st_ino, tool_stat.st_size,
                tool_stat.st_mode, tool_stat.st_nlink) == (31, 29149, 810072, 0o100755, 1), "readelf_pin")
        try:
            os.getxattr(tool_fd, "security.capability")
        except OSError as error:
            require(error.errno == errno.ENODATA, "readelf_capability_unknown")
        else:
            raise Refused("readelf_capabilities_present")
        tool_owner, package = owner_record(READELF, index)
        require((tool_owner["name"], tool_owner["version"]) == ("binutils", "2.47-4"), "readelf_package_pin")
        package_snapshots[package] = tool_owner
        receipt["readelf"] = {"path": READELF, "sha256": tool_hash, "package": tool_owner}
        queue = [(path, 0, "original_manifest", None) for path in inventory["elfs"]] + [(CANDIDATE, 0, "explicit_observation_candidate", None)]
        known = {row["resolved_path"]: row["sha256"] for row in inventory["elfs"].values()}
        known_modes = {row["resolved_path"]: int(row["mode"], 8) for row in inventory["elfs"].values()}
        seen, total, steps, aliases = set(), 0, 0, {}
        while queue:
            require(time.monotonic() < deadline and not base.UNSETTLED, "capture_deadline_or_child_unknown")
            path, depth, source, expected_edge = queue.pop(0)
            steps += 1
            require(steps <= 512 and len(queue) <= 512, "static_edge_bound")
            require(depth <= MAX_DEPTH, "static_depth_bound")
            canonical, links = canonical_public(path)
            require(expected_edge is None or (canonical, links) == expected_edge,
                    "queued_dependency_edge_changed")
            if path in inventory["elfs"]:
                require(canonical == inventory["elfs"][path]["resolved_path"], "original_alias_changed")
            require(path not in aliases or aliases[path] == (canonical, links), "candidate_alias_changed")
            aliases[path] = (canonical, links)
            if canonical in seen:
                continue
            require(len(seen) < MAX_COUNT, "static_object_count")
            fd = os.open(canonical, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC)
            fds.append(fd)
            before, digest = digest_fd(fd, deadline)
            total += before.st_size
            require(total <= MAX_TOTAL, "static_total_bound")
            require(canonical not in known or digest == known[canonical], "original_elf_hash_changed")
            require(canonical not in known_modes or stat.S_IMODE(before.st_mode) == known_modes[canonical],
                    "original_elf_mode_changed")
            owner, package = owner_record(canonical, index)
            package_snapshots[package] = owner
            command = base.command([f"/proc/self/fd/{tool_fd}", "--wide", "--dynamic", "--program-headers",
                                    f"/proc/self/fd/{fd}"], pass_fds=(tool_fd, fd),
                                   env={"PATH": "/usr/bin", "LANG": "C", "LC_ALL": "C"})
            require(command.stderr == b"", "readelf_diagnostic_unknown")
            dynamic = decode_readelf(command.stdout)
            require(identity(before) == identity(os.fstat(fd)) == identity(os.lstat(canonical))
                    and canonical_public(path) == (canonical, links), "elf_or_links_changed")
            dependencies = []
            for name in dynamic["needed"]:
                logical, resolved, chain = resolve_needed(name)
                dependencies.append({"name": name, "logical_path": logical, "resolved_path": resolved, "links": chain})
                queue.append((logical, depth + 1, "static_needed_candidate", (resolved, chain)))
            if dynamic["interpreter"]:
                interpreter_edge = canonical_public(dynamic["interpreter"])
                queue.append((dynamic["interpreter"], depth + 1, "fixed_interpreter_candidate", interpreter_edge))
            receipt["records"].append({"path": path, "resolved_path": canonical, "links": links,
                "device": before.st_dev, "inode": before.st_ino, "uid": before.st_uid, "gid": before.st_gid,
                "mode": before.st_mode, "nlink": before.st_nlink, "size": before.st_size, "sha256": digest,
                "package": owner, "source": source, "depth": depth, "dynamic": dynamic,
                "dependencies": dependencies, "matches_original_manifest": known.get(canonical) == digest})
            snapshots.append((fd, path, canonical, links, before, digest))
            seen.add(canonical)
        for fd, path, canonical, links, before, digest in snapshots:
            require(digest_fd(fd, deadline)[1] == digest and identity(os.lstat(canonical)) == identity(before)
                    and canonical_public(path) == (canonical, links), "final_candidate_changed")
        for package, owner in package_snapshots.items():
            require(time.monotonic() < deadline, "static_capture_deadline")
            folder = index[0] / package
            require(hashlib.sha256(package_bytes(folder / "files")).hexdigest() == owner["file_list_sha256"]
                    and hashlib.sha256(package_bytes(folder / "desc")).hexdigest() == owner["description_sha256"],
                    "package_provenance_changed")
        for path, expected in aliases.items():
            require(time.monotonic() < deadline and canonical_public(path) == expected, "final_alias_changed")
        for record in receipt["records"]:
            for edge in record["dependencies"]:
                require(aliases.get(edge["logical_path"]) == (edge["resolved_path"], edge["links"]),
                        "final_dependency_edge_changed")
        receipt["aliases"] = [{"path": path, "resolved_path": value[0], "links": value[1]}
                              for path, value in sorted(aliases.items())]
        require(digest_fd(tool_fd, deadline)[1] == READELF_SHA and identity(os.lstat(READELF)) == identity(tool_stat),
                "final_readelf_changed")
        receipt["outcome"] = "OBSERVED_STATIC_CANDIDATE_CLOSURE"
    except Exception as error:
        receipt["reason"] = str(error) if isinstance(error, (Refused, base.Refused)) else type(error).__name__
    finally:
        for fd in reversed(fds):
            try:
                os.close(fd)
            except OSError:
                receipt["outcome"] = "NONPASS"
                receipt["cleanup_reason"] = "candidate_fd_close_unknown"
    return receipt


if __name__ == "__main__":
    require(sys.argv[1:] == ["--capture-static-public-provenance"], "explicit_capture_opt_in")
    os.umask(0o077)
    base.limits()
    result = capture()
    print(json.dumps(result, sort_keys=True))
    sys.exit(0 if result["outcome"] == "OBSERVED_STATIC_CANDIDATE_CLOSURE" else 1)
