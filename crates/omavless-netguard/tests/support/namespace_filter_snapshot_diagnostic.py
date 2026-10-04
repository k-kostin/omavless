#!/usr/bin/python3
"""Hash-pinned original snapshot only. Never start/reload/stop a unit or runner."""
import hashlib
import json
import os
from pathlib import Path
import stat
import sys

ORIGINAL = Path("/run/omavless-k1-namespace-filter-fixture")
DESTINATION = Path("/run/omavless-k1-namespace-filter-diagnostic")
ORIGINAL_SHA = "c4a688875037f1d990ee93f0018108ab82bceb211090f61f77c64b83a4c7cc40"
SOURCE_LABEL = "<k1-original-guard-191c67e>"
ARTIFACTS = {
    "probe": ("b7dc81b89045c591efd375765ddf4fc4792afedf86cbc73c18fd94d1227d7332", 0o700),
    "runner.sh": ("f3e064489cab6e66346f8d419cd8e8200ba50db019cd7b3e6af141df163e6e2d", 0o600),
    "guard.py": (ORIGINAL_SHA, 0o600),
    "control.service": ("1f24f558019a60923531f4ad673642e2878f2b77391e60bc03ac5d0cea48e7b8", 0o600),
    "filtered.service": ("8648d11b754e287fb8e6d64e5ba0b9e2bf2f4adb6b968c116d011ec57aca5f0c", 0o600),
}


class DiagnosticRefused(Exception):
    pass


def require(value):
    if not value:
        raise DiagnosticRefused()


def load_original(data):
    require(hashlib.sha256(data).hexdigest() == ORIGINAL_SHA)
    namespace = {"__name__": "retained_original_guard", "__file__": SOURCE_LABEL}
    exec(compile(data, SOURCE_LABEL, "exec"), namespace)
    return namespace


def commands():
    user = ("/usr/bin/runuser", "-u", "kdk_vm", "--", "/usr/bin/env",
            "XDG_RUNTIME_DIR=/run/user/1000", "/usr/bin/systemctl", "--user", "show",
            "omavless-runtime.service")
    result = {
        ("/usr/bin/systemd-detect-virt", "--vm"): "vm_identity",
        ("/usr/bin/resolvectl", "status", "--no-pager"): "resolver",
        ("/usr/bin/pacman", "-Q"): "packages",
        ("/usr/bin/systemctl", "show-environment"): "manager_environment",
        ("/usr/bin/systemctl", "--version"): "systemd_identity",
        ("/usr/bin/uname", "-srvm"): "kernel_identity",
    }
    for field in ("ActiveState", "SubState", "MainPID"):
        result[(*user, "-p", field)] = "canonical_service"
    result[(*user, "-p", "FragmentPath", "-p", "DropInPaths", "-p", "ExecStart",
            "-p", "EnvironmentFiles", "-p", "PassEnvironment", "-p", "Requires",
            "-p", "Wants", "-p", "Before", "-p", "After", "-p", "UnitFileState")] = "canonical_graph"
    for name, args in {
        "addresses": ("-j", "address", "show"),
        "routes4": ("-j", "route", "show", "table", "all"),
        "rules4": ("-j", "rule", "show"),
        "routes6": ("-6", "-j", "route", "show", "table", "all"),
        "rules6": ("-6", "-j", "rule", "show"),
    }.items():
        result[("/usr/bin/ip", *args)] = name
    for name in ARTIFACTS:
        result[("/usr/bin/getcap", str(ORIGINAL / name))] = "artifact_capabilities"
    for path in ("/usr/bin/omavless", "/usr/bin/mihomo", "/usr/lib/omavless-dns/omavless-dns-broker"):
        result[("/usr/bin/getcap", path)] = "installed_capabilities"
    return result


def public_failure(error, guard, command_phase, default_phase):
    # Only fixed codes and line numbers from hash-pinned public source. Never
    # stringify exceptions, inspect traceback locals, or publish path values.
    phase, line = default_phase, 0
    trace = error.__traceback__
    while trace is not None:
        frame, number = trace.tb_frame, trace.tb_lineno
        if frame.f_code.co_filename == SOURCE_LABEL:
            name = frame.f_code.co_name
            if name in {"command", "spawn", "await_child", "await_child_once"}:
                phase, line = "command_" + command_phase, number
            elif name == "pinned_file":
                phase, line = "artifact_validation", number
            elif name == "visit":
                phase = "activation_inventory"
                if number in {148, 149, 150}:
                    phase = "activation_target_outside_allowlist"
                elif number == 147:
                    phase = "activation_target_resolution"
                line = number
            elif name == "snapshot":
                line = number
                if number <= 191: phase = "canonical_service"
                elif number <= 197: phase = "canonical_executable"
                elif number <= 203: phase = "namespace_relationship"
                elif number <= 214: phase = "core_inventory"
                elif number == 215: phase = "network_shape"
                elif number == 216: phase = "tun_inventory"
                elif number == 223: phase = "private_hash_inventory"
                elif number <= 232: phase = "installed_identity"
                elif number == 237: phase = "resolver_file"
                elif number == 241: phase = "activation_inventory"
        trace = trace.tb_next
    if guard["UNCERTAIN"]:
        category = "process_uncertain"
    elif isinstance(error, (KeyboardInterrupt, SystemExit)):
        category = "interrupted"
    elif isinstance(error, (DiagnosticRefused, guard["Refused"])):
        category = "refused"
    elif isinstance(error, FileNotFoundError):
        category = "missing"
    elif isinstance(error, PermissionError):
        category = "permission"
    elif isinstance(error, OSError):
        category = "os_error"
    else:
        category = "unknown"
    return {"phase": phase, "category": category, "original_source_line": line}


def diagnose(guard):
    original_command = guard["command"]
    allowed = commands()
    phase = "preflight"
    command_phase = "not_called"

    def checked_command(args):
        nonlocal command_phase
        require(not guard["UNCERTAIN"])
        command_phase = allowed.get(tuple(args), "unrecognized")
        require(command_phase != "unrecognized")
        return original_command(args)

    guard["command"] = checked_command
    result = {"schema": "k1-original-snapshot-diagnostic-v1", "original_guard_sha256": ORIGINAL_SHA,
              "snapshot_complete": False, "runner_invoked": False, "normal_authority": False}
    try:
        require(not guard["UNCERTAIN"])
        require(checked_command(["/usr/bin/systemd-detect-virt", "--vm"]).strip() == b"kvm")
        for name, (expected, mode) in ARTIFACTS.items():
            phase = "artifact_validation"
            guard["pinned_file"](ORIGINAL / name, expected, mode)
        phase = "snapshot"
        guard["snapshot"]()  # Discard private observations; only completion is public.
        result.update(snapshot_complete=True, phase="snapshot_complete", category="complete",
                      original_source_line=0)
    except BaseException as error:
        result.update(public_failure(error, guard, command_phase, phase))
    result["process_uncertain"] = guard["UNCERTAIN"]
    return result


def main():
    require(os.geteuid() == 0 and len(sys.argv) == 1
            and os.environ.get("OMAVLESS_K1_SNAPSHOT_DIAGNOSTIC") == "1")
    for directory in (ORIGINAL, DESTINATION):
        info = directory.lstat()
        require(stat.S_ISDIR(info.st_mode) and info.st_uid == info.st_gid == 0
                and stat.S_IMODE(info.st_mode) == 0o700)
    require(set(path.name for path in ORIGINAL.iterdir()) == set(ARTIFACTS))
    link = Path("/run/systemd/system/omavless-k1-namespace-filter-fixture.service")
    require(not link.exists() and not link.is_symlink())
    require(not Path("/sys/fs/cgroup/system.slice/omavless-k1-namespace-filter-fixture.service").exists())
    path = ORIGINAL / "guard.py"
    descriptor = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_CLOEXEC)
    with os.fdopen(descriptor, "rb") as stream:
        info = os.fstat(stream.fileno())
        require(stat.S_ISREG(info.st_mode) and info.st_uid == info.st_gid == 0
                and stat.S_IMODE(info.st_mode) == 0o600 and info.st_nlink == 1)
        data = stream.read(256 * 1024 + 1)
    require(len(data) <= 256 * 1024)
    result = diagnose(load_original(data))
    descriptor = os.open(DESTINATION / "result.json", os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
    with os.fdopen(descriptor, "w") as stream:
        json.dump(result, stream, sort_keys=True)
    print(json.dumps(result, sort_keys=True))
    return 0 if result["snapshot_complete"] else 2


if __name__ == "__main__":
    os.umask(0o077)
    try:
        sys.exit(main())
    except Exception:
        print("K1_SNAPSHOT_DIAGNOSTIC_ENTRY_REFUSED", file=sys.stderr)
        sys.exit(2)
