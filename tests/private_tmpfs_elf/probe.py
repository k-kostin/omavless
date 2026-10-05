#!/usr/bin/env python3
"""Opt-in pinned private tmpfs ELF copies; private bus/resolved inventory only."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import resource
import stat
import subprocess
import sys
import time
import types

CONTAINMENT_SHA = "2b9980266bd467c0684ee489167aadb4b53495340d26c6389aed723d67736592"
MAX_ELF = 32 * 1024 * 1024
COPY_RECEIPTS = {}
COPY_FDS = {}
STORE = Path("/elf-copy-store")
MAX_TOTAL = 128 * 1024 * 1024
PUBLIC_PATH = re.compile(r"/usr/(?:lib|bin)/[A-Za-z0-9_./+:-]+\Z")


def load_containment():
    path = Path(__file__).with_name("containment.py")
    if not path.exists():
        path = Path(__file__).parent.parent / "real_resolved_binary/probe.py"
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC)
    try:
        before = os.fstat(fd)
        assert stat.S_ISREG(before.st_mode) and before.st_size < 131072
        with os.fdopen(os.dup(fd), "rb") as source:
            data = source.read(131073)
        after = os.fstat(fd)
        identity = lambda value: (value.st_dev, value.st_ino, value.st_size,
                                  value.st_mtime_ns, value.st_ctime_ns, value.st_mode)
        assert identity(before) == identity(after) and hashlib.sha256(data).hexdigest() == CONTAINMENT_SHA
    finally:
        os.close(fd)
    module = types.ModuleType("frozen_resolved_containment")
    module.__file__ = str(path)
    exec(compile(data, str(path), "exec"), module.__dict__)
    return module, data


base, CONTAINMENT_BYTES = load_containment()

EXPECTED_ELFS = {
    "/lib64/ld-linux-x86-64.so.2": {
        "mode": "0o755",
        "resolved_path": "/usr/lib/ld-linux-x86-64.so.2",
        "sha256": "d011113b7054c641c8ca064f58bcc23804fd2654a3dff2444dad06ddeda61bfb",
        "uid": 0
    },
    "/usr/bin/dbus-daemon": {
        "mode": "0o755",
        "resolved_path": "/usr/bin/dbus-daemon",
        "sha256": "e9ffeaea83965fa53077f716e352d3038794200bacb8eee043fe55117ce2c7d7",
        "uid": 0
    },
    "/usr/bin/newgidmap": {
        "mode": "0o755",
        "resolved_path": "/usr/bin/newgidmap",
        "sha256": "326370106ae7d202a21216d75eb74dfc38ba93ac81d0fe9dc3ddd19bd68efa2f",
        "uid": 0
    },
    "/usr/bin/newuidmap": {
        "mode": "0o755",
        "resolved_path": "/usr/bin/newuidmap",
        "sha256": "85099fcbafde31fe5f27dbca529e3da1531f9b97f899db84758132313445af36",
        "uid": 0
    },
    "/usr/bin/setpriv": {
        "mode": "0o755",
        "resolved_path": "/usr/bin/setpriv",
        "sha256": "3bc16fb066583cbdce19af898ada51014de1352695f5463003b156924d4b766b",
        "uid": 0
    },
    "/usr/bin/unshare": {
        "mode": "0o755",
        "resolved_path": "/usr/bin/unshare",
        "sha256": "1fd26966b5ef37a3021e00059a00afb4ddc3fcee181ba2431001f08a372e5179",
        "uid": 0
    },
    "/usr/lib/libaudit.so.1": {
        "mode": "0o755",
        "resolved_path": "/usr/lib/libaudit.so.1.0.0",
        "sha256": "16813308197ab091526836d4d9ee79b7780a49e63a0ee1d7e01da2b662d057c3",
        "uid": 0
    },
    "/usr/lib/libc.so.6": {
        "mode": "0o755",
        "resolved_path": "/usr/lib/libc.so.6",
        "sha256": "e221b10fee9ee4776d8f0f1701253bc06817f7a4dbe6c5292487277d0bf8ffff",
        "uid": 0
    },
    "/usr/lib/libcap-ng.so.0": {
        "mode": "0o755",
        "resolved_path": "/usr/lib/libcap-ng.so.0.0.0",
        "sha256": "583c413d5a6d6dbe43fbcb7e1d41eb347fc68b0afe8d48dd4a8022af12ec32e9",
        "uid": 0
    },
    "/usr/lib/libdbus-1.so.3": {
        "mode": "0o755",
        "resolved_path": "/usr/lib/libdbus-1.so.3.38.3",
        "sha256": "61be5c82b1d494a3690ad38abd756691bc129a51ba3fb7d5b9074016861b7d60",
        "uid": 0
    },
    "/usr/lib/libexpat.so.1": {
        "mode": "0o755",
        "resolved_path": "/usr/lib/libexpat.so.1.12.4",
        "sha256": "516a880ec607ee15f664d92eeb587deeb609f6ef6d05c947e9bfeb781a9debaf",
        "uid": 0
    },
    "/usr/lib/libgcc_s.so.1": {
        "mode": "0o644",
        "resolved_path": "/usr/lib/libgcc_s.so.1",
        "sha256": "e618cb9c90c2eb3a1dad1cfea5b6c1eebcf2e799ad805976f4ae47bdd61c716a",
        "uid": 0
    },
    "/usr/lib/libsystemd.so.0": {
        "mode": "0o755",
        "resolved_path": "/usr/lib/libsystemd.so.0.44.0",
        "sha256": "2e971237c56af98844a847344edeae0f59ae2e3437de84162b37f308f82c1615",
        "uid": 0
    },
    "/usr/lib/systemd/libsystemd-shared-261.2-1.so": {
        "mode": "0o755",
        "resolved_path": "/usr/lib/systemd/libsystemd-shared-261.2-1.so",
        "sha256": "a9138b761996fb49bb45511b2a3f365e5bfac26c461d23d93788c58a5fb6d6f5",
        "uid": 0
    },
    "/usr/lib/systemd/systemd-resolved": {
        "mode": "0o755",
        "resolved_path": "/usr/lib/systemd/systemd-resolved",
        "sha256": "feb36cd417f4e6a065222be0f88ce25933244bc22a9f5dd54030f0fa6ac5cfcd",
        "uid": 0
    },
    "/usr/lib64/ld-linux-x86-64.so.2": {
        "mode": "0o755",
        "resolved_path": "/usr/lib/ld-linux-x86-64.so.2",
        "sha256": "d011113b7054c641c8ca064f58bcc23804fd2654a3dff2444dad06ddeda61bfb",
        "uid": 0
    }
}


def fixed_targets(inventory):
    base.require(inventory["elfs"] == EXPECTED_ELFS and len(EXPECTED_ELFS) == 16,
                 "copy_manifest_changed")
    targets = {}
    for row in EXPECTED_ELFS.values():
        path = row["resolved_path"]
        base.require(path not in targets or targets[path] == row, "copy_alias_conflict")
        targets[path] = row
    base.require(len(targets) == 15, "copy_target_count")
    return targets


def mount_policy(path, *, readonly):
    text = Path("/proc/self/mountinfo").read_text()
    base.require(len(text) <= 1024 * 1024, "copy_mount_bound")
    rows = [line.split() for line in text.splitlines() if len(line.split()) >= 10
            and line.split()[4] == str(path)]
    base.require(len(rows) == 1, "copy_mount_not_unique")
    row = rows[0]
    separator = row.index("-")
    base.require(separator >= 6 and len(row) == separator + 4
                 and row[separator + 1] == "tmpfs", "copy_mount_filesystem")
    flags = set(row[5].split(","))
    super_flags = set(row[separator + 3].split(","))
    base.require({"nosuid", "nodev"} <= flags and "noexec" not in flags
                 and ("ro" if readonly else "rw") in flags
                 and ("ro" if readonly else "rw") in super_flags,
                 "copy_mount_policy")


def checked_digest(fd, deadline):
    before = os.fstat(fd)
    base.require(stat.S_ISREG(before.st_mode) and 0 < before.st_size <= MAX_ELF,
                 "copy_file_bound")
    os.lseek(fd, 0, os.SEEK_SET)
    digest, size = hashlib.sha256(), 0
    while True:
        base.require(time.monotonic() < deadline, "copy_deadline")
        chunk = os.read(fd, 65536)
        if not chunk:
            break
        if size == 0:
            base.require(chunk.startswith(b"\x7fELF"), "copy_not_elf")
        size += len(chunk)
        base.require(size <= before.st_size, "copy_read_bound")
        digest.update(chunk)
    base.require(size == before.st_size and stable_identity(before) == stable_identity(os.fstat(fd)),
                 "copy_fd_changed")
    return digest.hexdigest()


def copy_one(path, expected, destination, deadline, remaining):
    flags = os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC
    source_fd = os.open(path, flags)
    try:
        original = os.fstat(source_fd)
        base.require(stat.S_ISREG(original.st_mode) and original.st_uid == original.st_gid == 65534
                     and stat.S_IMODE(original.st_mode) == int(expected["mode"], 8)
                     and original.st_nlink > 0 and 0 < original.st_size <= min(MAX_ELF, remaining),
                     "installed_source_shape")
        digest = checked_digest(source_fd, deadline)
        base.require(digest == expected["sha256"], "installed_source_hash")
        output_fd = os.open(destination, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW
                            | os.O_CLOEXEC, 0o600)
        try:
            os.lseek(source_fd, 0, os.SEEK_SET)
            copied = 0
            while copied < original.st_size:
                base.require(time.monotonic() < deadline, "copy_deadline")
                chunk = os.read(source_fd, min(65536, original.st_size - copied))
                base.require(chunk and os.write(output_fd, chunk) == len(chunk), "copy_short_io")
                copied += len(chunk)
            base.require(os.read(source_fd, 1) == b"", "copy_source_grew")
            os.fchmod(output_fd, int(expected["mode"], 8))
            target = os.fstat(output_fd)
            base.require(target.st_uid == target.st_gid == 0 and target.st_nlink == 1
                         and target.st_size == original.st_size, "copy_destination_shape")
        finally:
            os.close(output_fd)
        base.require(stable_identity(original) == stable_identity(os.fstat(source_fd))
                     == stable_identity(os.stat(path, follow_symlinks=False)), "installed_source_replaced")
        verify_fd = os.open(destination, flags)
        try:
            base.require(checked_digest(verify_fd, deadline) == digest, "copy_destination_hash")
            copied_stat = os.fstat(verify_fd)
            base.require(stable_identity(target) == stable_identity(copied_stat), "copy_destination_replaced")
        finally:
            os.close(verify_fd)
        return {"source_device": original.st_dev, "source_inode": original.st_ino,
                "device": target.st_dev, "inode": target.st_ino, "size": target.st_size,
                "mode": target.st_mode, "uid": 0, "gid": 0, "sha256": digest}
    finally:
        os.close(source_fd)


def no_writable_copy_fds(device):
    for name in os.listdir("/proc/self/fd"):
        fd = int(name)
        try:
            value = os.fstat(fd)
        except OSError as error:
            # The listdir directory FD is already closed; no concurrent threads exist.
            base.require(error.errno == 9, "copy_fd_inventory_unknown")
            continue
        if value.st_dev == device:
            info = Path(f"/proc/self/fdinfo/{fd}").read_text()
            base.require(len(info) <= 4096, "copy_fdinfo_bound")
            flags = re.findall(r"^flags:\s*([0-7]+)$", info, re.MULTILINE)
            base.require(len(flags) == 1 and int(flags[0], 8) & os.O_ACCMODE == os.O_RDONLY,
                         "writable_copy_fd")


def verify_target(path, deadline):
    record = COPY_RECEIPTS[path]
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC)
    try:
        value = os.fstat(fd)
        retained = os.fstat(COPY_FDS[path])
        base.require(stable_identity(value) == stable_identity(retained)
                     and (value.st_dev, value.st_ino, value.st_size, value.st_mode, value.st_uid, value.st_gid)
                     == tuple(record[key] for key in ("device", "inode", "size", "mode", "uid", "gid"))
                     and value.st_nlink == 1, "copy_target_identity")
        base.require(os.fstatvfs(fd).f_flag & os.ST_RDONLY, "copy_target_writable")
        mount_policy(path, readonly=True)
        base.require(checked_digest(fd, deadline) == record["sha256"], "copy_target_hash")
    finally:
        os.close(fd)


def verify_copies(deadline):
    base.require(len(COPY_RECEIPTS) == len(COPY_FDS) == 15, "copy_receipt_count")
    mount_policy(STORE, readonly=True)
    base.require(os.statvfs(STORE).f_flag & os.ST_RDONLY, "copy_store_writable")
    no_writable_copy_fds(os.stat(STORE).st_dev)
    for path in sorted(COPY_RECEIPTS):
        verify_target(path, deadline)


def prepare_copies(inventory, original):
    targets = fixed_targets(inventory)
    base.require(not COPY_RECEIPTS and not COPY_FDS and os.geteuid() == os.getegid() == 0,
                 "copy_single_private_root")
    base.require(set(original) == set(base.NS) and all(base.namespace(name) != original[name] for name in base.NS),
                 "copy_namespace_boundary")
    base.validate_maps(Path("/proc/self/uid_map").read_text(), Path("/proc/self/gid_map").read_text(),
                       Path("/proc/self/setgroups").read_text())
    base.require(os.readlink("/proc/self") == str(os.getpid()) and os.stat("/").st_uid == 0,
                 "copy_private_proc_root")
    mount_policy("/", readonly=False)
    STORE.mkdir(mode=0o755)
    base.command(["/usr/bin/mount", "-t", "tmpfs", "-o", "size=128m,mode=0755,nosuid,nodev", "tmpfs", str(STORE)])
    mount_policy(STORE, readonly=False)
    deadline, total, staged = time.monotonic() + 15, 0, {}
    for index, (path, expected) in enumerate(sorted(targets.items())):
        base.require(str(Path(path).resolve(strict=True)) == path, "copy_source_symlink")
        destination = STORE / str(index)
        record = copy_one(path, expected, destination, deadline, MAX_TOTAL - total)
        total += record["size"]
        staged[path] = (destination, record)
    # All source/copy hashes are checked before the FIRST overlay bind or daemon.
    base.require(len(staged) == 15 and total <= MAX_TOTAL, "copy_all_before_bind")
    no_writable_copy_fds(os.stat(STORE).st_dev)
    base.command(["/usr/bin/mount", "-o", "remount,ro,nosuid,nodev", str(STORE)])
    mount_policy(STORE, readonly=True)
    base.require(os.statvfs(STORE).f_flag & os.ST_RDONLY, "copy_store_not_frozen")
    for path, (destination, record) in staged.items():
        fd = os.open(destination, os.O_RDONLY | os.O_NOFOLLOW | os.O_CLOEXEC)
        COPY_FDS[path] = fd
        base.require((os.fstat(fd).st_dev, os.fstat(fd).st_ino) == (record["device"], record["inode"])
                     and checked_digest(fd, deadline) == record["sha256"], "frozen_copy_changed")
        COPY_RECEIPTS[path] = record
        base.command(["/usr/bin/mount", "--bind", str(destination), path])
        base.command(["/usr/bin/mount", "-o", "remount,bind,ro,nosuid,nodev", path])
        verify_target(path, deadline)
    for alias, expected in EXPECTED_ELFS.items():
        base.require(str(Path(alias).resolve(strict=True)) == expected["resolved_path"], "copy_alias_drift")
    verify_copies(deadline)


def map_objects(text):
    base.require(len(text) <= 1024 * 1024, "mapping_bound")
    objects = {}
    for line in text.splitlines():
        row = line.split(maxsplit=5)
        base.require(len(row) >= 5, "mapping_shape")
        if len(row) == 5 or row[5].startswith("["):
            continue
        path = row[5]
        base.require(PUBLIC_PATH.fullmatch(path) is not None and ".." not in Path(path).parts,
                     "nonpublic_or_deleted_mapping")
        try:
            major, minor = (int(part, 16) for part in row[3].split(":"))
            identity = (os.makedev(major, minor), int(row[4]))
        except (ValueError, OverflowError):
            raise base.Refused("mapping_identity_shape") from None
        base.require(identity[1] > 0 and (path not in objects or objects[path] == identity),
                     "mapping_identity_conflict")
        objects[path] = identity
        base.require(len(objects) <= 64, "mapping_object_bound")
    base.require(objects, "mapping_empty")
    return objects


def stable_identity(value):
    return (value.st_dev, value.st_ino, value.st_size, value.st_mode, value.st_uid,
            value.st_gid, value.st_mtime_ns, value.st_ctime_ns, value.st_nlink)


class ObjectIdentityRefused(base.Refused):
    """Public original-open-FD metadata only; never a loaded-object measurement."""

    def __init__(self, diagnostic):
        super().__init__("mapped_object_identity")
        self.diagnostic = diagnostic


def measure_object(path, identity, deadline):
    base.require(isinstance(path, str) and len(path) <= 4096
                 and PUBLIC_PATH.fullmatch(path) is not None and ".." not in Path(path).parts,
                 "nonpublic_or_deleted_mapping")
    base.require(path in COPY_RECEIPTS, "uncopied_loaded_object")
    base.require(identity == (COPY_RECEIPTS[path]["device"], COPY_RECEIPTS[path]["inode"]),
                 "mapped_copy_identity")
    verify_target(path, deadline)
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC)
    try:
        before = os.fstat(fd)
        predicates = {"regular_file": stat.S_ISREG(before.st_mode),
                      "namespace_copy_uid_gid": before.st_uid == before.st_gid == 0,
                      "not_group_or_other_writable": before.st_mode & 0o022 == 0,
                      "positive_size": before.st_size > 0,
                      "bounded_size": before.st_size <= MAX_ELF,
                      "device_matches_maps": before.st_dev == identity[0],
                      "inode_matches_maps": before.st_ino == identity[1]}
        if not all(predicates.values()):
            raise ObjectIdentityRefused({
                "schema": "public-mapped-object-identity-refusal-v1", "path": path,
                "expected_maps": {"device": identity[0], "inode": identity[1]},
                "original_open_fd": {"device": before.st_dev, "inode": before.st_ino,
                                     "uid": before.st_uid, "gid": before.st_gid,
                                     "mode": before.st_mode, "nlink": before.st_nlink,
                                     "size": before.st_size},
                "predicates": predicates, "loaded_elf_identity_proven": False,
                "content_read": False, "allowlist_adoption": False})
        digest = hashlib.sha256()
        size = 0
        while True:
            base.require(time.monotonic() < deadline, "mapping_hash_deadline")
            block = os.read(fd, 65536)
            if not block:
                break
            if size == 0:
                base.require(block.startswith(b"\x7fELF"), "mapped_object_not_elf")
            size += len(block)
            base.require(size <= MAX_ELF, "mapping_read_bound")
            digest.update(block)
        after = os.fstat(fd)
        base.require(stable_identity(before) == stable_identity(after) and size == before.st_size,
                     "mapped_object_changed")
        return {"path": path, "device": before.st_dev, "inode": before.st_ino,
                "size": size, "sha256": digest.hexdigest()}
    finally:
        os.close(fd)


def inventory_child(child, inventory):
    base.require(base.child_status(child) is None, "mapping_owner_not_live")
    before = map_objects(Path(f"/proc/{child.pid}/maps").read_text())
    approved = {row["resolved_path"]: row["sha256"] for row in inventory["elfs"].values()}
    deadline = time.monotonic() + 3
    result = []
    for path, identity in sorted(before.items()):
        if path not in COPY_RECEIPTS:
            raise ObjectIdentityRefused({"schema": "unknown-public-mapping-v1", "path": path,
                                        "loaded_elf_identity_proven": False, "allowlist_adoption": False})
        record = measure_object(path, identity, deadline)
        base.require(path not in approved or approved[path] == record["sha256"],
                     "reviewed_loader_object_changed")
        record["matches_previously_reviewed_loader_object"] = approved.get(path) == record["sha256"]
        result.append(record)
    after = map_objects(Path(f"/proc/{child.pid}/maps").read_text())
    base.require(before == after and base.child_status(child) is None, "mapping_snapshot_changed")
    return result


def observe(inventory):
    children, logs = [], []
    receipt = {"outcome": "NONPASS", "broker_executed": False, "core_executed": False,
               "dns_mutations": False, "allowlist_adoption": False,
               "bootstrap_evidence": "exact_direct_child_only_namespace_contained"}
    try:
        verify_copies(time.monotonic() + 5)
        Path("/tmp/dbus.xml").write_text(base.bus_config("success"))
        for name, argv, uid, caps in (
            ("bus", ["/usr/bin/dbus-daemon", "--nofork", "--nopidfile", "--config-file=/tmp/dbus.xml"], None, None),
            ("resolved", base.resolved_exec(), 974, base.RESOLVER_CAPS),
        ):
            log = open("/tmp/" + name + ".log", "xb")
            logs.append(log)
            child = base.OwnedProcess(argv, env=base.ENV, stdin=subprocess.DEVNULL,
                                     stdout=log, stderr=log, preexec_fn=base.limits)
            children.append(child)
            socket = "/run/dbus/system_bus_socket" if name == "bus" else "/run/systemd/resolve/io.systemd.Resolve"
            base.wait(lambda: Path(socket).is_socket(), children)
            if uid is not None:
                base.verify_child(child, uid, caps)
        receipt["initial"] = {name: inventory_child(child, inventory)
                              for name, child in zip(("bus", "resolved"), children)}
        receipt["final"] = {name: inventory_child(child, inventory)
                            for name, child in zip(("bus", "resolved"), children)}
        base.require(receipt["initial"] == receipt["final"], "loader_inventory_changed")
        base.verify_child(children[1], 974, base.RESOLVER_CAPS)
        verify_copies(time.monotonic() + 5)
        receipt["outcome"] = "OBSERVED_INVENTORY_ONLY"
    except Exception as error:
        receipt["reason"] = str(error) if isinstance(error, base.Refused) else type(error).__name__
        if isinstance(error, ObjectIdentityRefused):
            receipt["object_identity_refusal"] = error.diagnostic
    finally:
        for child in reversed(children):
            if base.UNSETTLED:
                receipt["outcome"] = "NONPASS"
                receipt["cleanup_reason"] = "owned_state_quarantined"
                break
            try:
                base.stop(child)
            except Exception as error:
                receipt["outcome"] = "NONPASS"
                receipt["cleanup_reason"] = str(error) if isinstance(error, base.Refused) else type(error).__name__
        for log in logs:
            log.close()
        receipt["diagnostics"] = {name: Path("/tmp/" + name + ".log").read_bytes()[-8192:].decode("utf-8", "replace")
                                  for name in ("bus", "resolved") if Path("/tmp/" + name + ".log").exists()}
    return receipt


def copy_limits():
    # The original 4 MiB log limit cannot hold the pinned shared-systemd ELF.
    # Only the bounded copy supervisor gets 32 MiB; daemon children restore the
    # exact original hard limits through base.limits before exec.
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
    resource.setrlimit(resource.RLIMIT_FSIZE, (MAX_ELF, MAX_ELF))
    resource.setrlimit(resource.RLIMIT_NOFILE, (128, 128))


def main():
    os.umask(0o077)
    if len(sys.argv) == 5 and sys.argv[1] == "--isolated-child":
        original, root, inputs = sys.argv[2:]
        inventory = base.decode(base.object_bytes(Path(inputs) / "guest-inventory.json", base.INVENTORY, 32768))
        base.verify_system_inputs(inventory, namespaced=True)
        base.isolate(base.decode(original), Path(root), Path(inputs))
        receipt = {"outcome": "NONPASS", "broker_executed": False, "core_executed": False,
                   "dns_mutations": False, "allowlist_adoption": False}
        try:
            prepare_copies(inventory, base.decode(original))
            receipt = observe(inventory)
            receipt["copies"] = COPY_RECEIPTS
        except Exception as error:
            receipt["reason"] = str(error) if isinstance(error, base.Refused) else type(error).__name__
        finally:
            for fd in COPY_FDS.values():
                try:
                    os.close(fd)
                except OSError:
                    receipt["outcome"] = "NONPASS"
                    receipt["cleanup_reason"] = "copy_fd_close_unknown"
        print(json.dumps(receipt, sort_keys=True))
        return
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--run-inventory", action="store_true")
    parser.add_argument("--ack-disposable-vm", action="store_true")
    parser.add_argument("--source-sha")
    parser.add_argument("--scratch", type=Path)
    args = parser.parse_args()
    base.require(args.run_inventory and args.ack_disposable_vm and os.geteuid() == 1000,
                 "inventory_execution_opt_in")
    base.require(args.scratch is not None, "inventory_scratch_missing")
    base.private_parent(args.scratch)
    base.private_parent(Path(__file__).absolute().parent)
    source = base.object_bytes(Path(__file__), args.source_sha)
    inventory_bytes = base.object_bytes(Path(__file__).with_name("guest-inventory.json"), base.INVENTORY, 32768)
    inventory = base.decode(inventory_bytes)
    base.verify_system_inputs(inventory)
    base.verify_subordinates()
    root = args.scratch / "tmpfs-inventory"
    root.mkdir(mode=0o700)
    inputs = root / "inputs"
    inputs.mkdir(mode=0o700)
    for name, data in (("probe.py", source), ("containment.py", CONTAINMENT_BYTES),
                       ("guest-inventory.json", inventory_bytes)):
        with open(inputs / name, "xb") as output:
            output.write(data)
    mount_root = root / "root"
    mount_root.mkdir(mode=0o700)
    original = {name: base.namespace(name) for name in base.NS}
    argv = ["/usr/bin/unshare", "--user", "--map-root-user", "--map-users=974:100001:1",
            "--map-groups=974:100001:1", "--map-users=1000:100000:1", "--map-groups=1000:100000:1",
            "--net", "--mount", "--pid", "--uts", "--fork", "--kill-child=SIGKILL",
            "/usr/bin/python3", str(inputs / "probe.py"), "--isolated-child", json.dumps(original),
            str(mount_root), str(inputs)]
    with open(root / "child.stdout", "xb") as output, open(root / "child.stderr", "xb") as error:
        child = base.OwnedProcess(argv, env={"PATH": "/usr/bin", "HOME": os.environ["HOME"],
            "LANG": "C", "TMPDIR": str(args.scratch)}, stdin=subprocess.DEVNULL,
            stdout=output, stderr=error, start_new_session=True, preexec_fn=copy_limits)
        completed = base.supervise(child, 50)
    raw = (root / "child.stdout").read_bytes()
    base.require(base.namespace("net") == original["net"], "parent_namespace_changed")
    if completed and child.returncode == 0 and len(raw) <= 131072:
        receipt = base.decode(raw)
    else:
        receipt = {"outcome": "NONPASS", "reason": "inventory_child_refused"}
    result = {"schema": "private-tmpfs-resolved-inventory-v1", "source_sha256": args.source_sha,
              "containment_sha256": CONTAINMENT_SHA, "inventory_sha256": base.INVENTORY,
              "receipt": receipt, "compatibility_acceptance": False, "allowlist_adoption": False}
    with open(root / "result.json", "x") as output:
        json.dump(result, output, sort_keys=True, indent=2)
    print("private_loader_inventory: " + receipt["outcome"] + "; no compatibility or allowlist adoption")
    base.require(receipt["outcome"] == "OBSERVED_INVENTORY_ONLY", "inventory_nonpass")


if __name__ == "__main__":
    main()
