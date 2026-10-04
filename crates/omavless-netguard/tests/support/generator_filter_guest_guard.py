#!/usr/bin/python3
"""Root-reviewed dedicated-VM guard. No automatic failure cleanup or recovery.

This is developer tooling, never installed. Run only after an exclusive VM
lease and independent artifact/host review. No authentication data is accepted.
"""
import hashlib
import json
import os
from pathlib import Path
import stat
import subprocess
import sys
import tempfile
import time

STAGE = Path("/run/omavless-k1-typed-order-filter-fixture")
UNIT = "omavless-k1-typed-order-filter-fixture.service"
LINK = Path("/run/systemd/system") / UNIT
CGROUP = Path("/sys/fs/cgroup/system.slice") / UNIT
PROBE_SHA = "b7dc81b89045c591efd375765ddf4fc4792afedf86cbc73c18fd94d1227d7332"
# Filled only from reviewed, frozen runner bytes; an unset pin refuses.
RUNNER_SHA = "d573b198ea685495399475732dd84cd1cbd687c12404966cb9eccb8ed69d8637"
SERVICE_FIELDS = ["ActiveState", "SubState", "MainPID"]
NETWORK_COMMANDS = {
    "address": ["/usr/bin/ip", "-j", "address", "show"],
    "route": ["/usr/bin/ip", "-j", "route", "show", "table", "all"],
    "rule": ["/usr/bin/ip", "-j", "rule", "show"],
    "route6": ["/usr/bin/ip", "-6", "-j", "route", "show", "table", "all"],
    "rule6": ["/usr/bin/ip", "-6", "-j", "rule", "show"],
}
ACTIVATION_ROOTS = (
    "/etc/systemd", "/usr/lib/systemd/system", "/usr/lib/systemd/user",
    "/usr/lib/systemd/system-generators", "/usr/lib/systemd/user-generators",
    "/usr/lib/systemd/system-environment-generators",
    "/usr/lib/systemd/user-environment-generators", "/run/systemd/system",
    "/run/systemd/transient", "/run/systemd/generator", "/run/systemd/generator.early",
    "/run/systemd/generator.late", "/run/user/1000/systemd/generator",
    "/run/user/1000/systemd/generator.early", "/run/user/1000/systemd/generator.late",
    "/etc/pam.d", "/etc/profile.d", "/etc/environment", "/etc/security/pam_env.conf",
    "/home/kdk_vm/.config/systemd", "/home/kdk_vm/.config/environment.d",
)
TARGET_ROOTS = ("/usr/lib/systemd", "/etc/systemd", "/run/systemd",
                "/home/kdk_vm/.config/systemd",
                "/run/user/1000/systemd/generator",
                "/run/user/1000/systemd/generator.early",
                "/run/user/1000/systemd/generator.late")


class Refused(Exception):
    pass


UNCERTAIN = False
RETAINED = []


class OwnedProcess(subprocess.Popen):
    """No implicit wait query or ECHILD-to-success path during destruction."""
    def __del__(self):
        pass

    def _internal_poll(self, *args, **kwargs):
        return self.returncode

    def poll(self):
        raise Refused()

    def wait(self, *args, **kwargs):
        raise Refused()


def uncertain(child):
    global UNCERTAIN
    UNCERTAIN = True
    RETAINED.append(child)


def spawn(args, **kwargs):
    require(not UNCERTAIN)
    try:
        return OwnedProcess(args, **kwargs)
    except BaseException:
        uncertain(None)
        raise Refused() from None


def require(condition):
    if not condition:
        raise Refused()


def digest(data):
    return hashlib.sha256(data).hexdigest()


def command(args):
    require(not UNCERTAIN)
    # No pipe deadlock, hidden communicate/wait fallback or automatic timeout
    # signal. Private anonymous files remain harmless even if a refused child
    # still holds its own output descriptor until it naturally exits.
    with tempfile.TemporaryFile(dir=STAGE) as stdout, tempfile.TemporaryFile(dir=STAGE) as stderr:
        child = spawn(args, env={"PATH": "/usr/bin", "LC_ALL": "C"},
                      stdout=stdout, stderr=stderr)
        code = await_child(child, seconds=12)
        require(code == 0 and stdout.tell() <= 8 * 1024 * 1024
                and stderr.tell() <= 8 * 1024 * 1024)
        stdout.seek(0)
        return stdout.read(8 * 1024 * 1024 + 1)


def pinned_file(path, expected, mode):
    info = path.lstat()
    require(stat.S_ISREG(info.st_mode) and info.st_uid == info.st_gid == 0
            and stat.S_IMODE(info.st_mode) == mode and info.st_nlink == 1)
    require(digest(path.read_bytes()) == expected)
    require(command(["/usr/bin/getcap", str(path)]) == b"")
    return (info.st_dev, info.st_ino)


def inventory():
    """Complete path/type/mode/owner/link/content inventory; never follow links."""
    records = {}
    total = 0

    def visit(path):
        nonlocal total
        if str(path) in records:
            return
        require(len(records) < 30000)
        try:
            info = path.lstat()
        except FileNotFoundError:
            records[str(path)] = ["absent"]
            return
        row = [info.st_mode, info.st_uid, info.st_gid]
        if stat.S_ISLNK(info.st_mode):
            row += ["link", os.readlink(path)]
        elif stat.S_ISREG(info.st_mode):
            total += info.st_size
            require(total <= 128 * 1024 * 1024)
            row += ["file", digest(path.read_bytes())]
        elif stat.S_ISDIR(info.st_mode):
            row += ["directory"]
        elif path == Path("/dev/null") and stat.S_ISCHR(info.st_mode):
            require(os.major(info.st_rdev) == 1 and os.minor(info.st_rdev) == 3)
            row += ["null-device"]
        else:
            raise Refused()
        records[str(path)] = row
        if stat.S_ISLNK(info.st_mode):
            target = path.resolve(strict=True)
            require(target == Path("/dev/null") or any(target.is_relative_to(Path(prefix))
                    for prefix in TARGET_ROOTS))
            visit(target)
        if stat.S_ISDIR(info.st_mode):
            for child in sorted(path.iterdir()):
                visit(child)

    for root in ACTIVATION_ROOTS:
        visit(Path(root))
    return records


def network_equal(before, after, path=()):
    if type(before) is not type(after):
        return False
    if isinstance(before, dict):
        return before.keys() == after.keys() and all(
            network_equal(before[key], after[key], (*path, key)) for key in before)
    if isinstance(before, list):
        return len(before) == len(after) and all(
            network_equal(left, right, (*path, index))
            for index, (left, right) in enumerate(zip(before, after)))
    if before == after:
        return True
    return (len(path) == 5 and path[0] == "address" and type(path[1]) is int
            and path[2] == "addr_info" and type(path[3]) is int
            and path[4] in {"valid_life_time", "preferred_life_time"}
            and type(before) is int and 0 <= after <= before)


def snapshot():
    require(not UNCERTAIN)
    user = ["/usr/bin/runuser", "-u", "kdk_vm", "--", "/usr/bin/env",
            "XDG_RUNTIME_DIR=/run/user/1000", "/usr/bin/systemctl", "--user", "show",
            "omavless-runtime.service"]
    service = {}
    for name in SERVICE_FIELDS:
        raw = command([*user, "-p", name]).decode().strip()
        require(raw.startswith(name + "=") and "\n" not in raw)
        service[name] = raw.split("=", 1)[1]
    require(service["ActiveState"] == "active" and service["SubState"] == "running")
    pid = service["MainPID"]
    require(pid.isdecimal() and int(pid) > 1)
    process = Path("/proc") / pid
    process_stat = (process / "stat").read_text()
    require(") " in process_stat)
    start = process_stat.rsplit(") ", 1)[1].split()[19]
    executable = digest((process / "exe").read_bytes())
    require(executable == digest(Path("/usr/bin/omavless").read_bytes()))
    ns = (process / "ns/net").stat()
    self_ns = Path("/proc/self/ns/net").stat()
    manager_ns = Path("/proc/1/ns/net").stat()
    require((ns.st_dev, ns.st_ino) == (self_ns.st_dev, self_ns.st_ino)
            == (manager_ns.st_dev, manager_ns.st_ino))
    canonical = (service, start, executable, ns.st_dev, ns.st_ino)
    cores = []
    for proc in Path("/proc").iterdir():
        if not proc.name.isdecimal():
            continue
        try:
            if (proc / "comm").read_text().strip() == "mihomo":
                fields = (proc / "stat").read_text().rsplit(") ", 1)[1].split()
                cores.append((proc.name, fields[19], digest((proc / "exe").read_bytes())))
        except FileNotFoundError:
            # An unreadable changing inventory is uncertainty, not empty.
            raise Refused() from None
    network = {name: json.loads(command(args)) for name, args in NETWORK_COMMANDS.items()}
    tuns = sorted(str(item) for item in Path("/sys/class/net").glob("*/tun_flags"))
    private_paths = (
        "/home/kdk_vm/.config/omavless/profiles.json",
        "/home/kdk_vm/.config/omavless/route-template.yaml",
        "/home/kdk_vm/.local/state/omavless/desired.json",
        "/home/kdk_vm/.local/state/omavless/ownership.json",
    )
    private = [digest(Path(path).read_bytes()) for path in private_paths]
    graph = command([*user, "-p", "FragmentPath", "-p", "DropInPaths", "-p", "ExecStart",
                     "-p", "EnvironmentFiles", "-p", "PassEnvironment", "-p", "Requires",
                     "-p", "Wants", "-p", "Before", "-p", "After", "-p", "UnitFileState"])
    installed = {}
    for path in ["/usr/bin/omavless", "/usr/bin/mihomo", "/usr/lib/omavless-dns/omavless-dns-broker"]:
        info = Path(path).stat()
        installed[path] = (info.st_mode, info.st_uid, info.st_gid,
                           digest(Path(path).read_bytes()),
                           command(["/usr/bin/getcap", path]).decode())
    return {
        "canonical": canonical, "cores": sorted(cores), "tuns": tuns,
        "network": network, "private_hashes": private,
        "resolver": digest(command(["/usr/bin/resolvectl", "status", "--no-pager"])),
        "resolvconf": digest(Path("/etc/resolv.conf").read_bytes()),
        "packages": digest(command(["/usr/bin/pacman", "-Q"])),
        "installed": installed, "runtime_graph": digest(graph),
        "manager_environment": digest(command(["/usr/bin/systemctl", "show-environment"])),
        "activation": inventory(),
        "host": command(["/usr/bin/uname", "-srvm"]).decode(),
        "systemd": command(["/usr/bin/systemctl", "--version"]).decode(),
        "container": command(["/usr/bin/systemd-detect-virt", "--vm"]).decode(),
    }


def preserve(before, after):
    return (before.keys() == after.keys()
            and network_equal(before["network"], after["network"])
            and all(before[key] == after[key] for key in before if key != "network"))


def await_child(child, seconds=45):
    require(not UNCERTAIN)
    try:
        return await_child_once(child, seconds)
    except BaseException:
        uncertain(child)
        raise Refused() from None


def await_child_once(child, seconds):
    # Keep the waitable leader until its actual exit has been observed. Do not
    # use Popen.wait's ECHILD compatibility fallback as an exit-zero receipt.
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        seen = os.waitid(os.P_PID, child.pid, os.WEXITED | os.WNOHANG | os.WNOWAIT)
        if seen is None:
            time.sleep(0.05)
            continue
        require(seen.si_pid == child.pid and seen.si_code in
                {os.CLD_EXITED, os.CLD_KILLED, os.CLD_DUMPED})
        reaped, status = os.waitpid(child.pid, os.WNOHANG)
        require(reaped == child.pid and (os.WIFEXITED(status) or os.WIFSIGNALED(status)))
        code = os.waitstatus_to_exitcode(status)
        expected = seen.si_status if seen.si_code == os.CLD_EXITED else -seen.si_status
        require(code == expected)
        child.returncode = code
        return code
    # No signal/reap or subsequent unit command after a timeout or query error.
    raise Refused()


def main():
    require(os.geteuid() == 0 and len(sys.argv) == 1
            and os.environ.get("OMAVLESS_K1_NAMESPACE_FILTER_GUARD") == "1")
    stage_info = STAGE.lstat()
    require(stat.S_ISDIR(stage_info.st_mode) and stage_info.st_uid == stage_info.st_gid == 0
            and stat.S_IMODE(stage_info.st_mode) == 0o700)
    require(command(["/usr/bin/systemd-detect-virt", "--vm"]).strip() == b"kvm")
    require(not LINK.exists() and not LINK.is_symlink() and not CGROUP.exists())
    probe_identity = pinned_file(STAGE / "probe", PROBE_SHA, 0o700)
    runner_identity = pinned_file(STAGE / "runner.sh", RUNNER_SHA, 0o600)
    query_identity = pinned_file(STAGE / "typed-properties.py", "de1ee66fa76d6faa53d4606d1486653ecf19a9746200fc2d997c0101a68f9eda", 0o600)
    query_guard_identity = pinned_file(STAGE / "query-guard.py", "c4a688875037f1d990ee93f0018108ab82bceb211090f61f77c64b83a4c7cc40", 0o600)
    before = snapshot()
    # All output is private and create-only. Failure/timeout never removes evidence.
    log = os.open(STAGE / "runner.log", os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    with os.fdopen(log, "wb") as stream:
        child = spawn(["/usr/bin/bash", str(STAGE / "runner.sh")],
                                 env={"PATH": "/usr/bin", "LC_ALL": "C",
                                      "OMAVLESS_K1_NAMESPACE_FILTER_VM": "1"},
                                 stdout=stream, stderr=subprocess.STDOUT)
        code = await_child(child)
    # No later command/snapshot can heal a failed or uncertain nested query.
    require(code == 0)
    after = snapshot()
    checks = {
        "runner_exit_zero": code == 0,
        "preserved": preserve(before, after),
        "unit_link_absent": not LINK.exists() and not LINK.is_symlink(),
        "cgroup_absent": not CGROUP.exists() and not CGROUP.is_symlink(),
        "probe_unchanged": pinned_file(STAGE / "probe", PROBE_SHA, 0o700) == probe_identity,
        "runner_unchanged": pinned_file(STAGE / "runner.sh", RUNNER_SHA, 0o600) == runner_identity,
        "query_unchanged": pinned_file(STAGE / "typed-properties.py", "de1ee66fa76d6faa53d4606d1486653ecf19a9746200fc2d997c0101a68f9eda", 0o600) == query_identity,
        "query_guard_unchanged": pinned_file(STAGE / "query-guard.py", "c4a688875037f1d990ee93f0018108ab82bceb211090f61f77c64b83a4c7cc40", 0o600) == query_guard_identity,
        "exact_receipts": (STAGE / "runner.log").read_text().splitlines() == [
            "K1_NAMESPACE_FILTER_control_PASS", "K1_NAMESPACE_FILTER_filtered_PASS",
            "K1_NAMESPACE_FILTER_VM_PASS"],
    }
    with (STAGE / "result.json").open("x") as out:
        json.dump({"schema": "k1-namespace-filter-v1", "checks": checks,
                   "probe_sha256": PROBE_SHA, "runner_sha256": RUNNER_SHA}, out)
    require(all(checks.values()))
    print("K1_NAMESPACE_FILTER_GUARDED_PASS")


if __name__ == "__main__":
    os.umask(0o077)
    try:
        main()
    except Exception:
        print("K1_NAMESPACE_FILTER_GUARDED_NONPASS_RETAINED", file=sys.stderr)
        sys.exit(2)
