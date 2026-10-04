#!/usr/bin/env python3
"""Fixed static child, using the frozen unknown-terminal owned supervisor."""
import hashlib
import json
import os
from pathlib import Path
import stat
import sys
import types

CONTAINMENT_SHA = "2b9980266bd467c0684ee489167aadb4b53495340d26c6389aed723d67736592"
PROBE_SHA = "cccc171213f4631f54d906652aeaf7954230949a7de40b2093c8ab587f86aa00"
INVENTORY_SHA = "4f1b92aeaff78f5f376576fc1da722cf9077735ff5c36d8674ff92581342ada4"
STAGE = Path("/home/kdk_vm/.cache/t3-static-elf-four-mib-review-1")


def identity(value):
    return (value.st_dev, value.st_ino, value.st_size, value.st_uid, value.st_gid,
            value.st_mode, value.st_nlink, value.st_mtime_ns, value.st_ctime_ns)


def load_containment(path):
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC)
    try:
        before = os.fstat(fd)
        assert stat.S_ISREG(before.st_mode) and before.st_size <= 131072
        with os.fdopen(os.dup(fd), "rb") as stream:
            source = stream.read(131073)
        assert identity(before) == identity(os.fstat(fd)) == identity(path.lstat())
        assert hashlib.sha256(source).hexdigest() == CONTAINMENT_SHA
    finally:
        os.close(fd)
    module = types.ModuleType("elf_static_containment")
    module.__file__ = str(path)
    exec(compile(source, str(path), "exec"), module.__dict__)
    return module


# Import is inert. Tests explicitly load the frozen repository helper; actual
# execution below permits only the fixed stage, never a repository fallback.
base = None


def run_child(stage):
    result = {"schema": "elf-static-owned-child-v1", "outcome": "NONPASS"}
    try:
        base.require(stage == STAGE and os.getuid() == os.geteuid() == 1000, "fixed_stage_uid")
        base.private_parent(stage)
        base.require(not base.UNSETTLED, "owned_state_quarantined")
        base.object_bytes(stage / "probe.py", PROBE_SHA, 32768)
        base.object_bytes(stage / "guest-inventory.json", INVENTORY_SHA, 32768)
        with (stage / "result.json").open("xb") as output, (stage / "private-stderr.log").open("xb") as errors:
            child = base.OwnedProcess(
                ["/usr/bin/python3", str(stage / "probe.py"), "--capture-static-public-provenance"],
                stdin=base.subprocess.DEVNULL, stdout=output, stderr=errors,
                env={"HOME": "/home/kdk_vm", "PATH": "/usr/bin", "LANG": "C"},
                start_new_session=True, preexec_fn=base.limits)
            # Exactly one supervisor call. Unknown is terminal: no fallback,
            # poll, wait, signal, reap, or synthesized returncode here.
            completed = base.supervise(child, 75)
            base.require(completed and child.returncode is not None, "static_not_completed")
            result["returncode"] = child.returncode
            base.require(child.returncode == 0, "static_child_nonpass")
            result["outcome"] = "KNOWN_COMPLETED"
    except BaseException as error:
        result["reason"] = str(error) if isinstance(error, base.Refused) else type(error).__name__
    return result


if __name__ == "__main__":
    if Path(__file__) != STAGE / "supervisor.py":
        raise RuntimeError("fixed_supervisor_path")
    base = load_containment(STAGE / "containment.py")
    base.require(sys.argv[1:] == ["--run-static-child"], "explicit_static_supervision")
    os.umask(0o077)
    receipt = run_child(STAGE)
    with (STAGE / "supervisor-receipt.json").open("x") as stream:
        json.dump(receipt, stream, sort_keys=True)
        stream.write("\n")
    sys.exit(0 if receipt["outcome"] == "KNOWN_COMPLETED" else 1)
