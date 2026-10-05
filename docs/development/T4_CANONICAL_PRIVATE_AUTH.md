# T4 canonical private authentication cut

Status: SOURCE implementation from exact `3671084292672c979ca0886641fa774919e5ae8d`
in separate `dev/t4-canonical-private-auth`. No new actor, credential/query/proc
backend, VM packet or guest action is selected. The original367 checkout and
its zero-BOOT provisional packet stay immutable. Its earlier b42/3c actual
canonical runs remain NONPASS; this cut borrows no canonical acceptance.

Owning contracts: [actor service](T4_MANAGER_ACTOR_SERVICE.md),
[canonical inventory](T4_STOPPED_INVENTORY_ACTOR.md),
[transaction ledger](T4_ACTOR_TRANSACTION_LEDGER.md),
[private backup/restore](../roadmap/PRIVATE_BACKUP_RESTORE.md) and
[execution policy](EXECUTION_POLICY.md). Legacy Bundle/StoppedOwner admission,
normal runtime, product backup/restore and classic actor64 are unchanged.

## Smallest real executable scenario

Only opt-in `--authenticate-canonical-backup` selects the new fixed supervisor
scenario. It seals public synthetic input before launch under the existing
whole15-second deadline, then uses the same original authenticated post-exec
private channel, child/pidfd, challenge/READY and exclusive fixed epoch:

1. ObserveStopped12 -> StoppedObserved13 requires the whole actual canonical
   manager/complete inventory operation. Its originals remain in that actor.
2. AuthenticateBackup8 consumes one original pending capability and the finite
   private input slot. Before receiving any private body, the same canonical
   owner performs a fresh retained-original pass; no PID1 or imported proof
   substitutes for it.
3. The unchanged v1 private header/body/authentication backend runs inside the
   original operation. Every input prefix and positive plaintext return remain
   in Transfer before fallible postchecks. A second retained-original pass
   after crypto must complete before BackupAuthenticated9 can be emitted.
4. A separately valid normal Halt5 releases only fully completed owners/input
   before Closed6 and original child0. Any post-READY operation Result error
   permanently revokes that context and parks Canonical, Transfer and channel
   together while alive.
   No second query/input operation, reply, Halt, reset or cleanup follows it.

The completed reply is historical private authentication under these fresh
developer observations, NOT reusable live authority, real StoppedOwner,
product Restore, StageRecorded, commit/rollback or a manager mutation.
No stage/member/journal/live-pair effect is activated by this cut.

## Same originals, strict fresh pass

The charged row owner allows a new ordered sweep ONLY from its fully Complete
state, with the exact original nonempty sorted PID set and no live scratch.
It resets only the private sweep cursor. Original row directory/image/class,
charges, expected set and ownership remain unchanged; no row/image acquisition,
replacement, skipped negative row or uncertain capacity recycling is permitted.
Each pass checks all rows in the original order, then exact final catalogue
equality and its final sampled gate. Wrong set, premature/repeated sweep,
missing row, late callback or scratch error seals the original owner forever.

Each actual refresh finishes three fixed unit queries first, using existing
child UID/GID and original stdoutEOF/WNOWAIT original0/wait0/strict parser/tool
checks. Only then does it freeze a fresh catalogue, preventing its own query
children from appearing as artificial churn. It checks current installed paths,
original manager/self text/image/namespaces/live pidfd, unrestricted proc and
fixed listener absence, then every held row and a final complete set.
All original equality, positive-start, daemon, visibility, cardinality and
churn predicates remain intact. Ordinary task churn may legitimately refuse.

The first fdinfo visibility File is retained and reused via offset0 reads.
Its current name/held identity is included in the existing named-binding loop
on each later pass; repeated boundaries do not open another fdinfo File. Row
text reads still use the existing charged scratch slot through the same original
PID directory, with positive-only release after full checks; uncertainty retains
that reported File. Queries retain their original Child/stdout/pidfd on failure.

An unchanged2-second/16MiB local budget clips every refresh to the SAME original
5-second authentication operation deadline. Pre/post crypto passes do not extend
that deadline, and whole15 remains unchanged. Guards cannot preempt a blocking
syscall/KDF. A slow pass or large/changing inventory refuses, not a skipped row,
weakened predicate or invented simultaneous snapshot.

## Finite role, query and diagnostic ledger

The same4096 original row directories plus4096 SameUid executable ceiling,
fixed120 and scratch8 remain within explicit canonical8320. Classic64 is not
raised. Reusing fdinfo changes the actual fixed Files upper bound from34 to33;
with manager pidfd1, actor base4 and active query stdout/pidfd2 this reports at
most40 fixed roles. The existing conservative41 bound remains safe. Private
authentication opens no new application File; no lower ledger is admitted here.
Std's hidden/partial spawn/null/pipe resources remain ordinary backend assumptions,
not a process-global all-FD custody or memory-limit proof.

One Transfer header16 and initialized MAX_BACKUP_BYTES+1024 input slot reserve
before READY, even in the canonical observation-only entry of this successor.
Positive authenticated bytes stay in Transfer before the post-call tick. Its
backend Argon2 fixed64MiB/3iterations/1lane, allocator/parser/CSPRNG internals and
temporary-error behavior retain their ordinary guarantees, not reported-resource
ownership or fatal/unwind survival. No transferred private byte/path/passphrase,
PID/count, raw error/errno or profile value enters output or argv.

Seven initial queries plus three pre-auth and three post-crypto queries =13;
each emits its existing ordered seven milestones. Initial observation59 frames
+two refreshes23 each+one authenticated label =106 positive actor frames.
At mostone existing first-inventory-refusal attempt across the SAME owner gives
conservative107; the attempt bit never resets between passes. Canonical vocabulary
is17existing+23refusals+two refresh labels+one auth label =43. Success contains
3735 literal bytes; adding the longest43-byte refusal conservatively gives3778,
within4096. The new packet must use this source-shaped byte bound rather than
claiming longest-label-times107 fits4096. Supervisor positive output is nine
frames (canonical seven plus existing before-transfer/authenticated two).

The two new actor labels are `t4_actor_before_canonical_refresh` and
`t4_actor_canonical_refresh_completed`; private authentication reuses existing
`t4_actor_backup_authenticated`. All share existing sampled output gates.
Refusal categories identify only an already refused suboperation, not its exact
cause. Missing diagnostic may mean an uninstrumented/expired/output cut. No new
probe or raw payload is logged. Lexical observer output never supplies ordering,
ownership, current authority or whole-run success.

## Deterministic gates and actual questions

Memory controls exercise repeated complete sweeps borrowing the same originals,
unchanged charges, all negative rows, wrong sets/incomplete/late/reentry refusal,
SameUid image retention and late scratch prefix with no replacement, retained
prefixes with no downstream backend/finish, one-shot auth sequencing,
same-context kind/nonce/sequence-bound observe/auth/Halt, genuine v1 public input
with pre-input/header/post-crypto failures and positive plaintext retention.
They do not run canonical query/proc/credential/actor backends.

SOURCE gate checkpoint, 2026-10-05: canonical/inventory29, service/protocol/
transfer/stage/ledger41 and legacy capture7 controls PASS; feature all-target
Clippy with warnings denied, normal non-feature library check, whole-workspace
format check and diff-whitespace check PASS. Builds use HOME-only
`t4-canonical-auth-source-build`; the actor executable was not invoked.
The formatter fix sorts only the existing feature-gated module declaration in
`lib.rs`, correcting the inherited #667 CI failure in this successor without
rewriting367. Earlier SOURCE failures remain failures: inherited format ordering,
Clippy's constant-test assertion, and the new memory test's Debug-requiring
assertion were corrected narrowly before the fresh passing checks. None was
an uncertain actor/native/VM operation or canonical acceptance.

Before actual selection, ROOT and independent review must read this new refresh/
private operation boundary, exact source/artifact, complete finite packet and
pre-scoped two-capture vocabulary/107/4096 policy. Actual required result is
original whole0 with genuine canonical observation, both original refreshes,
private auth, normal Halt and original child0. A matching phase prefix alone is
not PASS. No actual result or new packet is supplied here yet.

After this cut, the next functional composition is canonical-fenced lower
stage/journal using actual owner-derived admission, not the old17-PID1 scalar.
Its complete effect/FD/query ledger requires separate review. Real product
Restore additionally needs installed invocation/broker, original migration/
singleton/owner/generation/Off-idle/core-TUN and private live-pair boundaries.
Canonical developer authentication does not construct or export those proofs,
and cannot fabricate legacy StoppedOwner or enable crash recovery/adoption.
