# Inactive K1 authority and session lifetime composition

This successor to the exact #631 private lifecycle checkpoint adds normal-
compiled Rust composition, not another guest proof script or a production
provider. K1 remains unavailable. The private lifecycle's negative namespace
witness and synthetic creator epoch cannot enter this candidate as authority.

## Concrete gap and bounded change

The older `SessionOwner` receives a copied `NamespaceObservation` independently
of its `EffectPort`. Enrollment and listener admission are checked, but those
facts alone do not bind a canonical namespace to the creator across a client
exchange. Its `LockedState` exchange also previously had no provider fence
after request decoding or immediately before response delivery.

`AuthoritySession` now owns one admitted listener, one enrollment-bound
`LockedState`, and one private `BoundEffects<CanonicalCreator>`. The namespace
projection is obtained exactly once from that owned provider, never supplied
as a separate argument. Every later provider projection must match the entire
initial epoch. Equality is consistency only, not provenance. Both traits remain
crate-sealed; there is no blanket conversion from an ordinary EffectPort,
receipt, integer tuple, namespace FD supplied by a caller, or private fixture.
There is no non-test CanonicalCreator implementation.

The shared exchange now supplies before/after accept, receive and reply fences.
The wrapper additionally checks before/after every observe/create/replace/delete.
It sets its refusal latch before entering provider or effect code, so an error
or unwind cannot revive that instance. Any non-idle/non-served session result
permanently seals this stronger candidate before another accept or callback.
Legacy/model EffectPorts retain a default no-op exchange hook; that compatibility
path is explicitly not authentication and cannot construct AuthoritySession.

Only LockedState writes durable Pending, Armed/Closed and terminal receipts.
Loss after an effect but before verified return retains Pending; there is no
compensation or automatic retry. Loss before reply after an already durable
commit preserves that exact committed state and suppresses delivery. A loss
after send cannot retract bytes already delivered; it seals subsequent work
without rewriting a committed record or claiming the client received nothing.

The whole owner is placed in ManuallyDrop before fallible listener setup or
provider admission. Normal drop and unwinding retain the listener, creator and
state lock for process lifetime, without unlink, disarm or closing an ownership
socket. There is deliberately no normal release/recovery API. A synthetic-only
test release exists. This is a conservative inactive retention policy, not a
claim that resources survive process death or an implemented service shutdown.
The wrapper is not Send/Sync; this prevents cross-thread movement but does not
prove same-thread namespace switch-and-return cannot occur.

## Required provider, not supplied by this slice

A future provider must retain original safe descriptors from reviewed canonical
system-manager launch; establish namespace type/ID and socket namespace cookie
with adopted safe APIs; structurally prohibit namespace transitions; hold the
actual exclusive creator socket and complete verified policy inventory through
conditional effects; and establish nft subsystem/boot continuity. It must not
reconstruct ownership from matching names, ports, handles or durable records.
Canonical launch, dependency adoption and first-listener publication authority
still require their separate review. The existing admitted listener must come
from the trusted bind-before-access lifetime, not merely a matching path.

Deterministic controls use the real private listener publisher, authenticated
Unix exchange and actual LockedState writers with a synthetic provider. They
cover successful arm/disarm, admission/accept/receive/observe/effect/reply cuts,
actual same-generation replacement and delete cuts, errors/panics after actual
synthetic effects, post-effect observation loss, constructor panic retention,
listener replacement, unchanged-inode epoch replacement and retained drop. Their fake
namespace/kernel facts cannot be relabelled as installed or kernel acceptance.

The existing inactive socket writer additionally checks its elapsed budget
after timeout setup and after its final write. Deterministic real-socket tests
show that a late write may deliver complete bytes yet must return delivery
unknown, while a late timeout-setup return starts no write. This does not cancel
a blocked syscall or retract bytes. No new product caller is added.

The first expanded test run correctly rejected a proposed replacement request
using a different generation while already armed; two new tests had mistakenly
expected an effect there. The controls now use the contract's explicit
same-generation Arm replacement. The production planner was not weakened.

Installed service/package/enrollment/recovery, runtime protected connect and
disconnect sequencing, core-mark/DNS/firewall integration and all host/physical
acceptance remain open under [KILL_SWITCH](../roadmap/KILL_SWITCH.md) and the
[safe namespace prerequisite](K1_NAMESPACE_API_PREREQUISITE.md). No new runtime
dependency, privileged IPC method, helper binary, installation, VM operation,
main/RC merge or release is introduced.
