#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""Opt-in developer WG smoke. Never reads profiles or changes installed units.

All effects are in a newly owned user/network namespace; missing prerequisites
refuse. Package/module preparation is external, separately authorized work.
Only fixed safe result classes escape; peer credentials and core logs do not.
"""
import argparse
import hashlib
import http.server
import json
import os
from pathlib import Path
import re
import resource
import shutil
import signal
import socket
import stat
import struct
import subprocess
import sys
import tempfile
import time

ENV = {"PATH": "/usr/bin:/bin", "LANG": "C.UTF-8"}
RESPONSE = b"P4_WG_LOOPBACK_OK\n"
DEVICE = "wg-p4"
BODY_LIMIT = 131072


class Refused(Exception):
    def __init__(self, stage):
        # Stages are fixed literals controlled by this harness, never raw errors.
        self.stage = stage
        super().__init__(stage)


def require(condition, stage):
    if not condition:
        raise Refused(stage)


def command(argv, stage, *, data=None, timeout=5, okay=(0,), env=None):
    try:
        result = subprocess.run(argv, input=data, stdout=subprocess.PIPE,
                                stderr=subprocess.PIPE, env=ENV if env is None else env, timeout=timeout,
                                check=False)
    except (OSError, subprocess.TimeoutExpired):
        raise Refused(stage) from None
    require(result.returncode in okay and len(result.stdout) <= BODY_LIMIT
            and len(result.stderr) <= BODY_LIMIT, stage)
    return result


def private_write(path, payload, mode=0o600):
    descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, mode)
    with os.fdopen(descriptor, "wb") as output:
        output.write(payload)
        output.flush()
        os.fsync(output.fileno())


def private_directory(path):
    metadata = path.lstat()
    return stat.S_ISDIR(metadata.st_mode) and metadata.st_uid == os.getuid() \
        and metadata.st_mode & 0o077 == 0


def namespace(name, pid="self"):
    return os.stat(f"/proc/{pid}/ns/{name}").st_ino


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def copy_binary(source, target, expected):
    require(source.is_absolute() and re.fullmatch(r"[a-f0-9]{64}", expected), "binary_identity")
    with source.open("rb") as handle:
        before = os.fstat(handle.fileno())
        require(stat.S_ISREG(before.st_mode) and before.st_mode & 0o022 == 0
                and before.st_uid in (0, os.getuid()) and before.st_size <= 128 * 1024 * 1024,
                "binary_policy")
        data = handle.read(128 * 1024 * 1024 + 1)
        after = os.fstat(handle.fileno())
    require((before.st_dev, before.st_ino, before.st_size, before.st_mtime_ns, before.st_ctime_ns)
            == (after.st_dev, after.st_ino, after.st_size, after.st_mtime_ns, after.st_ctime_ns)
            and len(data) == before.st_size and hashlib.sha256(data).hexdigest() == expected,
            "binary_identity")
    private_write(target, data, 0o500)  # New inode: never copy file capabilities/xattrs.
    require(command(["/usr/bin/getcap", str(target)], "binary_capabilities").stdout == b"",
            "binary_capabilities")


def dropped(argv):
    return ["/usr/bin/setpriv", "--no-new-privs", "--bounding-set=-all",
            "--inh-caps=-all", "--ambient-caps=-all", "--", *map(str, argv)]


def check_process(process, net):
    require(process.poll() is None and namespace("net", process.pid) == net, "child_identity")
    values = dict(line.split(":", 1) for line in Path(f"/proc/{process.pid}/status").read_text().splitlines()
                  if ":" in line)
    require(values.get("NoNewPrivs", "").strip() == "1"
            and all(int(values.get(field, "1").strip(), 16) == 0
                    for field in ("CapEff", "CapPrm", "CapAmb")), "child_privileges")


def stop(process):
    if process is None:
        return
    if process.poll() is None:
        process.terminate()
        try:
            process.wait(timeout=3)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait(timeout=3)
    require(process.poll() is not None, "owned_child_cleanup")


def log_handle(path):
    descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
    return os.fdopen(descriptor, "wb")


def child_limit():
    resource.setrlimit(resource.RLIMIT_FSIZE, (2 * 1024 * 1024, 2 * 1024 * 1024))
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))


def launch(argv, log, pass_fds=()):
    return subprocess.Popen(dropped(argv), stdin=subprocess.DEVNULL, stdout=log, stderr=log,
                            env=ENV, preexec_fn=child_limit, pass_fds=pass_fds)


def controller(path, pid):
    with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as client:
        client.settimeout(1)
        client.connect(str(path))
        peer_pid, _, _ = struct.unpack("3i", client.getsockopt(socket.SOL_SOCKET, socket.SO_PEERCRED, 12))
        require(peer_pid == pid, "controller_peer")
        client.sendall(b"GET /proxies/PROXY HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
        payload = bytearray()
        while True:
            part = client.recv(4096)
            if not part:
                break
            payload.extend(part)
            require(len(payload) <= BODY_LIMIT, "controller_bound")
    header, body = bytes(payload).split(b"\r\n\r\n", 1)
    require(header.startswith(b"HTTP/1.1 200 ") and b"Transfer-Encoding:" not in header,
            "controller_frame")
    group = json.loads(body)
    require(group.get("type") == "Selector" and group.get("now") == "P4 loopback"
            and group.get("all") == ["P4 loopback"], "no_direct_fallback")


def peer_stats(public_key):
    handshake = command(["/usr/bin/wg", "show", DEVICE, "latest-handshakes"], "peer_observation").stdout.splitlines()
    transfer = command(["/usr/bin/wg", "show", DEVICE, "transfer"], "peer_observation").stdout.splitlines()
    require(len(handshake) == len(transfer) == 1, "peer_count")
    h = handshake[0].split(); t = transfer[0].split()
    require(len(h) == 2 and len(t) == 3 and h[0] == t[0] == public_key.strip(), "peer_identity")
    return int(h[1]), int(t[1]), int(t[2])


def generate_round(root):
    server = command(["/usr/bin/wg", "genkey"], "key_generation").stdout
    client = command(["/usr/bin/wg", "genkey"], "key_generation").stdout
    wrong = command(["/usr/bin/wg", "genkey"], "key_generation").stdout
    psk = command(["/usr/bin/wg", "genpsk"], "key_generation").stdout
    require(client != wrong, "negative_key_identity")
    public_server = command(["/usr/bin/wg", "pubkey"], "key_generation", data=server).stdout
    public_client = command(["/usr/bin/wg", "pubkey"], "key_generation", data=client).stdout
    for name, value in (("server.key", server), ("client.key", client), ("wrong.key", wrong), ("psk.key", psk)):
        private_write(root / name, value)
    server_conf = b"[Interface]\nPrivateKey = " + server.strip() + b"\nListenPort = 51888\n[Peer]\nPublicKey = " + public_client.strip() + b"\nPresharedKey = " + psk.strip() + b"\nAllowedIPs = 10.203.0.2/32\n"
    private_write(root / "server.conf", server_conf)
    for name, key in (("positive", client), ("negative", wrong)):
        conf = b"[Interface]\nPrivateKey = " + key.strip() + b"\nAddress = 10.203.0.2/32\nMTU = 1420\n[Peer]\nPublicKey = " + public_server.strip() + b"\nPresharedKey = " + psk.strip() + b"\nAllowedIPs = 10.203.0.1/32\nEndpoint = 127.0.0.1:51888\nPersistentKeepalive = 1\n"
        private_write(root / f"{name}.conf", conf)
    command(["/usr/bin/wg", "setconf", DEVICE, str(root / "server.conf")], "peer_configuration")
    return public_client


def phase(root, core, phase_name, attempt, public_key, net, *, observe=None):
    observe = peer_stats if observe is None else observe
    config = root / f"{phase_name}.yaml"
    require(config.lstat().st_mode & 0o077 == 0, "config_policy")
    # Validation never receives secret bytes through argv or environment.
    with log_handle(root / f"validate-{attempt}.log") as log:
        tested = launch([core, "-t", "-d", root, "-f", config], log)
        try:
            require(tested.wait(timeout=5) == 0, "core_validation")
        finally:
            stop(tested)
    sock = root / "mihomo.sock"
    require(not sock.exists(), "controller_collision")
    before = observe(public_key)
    started = int(time.time())
    child = None
    try:
        with log_handle(root / f"core-{attempt}.log") as log:
            child = launch([core, "-d", root, "-f", config], log)
            deadline = time.monotonic() + 5
            while True:
                require(child.poll() is None and time.monotonic() < deadline, "core_readiness")
                if sock.exists():
                    try:
                        controller(sock, child.pid)
                        break
                    except (OSError, ValueError, Refused):
                        pass
                time.sleep(0.05)
            sock.chmod(0o600)
            check_process(child, net)
            result = command(dropped(["/usr/bin/curl", "--silent", "--show-error", "--fail",
                                      "--noproxy", "", "--socks5", "127.0.0.1:7898",
                                      "--max-time", "3", "--max-filesize", "128",
                                      "http://10.203.0.1:8089/"]), "transport_request",
                             timeout=5, okay=tuple(range(256)))
            after = observe(public_key)
            if phase_name == "positive":
                require(result.returncode == 0 and result.stdout == RESPONSE,
                        "positive_transport")
                require(after[0] >= started - 2 and after[1] > before[1] and after[2] > before[2],
                        "handshake_transfer")
            else:
                require(result.returncode != 0 and result.stdout == b"", "negative_direct_bypass")
                require(after[0] == before[0] and after[1:] == before[1:], "negative_peer_unchanged")
            check_process(child, net)
    finally:
        stop(child)
        if sock.exists():
            require(stat.S_ISSOCK(sock.lstat().st_mode), "controller_cleanup")
            sock.unlink()


def namespace_run(args):
    root = Path(args.scratch)
    require(private_directory(root) and os.getuid() == 0
            and os.getppid() == int(args.parent_pid)
            and os.fstat(int(args.parent_net_fd)).st_ino == int(args.parent_net)
            and os.fstat(int(args.parent_user_fd)).st_ino == int(args.parent_user)
            and namespace("net") != int(args.parent_net)
            and namespace("user") != int(args.parent_user), "namespace_identity")
    private_write(root / "namespace-identity.json", json.dumps({
        "net": namespace("net"), "user": namespace("user")}).encode())
    links = json.loads(command(["/usr/bin/ip", "-j", "link", "show"], "namespace_inventory").stdout)
    require([row["ifname"] for row in links] == ["lo"], "namespace_interfaces")
    routes = json.loads(command(["/usr/bin/ip", "-j", "route", "show", "table", "all"], "namespace_inventory").stdout)
    require(not routes, "namespace_routes")
    command(["/usr/bin/ip", "link", "set", "lo", "up"], "namespace_loopback")
    command(["/usr/bin/ip", "link", "add", DEVICE, "type", "wireguard"], "namespace_wireguard")
    http = None
    http_net_fd = None
    result = {"positive": 0, "negative": 0, "recovery": 0, "private_roundtrip": 0,
              "net_inode": namespace("net"), "user_inode": namespace("user")}
    try:
        command(["/usr/bin/ip", "address", "add", "10.203.0.1/32", "dev", DEVICE], "peer_address")
        command(["/usr/bin/ip", "link", "set", DEVICE, "up"], "peer_link")
        command(["/usr/bin/ip", "route", "add", "10.203.0.2/32", "dev", DEVICE], "peer_return_route")
        with log_handle(root / "http.log") as log:
            http_net_fd = os.open("/proc/self/ns/net", os.O_RDONLY | os.O_CLOEXEC)
            http = launch([sys.executable, Path(__file__).resolve(), "--http-child",
                           "--parent-pid", str(os.getpid()),
                           "--parent-net-fd", str(http_net_fd)], log, pass_fds=(http_net_fd,))
            deadline = time.monotonic() + 3
            while True:
                require(http.poll() is None and time.monotonic() < deadline, "http_readiness")
                try:
                    with socket.create_connection(("10.203.0.1", 8089), timeout=0.1):
                        break
                except OSError:
                    time.sleep(0.05)
            check_process(http, result["net_inode"])
            for number in range(args.rounds):
                round_root = root / f"r{number + 1}"
                round_root.mkdir(mode=0o700)
                public = generate_round(round_root)
                require(peer_stats(public) == (0, 0, 0), "fresh_peer")
                for name in ("positive", "negative"):
                    rendered = command(dropped([args.renderer, str(round_root), name]), "private_render")
                    require(json.loads(rendered.stdout) == {"private_roundtrip": True, "flavor": "wireguard"}, "private_roundtrip")
                    result["private_roundtrip"] += 1
                phase(round_root, Path(args.core), "positive", "positive", public, result["net_inode"])
                result["positive"] += 1
                phase(round_root, Path(args.core), "negative", "negative", public, result["net_inode"])
                result["negative"] += 1
                phase(round_root, Path(args.core), "positive", "recovery", public, result["net_inode"])
                result["recovery"] += 1
            check_process(http, result["net_inode"])
    finally:
        stop(http)
        if http_net_fd is not None:
            os.close(http_net_fd)
        command(["/usr/bin/ip", "link", "del", DEVICE], "namespace_interface_cleanup")
    result["interface_cleanup"] = True
    print(json.dumps(result, sort_keys=True))


def outside_snapshot():
    network = []
    for argv in (["link", "show"], ["address", "show"], ["route", "show", "table", "all"],
                 ["-6", "route", "show", "table", "all"]):
        rows = json.loads(command(["/usr/bin/ip", "-j", *argv], "outside_snapshot").stdout)
        # Lifetime countdowns/traffic counters are not mutations by this fixture.
        def stable(value):
            if isinstance(value, dict):
                return {key: stable(item) for key, item in value.items()
                        if key not in ("valid_life_time", "preferred_life_time", "expires", "stats64", "stats")}
            if isinstance(value, list):
                return [stable(item) for item in value]
            return value
        network.append(stable(rows))
    runtime_dir = Path(f"/run/user/{os.getuid()}")
    require(private_directory(runtime_dir), "runtime_snapshot")
    runtime = command(["/usr/bin/systemctl", "--user", "show", "omavless-runtime.service",
                       "-p", "MainPID", "-p", "ActiveState", "-p", "SubState"], "runtime_snapshot",
                      env={**ENV, "XDG_RUNTIME_DIR": str(runtime_dir)}).stdout
    core_pids = command(["/usr/bin/pgrep", "-x", "mihomo"], "runtime_snapshot", okay=(0, 1)).stdout
    return hashlib.sha256(json.dumps(network, sort_keys=True).encode()).hexdigest(), runtime, core_pids


def outer(args):
    require(args.run and os.getuid() != 0 and 1 <= args.rounds <= 5, "explicit_vm_opt_in")
    require(re.fullmatch(r"[a-f0-9]{40}", args.source_sha or ""), "source_identity")
    require(Path("/sys/module/wireguard").is_dir(), "stock_wireguard_unavailable")
    for tool in ("unshare", "ip", "wg", "curl", "setpriv", "getcap"):
        require(Path(f"/usr/bin/{tool}").is_file(), "missing_tool")
    before = outside_snapshot()
    harness_identity = digest(Path(__file__).resolve())
    cache = Path.home() / ".cache" / "omavless-p4-loopback"
    cache.mkdir(mode=0o700, parents=False, exist_ok=True)
    require(private_directory(cache), "scratch_parent")
    root = Path(tempfile.mkdtemp(prefix="run-", dir=cache))
    outcome = None
    child = None
    parent_fds = []
    try:
        copy_binary(Path(args.core), root / "core", args.expected_core_sha256)
        copy_binary(Path(args.renderer), root / "renderer", args.expected_renderer_sha256)
        parent_fds.append(os.open("/proc/self/ns/net", os.O_RDONLY | os.O_CLOEXEC))
        parent_fds.append(os.open("/proc/self/ns/user", os.O_RDONLY | os.O_CLOEXEC))
        argv = ["/usr/bin/unshare", "--user", "--map-root-user", "--net", sys.executable,
                str(Path(__file__).resolve()), "--namespace-child", "--scratch", str(root),
                "--parent-net", str(namespace("net")), "--parent-user", str(namespace("user")),
                "--parent-pid", str(os.getpid()),
                "--parent-net-fd", str(parent_fds[0]), "--parent-user-fd", str(parent_fds[1]),
                "--core", str(root / "core"), "--renderer", str(root / "renderer"), "--rounds", str(args.rounds)]
        with log_handle(root / "namespace-result.json") as log:
            child = subprocess.Popen(argv, stdin=subprocess.DEVNULL, stdout=log, stderr=log,
                                     env=ENV, start_new_session=True, pass_fds=parent_fds)
            try:
                return_code = child.wait(timeout=100)
            except subprocess.TimeoutExpired:
                raise Refused("namespace_timeout") from None
        data = (root / "namespace-result.json").read_bytes()
        require(len(data) <= BODY_LIMIT, "namespace_result_bound")
        outcome = json.loads(data)
        if return_code != 0:
            # Never echo an arbitrary child/log fragment. Known harness classes only.
            safe_stages = {"namespace_identity", "namespace_interfaces", "namespace_routes",
                           "namespace_wireguard", "child_privileges", "child_identity",
                           "private_render", "private_roundtrip", "core_validation",
                           "core_readiness", "positive_transport", "handshake_transfer",
                           "negative_direct_bypass", "negative_peer_unchanged", "fresh_peer",
                           "peer_configuration", "peer_observation", "controller_cleanup",
                           "namespace_interface_cleanup", "http_readiness", "http_namespace",
                           "namespace_loopback", "namespace_inventory", "peer_address", "peer_link",
                           "peer_return_route", "key_generation", "peer_count", "peer_identity",
                           "config_policy", "controller_collision", "transport_request",
                           "controller_peer", "controller_frame", "controller_bound", "no_direct_fallback",
                           "owned_child_cleanup", "unexpected_fixed_failure"}
            stage = outcome.get("stage") if isinstance(outcome, dict) else None
            raise Refused(stage if stage in safe_stages else "namespace_smoke")
        require(outcome.get("positive") == outcome.get("negative") == outcome.get("recovery") == args.rounds
                and outcome.get("interface_cleanup") is True, "namespace_result")
    finally:
        if child is not None:
            # The group/session is exclusively created by this Popen. Also
            # terminate residual owned children if its supervisor already exited.
            for action in (signal.SIGTERM, signal.SIGKILL):
                try:
                    os.killpg(child.pid, action)
                except ProcessLookupError:
                    break
                if child.poll() is None:
                    try:
                        child.wait(timeout=3)
                    except subprocess.TimeoutExpired:
                        pass
            require(child.poll() is not None, "owned_namespace_cleanup")
        for descriptor in parent_fds:
            os.close(descriptor)
        identity_file = root / "namespace-identity.json"
        if identity_file.exists():
            identity = json.loads(identity_file.read_bytes())
            remaining = []
            for entry in Path("/proc").iterdir():
                if entry.name.isdecimal():
                    try:
                        if namespace("net", entry.name) == identity["net"]:
                            remaining.append(entry.name)
                    except (OSError, PermissionError):
                        pass
            require(not remaining, "namespace_process_cleanup")
        require(root.parent == cache and private_directory(root), "scratch_cleanup_target")
        shutil.rmtree(root)
        require(not root.exists(), "scratch_cleanup")
        require(outside_snapshot() == before, "outside_state_changed")
        require(digest(Path(__file__).resolve()) == harness_identity, "harness_identity")
    outcome.update({"status": "PASS", "source_sha": args.source_sha,
                    "harness_sha256": digest(Path(__file__).resolve()),
                    "core_sha256": args.expected_core_sha256,
                    "renderer_sha256": args.expected_renderer_sha256,
                    "scratch_cleanup": True, "outside_state_unchanged": True,
                    "installed_runtime_unchanged": True})
    print(json.dumps(outcome, sort_keys=True))


def http_child(args):
    require(os.getuid() == 0 and os.getppid() == int(args.parent_pid)
            and namespace("net") == os.fstat(int(args.parent_net_fd)).st_ino, "http_namespace")
    links = json.loads(command(["/usr/bin/ip", "-j", "link", "show"], "http_namespace").stdout)
    require(sorted(row["ifname"] for row in links) == ["lo", DEVICE], "http_namespace")
    class Handler(http.server.BaseHTTPRequestHandler):
        def do_GET(self):
            self.send_response(200)
            self.send_header("Content-Length", str(len(RESPONSE)))
            self.end_headers()
            self.wfile.write(RESPONSE)

        def log_message(self, *_args):
            pass
    http.server.HTTPServer(("10.203.0.1", 8089), Handler).serve_forever()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--run", action="store_true")
    parser.add_argument("--namespace-child", action="store_true", help=argparse.SUPPRESS)
    parser.add_argument("--http-child", action="store_true", help=argparse.SUPPRESS)
    parser.add_argument("--scratch", help=argparse.SUPPRESS)
    parser.add_argument("--parent-net", help=argparse.SUPPRESS)
    parser.add_argument("--parent-user", help=argparse.SUPPRESS)
    parser.add_argument("--parent-pid", help=argparse.SUPPRESS)
    parser.add_argument("--parent-net-fd", help=argparse.SUPPRESS)
    parser.add_argument("--parent-user-fd", help=argparse.SUPPRESS)
    parser.add_argument("--core", default="/usr/bin/mihomo")
    parser.add_argument("--renderer")
    parser.add_argument("--expected-core-sha256")
    parser.add_argument("--expected-renderer-sha256")
    parser.add_argument("--source-sha")
    parser.add_argument("--rounds", type=int, default=3)
    args = parser.parse_args()
    try:
        if args.http_child:
            http_child(args)
        elif args.namespace_child:
            namespace_run(args)
        else:
            outer(args)
    except Refused as error:
        print(json.dumps({"status": "REFUSED", "stage": error.stage}, sort_keys=True))
        return 2
    except (Exception, KeyboardInterrupt):
        print(json.dumps({"status": "REFUSED", "stage": "unexpected_fixed_failure"}, sort_keys=True))
        return 2
    return 0


if __name__ == "__main__":
    sys.exit(main())
