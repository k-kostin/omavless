#!/usr/bin/python3
# SPDX-License-Identifier: MIT
"""Opt-in TEST TOOL, not a product Python dependency. Never activates ownership.

Exercises the already installed, committed Rust owner using the user's last
usable VLESS profile and the accepted loopback-proxy/system-TUN policy class.
Private responses stay in memory. No profile IDs, endpoints or raw errors print.
"""
import argparse
import importlib.util
import ipaddress
import json
import os
from pathlib import Path
import re
import stat
import subprocess
import time
import uuid

_spec = importlib.util.spec_from_file_location("native_gate", Path(__file__).with_name("native_service_acceptance.py"))
gate = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(gate)
BINARY = "/usr/bin/omavless"
UNIT = "omavless-runtime.service"
LEGACY = "omavless.service"
LIMIT = 1048576


def template_policy(config):
    """Accepted fixture POLICY only; never parse credentials or rewrite YAML."""
    gate.require(isinstance(config, bytes) and len(config) <= 5242880, "fixture_unavailable")
    def scalar(key):
        values = re.findall(rb"(?m)^" + key + rb": *([^\r\n]+) *$", config)
        gate.require(len(values) == 1, "fixture_unavailable")
        return values[0].strip()
    port = scalar(b"mixed-port")
    gate.require(port.isdigit() and 0 < int(port) <= 65535, "fixture_unavailable")
    gate.require(scalar(b"allow-lan") == b"false", "fixture_unavailable")
    gate.require(scalar(b"bind-address") in (b"127.0.0.1", b"'127.0.0.1'", b'"127.0.0.1"'), "fixture_unavailable")
    gate.require(not re.search(rb"(?m)^(port|socks-port|redir-port|tproxy-port):", config), "fixture_unavailable")
    gate.require(re.findall(rb"(?m)^ +stack: *([^\r\n]+)$", config) == [b"system"], "fixture_unavailable")
    keys = re.findall(rb'''(?m)^[ \t]*["']?(external-controller(?:-[A-Za-z0-9_-]+)?)["']?[ \t]*:''', config)
    gate.require(all(key == b"external-controller-unix" for key in keys) and len(keys) <= 1, "fixture_unavailable")
    return int(port)


def proc_address(address):
    packed = ipaddress.ip_address(address).packed
    return b"".join(packed[n:n + 4][::-1].hex().upper().encode() for n in range(0, len(packed), 4))


def listener_rows(tcp, tcp6):
    """Bounded strict LISTEN inventory: (protocol, encoded address, port, inode)."""
    rows = []
    for protocol, raw in (("tcp", tcp), ("tcp6", tcp6)):
        gate.require(len(raw) <= LIMIT, "listener_invalid")
        lines = raw.splitlines()
        gate.require(lines and b"local_address" in lines[0] and b"inode" in lines[0], "listener_invalid")
        gate.require(len(lines) <= 8193, "listener_invalid")
        for line in lines[1:]:
            fields = line.split()
            gate.require(len(fields) >= 10 and fields[9].isdigit(), "listener_invalid")
            gate.require(re.fullmatch(rb"[0-9A-Fa-f]{2}", fields[3]) is not None, "listener_invalid")
            if fields[3].upper() != b"0A":
                continue
            endpoint = fields[1].split(b":")
            gate.require(len(endpoint) == 2 and re.fullmatch(rb"[0-9A-Fa-f]{4}", endpoint[1]) is not None, "listener_invalid")
            gate.require(re.fullmatch(rb"[0-9A-Fa-f]{" + (b"8" if protocol == "tcp" else b"32") + rb"}", endpoint[0]) is not None, "listener_invalid")
            rows.append((protocol, endpoint[0].upper(), int(endpoint[1], 16), fields[9].decode("ascii")))
    return rows


def classify_listeners(baseline, rows, mixed_port, tun_addresses, ss_output, core_pid):
    """Pure classifier. Baseline unrelated sockets excluded; new ones need PID proof."""
    gate.require(type(core_pid) is int and core_pid > 1 and len(ss_output) <= LIMIT, "tcp_attribution_unavailable")
    wanted = {row[3] for row in rows} - baseline
    checked, proxies, forwarders = set(), 0, 0
    for protocol, address, port, inode in rows:
        if inode not in wanted:
            continue
        loopback = address == (b"0100007F" if protocol == "tcp" else b"00000000000000000000000001000000")
        proxy = loopback and port == mixed_port
        forwarder = address in tun_addresses and port > 0
        gate.require(proxy or forwarder, "unexpected_tcp_listener")
        proxies += int(proxy)
        forwarders += int(forwarder)
        gate.require(inode not in checked, "listener_invalid")
        checked.add(inode)
    gate.require(checked == wanted and 1 <= proxies <= 2 and 1 <= forwarders <= len(tun_addresses), "proxy_listener_count")
    owned = set()
    for line in ss_output.splitlines():
        inode = re.search(rb"\bino:([0-9]+)\b", line)
        if inode and re.search(rb"\bpid=" + str(core_pid).encode() + rb",", line):
            owned.add(inode.group(1).decode("ascii"))
    gate.require(wanted <= owned, "tcp_attribution_failed")
    return True


def probe_args(tun):
    gate.require(isinstance(tun, str) and re.fullmatch(r"[A-Za-z0-9_][A-Za-z0-9_-]{0,14}", tun) is not None, "observed_tun_invalid")
    return ["/usr/bin/curl", "--silent", "--show-error", "--noproxy", "*", "--proxy", "",
            "--interface", "if!" + tun, "--proto", "=https", "--proto-redir", "=https",
            "--connect-timeout", "5", "--max-time", "15", "--max-redirs", "0",
            "--output", "/dev/null", "--write-out", "%{http_code}", "https://example.com/"]


def command(args, timeout=125):
    result = subprocess.run(args, stdin=subprocess.DEVNULL, capture_output=True, timeout=timeout)
    gate.require(len(result.stdout) + len(result.stderr) <= LIMIT and result.returncode == 0, "command_failed")
    return result.stdout


def cli(*args):
    data = json.loads(command([BINARY, *args]))
    gate.require(data.get("ok") is True, "native_action_rejected")
    return data


def action(name, *args):
    state = cli("plugin", "snapshot")
    return cli("plugin", name, state["result"]["instanceId"], str(state["revision"]),
               "installed-gate-" + uuid.uuid4().hex, *args)


def unit(name, field):
    gate.require(name in (UNIT, LEGACY) and field in ("MainPID", "ControlGroup", "ActiveState"), "command_failed")
    return command(["/usr/bin/systemctl", "--user", "show", name, "--property=" + field, "--value"]).decode().strip()


def tuns():
    return {row["ifname"] for row in json.loads(command(["/usr/bin/ip", "-d", "-j", "link", "show"]))
            if row.get("linkinfo", {}).get("info_kind") == "tun"}


def counters(tun):
    probe_args(tun)
    gate.require(tun in tuns(), "observed_tun_invalid")
    device = (Path("/sys/class/net") / tun).resolve(strict=True)
    gate.require(device.is_relative_to(Path("/sys/devices")), "tun_counter_invalid")
    values = []
    for field in ("rx_bytes", "tx_bytes"):
        descriptor = os.open(device / "statistics" / field, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
        try:
            gate.require(stat.S_ISREG(os.fstat(descriptor).st_mode), "tun_counter_invalid")
            value = os.read(descriptor, 33).strip()
            gate.require(len(value) <= 32 and value.isdigit(), "tun_counter_invalid")
            values.append(int(value))
        finally:
            os.close(descriptor)
    gate.require(tun in tuns() and (Path("/sys/class/net") / tun).resolve(strict=True) == device, "observed_tun_invalid")
    return tuple(values)


def run_gate():
    binary = Path(BINARY).lstat()
    gate.require(stat.S_ISREG(binary.st_mode) and binary.st_uid == 0 and not binary.st_mode & 0o022, "installed_binary_unsafe")
    home = Path.home()
    runtime = Path("/run/user") / str(os.getuid())
    gate.require(not os.environ.get("OMAVLESS_HOME") and os.environ.get("XDG_RUNTIME_DIR", str(runtime)) == str(runtime)
                 and os.environ.get("XDG_STATE_HOME", str(home / ".local/state")) == str(home / ".local/state"), "environment_mismatch")
    gate.require(command([BINARY, "plugin", "target"]) == b"rust\n", "not_native_owner")
    state = cli("plugin", "snapshot")["result"]
    gate.require(state["startup"]["configured"] and not state["startup"]["enabled"], "startup_not_disabled")
    gate.require(not state["desired"]["connected"] and state["lastKnownActual"] == "disconnected" and not tuns()
                 and not any(name == b"mihomo" for name in gate.processes().values()), "baseline_not_disconnected")
    initial = cli("runtime", "observation")["result"]
    gate.require(initial["availability"] == "observed" and initial["facts"]["visibleMihomoCount"] == 0
                 and initial["facts"]["visibleTunCount"] == 0 and not initial["facts"]["ownedCoreRunning"], "baseline_not_disconnected")
    profile = next((p for p in state["profiles"] if p["id"] == state["lastProfileId"] and p["protocol"] == "vless" and not p["missing"]), None)
    gate.require(profile is not None, "fixture_unavailable")
    mixed_port = template_policy(gate.bounded(home / ".config/omavless/route-template.yaml", 5242880))
    original_mode = state["desired"]["mode"]
    baseline = gate.tcp_listeners()
    pid = int(unit(UNIT, "MainPID"))
    cgroup = (Path("/sys/fs/cgroup") / unit(UNIT, "ControlGroup").lstrip("/")).resolve(strict=True)
    gate.require(cgroup.is_relative_to(Path("/sys/fs/cgroup")) and cgroup != Path("/sys/fs/cgroup"), "service_baseline")
    gate.require(pid > 1 and unit(UNIT, "ActiveState") == "active" and unit(LEGACY, "ActiveState") == "inactive"
                 and unit(LEGACY, "MainPID") == "0", "service_baseline")
    gate.require(gate.security_facts(pid) == {"nnp": 0, "effective_capabilities": 0}, "daemon_privileges")
    gate.require(set(map(int, gate.bounded(cgroup / "cgroup.procs").split())) == {pid}
                 and not os.path.lexists(runtime / "omavless/mihomo.sock"), "service_baseline")
    passed = False
    try:
        start = time.monotonic()
        action("connect", profile["id"], "global")
        elapsed = round((time.monotonic() - start) * 1000)
        observed = cli("runtime", "observation")["result"]
        gate.require(observed["lastKnownActual"] == "connected" and observed["facts"]["ownedControllerConfigVerified"], "controller_config")
        cores = {p for p, name in gate.processes().items() if name == b"mihomo"}
        members = set(map(int, gate.bounded(cgroup / "cgroup.procs").split()))
        gate.require(len(cores) == 1 and cores <= members and cores <= gate.descendants(pid), "owned_core")
        links = tuns()
        gate.require(len(links) == 1, "single_tun")
        tun, core = next(iter(links)), next(iter(cores))
        gate.require(os.getpgid(core) == core, "owned_process_group")
        gate.core_controller(runtime / "omavless/mihomo.sock", core, "global")
        config = gate.bounded(home / ".config/omavless/config.yaml", 5242880)
        gate.require(template_policy(config) == mixed_port, "fixture_unavailable")
        gate.require(re.findall(rb'''(?m)^[ \t]*["']?(external-controller(?:-[A-Za-z0-9_-]+)?)["']?[ \t]*:''', config) == [b"external-controller-unix"], "tcp_controller_config")
        addresses = {proc_address(entry["local"]) for link in json.loads(command(["/usr/bin/ip", "-j", "address", "show", "dev", tun])) for entry in link.get("addr_info", [])}
        rows = listener_rows(gate.bounded(Path("/proc/net/tcp"), LIMIT), gate.bounded(Path("/proc/net/tcp6"), LIMIT))
        gate.emit(authorization_required="read_only_socket_pid_attribution")
        # Explicit opt-in above; no deadline on ordinary human polkit dialogs.
        proof = command(["/usr/bin/pkexec", "/usr/bin/ss", "-H", "-ltnpe"], timeout=None)
        classify_listeners(baseline, rows, mixed_port, addresses, proof, core)
        before = counters(tun)
        result = subprocess.run(probe_args(tun), capture_output=True, timeout=25)
        probe, used, classification = gate.https_probe_evidence(result, before, counters(tun))
        gate.emit(case="installed-vless-full-vpn", connect=True, connect_ms=elapsed, service_owned_core=True,
                  single_tun=True, unix_controller=True, only_expected_proxy_and_tun_forwarder=True,
                  no_tcp_controller_config=True, tcp_pid_attribution=True, https_probe=probe, tun_used=used, classification=classification)
        gate.require(probe and used, "https_probe_failed")
        passed = True
    finally:
        action("disconnect")
        observed = cli("runtime", "observation")["result"]
        gate.require(observed["lastKnownActual"] == "disconnected" and not tuns()
                     and not any(name == b"mihomo" for name in gate.processes().values())
                     and set(map(int, gate.bounded(cgroup / "cgroup.procs").split())) == {pid}
                     and unit(LEGACY, "ActiveState") == "inactive" and unit(LEGACY, "MainPID") == "0"
                     and not os.path.lexists(runtime / "omavless/mihomo.sock"), "manual_recovery_required")
        if cli("plugin", "snapshot")["result"]["desired"]["mode"] != original_mode:
            action("mode", original_mode)
        restored = cli("plugin", "snapshot")["result"]["desired"]
        gate.require(not restored["connected"] and restored["mode"] == original_mode, "manual_recovery_required")
        gate.emit(disconnect=True, cleanup=True, mode_restored=True, passed=passed)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--run", action="store_true")
    parser.add_argument("--authorize-socket-inspection", action="store_true")
    args = parser.parse_args(argv)
    if not args.run or not args.authorize_socket_inspection:
        gate.emit(status="NOT RUN", reason="explicit_run_and_socket_inspection_authorization_required")
        return 0
    try:
        run_gate()
        return 0
    except Exception as error:
        allowed = {"fixture_unavailable", "unexpected_tcp_listener", "proxy_listener_count", "tcp_attribution_failed",
                   "tcp_attribution_unavailable", "https_probe_failed", "manual_recovery_required", "baseline_not_disconnected"}
        code = str(error) if isinstance(error, gate.Failure) and str(error) in allowed else "installed_gate_failed_check_safe_state"
        gate.emit(passed=False, classification="FIXTURE UNAVAILABLE" if code == "fixture_unavailable" else code)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
