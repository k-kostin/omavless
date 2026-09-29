"""Fixed developer-only Linux experiments; no production imports or receipt authority.

Only --self-test is safe for ordinary host tests. --isolated requires a pinned
parent network namespace on stdin and a different loopback-only namespace.
No expression, command, path, interface or address is accepted from a caller.
"""
import errno
import fcntl
import os
import socket
import struct
import sys
import time

LIMIT = 32768
TARGET = b"omavless_k1_capability\0"
SENTINEL = b"omavless_k1_sentinel\0"
TAG = b"fixed-k1-capability-fixture"
NS_GET_NSTYPE = 0xB703
# Linux generic ioctl ABI on the supported x86_64/ARM64 test architectures.
NS_GET_ID = 0x8008B70D
CLONE_NEWNET = 0x40000000
SO_NETNS_COOKIE = 71
NFT = 10 << 8
REQUEST, ACK, EXCL, CREATE = 1, 4, 0x200, 0x400


def require(condition):
    if not condition:
        raise ValueError("fixed fixture condition failed")


def align(length):
    return (length + 3) & ~3


def attr(kind, value):
    size = 4 + len(value)
    require(0 < kind < 0x4000 and size <= 65535)
    return struct.pack("=HH", size, kind) + value + bytes(align(size) - size)


def attrs(data):
    require(len(data) <= LIMIT)
    result = {}
    while data:
        require(len(data) >= 4)
        length, kind = struct.unpack_from("=HH", data)
        # Allow the documented network-byte-order flag, never nested input.
        require(not kind & 0x8000)
        kind &= 0x3FFF
        require(length >= 4 and align(length) <= len(data) and kind not in result)
        require(not any(data[length:align(length)]))
        result[kind] = data[4:length]
        data = data[align(length):]
    return result


def message(kind, flags, seq, payload):
    size = 16 + len(payload)
    require(0 < seq <= 0xFFFFFFFF and size <= LIMIT)
    return struct.pack("=IHHII", size, kind, flags, seq, 0) + payload + bytes(align(size) - size)


def messages(data):
    require(0 < len(data) <= LIMIT)
    result = []
    while data:
        require(len(data) >= 16)
        length, kind, flags, seq, pid = struct.unpack_from("=IHHII", data)
        require(16 <= length and align(length) <= len(data))
        require(not any(data[length:align(length)]))
        # DUMP_INTR is never a complete observation.
        require(not flags & 0x10)
        result.append((kind, flags, seq, pid, data[16:length]))
        data = data[align(length):]
    return result


def nf(family=1, resource=0):
    return struct.pack("!BBH", family, 0, resource)


def namespace(fd):
    require(fcntl.ioctl(fd, NS_GET_NSTYPE) == CLONE_NEWNET)
    st = os.fstat(fd)
    require(os.readlink(f"/proc/self/fd/{fd}") == f"net:[{st.st_ino}]")
    return (st.st_dev, st.st_ino)


def namespace_id(fd):
    value = bytearray(8)
    try:
        fcntl.ioctl(fd, NS_GET_ID, value, True)
    except OSError as error:
        if error.errno in (errno.ENOTTY, errno.EINVAL):
            return None
        raise
    result = struct.unpack("=Q", value)[0]
    require(result != 0)
    return result


class Guard:
    def __init__(self):
        require(os.environ.get("OMAVLESS_K1_CAPABILITY_CHILD") == "1")
        self.parent = namespace(0)
        require(self.parent == (int(os.environ["OMAVLESS_K1_NFT_PARENT_DEV"]),
                                int(os.environ["OMAVLESS_K1_NFT_PARENT_INO"])))
        self.fd = os.open("/proc/self/ns/net", os.O_RDONLY | os.O_CLOEXEC)
        self.child = namespace(self.fd)
        require(self.child != self.parent and self.child[0] == self.parent[0])
        self.check()

    def check(self):
        require(namespace(0) == self.parent and namespace(self.fd) == self.child)
        fd = os.open("/proc/self/ns/net", os.O_RDONLY | os.O_CLOEXEC)
        try:
            require(namespace(fd) == self.child)
        finally:
            os.close(fd)
        with open("/proc/net/dev", "rb") as source:
            data = source.read(LIMIT + 1)
        require(len(data) <= LIMIT)
        lines = data.splitlines()
        require(len(lines) == 3 and lines[0].startswith(b"Inter-|")
                and lines[1].lstrip().startswith(b"face |"))
        name, counters = lines[2].split(b":", 1)
        require(name.strip() == b"lo" and len(counters.split()) == 16
                and all(value.isdigit() for value in counters.split()))


class Netlink:
    def __init__(self, guard):
        guard.check()
        self.guard = guard
        self.sock = socket.socket(socket.AF_NETLINK, socket.SOCK_RAW, 12)
        self.sock.bind((0, 0))
        self.port = self.sock.getsockname()[0]
        self.seq = 0

    def next_seq(self):
        self.seq += 1
        require(self.seq < 1000)
        return self.seq

    def exchange(self, requests, expected, barrier_request=None):
        self.guard.check()
        require(len(requests) <= LIMIT)
        self.sock.sendto(requests, (0, 0))
        if barrier_request is not None:
            self.sock.sendto(barrier_request, (0, 0))
        replies, errors, acknowledged = {}, {}, set()
        deadline = time.monotonic() + 1.0
        # Every exchange ends in a GETGEN/GETTABLE carrying ACK. Seeing its
        # response and ACK bounds collection without trusting silence as success.
        terminal = max(expected)
        total = 0
        while terminal not in acknowledged or terminal not in replies:
            self.sock.settimeout(max(0.001, deadline - time.monotonic()))
            require(time.monotonic() < deadline)
            data, _, flags, sender = self.sock.recvmsg(LIMIT)
            total += len(data)
            require(sender == (0, 0) and not flags & socket.MSG_TRUNC and total <= LIMIT)
            for kind, _, seq, pid, body in messages(data):
                require(seq in expected and pid in (0, self.port))
                if kind == 2:  # NLMSG_ERROR, including successful ACK.
                    require(len(body) >= 20)
                    code = struct.unpack_from("=i", body)[0]
                    original = struct.unpack_from("=IHHII", body, 4)
                    require(original[1] == expected[seq] and original[3] == seq
                            and seq not in acknowledged and seq not in errors and code <= 0)
                    if code:
                        errors[seq] = -code
                        if seq == terminal:
                            return replies, errors, acknowledged
                    else:
                        acknowledged.add(seq)
                else:
                    require(kind in (NFT, NFT + 15) and seq not in replies
                            and len(body) >= 4 and body[1] == 0)
                    replies[seq] = (kind, attrs(body[4:]))
        return replies, errors, acknowledged

    def get(self, kind, payload):
        seq = self.next_seq()
        replies, errors, acknowledged = self.exchange(
            message(kind, REQUEST | ACK, seq, payload), {seq: kind})
        if seq in errors:
            require(errors[seq] == errno.ENOENT and kind == NFT + 1)
            return None
        require(seq in acknowledged and len(replies) == 1)
        return replies[seq]

    def generation(self):
        kind, result = self.get(NFT + 16, nf(0))
        require(kind == NFT + 15 and set(result).issubset({1, 2, 3}) and len(result[1]) == 4)
        value = struct.unpack("!I", result[1])[0]
        require(value != 0)
        return value

    def table(self, name):
        require(name in (TARGET, SENTINEL))
        result = self.get(NFT + 1, nf() + attr(1, name))
        if result is None:
            return None
        kind, values = result
        require(kind == NFT and set(values).issubset({1, 2, 3, 4, 5, 6, 7})
                and values[1] == name and len(values[4]) == 8
                and values[2] == bytes(4) and values[3] == bytes(4)
                and values[6] == TAG and 7 not in values)
        handle = struct.unpack("!Q", values[4])[0]
        require(handle != 0)
        return (handle, values)

    def batch(self, generation, operations):
        require(generation > 0 and 1 <= len(operations) <= 3)
        begin = self.next_seq()
        request = message(16, REQUEST, begin, nf(0, 10) + attr(1, struct.pack("!I", generation)))
        expected = {begin: 16}
        operation_ids = []
        for kind, flags, payload in operations:
            seq = self.next_seq()
            expected[seq] = kind
            operation_ids.append(seq)
            request += message(kind, REQUEST | ACK | flags, seq, nf() + payload)
        end = self.next_seq()
        expected[end] = 17
        request += message(17, REQUEST, end, nf(0, 10))
        # A following read acts as a bounded barrier, including failed batches.
        barrier = self.next_seq()
        expected[barrier] = NFT + 16
        barrier_request = message(NFT + 16, REQUEST | ACK, barrier, nf(0))
        replies, errors, acknowledged = self.exchange(request, expected, barrier_request)
        require(barrier in acknowledged and replies[barrier][0] == NFT + 15)
        if not errors:
            require(all(seq in acknowledged for seq in operation_ids))
        return set(errors.values())


def create(name):
    require(name in (TARGET, SENTINEL))
    return NFT, CREATE | EXCL, attr(1, name) + attr(6, TAG)


def delete(handle):
    require(0 < handle <= 0xFFFFFFFFFFFFFFFF)
    # Exact handle, never a name-only fallback or broad flush.
    return NFT + 2, 0, attr(4, struct.pack("!Q", handle))


def stage(name):
    print("K1_CAPABILITY_STAGE=" + name, flush=True)


def isolated_test():
    stage("namespace")
    guard = Guard()
    original_id, child_id = namespace_id(0), namespace_id(guard.fd)
    require((original_id is None) == (child_id is None))
    if child_id is not None:
        require(original_id != child_id)
    print("K1_NS_GET_ID=" + ("available" if child_id is not None else "unavailable"), flush=True)
    stage("socket")
    with socket.socket(socket.AF_INET, socket.SOCK_DGRAM) as probe:
        cookie = struct.unpack("=Q", probe.getsockopt(socket.SOL_SOCKET, SO_NETNS_COOKIE, 8))[0]
        require(cookie != 0)
        if child_id is not None:
            require(cookie == child_id)
    print("K1_SO_NETNS_COOKIE=available", flush=True)
    wire = Netlink(guard)
    require(wire.table(TARGET) is None and wire.table(SENTINEL) is None)
    stage("sentinel")
    require(not wire.batch(wire.generation(), [create(SENTINEL)]))
    sentinel = wire.table(SENTINEL)
    require(sentinel is not None)
    require(not wire.batch(wire.generation(), [create(TARGET)]))
    first = wire.table(TARGET)
    require(first is not None)
    stage("exclusive")
    before = wire.generation()
    require(wire.batch(before, [create(TARGET)]) == {errno.EEXIST})
    require(wire.generation() == before and wire.table(TARGET) == first)
    stage("generation")
    stale = before
    require(not wire.batch(before, [delete(first[0]), create(TARGET)]))
    replacement = wire.table(TARGET)
    require(replacement is not None and replacement[0] != first[0])
    before = wire.generation()
    require(before != stale)
    # Linux nfnetlink rejects a mismatched batch generation with ERESTART.
    require(wire.batch(stale, [delete(replacement[0])]) == {errno.ERESTART})
    require(wire.generation() == before and wire.table(TARGET) == replacement)
    stage("stale_handle")
    require(wire.batch(before, [delete(first[0])]) == {errno.ENOENT})
    require(wire.generation() == before and wire.table(TARGET) == replacement)
    stage("rollback")
    # Later exclusive collision aborts the entire delete/recreate batch.
    require(wire.batch(before, [delete(replacement[0]), create(TARGET), create(SENTINEL)]) == {errno.EEXIST})
    require(wire.generation() == before and wire.table(TARGET) == replacement
            and wire.table(SENTINEL) == sentinel)
    stage("cleanup")
    require(not wire.batch(before, [delete(replacement[0])]))
    require(wire.table(TARGET) is None and wire.table(SENTINEL) == sentinel)
    require(not wire.batch(wire.generation(), [delete(sentinel[0])]))
    require(wire.table(TARGET) is None and wire.table(SENTINEL) is None)
    guard.check()
    require(namespace_id(guard.fd) == child_id and namespace_id(0) == original_id)
    wire.sock.close()
    os.close(guard.fd)
    stage("finished")
    print("K1_CAPABILITY_PASS", flush=True)


def self_test():
    # Pure codec checks: never create a socket/namespace or execute nft.
    require(attrs(attr(1, b"abc") + attr(4, struct.pack("!Q", 17))) == {1: b"abc", 4: struct.pack("!Q", 17)})
    encoded = message(NFT, REQUEST | ACK, 42, nf() + attr(1, TARGET))
    require(messages(encoded)[0][0:4] == (NFT, REQUEST | ACK, 42, 0))
    for malformed in [b"x", b"\x03\0\x01\0", attr(1, b"a") * 2,
                      struct.pack("=HH", 5, 0x8001) + b"a\0\0\0",
                      struct.pack("=HH", 5, 1) + b"a\0\0\x01"]:
        try:
            attrs(malformed)
        except (ValueError, struct.error):
            continue
        raise ValueError("malformed attribute accepted")
    for malformed in [b"", b"x", encoded[:-1], struct.pack("=IHHII", 15, 2, 0, 1, 0),
                      message(NFT, 0x10, 1, nf())]:
        try:
            messages(malformed)
        except (ValueError, struct.error):
            continue
        raise ValueError("malformed message accepted")
    print("K1_CAPABILITY_CODEC_PASS")


if __name__ == "__main__":
    try:
        if sys.argv[1:] == ["--self-test"]:
            self_test()
        elif sys.argv[1:] == ["--isolated"]:
            isolated_test()
        else:
            raise ValueError("unsupported invocation")
    except Exception:
        # No traceback, kernel payload, filesystem detail or ambient value.
        print("K1_CAPABILITY_FAILED", flush=True)
        sys.exit(1)
