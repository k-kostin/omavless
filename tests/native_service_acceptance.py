#!/usr/bin/python3
# SPDX-License-Identifier: MIT
"""Explicit, destructive-to-its-own-fixture-only native user-service smoke.

Opt in with --run --binary /absolute/exact/build/omavless. No installed unit,
private source store or persistent network configuration is changed. The default synthetic no-auto-route TUN
still exercises Mihomo/resolved and may show the NORMAL Omarchy polkit dialogs.
Allow human authorization; command waits are 120 seconds, cleanup 600 seconds.
The runtime's own unary deadline is unchanged. This is not VPN interoperability
or packaged installation evidence. Do not run concurrently with any VPN test.
All captured command output stays in private synthetic fixture files, never
stdout. On failure those files are retained under the private runtime directory.
Optional --private-vless-store reads one existing fixture into the isolated
store; it temporarily enables auto-route in Full VPN and probes generic HTTPS
bound to the generated TUN. Real-fixture command captures remain private.
"""
import argparse
import hashlib
import http.client
import json
import os
from pathlib import Path
import re
import shutil
import socket
import stat
import struct
import subprocess
import tempfile
import time
import uuid


class Failure(Exception):
    pass


PRIVATE_STORE_LIMIT = 5 * 1024 * 1024
PUBLIC_PROFILE_ID = "00000000-0000-4000-8000-000000000001"


def load_private_vless(path):
    """Read one private candidate; Rust still owns protocol/config validation.

    Preserve every protocol field. Detach only test identity/subscription links;
    the source path/record/name/URI is never returned in a public classification.
    """
    try:
        path = Path(path)
        require(path.is_absolute() and path.resolve(strict=True) == path, "private_store_unsafe")
        for parent in (path, *path.parents):
            require(not stat.S_ISLNK(parent.lstat().st_mode), "private_store_unsafe")
        fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
        with os.fdopen(fd, "rb") as stream:
            before = os.fstat(stream.fileno())
            require(stat.S_ISREG(before.st_mode) and before.st_uid == os.getuid()
                    and stat.S_IMODE(before.st_mode) == 0o600, "private_store_unsafe")
            require(before.st_size <= PRIVATE_STORE_LIMIT, "private_store_oversized")
            payload = stream.read(PRIVATE_STORE_LIMIT + 1)
            after = os.fstat(stream.fileno())
            current = path.lstat()
            identity = lambda m: (m.st_dev, m.st_ino, m.st_size, m.st_mtime_ns, m.st_ctime_ns, m.st_uid, m.st_mode)
            require(identity(before) == identity(after) == identity(current), "private_store_changed")
        require(len(payload) <= PRIVATE_STORE_LIMIT, "private_store_oversized")

        def pairs(items):
            result = {}
            for key, value in items:
                require(key not in result, "private_store_invalid")
                result[key] = value
            return result

        document = json.loads(payload.decode("utf-8"), object_pairs_hook=pairs,
                              parse_constant=lambda _: (_ for _ in ()).throw(Failure("private_store_invalid")))
        require(isinstance(document, dict) and document.get("version") == 3
                and isinstance(document.get("profiles"), list)
                and len(document["profiles"]) <= 256, "private_store_invalid")
        profiles = document["profiles"]
        last = document.get("lastId")
        # Prefer the user's existing selection without exposing its identity.
        # An unusable/missing/non-VLESS last selection still falls back safely.
        ordered = sorted(profiles, key=lambda p: not (isinstance(last, str) and last
                         and isinstance(p, dict) and p.get("id") == last))
        for profile in ordered:
            if (isinstance(profile, dict) and profile.get("protocol") == "vless"
                    and profile.get("missing", False) is False
                    and isinstance(profile.get("uri"), str) and profile["uri"].startswith("vless://")):
                # No new protocol inference and no URI parsing/rewriting here.
                detached = dict(profile)
                detached.update(id=PUBLIC_PROFILE_ID, name="Synthetic", subscriptionId="", subscriptionKey="")
                return detached
        raise Failure("vless_fixture_unavailable")
    except Failure:
        raise
    except Exception:
        raise Failure("private_store_unreadable") from None


def route_template(tun, real=False):
    require(re.fullmatch(r"ovna[0-9a-f]{10}", tun) is not None, "fixture_tun_invalid")
    dns = ("dns:\n  enable: true\n  ipv6: false\n  enhanced-mode: fake-ip\n"
           "  fake-ip-range: 198.18.0.1/16\n  default-nameserver: [1.1.1.1, 8.8.8.8]\n"
           "  nameserver: [https://1.1.1.1/dns-query]\n") if real else "dns: {enable: false}\n"
    return f"""mixed-port: 0
allow-lan: false
mode: rule
log-level: silent
{dns}tun:
  enable: true
  device: {tun}
  stack: gvisor
  auto-route: {str(real).lower()}
  auto-redirect: false
  auto-detect-interface: {str(real).lower()}
  dns-hijack: {"[any:53]" if real else "[]"}
proxies:
{{{{OMAVLESS_PROXY}}}}
proxy-groups:
  - {{name: PROXY, type: select, proxies: [Synthetic]}}
  - {{name: GLOBAL, type: select, proxies: [PROXY]}}
rules:
  - MATCH,{"PROXY" if real else "DIRECT"}
"""


def https_probe_args(tun):
    require(re.fullmatch(r"ovna[0-9a-f]{10}", tun) is not None, "fixture_tun_invalid")
    return ["/usr/bin/curl", "--silent", "--show-error", "--noproxy", "*", "--proxy", "",
            "--interface", "if!" + tun, "--proto", "=https", "--proto-redir", "=https",
            "--connect-timeout", "5", "--max-time", "15", "--max-redirs", "0",
            "--output", "/dev/null", "--write-out", "%{http_code}", "https://example.com/"]


def tun_counters(tun):
    require(re.fullmatch(r"ovna[0-9a-f]{10}", tun) is not None, "fixture_tun_invalid")
    values = []
    for field in ("rx_bytes", "tx_bytes"):
        value = bounded(Path("/sys/class/net") / tun / "statistics" / field, 32).strip()
        require(value.isdigit(), "tun_counter_invalid")
        values.append(int(value))
    return tuple(values)


def https_probe_evidence(result, before, after):
    good = result.returncode == 0 and re.fullmatch(rb"2[0-9]{2}", result.stdout) is not None
    used = good and len(before) == len(after) == 2 and all(b > a for a, b in zip(before, after))
    if not good:
        code = {6: "probe_dns_failed", 7: "probe_connect_failed", 28: "probe_timeout",
                45: "probe_interface_unavailable", 60: "probe_tls_failed"}.get(result.returncode, "https_probe_failed")
    else:
        code = "pass" if used else "tun_probe_evidence_unavailable"
    return good, used, code


def require(condition, code):
    if not condition:
        raise Failure(code)


def emit(**values):
    print(json.dumps(values, separators=(",", ":")), flush=True)


def bounded(path, maximum=131072):
    with path.open("rb") as stream:
        value = stream.read(maximum + 1)
    require(len(value) <= maximum, "observation_oversized")
    return value


def private_write(path, value):
    fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
    with os.fdopen(fd, "wb") as stream:
        stream.write(value if isinstance(value, bytes) else value.encode())


def processes():
    found = {}
    for entry in Path("/proc").iterdir():
        if entry.name.isdigit():
            try:
                found[int(entry.name)] = bounded(entry / "comm", 256).strip()
            except FileNotFoundError:
                pass
    return found


def descendants(pid):
    found, pending = set(), [pid]
    while pending:
        current = pending.pop()
        require(len(found) <= 256, "descendant_bound")
        try:
            threads = list((Path("/proc") / str(current) / "task").iterdir())
            require(len(threads) <= 1024, "thread_bound")
            for thread in threads:
                try:
                    children = map(int, bounded(thread / "children").split())
                    for child in children:
                        if child not in found:
                            found.add(child)
                            pending.append(child)
                except FileNotFoundError:
                    pass
        except FileNotFoundError:
            pass
    return found


def core_controller(path, pid, mode):
    metadata = path.lstat()
    require(stat.S_ISSOCK(metadata.st_mode) and metadata.st_uid == os.getuid(), "core_socket_identity")
    require(stat.S_IMODE(path.parent.stat().st_mode) == 0o700, "core_socket_directory")
    with socket.socket(socket.AF_UNIX) as connection:
        connection.settimeout(3)
        connection.connect(str(path))
        peer_pid, peer_uid, _ = struct.unpack("3i", connection.getsockopt(socket.SOL_SOCKET, socket.SO_PEERCRED, 12))
        require(peer_pid == pid and peer_uid == os.getuid(), "core_socket_peer")
        connection.sendall(b"GET /configs HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
        response = http.client.HTTPResponse(connection)
        response.begin()
        payload = response.read(65537)
        require(response.status == 200 and len(payload) <= 65536, "core_controller_response")
        require(json.loads(payload).get("mode", "").lower() == mode, "core_mode")


def tcp_listeners():
    inodes = set()
    for protocol in ("tcp", "tcp6"):
        rows = bounded(Path("/proc/net") / protocol, 1048576).splitlines()
        remote = b"rem_address" if protocol == "tcp" else b"remote_address"
        require(bool(rows) and rows[0].split()[:4] == [b"sl", b"local_address", remote, b"st"]
                and b"inode" in rows[0].split(), "tcp_observation_header")
        for row in rows[1:]:
            fields = row.split()
            require(len(fields) >= 10 and re.fullmatch(rb"[0-9A-F]{2}", fields[3])
                    and fields[9].isdigit(), "tcp_observation_shape")
            if fields[3] == b"0A":
                inodes.add(fields[9].decode("ascii"))
    return inodes


def tcp_absent(pid, listeners):
    try:
        inodes = set()
        for fd in (Path("/proc") / str(pid) / "fd").iterdir():
            try:
                target = os.readlink(fd)
                if target.startswith("socket:["):
                    inodes.add(target[8:-1])
            except FileNotFoundError:
                pass
        return not bool(inodes & listeners)
    except PermissionError:
        return None  # File capabilities can restrict proc FD visibility.


def security_facts(pid):
    fields = dict(row.split(b":", 1) for row in bounded(Path("/proc") / str(pid) / "status").splitlines())
    return {"nnp": int(fields[b"NoNewPrivs"].strip()),
            "effective_capabilities": int(fields[b"CapEff"].strip(), 16)}


def acceptance(options):
    private_source = getattr(options, "private_vless_store", None)
    real = private_source is not None
    require(not real or options.mode == "global", "private_fixture_requires_full_vpn")
    private_profile = load_private_vless(private_source) if real else None
    binary = Path(options.binary)
    require(binary.is_absolute() and binary.resolve() == binary, "binary_not_canonical")
    require(stat.S_ISREG(binary.lstat().st_mode) and os.access(binary, os.X_OK), "binary_not_executable")
    runtime_base = Path("/run/user") / str(os.getuid())
    metadata = runtime_base.lstat()
    require(stat.S_ISDIR(metadata.st_mode) and metadata.st_uid == os.getuid()
            and stat.S_IMODE(metadata.st_mode) == 0o700, "runtime_directory_unsafe")
    root = None
    cgroup = None
    binary_hash = hashlib.sha256(binary.read_bytes()).hexdigest()
    captures = 0

    def command(args, env=None, timeout=120, checked=True):
        nonlocal captures
        result = subprocess.run(args, env=env, stdin=subprocess.DEVNULL, capture_output=True, timeout=timeout)
        require(len(result.stdout) + len(result.stderr) <= 1048576, "command_output_oversized")
        if root is not None:
            captures += 1
            private_write(root / f"capture-{captures}.stdout", result.stdout)
            private_write(root / f"capture-{captures}.stderr", result.stderr)
        require(not checked or result.returncode == 0, "command_rejected")
        return result

    def tuns():
        links = json.loads(command(["ip", "-d", "-j", "link", "show"]).stdout)
        return {link["ifname"] for link in links if link.get("linkinfo", {}).get("info_kind") == "tun"}

    require(not any(name in (b"mihomo", b"omavless") for name in processes().values()), "baseline_runtime_present")
    require(not tuns(), "baseline_tun_present")
    baseline_listeners = tcp_listeners()
    root = Path(tempfile.mkdtemp(prefix="omavless-native-gate-", dir=runtime_base))
    token = uuid.uuid4().hex[:10]
    unit, tun = "omavless-native-acceptance-" + token, "ovna" + token
    started, passed = False, False
    config, state, runtime = root / "home/.config/omavless", root / "state/omavless", root / "runtime"
    try:
        for directory in (config, state, runtime):
            directory.mkdir(parents=True, mode=0o700)
        profile = PUBLIC_PROFILE_ID
        store = {"version": 3, "activeId": "", "lastId": profile, "profiles": [{"id": profile,
            "name": "Synthetic", "uri": "vless://11111111-1111-4111-8111-111111111111@192.0.2.1:443?security=none&type=tcp#Synthetic",
            "protocol": "vless", "subscriptionId": "", "subscriptionKey": "", "missing": False, "favorite": False}],
            "subscriptions": [], "routingPreset": "custom", "customRules": [], "rulesUpdatedAt": 0,
            "startupConfigured": True, "startup": {"enabled": False, "target": "last", "profileId": "", "mode": "rule"}, "onboardingComplete": True}
        if real:
            store["profiles"] = [private_profile]
        private_write(config / "profiles.json", json.dumps(store))
        private_write(config / "route-template.yaml", route_template(tun, real))
        private_write(state / "ownership.json", json.dumps({"schemaVersion": 1, "generation": 2, "phase": "rust"}))
        overrides = {"OMAVLESS_HOME": str(root / "home"), "XDG_STATE_HOME": str(root / "state"),
                     "XDG_RUNTIME_DIR": str(runtime), "OMAVLESS_MIHOMO": "/usr/bin/mihomo"}
        env = dict(os.environ, **overrides)
        args = ["systemd-run", "--user", "--collect", "--unit=" + unit, "-p", "NoNewPrivileges=no",
                "-p", "LimitCORE=0", "-p", "UMask=0077"]
        args += ["--setenv=" + key + "=" + value for key, value in overrides.items()]
        emit(binary_sha256=binary_hash, mode=options.mode, private_fixture=real,
             family="vless", authorization="normal_polkit_dialogs_may_appear")
        started = True  # Even a failed/timed-out start can leave a unit to clean.
        command(args + [str(binary), "daemon"])
        control = runtime / "omavless/control.sock"
        deadline = time.monotonic() + 30
        while not control.exists() and time.monotonic() < deadline:
            time.sleep(0.05)
        metadata = control.lstat()
        require(stat.S_ISSOCK(metadata.st_mode) and stat.S_IMODE(metadata.st_mode) == 0o600
                and metadata.st_uid == os.getuid(), "semantic_socket_private")
        require(json.loads(command([str(binary), "hello"], env).stdout).get("ok") is True, "hello_rejected")
        pid = int(command(["systemctl", "--user", "show", unit, "-p", "MainPID", "--value"]).stdout)
        group = command(["systemctl", "--user", "show", unit, "-p", "ControlGroup", "--value"]).stdout.decode().strip()
        cgroup = (Path("/sys/fs/cgroup") / group.lstrip("/")).resolve()
        require(cgroup.is_relative_to("/sys/fs/cgroup") and cgroup.name == unit + ".service", "service_cgroup_identity")
        daemon_security = security_facts(pid)
        require(daemon_security == {"nnp": 0, "effective_capabilities": 0}, "daemon_privilege_policy")
        for index in range(options.repetitions):
            connect_started = time.monotonic()
            command([str(binary), "connect", profile, options.mode], env)
            connect_ms = round((time.monotonic() - connect_started) * 1000)
            require(json.loads(command([str(binary), "status"], env).stdout)["result"]["actual"] == "connected", "connect_state")
            members = set(map(int, bounded(cgroup / "cgroup.procs").split()))
            cores = {member for member, name in processes().items() if name == b"mihomo"}
            require(len(cores) == 1 and cores <= members and cores <= descendants(pid), "core_ownership")
            require(tuns() == {tun}, "connected_tun_count")
            core = next(iter(cores))
            require(os.getpgid(core) == core, "owned_core_process_group")
            core_security = security_facts(core)
            require(core_security["nnp"] == 0 and core_security["effective_capabilities"] & 0x3400 == 0x3400,
                    "core_capability_policy")
            core_controller(runtime / "omavless/mihomo.sock", core, options.mode)
            listeners = tcp_listeners()
            tcp = tcp_absent(core, listeners)
            no_new_listener = not bool(listeners - baseline_listeners)
            no_tcp_config = not re.search(rb'''(?m)^[ \t]*["']?external-controller["']?[ \t]*:''',
                                          bounded(config / "config.yaml"))
            probe_passed = False
            if real:
                before_probe = tun_counters(tun)
                probe = command(https_probe_args(tun), timeout=25, checked=False)
                after_probe = tun_counters(tun)
                probe_passed, tun_used, probe_class = https_probe_evidence(probe, before_probe, after_probe)
                emit(case=index + 1, family="vless", https_probe=probe_passed,
                     tun_probe_evidence=tun_used, classification=probe_class)
                require(probe_passed and tun_used, probe_class)
            disconnect_started = time.monotonic()
            command([str(binary), "disconnect"], env)
            disconnect_ms = round((time.monotonic() - disconnect_started) * 1000)
            require(json.loads(command([str(binary), "status"], env).stdout)["result"]["actual"] == "disconnected", "disconnect_state")
            require(not tuns() and not any(name == b"mihomo" for name in processes().values()), "disconnect_resources")
            require(set(map(int, bounded(cgroup / "cgroup.procs").split())) == {pid}, "disconnect_helpers")
            emit(case=index + 1, connect=True, disconnect=True, tun_cleanup=True,
                 daemon_unprivileged=True, core_capabilities=True, owned_process_group=True,
                 connect_ms=connect_ms, disconnect_ms=disconnect_ms,
                 real_fixture=real, https_probe=probe_passed if real else "NOT RUN",
                 unix_controller=True, tcp_pid_attribution="NOT PROVEN" if tcp is None else tcp,
                 no_new_host_tcp_listener=no_new_listener, no_tcp_controller_config=bool(no_tcp_config))
            require(tcp is not False and no_new_listener and no_tcp_config, "tcp_listener_evidence_failed")
        require(hashlib.sha256(binary.read_bytes()).hexdigest() == binary_hash, "binary_changed_during_test")
        passed = True
    finally:
        clean = not started
        if started:
            # A failed connect/probe can leave an owned core. Always request its
            # semantic shutdown, then independently stop and verify the unit.
            try:
                command([str(binary), "disconnect"], env, checked=False)
            except Exception:
                pass  # Unit cleanup below is mandatory even on IPC failure.
            try:
                command(["systemctl", "--user", "stop", unit], timeout=600, checked=False)
                active = command(["systemctl", "--user", "is-active", "--quiet", unit], checked=False)
                clean = active.returncode in (3, 4) and not tuns() and not any(
                    name in (b"mihomo", b"omavless") for name in processes().values())
                if cgroup is not None and cgroup.exists():
                    clean = clean and not bounded(cgroup / "cgroup.procs").strip()
            except Exception:
                clean = False
        emit(cleanup=clean, diagnostic_files_retained=not (passed and clean))
        if passed and clean:
            shutil.rmtree(root)
        require(clean, "manual_cleanup_required")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--run", action="store_true", help="explicitly authorize isolated user-service/TUN smoke")
    parser.add_argument("--binary", required=True, help="absolute canonical exact candidate executable")
    parser.add_argument("--mode", choices=("global", "rule", "direct"), default="global")
    parser.add_argument("--repetitions", type=int, choices=range(1, 11), default=1)
    parser.add_argument("--private-vless-store", metavar="ABS_PATH",
                        help="read one existing private VLESS fixture; global only, temporary auto-route + bounded TUN HTTPS")
    options = parser.parse_args()
    require(options.run, "explicit_run_required")
    acceptance(options)


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        emit(result="FAIL", classification=str(error) if isinstance(error, Failure) else "host_probe_failed")
        raise SystemExit(1)
