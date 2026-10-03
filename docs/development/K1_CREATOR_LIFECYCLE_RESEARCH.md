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

## Exact-head verification

Immutable tested code is
`02df07a1bbe64463ce583fa9606b2b90cbbc1459`. The frozen standalone test
executable has SHA-256
`baa4053bbb0e63fc039eff3909531579b5f4faea5f298360673efd258bddc950`;
it was copied outside Cargo before testing, verified again in the VM, had no
file capabilities, and was never rebuilt during its self-reexecution suites.

On 2026-10-03, under an exclusive development-VM lease, kernel
`7.2.5-3-omarchy` passed all thirteen actual isolated scenarios in 21 complete
repetitions (273 scenario executions, including 63 real SIGKILL cuts). The
fixture-only nonzero UID mapping and retained capabilities are local test
history, not a canonical production launch recipe.

The last ten complete repetitions (130 scenarios) ran inside one guarded
before/after invocation. It confirmed unchanged parent network namespace,
canonical **user** `omavless-runtime.service` active/running at PID 86349,
runtime executable fingerprint, and private profile/desired/ownership file
fingerprints. Private bytes, hashes and network snapshots were not published.
Deep JSON address/route/rule comparison confirmed only changing DHCP
`preferred_life_time` and `valid_life_time` counters. It preserved equality of
all non-timer fields, including interface identity/name/flags/MTU, address and
prefix, all route tables, rules and metrics. No `expires` difference occurred.
An earlier exact, unnormalized network comparison exited nonzero after ten
successful mechanism repetitions; that preservation attempt is **not** PASS.
The subsequent guarded run records the explicitly confirmed clock-field
exception, not suppression of actual routing or process changes.

Owned namespace processes were reaped, the private fixture directory was empty
after every repetition, and the entire owned VM scratch (including raw private
network snapshots) was removed. Final checks confirmed no test process or
scratch remained and the same user service/PID and parent namespace persisted;
the exclusive VM lease was then released. The primary-PC network was never
used for effects. The installed VM runtime was observed only, not stopped,
restarted, replaced or exercised as the K1 writer.

Source checks at the same code head: netguard 195 passed /30 opt-in ignored;
four focused ordinary lifecycle tests passed; strict all-target netguard clippy
and formatting passed. The source suite passed 503 tests with two existing
skips, documentation navigation (93 links), native and QML checks. The full
current-RC Rust script passed, including workspace/TUI strict clippy and
terminal/parity checks; its repeated test-result lines total 1581 successful
test invocations, zero failures and 42 ignored (not a count of unique tests).
All five GitHub checks on that code head passed: test, package, package-arm64,
native-x86_64 and native-arm64. Documentation-only evidence changes after that
head do not claim another execution of modified code.

Ordinary wire/parser/direct-invocation tests are not kernel acceptance. These
disposable fixture results are mechanism evidence, not installed-runtime or
canonical production acceptance.

Remaining product gates include reviewed safe namespace API adoption and trusted
canonical launch; structural no-switch authority; nft subsystem continuity;
durable creator/orphan adjudication; power-cut/storage/boot matrices; installed
service/package/runtime exchange; core mark/DNS/firewall integration; and the
physical NIC/suspend/boot acceptance matrix in [KILL_SWITCH](../roadmap/KILL_SWITCH.md).
This is a development checkpoint, not product policy adoption, main/RC merge,
release publication, marketplace submission or completed K1.
