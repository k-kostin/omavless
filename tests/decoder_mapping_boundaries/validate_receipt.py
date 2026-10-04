#!/usr/bin/env python3
"""Inert strict fixed-invocation receipt validator; no process operations."""
import hashlib
import json
import os
from pathlib import Path
import stat
import sys

STAGE = Path("/home/kdk_vm/.cache/t3-mapping-boundaries-review-1")
PROBE_SHA = "97613c8dcbc6c7ef00ac722142802128b1fc653ac64b131a22b115b83fb023b0"
PINS = {
    "lifecycle.py": "deb8836caf1f31e94ab91a3ccba7eefd219678c9343acd68803986ad46e2df3a",
    "containment.py": "2b9980266bd467c0684ee489167aadb4b53495340d26c6389aed723d67736592",
    "admission.py": "6d73e2ec10969b4f097e2ee17b661a093e99b7276c7e7b8bb98e835bb3b3f0c3",
    "bridge.py": "b18a014aebd247f7922adecf18a66adfb9ba8a79401da03bff2b8b9a681f87b5",
    "copy-manifest.json": "d6ca9cd599adb573f4419f49147677f681db08fcc54a06cb0f34ed950ce19c36",
    "guest-inventory.json": "4f1b92aeaff78f5f376576fc1da722cf9077735ff5c36d8674ff92581342ada4"
}


def require(value):
    if not value:
        raise ValueError("invalid_reviewed_inventory_receipt")


def pairs(items):
    result = {}
    for key, value in items:
        require(key not in result)
        result[key] = value
    return result


def decode(raw):
    require(type(raw) is bytes and 0 < len(raw) <= 131072)
    return json.loads(raw, object_pairs_hook=pairs,
                      parse_constant=lambda _: (_ for _ in ()).throw(ValueError("nonfinite")))


def shape(value, keys):
    require(type(value) is dict and set(value) == set(keys.split()))


def integer(value, low, high):
    require(type(value) is int and low <= value <= high)


def validate(raw, manifest_raw):
    require(hashlib.sha256(manifest_raw).hexdigest() == PINS["copy-manifest.json"])
    manifest = decode(manifest_raw)
    expected = manifest["source_provenance"]
    value = decode(raw)
    shape(value, "schema outcome source_sha256 pins receipt known_outer_returncode broker_executed core_executed dns_mutations compatibility_acceptance")
    require(value["schema"] == "decoder-mapping-boundaries-inventory-v1"
            and value["outcome"] == "OBSERVED_INVENTORY_ONLY"
            and value["source_sha256"] == PROBE_SHA and value["pins"] == PINS)
    integer(value["known_outer_returncode"], 0, 0)
    for key in ("broker_executed", "core_executed", "dns_mutations", "compatibility_acceptance"):
        require(value[key] is False)
    receipt = value["receipt"]
    shape(receipt, "initial final copies shutdown live_during_both_passes pid_namespace_and_direct_child_bound")
    require(receipt["live_during_both_passes"] is True
            and receipt["pid_namespace_and_direct_child_bound"] is True)
    shape(receipt["shutdown"], "bus resolved")
    shutdown_ids = set()
    for row in receipt["shutdown"].values():
        shape(row, "pid starttime namespaces signal signal_count exit_code state")
        integer(row["pid"], 2, 2**31-1)
        integer(row["starttime"], 1, 2**64-1)
        integer(row["signal_count"], 1, 1)
        integer(row["exit_code"], 0, 0)
        require(row["signal"] == "SIGTERM" and row["state"] == "zero-reaped")
        require(row["pid"] not in shutdown_ids)
        shutdown_ids.add(row["pid"])
        shape(row["namespaces"], "pid net")
        for identity in row["namespaces"].values():
            require(type(identity) is list and len(identity) == 2)
            for number in identity:
                integer(number, 1, 2**64-1)
    require(receipt["shutdown"]["bus"]["namespaces"] == receipt["shutdown"]["resolved"]["namespaces"])
    copies = receipt["copies"]
    require(type(copies) is dict and set(copies) == set(expected) and len(copies) == 17)
    identities = set()
    for path, row in copies.items():
        shape(row, "device inode size mode uid gid nlink sha256 source_device source_inode")
        original = expected[path]
        for field in ("device", "inode", "size", "mode", "source_device", "source_inode"):
            integer(row[field], 1, 2**64 - 1)
        integer(row["uid"], 0, 0)
        integer(row["gid"], 0, 0)
        integer(row["nlink"], 1, 1)
        require(row["source_device"] == original["device"] and row["source_inode"] == original["inode"]
                and row["device"] != row["source_device"]
                and row["size"] == original["size"] and row["mode"] == original["mode"]
                and row["sha256"] == original["sha256"])
        identity = row["device"], row["inode"]
        require(identity not in identities)
        identities.add(identity)
    require(len({d for d, _ in identities}) == 1)
    for phase in ("initial", "final"):
        shape(receipt[phase], "bus resolved")
        for name, rows in receipt[phase].items():
            require(type(rows) is list and 0 < len(rows) <= 17)
            paths = []
            for row in rows:
                shape(row, "path device inode size sha256")
                path = row["path"]
                require(type(path) is str and path in copies)
                for key in ("device", "inode", "size"):
                    integer(row[key], 1, 2**64 - 1)
                require(all(row[key] == copies[path][key] for key in ("device", "inode", "size", "sha256")))
                paths.append(path)
            require(paths == sorted(set(paths)))
            required = "/usr/bin/dbus-daemon" if name == "bus" else "/usr/lib/systemd/systemd-resolved"
            require(required in paths and "/usr/lib/libc.so.6" in paths
                    and "/usr/lib/ld-linux-x86-64.so.2" in paths)
    require(receipt["initial"] == receipt["final"])
    return value


def read_fixed(path):
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC)
    try:
        before = os.fstat(fd)
        require(stat.S_ISREG(before.st_mode) and before.st_uid == before.st_gid == 1000
                and stat.S_IMODE(before.st_mode) == 0o600 and before.st_nlink == 1
                and 0 < before.st_size <= 131072 and not os.listxattr(fd))
        with os.fdopen(os.dup(fd), "rb") as stream:
            data = stream.read(131073)
        fields = lambda s: (s.st_dev, s.st_ino, s.st_mode, s.st_uid, s.st_gid,
                            s.st_nlink, s.st_size, s.st_mtime_ns, s.st_ctime_ns)
        require(fields(before) == fields(os.fstat(fd)) == fields(path.lstat())
                and len(data) == before.st_size)
        return data
    finally:
        os.close(fd)


def main():
    require(Path(__file__) == STAGE / "validate_receipt.py"
            and sys.argv[1:] == ["--validate-fixed-result"] and os.getuid() == os.geteuid() == 1000)
    for path in (STAGE, *STAGE.parents):
        value = path.lstat()
        require(stat.S_ISDIR(value.st_mode) and value.st_uid in (0, 1000) and value.st_mode & 0o022 == 0)
    require(stat.S_IMODE(STAGE.lstat().st_mode) == 0o700 and STAGE.lstat().st_uid == 1000)
    validate(read_fixed(STAGE / "result.json"), read_fixed(STAGE / "copy-manifest.json"))
    print("MAPPING_BOUNDARIES_TYPED_INVENTORY_ONLY")


if __name__ == "__main__":
    main()
