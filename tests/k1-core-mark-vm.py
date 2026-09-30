#!/usr/bin/env python3
"""Opt-in, VM-only Mihomo socket-mark probe in a fresh loopback-only netns.

Developer test tooling, not production Python. No OmaVLESS service, TUN,
physical network, profile or private core configuration is used.
"""

import http.server
import json
import os
import pathlib
import socket
import subprocess
import sys
import tempfile
import threading
import time


CORE = "/usr/lib/omavless-dns/mihomo"
CORE_MARK = 0x4F4D4101
OTHER_MARK = 524288
TABLE = "omavless_k1_mark_probe"
PROXY_PORT = 17890
SERVER_PORT = 18080
SERVER_IP = "192.0.2.2"
TOKEN = b"K1_MARK_OK\n"


def namespace_id(path):
    stat = os.stat(path)
    return stat.st_dev, stat.st_ino


def check_child():
    fd = int(os.environ["K1_PARENT_FD"])
    parent = os.fstat(fd)
    expected = (int(os.environ["K1_PARENT_DEV"]), int(os.environ["K1_PARENT_INO"]))
    if (parent.st_dev, parent.st_ino) != expected:
        raise RuntimeError("namespace")
    if os.readlink(f"/proc/self/fd/{fd}") != f"net:[{expected[1]}]":
        raise RuntimeError("namespace")
    child = namespace_id("/proc/self/ns/net")
    if child[0] != expected[0] or child == expected:
        raise RuntimeError("namespace")
    with open("/proc/net/dev", encoding="ascii") as stream:
        lines = stream.read(8193).splitlines()
    if len(lines) != 3 or lines[2].split(":", 1)[0].strip() != "lo":
        raise RuntimeError("interfaces")


def command(argv, *, input_bytes=None, timeout=5):
    check_child()
    result = subprocess.run(
        argv,
        input=input_bytes,
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
        timeout=timeout,
        check=False,
        env={"PATH": "/usr/bin:/bin", "LC_ALL": "C"},
    )
    if result.returncode != 0 or len(result.stdout) > 32768:
        raise RuntimeError("command")
    check_child()
    return result.stdout


def table_commands():
    # Both counters see the same synthetic core request; the first only when
    # the packet carries the K1 mark, the second regardless of mark.
    return (f"""table inet {TABLE} {{
  chain output {{
    type filter hook output priority 100; policy accept;
    ip daddr {SERVER_IP} meta mark {CORE_MARK} counter
    ip daddr {SERVER_IP} counter
  }}
}}
""").encode("ascii")


def counters():
    data = command(["/usr/bin/nft", "--json", "--numeric", "list", "table", "inet", TABLE])
    objects = json.loads(data)["nftables"]
    rules = [item["rule"] for item in objects if "rule" in item]
    if len(rules) != 2 or any(rule.get("table") != TABLE for rule in rules):
        raise RuntimeError("readback")
    values = []
    for rule in rules:
        found = [entry["counter"]["packets"] for entry in rule["expr"] if "counter" in entry]
        if len(found) != 1 or not isinstance(found[0], int):
            raise RuntimeError("readback")
        values.append(found[0])
    return values


class Handler(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        self.send_response(200)
        self.send_header("Content-Length", str(len(TOKEN)))
        self.end_headers()
        self.wfile.write(TOKEN)

    def log_message(self, *_):
        pass


def stop(proc):
    if proc.poll() is None:
        proc.terminate()
        try:
            proc.wait(timeout=3)
        except subprocess.TimeoutExpired:
            proc.kill()
            proc.wait(timeout=3)


def phase(scratch, mark, should_match):
    check_child()
    config = scratch / "config.yaml"
    config.write_text(
        f"""mixed-port: {PROXY_PORT}
allow-lan: false
bind-address: 127.0.0.1
mode: direct
log-level: silent
ipv6: false
find-process-mode: off
routing-mark: {mark}
tun:
  enable: false
dns:
  enable: false
proxies: []
proxy-groups: []
rules:
  - MATCH,DIRECT
""",
        encoding="ascii",
    )
    config.chmod(0o600)
    command(["/usr/bin/nft", "-f", "-"], input_bytes=table_commands())
    proc = subprocess.Popen(
        [CORE, "-d", str(scratch), "-f", str(config)],
        stdin=subprocess.DEVNULL,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        env={"PATH": "/usr/bin:/bin", "HOME": str(scratch), "TMPDIR": str(scratch)},
    )
    try:
        deadline = time.monotonic() + 8
        while time.monotonic() < deadline:
            check_child()
            if proc.poll() is not None:
                raise RuntimeError("core")
            try:
                with socket.create_connection(("127.0.0.1", PROXY_PORT), timeout=0.2):
                    break
            except OSError:
                time.sleep(0.05)
        else:
            raise RuntimeError("core")
        request = ["/usr/bin/curl", "--silent", "--show-error", "--fail", "--max-time", "5",
                   "--noproxy", "", "--proxy", f"http://127.0.0.1:{PROXY_PORT}",
                   f"http://{SERVER_IP}:{SERVER_PORT}/"]
        body = command(request, timeout=7)
        if body != TOKEN:
            raise RuntimeError("response")
        marked, total = counters()
        if total < 1:
            raise RuntimeError("no-egress")
        if marked > total:
            raise RuntimeError("readback")
        if should_match and marked == 0:
            raise RuntimeError("mark-missing")
        if not should_match and marked > 0:
            raise RuntimeError("wrong-mark")
        # The positive response must depend on Mihomo, not on a curl bypass.
        stop(proc)
        check_child()
        without_proxy = subprocess.run(
            request,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            timeout=7,
            check=False,
            env={"PATH": "/usr/bin:/bin", "LC_ALL": "C"},
        )
        check_child()
        if without_proxy.returncode == 0:
            raise RuntimeError("proxy-bypass")
    finally:
        stop(proc)
        command(["/usr/bin/nft", "delete", "table", "inet", TABLE])


def child():
    stage = "isolation"
    try:
        check_child()
        command(["/usr/bin/ip", "link", "set", "lo", "up"])
        # Documentation-only address is assigned to the child loopback. Its
        # destination is global-unicast-shaped to Mihomo's socket-mark code,
        # but cannot leave this netns (no other interface or default route).
        command(["/usr/bin/ip", "address", "add", f"{SERVER_IP}/32", "dev", "lo"])
        for family in ("-4", "-6"):
            if json.loads(command(["/usr/bin/ip", "-j", family, "route", "show", "default"])):
                raise RuntimeError("route")
        stage = "server"
        server = http.server.ThreadingHTTPServer((SERVER_IP, SERVER_PORT), Handler)
        server.daemon_threads = True
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        try:
            with tempfile.TemporaryDirectory(prefix="k1mark.", dir=os.environ["K1_SCRATCH_PARENT"]) as root:
                scratch = pathlib.Path(root)
                scratch.chmod(0o700)
                stage = "candidate"
                phase(scratch, CORE_MARK, True)
                stage = "control"
                phase(scratch, OTHER_MARK, False)
        finally:
            server.shutdown()
            server.server_close()
        check_child()
    except Exception as error:
        reason = str(error) if isinstance(error, RuntimeError) else "unavailable"
        if reason not in {"namespace", "interfaces", "route", "command", "readback", "core", "response", "no-egress", "mark-missing", "wrong-mark", "proxy-bypass"}:
            reason = "unavailable"
        print("K1_CORE_MARK_FAILED=" + stage + "." + reason, flush=True)
        return 1
    print("K1_CORE_MARK_PASS", flush=True)
    return 0


def parent():
    if os.environ.get("OMAVLESS_K1_CORE_MARK_VM") != "1" or len(sys.argv) != 1:
        raise SystemExit("VM opt-in required")
    if not os.path.isfile(CORE) or not os.access(CORE, os.X_OK):
        raise SystemExit("packaged core unavailable")
    descriptor = os.open("/proc/self/ns/net", os.O_RDONLY)
    identity = os.fstat(descriptor)
    cache = pathlib.Path.home() / ".cache"
    result = subprocess.run(
        ["/usr/bin/unshare", "--user", "--map-root-user", "--net", "--",
         sys.executable, os.path.abspath(__file__), "child"],
        pass_fds=[descriptor],
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
        timeout=45,
        check=False,
        env={"PATH": "/usr/bin:/bin", "LC_ALL": "C",
             "K1_PARENT_FD": str(descriptor),
             "K1_PARENT_DEV": str(identity.st_dev),
             "K1_PARENT_INO": str(identity.st_ino),
             "K1_SCRATCH_PARENT": str(cache)},
    )
    after = os.fstat(descriptor)
    if ((after.st_dev, after.st_ino) != (identity.st_dev, identity.st_ino)
            or namespace_id("/proc/self/ns/net") != (identity.st_dev, identity.st_ino)):
        raise SystemExit("parent namespace changed")
    if result.returncode != 0 or result.stdout != b"K1_CORE_MARK_PASS\n":
        for stage in ("isolation", "server", "candidate", "control"):
            for reason in ("namespace", "interfaces", "route", "command", "readback", "core", "response", "no-egress", "mark-missing", "wrong-mark", "proxy-bypass", "unavailable"):
                if result.stdout == f"K1_CORE_MARK_FAILED={stage}.{reason}\n".encode("ascii"):
                    raise SystemExit(f"isolated core mark probe failed at {stage}.{reason}")
        raise SystemExit("isolated core mark probe failed")
    print("K1_CORE_MARK_PASS")


if __name__ == "__main__":
    if len(sys.argv) == 2 and sys.argv[1] == "child" and "K1_PARENT_FD" in os.environ:
        raise SystemExit(child())
    parent()
