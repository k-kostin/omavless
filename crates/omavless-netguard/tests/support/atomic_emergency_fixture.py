# SPDX-License-Identifier: MIT
"""Developer-only consumer of Rust's fixed bytes; never a product executor."""
import errno
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
require, nf, NFT = (core[k] for k in ("require", "nf", "NFT"))


def encoded(generation, sequence):
    # Trusted current test executable, fixed exact pure test; never user IPC.
    with tempfile.TemporaryFile() as output:
        proc = subprocess.Popen([os.environ["OMAVLESS_K1_ATOMIC_EXE"], "--ignored", "--exact",
                                 "atomic_emergency::encoder_child", "--nocapture"],
                                env={"K1_ATOMIC_GENERATION": str(generation),
                                     "K1_ATOMIC_SEQUENCE": str(sequence)},
                                stdin=subprocess.DEVNULL, stdout=output, stderr=subprocess.DEVNULL)
        deadline = time.monotonic() + 2
        try:
            while proc.poll() is None:
                require(time.monotonic() < deadline and os.fstat(output.fileno()).st_size <= 32768)
                time.sleep(0.001)
            require(proc.returncode == 0)
            output.seek(0)
            data = output.read(32769)
            require(len(data) <= 32768)
        finally:
            if proc.poll() is None:
                proc.kill()
            proc.wait()
    result = []
    for key in (b"K1_ATOMIC_BATCH=", b"K1_ATOMIC_BARRIER="):
        lines = [line[len(key):] for line in data.splitlines() if line.startswith(key)]
        require(len(lines) == 1)
        result.append(bytes.fromhex(lines[0].decode("ascii")))
    return result


def checked_reply(result, first, extra=False):
    replies, errors, acknowledged = result
    barrier = first + (7 if extra else 6)
    require(barrier in acknowledged and set(replies) == {barrier}
            and replies[barrier][0] == NFT + 15)
    values = replies[barrier][1]
    require(set(values).issubset({1, 2, 3}) and len(values[1]) == 4 and values[1] != bytes(4))
    if not errors:
        require(acknowledged == set(range(first + 1, first + (6 if extra else 5))) | {barrier})
    return set(errors.values())


def check_rust_raw_reply(generation, first, port, datagrams):
    # Developer-only bridge: bounded kernel transcript, never a product socket.
    require(0 < len(datagrams) <= 16 and sum(len(item[0]) for item in datagrams) <= 32768)
    blob = struct.pack("=IIII", generation, first, port, len(datagrams))
    for data, sender, flags in datagrams:
        blob += struct.pack("=IIII", len(data), sender[0], sender[1], flags) + data
    proc = subprocess.Popen(
        [os.environ["OMAVLESS_K1_ATOMIC_EXE"], "--ignored", "--exact",
         "atomic_emergency::decoder_child", "--nocapture"],
        env={}, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
    try:
        output, _ = proc.communicate(blob, timeout=3)
    except subprocess.TimeoutExpired:
        proc.kill()
        proc.communicate()
        raise
    if proc.returncode != 0:
        for data, _, _ in datagrams:
            for kind, flags, sequence, pid, body in core["messages"](data):
                print(f"K1_RAW_REPLY_META={kind},{flags},{sequence - first},"
                      f"{pid == port},{len(body)}", flush=True)
    require(proc.returncode == 0 and len(output) <= 4096
            and b"K1_ATOMIC_RAW_REPLY_PASS" in output)


def batch(wire, generation, collision=False):
    first = wire.next_seq()
    request, barrier = encoded(generation, first)
    expected = {first: 16, first + 1: NFT, first + 2: NFT + 3,
                first + 3: NFT + 6, first + 4: NFT + 6, first + 5: 17, first + 6: NFT + 16}
    if collision:
        # Fault injection is fixture-only. Rust exposes no arbitrary operation API.
        kind, flags, payload = core["create"](core["SENTINEL"])
        request = request[:-20] + core["message"](kind, flags | 5, first + 5, nf() + payload)
        request += core["message"](17, 1, first + 6, nf(0, 10))
        barrier = core["message"](NFT + 16, 5, first + 7, nf(0))
        expected.update({first + 5: NFT, first + 6: 17, first + 7: NFT + 16})
    wire.seq = first + (7 if collision else 6)
    datagrams = []
    if not collision and os.environ.get("OMAVLESS_K1_RAW_REPLY_VM") == "1":
        wire.capture = lambda data, sender, flags: datagrams.append((data, sender, flags))
    try:
        result = wire.exchange(request, expected, barrier)
    finally:
        if hasattr(wire, "capture"):
            del wire.capture
    errors = checked_reply(result, first, collision)
    if datagrams and not errors:
        check_rust_raw_reply(generation, first, wire.port, datagrams)
    return errors


class AtomicEmergency(live["LiveEmergency"]):
    def __init__(self, guard):
        self.guard, self.wire, self.live = guard, core["Netlink"](guard), False
        try:
            self.cookie = struct.unpack("=Q", self.wire.sock.getsockopt(socket.SOL_SOCKET, 71, 8))[0]
            require(self.cookie == core["namespace_id"](guard.fd) and self.cookie != 0)
            require(not batch(self.wire, self.wire.generation()))
            self.stable = live["table"](self.wire)
            require(self.stable is not None and self.stable[2] == live["u32"](6)
                    and self.stable[7] == live["u32"](self.wire.port))
            self.live = True
            self.verify()
        except Exception:
            self.close()
            raise


def rollback(guard, wire):
    live["stage"]("rollback")
    require(not wire.batch(wire.generation(), [core["create"](core["SENTINEL"])]))
    sentinel = wire.table(core["SENTINEL"])
    before = wire.generation()
    require(batch(wire, before, True) == {errno.EEXIST})
    require(live["table"](wire) is None and wire.table(core["SENTINEL"]) == sentinel
            and wire.generation() == before)
    require(before > 1)
    require(batch(wire, before - 1) == {errno.ERESTART})
    require(live["table"](wire) is None and wire.table(core["SENTINEL"]) == sentinel
            and wire.generation() == before)
    require(not wire.batch(before, [core["delete"](sentinel[0])]))
    guard.check()


def self_test():
    # Independent previously kernel-validated fixture builders, not Rust output
    # regenerated into its own expectation. Both fence extremes are exercised.
    for generation, first in [(1, 1), (0xffffffff, 0xfffffff9)]:
        actual, barrier = encoded(generation, first)
        expected = core["message"](16, 1, first, nf(0, 10) + core["attr"](1, live["u32"](generation)))
        for i, (kind, flags, payload) in enumerate([live["create"](), live["chain"](), live["rule"](True), live["rule"](False)], 1):
            expected += core["message"](kind, flags | 5, first + i, nf() + payload)
        expected += core["message"](17, 1, first + 5, nf(0, 10))
        require(actual == expected and barrier == core["message"](NFT + 16, 5, first + 6, nf(0)))
    replies = {7: (NFT + 15, {1: live["u32"](1)})}
    acks = {2, 3, 4, 5, 7}
    require(not checked_reply((replies, {}, acks), 1))
    for missing in acks:
        live["refuses"](lambda: checked_reply((replies, {}, acks - {missing}), 1))
    live["refuses"](lambda: checked_reply(({}, {}, acks), 1))
    live["refuses"](lambda: checked_reply((replies, {}, acks | {1}), 1))
    require(checked_reply((replies, {3: errno.EINVAL}, {7}), 1) == {errno.EINVAL})
    print("K1_ATOMIC_CODEC_PASS")


if __name__ == "__main__":
    try:
        if sys.argv[1:] == ["--self-test"]:
            self_test()
        elif sys.argv[1:] == ["--isolated"]:
            live["isolated_test"](AtomicEmergency, rollback)
            print("K1_ATOMIC_PASS", flush=True)
        else:
            raise ValueError("unsupported invocation")
    except Exception:
        print("K1_ATOMIC_FAILED", flush=True)
        sys.exit(1)
