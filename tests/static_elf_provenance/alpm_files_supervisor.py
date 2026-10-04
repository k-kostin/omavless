#!/usr/bin/env python3
"""Fixed metadata child, using the frozen unknown-terminal owned supervisor."""
import hashlib
import json
import os
from pathlib import Path
import stat
import sys
import types

CONTAINMENT_SHA = "2b9980266bd467c0684ee489167aadb4b53495340d26c6389aed723d67736592"
DIAGNOSTIC_SHA = "5c696390ccdfdc536a77650cb0d85d5b7f4b817bc7b84eedba22166e914752d5"
STAGE = Path("/home/kdk_vm/.cache/t3-alpm-files-retained-fd-review-1")


def identity(value):
    return (value.st_dev, value.st_ino, value.st_size, value.st_uid, value.st_gid,
            value.st_mode, value.st_nlink, value.st_mtime_ns, value.st_ctime_ns)


def load_containment():
    path = Path(__file__).with_name("containment.py")
    if not path.exists():
        path = Path(__file__).parent.parent / "real_resolved_binary/probe.py"
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
    module = types.ModuleType("alpm_metadata_containment")
    module.__file__ = str(path)
    exec(compile(source, str(path), "exec"), module.__dict__)
    return module


base = load_containment()


def run_child(stage):
    result = {"schema": "alpm-metadata-owned-child-v1", "outcome": "NONPASS"}
    try:
        base.require(stage == STAGE and os.getuid() == os.geteuid() == 1000, "fixed_stage_uid")
        base.private_parent(stage)
        base.require(not base.UNSETTLED, "owned_state_quarantined")
        base.object_bytes(stage / "probe.py", DIAGNOSTIC_SHA, 32768)
        with (stage / "result.json").open("xb") as output, (stage / "private-stderr.log").open("xb") as errors:
            child = base.OwnedProcess(
                ["/usr/bin/python3", str(stage / "probe.py"), "--capture-alpm-shape"],
                stdin=base.subprocess.DEVNULL, stdout=output, stderr=errors,
                env={"HOME": "/home/kdk_vm", "PATH": "/usr/bin", "LANG": "C"},
                start_new_session=True, preexec_fn=base.limits)
            # Exactly one supervisor call. Unknown is terminal: no fallback,
            # poll, wait, signal, reap, or synthesized returncode here.
            completed = base.supervise(child, 15)
            base.require(completed and child.returncode is not None, "metadata_not_completed")
            result["returncode"] = child.returncode
            base.require(child.returncode == 0, "metadata_child_nonpass")
            result["outcome"] = "KNOWN_COMPLETED"
    except BaseException as error:
        result["reason"] = str(error) if isinstance(error, base.Refused) else type(error).__name__
    return result


if __name__ == "__main__":
    base.require(sys.argv[1:] == ["--run-metadata-child"], "explicit_metadata_supervision")
    os.umask(0o077)
    receipt = run_child(STAGE)
    with (STAGE / "supervisor-receipt.json").open("x") as stream:
        json.dump(receipt, stream, sort_keys=True)
        stream.write("\n")
    sys.exit(0 if receipt["outcome"] == "KNOWN_COMPLETED" else 1)
