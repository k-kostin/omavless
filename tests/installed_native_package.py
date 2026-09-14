#!/usr/bin/python3
# SPDX-License-Identifier: MIT
"""Attended, opt-in Arch package recovery TEST TOOL; not a product dependency.

No VPN action, store/marker write, arbitrary command, dependency bypass, automatic
rollback or automatic retry. A failure leaves the precise stage visible for an
attended recovery using the retained current archive. Never run this as root.
"""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import pwd
import re
import selectors
import stat
import subprocess
import sys
import time

_spec = importlib.util.spec_from_file_location(
    "package_human_authorization", Path(__file__).with_name("human_authorization.py"))
auth = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(auth)

BINARY = Path("/usr/bin/omavless")
UNIT = "omavless-runtime.service"
UNITS = (UNIT, "omavless-login-prepare.service")
ARCHIVE_LIMIT = 256 * 1024 * 1024
PRIVATE_LIMIT = 8 * 1024 * 1024
PAYLOAD = frozenset((
    ".BUILDINFO", ".MTREE", ".PKGINFO", "usr/bin/omavless",
    "usr/lib/systemd/user/omavless-runtime.service",
    "usr/lib/systemd/user/omavless-login-prepare.service",
    "usr/share/doc/omavless/README.md", "usr/share/doc/omavless/build-identity.txt",
    "usr/share/licenses/omavless/LICENSE",
    "usr/share/licenses/omavless/THIRD_PARTY_NOTICES.md"))
PRIVATE = (".config/omavless/profiles.json", ".local/state/omavless/desired.json",
           ".local/state/omavless/ownership.json",
           ".local/state/omavless/frontend-bridge.target")
OPTIONAL_PRIVATE = (".config/omavless/route-template.yaml", ".config/omavless/config.yaml")


class Refused(Exception):
    """Only fixed public codes are supplied by this module."""


def require(condition, code):
    if not condition:
        raise Refused(code)


def emit(stage, passed, **facts):
    print(json.dumps(dict(stage=stage, passed=passed, **facts), sort_keys=True), flush=True)


class PackageAuthorization(auth.HumanAuthorization):
    # Same real-terminal/pre/post rules; no resetting an unsettled invocation.
    PHASES = auth.HumanAuthorization.PHASES | {"package_install", "package_remove"}


def capture(argv, limit=1024 * 1024, timeout=30, digest=False):
    """Bound stdout before allocating; never print child stderr or raw errors."""
    env = dict(os.environ, LC_ALL="C")
    process = subprocess.Popen(argv, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                               stderr=subprocess.DEVNULL, env=env)
    output, total, end = bytearray(), 0, time.monotonic() + timeout
    hasher = hashlib.sha256()
    try:
        with selectors.DefaultSelector() as poll:
            poll.register(process.stdout, selectors.EVENT_READ)
            while True:
                remaining = end - time.monotonic()
                require(remaining > 0, "command_timeout")
                require(poll.select(remaining), "command_timeout")
                chunk = os.read(process.stdout.fileno(), 65536)
                if not chunk:
                    break
                total += len(chunk)
                require(total <= limit, "command_output_bound")
                if digest:
                    hasher.update(chunk)
                else:
                    output.extend(chunk)
        require(process.wait(timeout=max(0.01, end - time.monotonic())) == 0,
                "command_failed")
        return hasher.hexdigest() if digest else bytes(output)
    finally:
        if process.poll() is None:
            process.kill()
        process.wait()
        process.stdout.close()


def safe_parents(path, uid):
    require(path.is_absolute() and path == Path(os.path.normpath(path)), "unsafe_path")
    for parent in path.parents:
        info = parent.lstat()
        require(stat.S_ISDIR(info.st_mode) and info.st_uid in (0, uid)
                and not info.st_mode & 0o022, "unsafe_parent")


def fingerprint(path, uid, private=False, follow_proc=False):
    if not follow_proc:
        safe_parents(path, uid)
    flags = os.O_RDONLY | os.O_NONBLOCK
    if not follow_proc:
        flags |= os.O_NOFOLLOW
    fd = os.open(path, flags)
    try:
        before = os.fstat(fd)
        require(stat.S_ISREG(before.st_mode) and before.st_uid == uid,
                "unsafe_file")
        mode = stat.S_IMODE(before.st_mode)
        require(mode == 0o600 if private else not mode & 0o022, "unsafe_mode")
        require(before.st_size <= (PRIVATE_LIMIT if private else ARCHIVE_LIMIT), "file_bound")
        digest = hashlib.sha256()
        total = 0
        while chunk := os.read(fd, 65536):
            total += len(chunk)
            require(total <= (PRIVATE_LIMIT if private else ARCHIVE_LIMIT), "file_bound")
            digest.update(chunk)
        after = os.fstat(fd)
        require((before.st_dev, before.st_ino, before.st_size, before.st_mtime_ns,
                 before.st_ctime_ns) == (after.st_dev, after.st_ino, after.st_size,
                                       after.st_mtime_ns, after.st_ctime_ns), "file_changed")
        return (digest.hexdigest(), mode, before.st_uid)
    finally:
        os.close(fd)


def key_values(raw, separator, required, exact=False):
    require(len(raw) <= 16384, "metadata_bound")
    values = {}
    try:
        lines = raw.decode("utf-8", "strict").splitlines()
    except UnicodeError:
        raise Refused("metadata_invalid") from None
    for line in lines:
        if not line or line.startswith("#"):
            continue
        require(separator in line, "metadata_invalid")
        key, value = line.split(separator, 1)
        if key in required:
            require(key not in values, "metadata_duplicate")
            values[key] = value
        elif exact:
            raise Refused("metadata_invalid")
    require(set(values) == set(required), "metadata_missing")
    return values


def validate_members(raw):
    require(len(raw) <= 16384, "metadata_bound")
    try:
        entries = raw.decode("utf-8", "strict").splitlines()
    except UnicodeError:
        raise Refused("archive_members_invalid") from None
    require(len(entries) <= 64 and len(entries) == len(set(entries)), "archive_members_invalid")
    files = set()
    for entry in entries:
        require(entry and not entry.startswith("/") and ".." not in entry.split("/"),
                "archive_members_invalid")
        if entry.endswith("/"):
            require(any(member.startswith(entry) for member in PAYLOAD), "archive_members_invalid")
        else:
            require(entry in PAYLOAD, "archive_members_invalid")
            files.add(entry)
    require(files == PAYLOAD, "archive_members_invalid")


def validate_listing(raw):
    # LC_ALL=C bsdtar's portable verbose listing. Exact modes exclude set-id,
    # writable payloads, links/devices, and special extraction members.
    lines = raw.splitlines()
    require(0 < len(lines) <= 64, "archive_member_type")
    for line in lines:
        fields = line.split()
        require(len(fields) == 9 and fields[0] in (b"drwxr-xr-x", b"-rw-r--r--", b"-rwxr-xr-x")
                and fields[2] in (b"root", b"0") and fields[3] in (b"root", b"0"),
                "archive_member_type")


def validate_build_identity(package, raw):
    schema = key_values(raw, "=", {"schemaVersion"})["schemaVersion"]
    require(schema in ("1", "2"), "build_identity")
    keys = {"schemaVersion", "sourceCommit", "binarySha256", "architecture", "provenance"}
    if schema == "2":
        keys.add("productVersion")
    identity = key_values(raw, "=", keys, True)
    require(identity["architecture"] == package["arch"]
            and re.fullmatch(r"[0-9a-f]{40}", identity["sourceCommit"])
            and re.fullmatch(r"[0-9a-f]{64}", identity["binarySha256"])
            and identity["provenance"] == "caller-supplied-prebuilt", "build_identity")
    if schema == "1":
        require(re.fullmatch(r"0\.0\.0\.r[0-9]+\.g[0-9a-f]{12}-[0-9]+", package["pkgver"]),
                "package_version")
        require(".g" + identity["sourceCommit"][:12] + "-" in package["pkgver"], "build_identity")
    else:
        version = identity["productVersion"]
        require(re.fullmatch(r"[0-9]+\.[0-9]+\.[0-9]+-rc\.[1-9][0-9]*", version), "package_version")
        require(package["pkgver"] == version.replace("-rc.", "rc") + "-1", "package_version")
    return identity


def inspect_archive(path):
    require(path.name.endswith(".pkg.tar.zst"), "archive_type")
    uid = path.lstat().st_uid
    require(uid in (0, os.getuid()), "archive_owner")
    mark = fingerprint(path, uid)
    validate_members(capture(["/usr/bin/bsdtar", "-tf", str(path)], limit=16384))
    # Listing permits only regular payload members and directories; reject links,
    # device nodes and package install scripts before sudo pacman ever sees it.
    listing = capture(["/usr/bin/bsdtar", "-tvf", str(path)], limit=32768)
    validate_listing(listing)
    def member(name, digest=False):
        return capture(["/usr/bin/bsdtar", "-xOf", str(path), name],
                       limit=ARCHIVE_LIMIT if digest else 16384, digest=digest)
    package = key_values(member(".PKGINFO"), " = ", {"pkgname", "pkgver", "arch"})
    require(package["pkgname"] == "omavless" and package["arch"] == os.uname().machine,
            "package_identity")
    identity = validate_build_identity(package, member("usr/share/doc/omavless/build-identity.txt"))
    require(member("usr/bin/omavless", True) == identity["binarySha256"], "archive_binary_mismatch")
    units = {name: member("usr/lib/systemd/user/" + name, True) for name in UNITS}
    require(fingerprint(path, uid) == mark, "archive_changed")
    return dict(path=path, fingerprint=mark, version=package["pkgver"],
                binary=identity["binarySha256"], source=identity["sourceCommit"], units=units)


def private_snapshot(home):
    uid = os.getuid()
    for directory in (home / ".config/omavless", home / ".local/state/omavless"):
        info = directory.lstat()
        require(stat.S_ISDIR(info.st_mode) and info.st_uid == uid
                and stat.S_IMODE(info.st_mode) == 0o700, "private_directory_unsafe")
    result = {name: fingerprint(home / name, uid, True) for name in PRIVATE}
    for name in OPTIONAL_PRIVATE:
        result[name] = fingerprint(home / name, uid, True) if os.path.lexists(home / name) else None
    return result


def clean_observation(value):
    if not isinstance(value, dict):
        return False
    facts, desired = value.get("facts"), value.get("desired")
    return (value.get("availability") == "observed"
            and value.get("lastKnownActual") == "disconnected"
            and value.get("manualRecoveryRequired") is False
            and isinstance(desired, dict) and desired.get("connected") is False
            and isinstance(facts, dict) and facts.get("ownedCoreRunning") is False
            and all(type(facts.get(key)) is int and facts[key] == 0 for key in
                    ("visibleMihomoCount", "visibleTunCount", "ownedAuxiliaryMihomoCount")))


def cli(*args):
    value = json.loads(capture([str(BINARY), *args]))
    require(isinstance(value, dict) and value.get("ok") is True, "native_read_failed")
    return value["result"]


def process_inventory(expected_pid=None):
    """Refuse other users' native owners and every visible core, never kill them."""
    found = []
    for item in Path("/proc").iterdir():
        if not item.name.isdigit():
            continue
        try:
            name = (item / "comm").read_bytes().strip()
            uid = item.stat().st_uid
        except FileNotFoundError:
            continue
        require(name != b"mihomo", "core_present")
        if name == b"omavless":
            require(uid == os.getuid(), "other_user_runtime")
            found.append(int(item.name))
    require(found == ([] if expected_pid is None else [expected_pid]), "runtime_process_count")
    links = json.loads(capture(["/usr/bin/ip", "-d", "-j", "link", "show"]))
    require(not any(row.get("linkinfo", {}).get("info_kind") == "tun" for row in links), "tun_present")
    require(capture(["/usr/bin/systemctl", "--user", "show", "omavless.service",
                     "--property=ActiveState", "--value"]).strip() == b"inactive", "legacy_service_active")


def unit(field):
    require(field in {"MainPID", "ActiveState", "UnitFileState"}, "unit_field")
    return capture(["/usr/bin/systemctl", "--user", "show", UNIT,
                    "--property=" + field, "--value"]).decode("ascii").strip()


class PackageGate:
    def __init__(self, current, rollback, authorization=None):
        self.current, self.rollback = current, rollback
        self.authorization = authorization or PackageAuthorization()
        self.home = Path(pwd.getpwuid(os.getuid()).pw_dir)
        self.before = None
        self.removed = False

    def unchanged(self):
        require(private_snapshot(self.home) == self.before, "private_state_changed")

    def archive_unchanged(self, package):
        require(fingerprint(package["path"], package["fingerprint"][2]) == package["fingerprint"],
                "archive_changed")

    def check_running(self, package):
        require(capture(["/usr/bin/pacman", "-Q", "omavless"]).decode().strip()
                == "omavless " + package["version"], "installed_version")
        require(fingerprint(BINARY, 0)[0] == package["binary"], "installed_binary")
        require(all(fingerprint(Path("/usr/lib/systemd/user", name), 0)
                    == (package["units"][name], 0o644, 0) for name in UNITS), "installed_units")
        require(unit("ActiveState") == "active", "runtime_not_active")
        pid = int(unit("MainPID"))
        require(pid > 1 and os.readlink(f"/proc/{pid}/exe") == str(BINARY), "runtime_executable")
        require(fingerprint(Path(f"/proc/{pid}/exe"), 0, follow_proc=True)[0] == package["binary"],
                "running_binary")
        require(capture([str(BINARY), "plugin", "target"]) == b"rust\n", "not_native_owner")
        startup = cli("plugin", "snapshot")["startup"]
        require(startup.get("configured") is True and startup.get("enabled") is False, "startup_not_off")
        require(clean_observation(cli("runtime", "observation")), "runtime_not_clean")
        process_inventory(pid)
        require(not os.path.lexists(f"/run/user/{os.getuid()}/omavless/mihomo.sock"), "controller_present")
        self.unchanged()

    def stopped(self):
        require(unit("ActiveState") == "inactive" and unit("MainPID") == "0", "service_not_stopped")
        process_inventory()
        require(not os.path.lexists(f"/run/user/{os.getuid()}/omavless/mihomo.sock"), "controller_present")
        self.unchanged()

    def absent(self):
        # After pacman hooks/daemon-reload a missing unit can make `show` fail.
        # Absence is its own strict gate, never a fabricated inactive unit reply.
        result = subprocess.run(["/usr/bin/pacman", "-Q", "omavless"],
                                stdin=subprocess.DEVNULL, capture_output=True, timeout=15)
        require(result.returncode == 1 and len(result.stdout) + len(result.stderr) <= 4096,
                "package_still_installed")
        require(not os.path.lexists(BINARY)
                and all(not os.path.lexists(Path("/usr/lib/systemd/user", name)) for name in UNITS),
                "package_files_retained")
        process_inventory()
        require(not os.path.lexists(f"/run/user/{os.getuid()}/omavless/mihomo.sock"), "controller_present")
        self.unchanged()

    def stop(self, package):
        self.check_running(package)
        self.authorization.step("service_stop", lambda: capture(
            ["/usr/bin/systemctl", "--user", "stop", UNIT], timeout=125))
        self.stopped()

    def start(self, package):
        self.unchanged()
        capture(["/usr/bin/systemctl", "--user", "daemon-reload"])
        self.authorization.step("service_start", lambda: capture(
            ["/usr/bin/systemctl", "--user", "start", UNIT], timeout=125))
        # Reads only while normal startup settles; no transition retries.
        end = time.monotonic() + 10
        while True:
            try:
                self.check_running(package)
                break
            except (Refused, OSError, ValueError):
                if time.monotonic() >= end:
                    raise
                time.sleep(0.1)

    def pacman(self, package=None):
        if self.removed:
            require(package is self.current, "recovery_candidate_only")
            self.absent()
        else:
            self.stopped()
        self.archive_unchanged(self.current)  # retained recovery always available
        if package is not None:
            self.archive_unchanged(package)
            args = ["/usr/bin/sudo", "/usr/bin/pacman", "-U", "--", str(package["path"])]
            phase = "package_install"
        else:
            args = ["/usr/bin/sudo", "/usr/bin/pacman", "-R", "--", "omavless"]
            phase = "package_remove"
        def transaction():
            # Normal terminal auth + pacman confirmation. No timeout/password I/O.
            require(subprocess.run(args, env=dict(os.environ, LC_ALL="C")).returncode == 0,
                    "package_transaction_failed")
        self.authorization.step(phase, transaction)
        self.unchanged()

    def preflight(self, installed):
        self.authorization.require_terminal()
        require(os.getuid() != 0, "do_not_run_as_root")
        require(os.environ.get("HOME") == str(self.home) and "OMAVLESS_HOME" not in os.environ
                and os.environ.get("XDG_RUNTIME_DIR") == f"/run/user/{os.getuid()}"
                and os.environ.get("XDG_CONFIG_HOME", str(self.home / ".config")) == str(self.home / ".config")
                and os.environ.get("XDG_STATE_HOME", str(self.home / ".local/state")) == str(self.home / ".local/state"),
                "environment_mismatch")
        self.before = private_snapshot(self.home)
        self.check_running(installed)
        enabled = unit("UnitFileState")
        require(enabled == "enabled", "runtime_not_enabled")
        require(self.current["binary"] != self.rollback["binary"]
                and capture(["/usr/bin/vercmp", self.rollback["version"], self.current["version"]]).strip() == b"-1",
                "rollback_not_older")
        emit("preflight", True, current_source=self.current["source"], rollback_source=self.rollback["source"],
             current_archive=self.current["fingerprint"][0], rollback_archive=self.rollback["fingerprint"][0],
             current_binary=self.current["binary"], rollback_binary=self.rollback["binary"])
        return enabled

    def upgrade(self):
        """One attended update, not a destructive recovery rehearsal.

        Start on the verified rollback archive, already disconnected. Each
        effect uses the same human barrier; failure never triggers recovery.
        """
        enabled = self.preflight(self.rollback)
        self.stop(self.rollback)
        self.pacman(self.current)
        self.start(self.current)
        require(unit("UnitFileState") == enabled, "enablement_changed")
        emit("upgrade_restart", True, private_state_preserved=True, actual_binary=True,
             disconnected=True, startup_off=True, enabled=True)

    def run(self):
        enabled = self.preflight(self.current)
        self.stop(self.current)
        self.pacman(self.rollback)
        self.start(self.rollback)
        emit("downgrade_restart", True, private_state_preserved=True, actual_binary=True)
        self.stop(self.rollback)
        self.pacman(self.current)
        self.start(self.current)
        emit("upgrade_restart", True, private_state_preserved=True, actual_binary=True)
        self.stop(self.current)
        self.pacman()
        self.removed = True
        self.absent()
        # pacman -R keeps application private state and the user's enablement link.
        # This gate never uses --nosave, removes dependencies, or rewrites markers.
        emit("package_remove", True, private_state_preserved=True, no_runtime_core_tun=True)
        self.pacman(self.current)
        self.removed = False
        self.start(self.current)
        require(unit("UnitFileState") == enabled, "enablement_changed")
        emit("package_recovery", True, private_state_preserved=True, actual_binary=True,
             disconnected=True, startup_off=True, enabled=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--run", action="store_true")
    parser.add_argument("--upgrade-only", action="store_true",
                        help="update from the installed rollback archive to current; no removal/downgrade")
    parser.add_argument("--current-package", type=Path, required=True)
    parser.add_argument("--rollback-package", type=Path, required=True)
    args = parser.parse_args()
    if not args.run:
        parser.error("explicit --run is required; read the package recovery procedure first")
    try:
        authorization = PackageAuthorization()
        authorization.require_terminal()
        require(os.getuid() != 0, "do_not_run_as_root")
        current, rollback = inspect_archive(args.current_package), inspect_archive(args.rollback_package)
        instance = PackageGate(current, rollback, authorization)
        if args.upgrade_only:
            instance.upgrade()
        else:
            instance.run()
        return 0
    except auth.AuthorizationUnsettled:
        emit("stopped", False, classification="human_authorization_unsettled",
             recovery="inspect_package_and_host_before_new_attended_action")
    except Refused as error:
        emit("stopped", False, classification=str(error),
             recovery="retained_current_archive_available_no_automatic_compensation")
    except (Exception, KeyboardInterrupt):
        emit("stopped", False, classification="package_gate_failed",
             recovery="inspect_package_and_host_before_new_attended_action")
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
