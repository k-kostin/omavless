#!/usr/bin/python3
# SPDX-License-Identifier: MIT
"""Attended installed-owner DNS broker gate for an isolated Omarchy test VM.

This does not install, enroll, start, stop or recover the experimental broker.
The exact installed pair and an existing private VLESS profile are prerequisites.
Every network mutation has its own ready/settled barrier. Refusing one leaves
the current state for explicit inspection; no automatic recovery is attempted.
Only fixed classifications, counts and booleans are printed.
"""
import argparse
import hashlib
import importlib.util
import os
from pathlib import Path
import re
import stat
import subprocess


def sibling(name):
    spec = importlib.util.spec_from_file_location(name, Path(__file__).with_name(name + ".py"))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


gate = sibling("native_service_acceptance")
installed = sibling("installed_native_acceptance")
auth = sibling("human_authorization")

CORE_PATH = Path("/usr/lib/omavless-dns-experimental/mihomo")
BROKER_PATH = Path("/usr/lib/omavless/omavless-dns-broker")
RUNTIME = "omavless-runtime.service"
MODES = ("global", "rule", "direct", "global")


def require(condition, code):
    gate.require(condition, code)


def fixed_command(args):
    result = subprocess.run(args, stdin=subprocess.DEVNULL, capture_output=True, timeout=10)
    require(len(result.stdout) + len(result.stderr) <= 16384 and result.returncode == 0,
            "host_observation_unavailable")
    return result


def selected_profile(state):
    profiles = state.get("profiles")
    require(isinstance(profiles, list) and len(profiles) <= 256, "private_state_unavailable")
    for profile in profiles:
        if (isinstance(profile, dict) and profile.get("id") == state.get("lastProfileId")
                and profile.get("protocol") == "vless" and profile.get("missing") is False):
            return profile["id"]
    raise gate.Failure("vless_fixture_unavailable")


def installed_identity(expected_sha):
    require(re.fullmatch(r"[0-9a-f]{64}", expected_sha or "") is not None,
            "core_pin_required")
    require(installed.valid_environment(os.environ, Path.home(),
            Path("/run/user") / str(os.getuid())), "environment_mismatch")
    binary = Path(installed.BINARY).lstat()
    require(stat.S_ISREG(binary.st_mode) and binary.st_uid == 0
            and not binary.st_mode & 0o022, "installed_binary_unsafe")
    require(installed.command([installed.BINARY, "plugin", "target"]) == b"rust\n",
            "not_native_owner")
    require(gate.experimental_core(expected_sha) == str(CORE_PATH), "core_pin_mismatch")
    pid = int(installed.unit(RUNTIME, "MainPID"))
    require(pid > 1 and installed.unit(RUNTIME, "ActiveState") == "active",
            "runtime_unavailable")
    environment = gate.bounded(Path("/proc") / str(pid) / "environ", 65536).split(b"\0")
    require(b"OMAVLESS_MIHOMO=" + os.fsencode(CORE_PATH) in environment,
            "installed_core_not_selected")
    return pid


def broker_identity(expected_sha):
    require(re.fullmatch(r"[0-9a-f]{64}", expected_sha or "") is not None,
            "broker_pin_required")
    require(BROKER_PATH.resolve(strict=True) == BROKER_PATH, "broker_path_unsafe")
    for parent in BROKER_PATH.parents:
        metadata = parent.lstat()
        require(stat.S_ISDIR(metadata.st_mode) and metadata.st_uid == 0
                and not metadata.st_mode & 0o022, "broker_path_unsafe")
    metadata = BROKER_PATH.lstat()
    require(stat.S_ISREG(metadata.st_mode) and metadata.st_uid == 0
            and metadata.st_nlink == 1 and not metadata.st_mode & 0o022
            and 0 < metadata.st_size <= 32 * 1024 * 1024, "broker_path_unsafe")
    require(hashlib.sha256(BROKER_PATH.read_bytes()).hexdigest() == expected_sha,
            "broker_pin_mismatch")
    service = "omavless-dns-broker.service"
    pid = int(fixed_command(["/usr/bin/systemctl", "--system", "show", service,
                             "--property=MainPID", "--value"]).stdout.strip())
    require(pid > 1, "broker_service_unavailable")
    actual = (Path("/proc") / str(pid) / "exe").stat()
    pinned = BROKER_PATH.stat()
    require((actual.st_dev, actual.st_ino) == (pinned.st_dev, pinned.st_ino),
            "running_broker_not_pinned")


def broker_state(expected):
    gate.broker_evidence(expected, fixed_command)


def connected_state(runtime_pid, mode):
    observed = installed.cli("runtime", "observation")["result"]
    require(observed.get("availability") == "observed"
            and observed.get("lastKnownActual") == "connected"
            and observed.get("manualRecoveryRequired") is False,
            "connected_state_unverified")
    require(installed.cli("plugin", "snapshot")["result"]["desired"]["mode"] == mode,
            "mode_unconfirmed")
    require(installed.tuns() == {"Meta"}, "managed_tun_unverified")
    group = (Path("/sys/fs/cgroup") /
             installed.unit(RUNTIME, "ControlGroup").lstrip("/")).resolve(strict=True)
    require(group.is_relative_to(Path("/sys/fs/cgroup"))
            and runtime_pid in set(map(int, gate.bounded(group / "cgroup.procs").split())),
            "runtime_cgroup_unverified")
    cores = {member for member in set(map(int, gate.bounded(group / "cgroup.procs").split()))
             if gate.processes().get(member) == b"mihomo"}
    require(len(cores) == 1 and cores <= gate.descendants(runtime_pid),
            "owned_core_unverified")
    core = next(iter(cores))
    actual, pinned = Path("/proc") / str(core) / "exe", CORE_PATH
    require((actual.stat().st_dev, actual.stat().st_ino) ==
            (pinned.stat().st_dev, pinned.stat().st_ino), "running_core_not_pinned")
    gate.core_controller(Path("/run/user") / str(os.getuid()) / "omavless/mihomo.sock",
                         core, mode, managed=True)
    broker_state(1)
    return core


def mode_sequence(authorization, change, verify):
    for mode in MODES[1:]:
        authorization.step("mode_change", lambda mode=mode: change(mode))
        verify(mode)


def run_gate(authorization, expected_sha, broker_sha):
    authorization.require_terminal()  # before private state or host observation
    runtime_pid = installed_identity(expected_sha)
    broker_identity(broker_sha)
    initial = installed.cli("runtime", "observation")["result"]
    require(installed.clean_disconnected_observation(initial) and not installed.tuns(),
            "baseline_not_disconnected")
    broker_state(0)
    snapshot = installed.cli("plugin", "snapshot")["result"]
    require(snapshot["startup"]["enabled"] is False, "startup_not_disabled")
    profile = selected_profile(snapshot)
    original_mode = snapshot["desired"]["mode"]
    require(original_mode in ("global", "rule", "direct"), "original_mode_unavailable")
    gate.emit(case="installed-managed-dns", preflight=True,
              mode_sequence=list(MODES), authorization="attended")
    connected = False
    completed = False
    try:
        authorization.step("connect", lambda: installed.action("connect", profile, "global"))
        connected = True
        connected_state(runtime_pid, "global")
        before = gate.tun_counters("Meta")
        probe = subprocess.run(gate.https_probe_args("Meta"), stdin=subprocess.DEVNULL,
                               capture_output=True, timeout=25)
        https, tun_used, classification = gate.https_probe_evidence(
            probe, before, gate.tun_counters("Meta"))
        gate.emit(full_vpn_https=https, tun_used=tun_used, classification=classification)
        require(https and tun_used, "full_vpn_https_failed")
        mode_sequence(authorization, lambda mode: installed.action("mode", mode),
                      lambda mode: connected_state(runtime_pid, mode))
        completed = True
    finally:
        if authorization.blocked:
            gate.emit(passed=False, cleanup=False,
                      classification="human_authorization_unsettled")
            raise auth.AuthorizationUnsettled()
        # A rejected/unknown Connect can still have host effects. Its separately
        # attended Disconnect must not be skipped just because CLI rejected it.
        authorization.step("disconnect", lambda: installed.action("disconnect"))
        clean = installed.clean_disconnected_observation(
            installed.cli("runtime", "observation")["result"])
        require(clean and not installed.tuns(), "cleanup_unverified")
        broker_state(0)
        if installed.cli("plugin", "snapshot")["result"]["desired"]["mode"] != original_mode:
            authorization.step("restore_mode", lambda: installed.action("mode", original_mode))
        restored = installed.cli("plugin", "snapshot")["result"]["desired"]
        require(restored["connected"] is False and restored["mode"] == original_mode,
                "restoration_unverified")
        gate.emit(disconnected=True, dns_released=True, mode_restored=True,
                  passed=completed and connected)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--run", action="store_true")
    parser.add_argument("--experimental-core-sha", metavar="SHA256")
    parser.add_argument("--experimental-broker-sha", metavar="SHA256")
    args = parser.parse_args(argv)
    if not args.run:
        gate.emit(status="NOT RUN", reason="explicit_run_required")
        return 0
    try:
        run_gate(auth.HumanAuthorization(), args.experimental_core_sha,
                 args.experimental_broker_sha)
        return 0
    except Exception as error:
        allowed = {"vless_fixture_unavailable", "baseline_not_disconnected",
                   "full_vpn_https_failed", "cleanup_unverified", "restoration_unverified"}
        classification = ("human_authorization_unsettled"
                          if isinstance(error, auth.AuthorizationUnsettled)
                          else str(error) if isinstance(error, gate.Failure)
                          and str(error) in allowed else "installed_dns_gate_failed")
        gate.emit(passed=False, classification=classification)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
