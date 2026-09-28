#!/usr/bin/python3
# SPDX-License-Identifier: MIT
"""Attended installed-owner DNS broker gate for an isolated Omarchy test VM.

This does not install, enroll, start, stop or recover either broker package.
The exact installed pair and an existing private VLESS profile are prerequisites.
Every network mutation has its own ready/settled barrier. Refusing one leaves
the current state for explicit inspection; no automatic recovery is attempted.
Only fixed classifications, counts and booleans are printed.
The default remains the historical experimental pair; --pair release selects
only the production-name paths and requires its separately reviewed digests.
"""
import argparse
import hashlib
import http.client
import importlib.util
import json
import os
from pathlib import Path
import re
import signal
import socket
import stat
import struct
import subprocess
import time
import uuid


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
RELEASE_CORE_PATH = Path("/usr/lib/omavless-dns/mihomo")
RELEASE_BROKER_PATH = Path("/usr/lib/omavless-dns/omavless-dns-broker")
BROKER_UNIT = Path("/usr/lib/systemd/system/omavless-dns-broker.service")
BROKER_UNIT_SHA = "a82b31226f09dc0b8aa9d247a2919f66a2b4d46070356e56c21cf2df2672524f"
RELEASE_UNIT_SHA = "126b208158e9288c475084c8bd74029e2808edf9846da514a369266b6306462d"
RUNTIME = "omavless-runtime.service"
SELECTION_BYTES = b"managed-dns-source-pair-v1\n"
MODES = ("global", "rule", "direct", "global")
# The built-in isolated profile check uses these independent public HTTPS
# endpoints. A single website timeout is not evidence that the TUN is broken.
HTTPS_TARGETS = (
    "https://www.gstatic.com/generate_204",
    "https://cp.cloudflare.com/generate_204",
    "https://www.google.com/generate_204",
)
DIRECT_IP_HTTPS = "https://cloudflare-dns.com/cdn-cgi/trace"
DIRECT_IP_RESOLVE = "cloudflare-dns.com:443:1.1.1.1"
SAFE_FAILURES = frozenset((
    "vless_fixture_unavailable", "baseline_not_disconnected", "native_action_rejected",
    "connected_state_unverified",
    "mode_unconfirmed", "managed_tun_unverified", "runtime_cgroup_unverified",
    "owned_core_unverified", "running_core_not_pinned", "core_socket_identity",
    "core_socket_directory", "core_socket_peer", "core_controller_response",
    "core_mode", "broker_core_not_ready", "broker_service_unavailable", "broker_state_invalid",
    "broker_retention_mismatch", "broker_dns_readback_unavailable",
    "broker_dns_readback_mismatch", "full_vpn_https_failed", "cleanup_unverified",
    "restoration_unverified", "core_crash_unverified", "live_core_group_remains",
))


def require(condition, code):
    gate.require(condition, code)


def pair_paths(pair):
    require(pair in ("experimental", "release"), "pair_not_selected")
    if pair == "release":
        return RELEASE_CORE_PATH, RELEASE_BROKER_PATH, RELEASE_UNIT_SHA
    return CORE_PATH, BROKER_PATH, BROKER_UNIT_SHA


def release_core(expected_sha):
    """Pin the production-name package, never an experimental fallback."""
    require(re.fullmatch(r"[0-9a-f]{64}", expected_sha or "") is not None,
            "core_pin_required")
    path = RELEASE_CORE_PATH
    require(path.resolve(strict=True) == path, "core_path_unsafe")
    for parent in path.parents:
        metadata = parent.lstat()
        require(stat.S_ISDIR(metadata.st_mode) and metadata.st_uid == 0
                and not metadata.st_mode & 0o022, "core_path_unsafe")
    metadata = path.lstat()
    require(stat.S_ISREG(metadata.st_mode) and metadata.st_uid == 0
            and metadata.st_nlink == 1 and not metadata.st_mode & 0o022
            and 0 < metadata.st_size <= 128 * 1024 * 1024, "core_path_unsafe")
    require(hashlib.sha256(path.read_bytes()).hexdigest() == expected_sha,
            "core_pin_mismatch")
    return str(path)


def fixed_command(args):
    result = subprocess.run(args, stdin=subprocess.DEVNULL, capture_output=True, timeout=10)
    require(len(result.stdout) + len(result.stderr) <= 16384 and result.returncode == 0,
            "host_observation_unavailable")
    return result


def selected_profile(state, index=None):
    profiles = state.get("profiles")
    require(isinstance(profiles, list) and len(profiles) <= 256, "private_state_unavailable")
    eligible = [profile for profile in profiles if isinstance(profile, dict)
                and isinstance(profile.get("id"), str)
                and profile.get("protocol") == "vless"
                and profile.get("missing") is False]
    if index is not None:
        require(type(index) is int and 0 <= index < len(eligible),
                "vless_fixture_unavailable")
        return eligible[index]["id"]
    for profile in profiles:
        if (isinstance(profile, dict) and profile.get("id") == state.get("lastProfileId")
                and profile.get("protocol") == "vless" and profile.get("missing") is False):
            return profile["id"]
    raise gate.Failure("vless_fixture_unavailable")


def durable_core_selection(config_directory, uid):
    """Pin the normal post-reboot selector, not a transient systemd override."""
    try:
        parent = config_directory.lstat()
        before = (config_directory / "managed-dns-selection").lstat()
    except OSError as error:
        raise gate.Failure("installed_core_not_selected") from error
    require(stat.S_ISDIR(parent.st_mode) and parent.st_uid == uid
            and not parent.st_mode & 0o077, "installed_core_not_selected")
    path = config_directory / "managed-dns-selection"
    require(stat.S_ISREG(before.st_mode) and before.st_uid == uid
            and before.st_nlink == 1 and stat.S_IMODE(before.st_mode) == 0o600
            and before.st_size == len(SELECTION_BYTES), "installed_core_not_selected")
    try:
        descriptor = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_CLOEXEC)
    except OSError as error:
        raise gate.Failure("installed_core_not_selected") from error
    try:
        after = os.fstat(descriptor)
        require((after.st_dev, after.st_ino) == (before.st_dev, before.st_ino)
                and os.read(descriptor, len(SELECTION_BYTES) + 1) == SELECTION_BYTES,
                "installed_core_not_selected")
    finally:
        os.close(descriptor)


def installed_identity(expected_sha, pair="experimental"):
    require(re.fullmatch(r"[0-9a-f]{64}", expected_sha or "") is not None,
            "core_pin_required")
    require(installed.valid_environment(os.environ, Path.home(),
            Path("/run/user") / str(os.getuid())), "environment_mismatch")
    binary = Path(installed.BINARY).lstat()
    require(stat.S_ISREG(binary.st_mode) and binary.st_uid == 0
            and not binary.st_mode & 0o022, "installed_binary_unsafe")
    require(installed.command([installed.BINARY, "plugin", "target"]) == b"rust\n",
            "not_native_owner")
    core_path, _, _ = pair_paths(pair)
    pinned = gate.experimental_core if pair == "experimental" else release_core
    require(pinned(expected_sha) == str(core_path), "core_pin_mismatch")
    pid = int(installed.unit(RUNTIME, "MainPID"))
    require(pid > 1 and installed.unit(RUNTIME, "ActiveState") == "active",
            "runtime_unavailable")
    environment = gate.bounded(Path("/proc") / str(pid) / "environ", 65536).split(b"\0")
    overrides = [entry for entry in environment if entry.startswith(b"OMAVLESS_MIHOMO=")]
    require(not overrides or overrides == [b"OMAVLESS_MIHOMO=" + os.fsencode(core_path)],
            "installed_core_not_selected")
    durable_core_selection(Path.home() / ".config/omavless", os.getuid())
    return pid


def running_broker_projection(command, pid, status, cgroup, broker_path=BROKER_PATH):
    """Check fixed systemd/proc facts without privileged procfs inode access."""
    require(command.startswith("{ path=" + str(broker_path) + " ; argv[]="
                               + str(broker_path) + " --serve ; ignore_errors=no ; ")
            and command.endswith(" ; pid=" + str(pid) + " ; code=(null) ; status=0/0 }")
            and b"Name:\tomavless-dns-br" in status
            and b"Uid:\t0\t0\t0\t0" in status
            and cgroup == b"0::/system.slice/omavless-dns-broker.service\n",
            "running_broker_unverified")


def broker_identity(expected_sha, pair="experimental"):
    require(re.fullmatch(r"[0-9a-f]{64}", expected_sha or "") is not None,
            "broker_pin_required")
    _, broker_path, unit_sha = pair_paths(pair)
    require(broker_path.resolve(strict=True) == broker_path, "broker_path_unsafe")
    for parent in broker_path.parents:
        metadata = parent.lstat()
        require(stat.S_ISDIR(metadata.st_mode) and metadata.st_uid == 0
                and not metadata.st_mode & 0o022, "broker_path_unsafe")
    metadata = broker_path.lstat()
    require(stat.S_ISREG(metadata.st_mode) and metadata.st_uid == 0
            and metadata.st_nlink == 1 and not metadata.st_mode & 0o022
            and 0 < metadata.st_size <= 32 * 1024 * 1024, "broker_path_unsafe")
    require(hashlib.sha256(broker_path.read_bytes()).hexdigest() == expected_sha,
            "broker_pin_mismatch")
    service = "omavless-dns-broker.service"
    unit = BROKER_UNIT.lstat()
    require(stat.S_ISREG(unit.st_mode) and unit.st_uid == 0
            and unit.st_nlink == 1 and not unit.st_mode & 0o022
            and hashlib.sha256(BROKER_UNIT.read_bytes()).hexdigest() == unit_sha,
            "broker_unit_unsafe")
    def property_value(name):
        return fixed_command(["/usr/bin/systemctl", "--system", "show", service,
                              "--property=" + name, "--value"]).stdout.decode("utf-8").strip()
    require(property_value("FragmentPath") == str(BROKER_UNIT)
            and property_value("DropInPaths") == ""
            and property_value("ActiveState") == "active", "broker_unit_unverified")
    pid = int(property_value("MainPID"))
    require(pid > 1, "broker_service_unavailable")
    # An ordinary desktop user cannot stat /proc/<root PID>/exe on a hardened
    # host. Pin the package-owned unit and systemd's running ExecStart instead;
    # never weaken procfs permissions or request root merely for this read.
    command = property_value("ExecStart")
    require(property_value("ControlGroup") == "/system.slice/" + service,
            "running_broker_unverified")
    status = gate.bounded(Path("/proc") / str(pid) / "status", 16384).splitlines()
    cgroup = gate.bounded(Path("/proc") / str(pid) / "cgroup", 4096)
    running_broker_projection(command, pid, status, cgroup, broker_path)


def broker_state(expected):
    gate.broker_evidence(expected, fixed_command)


def owned_action(runtime_pid, name, *args):
    """Use the private control socket; never put a profile ID in process argv."""
    require(name in ("connect", "disconnect", "mode"), "native_action_rejected")
    require((name == "connect" and len(args) == 2)
            or (name == "mode" and len(args) == 1)
            or (name == "disconnect" and not args), "native_action_rejected")
    state = installed.cli("plugin", "snapshot")
    snapshot = state["result"]
    params = {"instanceId": snapshot["instanceId"],
              "expectedRevision": state["revision"],
              "operationId": "installed-gate-" + uuid.uuid4().hex,
              "action": name}
    if name == "connect":
        params.update(profileId=args[0], mode=args[1])
    elif name == "mode":
        params["mode"] = args[0]
    path = Path("/run/user") / str(os.getuid()) / "omavless/control.sock"
    directory = path.parent.lstat()
    metadata = path.lstat()
    require(stat.S_ISDIR(directory.st_mode) and directory.st_uid == os.getuid()
            and stat.S_IMODE(directory.st_mode) == 0o700
            and stat.S_ISSOCK(metadata.st_mode) and metadata.st_uid == os.getuid()
            and stat.S_IMODE(metadata.st_mode) == 0o600,
            "native_action_rejected")
    request_id = "installed-gate-" + uuid.uuid4().hex
    request = {"api": "omavless.control", "version": 1, "id": request_id,
               "method": "plugin.action", "params": params}
    frame = json.dumps(request, separators=(",", ":")).encode() + b"\n"
    require(len(frame) <= 65536, "native_action_rejected")
    with socket.socket(socket.AF_UNIX) as connection:
        connection.settimeout(120)
        connection.connect(str(path))
        peer_pid, peer_uid, _ = struct.unpack(
            "3i", connection.getsockopt(socket.SOL_SOCKET, socket.SO_PEERCRED, 12))
        require(peer_pid == runtime_pid and peer_uid == os.getuid(),
                "native_action_rejected")
        connection.sendall(frame)
        connection.shutdown(socket.SHUT_WR)
        response = bytearray()
        deadline = time.monotonic() + 120
        while True:
            require(time.monotonic() < deadline, "native_action_rejected")
            chunk = connection.recv(8192)
            if not chunk:
                break
            response.extend(chunk)
            require(len(response) <= 262144, "native_action_rejected")
    require(response.endswith(b"\n") and response.count(b"\n") == 1,
            "native_action_rejected")
    value = json.loads(response)
    require(isinstance(value, dict) and value.get("api") == "omavless.control"
            and type(value.get("version")) is int and value["version"] == 1
            and value.get("id") == request_id
            and value.get("ok") is True, "native_action_rejected")
    return value


def core_process_projection(status, runtime_pid, uid):
    """Check public proc status when file capabilities hide proc/PID/exe."""
    fields = {}
    for line in status.splitlines():
        if b":" not in line:
            continue
        key, value = line.split(b":", 1)
        if key in (b"Name", b"State", b"PPid", b"Uid", b"CapEff"):
            require(key not in fields, "owned_core_unverified")
            fields[key] = value.strip().split()
    require(fields.get(b"Name") == [b"mihomo"]
            and fields.get(b"State", [b"?"])[0] not in (b"Z", b"X")
            and fields.get(b"PPid") == [str(runtime_pid).encode()]
            and fields.get(b"Uid") == [str(uid).encode()] * 4
            and fields.get(b"CapEff") == [b"0000000000003400"],
            "owned_core_unverified")


def connected_state(runtime_pid, mode, pair="experimental"):
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
    members = set(map(int, gate.bounded(group / "cgroup.procs").split()))
    cores = {member for member in members
             if gate.bounded(Path("/proc") / str(member) / "comm", 256).strip() == b"mihomo"}
    require(len(cores) == 1, "owned_core_unverified")
    core = next(iter(cores))
    core_process_projection(gate.bounded(Path("/proc") / str(core) / "status", 16384),
                            runtime_pid, os.getuid())
    actual, pinned = Path("/proc") / str(core) / "exe", pair_paths(pair)[0]
    try:
        inode = actual.stat()
    except PermissionError:
        # The exact reviewed core has file capabilities. Linux can mark such
        # a process nondumpable and reject this ordinary user's procfs read.
        # The package pin, owner-selected path, status/cgroup and controller
        # peer PID below remain evidence, but not running-inode proof.
        pass
    else:
        require((inode.st_dev, inode.st_ino) ==
                (pinned.stat().st_dev, pinned.stat().st_ino), "running_core_not_pinned")
    gate.core_controller(Path("/run/user") / str(os.getuid()) / "omavless/mihomo.sock",
                         core, mode, managed=True)
    broker_state(1)
    return core


def process_stat(raw):
    """Read a bounded Linux stat record; the last ')' ends arbitrary comm bytes."""
    require(0 < len(raw) <= 4096, "core_crash_unverified")
    open_at, close_at = raw.find(b"("), raw.rfind(b")")
    require(open_at > 1 and close_at > open_at and raw[open_at - 1:open_at] == b" "
            and raw[close_at + 1:close_at + 2] == b" ", "core_crash_unverified")
    fields = raw[close_at + 2:].split()
    require(len(fields) >= 20 and raw[:open_at - 1].isdigit()
            and all(fields[index].isdigit() for index in (1, 2, 19))
            and len(fields[0]) == 1 and fields[0] in (b"R", b"S", b"D", b"T", b"t",
                                                       b"Z", b"X", b"x", b"K", b"W", b"P", b"I"),
            "core_crash_unverified")
    return (int(raw[:open_at - 1]), fields[0], int(fields[1]), int(fields[2]),
            int(fields[19]))


def core_stat(core_pid, runtime_pid, started_at, expected_state):
    pid, state, parent, group, start = process_stat(
        gate.bounded(Path("/proc") / str(core_pid) / "stat", 4096))
    require(pid == core_pid and parent == runtime_pid and group == core_pid
            and start == started_at and state in expected_state, "core_crash_unverified")


def live_core_group_members(core_pid, proc_root=Path("/proc")):
    """Count live members, without mistaking the deliberately pinned Z leader for one."""
    members = 0
    for index, entry in enumerate(proc_root.iterdir()):
        require(index < 32768, "live_core_group_remains")
        if not entry.name.isascii() or not entry.name.isdigit():
            continue
        try:
            raw = gate.bounded(entry / "stat", 4096)
        except (FileNotFoundError, ProcessLookupError):
            continue  # Process disappeared during enumeration.
        pid, state, _, group, _ = process_stat(raw)
        require(pid == int(entry.name), "live_core_group_remains")
        if group == core_pid and state not in (b"Z", b"X", b"x"):
            members += 1
            require(members <= 64, "live_core_group_remains")
    return members


def crash_verified_core(core_pid, runtime_pid):
    """Signal only the exact already-verified child through a pinned pidfd."""
    require(hasattr(os, "pidfd_open") and hasattr(signal, "pidfd_send_signal"),
            "core_crash_unverified")
    before = process_stat(gate.bounded(Path("/proc") / str(core_pid) / "stat", 4096))
    require(before[0] == core_pid and before[1] not in (b"Z", b"X", b"x")
            and before[2] == runtime_pid and before[3] == core_pid and before[4] > 0,
            "core_crash_unverified")
    descriptor = os.pidfd_open(core_pid, 0)
    try:
        core_stat(core_pid, runtime_pid, before[4], (before[1],))
        signal.pidfd_send_signal(descriptor, signal.SIGKILL, None, 0)
    finally:
        os.close(descriptor)
    return before[4]


def crash_release_evidence(core_pid, runtime_pid, started_at):
    deadline = time.monotonic() + 15
    while True:
        current = process_stat(gate.bounded(Path("/proc") / str(core_pid) / "stat", 4096))
        require(current[0] == core_pid and current[2] == runtime_pid
                and current[3] == core_pid and current[4] == started_at,
                "core_crash_unverified")
        dead = current[1] in (b"Z", b"X", b"x")
        live_members = live_core_group_members(core_pid)
        observation = installed.cli("runtime", "observation")["result"]
        facts = observation.get("facts")
        cleared = (observation.get("availability") == "observed"
                   and observation.get("manualRecoveryRequired") is False
                   and isinstance(facts, dict)
                   and facts.get("ownedCoreRunning") is False
                   and type(facts.get("visibleTunCount")) is int
                   and facts["visibleTunCount"] == 0 and not installed.tuns())
        if dead and live_members == 0 and cleared:
            try:
                broker_state(0)
            except gate.Failure as error:
                if str(error) != "broker_retention_mismatch":
                    raise
            else:
                gate.emit(core_dead_pinned=True, live_core_group_members=0,
                          tun_released=True, dns_released=True,
                          fresh_owned_core_running=False,
                          last_known_connected=observation.get("lastKnownActual") == "connected")
                return
        if time.monotonic() >= deadline:
            raise gate.Failure("live_core_group_remains" if live_members else
                               "core_crash_unverified")
        time.sleep(0.25)


def mode_sequence(authorization, change, verify):
    for mode in MODES[1:]:
        authorization.step("mode_change", lambda mode=mode: change(mode))
        verify(mode)


def route_projection():
    def reverse_filter(interface):
        value = gate.bounded(
            Path("/proc/sys/net/ipv4/conf") / interface / "rp_filter", 8).strip()
        require(value in (b"0", b"1", b"2"), "host_observation_unavailable")
        return int(value)

    def destination(address):
        raw = fixed_command(["/usr/bin/ip", "-4", "-j", "route", "get", address]).stdout
        rows = json.loads(raw)
        require(isinstance(rows, list) and len(rows) == 1
                and isinstance(rows[0], dict), "host_observation_unavailable")
        return "Meta" if rows[0].get("dev") == "Meta" else "other"

    raw_rules = fixed_command(["/usr/bin/ip", "-4", "-j", "rule", "show"]).stdout
    rules = json.loads(raw_rules)
    require(isinstance(rules, list) and len(rules) <= 64
            and all(isinstance(row, dict) for row in rules), "host_observation_unavailable")
    gate.emit(public_route=destination("1.1.1.1"),
              fakeip_route=destination("198.18.0.78"),
              ipv4_rule_count=len(rules),
              all_rp_filter=reverse_filter("all"),
              tun_rp_filter=reverse_filter("Meta"))


def ufw_projection():
    """Read-only hint, never proof that a TUN packet was accepted or blocked.

    UFW requires root for `ufw status` on Omarchy. Query only the fixed systemd
    unit and keep its output private; other firewall managers or retained kernel
    rules can still affect TUN ingress even when this unit is inactive.
    """
    try:
        result = subprocess.run(
            ["/usr/bin/systemctl", "--system", "is-active", "--quiet", "ufw.service"],
            stdin=subprocess.DEVNULL, capture_output=True, timeout=5,
        )
    except (OSError, subprocess.TimeoutExpired):
        state = "unknown"
    else:
        state = "active" if result.returncode == 0 else "unknown"
        if len(result.stdout) + len(result.stderr) > 4096:
            state = "unknown"
    gate.emit(ufw_service=state,
              tun_ingress_policy="review_required" if state == "active" else "unverified")
    return state


def probe_tun_https():
    for index, target in enumerate(HTTPS_TARGETS, start=1):
        before = gate.tun_counters("Meta")
        arguments = gate.https_probe_args("Meta")
        arguments[-1] = target
        result = subprocess.run(arguments, stdin=subprocess.DEVNULL,
                                capture_output=True, timeout=25)
        after = gate.tun_counters("Meta")
        https, tun_used, classification = gate.https_probe_evidence(result, before, after)
        # No URL, response body, address or private error reaches the output.
        gate.emit(https_target=index, full_vpn_https=https,
                  tun_used=tun_used,
                  tun_rx_moved=after[0] > before[0],
                  tun_tx_moved=after[1] > before[1],
                  classification=classification)
        if https and tun_used:
            return True
    return False


def tun_tracker_count(payload):
    snapshot = json.loads(payload)
    require(isinstance(snapshot, dict) and "connections" in snapshot,
            "core_controller_response")
    connections = snapshot["connections"]
    # Mihomo serializes an empty, nil tracker slice as JSON null.
    if connections is None:
        connections = []
    require(isinstance(connections, list) and len(connections) <= 4096,
            "core_controller_response")
    require(all(isinstance(row, dict) and isinstance(row.get("metadata"), dict)
                for row in connections), "core_controller_response")
    return sum(row["metadata"].get("type") == "Tun" for row in connections)


def core_tun_tracker_count(core_pid):
    path = Path("/run/user") / str(os.getuid()) / "omavless/mihomo.sock"
    metadata = path.lstat()
    require(stat.S_ISSOCK(metadata.st_mode) and metadata.st_uid == os.getuid()
            and stat.S_IMODE(path.parent.stat().st_mode) == 0o700,
            "core_socket_identity")
    with socket.socket(socket.AF_UNIX) as connection:
        connection.settimeout(2)
        connection.connect(str(path))
        peer_pid, peer_uid, _ = struct.unpack(
            "3i", connection.getsockopt(socket.SOL_SOCKET, socket.SO_PEERCRED, 12))
        require(peer_pid == core_pid and peer_uid == os.getuid(), "core_socket_peer")
        connection.sendall(b"GET /connections/ HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
        response = http.client.HTTPResponse(connection)
        response.begin()
        payload = response.read(1048577)
        require(response.status == 200 and len(payload) <= 1048576,
                "core_controller_response")
    return tun_tracker_count(payload)


def probe_direct_ip_tun(core_pid):
    """Diagnostic only: bypass DNS; do not count it as Full VPN acceptance."""
    before = gate.tun_counters("Meta")
    arguments = gate.https_probe_args("Meta")
    arguments[-1] = DIRECT_IP_HTTPS
    arguments[-1:-1] = ["--resolve", DIRECT_IP_RESOLVE]
    max_tun_trackers = 0
    tracker_observed = True
    with subprocess.Popen(arguments, stdin=subprocess.DEVNULL,
                          stdout=subprocess.PIPE, stderr=subprocess.PIPE) as process:
        deadline = time.monotonic() + 20
        while process.poll() is None and time.monotonic() < deadline:
            if tracker_observed:
                try:
                    max_tun_trackers = max(max_tun_trackers,
                                           core_tun_tracker_count(core_pid))
                except Exception:
                    # Supplemental telemetry must not obscure the HTTPS gate.
                    tracker_observed = False
            time.sleep(0.25)
        try:
            stdout, stderr = process.communicate(timeout=max(1, deadline - time.monotonic()))
        except subprocess.TimeoutExpired:
            process.kill()
            stdout, stderr = process.communicate()
        require(len(stdout) + len(stderr) <= 16384, "host_observation_unavailable")
        result = subprocess.CompletedProcess(arguments, process.returncode, stdout, stderr)
    after = gate.tun_counters("Meta")
    https, tun_used, classification = gate.https_probe_evidence(result, before, after)
    gate.emit(direct_ip_https=https, direct_ip_tun_used=tun_used,
              direct_ip_classification=classification,
              direct_ip_tracker_observed=tracker_observed,
              direct_ip_tun_trackers_seen=max_tun_trackers > 0,
              direct_ip_tun_rx_moved=after[0] > before[0],
              direct_ip_tun_tx_moved=after[1] > before[1])


def probe_core_proxy_https():
    arguments = [
        "/usr/bin/curl", "--silent", "--show-error", "--noproxy", "",
        "--proxy", "http://127.0.0.1:7890", "--proto", "=https",
        "--connect-timeout", "5", "--max-time", "15", "--max-redirs", "0",
        "--output", "/dev/null", "--write-out", "%{http_code}", HTTPS_TARGETS[0],
    ]
    result = subprocess.run(arguments, stdin=subprocess.DEVNULL,
                            capture_output=True, timeout=25)
    passed = result.returncode == 0 and re.fullmatch(rb"2[0-9]{2}", result.stdout) is not None
    gate.emit(core_proxy_https=passed)
    return passed


def failure_classification(error):
    if isinstance(error, auth.AuthorizationUnsettled):
        return "human_authorization_unsettled"
    if isinstance(error, gate.Failure):
        code = str(error)
        # An allowlist, not just a syntax check: opaque private ASCII tokens
        # can also satisfy a simple identifier pattern.
        if code in SAFE_FAILURES:
            return code
    return "installed_dns_gate_failed"


def failure_type(error):
    # Type-only diagnostics cannot contain a provider error or profile field.
    if isinstance(error, subprocess.TimeoutExpired):
        return "subprocess_timeout"
    for kind in (FileNotFoundError, PermissionError, KeyError, IndexError,
                 TypeError, ValueError, AttributeError, OSError):
        if isinstance(error, kind):
            return kind.__name__
    return "other"


def run_gate(authorization, expected_sha, broker_sha, profile_index=None,
             pair="experimental"):
    authorization.require_terminal()  # before private state or host observation
    pair_paths(pair)
    runtime_pid = installed_identity(expected_sha, pair)
    broker_identity(broker_sha, pair)
    initial = installed.cli("runtime", "observation")["result"]
    require(installed.clean_disconnected_observation(initial) and not installed.tuns(),
            "baseline_not_disconnected")
    broker_state(0)
    snapshot = installed.cli("plugin", "snapshot")["result"]
    require(snapshot["startup"]["enabled"] is False, "startup_not_disabled")
    profile = selected_profile(snapshot, profile_index)
    original_mode = snapshot["desired"]["mode"]
    require(original_mode in ("global", "rule", "direct"), "original_mode_unavailable")
    gate.emit(case="installed-managed-dns", pair=pair, preflight=True,
              mode_sequence=list(MODES), authorization="attended")
    # This never installs a firewall exception. Connected DNS readback is not
    # evidence that UFW (or another firewall) passes actual TUN ingress.
    ufw_state = ufw_projection()
    connected = False
    completed = False
    checkpoint = "connect"
    try:
        authorization.step("connect", lambda: owned_action(runtime_pid, "connect", profile, "global"))
        connected = True
        checkpoint = "connected_state"
        core_pid = connected_state(runtime_pid, "global", pair)
        route_projection()
        checkpoint = "https_probe"
        probe_direct_ip_tun(core_pid)
        if not probe_tun_https():
            proxy_ok = probe_core_proxy_https()  # cannot replace TUN evidence
            gate.emit(tun_failure_hint=(
                "ufw_ingress_possible_not_proven"
                if ufw_state == "active" and proxy_ok else "undetermined"))
            raise gate.Failure("full_vpn_https_failed")
        checkpoint = "mode_sequence"
        mode_sequence(authorization, lambda mode: owned_action(runtime_pid, "mode", mode),
                      lambda mode: connected_state(runtime_pid, mode, pair))
        completed = True
    except Exception as error:
        gate.emit(checkpoint=checkpoint, failure_type=failure_type(error))
        raise
    finally:
        if authorization.blocked:
            gate.emit(passed=False, cleanup=False,
                      classification="human_authorization_unsettled")
            raise auth.AuthorizationUnsettled()
        # A rejected/unknown Connect can still have host effects. Its separately
        # attended Disconnect must not be skipped just because CLI rejected it.
        authorization.step("disconnect", lambda: owned_action(runtime_pid, "disconnect"))
        clean = installed.clean_disconnected_observation(
            installed.cli("runtime", "observation")["result"])
        require(clean and not installed.tuns(), "cleanup_unverified")
        broker_state(0)
        if installed.cli("plugin", "snapshot")["result"]["desired"]["mode"] != original_mode:
            authorization.step("restore_mode", lambda: owned_action(runtime_pid, "mode", original_mode))
        restored = installed.cli("plugin", "snapshot")["result"]["desired"]
        require(restored["connected"] is False and restored["mode"] == original_mode,
                "restoration_unverified")
        gate.emit(disconnected=True, dns_released=True, mode_restored=True,
                  passed=completed and connected)


def run_core_crash_gate(authorization, expected_sha, broker_sha, profile_index=None,
                        pair="experimental"):
    """Opt-in installed crash diagnostic; never signals an unverified PID/name."""
    authorization.require_terminal()
    pair_paths(pair)
    runtime_pid = installed_identity(expected_sha, pair)
    broker_identity(broker_sha, pair)
    initial = installed.cli("runtime", "observation")["result"]
    require(installed.clean_disconnected_observation(initial) and not installed.tuns(),
            "baseline_not_disconnected")
    broker_state(0)
    snapshot = installed.cli("plugin", "snapshot")["result"]
    require(snapshot["startup"]["enabled"] is False, "startup_not_disabled")
    profile = selected_profile(snapshot, profile_index)
    original_mode = snapshot["desired"]["mode"]
    require(original_mode in ("global", "rule", "direct"), "original_mode_unavailable")
    gate.emit(case="installed-managed-dns-core-crash", pair=pair, preflight=True,
              authorization="attended")
    completed = False
    core_pid = None
    checkpoint = "connect"
    try:
        authorization.step("connect", lambda: owned_action(runtime_pid, "connect", profile, "global"))
        checkpoint = "connected_state"
        core_pid = connected_state(runtime_pid, "global", pair)
        checkpoint = "core_crash"
        started_at = authorization.step(
            "core_crash", lambda: crash_verified_core(core_pid, runtime_pid))
        checkpoint = "crash_release"
        crash_release_evidence(core_pid, runtime_pid, started_at)
        completed = True
    except Exception as error:
        gate.emit(checkpoint=checkpoint, failure_type=failure_type(error))
        raise
    finally:
        if authorization.blocked:
            gate.emit(passed=False, cleanup=False,
                      classification="human_authorization_unsettled")
            raise auth.AuthorizationUnsettled()
        authorization.step("disconnect", lambda: owned_action(runtime_pid, "disconnect"))
        clean = installed.clean_disconnected_observation(
            installed.cli("runtime", "observation")["result"])
        require(clean and not installed.tuns()
                and (core_pid is None or not (Path("/proc") / str(core_pid)).exists()),
                "cleanup_unverified")
        broker_state(0)
        if installed.cli("plugin", "snapshot")["result"]["desired"]["mode"] != original_mode:
            authorization.step("restore_mode", lambda: owned_action(runtime_pid, "mode", original_mode))
        restored = installed.cli("plugin", "snapshot")["result"]["desired"]
        require(restored["connected"] is False and restored["mode"] == original_mode,
                "restoration_unverified")
        gate.emit(disconnected=True, dns_released=True, mode_restored=True,
                  passed=completed)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--run", action="store_true")
    parser.add_argument("--pair", choices=("experimental", "release"),
                        default="experimental")
    parser.add_argument("--experimental-core-sha", metavar="SHA256")
    parser.add_argument("--experimental-broker-sha", metavar="SHA256")
    parser.add_argument("--release-core-sha", metavar="SHA256")
    parser.add_argument("--release-broker-sha", metavar="SHA256")
    parser.add_argument("--profile-index", type=int, metavar="N",
                        help="0-based index among available VLESS profiles; default: last selected")
    parser.add_argument("--core-crash", action="store_true",
                        help="opt-in exact-pidfd core death and cleanup diagnostic")
    args = parser.parse_args(argv)
    if not args.run:
        gate.emit(status="NOT RUN", reason="explicit_run_required")
        return 0
    try:
        if args.pair == "release":
            require(args.experimental_core_sha is None and args.experimental_broker_sha is None,
                    "pair_pin_mismatch")
            core_sha, broker_sha = args.release_core_sha, args.release_broker_sha
        else:
            require(args.release_core_sha is None and args.release_broker_sha is None,
                    "pair_pin_mismatch")
            core_sha, broker_sha = args.experimental_core_sha, args.experimental_broker_sha
        require(re.fullmatch(r"[0-9a-f]{64}", core_sha or "") is not None
                and re.fullmatch(r"[0-9a-f]{64}", broker_sha or "") is not None,
                "pair_pin_mismatch")
        runner = run_core_crash_gate if args.core_crash else run_gate
        runner(auth.HumanAuthorization(), core_sha, broker_sha, args.profile_index,
               args.pair)
        return 0
    except Exception as error:
        gate.emit(passed=False, classification=failure_classification(error))
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
