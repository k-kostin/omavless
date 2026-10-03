#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""Strict opt-in actual broker/core gate with MOCK host services, never installer.

Normal tests import only pure guards. Execution needs a reviewed disposable VM,
rootless subordinate-ID maps and entirely new user/net/mount/PID namespaces.
No normal runtime, package receipt, service or private settings are used.
"""
import argparse
import errno
import hashlib
import http.client
import json
import os
from pathlib import Path
import resource
import secrets
import select
import signal
import socket
import stat
import subprocess
import sys
import threading
import time

C4 = "c4e800425243c1b02165f82153e4bf418fe465e6"
BUILDER = "8c038e76c8407eebd7afdd6e0389fc2bbc28cab9"
MANIFEST = "39fa6ae1e39b47e511e7a794cadff3322df9b445f04902dcc0c0d1bcd013b532"
CORE = "3b1da75d3c9fd8440216f9c256c6c59da812faae88debc936f3c72fef9724544"
BROKER = "6126e5b159eb7996cbf8ac6bdb212be3d7b4b12b1809e09e74c19dbc1394001e"
RELEASE_ENROLLMENT = '{"schema":1,"uid":1000,"policy":"meta-ipv4-release-v1"}\n'
CASES = ("success", "denial")
NS = ("user", "net", "mnt", "pid")
LIMIT = 128 * 1024 * 1024


class Refused(RuntimeError):
    pass


def require(value, reason="fixture_refused"):
    if not value:
        raise Refused(reason)


def pairs(items):
    result = {}
    for key, value in items:
        require(key not in result, "duplicate_json")
        result[key] = value
    return result


def decode(data):
    return json.loads(data, object_pairs_hook=pairs)


def namespace(name):
    return os.readlink("/proc/self/ns/" + name)


def object_bytes(path, expected, maximum=LIMIT):
    require(type(expected) is str and len(expected) == 64 and
            all(c in "0123456789abcdef" for c in expected), "digest_shape")
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC)
    try:
        before = os.fstat(fd)
        require(stat.S_ISREG(before.st_mode) and before.st_uid == os.getuid()
                and before.st_nlink == 1 and before.st_mode & 0o7022 == 0
                and 0 < before.st_size <= maximum, "input_shape")
        require(os.listxattr(fd) == [], "input_xattrs")
        with os.fdopen(os.dup(fd), "rb") as stream:
            data = stream.read(maximum + 1)
        after = os.fstat(fd)
        fields = lambda value: (value.st_dev, value.st_ino, value.st_size,
                                value.st_mtime_ns, value.st_ctime_ns, value.st_mode,
                                value.st_uid, value.st_gid, value.st_nlink)
        require(fields(before) == fields(after) and len(data) == before.st_size
                and hashlib.sha256(data).hexdigest() == expected, "input_changed")
        return data
    finally:
        os.close(fd)


def bundle_inputs(bundle):
    document = decode(object_bytes(bundle / "developer-manifest.json", MANIFEST, 32768))
    require(document["schema"] == "omavless-composed-developer-artifacts-v1"
            and document["builder_source"] == BUILDER and document["dns_source"] == C4
            and document["architecture"] == "x86_64"
            and document["broker_feature"] == "release-package", "manifest_identity")
    for field in ("broker_executed", "installed_compatibility", "package_attestation", "effect_authority"):
        require(document[field] is False, "manifest_authority")
    require(document["sha256"]["mihomo"] == CORE
            and document["sha256"]["omavless-dns-broker"] == BROKER, "pair_identity")
    return {"mihomo": object_bytes(bundle / "mihomo", CORE),
            "omavless-dns-broker": object_bytes(bundle / "omavless-dns-broker", BROKER)}


def private_parent(path):
    require(path.is_absolute() and ".." not in path.parts, "private_path")
    for ancestor in (path, *path.parents):
        value = ancestor.lstat()
        require(stat.S_ISDIR(value.st_mode) and value.st_uid in (0, os.getuid())
                and value.st_mode & 0o022 == 0, "private_ancestry")
    value = path.lstat()
    require(value.st_uid == os.getuid() and stat.S_IMODE(value.st_mode) == 0o700,
            "private_parent")


def live_group(group):
    """Read only own-group IDs/states, never command lines or private argv."""
    members = []
    for path in Path("/proc").iterdir():
        if not path.name.isdecimal():
            continue
        try:
            fields = (path / "stat").read_text().rsplit(")", 1)[1].split()
            if int(fields[2]) == group and fields[0] != "Z":
                members.append(int(path.name))
        except (OSError, ValueError, IndexError):
            pass
    return members


def supervise(child, seconds):
    # Retain the group leader as an unreaped child until cancellation/absence;
    # its PID/group identity cannot be recycled during killpg.
    deadline = time.monotonic() + seconds
    timed_out = False
    try:
        while os.waitid(os.P_PID, child.pid, os.WEXITED | os.WNOHANG | os.WNOWAIT) is None:
            if time.monotonic() >= deadline:
                timed_out = True
                break
            time.sleep(0.02)
    finally:
        try:
            os.killpg(child.pid, signal.SIGKILL)
            until = time.monotonic() + 3
            while live_group(child.pid):
                require(time.monotonic() < until, "owned_group_cleanup")
                time.sleep(0.02)
        finally:
            child.wait(timeout=3)
    return not timed_out


def validate_maps(uid_text, gid_text, groups):
    parse = lambda text: [tuple(map(int, line.split())) for line in text.splitlines()]
    expected = [(0, 1000, 1), (1000, 100000, 1)]
    require(parse(uid_text) == expected and parse(gid_text) == expected
            and groups.strip() == "allow", "namespace_id_maps")


def command(args, **kwargs):
    return subprocess.run(args, stdin=subprocess.DEVNULL, check=True, timeout=5,
                          stdout=subprocess.PIPE, stderr=subprocess.PIPE, **kwargs)


def links():
    return decode(command(["/usr/bin/ip", "-j", "link"]).stdout)


def no_usr_submounts(text):
    require(not any(line.split()[4].startswith("/usr/") for line in text.splitlines()),
            "usr_child_mount")


def no_directory_fds():
    for name in os.listdir("/proc/self/fd"):
        try:
            value = os.fstat(int(name))
        except OSError as error:
            # The transient directory FD used by listdir is already closed.
            require(error.errno == errno.EBADF, "fd_inventory_unknown")
            continue
        require(not stat.S_ISDIR(value.st_mode), "outside_directory_fd")


def isolate(original, root, inputs):
    # ALL these checks precede mount, device/config creation or ELF execution.
    require(set(original) == set(NS) and all(namespace(name) != original[name] for name in NS),
            "namespace_boundary")
    require(os.geteuid() == 0 and os.getegid() == 0, "namespace_root")
    validate_maps(Path("/proc/self/uid_map").read_text(),
                  Path("/proc/self/gid_map").read_text(),
                  Path("/proc/self/setgroups").read_text())
    require({item["ifname"] for item in links()} == {"lo"}, "namespace_links")
    no_usr_submounts(Path("/proc/self/mountinfo").read_text())
    require(root.is_absolute() and root.is_dir() and not root.is_symlink()
            and list(root.iterdir()) == [], "empty_root")
    command(["/usr/bin/mount", "--make-rprivate", "/"])
    command(["/usr/bin/mount", "-t", "tmpfs", "-o", "size=32m,mode=0755,nosuid,nodev", "tmpfs", str(root)])
    for name in ("usr", "proc", "dev", "dev/net", "etc", "etc/omavless-dns",
                 "run", "run/dbus", "run/systemd", "run/omavless-dns",
                 "run/omavless-dns/private", "tmp", "home", "artifacts"):
        (root / name).mkdir(exist_ok=True, mode=0o755)
    for source, target in (("/usr", "usr"), (str(inputs), "artifacts")):
        command(["/usr/bin/mount", "--bind", source, str(root / target)])
        command(["/usr/bin/mount", "-o", "remount,bind,ro,nosuid,nodev", str(root / target)])
    # No parent /home, /etc, /run, sysfs or whole /dev exposure.
    for source in ("/dev/null", "/dev/urandom", "/dev/net/tun"):
        target = root / source.lstrip("/")
        target.touch(mode=0o600)
        command(["/usr/bin/mount", "--bind", source, str(target)])
        command(["/usr/bin/mount", "-o", "remount,bind,nosuid,noexec", str(target)])
    (root / "bin").symlink_to("usr/bin")
    (root / "lib").symlink_to("usr/lib")
    (root / "lib64").symlink_to("usr/lib")
    command(["/usr/bin/mount", "-t", "proc", "-o", "nosuid,nodev,noexec", "proc", str(root / "proc")])
    os.chroot(root)
    os.chdir("/")
    require(os.stat("/").st_uid == 0 and not list(Path("/home").iterdir()), "private_root")
    no_directory_fds()
    Path("/run/omavless-dns").chmod(0o711)
    Path("/run/omavless-dns/private").chmod(0o700)
    Path("/tmp").chmod(0o700)
    Path("/etc/omavless-dns/release-enrollment.json").write_text(RELEASE_ENROLLMENT)
    Path("/etc/omavless-dns/release-enrollment.json").chmod(0o600)
    Path("/etc/passwd").write_text("root:x:0:0:fixture:/tmp:/usr/bin/false\ncore:x:1000:1000:fixture:/home/core:/usr/bin/false\n")
    Path("/etc/group").write_text("root:x:0:\ncore:x:1000:\n")
    Path("/etc/nsswitch.conf").write_text("passwd: files\ngroup: files\nhosts: files\n")
    Path("/etc/machine-id").write_text("0123456789abcdef0123456789abcdef\n")
    command(["/usr/bin/ip", "link", "set", "dev", "lo", "up"])


ENV = {"PATH": "/usr/bin", "HOME": "/tmp", "LANG": "C", "TMPDIR": "/tmp", "GOMAXPROCS": "2"}
CAP = 1 << 12


def cap_exec(binary, uid, args):
    # Fixed reviewed code, not caller-selected shell/privileged IPC.
    require(binary in ("mihomo", "omavless-dns-broker", "host-fixture") and uid in (0, 1000))
    env = dict(ENV, **({"HOME": "/home/core", "TMPDIR": "/home/core"} if uid == 1000 else {}))
    check = ("import os; from pathlib import Path; "
             "s=dict(x.split(':',1) for x in Path('/proc/self/status').read_text().splitlines() if ':' in x); "
             f"assert os.getuid()=={uid} and os.getgid()=={uid}; "
             f"assert all(int(s[k].strip(),16)=={CAP} for k in ('CapEff','CapPrm','CapInh','CapBnd','CapAmb')); "
             "assert s['NoNewPrivs'].strip()=='1'; "
             f"os.execve('/artifacts/{binary}', {['/artifacts/' + binary, *args]!r}, {env!r})")
    return ["/usr/bin/setpriv", "--reuid=" + str(uid), "--regid=" + str(uid),
            "--clear-groups", "--inh-caps=-all,+net_admin", "--ambient-caps=-all,+net_admin",
            "--bounding-set=-all,+net_admin", "--no-new-privs", "/usr/bin/python3", "-c", check]


def bounded_line(child, seconds=4):
    deadline = time.monotonic() + seconds
    data = getattr(child, "receipt_buffer", b"")
    while b"\n" not in data:
        require(child.poll() is None, "helper_exited")
        left = deadline - time.monotonic()
        require(left > 0, "helper_reply_timeout")
        ready, _, _ = select.select([child.stdout], [], [], left)
        require(ready, "helper_reply_timeout")
        chunk = os.read(child.stdout.fileno(), 16385 - len(data))
        require(chunk, "helper_reply_eof")
        data += chunk
        require(len(data) <= 16384, "helper_reply_shape")
    line, remainder = data.split(b"\n", 1)
    child.receipt_buffer = remainder
    return line + b"\n"


def snapshot(host):
    host.stdin.write(b"snapshot\n")
    host.stdin.flush()
    value = decode(bounded_line(host))
    require(value["notify_alive"] is True, "notification_failed")
    return value


def wait(predicate, children, seconds=8):
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        require(all(child.poll() is None for child in children), "owned_child_exited")
        try:
            value = predicate()
            if value:
                return value
        except (OSError, http.client.HTTPException):
            pass
        time.sleep(0.025)
    raise Refused("phase_timeout")


def controller(method, path, token=None):
    connection = http.client.HTTPConnection("127.0.0.1", 19091, timeout=1)
    try:
        connection.request(method, path, headers={"Authorization": "Bearer synthetic-binary-gate",
            **({} if token is None else {"If-Match": '"' + token + '"'})})
        response = connection.getresponse()
        body = response.read(65537)
        require(len(body) <= 65536, "controller_bound")
        return response.status, body
    finally:
        connection.close()


def core_ready():
    status, body = controller("GET", "/configs")
    require(status == 200, "config_status")
    value = decode(body).get("tun", {})
    return value.get("enable") is True and value.get("omavless-dns-ready") is True


def capabilities(value):
    require(type(value) is dict and set(value) == {"abi", "ready"}
            and type(value["abi"]) is int and value["abi"] == 1
            and value["ready"] is True, "conditional_abi")


def denied_readiness(core, owners, seconds=1):
    """Bounded observation AFTER a real settled DNS denial, not a startup timer PASS."""
    deadline = time.monotonic() + seconds
    observed = False
    while time.monotonic() < deadline:
        require(all(child.poll() is None for child in owners), "denial_owner_exited")
        code = core.poll()
        if code is not None:
            require(code >= 0, "denial_core_signalled")
            return "known_core_exit"
        try:
            status, body = controller("GET", "/configs")
            if status == 200:
                tun = decode(body).get("tun")
                if type(tun) is dict:
                    ready = tun.get("omavless-dns-ready")
                    require(ready is not True, "denied_core_ready")
                    observed |= ready is False
        except (OSError, http.client.HTTPException):
            pass
        time.sleep(0.025)
    require(observed, "denial_readiness_unknown")
    return "observed_nonready_bounded"


def tunnel():
    stream = socket.create_connection(("127.0.0.1", 19090), timeout=2)
    stream.sendall(b"CONNECT 127.0.0.1:19092 HTTP/1.1\r\nHost: 127.0.0.1:19092\r\n\r\n")
    header = b""
    while not header.endswith(b"\r\n\r\n") and len(header) <= 4096:
        chunk = stream.recv(1)
        require(chunk, "tunnel_eof")
        header += chunk
    require(header.startswith(b"HTTP/1.1 200 ") and len(header) <= 4096, "tunnel_status")
    return stream


def echo(stream):
    payload = b"actual-composed-binary-gate:" + secrets.token_bytes(32)
    stream.sendall(payload)
    result = b""
    while len(result) < len(payload):
        chunk = stream.recv(len(payload) - len(result))
        require(chunk, "echo_eof")
        result += chunk
    require(result == payload, "echo_bytes")


def echo_server(listener, stopped):
    peers = []
    listener.settimeout(0.05)
    try:
        while not stopped.is_set():
            try:
                peer, _ = listener.accept()
                peer.settimeout(0.05)
                peers.append(peer)
            except socket.timeout:
                pass
            for peer in peers[:]:
                try:
                    data = peer.recv(4096)
                    if data:
                        peer.sendall(data)
                    else:
                        peers.remove(peer)
                        peer.close()
                except socket.timeout:
                    pass
                except OSError:
                    peers.remove(peer)
                    peer.close()
    finally:
        for peer in peers:
            peer.close()


def active(value):
    require(value["stored"] is True and value["received_tun"] is True
            and value["phase"] == "active" and value["tun_exists"] is True
            and value["calls"] == ["dns", "domains", "route"]
            and value["servers"] == [[2, [198, 18, 0, 2]]]
            and value["domains"] == [[".", True]] and value["route"] is True,
            "active_receipt")


def clean(value, case):
    require(case in CASES, "unknown_case")
    require(value["stored"] is False and value["received_tun"] is True
            and value["phase"] is None and value["tun_exists"] is False
            and value["servers"] == [] and value["domains"] == [] and value["route"] is False
            and value["calls"] == (["dns", "domains", "route", "revert"] if case == "success"
                                   else ["dns", "revert"]), "clean_receipt")
    require(value["notifications"] == ["READY=1", "FDSTORE=1\nFDNAME=omavless-tun-lease\nFDPOLL=0",
        "BARRIER=1", "FDSTOREREMOVE=1\nFDNAME=omavless-tun-lease", "BARRIER=1"],
        "notification_sequence")


def stop(child, seconds=6):
    if child.poll() is None:
        child.terminate()
        try:
            child.wait(timeout=seconds)
        except subprocess.TimeoutExpired:
            child.kill()
            child.wait(timeout=2)
            raise Refused("owned_cleanup_forced")


def exercise(case):
    require(case in CASES)
    children = []
    logs = []
    result = {"case": case, "outcome": "NONPASS", "stages": []}
    host = core = broker = None
    try:
        # Fixed system-bus address; root owns both well-known fixture names.
        config = Path("/tmp/dbus.xml")
        config.write_text('<busconfig><type>system</type><listen>unix:path=/run/dbus/system_bus_socket</listen>'
            '<auth>EXTERNAL</auth><policy user="root"><allow own="*"/><allow send_destination="*"/>'
            '<allow receive_sender="*"/></policy></busconfig>')
        dbuslog = open("/tmp/dbus.log", "xb")
        logs.append(dbuslog)
        bus = subprocess.Popen(["/usr/bin/dbus-daemon", "--nofork", "--nopidfile", "--config-file=/tmp/dbus.xml"],
                               env=ENV, stdin=subprocess.DEVNULL, stdout=dbuslog, stderr=dbuslog)
        children.append(bus)
        wait(lambda: Path("/run/dbus/system_bus_socket").is_socket(), children)
        # Child waits before exec; its real PID is known BEFORE mock unit admission.
        brokerlog = open("/tmp/broker.log", "xb")
        logs.append(brokerlog)
        broker_command = cap_exec("omavless-dns-broker", 0, ["--serve"])
        gate = "import os; assert os.read(0,1)==b'G'; os.execve(" + repr(broker_command[0]) + "," + repr(broker_command) + "," + repr(ENV) + ")"
        broker = subprocess.Popen(["/usr/bin/python3", "-c", gate], env=ENV,
            stdin=subprocess.PIPE, stdout=brokerlog, stderr=brokerlog)
        children.append(broker)
        hostlog = open("/tmp/host.log", "xb")
        logs.append(hostlog)
        host = subprocess.Popen(cap_exec("host-fixture", 0, [case, str(broker.pid)]),
            env=ENV, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=hostlog, bufsize=0)
        children.append(host)
        require(bounded_line(host) == b"fixture-ready\n", "mock_host_start")
        broker.stdin.write(b"G")
        broker.stdin.close()
        wait(lambda: "READY=1" in snapshot(host)["notifications"], children)
        result["stages"].append("actual_broker_ready")
        core_dir = Path("/home/core")
        core_dir.mkdir(mode=0o700)
        os.chown(core_dir, 1000, 1000)
        core_config = core_dir / "config.yaml"
        core_config.write_text("mixed-port: 19090\nexternal-controller: 127.0.0.1:19091\n"
            "secret: synthetic-binary-gate\nallow-lan: false\nbind-address: 127.0.0.1\n"
            "mode: direct\nlog-level: silent\nipv6: false\n"
            "tun:\n  enable: true\n  device: Meta\n  stack: system\n  mtu: 1500\n  gso: false\n"
            "  auto-route: false\n  auto-detect-interface: false\n  disable-system-dns: true\n"
            "  omavless-dns-broker: true\n  dns-hijack: []\n"
            "dns:\n  enable: true\n  enhanced-mode: fake-ip\n  fake-ip-range: 198.18.0.1/16\n"
            "  ipv6: false\n  nameserver: [127.0.0.1:19093]\n"
            "proxies: []\nproxy-groups: []\nrules:\n  - MATCH,DIRECT\n")
        core_config.chmod(0o600)
        os.chown(core_config, 1000, 1000)
        corelog = open("/tmp/core.log", "xb")
        logs.append(corelog)
        core = subprocess.Popen(cap_exec("mihomo", 1000, ["-d", str(core_dir), "-f", str(core_config)]),
            env=ENV, stdin=subprocess.DEVNULL, stdout=corelog, stderr=corelog)
        children.append(core)
        if case == "denial":
            # Require actual authenticated transport/effect attempt, not timeout/startup failure.
            wait(lambda: snapshot(host)["calls"] == ["dns", "revert"], [bus, broker, host])
            result["stages"].append("actual_dns_denied")
            result["denial_readiness"] = denied_readiness(core, [bus, broker, host])
            stop(core)
            wait(lambda: not snapshot(host)["tun_exists"], [bus, broker, host])
            result["close_dispatched"] = False
        else:
            wait(core_ready, children)
            status, body = controller("GET", "/connections/conditional-capabilities")
            require(status == 200, "conditional_status")
            capabilities(decode(body))
            active(snapshot(host))
            result["stages"].append("actual_dns_ready")
            with socket.socket() as listener:
                listener.bind(("127.0.0.1", 19092))
                listener.listen(4)
                stopped = threading.Event()
                thread = threading.Thread(target=echo_server, args=(listener, stopped))
                thread.start()
                streams = []
                try:
                    streams = [tunnel(), tunnel()]
                    for stream in streams:
                        echo(stream)
                    status, body = controller("GET", "/connections")
                    rows = decode(body)["connections"]
                    require(status == 200 and len(rows) == 2, "actual_tracker_count")
                    by_port = {int(row["metadata"]["sourcePort"]): row for row in rows}
                    first, second = [by_port[stream.getsockname()[1]] for stream in streams]
                    first_token, second_token = first["omavlessCloseToken"], second["omavlessCloseToken"]
                    require(all(type(token) is str and token.isdecimal() and token != "0"
                                for token in (first_token, second_token)) and first_token != second_token,
                            "actual_tracker_tokens")
                    path = "/connections/" + first["id"] + "/close-conditional"
                    require(controller("POST", path, second_token)[0] == 409, "wrong_token")
                    for stream in streams:
                        echo(stream)
                    require(controller("POST", path, first_token)[0] == 204, "exact_close")
                    try:
                        require(streams[0].recv(1) == b"", "selected_stream_live")
                    except ConnectionResetError:
                        pass
                    echo(streams[1])
                    require(controller("POST", path, first_token)[0] == 404, "close_replay")
                    status, body = controller("GET", "/connections")
                    rows = decode(body)["connections"]
                    require(status == 200 and len(rows) == 1 and rows[0]["id"] == second["id"],
                            "unselected_tracker")
                    active(snapshot(host))
                    require(core_ready(), "connection_close_lost_dns")
                    result["stages"].append("conditional_close_kept_dns_lease")
                    result["close_dispatched"] = True
                finally:
                    stopped.set()
                    for stream in streams:
                        stream.close()
                    thread.join(timeout=2)
                    require(not thread.is_alive(), "echo_cleanup")
            stop(core)
            require(core.returncode == 0, "core_clean_exit")
            result["stages"].append("owned_core_shutdown")
            wait(lambda: not snapshot(host)["tun_exists"], [bus, broker, host])
        final = snapshot(host)
        clean(final, case)
        result["receipt"] = final
        result["stages"].append("actual_dns_released")
        result["outcome"] = "MEASURED"
    except Exception as error:
        result["reason"] = str(error) if isinstance(error, Refused) else type(error).__name__
        if host is not None and host.poll() is None:
            try:
                result["partial_receipt"] = snapshot(host)
            except Exception:
                pass
    finally:
        # Teardown is only of our private namespace, never recovery on the host.
        for child in reversed(children):
            try:
                if child is host and child.poll() is None:
                    child.stdin.close()
                    child.wait(timeout=2)
                else:
                    stop(child)
            except Exception as error:
                result["outcome"] = "NONPASS"
                result["cleanup_reason"] = type(error).__name__
        for log in logs:
            log.close()
        result["diagnostics"] = {name: Path("/tmp/" + name + ".log").read_bytes()[-8192:].decode("utf-8", "replace")
                                 for name in ("dbus", "broker", "host", "core")
                                 if Path("/tmp/" + name + ".log").exists()}
    return result


def limits():
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
    resource.setrlimit(resource.RLIMIT_FSIZE, (4 * 1024 * 1024,) * 2)
    resource.setrlimit(resource.RLIMIT_NOFILE, (128, 128))


def main():
    os.umask(0o077)
    if len(sys.argv) == 7 and sys.argv[1] == "--isolated-child":
        original, root, inputs, case, parent_uid = sys.argv[2:]
        require(parent_uid == "1000", "parent_uid")
        isolate(decode(original), Path(root), Path(inputs))
        print(json.dumps(exercise(case), sort_keys=True))
        return
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--run", action="store_true")
    parser.add_argument("--ack-disposable-vm", action="store_true")
    parser.add_argument("--bundle", type=Path)
    parser.add_argument("--fixture", type=Path)
    parser.add_argument("--fixture-sha")
    parser.add_argument("--scratch", type=Path)
    args = parser.parse_args()
    require(args.run and args.ack_disposable_vm and os.geteuid() == 1000, "execution_opt_in")
    require(all(value is not None for value in (args.bundle, args.fixture, args.fixture_sha, args.scratch)))
    private_parent(args.scratch)
    private_parent(args.bundle)
    private_parent(args.fixture.parent)
    private_parent(Path(__file__).absolute().parent)
    values = bundle_inputs(args.bundle)
    values["host-fixture"] = object_bytes(args.fixture, args.fixture_sha)
    # Existing output refuses; no automatic old staging takeover/cleanup.
    root = args.scratch / "binary-gate"
    root.mkdir(mode=0o700)
    inputs = root / "inputs"
    # Traversable by inner UID1000, still enclosed by the outer 0700 stage.
    inputs.mkdir(mode=0o755)
    inputs.chmod(0o755)
    for name, data in values.items():
        with open(inputs / name, "xb") as stream:
            stream.write(data)
        (inputs / name).chmod(0o755)
    original = {name: namespace(name) for name in NS}
    results = []
    for case in CASES:
        mount_root = root / (case + "-root")
        mount_root.mkdir(mode=0o700)
        argv = ["/usr/bin/unshare", "--user", "--map-root-user",
            "--map-users=1000:100000:1", "--map-groups=1000:100000:1", "--net", "--mount", "--pid",
            "--fork", "--kill-child=SIGKILL", "/usr/bin/python3", str(Path(__file__).resolve()),
            "--isolated-child", json.dumps(original), str(mount_root), str(inputs), case, "1000"]
        # Always retain private diagnostics/results, including timeout/refusal.
        with open(root / (case + ".stdout"), "xb") as output, open(root / (case + ".stderr"), "xb") as error:
            child = subprocess.Popen(argv, env={"PATH": "/usr/bin", "HOME": os.environ["HOME"],
                "LANG": "C", "TMPDIR": str(args.scratch)}, stdin=subprocess.DEVNULL,
                stdout=output, stderr=error, start_new_session=True, preexec_fn=limits)
            completed = supervise(child, 35)
            require(namespace("net") == original["net"], "parent_namespace_changed")
        raw = (root / (case + ".stdout")).read_bytes()
        if completed and child.returncode == 0 and len(raw) <= 65536:
            result = decode(raw)
            require(result["case"] == case and result["outcome"] in ("MEASURED", "NONPASS"), "case_receipt")
        else:
            result = {"case": case, "outcome": "NONPASS", "reason": "namespace_child_refused"}
        results.append(result)
        if not completed or child.returncode != 0 or "cleanup_reason" in result:
            # A containment/authority/cleanup refusal stops later execution.
            for unrun in CASES[len(results):]:
                results.append({"case": unrun, "outcome": "NONPASS", "reason": "not_run_after_boundary_refusal"})
            break
    receipt = {"schema": "composed-binary-mock-host-v1", "manifest_sha256": MANIFEST,
        "core_sha256": CORE, "broker_sha256": BROKER, "fixture_sha256": args.fixture_sha,
        "results": results, "normal_activation": False, "installed_attestation": False}
    with open(root / "results.json", "x") as stream:
        json.dump(receipt, stream, sort_keys=True, indent=2)
    print("actual_binary_mock_host: " + str(sum(row["outcome"] == "MEASURED" for row in results))
          + "/2 measured; installed/normal acceptance NOT claimed")
    require(all(row["outcome"] == "MEASURED" for row in results), "matrix_nonpass")


if __name__ == "__main__":
    main()
