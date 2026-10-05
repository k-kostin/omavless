# K1 private exclusive-create/readback extraction — uncompiled

This successor to #656's read-only checkpoint is restricted to `cfg(test)`.
It adds no production EffectPort, adapter mutation method, process, descriptor
opener, protocol, dependency or caller in the stage6 executable. The stage6
`TABLE_ABSENT` receipt is not permission to use its namespace for effects.

The existing `FixtureCreator::full(None)` still requires its durable
`pending_create` barrier under the original state lock. It then moves the
existing complete absent `LocalInventoryLease` into a private non-Copy
`PreparedCreate`. That value retains the original mutable session borrow,
generation and deadline, shortened rather than renewed by the caller's bound.
The existing `BatchSender` performs the existing single atomic send and strict
bounded receive. A new return seam preserves the actual `AtomicReplies` rather
than discarding it as unit; existing non-create callers still discard it.

The extraction requires `AllAcknowledged`, including END, and equality with the
exact fixed exclusive-create batch at the original generation/sequence. It
does not accept a copied success boolean, generation refusal, prefix replies,
replacement batch or generation-zero condition. This is structural validation,
not authentication of an arbitrary callback: the only effect caller is the
existing trusted test sender on the supplied original session. That private
callback is not a production extension point.

Only then does the same still-borrowed session run the existing complete
inventory parser under the unchanged deadline. No second parser or supplied
readback tuple is added. Only full exact FullVpn readback with a nonzero table
handle yields the non-Copy `CreatedLease`; final recheck consumes that lease.
An error or unwind during send/readback poisons the original session in memory;
dropping an unfinished created lease does likewise, without I/O or close.
The existing fixture's loss/cut controls, durable Pending and failure barrier
remain. Replacement and deletion are not redirected through this extraction.

The consumed handle still feeds only the pre-existing synthetic-epoch fixture
identity. It is not durable canonical ownership, a transferable effect receipt,
namespace/no-switch proof, nft subsystem continuity or an installed provider.
Matching handles, owner ports and `ExactUntrusted` classes alone cannot invoke
the constructor or readback path.

Three new pure controls are prepared for complete-vs-prefix/wrong-batch ACKs,
generation refusal/poison and classification without readback metadata. These
exercise existing wire parsing on synthetic bytes only; they do not exercise
the effect/readback adapter or claim non-Copy compiler evidence. No tests,
compiler, native fixture, VM or firewall operation has been selected for this
new source. Compile/type and materially affected cfg(test) regressions require
a separately reviewed ROOT recipe and fresh targets.

## Remaining actual-owner integration seam

The current exported ActualCreator remains read-only. No mutation method is
added to it: first integrating this core requires a separately reviewed durable
Pending owner and explicit disposable namespace effect scope. A future
integration must retain the same ActualCreator/session through complete lease,
original `LaunchBorrow` verification and final lease recheck before consuming a
witness; a handle returned early cannot substitute for that borrow. The
existing #641 parser and #639 authority composition should be reused, not
duplicated or bypassed. Canonical-host authority and an actual effect-capable
constructor remain unavailable.
