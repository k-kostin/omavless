"""Developer-only synthetic sockets; no VPN state, credentials, or host commands.

The Rust caller owns namespace creation, guarded fixed ip/nft setup and policy
readback. This helper refuses all socket effects without its inherited pinned
parent namespace descriptor, exact child identity and fixed interface inventory.
"""
import ipaddress
import os
import select
import socket
import struct
import sys
import time

MARK = 0x4F4D4101
TOKEN = b"OMAVLESS_K1_SYNTHETIC_"


def checksum(data):
    if len(data) % 2:
        data += b"\0"
    total = sum(struct.unpack("!" + "H" * (len(data) // 2), data))
    while total >> 16:
        total = (total & 0xFFFF) + (total >> 16)
    return (~total) & 0xFFFF


def vector(name, family=4, protocol=17, src=None, dst=None, sport=41000,
           dport=443, mark=0, hop=255, code=0, kind=133, allowed=False):
    return dict(name=name, family=family, protocol=protocol,
                src=src or ("192.0.2.1" if family == 4 else "2001:db8::1"),
                dst=dst or ("192.0.2.2" if family == 4 else "2001:db8::2"),
                sport=sport, dport=dport, mark=mark, hop=hop, code=code,
                kind=kind, allowed=allowed)


def vectors():
    cases = [vector("udp4"), vector("udp6", 6), vector("tcp4", protocol=6),
             vector("tcp6", 6, protocol=6)]
    for family in [4, 6]:
        for protocol in [6, 17]:
            cases.append(vector(f"dns{family}_{protocol}", family, protocol, dport=53))
        cases.append(vector(f"dot{family}", family, 6, dport=853))
        cases.append(vector(f"marked{family}", family, dport=53, mark=MARK, allowed=True))
        cases.append(vector(f"wrong_mark{family}", family, mark=MARK + 1))
    cases += [vector("lan4", dst="10.0.0.2"), vector("ula6", 6, dst="fd00::2"),
              vector("linklocal6", 6, src="fe80::1", dst="fe80::2")]
    dhcp4 = dict(sport=68, dport=67, dst="255.255.255.255")
    dhcp6 = dict(family=6, sport=546, dport=547, src="fe80::1", dst="ff02::1:2")
    cases += [vector("dhcp4", **dhcp4, allowed=True), vector("dhcp6", **dhcp6, allowed=True)]
    for name, base, changes in [
        ("dhcp4_sport", dhcp4, dict(sport=69)),
        ("dhcp4_dport", dhcp4, dict(dport=53)),
        ("dhcp4_destination", dhcp4, dict(dst="192.0.2.2")),
        ("dhcp6_sport", dhcp6, dict(sport=545)),
        ("dhcp6_dport", dhcp6, dict(dport=53)),
        ("dhcp6_source", dhcp6, dict(src="2001:db8::1")),
        ("dhcp6_destination", dhcp6, dict(dst="fe80::2")),
    ]:
        cases.append(vector(name, **(base | changes)))
    rs = dict(family=6, protocol=58, src="fe80::1", dst="ff02::2", kind=133)
    ns = rs | dict(dst="ff02::1:ff00:2", kind=135)
    na = rs | dict(dst="fe80::2", kind=136)
    for name, base in [("rs", rs), ("ns", ns), ("na", na)]:
        cases.append(vector(name, **base, allowed=True))
        cases.append(vector(name + "_hop", **(base | dict(hop=64))))
        cases.append(vector(name + "_code", **(base | dict(code=1))))
        cases.append(vector(name + "_type", **(base | dict(kind=128))))
        cases.append(vector(name + "_destination", **(base | dict(dst="2001:db8::2"))))
    cases.append(vector("rs_global_source", **(rs | dict(src="2001:db8::1"))))
    cases.append(vector("na_global_source", **(na | dict(src="2001:db8::1"))))
    return cases


def packet(case):
    src = ipaddress.ip_address(case["src"]).packed
    dst = ipaddress.ip_address(case["dst"]).packed
    body = TOKEN + case["name"].encode("ascii")
    protocol = case["protocol"]
    if protocol == 17:
        transport = struct.pack("!HHHH", case["sport"], case["dport"], 8 + len(body), 0) + body
        offset = 6
    elif protocol == 6:
        transport = struct.pack("!HHIIBBHHH", case["sport"], case["dport"], 1, 0, 0x50, 2, 1024, 0, 0) + body
        offset = 16
    else:
        # Fixed synthetic ICMPv6 maintenance-shaped payload; tests header policy,
        # not valid DHCP/ND client state machines or link acquisition semantics.
        transport = struct.pack("!BBHI", case["kind"], case["code"], 0, 0) + body
        offset = 2
    if case["family"] == 4:
        pseudo = src + dst + struct.pack("!BBH", 0, protocol, len(transport))
    else:
        pseudo = src + dst + struct.pack("!I3xB", len(transport), protocol)
    check = checksum(pseudo + transport) or 0xFFFF
    transport = transport[:offset] + struct.pack("!H", check) + transport[offset + 2:]
    if case["family"] == 4:
        header = struct.pack("!BBHHHBBH4s4s", 0x45, 0, 20 + len(transport), 0, 0,
                             case["hop"], protocol, 0, src, dst)
        header = header[:10] + struct.pack("!H", checksum(header)) + header[12:]
    else:
        header = struct.pack("!IHBB16s16s", 6 << 28, len(transport), protocol, case["hop"], src, dst)
    return header + transport


def frame_matches(frame, expected, family):
    if len(frame) < 14:
        return False
    if frame[12:14] != (b"\x08\x00" if family == 4 else b"\x86\xdd"):
        return False
    observed = frame[14:14 + len(expected)]
    if len(observed) != len(expected):
        return False
    if family == 4:
        # IP_HDRINCL legitimately lets the kernel choose ID and checksum.
        normalize = lambda p: p[:4] + b"\0\0" + p[6:10] + b"\0\0" + p[12:]
        return normalize(observed) == normalize(expected)
    return observed == expected


def namespace_facts_valid(parent, inherited, current, names, phase):
    output = "omavless0" if phase in ("interface", "emergency") else "k1out0"
    return (parent == inherited and parent[0] == current[0]
            and parent[1] > 0 and current[1] > 0 and parent != current
            and names == {"lo", output, "k1peer0"})


def guard(phase):
    parent = (int(os.environ["K1_PARENT_DEV"]), int(os.environ["K1_PARENT_INO"]))
    inherited = os.fstat(0)
    if os.readlink("/proc/self/fd/0") != f"net:[{parent[1]}]":
        raise RuntimeError("namespace descriptor required")
    current = os.stat("/proc/self/ns/net")
    with open("/proc/net/dev", encoding="ascii") as stream:
        lines = stream.read(8193)
    if len(lines) > 8192:
        raise RuntimeError("interface inventory oversized")
    names = {line.split(":", 1)[0].strip() for line in lines.splitlines()[2:]}
    if not namespace_facts_valid(parent, (inherited.st_dev, inherited.st_ino),
                                 (current.st_dev, current.st_ino), names, phase):
        raise RuntimeError("namespace isolation refused")
    if current.st_ino != int(os.environ["K1_CHILD_INO"]):
        raise RuntimeError("child identity changed")
    output = "omavless0" if phase in ("interface", "emergency") else "k1out0"
    if (socket.if_nametoindex(output) != int(os.environ["K1_OUT_INDEX"])
            or socket.if_nametoindex("k1peer0") != int(os.environ["K1_PEER_INDEX"])):
        raise RuntimeError("link identity changed")
    return output


def run_case(case, phase):
    output = guard(phase)
    expected = packet(case)
    guard(phase)
    with socket.socket(socket.AF_PACKET, socket.SOCK_RAW, socket.htons(3)) as capture:
        capture.bind(("k1peer0", 0))
        capture.setblocking(False)
        family = socket.AF_INET if case["family"] == 4 else socket.AF_INET6
        guard(phase)
        with socket.socket(family, socket.SOCK_RAW, socket.IPPROTO_RAW) as sender:
            sender.setsockopt(socket.SOL_SOCKET, socket.SO_BINDTODEVICE, output.encode() + b"\0")
            sender.setsockopt(socket.SOL_SOCKET, socket.SO_MARK, case["mark"])
            sender.setsockopt(socket.SOL_SOCKET, socket.SO_BROADCAST, 1)
            destination = ((case["dst"], 0) if family == socket.AF_INET else
                           (case["dst"], 0, 0, socket.if_nametoindex(output)))
            guard(phase)
            try:
                sender.sendto(expected, destination)
            except PermissionError:
                # nft drop may return EPERM; packet observation is still the
                # authority. Baseline for every identical vector must pass.
                pass
            deadline = time.monotonic() + 0.08
            seen = False
            while time.monotonic() < deadline:
                ready, _, _ = select.select([capture], [], [], max(0, deadline - time.monotonic()))
                if not ready:
                    break
                frame = capture.recv(4096)
                if frame_matches(frame, expected, case["family"]):
                    seen = True
                    break
    allowed = phase in ("baseline", "interface") or (phase == "full" and case["allowed"])
    if seen != allowed:
        raise RuntimeError("packet vector mismatch: " + case["name"])


def loopback(phase):
    for address in ["127.0.0.1", "::1"]:
        guard(phase)
        family = socket.AF_INET if address == "127.0.0.1" else socket.AF_INET6
        with socket.socket(family, socket.SOCK_DGRAM) as receiver:
            receiver.bind((address, 0))
            receiver.settimeout(0.2)
            with socket.socket(family, socket.SOCK_DGRAM) as sender:
                guard(phase)
                sender.sendto(TOKEN, receiver.getsockname())
                if receiver.recv(256) != TOKEN:
                    raise RuntimeError("loopback mismatch")


def self_test():
    # No socket creation, namespace entry, privilege or ip/nft command.
    cases = vectors()
    assert len(cases) == 43 and len({c["name"] for c in cases}) == 43
    assert sum(c["allowed"] for c in cases) == 7
    for case in cases:
        encoded = packet(case)
        frame = b"\0" * 12 + (b"\x08\x00" if case["family"] == 4 else b"\x86\xdd") + encoded
        assert frame_matches(frame, encoded, case["family"])
        assert not frame_matches(frame[:-1], encoded, case["family"])
        assert not frame_matches(frame[:-1] + bytes([frame[-1] ^ 1]), encoded, case["family"])
    assert namespace_facts_valid((4, 1), (4, 1), (4, 2), {"lo", "k1out0", "k1peer0"}, "full")
    assert not namespace_facts_valid((4, 1), (4, 1), (4, 1), {"lo", "k1out0", "k1peer0"}, "full")
    assert not namespace_facts_valid((4, 1), (4, 3), (4, 2), {"lo", "k1out0", "k1peer0"}, "full")
    assert not namespace_facts_valid((4, 1), (4, 1), (4, 2), {"lo", "eth0", "k1peer0"}, "full")
    print("K1_PACKET_SELF_TEST_PASS")


if __name__ == "__main__":
    if len(sys.argv) != 2:
        raise SystemExit("fixed fixture mode required")
    mode = sys.argv[1]
    if mode == "self-test":
        self_test()
    elif mode in ("baseline", "full", "interface", "emergency"):
        try:
            guard(mode)
            loopback(mode)
            for index, item in enumerate(vectors(), 1):
                print("K1_PACKET_CASE=" + str(index), flush=True)
                run_case(item, mode)
            print("K1_PACKET_PASS=" + mode)
        except Exception:
            # Never expose packet bytes, paths or tool output on failure.
            raise SystemExit("K1_PACKET_FIXTURE_FAILED") from None
    else:
        raise SystemExit("unknown fixture mode")
