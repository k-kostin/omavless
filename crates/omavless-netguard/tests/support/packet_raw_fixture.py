# SPDX-License-Identifier: MIT
"""New veth-specific guard; never relaxes the loopback-only capability Guard."""
import copy
import json
import os
import runpy
import socket
import struct
import subprocess
import sys
import tempfile
import time

directory = os.path.dirname(__file__)
packet = runpy.run_path(os.path.join(directory, "packet.py"))
full = runpy.run_path(os.path.join(directory, "atomic_full.py"))
core, live = full["core"], full["live"]
require = core["require"]


def ip(args):
    with tempfile.TemporaryFile() as output:
        proc = subprocess.Popen(["/usr/bin/ip"] + args, env={}, stdin=subprocess.DEVNULL,
                                stdout=output, stderr=subprocess.DEVNULL)
        deadline = time.monotonic() + 1
        try:
            while proc.poll() is None:
                require(time.monotonic() < deadline and os.fstat(output.fileno()).st_size <= 32768)
                time.sleep(0.001)
            require(proc.returncode == 0)
            output.seek(0)
            raw = output.read(32769)
            require(len(raw) <= 32768)
        finally:
            if proc.poll() is None:
                proc.kill()
            proc.wait()
    return raw


def topology(links, output, indexes):
    require(type(links) is list and len(links) == 3)
    names = {entry["ifname"]: entry for entry in links}
    require(set(names) == {"lo", output, "k1peer0"})
    require(names["lo"]["link_type"] == "loopback" and names["lo"]["ifindex"] == 1)
    require(all("master" not in e and "link_netnsid" not in e for e in links))
    out, peer = names[output], names["k1peer0"]
    require(indexes[0] > 1 and indexes[1] > 1 and indexes[0] != indexes[1])
    require((out["ifindex"], peer["ifindex"]) == indexes)
    require(out["linkinfo"]["info_kind"] == peer["linkinfo"]["info_kind"] == "veth")
    named = "link_index" not in out and "link_index" not in peer and out.get("link") == "k1peer0" and peer.get("link") == output
    indexed = "link" not in out and "link" not in peer and out.get("link_index") == indexes[1] and peer.get("link_index") == indexes[0]
    require(named or indexed)


def routes(values, output):
    require(type(values) is list and len(values) <= 40)
    destinations = {"127.0.0.0/8", "127.0.0.1", "127.255.255.255", "::1", "192.0.2.0/24",
                    "192.0.2.0", "192.0.2.1", "192.0.2.255", "10.0.0.2", "255.255.255.255",
                    "2001:db8::/64", "2001:db8::1", "fd00::2", "fe80::/64", "fe80::1", "ff00::/8"}
    for route in values:
        require(route.get("dst") in destinations and "gateway" not in route and "multipath" not in route)
        require(route.get("dev") in ("lo", output) or (route.get("dev") == "k1peer0" and route["dst"] == "ff00::/8"))


class VethGuard:
    def __init__(self):
        require(os.environ.get("OMAVLESS_K1_RAW_PACKET_CHILD") == "1")
        self.phase = "baseline"
        self.fd = os.open("/proc/self/ns/net", os.O_RDONLY | os.O_CLOEXEC)
        self.child = core["namespace"](self.fd)
        self.indexes = (int(os.environ["K1_OUT_INDEX"]), int(os.environ["K1_PEER_INDEX"]))
        self.check()

    def check(self):
        output = packet["guard"](self.phase)
        current = os.open("/proc/self/ns/net", os.O_RDONLY | os.O_CLOEXEC)
        try:
            require(core["namespace"](self.fd) == self.child == core["namespace"](current))
        finally:
            os.close(current)
        topology(json.loads(ip(["-j", "-d", "link", "show"])), output, self.indexes)
        for family in ["-4", "-6"]:
            routes(json.loads(ip(["-j", family, "route", "show", "table", "all"])), output)


def packets(guard):
    guard.check()
    print("K1_RAW_PACKET_STAGE=" + guard.phase, flush=True)
    vectors = packet["vectors"]()
    applications = packet["application_cases"]()
    require(len(vectors) == 43 and len(applications) == 10)
    for i, case in enumerate(vectors, 1):
        print("K1_PACKET_CASE=" + str(i), flush=True)
        packet["run_case"](case, guard.phase)
    for i, case in enumerate(applications, 44):
        print("K1_PACKET_CASE=" + str(i), flush=True)
        packet["application_case"](case, guard.phase)
    packet["loopback"](guard.phase)
    guard.check()


def isolated():
    guard = VethGuard()
    wire = core["Netlink"](guard)
    cookie = struct.unpack("=Q", wire.sock.getsockopt(socket.SOL_SOCKET, 71, 8))[0]
    require(cookie != 0 and cookie == core["namespace_id"](guard.fd))
    require(live["table"](wire) is None)
    packets(guard)  # Every vector must reach the peer before policy.
    require(not full["batch"](wire, wire.generation()))
    stable = live["table"](wire)
    require(stable is not None and stable[2] == struct.pack("!I", 6) and stable[7] == struct.pack("!I", wire.port))

    def proof():
        guard.check()
        require(wire.sock.fileno() >= 0 and wire.sock.getsockname() == (wire.port, 0))
        require(struct.unpack("=Q", wire.sock.getsockopt(socket.SOL_SOCKET, 71, 8))[0] == cookie == core["namespace_id"](guard.fd))
        before = wire.generation()
        require(live["table"](wire) == stable)
        full["shape"](live["readback"](guard), ["owner", "persist"])
        require(live["table"](wire) == stable and wire.generation() == before)

    guard.phase = "full"
    proof()
    packets(guard)
    proof()
    ip(["link", "set", "k1out0", "name", "omavless0"])
    guard.phase = "interface"
    proof()
    packets(guard)
    proof()
    wire.sock.close()  # No delete/adopt/replace follows loss of creator.
    guard.check()
    os.close(guard.fd)
    print("K1_RAW_PACKET_PASS", flush=True)


def self_test():
    links = [{"ifname": "lo", "link_type": "loopback", "ifindex": 1},
             {"ifname": "k1out0", "ifindex": 2, "link": "k1peer0", "linkinfo": {"info_kind": "veth"}},
             {"ifname": "k1peer0", "ifindex": 3, "link": "k1out0", "linkinfo": {"info_kind": "veth"}}]
    topology(links, "k1out0", (2, 3))
    for key, value in [("master", "br0"), ("link_netnsid", 1), ("ifindex", 4), ("link", "eth0")]:
        changed = copy.deepcopy(links)
        changed[1][key] = value
        live["refuses"](lambda: topology(changed, "k1out0", (2, 3)))
    live["refuses"](lambda: topology(links + [links[0]], "k1out0", (2, 3)))
    routes([{"dst": "192.0.2.0/24", "dev": "k1out0"}], "k1out0")
    for changed in [{"dst": "default", "dev": "k1out0"}, {"dst": "10.0.0.2", "dev": "eth0"},
                    {"dst": "10.0.0.2", "dev": "k1out0", "gateway": "192.0.2.2"}]:
        live["refuses"](lambda: routes([changed], "k1out0"))
    print("K1_RAW_PACKET_CODEC_PASS")


if __name__ == "__main__":
    try:
        if sys.argv[1:] == ["--self-test"]:
            self_test()
        elif sys.argv[1:] == ["--isolated"]:
            isolated()
        else:
            raise ValueError("unsupported invocation")
    except Exception:
        print("K1_RAW_PACKET_FAILED", flush=True)
        sys.exit(1)
