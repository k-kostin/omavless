#!/usr/bin/python3
# SPDX-License-Identifier: MIT
"""One reviewed root observer, one unprivileged matrix. Never installed or IPC."""
import hashlib
import json
import os
from pathlib import Path
import re
import stat
import subprocess
import sys
import types

LEGACY_SHA = "3631492302b3e87beb0b2a801ded9ee5df1235a5c125117b0f4728596067049d"
LIMIT = 8 * 1024 * 1024
ENV = {"HOME": "/home/kdk_vm", "PATH": "/usr/bin", "LC_ALL": "C", "XDG_RUNTIME_DIR": "/run/user/1000"}
CREDENTIALS = {"user": 1000, "group": 1000, "extra_groups": (), "umask": 0o077, "close_fds": True}
core = None
PHASE = "admission"
RETAINED = []


class Refused(Exception):
    pass


def require(value):
    if not value:
        raise Refused()


def available():
    if core is not None:
        core.available()


def identity(m):
    return (m.st_dev, m.st_ino, m.st_uid, m.st_gid, m.st_mode,
            m.st_nlink, m.st_size, m.st_mtime_ns, m.st_ctime_ns)


def directory_identity(m):
    return (m.st_dev, m.st_ino, m.st_uid, m.st_gid, m.st_mode)


class Directory:
    def __init__(self, path, uid):
        available()
        self.path = Path(path)
        require(self.path.is_absolute())
        for parent in (self.path, *self.path.parents):
            m = parent.lstat()
            require(stat.S_ISDIR(m.st_mode) and m.st_uid in (0, uid) and m.st_mode & 0o6022 == 0)
            try:
                (parent / ".git").lstat()
            except FileNotFoundError:
                pass
            else:
                raise Refused()
        self.fd = os.open(self.path, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC)
        self.before = os.fstat(self.fd)
        require(self.before.st_uid == self.before.st_gid == uid and stat.S_IMODE(self.before.st_mode) == 0o700)
        RETAINED.append(self)
        self.recheck()

    def recheck(self):
        available()
        require(directory_identity(self.before) == directory_identity(os.fstat(self.fd))
                == directory_identity(self.path.lstat()))


class HeldFile:
    def __init__(self, parent, name, maximum, mode, uid, expected=None):
        available()
        require(re.fullmatch(r"[a-zA-Z0-9_.-]+", name) is not None)
        self.parent, self.name, self.maximum = parent, name, maximum
        parent.recheck()
        self.fd = os.open(name, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC, dir_fd=parent.fd)
        self.before = os.fstat(self.fd)
        require(stat.S_ISREG(self.before.st_mode) and self.before.st_uid == self.before.st_gid == uid
                and stat.S_IMODE(self.before.st_mode) == mode and self.before.st_nlink == 1
                and self.before.st_size <= maximum and not os.listxattr(self.fd))
        RETAINED.append(self)
        self.sha = self.hash()
        require(expected is None or expected == self.sha)
        self.recheck()

    @property
    def path(self):
        return self.parent.path / self.name

    def hash(self):
        h = hashlib.sha256()
        offset = 0
        while offset < self.before.st_size:
            data = os.pread(self.fd, min(65536, self.before.st_size - offset), offset)
            require(data)
            h.update(data)
            offset += len(data)
        require(os.pread(self.fd, 1, offset) == b"")
        return h.hexdigest()

    def recheck(self):
        available()
        self.parent.recheck()
        named = os.stat(self.name, dir_fd=self.parent.fd, follow_symlinks=False)
        require(identity(self.before) == identity(os.fstat(self.fd)) == identity(named)
                and not os.listxattr(self.fd))
        require(self.hash() == self.sha)
        require(identity(self.before) == identity(os.fstat(self.fd))
                == identity(os.stat(self.name, dir_fd=self.parent.fd, follow_symlinks=False)))
        self.parent.recheck()

    def bytes(self):
        self.recheck()
        require(self.before.st_size <= LIMIT)
        data = os.pread(self.fd, self.before.st_size + 1, 0)
        require(len(data) == self.before.st_size)
        self.recheck()
        return data


class Evidence:
    def __init__(self, stage):
        stage.recheck()
        os.mkdir("evidence", mode=0o700, dir_fd=stage.fd)
        self.directory = Directory(stage.path / "evidence", 0)
        self.count = 0

    def create(self, name):
        available()
        require(re.fullmatch(r"[a-zA-Z0-9_.-]+", name) is not None)
        self.directory.recheck()
        fd = os.open(name, os.O_RDWR | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW | os.O_CLOEXEC,
                     0o600, dir_fd=self.directory.fd)
        m = os.fstat(fd)
        require(stat.S_ISREG(m.st_mode) and m.st_uid == m.st_gid == 0
                and stat.S_IMODE(m.st_mode) == 0o600 and m.st_nlink == 1)
        RETAINED.append(fd)
        return fd

    def write(self, name, value):
        fd = self.create(name)
        data = json.dumps(value, sort_keys=True).encode()
        require(len(data) <= LIMIT)
        offset = 0
        while offset < len(data):
            count = os.write(fd, data[offset:])
            require(count > 0)
            offset += count
        os.fsync(fd)
        os.fsync(self.directory.fd)


def allowed_commands():
    commands = [tuple(v) for v in core.NETWORK.values()]
    commands.extend([("/usr/bin/systemd-detect-virt", "--vm"), ("/usr/bin/pgrep", "-x", "mihomo"),
                     ("/usr/bin/resolvectl", "status", "--no-pager")])
    fields = [argument for field in core.FIELDS for argument in ("-p", field)]
    for unit in core.ROOT_UNITS:
        commands.append(tuple(["/usr/bin/systemctl", "--system", "--no-pager", "show", unit, *fields]))
    commands.append(tuple(["/usr/bin/systemctl", "--user", "--no-pager", "show", "omavless-runtime.service", *fields]))
    return commands


def command(evidence, args, allowed=(0,)):
    available()
    require(tuple(args) in allowed_commands())
    require(allowed == ((0, 1) if args == ["/usr/bin/pgrep", "-x", "mihomo"] else (0,)))
    evidence.count += 1
    require(evidence.count <= 64)
    out = evidence.create(f"command-{evidence.count}.stdout")
    err = evidence.create(f"command-{evidence.count}.stderr")
    child = core.spawn(args, env=dict(ENV), stdin=subprocess.DEVNULL, stdout=out, stderr=err, **CREDENTIALS)
    code = core.await_exact(child, 12)
    require(code in allowed)
    require(os.fstat(out).st_size <= LIMIT and os.fstat(err).st_size <= LIMIT)
    return code, os.pread(out, LIMIT + 1, 0)


def execute_matrix(evidence, elf, receipt):
    available()
    elf.recheck()
    out, err = evidence.create("matrix.stdout"), evidence.create("matrix.stderr")
    env = dict(ENV, OMAVLESS_ABORT_FROZEN_ELF=str(elf.path), OMAVLESS_ABORT_FROZEN_SHA256=elf.sha,
               OMAVLESS_ABORT_BUILD_TARGET=receipt["host_build_target"],
               OMAVLESS_ABORT_EXPECTED_DEVICE=str(elf.before.st_dev), OMAVLESS_ABORT_EXPECTED_INODE=str(elf.before.st_ino))
    # The sole extra inherited FD is this verified read-only ELF, never a root
    # evidence/directory FD. Native Popen credential kwargs, no preexec_fn/PAM.
    child = core.spawn([str(elf.path), "--exact", core.MATRIX, "--ignored", "--nocapture", "--test-threads=1"],
                       executable=f"/proc/self/fd/{elf.fd}", pass_fds=(elf.fd,), env=env,
                       stdin=subprocess.DEVNULL, stdout=out, stderr=err, **CREDENTIALS)
    if core.await_exact(child, 300) != 0:
        core.quarantine(child)
    # No observation or further operation is reached on unknown/nonzero exit.
    elf.recheck()
    require(os.fstat(out).st_size <= LIMIT and os.fstat(err).st_size <= LIMIT)
    os.fsync(out)
    os.fsync(err)
    return os.pread(err, LIMIT + 1, 0).decode()


def inspect_cases(output, elf):
    available()
    lines = output.splitlines()
    expected = f"{elf.before.st_dev}:{elf.before.st_ino}"
    require(len(lines) == 7 and lines[0] == "matrix-executed-identity=" + expected
            and lines[-1] == "matrix-completed-identity=" + expected)
    roots = [line.removeprefix("private-fixture-root=") for line in lines[1:-1]]
    require(len(set(roots)) == 5 and all(re.fullmatch(r"/run/user/1000/ov-abort-process-[0-9a-f]{32}", root) for root in roots))
    cases = set()
    for root in roots:
        directory = Directory(root, 1000)
        result = HeldFile(directory, "result.json", 4096, 0o600, 1000)
        value = json.loads(result.bytes(), object_pairs_hook=core.pairs)
        require(set(value) == {"schema", "case", "signal", "reentry"}
                and type(value["schema"]) is int and value["schema"] == 1
                and type(value["signal"]) is int and value["signal"] == 9)
        require(value["case"] in {"linked", "mixed", "empty", "full", "final"} and value["case"] not in cases)
        require(value["reentry"] == ("refused-preserved" if value["case"] == "empty" else "aborted-still-fenced"))
        HeldFile(directory, "archive.ovb", 64 * 1024 * 1024, 0o600, 1000)
        cases.add(value["case"])


def proc_fields(proc):
    data = (proc / "stat").read_bytes()
    require(len(data) <= 65536)
    fields = data.rsplit(b") ", 1)[1].split()
    require(len(fields) > 19 and fields[19].isdigit())
    return fields[0], fields[19]


def executable_absence(target):
    available()
    try:
        return observe_executables(target)
    except BaseException:
        core.quarantine(None)


def observe_executables(target):
    available()
    entries = list(Path("/proc").iterdir())
    require(len(entries) <= 32768)
    checked = 0
    for proc in entries:
        if not proc.name.isdecimal():
            continue
        owner = proc.stat()
        if owner.st_uid != 1000:
            continue
        state, start = proc_fields(proc)
        if state == b"Z":
            require(proc_fields(proc) == (state, start) and proc.stat().st_uid == 1000)
            continue
        fd = os.open(proc / "exe", os.O_RDONLY | os.O_CLOEXEC)
        RETAINED.append(fd)
        before = os.fstat(fd)
        after_state, after_start = proc_fields(proc)
        require(start == after_start and after_state not in (b"Z", b"X") and proc.stat().st_uid == 1000
                and identity(before) == identity(os.fstat(fd)) == identity((proc / "exe").stat()))
        require((before.st_dev, before.st_ino) != (target.st_dev, target.st_ino))
        checked += 1
    return checked  # Point-in-time exact inode observation, not atomic descendants.


def main():
    global core, PHASE
    require(os.getresuid() == os.getresgid() == (0, 0, 0))
    require(sys.flags.isolated == 1 and sys.dont_write_bytecode)
    require(len(sys.argv) == 3 and sys.argv[1] == "--run-reviewed-root-observed-abort"
            and re.fullmatch(r"[0-9a-f]{64}", sys.argv[2]))
    path = Path(__file__)
    require(path.is_absolute() and path.name == "root_vm_guard.py" and path.parent.parent == Path("/run"))
    match = re.fullmatch(r"ov-abort-root-([0-9a-f]{32})", path.parent.name)
    require(match is not None)
    stage = Directory(path.parent, 0)
    receipt = HeldFile(stage, "receipt.json", 8192, 0o600, 0, sys.argv[2])
    legacy = HeldFile(stage, "vm_guard.py", LIMIT, 0o500, 0, LEGACY_SHA)
    # Only the byte-exact reviewed legacy definitions; its main never runs.
    loaded = types.ModuleType("sealed_legacy_definitions_not_main")
    exec(compile(legacy.bytes(), "<sealed-reviewed-legacy-guard>", "exec"), loaded.__dict__)
    core = loaded
    value = core.copy_receipt(json.loads(receipt.bytes(), object_pairs_hook=core.pairs))
    own = HeldFile(stage, "root_vm_guard.py", LIMIT, 0o500, 0, value["guard_sha256"])
    user_stage = Directory(Path("/run/user/1000") / stage.path.name, 1000)
    elf = HeldFile(user_stage, "fixture", 1024**3, 0o500, 1000, value["elf_sha256"])
    require(elf.before.st_size == value["host_frozen"]["size"] and os.pread(elf.fd, 4, 0) == b"\x7fELF")
    evidence = Evidence(stage)
    core.command = command  # Same fixed snapshots, all commands as UID/GID1000.
    PHASE = "before-baseline"
    require(command(evidence, ["/usr/bin/systemd-detect-virt", "--vm"])[1].strip() == b"kvm")
    before = core.snapshot(evidence)
    evidence.write("baseline-before.json", before)  # fsync before any matrix effects.
    for pin in (receipt, legacy, own, elf):
        pin.recheck()
    PHASE = "matrix"
    output = execute_matrix(evidence, elf, value)
    PHASE = "case-receipts"
    inspect_cases(output, elf)
    PHASE = "root-inode-observation"
    checked = executable_absence(elf.before)
    PHASE = "after-baseline"
    after = core.snapshot(evidence)
    evidence.write("baseline-after.json", after)
    require(before.keys() == after.keys() and all(before[k] == after[k] for k in before if k != "network")
            and core.network_equal(before["network"], after["network"]))
    for pin in (receipt, legacy, own, elf):
        pin.recheck()
    PHASE = "result"
    evidence.write("result.json", {"schema": "t4-root-observed-process-v1", "head": value["head"],
        "elf_sha256": elf.sha, "cases": 5, "canonical_checks": 9, "root_units_preserved": True,
        "network_preserved": True, "current_uid1000_executables_checked": checked,
        "point_in_time_exact_fixture_inode_absent": True, "atomic_descendant_absence_claimed": False,
        "outcome": "PROCESS_LOSS_ONLY_STILL_FENCED"})
    # Evidence stays root-owned; any read-only handoff needs separate approval.
    print("T4_ROOT_OBSERVED_PROCESS_LOSS_NOT_POWERLOSS_OR_PRODUCT_PASS")


if __name__ == "__main__":
    os.umask(0o077)
    try:
        main()
    except BaseException:
        if core is not None:
            core.UNCERTAIN = True
        print("T4_ROOT_OBSERVED_NONPASS_RETAINED_PHASE_" + PHASE, file=sys.stderr)
        sys.exit(2)
