"""Fixed empty-table socket-lifetime experiment; test-only, never ownership proof."""
import errno
import os
import runpy
import struct
import sys

# Fixed sibling embedded by the Rust harness; isolated Python ignores ambient
# import paths. Loading definitions performs no socket or namespace operation.
core = runpy.run_path(os.path.join(os.path.dirname(__file__), "capability.py"))
require, Guard, Netlink = core["require"], core["Guard"], core["Netlink"]
NFT, TARGET, SENTINEL, TAG = (core[key] for key in ("NFT", "TARGET", "SENTINEL", "TAG"))
attr, nf, delete = core["attr"], core["nf"], core["delete"]
OWNER, PERSIST = 2, 4


def table(wire, name, flags=0, owner=None):
    require(name in (TARGET, SENTINEL) and flags in (0, OWNER, PERSIST, OWNER | PERSIST))
    response = wire.get(NFT + 1, nf() + attr(1, name))
    if response is None:
        return None
    kind, values = response
    require(kind == NFT and set(values).issubset({1, 2, 3, 4, 5, 6, 7})
            and values[1] == name and values[2] == struct.pack("!I", flags)
            and values[3] == bytes(4) and len(values[4]) == 8 and values[6] == TAG)
    require((7 not in values) if owner is None else values.get(7) == struct.pack("!I", owner))
    handle = struct.unpack("!Q", values[4])[0]
    require(handle > 0)
    return handle, values


def create_target(flags):
    require(flags in (OWNER, OWNER | PERSIST))
    return NFT, core["CREATE"] | core["EXCL"], attr(1, TARGET) + attr(2, struct.pack("!I", flags)) + attr(6, TAG)


def claim_target():
    # Only this test's previously exclusively-created empty target, after owner
    # release. This illustrates adoption mechanics, not an application adapter.
    return NFT, core["CREATE"], attr(1, TARGET) + attr(2, struct.pack("!I", OWNER | PERSIST))


def stage(name):
    print("K1_OWNER_STAGE=" + name, flush=True)


def isolated_test():
    stage("namespace")
    require(os.environ.get("OMAVLESS_K1_OWNER_CHILD") == "1")
    guard = Guard()  # Pinned parent, different current netns, loopback-only.
    monitor, owner, challenger = (Netlink(guard) for _ in range(3))
    require(len({monitor.port, owner.port, challenger.port}) == 3)
    require(table(monitor, TARGET) is None and table(monitor, SENTINEL) is None)
    stage("sentinel")
    require(not monitor.batch(monitor.generation(), [core["create"](SENTINEL)]))
    sentinel = table(monitor, SENTINEL)
    require(sentinel is not None)

    stage("owner_only")
    require(not owner.batch(owner.generation(), [create_target(OWNER)]))
    owned = table(monitor, TARGET, OWNER, owner.port)
    require(owned is not None)
    before = monitor.generation()
    require(challenger.batch(before, [delete(owned[0])]) == {errno.EPERM})
    require(monitor.generation() == before and table(monitor, TARGET, OWNER, owner.port) == owned)
    owner.sock.close()
    stage("owner_release")
    require(table(monitor, TARGET) is None and table(monitor, SENTINEL) == sentinel)

    stage("persistent")
    owner = Netlink(guard)
    require(owner.port not in (monitor.port, challenger.port))
    require(not owner.batch(owner.generation(), [create_target(OWNER | PERSIST)]))
    persistent = table(monitor, TARGET, OWNER | PERSIST, owner.port)
    require(persistent is not None and persistent[0] != owned[0])
    before = monitor.generation()
    require(challenger.batch(before, [delete(persistent[0])]) == {errno.EPERM})
    require(challenger.batch(before, [claim_target()]) == {errno.EPERM})
    require(monitor.generation() == before and table(monitor, SENTINEL) == sentinel)
    require(table(monitor, TARGET, OWNER | PERSIST, owner.port) == persistent)
    owner.sock.close()

    stage("persistent_release")
    orphan = table(monitor, TARGET, PERSIST)
    require(orphan is not None and orphan[0] == persistent[0])
    require(table(monitor, SENTINEL) == sentinel)
    stage("adoption")
    require(not challenger.batch(challenger.generation(), [claim_target()]))
    adopted = table(monitor, TARGET, OWNER | PERSIST, challenger.port)
    require(adopted is not None and adopted[0] == persistent[0])
    before = monitor.generation()
    require(monitor.batch(before, [delete(adopted[0])]) == {errno.EPERM})
    require(monitor.generation() == before and table(monitor, SENTINEL) == sentinel)
    require(table(monitor, TARGET, OWNER | PERSIST, challenger.port) == adopted)
    challenger.sock.close()

    stage("cleanup")
    require(table(monitor, TARGET, PERSIST) == orphan)
    require(not monitor.batch(monitor.generation(), [delete(orphan[0])]))
    require(table(monitor, TARGET) is None and table(monitor, SENTINEL) == sentinel)
    require(not monitor.batch(monitor.generation(), [delete(sentinel[0])]))
    require(table(monitor, TARGET) is None and table(monitor, SENTINEL) is None)
    guard.check()
    monitor.sock.close()
    os.close(guard.fd)
    stage("finished")
    print("K1_OWNER_PASS", flush=True)


def self_test():
    # Pure fixed-builder checks; no Guard, socket, namespace or netlink use.
    for flags in (OWNER, OWNER | PERSIST):
        kind, mode, payload = create_target(flags)
        values = core["attrs"](payload)
        require(kind == NFT and mode == core["CREATE"] | core["EXCL"])
        require(values == {1: TARGET, 2: struct.pack("!I", flags), 6: TAG})
    for flags in (0, PERSIST, -1, 8):
        try:
            create_target(flags)
        except ValueError:
            continue
        raise ValueError("unsafe flags accepted")
    kind, mode, payload = claim_target()
    require(kind == NFT and mode == core["CREATE"] and core["attrs"](payload) == {
        1: TARGET, 2: struct.pack("!I", OWNER | PERSIST)})
    print("K1_OWNER_CODEC_PASS")


if __name__ == "__main__":
    try:
        if sys.argv[1:] == ["--self-test"]:
            self_test()
        elif sys.argv[1:] == ["--isolated"]:
            isolated_test()
        else:
            raise ValueError("unsupported invocation")
    except Exception:
        print("K1_OWNER_FAILED", flush=True)
        sys.exit(1)
