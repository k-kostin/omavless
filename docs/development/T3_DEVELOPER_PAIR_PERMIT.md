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

No VM execution, replacement/restoration matrix or product acceptance is
claimed at this initial source checkpoint. Release pair adoption, ARM64,
public-method/UI activation and independently applicable acceptance remain
separate requirements.
