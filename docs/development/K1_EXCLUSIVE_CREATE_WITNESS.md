# K1 private exclusive-create/readback extraction

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

Three new pure controls cover complete-vs-prefix/wrong-batch ACKs,
generation refusal/poison and classification without readback metadata. These
exercise existing wire parsing on synthetic bytes only; they do not exercise
the effect/readback adapter or provide dedicated negative lifetime/non-Copy
compiler controls.

## Exact developer checkpoint

At source `83f42593e28a6f16814a18c84fd16969d101cccc`, a separately reviewed,
fresh-target, offline/locked `omavless-netguard` library test compilation and
the three exact pure filters completed with original terminal zero. ROOT's
selection `8191c5` completed as `4ce52f`; the admitted test binary SHA-256 was
`9acfc87d5fd1209a0465c4ff22390f20b008896c8d3b1bcda94740831c380b30`.
Eight separate mocked recipe controls also passed (`91caa4`, checked by
`129112`). The compiler recipe explicitly trusts ordinary developer compiler,
linker, standard library and offline cache; this is not full toolchain
attestation or production artifact provenance.

Only those three pure controls ran. No ignored/native creator, actual
effect/readback adapter, VM or firewall operation was selected for this
extraction. Compilation establishes that the reached test graph type-checks;
it does not replace negative borrow/type controls or demonstrate kernel
readback, durable ownership, canonical authority or whole K1 acceptance.

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
