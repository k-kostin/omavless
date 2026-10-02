"""VM-only Emergency-policy lifetime experiment, not a production authenticator.

All creation stays in a newly isolated loopback-only namespace. The retained
creator provides causal in-process evidence, never persistent orphan authority.
"""
import copy
import errno
import json
import os
import runpy
import socket
import struct
import subprocess
import sys
import tempfile
import time

core = runpy.run_path(os.path.join(os.path.dirname(__file__), "capability.py"))
require, Guard, Netlink = (core[k] for k in ("require", "Guard", "Netlink"))
attr, nf, NFT = (core[k] for k in ("attr", "nf", "NFT"))
TARGET, CHAIN = b"omavless_netguard\0", b"output_guard\0"
LIMIT = 32768


def u32(value):
    return struct.pack("!I", value)


def nested(kind, payload):
    value = bytearray(attr(kind, payload))
    struct.pack_into("=H", value, 2, kind | 0x8000)
    return bytes(value)


def expression(name, data):
    return nested(1, attr(1, name) + nested(2, data))


def verdict(code):
    require(code in (0, 1))
    return expression(b"immediate\0", attr(1, u32(0)) + nested(2, nested(2, attr(1, u32(code)))))


def create():
    return NFT, core["CREATE"] | core["EXCL"], attr(1, TARGET) + attr(2, u32(6))


def chain():
    return NFT + 3, core["CREATE"] | core["EXCL"], (attr(1, TARGET) + attr(3, CHAIN)
        + nested(4, attr(1, u32(3)) + attr(2, u32(300)))
        + attr(5, u32(0)) + attr(7, b"filter\0"))


def rule(loopback):
    require(type(loopback) is bool)
    expr = (expression(b"meta\0", attr(1, u32(1)) + attr(2, u32(7)))
            + expression(b"cmp\0", attr(1, u32(1)) + attr(2, u32(0))
                         + nested(3, attr(1, b"lo\0")))) if loopback else b""
    expr += verdict(1 if loopback else 0)
    # NLM_F_APPEND preserves exact policy order.
    return NFT + 6, core["CREATE"] | 0x800, attr(1, TARGET) + attr(2, CHAIN) + nested(4, expr)


def table(wire):
    result = wire.get(NFT + 1, nf() + attr(1, TARGET))
    if result is None:
        return None
    kind, values = result
    require(kind == NFT and set(values).issubset({1, 2, 3, 4, 5, 7})
            and values[1] == TARGET and len(values[2]) == 4
            and len(values[3]) == 4 and len(values[4]) == 8)
    return values


def strict_object(pairs):
    result = {}
    for key, value in pairs:
        require(key not in result)
        result[key] = value
    return result


def decode(data):
    require(0 < len(data) <= LIMIT)
    return json.loads(data, object_pairs_hook=strict_object)


def expected():
    with open(os.path.join(os.path.dirname(__file__), "expected.json"), "rb") as file:
        commands = decode(file.read(LIMIT + 1))["nftables"]
    return [c["create" if i == 0 else "add"] for i, c in enumerate(commands)]


def exact_shape(data, flags):
    doc = decode(data)
    require(set(doc) == {"nftables"})
    objects = doc["nftables"]
    require(type(objects) is list)
    if objects and set(objects[0]) == {"metainfo"}:
        meta = objects.pop(0)["metainfo"]
        require(set(meta) == {"version", "release_name", "json_schema_version"}
                and meta["json_schema_version"] == 1
                and type(meta["version"]) is str and type(meta["release_name"]) is str)
    require(len(objects) == 4)
    seen = set()
    for i, obj in enumerate(objects):
        kind = "table" if i == 0 else "chain" if i == 1 else "rule"
        require(set(obj) == {kind})
        value = obj[kind]
        handle = value.pop("handle")
        require(type(handle) is int and 0 < handle < 2**64)
        if i:
            require(handle not in seen)
            seen.add(handle)
        else:
            require(value.pop("flags") == flags)
    require(objects == expected())


def readback(guard):
    guard.check()
    # Files avoid unbounded PIPE collection and descendants retaining pipes.
    with tempfile.TemporaryFile() as output:
        proc = subprocess.Popen(["/usr/bin/nft", "--json", "--handle", "--numeric",
                                 "--numeric-priority", "list", "table", "inet", "omavless_netguard"],
                                env={}, stdin=subprocess.DEVNULL, stdout=output, stderr=subprocess.DEVNULL)
        deadline = time.monotonic() + 1
        try:
            while proc.poll() is None:
                require(time.monotonic() < deadline and os.fstat(output.fileno()).st_size <= LIMIT)
                time.sleep(0.001)
            require(proc.returncode == 0)
            output.seek(0)
            result = output.read(LIMIT + 1)
            require(len(result) <= LIMIT)
        finally:
            if proc.poll() is None:
                proc.kill()
            proc.wait()
    guard.check()
    return result


class LiveEmergency:
    def __init__(self, guard):
        self.guard, self.wire, self.live = guard, Netlink(guard), False
        try:
            self.cookie = struct.unpack("=Q", self.wire.sock.getsockopt(socket.SOL_SOCKET, 71, 8))[0]
            require(self.cookie == core["namespace_id"](guard.fd) and self.cookie != 0)
            require(not self.wire.batch(self.wire.generation(), [create()]))
            self.created = table(self.wire)
            require(self.created is not None and self.created[2] == u32(6)
                    and self.created[7] == u32(self.wire.port))
            require(not self.wire.batch(self.wire.generation(), [chain(), rule(True), rule(False)]))
            self.stable = table(self.wire)
            require(self.stable[4] == self.created[4])
            self.live = True
            self.verify()
        except Exception:
            self.close()
            raise

    def verify(self):
        # Closed/malformed proof refuses BEFORE any kernel/child operation.
        require(self.live and self.wire.sock.fileno() >= 0)
        try:
            self.guard.check()
            require(self.wire.sock.getsockname() == (self.wire.port, 0))
            require(struct.unpack("=Q", self.wire.sock.getsockopt(socket.SOL_SOCKET, 71, 8))[0]
                    == self.cookie == core["namespace_id"](self.guard.fd))
            before = self.wire.generation()
            require(table(self.wire) == self.stable)
            exact_shape(readback(self.guard), ["owner", "persist"])
            require(table(self.wire) == self.stable and self.wire.generation() == before)
            self.guard.check()
        except Exception:
            self.live = False  # No retry can revive uncertain proof.
            raise

    def close(self):
        self.live = False
        self.wire.sock.close()


def refuses(action):
    try:
        action()
    except (ValueError, KeyError, TypeError, OSError):
        return
    raise ValueError("uncertain proof accepted")


def stage(name):
    print("K1_LIVE_OWNER_STAGE=" + name, flush=True)


def isolated_test(factory=LiveEmergency, prelude=None):
    stage("namespace")
    require(os.environ.get("OMAVLESS_K1_LIVE_OWNER_CHILD") == "1")
    guard = Guard()
    observer = Netlink(guard)
    require(table(observer) is None)
    if prelude is not None:
        prelude(guard, observer)
    require(not observer.batch(observer.generation(), [core["create"](core["SENTINEL"])]))
    sentinel = observer.table(core["SENTINEL"])
    stage("collision")
    # Identical policy from a different retained creator is not adoptable.
    foreign = factory(guard)
    prior = table(observer)
    refuses(lambda: factory(guard))
    require(table(observer) == prior)
    stage("drift")
    # A still-owned, same-handle table with a late extra rule is not the policy.
    require(not foreign.wire.batch(foreign.wire.generation(), [rule(False)]))
    refuses(foreign.verify)
    require(not foreign.live)
    refuses(foreign.verify)
    # Fixture-only cleanup through its still-live creator. No recovery/adoption
    # operation is exposed by LiveEmergency; this is not a product effect path.
    require(not foreign.wire.batch(foreign.wire.generation(), [core["delete"](struct.unpack("!Q", prior[4])[0])]))
    foreign.close()
    stage("create")
    live = factory(guard)
    stage("readback")
    live.verify()
    stage("foreign")
    before = observer.generation()
    require(observer.batch(before, [core["delete"](struct.unpack("!Q", live.stable[4])[0])]) == {errno.EPERM})
    require(observer.generation() == before and observer.table(core["SENTINEL"]) == sentinel)
    live.verify()
    stage("closed")
    stable = live.stable
    live.close()
    refuses(live.verify)
    orphan = table(observer)
    require(orphan[4] == stable[4] and orphan[2] == u32(4) and 7 not in orphan)
    exact_shape(readback(guard), ["persist"])
    # A new creator refuses even a byte-for-byte policy-shaped orphan.
    refuses(lambda: factory(guard))
    require(table(observer) == orphan and observer.table(core["SENTINEL"]) == sentinel)
    guard.check()
    observer.sock.close()
    # Do not turn the test's known orphan into a recovery precedent. Namespace
    # teardown removes it and its sentinel; no orphan acquisition/delete exists.
    os.close(guard.fd)
    stage("finished")
    print("K1_LIVE_OWNER_PASS", flush=True)


def self_test():
    require(create() == (NFT, 0x600, attr(1, TARGET) + attr(2, u32(6))))
    require(chain()[0] == NFT + 3 and rule(True)[0] == NFT + 6)
    require(rule(True) != rule(False))
    refuses(lambda: rule(1))
    refuses(lambda: verdict(3))
    refuses(lambda: decode(b'{"nftables":[],"nftables":[]}'))
    refuses(lambda: decode(b" " * (LIMIT + 1)))
    objects = copy.deepcopy(expected())
    for i, obj in enumerate(objects):
        value = next(iter(obj.values()))
        value["handle"] = i + 1
        if i == 0:
            value["flags"] = ["owner", "persist"]
    valid = {"nftables": objects}
    exact_shape(json.dumps(valid).encode(), ["owner", "persist"])
    for mutated in [objects + [objects[-1]], objects[:2] + objects[3:], objects[::-1]]:
        refuses(lambda: exact_shape(json.dumps({"nftables": mutated}).encode(), ["owner", "persist"]))
    wrong = copy.deepcopy(valid)
    wrong["nftables"][2]["rule"]["expr"][0]["match"]["right"] = "eth0"
    refuses(lambda: exact_shape(json.dumps(wrong).encode(), ["owner", "persist"]))
    for key, value in [("flags", ["persist"]), ("comment", "copied-proof"), ("handle", 0)]:
        wrong = copy.deepcopy(valid)
        wrong["nftables"][0]["table"][key] = value
        refuses(lambda: exact_shape(json.dumps(wrong).encode(), ["owner", "persist"]))
    closed = LiveEmergency.__new__(LiveEmergency)
    closed.live = False
    refuses(closed.verify)  # No wire exists: refusal must precede its access.
    print("K1_LIVE_OWNER_CODEC_PASS")


if __name__ == "__main__":
    try:
        if sys.argv[1:] == ["--self-test"]:
            self_test()
        elif sys.argv[1:] == ["--isolated"]:
            isolated_test()
        else:
            raise ValueError("unsupported invocation")
    except Exception:
        print("K1_LIVE_OWNER_FAILED", flush=True)
        sys.exit(1)
