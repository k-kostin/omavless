#!/usr/bin/python3
# SPDX-License-Identifier: MIT
"""Dedicated-VM developer guard. Explicit lease/review required; never installed."""
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import stat
import subprocess
import sys
import tempfile
import time

LIMIT = 8 * 1024 * 1024
MATRIX = "production_owner::first_abort::process_reentry::fixed_current_process_loss_and_fresh_reentry"
ROOT_UNITS = ("omavless-dns-broker.service", "systemd-resolved.service",
              "omavless-k1-namespace-filter-fixture.service",
              "omavless-k1-generator-filter-fixture.service")
FIELDS = ("LoadState", "ActiveState", "SubState", "MainPID", "ControlPID", "FragmentPath", "DropInPaths")
NETWORK = {"address": ["/usr/bin/ip", "-j", "address", "show"],
           "route": ["/usr/bin/ip", "-j", "route", "show", "table", "all"],
           "rule": ["/usr/bin/ip", "-j", "rule", "show"],
           "route6": ["/usr/bin/ip", "-6", "-j", "route", "show", "table", "all"],
           "rule6": ["/usr/bin/ip", "-6", "-j", "rule", "show"]}
RUNTIME_SHA = "76abb574f611c11c2513436ac4be48d2fb07a69ba35f56127c93c21c6205921c"
CORE_SHA = "ba7a74ed6bbc3098930e6e172fa9b7d4ef7e42a77f7a516046d61fbeb3bfcfe6"
UNCERTAIN = False
RETAINED = []


class Refused(Exception):
    pass


def require(value):
    if not value:
        raise Refused()


def available():
    require(not UNCERTAIN)


def quarantine(child):
    global UNCERTAIN
    UNCERTAIN = True
    RETAINED.append(child)
    raise Refused()


class OwnedProcess(subprocess.Popen):
    def __del__(self):
        pass  # No hidden Popen polling/reaping after unknown ownership.

    def _internal_poll(self, *args, **kwargs):
        return self.returncode

    def poll(self):
        raise Refused()

    def wait(self, *args, **kwargs):
        raise Refused()


def spawn(args, **kwargs):
    available()
    try:
        return OwnedProcess(args, **kwargs)
    except BaseException:
        quarantine(None)


def await_exact(child, seconds):
    available()
    require(child.returncode is None)  # Never query a reaped/reusable PID again.
    deadline = time.monotonic() + seconds
    try:
        while time.monotonic() < deadline:
            seen = os.waitid(os.P_PID, child.pid, os.WEXITED | os.WNOHANG | os.WNOWAIT)
            if seen is None:
                time.sleep(0.02)
                continue
            require(seen.si_pid == child.pid and seen.si_code in (os.CLD_EXITED, os.CLD_KILLED, os.CLD_DUMPED))
            require(type(seen.si_status) is int and (0 <= seen.si_status <= 255 if seen.si_code == os.CLD_EXITED else 1 <= seen.si_status <= 64))
            pid, status = os.waitpid(child.pid, os.WNOHANG)
            require(pid == child.pid and (os.WIFEXITED(status) or os.WIFSIGNALED(status)))
            require(os.WIFEXITED(status) if seen.si_code == os.CLD_EXITED else
                    os.WIFSIGNALED(status) and bool(os.WCOREDUMP(status)) == (seen.si_code == os.CLD_DUMPED))
            expected = seen.si_status if seen.si_code == os.CLD_EXITED else -seen.si_status
            require(os.waitstatus_to_exitcode(status) == expected)
            child.returncode = expected
            return expected
        quarantine(child)  # No timeout signal or reap retry.
    except BaseException:
        if not UNCERTAIN:
            quarantine(child)
        raise


def command(stage, args, allowed=(0,)):
    available()
    with tempfile.TemporaryFile(dir=stage) as out, tempfile.TemporaryFile(dir=stage) as err:
        child = spawn(args, env={"HOME": "/home/kdk_vm", "PATH": "/usr/bin", "LC_ALL": "C", "XDG_RUNTIME_DIR": "/run/user/1000"},
                      stdin=subprocess.DEVNULL, stdout=out, stderr=err)
        code = await_exact(child, 12)
        require(code in allowed and out.tell() <= LIMIT and err.tell() <= LIMIT)
        out.seek(0)
        return code, out.read(LIMIT + 1)


def identity(value):
    return (value.st_dev, value.st_ino, value.st_uid, value.st_gid, value.st_mode,
            value.st_nlink, value.st_size, value.st_mtime_ns, value.st_ctime_ns)


class Pin:
    def __init__(self, path, maximum, mode=None, expected=None):
        self.path, self.maximum = Path(path), maximum
        self.fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC)
        self.before = os.fstat(self.fd)
        require(stat.S_ISREG(self.before.st_mode) and self.before.st_uid == os.getuid() and self.before.st_gid == os.getgid()
                and self.before.st_nlink == 1 and self.before.st_size <= maximum)
        if mode is not None:
            require(stat.S_IMODE(self.before.st_mode) == mode)
        require(os.listxattr(self.fd) == [])
        self.parent_fd = os.open(self.path.parent, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC)
        self.parent = os.fstat(self.parent_fd)
        require(self.parent.st_uid == os.getuid() and self.parent.st_gid == os.getgid() and stat.S_IMODE(self.parent.st_mode) == 0o700)
        self.sha = self.hash()
        if expected is not None:
            require(self.sha == expected)
        self.recheck()

    def hash(self):
        digest = hashlib.sha256()
        offset = 0
        while offset < self.before.st_size:
            data = os.pread(self.fd, min(65536, self.before.st_size - offset), offset)
            require(data)
            digest.update(data)
            offset += len(data)
        require(os.pread(self.fd, 1, offset) == b"")
        return digest.hexdigest()

    def recheck(self):
        available()
        require(identity(self.before) == identity(os.fstat(self.fd)) == identity(self.path.lstat()))
        parent = self.path.parent.lstat()
        fields = lambda m: (m.st_dev, m.st_ino, m.st_uid, m.st_gid, m.st_mode)
        require(fields(self.parent) == fields(os.fstat(self.parent_fd)) == fields(parent))
        require(self.hash() == self.sha)
        require(identity(self.before) == identity(os.fstat(self.fd)) == identity(self.path.lstat()))

    def bytes(self):
        self.recheck()
        require(self.before.st_size <= LIMIT)
        result = os.pread(self.fd, self.before.st_size + 1, 0)
        self.recheck()
        return result


def normalized(value):
    return (type(value) is str and 1 < len(value) <= 4096 and value.startswith("/")
            and not value.endswith("/") and "\0" not in value
            and all(part not in ("", ".", "..") for part in value.split("/")[1:]))


def pairs(values):
    result = {}
    for key, value in values:
        require(key not in result)
        result[key] = value
    return result


def copy_receipt(value):
    require(set(value) == {"schema", "head", "host_build_target", "host_original", "host_frozen", "elf_sha256", "guard_sha256"})
    require(value["schema"] == "t4-abort-vm-copy-v1")
    hex_text = lambda text, n: type(text) is str and len(text) == n and all(c in "0123456789abcdef" for c in text)
    require(hex_text(value["head"], 40) and hex_text(value["elf_sha256"], 64) and hex_text(value["guard_sha256"], 64))
    require(normalized(value["host_build_target"]))
    build = PurePosixPath(value["host_build_target"])
    for key, mode in (("host_original", "0755"), ("host_frozen", "0500")):
        item = value[key]
        require(set(item) == {"path", "device", "inode", "uid", "gid", "mode", "nlink", "size", "sha256"})
        require(normalized(item["path"]) and item["mode"] == mode and item["nlink"] == 1
                and item["sha256"] == value["elf_sha256"] and 4 < item["size"] <= 1024**3)
        require(all(type(item[k]) is int and item[k] >= 0 for k in ("device", "inode", "uid", "gid", "nlink", "size")))
    require(PurePosixPath(value["host_original"]["path"]).is_relative_to(build))
    require(not PurePosixPath(value["host_frozen"]["path"]).is_relative_to(build))
    require(value["host_original"]["size"] == value["host_frozen"]["size"])
    return value


def file_digest(path, maximum=LIMIT, follow=False):
    flags = os.O_RDONLY | os.O_CLOEXEC | os.O_NONBLOCK
    fd = os.open(path, flags if follow else flags | os.O_NOFOLLOW)
    try:
        before = os.fstat(fd)
        require(stat.S_ISREG(before.st_mode) and before.st_size <= maximum)
        digest = hashlib.sha256()
        offset = 0
        while offset < before.st_size:
            chunk = os.pread(fd, min(65536, before.st_size - offset), offset)
            require(chunk)
            digest.update(chunk)
            offset += len(chunk)
        require(os.pread(fd, 1, offset) == b"" and identity(before) == identity(os.fstat(fd)))
        return digest.hexdigest()
    finally:
        os.close(fd)


def show(stage, unit, user=False):
    args = ["/usr/bin/systemctl", "--user" if user else "--system", "--no-pager", "show", unit]
    raw = command(stage, args + [argument for field in FIELDS for argument in ("-p", field)])[1].decode()
    rows = pairs(line.split("=", 1) for line in raw.splitlines())
    require(set(rows) == set(FIELDS))
    return rows


def snapshot(stage):
    available()
    boot = Path("/proc/sys/kernel/random/boot_id").read_text().strip()
    fields = Path("/proc/938/stat").read_text().rsplit(")", 1)[1].split()
    loaded, installed = os.stat("/proc/938/exe"), os.stat("/usr/bin/omavless")
    require(boot == "9cdd6950-1655-495b-a52e-0f8f14200d19" and fields[0] != "Z" and int(fields[19]) == 1901)
    require((loaded.st_dev, loaded.st_ino) == (installed.st_dev, installed.st_ino) == (31, 292400))
    executable = file_digest("/proc/938/exe", 1024**3, follow=True)
    require(executable == file_digest("/usr/bin/omavless", 1024**3) == RUNTIME_SHA)
    require(file_digest("/usr/bin/mihomo", 1024**3, follow=True) == CORE_SHA)
    service = show(stage, "omavless-runtime.service", user=True)
    require((service["ActiveState"], service["SubState"], service["MainPID"]) == ("active", "running", "938"))
    namespace = os.stat("/proc/self/ns/net")
    require((namespace.st_dev, namespace.st_ino) == (5, 4026531833))
    private = [file_digest(Path("/home/kdk_vm") / name) for name in (
        ".config/omavless/profiles.json", ".config/omavless/route-template.yaml",
        ".local/state/omavless/desired.json", ".local/state/omavless/ownership.json")]
    code, cores = command(stage, ["/usr/bin/pgrep", "-x", "mihomo"], allowed=(0, 1))
    require(code == 0 or cores == b"")
    return {"CANONICAL_EPOCH": [boot, 938, 1901, loaded.st_dev, loaded.st_ino],
            "PRIVATE_FILES": private, "USER_SERVICE": service, "EXECUTABLE": executable,
            "NAMESPACE": [namespace.st_dev, namespace.st_ino], "CORE_INVENTORY": cores.decode(),
            "TUN_INVENTORY": sorted(str(p) for p in Path("/sys/class/net").glob("*/tun_flags")),
            "RESOLVER": hashlib.sha256(command(stage, ["/usr/bin/resolvectl", "status", "--no-pager"])[1]).hexdigest(),
            "RESOLVCONF": file_digest("/etc/resolv.conf", follow=True),
            "root_units": {name: show(stage, name) for name in ROOT_UNITS},
            "network": {name: json.loads(command(stage, args)[1], object_pairs_hook=pairs) for name, args in NETWORK.items()}}


def network_equal(before, after, path=()):
    if type(before) is not type(after):
        return False
    if isinstance(before, dict):
        return before.keys() == after.keys() and all(network_equal(before[k], after[k], (*path, k)) for k in before)
    if isinstance(before, list):
        return len(before) == len(after) and all(network_equal(a, b, (*path, i)) for i, (a, b) in enumerate(zip(before, after)))
    return before == after or (len(path) == 5 and path[0] == "address" and type(path[1]) is int
        and path[2] == "addr_info" and type(path[3]) is int and path[4] in {"valid_life_time", "preferred_life_time"}
        and type(before) is int and 0 <= after <= before)


def write_private(path, value):
    fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_CLOEXEC, 0o600)
    with os.fdopen(fd, "w") as stream:
        json.dump(value, stream, sort_keys=True)


def main():
    require(len(sys.argv) == 3 and sys.argv[1] == "--run-reviewed-first-abort-process")
    require(os.getuid() == os.getgid() == 1000 and os.environ.get("HOME") == "/home/kdk_vm")
    stage = Path(__file__).resolve().parent
    receipt = Pin(stage / "receipt.json", 8192, 0o600, sys.argv[2])
    value = copy_receipt(json.loads(receipt.bytes(), object_pairs_hook=pairs))
    guard = Pin(stage / "vm_guard.py", LIMIT, 0o500, value["guard_sha256"])
    elf = Pin(stage / "fixture", 1024**3, 0o500, value["elf_sha256"])
    require(elf.before.st_size == value["host_frozen"]["size"] and os.pread(elf.fd, 4, 0) == b"\x7fELF")
    require(command(stage, ["/usr/bin/systemd-detect-virt", "--vm"])[1].strip() == b"kvm")
    before = snapshot(stage)
    write_private(stage / "baseline-before.json", before)
    stdout = os.open(stage / "matrix.stdout", os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    stderr = os.open(stage / "matrix.stderr", os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    env = {"HOME": "/home/kdk_vm", "PATH": "/usr/bin", "LC_ALL": "C", "XDG_RUNTIME_DIR": "/run/user/1000",
           "OMAVLESS_ABORT_FROZEN_ELF": str(elf.path), "OMAVLESS_ABORT_FROZEN_SHA256": elf.sha,
           "OMAVLESS_ABORT_BUILD_TARGET": value["host_build_target"]}
    child = spawn([str(elf.path), "--exact", MATRIX, "--ignored", "--nocapture", "--test-threads=1"],
                  env=env, stdin=subprocess.DEVNULL, stdout=stdout, stderr=stderr)
    # Nonzero matrix may mean quarantined descendants: no after-query/cleanup.
    if await_exact(child, 300) != 0:
        quarantine(child)
    os.close(stdout)
    os.close(stderr)
    for pin in (receipt, guard, elf):
        pin.recheck()
    output = Pin(stage / "matrix.stderr", LIMIT, 0o600).bytes().decode()
    roots = [Path(line.split("=", 1)[1]) for line in output.splitlines() if line.startswith("private-fixture-root=")]
    require(len(roots) == len(set(roots)) == 5)
    cases = set()
    for root in roots:
        require(root.parent == Path("/run/user/1000") and root.name.startswith("ov-abort-process-")
                and len(root.name.removeprefix("ov-abort-process-")) == 32)
        result = json.loads(Pin(root / "result.json", 4096, 0o600).bytes(), object_pairs_hook=pairs)
        require(set(result) == {"schema", "case", "signal", "reentry"} and result["schema"] == 1 and result["signal"] == 9)
        require(result["case"] not in cases and result["case"] in {"linked", "mixed", "empty", "full", "final"})
        require(result["reentry"] == ("refused-preserved" if result["case"] == "empty" else "aborted-still-fenced"))
        Pin(root / "archive.ovb", 64 * 1024 * 1024, 0o600)
        cases.add(result["case"])
    # Strict executable-inode quiescence, not a broad process-name kill.
    for proc in Path("/proc").iterdir():
        if proc.name.isdecimal() and proc.stat().st_uid == 1000:
            try:
                loaded = (proc / "exe").stat()
            except FileNotFoundError:
                raise Refused() from None
            require((loaded.st_dev, loaded.st_ino) != (elf.before.st_dev, elf.before.st_ino))
    after = snapshot(stage)
    write_private(stage / "baseline-after.json", after)
    require(before.keys() == after.keys() and all(before[k] == after[k] for k in before if k != "network")
            and network_equal(before["network"], after["network"]))
    for pin in (receipt, guard, elf):
        pin.recheck()
    write_private(stage / "result.json", {"schema": "t4-abort-process-guard-v1", "head": value["head"],
                  "elf_sha256": elf.sha, "cases": 5, "canonical_checks": 9, "network_preserved": True,
                  "outcome": "PROCESS_LOSS_ONLY_STILL_FENCED"})
    print("T4_ABORT_PROCESS_LOSS_GUARDED_NOT_POWERLOSS_OR_PRODUCT_PASS")


if __name__ == "__main__":
    os.umask(0o077)
    try:
        main()
    except BaseException:
        print("T4_ABORT_PROCESS_GUARDED_NONPASS_RETAINED", file=sys.stderr)
        sys.exit(2)
