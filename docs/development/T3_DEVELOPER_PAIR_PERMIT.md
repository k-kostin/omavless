# T3 opt-in developer pair

This successor builds on `68cdd9dfa17f7014c9c1c28d90a7fbf5901b0dca`
(#590). It reuses the existing actual-owner capture, opaque selection,
confirmation, shared scheduler, durable per-chunk lease and exact replay path.
It does not add a second coordinator or enabled UI action. The successor below
adds an explicitly opt-in development IPC workspace; default/product methods
and permits remain unchanged.
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

## Same-head positive and restoration checkpoint

Tested source: `faad53b7350297d5bdba8e2fcdfdb783d603dc25`. The successor
adds one ignored test and a test-only mount dependency; production behavior is
unchanged. The full feature-enabled library run returned original zero:
1109 passed, zero failed, 35 ignored. Focused candidate tests (30 passed,
two ignored), all-target strict feature Clippy, default library check, format
and diff checks passed. The separately compiled release test image was
21,439,224 bytes, SHA-256
`adb773240536f78dac8875be285292ca9a2190af5a9420c5ae69df4e9571718b`.

The positive selected-close scenario was repeated on this exact image in a
fresh loopback-only PID/mount/network namespace, dropping to UID/GID 1000.
Original run and observer both returned zero. Its complete one-test success
grammar was 225 bytes, SHA-256
`8df7786d30808be27bb17ce5de3026c45b29c4cdb0578b286fb8a65243b19cbf`,
with empty stderr. It again asserted one selected close, one surviving tunnel,
unchanged desired state and exact receipt replay.

The new ignored `actual_owner_developer_pair_rebind_restoration_in_dev_vm`
ran as root/PID1 in a **separate fresh, non-propagating mount/PID/network
namespace**. The reviewed administrator packet read and hash-checked the
original public pair first, made mounts recursively private, then overlaid the
fixed pair path with a namespace-only tmpfs copy. A distinct same-byte broker
and exclusive root marker were created only in that copy. The original global
pair, old process scopes, private profiles, services, DNS, TUN and routes were
not modified. Root/PID1/marker predicates alone do not establish isolation;
the separately reviewed original launch is part of this evidence boundary.

After actual coordinator capture and confirmation preparation, the test
bind-mounted the distinct broker over the copied original. The actual Session
observed the changed object and revoked its evidence. Successful unmount then
restored the original full identity tuple, including ctime/mtime; the old
Session nevertheless remained refused. Confirmation and its exact retry
returned `RefusedBeforeWrite`, revision stayed zero, desired state was unchanged,
and **both real tunnels still echoed**. This is not merely confirmation expiry.
Original whole run and observer returned zero. Complete success grammar was
229 bytes, SHA-256
`e7a90517fbe4ee5171d8c42996f9a904bc06c28b2694ecae8232f053605f036c`,
with empty stderr. Five inert packet controls passed before execution.

Disposable namespace-init teardown has the same containment/not-product-
recovery limitation described above. The actual checkpoint proves one broker
replacement/restoration case, not every possible pair drift or fatal resource
loss. Released-pair adoption, ARM64 and installed-product acceptance remain
pending. Default builds still do not grant this developer permit; public-method
and UI activation are not part of these checkpoints.

## Development semantic socket workspace

With the non-default `developer-conditional-close` feature, the existing
same-UID runtime socket provides four fixed methods:
`development.connections.snapshot`, `.prepare`, `.confirm`, and `.receipt`.
They are absent from default builds. Advertising these methods describes API
availability, not current pair authority or a supported production feature.

Every request names the actual runtime instance. Snapshot returns only bounded
private display rows and opaque handles, never controller IDs or close tokens.
Prepare returns a single-use ticket; confirm also requires an operation ID and
expected revision. All field sets are exact. Tokens are canonical nonzero
256-bit lowercase hexadecimal values. There are no caller-selected commands,
paths, services, privileged transports, or passive-attestation adoption.

The registered owner uses its existing coordinator and original capture.
Controller discovery and full pair proof run outside the owner mutex and
migration lease; retention checks the same capture identity, cancellation,
durable context and original expiry. Unsupported lifecycle hosts cannot mint
the opaque observation. Confirmation uses the existing detached effect worker.
Exact replay precedes new authority checks and cannot resend a controller POST.
Receipt polling distinguishes missing, pending, and terminal results, including
Unknown, without constructing another effect request.

Focused socket tests prove strict validation, unsupported-host refusal,
default-build absence, successful selected close and exact receipt replay after
a lost client response/expired confirmation. A dropped controller response
remains Unknown; replay sends no second POST and leaves desired VPN state
unchanged. The socket fixture transfers its original coordinator into the real
same-UID server using a concrete test-and-feature-only construction seam. This
synthetic-permit seam does not attest ordinary installed startup or cleanup.

An additional opt-in VM test uses the already owned passive host and exact
root-provisioned developer pair, with no synthetic effect permit. It exercises
snapshot, preparation, confirmation, receipt and replay through the real socket,
then checks one selected tunnel terminates while the other still echoes. Its
environment switch is `OMAVLESS_CLOSE_DEVELOPER_SOCKET_VM=1`; run it only inside
the explicitly admitted disposable development namespace. The prior
direct-coordinator VM receipts above do **not** cover this new socket test.
The subsequent socket checkpoint below covers this successor's VM gate;
UI integration remains a separate gate.
The opt-in [development TUI client candidate](T3_DEVELOPER_CLOSE_CLIENT.md)
adds a distinct explicit client mode on this socket contract; its synthetic
checks do not extend the historical VM receipt to installed UI acceptance.
No release package, TUI/QML action, main/RC merge or production activation is
supplied by this workspace.

## Exact socket checkpoint, 2026-10-06

Tested implementation: `0e812f34a01341d3bb093c38fc2be85f6d0318ea`.
The final feature-enabled library run returned original zero: 1119 passed,
zero failed, 36 ignored. Focused connection-close tests (27 passed, three
ignored), strict all-target feature and default Clippy, default-method absence,
format and diff checks passed. The ignored cases remain separately selected
integration gates, not ordinary test coverage.

The separately compiled release test image was 21,485,776 bytes, SHA-256
`044b4e3e75efb0d18a241ad75f3341eeb1b18bd9586ceea62ba8488410f0c1e4`.
Six pure delivery controls and primary/independent boundary reviews preceded
the actual selections. Public upload returned original zero. In a fresh
x86_64 Omarchy dev-VM boot, the normal application was confirmed disconnected
and its user service stopped, with both fixed units inactive and no TUN.
The test then ran in a separate PID/mount/network namespace with loopback only,
dropping supplementary groups and UID/GID to 1000 before execution.

The actual `actual_owner_developer_pair_socket_selected_close_in_dev_vm` run
returned original zero. A separately selected two-file observer also returned
zero with the complete exact one-test success grammar: 232 stdout bytes,
SHA-256 `e85ce6910159a484c44a43e93433d6d368a0c375a0f11ad0a42bf7f66b7b7916`,
and empty stderr. The same actual coordinator and root-provisioned developer
pair were used without a fixture permit. The socket scenario checked snapshot,
prepare, confirmation, receipt polling and exact replay, one selected tunnel
termination, one surviving echo tunnel, and unchanged desired state.
This closes that scoped developer-socket VM gate, not UI or product adoption.
Namespace-init teardown is containment, not retained-child reaping or product
recovery proof; the ordinary shared filesystem/sysfs boundary is unchanged.

The first exact-head Test and x86_64 package CI jobs never obtained a hosted
runner and were cancelled before any steps. GitHub's check annotation explicitly
reported runner acquisition failure; this is neither a passing CI gate nor a
source test failure. ARM64 packaging passed. Subsequent CI results must be
recorded independently, without borrowing local or VM success.
