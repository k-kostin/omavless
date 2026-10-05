# T4 same-original canonical synthetic Stage

Status: developer-only SOURCE successor from exact
`44acc6743389141ed9d7e408f68584dfc04297e9`, in separate
`dev/t4-canonical-synthetic-stage`. No actor, canonical query/proc/credential
backend, storage operation, packet or VM action is selected. The auth checkpoint
and its previous canonical NONPASS scopes remain immutable. This implementation
does not borrow actual canonical acceptance or fabricate product authority.

Owning contracts: [canonical auth](T4_CANONICAL_PRIVATE_AUTH.md),
[canonical inventory](T4_STOPPED_INVENTORY_ACTOR.md),
[actor service](T4_MANAGER_ACTOR_SERVICE.md),
[transaction ledger](T4_ACTOR_TRANSACTION_LEDGER.md),
[private restore](../roadmap/PRIVATE_BACKUP_RESTORE.md) and
[execution policy](EXECUTION_POLICY.md). Default runtime/package/CLI activation,
classic64/17-FD admission and legacy Bundle/StoppedOwner are unchanged.

## Executable developer cut

Only the feature-gated actor binary's explicit
`--stage-canonical-synthetic-backup` selects this supervisor scenario:

1. Same original authenticated private channel/READY/nonce/child/pidfd completes
   real canonical ObserveStopped12 -> StoppedObserved13.
2. AuthenticateBackup8 -> BackupAuthenticated9 keeps the original private input
   and positive plaintext, with both same-original full refreshes from44acc.
3. StageAuthenticatedBackup10 -> StageRecorded11 consumes those already held
   authenticated bytes in the canonical entry. It receives no second body or
   caller FD/proof. Classic entry's kind10 inline-transfer semantics stay intact.
   Restore-store-off normalization is one-shot; its positive return is retained
   before its postcheck and throughout Stage failure.
4. A separately valid normal Halt5 -> Closed6 and original child0 finish only
   completed owners. Persistent synthetic transaction names remain after Halt.

The new entry reserves the entire lower36-role ledger, metadata slots and8KiB
directory buffer before READY, alongside Canonical and Transfer. The private
StageOwner enum borrows the actual Canonical, never a client count or fake17-FD
Retained. Canonical admission consumes before checks, tracing or acquisition;
it requires completed authentication, real manager/pidfd/visibility originals,
no pending query, and the Complete exact original-row/image owner without scratch.
No descriptor, process identity or reusable authority leaves that operation.

## Exact fences, complete storage checks and refusal

The existing fixed Stage/FileIo graph remains the writer. Four new directories
and nine files use original nofollow directory-relative opens and exclusive
creates, root/safe-mask and exact700/600/single/noattrs checks. Every returned
File is in its pre-reserved slot before subsequent gates or validations.
Each write/read continuation, metadata/attribute call, mkdir/fsync and directory
next keeps its existing sampled pre/post gates. Current named/held bindings,
full bytes/digests and strict dots/member/EOF catalogues remain unchanged.
RawDir borrows the original File and fixed buffer; no duplicate iterator FD.

Positive record has28 owner fences: admission1, four mkdir pre/post8, nine
member write pre/post18, final1. Its two full canonical refreshes occur at
admission and the LAST fence after ALL file/hash/catalogue/hierarchy/journal
verification, before Stage completion/output/reply. Each performs three real
unit queries with original EOF/WNOWAIT0/wait0/parser/tool requirements before
its fresh PID sets; every original row/image and exact final set is rechecked.
The other26 fences use the same retained installed/self/manager/namespaces,
proc-visibility/pidfd/current bindings and original listener text. They do not
query, open a new row/image or reuse copied facts. Exact26 is enforced before
final refresh; missing/extra/repeated fence refuses permanently.

The private lower admission is distinct from unchanged classic17/64. Stage
and lower consume before effects; after consumption only normal Halt following
completion is admitted. Any other request is rejected BEFORE its match branch.
All early admission, origin, storage, final-refresh or output errors revoke
context/Canonical/Stage/lower; the actor parks those owners together with its
private Transfer and channel. No retry, later query/operation/Halt, error cleanup,
capacity recycling or guessed persistent-state adoption follows it. Reported
prefixes remain while alive; panic/fatal and hidden backend partial resources
remain unavailable, not an all-FD/heap custody guarantee.

Stage's diagnostic notes only the first already returned original Err. The
outer record seals both owner and lower ledger BEFORE its one Stage error-label
attempt. Nested categories do not replace that first choice; failed output
preserves the original Err and cannot emit another label. Existing inventory
first-refusal may precede the Stage refusal. Uninstrumented/expired/output cuts
need not produce a refusal label; absence proves no different cause.

## Finite resource, budget and output ledger

Canonical8320 remains explicit:4096 row directories +4096 SameUid images +
fixed120 +scratch8. Conservative canonicalfixed41 plus full lower36 =77 <=120;
worst reported bound8277 <=8320, active lower16 bound8257. Slots are reserved
before READY; lower and canonical scratch are disjoint. No unknown owner is
closed to make room, and the classic limit remains64.

ONE Stage5-second deadline clips every fence and storage continuation. Full
refreshes retain2-second/16MiB local caps clipped to that same deadline; the26
origin passes preserve original bounded reads and never extend the operation.
Large/changing inventories, fsync or blocking backends may legitimately refuse.
No worst-case timing success, syscall preemption or simultaneous snapshot is
claimed. Supervisor whole15 seconds is unchanged. Complete observe/auth/Stage
uses19 queries: initial7 +auth6 +Stage6, not three queries per storage fence.

Only Stage full refreshes choose a fixed internal silent Trace mode. QueryProgress
still advances the same seven ordered milestones and both sampled gates;
credential/tool/child/EOF/original-zero/wait/parse predicates are identical.
No input selects this mode. Trace failure still refuses. Instead the new Stage
owner-admitted/owner-checked milestones bracket those complete refreshes.

| Actor trace | Frames | Bytes |
| --- | ---: | ---: |
| Unchanged observe/auth positive |106|3735|
| Six existing Stage literals |6|207|
| Two Stage owner literals |2|59|
| Positive total |114|4001|
| Conservative inventory +Stage first-refusal addition |2|43+37|
| Complete finite bound |116|4081 <=4096|

Vocabulary is59 literals (auth43 +Stage6 +owner2 +refusal8), not116 choices.
Stage refusal suffixes: admission, origin, directory, member_write,
member_verify, catalogue, journal, final_owner; prefix `t4_actor_stage_`,
suffix `_refused`/LF. Supervisor positive has11 frames. No raw PID/count,
metadata, private bytes/path/passphrase, errno or traceback is output.
Source-shaped sequence bytes, not longest-label-times116, prove the4096 bound.
Lexical file projections do not prove ordering, custody, authority or whole0.

## Scope, gates and next actual

Memory controls cover exact private fence cadence/premature final/reentry,
same complete owner without charge/replacement, silent query progress and
expiry, nested inventory/Stage errors, one attempted error output, empty-origin
refusal before IO, all request/reply/output prefixes and permanent Halt denial,
positive plaintext/normalized-store retention on failed Stage, and finite
resource/frame/byte/catalogue plans. They do not exercise actual File storage,
canonical query/proc/credential backends or the actor service.

Exact final source/compile gates and head are recorded at the checkpoint below.
PRIMARY and independent FULL affected source review, compiled artifact identity,
fresh fixed packet/boot and pre-scoped diagnostics remain required before ROOT
alone selects the real developer whole scenario. Required outcome is original
whole0, complete original Stage checks, normal Halt and original child0, not a
marker, old synthetic result or successful compiler invocation.

SOURCE checkpoint, 2026-10-06: canonical/inventory34, service/protocol/transfer/
stage/ledger/default-absence48 and unchanged legacy capture7 controls PASS.
Feature all-target Clippy with warnings denied, normal non-feature library
check, whole-workspace format and diff-whitespace checks PASS. Compile-only
actor build PASS; the binary was not executed. All compilation/pure gates use
separate HOME-only `t4-canonical-stage-source-build.6uUosgPr`, its700 temporary
directory and at most four Cargo jobs. No canonical query/proc/credential or
storage backend, old capture/scope or guest operation was selected. These gates
do not supply actual origin, Stage completion, custody or product acceptance.

The only tree is the existing fixed synthetic `authenticated-transaction`
inside the one administrator epoch. Its old pair is public fixture data, its
intent/Abort terminal uses fixturegeneration1 and nonce-derived transaction ID.
No product live pair is replaced. Persistent files are not automatically retired;
old five-member cleanup recipes are invalid. Recovery/cleanup requires its own
positive or uncertain-effect reconciliation and full review.

Canonical observations are not exclusive product authority. Inactive units can
change between queries; per-effect original checks do not supply missing
installed invocation/broker/singleton/migration/owner/generation/Off-idle/core-TUN
or private live-pair rights. This cut supports only the existing trusted-admin
synthetic fixture, not product Restore/Commit, rollback, crash recovery or release.
