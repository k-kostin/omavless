#!/usr/bin/env python3
"""Fixed private bus/resolved inventory. No broker/core or DNS setter."""
import hashlib
import json
import os
from pathlib import Path
import resource
import stat
import subprocess
import sys
import time
import types

STAGE = Path("/home/kdk_vm/.cache/t3-reviewed-tmpfs-copy-review-1")
ROOT = STAGE / "scratch/inventory/root"
INPUTS = STAGE / "scratch/inventory/inputs"
PINS = {
    "containment.py": "2b9980266bd467c0684ee489167aadb4b53495340d26c6389aed723d67736592",
    "admission.py": "daa484837263cb13cf3dafae838681229c58cc694eb995e5014289a0eb316dc7",
    "bridge.py": "1868a0b316c2f5782c5a6fdd53c91f8f1a3c6a157da6db3b2e60666fe8cac871",
    "copy-manifest.json": "40a95c1e682f94ee379a8f1cf8c387e60cdbe08ac16516309ede5e0711a5f5fb",
    "guest-inventory.json": "4f1b92aeaff78f5f376576fc1da722cf9077735ff5c36d8674ff92581342ada4",
}


def require(value, reason):
    if not value:
        raise RuntimeError(reason)


def identity(s):
    return (s.st_dev, s.st_ino, s.st_mode, s.st_size, s.st_uid, s.st_gid,
            s.st_nlink, s.st_mtime_ns, s.st_ctime_ns)


def read_input(path, expected=None):
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC)
    try:
        before = os.fstat(fd)
        require(stat.S_ISREG(before.st_mode) and before.st_uid == before.st_gid == os.getuid()
                and before.st_nlink == 1 and stat.S_IMODE(before.st_mode) in (0o500, 0o600)
                and 0 < before.st_size <= 131072 and not os.listxattr(fd), "input_shape")
        data = b""
        while len(data) <= 131072:
            block = os.read(fd, min(65536, 131073 - len(data)))
            if not block:
                break
            data += block
        require(len(data) == before.st_size and identity(before) == identity(os.fstat(fd))
                == identity(path.lstat()), "input_changed")
        require(expected is None or hashlib.sha256(data).hexdigest() == expected, "input_pin")
        return data
    finally:
        os.close(fd)


def load_inputs(directory):
    # Fixed finite filenames/hashes only, no fallback or source-discovery branch.
    require(directory in (STAGE, INPUTS), "fixed_input_directory")
    raw = {name: read_input(directory / name, digest) for name, digest in PINS.items()}
    modules = []
    for name in ("containment.py", "admission.py", "bridge.py"):
        module = types.ModuleType("fixed_" + name[:-3])
        module.__file__ = str(directory / name)
        exec(compile(raw[name], module.__file__, "exec"), module.__dict__)
        modules.append(module)
    return (*modules, raw)


def private_inputs(directory, child):
    allowed = (0, 65534) if child else (0, 1000)
    for path in (directory, *directory.parents):
        value = path.lstat()
        require(stat.S_ISDIR(value.st_mode) and value.st_uid in allowed
                and value.st_gid in allowed and value.st_mode & 0o022 == 0, "private_input_ancestry")
    value = directory.lstat()
    require(value.st_uid == value.st_gid == os.getuid() and stat.S_IMODE(value.st_mode) == 0o700,
            "private_input_directory")


def copy_limits():
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
    resource.setrlimit(resource.RLIMIT_FSIZE, (32 * 1024 * 1024,) * 2)
    resource.setrlimit(resource.RLIMIT_NOFILE, (128, 128))


def create(path, data, mode=0o600):
    fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW | os.O_CLOEXEC, mode)
    try:
        require(os.write(fd, data) == len(data), "output_short_write")
        os.fsync(fd)
    finally:
        os.close(fd)


def observe(base, copies):
    # There is deliberately no finally-stop branch: first unknown/error never
    # authorizes another child query, signal, reap or diagnostic read.
    children, logs = [], []
    copies.verify(time.monotonic() + 5)
    create(Path("/tmp/dbus.xml"), base.bus_config("success").encode())
    for name, argv in (("bus", ["/usr/bin/dbus-daemon", "--nofork", "--nopidfile",
                              "--config-file=/tmp/dbus.xml"]),
                       ("resolved", base.resolved_exec())):
        log = open("/tmp/" + name + ".log", "xb")
        logs.append(log)
        require(not base.UNSETTLED, "owned_state_quarantined")
        child = base.OwnedProcess(argv, env=base.ENV, stdin=subprocess.DEVNULL,
                                  stdout=log, stderr=log, preexec_fn=base.limits)
        children.append(child)
        socket = "/run/dbus/system_bus_socket" if name == "bus" else "/run/systemd/resolve/io.systemd.Resolve"
        base.wait(lambda: Path(socket).is_socket(), children)
        if name == "resolved":
            base.verify_child(child, 974, base.RESOLVER_CAPS)
    initial = {name: copies.inventory(child, time.monotonic() + 5)
               for name, child in zip(("bus", "resolved"), children)}
    final = {name: copies.inventory(child, time.monotonic() + 5)
             for name, child in zip(("bus", "resolved"), children)}
    require(initial == final, "map_passes_changed")
    base.verify_child(children[1], 974, base.RESOLVER_CAPS)
    copies.verify(time.monotonic() + 5)
    stops = {}
    for name, child in reversed(list(zip(("bus", "resolved"), children))):
        base.stop(child)
        require(type(child.returncode) is int and child.returncode in (0, -15), "child_stop_status")
        stops[name] = child.returncode
    for log in logs:
        log.close()
    records = copies.records
    copies.close()
    return {"initial": initial, "final": final, "copies": records,
            "known_stop_status": stops, "live_during_both_passes": True,
            "pid_namespace_and_direct_child_bound": True}


def isolated(base, admission, bridge, raw, original):
    base.require(set(original) == set(base.NS), "original_namespace_keys")
    for name, value in original.items():
        require(type(value) is str and value.startswith(name + ":[") and value.endswith("]")
                and value[len(name) + 2:-1].isdigit(), "original_namespace_shape")
    inventory = base.decode(raw["guest-inventory.json"])
    base.verify_system_inputs(inventory, namespaced=True)
    base.isolate(original, ROOT, INPUTS)
    copies = bridge.Bridge(base, admission, raw["copy-manifest.json"])
    copies.prepare(original)
    return observe(base, copies)


def main():
    os.umask(0o077)
    is_child = sys.argv[1:2] == ["--isolated-child"]
    directory = INPUTS if is_child else STAGE
    require(Path(__file__) == directory / "probe.py", "fixed_probe_path")
    require((is_child and len(sys.argv) == 3) or sys.argv[1:] == ["--run-reviewed-inventory"],
            "fixed_probe_arguments")
    private_inputs(directory, is_child)
    base, admission, bridge, raw = load_inputs(directory)
    source = read_input(directory / "probe.py")
    source_sha = hashlib.sha256(source).hexdigest()
    if is_child:
        try:
            receipt = isolated(base, admission, bridge, raw, base.decode(sys.argv[2]))
        except bridge.UnknownMapping as error:
            # Already captured, bounded public identity only. No new read/query,
            # cleanup or admission; outer failure must not read this receipt.
            print(json.dumps({"outcome": "NONPASS", "refusal": error.diagnostic}, sort_keys=True))
            raise SystemExit(1) from None
        print(json.dumps(receipt, sort_keys=True))
        return
    require(os.getuid() == os.geteuid() == 1000, "outer_uid")
    base.private_parent(STAGE / "scratch")
    base.verify_system_inputs(base.decode(raw["guest-inventory.json"]))
    base.verify_subordinates()
    (STAGE / "scratch/inventory").mkdir(mode=0o700)
    INPUTS.mkdir(mode=0o700)
    ROOT.mkdir(mode=0o700)
    for name, data in {**raw, "probe.py": source}.items():
        create(INPUTS / name, data, 0o500 if name.endswith(".py") else 0o600)
    original = {name: base.namespace(name) for name in base.NS}
    argv = ["/usr/bin/unshare", "--user", "--map-root-user", "--map-users=974:100001:1",
            "--map-groups=974:100001:1", "--map-users=1000:100000:1", "--map-groups=1000:100000:1",
            "--net", "--mount", "--pid", "--uts", "--fork", "--kill-child=SIGKILL",
            "/usr/bin/python3", str(INPUTS / "probe.py"), "--isolated-child", json.dumps(original)]
    with (STAGE / "child.stdout").open("xb") as output, (STAGE / "child.stderr").open("xb") as error:
        child = base.OwnedProcess(argv, env={"HOME": "/home/kdk_vm", "PATH": "/usr/bin",
            "LANG": "C", "TMPDIR": str(STAGE / "scratch")}, stdin=subprocess.DEVNULL,
            stdout=output, stderr=error, start_new_session=True, preexec_fn=copy_limits)
        completed = base.supervise(child, 65)
        # Failure is terminal: no receipt read or after-state query.
        require(completed and type(child.returncode) is int and child.returncode == 0,
                "isolated_child_nonpass")
    receipt = base.decode(read_input(STAGE / "child.stdout"))
    result = {"schema": "reviewed-tmpfs-inventory-v1", "outcome": "OBSERVED_INVENTORY_ONLY",
              "source_sha256": source_sha, "pins": PINS, "receipt": receipt,
              "known_outer_returncode": 0, "broker_executed": False, "core_executed": False,
              "dns_mutations": False, "compatibility_acceptance": False}
    create(STAGE / "result.json", (json.dumps(result, sort_keys=True) + "\n").encode())


if __name__ == "__main__":
    main()
