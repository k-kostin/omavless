#!/usr/bin/env python3
"""Opt-in private bus/resolved loader inventory; no broker/core or DNS mutation."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import stat
import subprocess
import sys
import time
import types

CONTAINMENT_SHA = "2b9980266bd467c0684ee489167aadb4b53495340d26c6389aed723d67736592"
MAX_ELF = 128 * 1024 * 1024
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


def measure_object(path, identity, deadline):
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC)
    try:
        before = os.fstat(fd)
        base.require(stat.S_ISREG(before.st_mode) and before.st_uid == 65534
                     and before.st_mode & 0o022 == 0 and 0 < before.st_size <= MAX_ELF
                     and (before.st_dev, before.st_ino) == identity,
                     "mapped_object_identity")
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
        Path("/tmp/dbus.xml").write_text(base.bus_config("success"))
        for name, argv, uid, caps in (
            ("bus", ["/usr/bin/dbus-daemon", "--nofork", "--nopidfile", "--config-file=/tmp/dbus.xml"], None, None),
            ("resolved", base.resolved_exec(), 974, base.RESOLVER_CAPS),
        ):
            log = open("/tmp/" + name + ".log", "xb")
            logs.append(log)
            child = base.OwnedProcess(argv, env=base.ENV, stdin=subprocess.DEVNULL,
                                     stdout=log, stderr=log)
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
        receipt["outcome"] = "OBSERVED_INVENTORY_ONLY"
    except Exception as error:
        receipt["reason"] = str(error) if isinstance(error, base.Refused) else type(error).__name__
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


def main():
    os.umask(0o077)
    if len(sys.argv) == 5 and sys.argv[1] == "--isolated-child":
        original, root, inputs = sys.argv[2:]
        inventory = base.decode(base.object_bytes(Path(inputs) / "guest-inventory.json", base.INVENTORY, 32768))
        base.verify_system_inputs(inventory, namespaced=True)
        base.isolate(base.decode(original), Path(root), Path(inputs))
        print(json.dumps(observe(inventory), sort_keys=True))
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
    root = args.scratch / "loader-inventory"
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
            stdout=output, stderr=error, start_new_session=True, preexec_fn=base.limits)
        completed = base.supervise(child, 35)
    raw = (root / "child.stdout").read_bytes()
    base.require(base.namespace("net") == original["net"], "parent_namespace_changed")
    if completed and child.returncode == 0 and len(raw) <= 131072:
        receipt = base.decode(raw)
    else:
        receipt = {"outcome": "NONPASS", "reason": "inventory_child_refused"}
    result = {"schema": "private-resolved-loader-inventory-v1", "source_sha256": args.source_sha,
              "containment_sha256": CONTAINMENT_SHA, "inventory_sha256": base.INVENTORY,
              "receipt": receipt, "compatibility_acceptance": False, "allowlist_adoption": False}
    with open(root / "result.json", "x") as output:
        json.dump(result, output, sort_keys=True, indent=2)
    print("private_loader_inventory: " + receipt["outcome"] + "; no compatibility or allowlist adoption")
    base.require(receipt["outcome"] == "OBSERVED_INVENTORY_ONLY", "inventory_nonpass")


if __name__ == "__main__":
    main()
