# K1 inactive kernel-effect boundary

The shared-lock context previously used the older coordinator's `KernelPort`,
whose contract requires the kernel adapter to persist an independent receipt
before returning success. That is incompatible with making `LockedState` the
single marker/receipt writer: a future adapter could reacquire its held lock or
create a second receipt authority.

`EffectPort` now owns only kernel observation and conditional effects.
`LockedState` alone publishes Pending, updates the marker and publishes the
terminal receipt. The older coordinator's contract is preserved for its own
synthetic callers. There is no blanket conversion, production implementation,
socket, nft invocation, service, packaging or activation.

The new snapshot and identity vocabulary also stays separate from the older
receipt-backed vocabulary. `EffectIdentity` is a modeled input proof obligation,
not an attestation minted by this module. Copying its fields, reading a receipt,
matching a handle/name/policy or acquiring an orphan cannot prove ownership.
The future adapter must retain independently authenticated namespace and socket
lifetime throughout a request, use exclusive/conditional kernel operations and
provide complete verified readback. This change supplies none of that authority.

Existing shared-lock fault tests now exercise the effect-only seam. The synthetic
kernel observes that Pending is durable during every effect and that the store
lock cannot be reacquired. Added assertions verify that kernel success leaves
the pending receipt untouched until the caller commits the terminal phase.
Lost kernel acknowledgement still leaves Pending and requires recovery; it
cannot be upgraded to durable product success. Old coordinator tests remain
unchanged. These are deterministic composition checks, not kernel or host
acceptance. Persistent orphan provenance, canonical namespace/subsystem
continuity and the full K1 host matrix remain open.
