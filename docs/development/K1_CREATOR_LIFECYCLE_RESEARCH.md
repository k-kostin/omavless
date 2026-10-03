# K1 actual creator and private-writer lifecycle research

This branch starts at current `rc/0.9.7` source
`c4e800425243c1b02165f82153e4bf418fe465e6`. Composition commit
`688dcef` copies the exact self-contained netguard crate, owning K1 contracts,
evidence and developer fixtures from #547
`c223ba2da88da65d2563869c1fe5df5c75f89cb0`. Existing runtime, DNS crates,
package pins and production paths are preserved. Cargo adds this workspace
member and its package entry only; all dependencies already exist in the RC
lockfile. Baseline ordinary netguard suite: 191 passed, 27 opt-in ignored.
The review-only namespace patch is retained as an artifact, not adopted.

## Actual mechanism composition, test compilation only

`kernel_creator_lifecycle.rs` exists only under `cfg(test)`. Its creator owns
one `LocalReadSession`, retained namespace descriptor and netlink socket from
exclusive atomic FullVpn creation through all observations and effects. Live
causality stays inside that object; no stored receipt, port, handle or matching
policy can reconstruct it. Each effect uses a complete generation-bracketed
inventory on that same socket, never a later generation-refresh retry.

Create installs the entire existing ten-rule FullVpn template in one exclusive,
generation-conditioned batch. Replace deletes the retained handle and installs
that complete fixed template in the **same** atomic batch. Begin, every
operation and commit END require exact kernel ACKs. Queued operation successes
without END never complete. Stale generation, unknown/lost/malformed replies,
readback drift and generation regression/wrap seal the session. Complete
table/chain/rule/set/object/flowtable readback is required before the creator
returns a new handle. Delete consumes #547's same-inventory handle witness and
requires definitive commit plus absent readback. No retry or compensation
mutation follows an uncertain send.

The test creator implements the existing sealed EffectPort solely to exercise
the real `LockedState` private marker/receipt writers. Pending must already be
durable and a competing root-state opener must be Busy at every kernel effect.
Only LockedState writes Armed/Closed/terminal records. The same test creator
also composes with the existing peer-authenticated, one-request `SessionOwner`.

The test explicitly supplies a **synthetic local epoch** using the existing
`NamespaceObservation::Canonical` model seam. That label is not authenticated
host authority. Production still has no EffectPort implementation, namespace
authenticator, helper binary, root service or kill-switch activation. No local
unsafe wrapper, nsenter shortcut or adopted dependency is introduced.

## Isolated scenarios and limits

The opt-in gate runs thirteen fresh loopback-only user/network namespaces:
arm/replace/disarm and private writer reopen with retained creator; actual
SessionOwner exchanges; lost create/replace/delete adapter results; orphan
refusal after creator close; same-handle appended-rule drift; foreign-generation
changes before create/replace; lost service response after durable commit;
and SIGKILL after actual create/replace/delete completion before adapter return.
Every process retains the original namespace descriptor and refuses the parent
namespace before any socket/effect. Namespace teardown is the final cleanup.

The three adapter-result losses and SIGKILL cuts happen **after actual raw ACKs
and complete readback**. They do not claim that raw netlink ACK packets were
dropped. Pending barriers and zero subsequent effects remain mandatory even
after observed deletion/absence. A missing service reply preserves the completed
state; a later explicit Status does not prove delivery of the earlier response
and never automatically repeats Arm. Orphan policy is untrusted and never
adopted/deleted by a new creator. SIGKILL tests use actual surviving kernel
state, not a reconstruction from a synthetic checkpoint number.

Run only after an explicit exclusive development-VM lease:

```sh
OMAVLESS_K1_LIFECYCLE_VM=1 cargo test --locked -p omavless-netguard --lib \
  kernel_observer::creator_lifecycle::tests::creator_lifecycle_in_disposable_vm \
  -- --ignored --exact --nocapture
```

At this initial source checkpoint the VM gate is **not yet run**. Ordinary
wire/parser/direct-invocation tests and compilation are not kernel acceptance.
The current primary-PC network and installed VM runtime are outside this gate.

Remaining product gates include reviewed safe namespace API adoption and trusted
canonical launch; structural no-switch authority; nft subsystem continuity;
durable creator/orphan adjudication; power-cut/storage/boot matrices; installed
service/package/runtime exchange; core mark/DNS/firewall integration; and the
physical NIC/suspend/boot acceptance matrix in [KILL_SWITCH](../roadmap/KILL_SWITCH.md).
This is a development checkpoint, not product policy adoption, main/RC merge,
release publication, marketplace submission or completed K1.
