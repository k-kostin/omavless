# K1 actual effect-reply receive truncation

This test-only slice starts at sealed creator/private-writer research
[#560](https://github.com/k-kostin/omavless/pull/560), exact source
`fc10fb39aa2ceb25bce251fa63ba55026ce88f33`. It preserves that branch and its
[evidence](K1_CREATOR_LIFECYCLE_RESEARCH.md). No production EffectPort,
namespace authority, service, installed path or dependency is added.

## Fault and barrier

The private `cfg(test)` one-shot fault binds the existing retained creator
socket. After its actual fixed batch send, the next successful real `recvmsg`
uses a **one-byte iovec**. EAGAIN does not consume the fault. The receive must
actually return one copied byte and `MSG_TRUNC`; otherwise the scenario fails.
No input MSG_TRUNC/PEEK flag, invented errno, synthetic ACK, altered sender or
fabricated receive flags are used. The exact actual byte, sender and flags then
reach the existing ACK collector, which refuses and permanently poisons.

Create, atomic full-policy replacement and same-inventory generation-conditioned
delete each get a fresh loopback-only disposable namespace. The ordinary delete
`consume()` interface remains unchanged; a private test-only receiver path
supplies this fault. There is exactly one additional attempted mutation in each
scenario, no resend, generation refresh, compensation or socket-reopened effect.
The original socket descriptor/inode/port and namespace remain retained through
the fault; the creator loses live causality and its session is poisoned.

Real LockedState persistence remains PendingCreate, PendingReplace retaining
the old handle, or PendingDelete retaining the old handle and durable Closed
generation respectively. Repeated Status/Arm/Disarm on the retained writer,
reopened private writer with the same creator, and then reopened writer with a
new creator all refuse with zero further effects and unchanged record bytes.
An independent **read-only** socket observes actual policy presence (create/
replace) or absence (delete). It verifies full rules and changed replacement
handle, but neither presence nor absence repairs a receipt or authorizes retry.

This is actual **receive truncation after send**, not raw kernel ACK dropping,
loss specifically of END, timeout, network packet loss, or #560's separate
post-ACK/readback adapter-result loss. In particular, no valid commit ACK is
inferred from independent readback. Pure tests separately require permanent
collector refusal for MSG_TRUNC, MSG_CTRUNC and both flags for all three effects.

## Gates

The opt-in gate is not run until an exclusive development-VM lease and frozen
artifact are verified. Ordinary parser checks are not actual kernel evidence.

```sh
OMAVLESS_K1_RECV_TRUNC_VM=1 frozen-netguard-tests --ignored --exact \
  kernel_observer::creator_lifecycle::tests::real_receive_truncation_in_disposable_vm \
  --nocapture
```

At the first source checkpoint, the actual VM gate is pending. Evidence will
identify the immutable code and executable hashes, checks and cleanup separately.
Synthetic epoch/Canonical fixture vocabulary remains untrusted local history,
not authentication of the canonical host. Reviewed safe namespace API adoption,
trusted launch/no-switch authority, nft continuity, durable orphan adjudication,
installed service/package/runtime integration, power-cut/boot and physical-host
acceptance remain open under [KILL_SWITCH](../roadmap/KILL_SWITCH.md).
No main/RC merge, release, marketplace or upstream submission is implied.
