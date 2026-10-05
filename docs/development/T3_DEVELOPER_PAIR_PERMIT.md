# T3 opt-in developer pair

This successor builds on `68cdd9dfa17f7014c9c1c28d90a7fbf5901b0dca`
(#590). It reuses the existing actual-owner capture, opaque selection,
confirmation, shared scheduler, durable per-chunk lease and exact replay path.
It does not add a second coordinator, public IPC method or enabled UI action.
Default builds still return `MissingAttestation` for passive core bytes/ABI.

## Distinct authority boundary

The non-default Cargo feature `developer-conditional-close` admits one fixed
x86_64 developer pair under `/var/lib/omavless-close-development-pair`.
The separate strict receipt schema is
`omavless-developer-conditional-pair-v1`; it rejects unknown/duplicate fields,
foreign source/hash/architecture/ABI and `production_adoption: true`.
There is no input-selected path, installer, repair, privileged command or grant.
The older research receipt is unchanged and cannot become this permit.

The admitted administrator-provisioned pair is:

- Core source `8c038e76c8407eebd7afdd6e0389fc2bbc28cab9`, SHA-256
  `3b1da75d3c9fd8440216f9c256c6c59da812faae88debc936f3c72fef9724544`.
- Broker source `aff0c38075338d51d979acc9f10dab1ae6dbba6f`, SHA-256
  `ea958302d745b901294df6164c624a431a7493b67457a255306ec8216545eb9d`.
- Whole composition source `12b0253564f25f918af18a0c0ddc6f2e2231b2db`,
  capsule `06507e5cc4777c4a6b88be1b16e37d623663eeaf2b8600a0ee72a38af6d3271d`,
  original-zero capture
  `f174aedb11ddef42f2a12d96bf872ecb93d218835e43e3a611f6588e232cab23`.

These are exact developer policy pins, not independently reconstructed build
provenance or released-package adoption. Holding the broker object does not
prove that a broker invocation is running. The whole composition evidence
belongs to its recorded source/artifacts; it is not acceptance of this adapter.
ARM64 and installed-product acceptance are not inherited from it.

## Original owner and revocation

Admission retains the original root-owned directory chain, receipt, core and
broker descriptors. The actual core image and source must equal the same held
core object. Evidence binds one original Session Arc; observed drift latches
refusal permanently. Returning names/bytes cannot revive that evidence.
The permit has no Clone, serialization or wire constructor.

Initial hashing runs outside the owner mutex. New object rechecks use the
existing counted `ProofFlight`, outside the urgent lifetime-revocation gate.
Confirmation performs only the original fast lifetime check while holding its
scheduler lease. The worker must recheck the developer objects and actual
durable owner before every effect chunk and definitive finish, then re-lock
and validate reservation, cancellation and original confirmation expiry before
its nonblocking write. No new filesystem traversal holds the revoke gate.
Slow filesystem calls are not hard-preempted; revocation may return the existing
bounded busy result while the exact proof flight drains. No lease is held
across controller waits, callbacks or owner work.

## Verification boundary

Ordinary tests cover strict receipt pins and sticky session binding, existing
object-identity primitives, normal feature-enabled passive-path refusal,
confirmation expiry, cancellation, partial writes and exact Unknown replay.
They do not provision root objects or install/start a service.

The ignored `actual_owner_developer_pair_selected_close_in_dev_vm` is a
separate opt-in VM scenario. It uses the same two real loopback tunnels and
actual Rust coordinator as the existing composed-core test, but removes the
test fixture permit. Only this new developer evidence may schedule its close.
One selected tunnel must terminate, the other still echo, the exact receipt
must replay, and desired connection state must remain unchanged. Root object
provisioning and mutations belong to separately reviewed VM administration,
not to the test or normal runtime.

## Exact-head developer VM checkpoint, 2026-10-05

Tested implementation: `839f35c9e1747569b0212da6950384dfb87fe3fd`.
Feature-enabled library gate: 1109 passed, zero failed, 34 ignored; focused
candidate gate: 30 passed, zero failed, two ignored. Feature all-target Clippy
with warnings denied, default library check, formatter and three static boundary
controls passed. Ignored tests were not counted as ordinary integration evidence.

Separately compiled release test image: 21,364,272 bytes, SHA-256
`a1a47f9ff995d34ad2b7918c36a7bf31f804feabe490437e0c213209949093cb`.
The reviewed fixed administrator provision returned original zero and created
only the pinned root developer objects above; it did not launch a service/core.

The initial unisolated test returned 101 at the unchanged fixture-adoption
observation guard. A separately scoped read-only inventory found five global
Mihomo processes and no TUN. This is not a successful developer-pair test or
permission to stop/adopt any old scope. The actual pair test had not been reached.

A fresh isolated x86_64 Omarchy dev-VM scenario then used genuine new PID,
mount and network namespaces, loopback only, and dropped to UID/GID 1000 before
running the exact ignored test. Original whole run returned zero. Its separately
selected two-file observer returned zero with the complete exact one-test
success grammar and empty stderr. Stdout was 225 bytes, SHA-256
`1a1b330c58cc0ce5792fb0effaf9f24a3ad17596c10b4709fe9bebebcf56f57e`;
stderr was empty. The actual test asserted selected closure, one surviving echo
tunnel, unchanged desired state, exact receipt replay and absence of a fixture
permit. Neither the physical PC nor old VM process scopes were changed.

The test becomes namespace init: its exit lets the kernel tear down remaining
children in that fresh disposable namespace. This is containment, not
unknown-child custody or product recovery acceptance. Ordinary filesystem and
the existing sysfs mount persist; no TUN was configured. Reviewed transport and
finite receipts remain local development evidence, not package provenance.

The replacement/restoration matrix, released-pair adoption, ARM64 and installed
product acceptance remain pending. Default builds still do not grant this
developer permit; public-method/UI activation is not part of this checkpoint.
