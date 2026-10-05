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
import tempfile
import time

STAGE = Path("/run/omavless-k1-retained-lease-regression")
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
SCOPE_DEADLINE = float('inf')  # Fixed outer guard installs its original budget.


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
    child = None
    try:
        child = OwnedProcess(args, **kwargs)
        require(not UNCERTAIN)
        return child
    except BaseException:
        uncertain(child)
        raise Refused() from None


def require(condition):
    if not condition or time.monotonic() >= SCOPE_DEADLINE:
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


RETAINED_LINK = Path('/run/systemd/system/omavless-k1-versioned-config-reference.service')
RETAINED_FRAGMENT = Path('/run/omavless-k1-versioned-config-reference/fixture.service')
RETAINED_SHA = '8916ea57f66461fcc1096c74589388221a7826e65bf64e97ea77c3b3809de81c'
RETAINED_ACTIVATION = None
ACTIVATION_REFUSED = False
FAILED_LINK = Path('/run/systemd/system/omavless-k1-private-lifecycle-admission.service')
FAILED_STAGE = Path('/run/omavless-k1-private-lifecycle-admission')
FAILED_PINS = {
    'guard.py': (0o500, 256 * 1024, '67c65f37e22dc8418108a86832416a70084fd28f105a799ed72132a9e3abe45c'),
    'query-guard.py': (0o600, 256 * 1024, 'c6a0bed9b6572effc2b445416b356c62d0ec64788906792e0a9af6b5b326eedd'),
    'fixture.service': (0o600, 16384, '7912c3204829773d7b5598fbd7bfb9174e14674d69ef5c30579631324a180f85'),
    'probe': (0o500, 128 * 1024 * 1024, '803f7959df873660d262da02b19a7676cb00f36ef16ec0709120ff03582ad242'),
}
FAILED_ACTIVATION = None
RESPONSE_LINK = Path('/run/systemd/system/omavless-k1-admission-response-diagnostic.service')
RESPONSE_STAGE = Path('/run/omavless-k1-admission-response-diagnostic')
RESPONSE_PINS = {
    'guard.py': (0o500, 256 * 1024, 'cc19409a109393f11f1328bfa41d951a185276c5e202eb9f3bc069213cf10565'),
    'query-guard.py': (0o600, 256 * 1024, 'ee277f747c3e654e21606a228172606cfc39d839ec7091074b778268566f2ec0'),
    'fixture.service': (0o600, 16384, 'a3b03103bbd8c43f6e6ca6755c063a7e851f40a006303c93028be80aec411e58'),
    'probe': (0o500, 128 * 1024 * 1024, 'a2b8dd3b7cc1658c536fd81bd25a74ae55255cb2159624cfe8f548c162ad0e9a'),
}
RESPONSE_ACTIVATION = None


def exact_identity(m):
    return (m.st_dev, m.st_ino, m.st_mode, m.st_uid, m.st_gid, m.st_nlink,
            m.st_size, m.st_mtime_ns, m.st_ctime_ns)


def parent_identity(m):
    return (m.st_dev, m.st_ino, m.st_mode, m.st_uid, m.st_gid)


class RetainedFailedActivation:
    """Only two literal historical tuples; no caller paths or manager queries."""
    def __init__(self, generation=615):
        require(type(generation) is int and generation in (615, 621))
        self.stage, self.link_path, pins = ((FAILED_STAGE, FAILED_LINK, FAILED_PINS)
            if generation == 615 else (RESPONSE_STAGE, RESPONSE_LINK, RESPONSE_PINS))
        self.sealed = False
        self.parents, self.files = [], {}
        try:
            for path in (Path('/'), Path('/run'), self.stage,
                         Path('/run/systemd'), self.link_path.parent):
                fd = os.open(path, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC)
                info = os.fstat(fd)
                self.parents.append((path, fd, info))
                require(info.st_uid == info.st_gid == 0 and not info.st_mode & 0o022
                        and parent_identity(info) == parent_identity(path.lstat()))
            require(stat.S_IMODE(self.parents[2][2].st_mode) == 0o700)
            self.link_fd = os.open(self.link_path.name, os.O_PATH | os.O_NOFOLLOW | os.O_CLOEXEC,
                                   dir_fd=self.parents[4][1])
            self.link = os.fstat(self.link_fd)
            require(stat.S_ISLNK(self.link.st_mode) and self.link.st_uid == self.link.st_gid == 0
                    and self.link.st_nlink == 1)
            for name, (mode, maximum, sha) in pins.items():
                fd = os.open(name, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC,
                             dir_fd=self.parents[2][1])
                info = os.fstat(fd)
                self.files[name] = (fd, info, sha)
                require(stat.S_ISREG(info.st_mode) and info.st_uid == info.st_gid == 0
                        and stat.S_IMODE(info.st_mode) == mode and info.st_nlink == 1
                        and 0 < info.st_size <= maximum and not os.listxattr(fd))
            self.recheck()
        except BaseException:
            self.sealed = True
            raise

    def recheck(self):
        require(not self.sealed)
        try:
            for path, fd, info in self.parents:
                require(parent_identity(info) == parent_identity(os.fstat(fd)) == parent_identity(path.lstat()))
            require(exact_identity(self.parents[2][2]) == exact_identity(os.fstat(self.parents[2][1])))
            require(exact_identity(self.link) == exact_identity(os.fstat(self.link_fd))
                    == exact_identity(os.stat(self.link_path.name, dir_fd=self.parents[4][1], follow_symlinks=False)))
            require(os.readlink(self.link_path.name, dir_fd=self.parents[4][1]) == str(self.stage / 'fixture.service'))
            for name, (fd, info, sha) in self.files.items():
                require(exact_identity(info) == exact_identity(os.fstat(fd))
                        == exact_identity(os.stat(name, dir_fd=self.parents[2][1], follow_symlinks=False))
                        and not os.listxattr(fd))
                hashed, offset = hashlib.sha256(), 0
                while offset < info.st_size:
                    block = os.pread(fd, min(65536, info.st_size - offset), offset)
                    require(block); hashed.update(block); offset += len(block)
                require(os.pread(fd, 1, offset) == b'' and hashed.hexdigest() == sha
                        and exact_identity(info) == exact_identity(os.fstat(fd))
                        == exact_identity(os.stat(name, dir_fd=self.parents[2][1], follow_symlinks=False)))
            require(exact_identity(self.link) == exact_identity(os.fstat(self.link_fd))
                    == exact_identity(os.stat(self.link_path.name, dir_fd=self.parents[4][1], follow_symlinks=False)))
            require(os.readlink(self.link_path.name, dir_fd=self.parents[4][1]) == str(self.stage / 'fixture.service'))
            for path, fd, info in self.parents:
                require(parent_identity(info) == parent_identity(os.fstat(fd)) == parent_identity(path.lstat()))
            require(exact_identity(self.parents[2][2]) == exact_identity(os.fstat(self.parents[2][1])))
        except BaseException:
            self.sealed = True
            raise

    def admits(self, link, target):
        self.recheck()
        return link == self.link_path and target == self.stage / 'fixture.service'

    def file_record(self, path, info):
        self.recheck()
        require(path.parent == self.stage and path.name in self.files)
        _, original, sha = self.files[path.name]
        require(exact_identity(info) == exact_identity(original))
        return ["file", sha]


def retained_failed_activation():
    global FAILED_ACTIVATION, ACTIVATION_REFUSED
    require(not ACTIVATION_REFUSED)
    try:
        if FAILED_ACTIVATION is None:
            FAILED_ACTIVATION = RetainedFailedActivation()
        FAILED_ACTIVATION.recheck()
        return FAILED_ACTIVATION
    except BaseException:
        ACTIVATION_REFUSED = True
        raise


def retained_response_activation():
    global RESPONSE_ACTIVATION, ACTIVATION_REFUSED
    require(not ACTIVATION_REFUSED)
    try:
        if RESPONSE_ACTIVATION is None:
            RESPONSE_ACTIVATION = RetainedFailedActivation(621)
        RESPONSE_ACTIVATION.recheck()
        return RESPONSE_ACTIVATION
    except BaseException:
        ACTIVATION_REFUSED = True
        raise


class RetainedActivation:
    """One historical inert unit, observed without activating or repairing it."""
    def __init__(self):
        self.parents = []
        for path in (Path('/'), Path('/run'), RETAINED_FRAGMENT.parent,
                     Path('/run/systemd'), RETAINED_LINK.parent):
            fd = os.open(path, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC)
            meta = os.fstat(fd)
            require(meta.st_uid == meta.st_gid == 0 and meta.st_mode & 0o022 == 0
                    and parent_identity(meta) == parent_identity(path.lstat()))
            self.parents.append((path, fd, meta))
        require(stat.S_IMODE(self.parents[2][2].st_mode) == 0o700)
        self.link_fd = os.open(RETAINED_LINK.name, os.O_PATH | os.O_NOFOLLOW | os.O_CLOEXEC,
                               dir_fd=self.parents[4][1])
        self.link = os.fstat(self.link_fd)
        require(stat.S_ISLNK(self.link.st_mode) and self.link.st_uid == self.link.st_gid == 0
                and self.link.st_nlink == 1)
        self.fragment_fd = os.open(RETAINED_FRAGMENT.name,
            os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC, dir_fd=self.parents[2][1])
        self.fragment = os.fstat(self.fragment_fd)
        require(stat.S_ISREG(self.fragment.st_mode)
                and self.fragment.st_uid == self.fragment.st_gid == 0
                and stat.S_IMODE(self.fragment.st_mode) == 0o600
                and self.fragment.st_nlink == 1 and 0 < self.fragment.st_size <= 16384
                and not os.listxattr(self.fragment_fd))
        self.recheck()

    def recheck(self):
        for path, fd, meta in self.parents:
            require(parent_identity(meta) == parent_identity(os.fstat(fd))
                    == parent_identity(path.lstat()))
        require(exact_identity(self.link) == exact_identity(os.fstat(self.link_fd))
                == exact_identity(os.stat(RETAINED_LINK.name, dir_fd=self.parents[4][1],
                                          follow_symlinks=False)))
        require(os.readlink(RETAINED_LINK.name, dir_fd=self.parents[4][1]) == str(RETAINED_FRAGMENT))
        require(exact_identity(self.fragment) == exact_identity(os.fstat(self.fragment_fd))
                == exact_identity(os.stat(RETAINED_FRAGMENT.name, dir_fd=self.parents[2][1],
                                          follow_symlinks=False)))
        raw = os.pread(self.fragment_fd, self.fragment.st_size + 1, 0)
        require(len(raw) == self.fragment.st_size and digest(raw) == RETAINED_SHA
                and exact_identity(self.fragment) == exact_identity(os.fstat(self.fragment_fd))
                == exact_identity(os.stat(RETAINED_FRAGMENT.name, dir_fd=self.parents[2][1],
                                          follow_symlinks=False)))
        require(exact_identity(self.link) == exact_identity(os.fstat(self.link_fd))
                == exact_identity(os.stat(RETAINED_LINK.name, dir_fd=self.parents[4][1],
                                          follow_symlinks=False)))
        for path, fd, meta in self.parents:
            require(parent_identity(meta) == parent_identity(os.fstat(fd))
                    == parent_identity(path.lstat()))
        return raw

    def admits(self, link, target):
        self.recheck()
        return link == RETAINED_LINK and target == RETAINED_FRAGMENT


def retained_activation():
    global RETAINED_ACTIVATION, ACTIVATION_REFUSED
    require(not ACTIVATION_REFUSED)
    try:
        if RETAINED_ACTIVATION is None:
            RETAINED_ACTIVATION = RetainedActivation()
        RETAINED_ACTIVATION.recheck()
        return RETAINED_ACTIVATION
    except BaseException:
        ACTIVATION_REFUSED = True
        raise


def inventory():
    global ACTIVATION_REFUSED
    require(not ACTIVATION_REFUSED)
    try:
        return _inventory()
    except BaseException:
        ACTIVATION_REFUSED = True
        raise


def _inventory():
    """Complete path/type/mode/owner/link/content inventory; never follow links."""
    retained = retained_activation()
    failed = retained_failed_activation()
    response = retained_response_activation()
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
            if path == RETAINED_FRAGMENT:
                raw = retained.recheck()
                require(exact_identity(info) == exact_identity(retained.fragment))
                row += ["file", digest(raw)]
            elif path.parent == FAILED_STAGE and path.name in FAILED_PINS:
                row += failed.file_record(path, info)
            elif path.parent == RESPONSE_STAGE and path.name in RESPONSE_PINS:
                row += response.file_record(path, info)
            else:
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
                    for prefix in TARGET_ROOTS) or retained.admits(path, target)
                    or failed.admits(path, target) or response.admits(path, target))
            visit(target)
        if stat.S_ISDIR(info.st_mode):
            for child in sorted(path.iterdir()):
                visit(child)

    for root in ACTIVATION_ROOTS:
        visit(Path(root))
    retained.recheck()
    failed.recheck()
    response.recheck()
    require(str(RETAINED_LINK) in records and str(RETAINED_FRAGMENT) in records)
    require(str(FAILED_LINK) in records and str(FAILED_STAGE / 'fixture.service') in records)
    require(str(RESPONSE_LINK) in records and str(RESPONSE_STAGE / 'fixture.service') in records)
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
    try:
        require(not UNCERTAIN)
        return await_child_once(child, seconds)
    except BaseException:
        uncertain(child)
        raise Refused() from None


def await_child_once(child, seconds):
    # ONLY an exact typed waitable exit-zero observation permits one reap.
    # Nonzero/signaled/malformed/unknown states retain the waitable child.
    require(not UNCERTAIN and type(child.pid) is int and child.pid > 0
            and child.returncode is None and type(seconds) is int and seconds in (12, 45, 180))
    deadline = min(SCOPE_DEADLINE, time.monotonic() + seconds)
    while time.monotonic() < deadline:
        seen = os.waitid(os.P_PID, child.pid, os.WEXITED | os.WNOHANG | os.WNOWAIT)
        if seen is None:
            time.sleep(0.05)
            continue
        require(type(seen.si_pid) is int and seen.si_pid == child.pid
                and type(seen.si_code) is int and seen.si_code == os.CLD_EXITED
                and type(seen.si_status) is int and seen.si_status == 0
                and time.monotonic() < deadline)
        result = os.waitpid(child.pid, os.WNOHANG)
        require(type(result) is tuple and len(result) == 2
                and type(result[0]) is int and result[0] == child.pid
                and type(result[1]) is int and result[1] == 0)
        require(time.monotonic() < deadline)
        child.returncode = 0
        return 0
    # No signal/reap or subsequent unit command after a timeout or query error.
    raise Refused()


# Definitions only: the separately pinned observer owns the invocation.
