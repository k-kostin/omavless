# SPDX-License-Identifier: MIT
"""Only exact Rust FullVpn bytes in a pinned disposable loopback namespace."""
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

live = runpy.run_path(os.path.join(os.path.dirname(__file__), "live_owner.py"))
core = live["core"]
require = core["require"]


def stage(name):
    print("K1_FULL_STAGE=" + name, flush=True)


def encoded(generation, first):
    with tempfile.TemporaryFile() as output:
        proc = subprocess.Popen([os.environ["OMAVLESS_K1_FULL_EXE"], "--ignored", "--exact",
                                 "atomic_full::encoder_child", "--nocapture"],
                                env={"K1_FULL_GENERATION": str(generation), "K1_FULL_SEQUENCE": str(first)},
                                stdin=subprocess.DEVNULL, stdout=output, stderr=subprocess.DEVNULL)
        deadline = time.monotonic() + 2
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
    values = []
    for key in (b"K1_FULL_BATCH=", b"K1_FULL_BARRIER="):
        lines = [line[len(key):] for line in raw.splitlines() if line.startswith(key)]
        require(len(lines) == 1)
        values.append(bytes.fromhex(lines[0].decode("ascii")))
    return values


def checked(result, first, collision=False):
    replies, errors, acks = result
    barrier = first + (15 if collision else 14)
    require(barrier in acks and set(replies) == {barrier} and replies[barrier][0] == 0xa0f)
    values = replies[barrier][1]
    require(set(values).issubset({1, 2, 3}) and len(values[1]) == 4 and values[1] != bytes(4))
    if not errors:
        require(acks == set(range(first + 1, first + (14 if collision else 13))) | {barrier})
    return set(errors.values())


def check_rust_raw_reply(generation, first, port, datagrams):
    # Bounded developer bridge only: metadata is not authenticated by the parser.
    require(0 < len(datagrams) <= 16 and sum(len(item[0]) for item in datagrams) <= 32768)
    blob = struct.pack("=IIII", generation, first, port, len(datagrams))
    for data, sender, flags in datagrams:
        blob += struct.pack("=IIII", len(data), sender[0], sender[1], flags) + data
    proc = subprocess.Popen(
        [os.environ["OMAVLESS_K1_FULL_EXE"], "--ignored", "--exact",
         "atomic_full::decoder_child", "--nocapture"],
        env={}, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
    try:
        output, _ = proc.communicate(blob, timeout=3)
    except subprocess.TimeoutExpired:
        proc.kill()
        proc.communicate()
        raise
    require(proc.returncode == 0 and len(output) <= 4096
            and output.splitlines().count(b"K1_FULL_RAW_REPLY_PASS") == 1)
    print("K1_FULL_RAW_REPLY_PASS", flush=True)


def batch(wire, generation, collision=False):
    first = wire.next_seq()
    request, barrier = encoded(generation, first)
    expected = {first: 16, first + 1: 0xa00, first + 2: 0xa03,
                **{first+i: 0xa06 for i in range(3, 13)}, first+13: 17, first+14: 0xa10}
    if collision:
        kind, flags, payload = core["create"](core["SENTINEL"])
        request = request[:-20] + core["message"](kind, flags | 5, first+13, core["nf"]()+payload)
        request += core["message"](17, 1, first+14, core["nf"](0, 10))
        barrier = core["message"](0xa10, 5, first+15, core["nf"](0))
        expected.update({first+13: 0xa00, first+14: 17, first+15: 0xa10})
    wire.seq = first + (15 if collision else 14)
    capture = not collision and os.environ.get("OMAVLESS_K1_FULL_RAW_REPLY_VM") == "1"
    datagrams = []
    if capture:
        wire.capture = lambda data, sender, flags: datagrams.append((data, sender, flags))
    try:
        result = wire.exchange(request, expected, barrier)
    finally:
        if capture:
            del wire.capture
    errors = checked(result, first, collision)
    if capture and not errors:
        check_rust_raw_reply(generation, first, wire.port, datagrams)
    return errors


def shape(raw, flags):
    doc = live["decode"](raw)
    require(set(doc) == {"nftables"})
    objects = doc["nftables"]
    require(type(objects) is list)
    if objects and set(objects[0]) == {"metainfo"}:
        meta = objects.pop(0)["metainfo"]
        require(set(meta) == {"version", "release_name", "json_schema_version"} and meta["json_schema_version"] == 1)
    require(len(objects) == 12)
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
    expected = live["expected"]()
    elided = copy.deepcopy(expected)
    for obj in elided[5:11]:
        expressions = obj["rule"]["expr"]
        require(expressions[0]["match"]["left"] == {"meta": {"key": "nfproto"}})
        del expressions[0]
    require(objects == expected or objects == elided)


def packets(guard):
    guard.check()
    for family, address in [(socket.AF_INET, "127.0.0.1"), (socket.AF_INET6, "::1")]:
        with socket.socket(family, socket.SOCK_DGRAM) as receiver, socket.socket(family, socket.SOCK_DGRAM) as sender:
            receiver.settimeout(0.25)
            receiver.bind((address, 0))
            sender.sendto(b"K1 fixed FullVpn loopback", receiver.getsockname())
            require(receiver.recv(64) == b"K1 fixed FullVpn loopback")
    guard.check()


def isolated():
    stage("namespace")
    require(os.environ.get("OMAVLESS_K1_FULL_CHILD") == "1")
    guard = core["Guard"]()
    wire = core["Netlink"](guard)
    require(struct.unpack("=Q", wire.sock.getsockopt(socket.SOL_SOCKET, 71, 8))[0] == core["namespace_id"](guard.fd))
    require(live["table"](wire) is None)
    guard.check()
    subprocess.run(["/usr/bin/ip", "link", "set", "lo", "up"], env={}, stdin=subprocess.DEVNULL,
                   stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=1, check=True)
    packets(guard)  # Positive control before policy.
    stage("rollback")
    require(not wire.batch(wire.generation(), [core["create"](core["SENTINEL"])]))
    sentinel = wire.table(core["SENTINEL"])
    before = wire.generation()
    require(batch(wire, before, True) == {errno.EEXIST})
    require(live["table"](wire) is None and wire.table(core["SENTINEL"]) == sentinel and wire.generation() == before)
    require(before > 1 and batch(wire, before-1) == {errno.ERESTART})
    require(live["table"](wire) is None and wire.table(core["SENTINEL"]) == sentinel and wire.generation() == before)
    stage("create")
    require(not batch(wire, before))
    stable = live["table"](wire)
    require(stable is not None and stable[2] == struct.pack("!I", 6) and stable[7] == struct.pack("!I", wire.port))
    stage("readback")
    generation = wire.generation()
    raw = live["readback"](guard)
    try:
        shape(raw, ["owner", "persist"])
    except Exception:
        print("K1_NFT_SYNTHETIC_READBACK=" + json.dumps(live["decode"](raw), separators=(",", ":")), flush=True)
        raise
    require(live["table"](wire) == stable and wire.generation() == generation)
    stage("packets")
    packets(guard)
    require(live["table"](wire) == stable and wire.generation() == generation and wire.table(core["SENTINEL"]) == sentinel)
    stage("closed")
    wire.sock.close()
    observer = core["Netlink"](guard)
    orphan = live["table"](observer)
    require(orphan[4] == stable[4] and orphan[2] == struct.pack("!I", 4) and 7 not in orphan)
    shape(live["readback"](guard), ["persist"])
    require(batch(observer, observer.generation()) == {errno.EEXIST})
    require(live["table"](observer) == orphan and observer.table(core["SENTINEL"]) == sentinel)
    observer.sock.close()
    guard.check()
    os.close(guard.fd)  # Namespace teardown owns all fixture cleanup; no orphan deletion.
    print("K1_FULL_PASS", flush=True)


def self_test():
    result = ({15: (0xa0f, {1: struct.pack("!I", 1)})}, {}, set(range(2,14)) | {15})
    require(not checked(result, 1))
    for missing in result[2]:
        live["refuses"](lambda: checked((result[0], {}, result[2]-{missing}), 1))
    live["refuses"](lambda: checked((result[0], {}, result[2] | {1}), 1))
    live["refuses"](lambda: checked(({}, {}, result[2]), 1))
    objects = copy.deepcopy(live["expected"]())
    for i, obj in enumerate(objects):
        value = next(iter(obj.values()))
        value["handle"] = i + 1
        if i == 0:
            value["flags"] = ["owner", "persist"]
    shape(json.dumps({"nftables": objects}).encode(), ["owner", "persist"])
    elided = copy.deepcopy(objects)
    for obj in elided[5:11]:
        del obj["rule"]["expr"][0]
    shape(json.dumps({"nftables": elided}).encode(), ["owner", "persist"])
    for mutation in [objects + [objects[-1]], objects[:-1], objects[::-1]]:
        live["refuses"](lambda: shape(json.dumps({"nftables": mutation}).encode(), ["owner", "persist"]))
    wrong = copy.deepcopy(objects)
    wrong[4]["rule"]["expr"][0]["match"]["right"] = 0
    live["refuses"](lambda: shape(json.dumps({"nftables": wrong}).encode(), ["owner", "persist"]))
    print("K1_FULL_CODEC_PASS")


if __name__ == "__main__":
    try:
        if sys.argv[1:] == ["--self-test"]:
            self_test()
        elif sys.argv[1:] == ["--isolated"]:
            isolated()
        else:
            raise ValueError("unsupported invocation")
    except Exception:
        print("K1_FULL_FAILED", flush=True)
        sys.exit(1)
