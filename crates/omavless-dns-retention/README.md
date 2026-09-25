# Uninstalled systemd descriptor-retention boundary

This crate implements fixed notification and typed manager-readback mechanics.
It has no installed production consumer, host-bus discovery, environment-based
notification destination, service installation or DNS effects. The trusted
`from_admitted_parts` constructor accepts a connection, pinned unique manager
owner and notification descriptor from the broker's fixed root-service admission.
These are internal composition values, never caller IPC fields. It validates
the owner shape, Unix datagram descriptor and service policy; it does not replace
the root context's filesystem/bus authentication. Tests instantiate it on a
disposable bus and private Unix datagram socket. The existing [real user-unit experiment](../../docs/development/DNS_FDSTORE.md)
establishes local systemd behavior separately; these tests exercise the Rust wire
implementation against an independently implemented manager fixture.

## One fixed resource, not a privileged command channel

The service object and descriptor name are fixed constants. Notifications can
only insert the supplied descriptor (`FDSTORE=1`, `FDPOLL=0`), send a separate
pipe-descriptor `BARRIER=1`, or remove the fixed descriptor name. No caller IPC
can supply notification keys, paths, units, commands or DNS parameters.

Admission requires all of the following before any insertion:

- Manager properties report capacity **exactly one**, preserve policy **yes**,
  notify access **main**, and MainPID equal to this process.
- Both the store count and descriptor dump are empty. An old entry is refused
  even if its metadata matches the supplied descriptor; no adoption is inferred.
- The trusted caller serializes the sole service-main-process notification
  writer. This Rust object prevents concurrent mutable operations but cannot
  constrain unrelated code deliberately sending notifications from the same
  privileged process.

The actual `SCM_RIGHTS` insertion carries the held object, not its name. A
separate real pipe barrier waits for writer closure. **Barrier completion is not
acceptance.** The implementation then rechecks policy, count and the manager's
`DumpFileDescriptorStore` result against the original descriptor's metadata.
Every success reply must also come from the pinned unique manager bus owner;
well-known name replacement is not adopted.

The installed `org.freedesktop.systemd1.Service.xml` and version-matched systemd
source establish the dump signature `a(suuutuusu)` and field order: descriptor
name, mode, device major/minor, inode, special-device major/minor, path and open
flags. The implementation compares these identity fields, normalizing
`O_LARGEFILE` exactly as the manager does. Path is bounded/validated, never
displayed or used to reopen the object.

**Metadata is not an open-file-description identifier.** Different opens of
`/dev/net/tun` can have identical metadata. The original-object retention claim
depends on the complete chain: trusted manager, exclusive empty single-slot
store, one exact descriptor transfer, barrier, and confirming metadata/count.
Readback alone must never authorize adoption of a pre-existing TUN slot.

## State and uncertainty

`retain(OwnedFd)` is one-shot. It retains a local copy on any refusal/failure and
never retries insertion or automatically removes a manager-held reference.
`verify()` can confirm an already admitted state, but cannot promote quarantine
back to success. `quarantine()` permanently blocks ordinary removal/readiness
after an unknown DNS outcome reported by the transaction owner.

`release_after_proven_boundary()` is only a fixed wire primitive. It does not
prove DNS cleanup: the trusted broker must establish that boundary first.
It verifies the current entry, sends removal, completes a barrier, then requires
an empty count/dump. An unknown removal outcome keeps the local object and
forbids retries. Drop sends **no** notification. There is no force-release or
automatic restart/recovery API.

No local Rust object can survive process death. Production requires the manager
store, durable transaction/quarantine decisions and a restart path that treats
inherited descriptors as recovery-required. This crate does not provide that
journal or crash-adoption constructor. Neither a new readback nor successful retention fences
an old queued resolved operation. Initial crash recovery may deliberately require
administrative recovery/reboot while the original TUN remains retained; that is
an availability tradeoff, not a kill-switch or automatic recovery claim.

## Bounds and acceptance

Notification sends/barrier waits and full asynchronous D-Bus send/reply futures
have finite per-operation deadlines. Replies are capped at 16 KiB before body
decoding, accept no received descriptors and allow at most one dump row; paths
are limited to 4096 bytes without control characters. Errors/Debug expose only
fixed English classifications. As in the resolved boundary, the acceptance cap
is not zbus's upstream 128-MiB wire-allocation ceiling. Production adoption still
needs authenticated root-bus/manager admission and an explicit resource policy.

All descriptor operations use safe locked rustix 1.1.5 APIs; workspace unsafe
forbid is unchanged. Other dependencies reuse the locked zbus/async-io graph.

```sh
CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0 \
  cargo test --locked -p omavless-dns-retention
```

Tests cover trusted-constructor refusal, shared absolute deadlines, real ancillary transfer and pipe barriers, typed metadata,
capacity disappearing with a successful barrier, pre-existing equal-metadata
slots, every identity field, malformed/oversized/FD-bearing replies, unexpected
sender/manager replacement, invalid manager policy, barrier data/timeouts,
unconfirmed removal, local Drop retention and permanent quarantine. They never
contact the user's session/system bus or invoke systemctl, sudo, TUN or DNS.

Sources: installed `/usr/share/dbus-1/interfaces/org.freedesktop.systemd1.Service.xml`,
`sd_notify(3)`, and the [systemd v261 dump implementation](https://github.com/systemd/systemd/blob/v261/src/core/dbus-service.c#L239).
