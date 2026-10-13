# Restore 0.9.9: PC continuation

Owner-requested shutdown handoff, 2026-10-12. Continue PR #739,
`dev/099-restore-integration` targeting `beta/0.9.9`. Do not publish, merge
main/RC, or mark Restore accepted from this checkpoint.

## Outcome and remaining gate

Restore recovery is **not yet accepted**. The installed optimized candidate
`bd83fce19cea357dd1cb9da114a16d8c0ab50b49` earned real Backup, synthetic import,
Intent pause and normal Stop. Fresh OLD recovery refused during process
inventory, before its Stop/effects; its original Intent and staged pair remain.
No successful installed recovery-to-ordinary-availability result was earned.

The refusal was narrowed to normal nondumpable systemd/PAM processes and an
unrelated global PID-set stability requirement. This successor replaces only
the recovery inventory boundary with pinned fixed-unit/cgroup quiescence.
The historical Abort path remains unchanged. Read
[the recovery contract](RESTORE_099_RECOVERY_PLAN.md) before execution.

Primary and independent source review accepted the cooperative boundary:
original manager peer/identity, original singleton and migration locks, fixed
root-owned unit without drop-ins, exact cgroup2 hierarchy, and recursive empty
cgroup or verified absence. This does not exclude malicious same-user actors.
Populated, substituted, unreadable or mismatched state refuses. No OS security
relaxation, process-name exception or privileged helper was introduced.

This checkpoint also wires exact commands for OLD, mixed and committed NEW:

```
restore recover-old --confirm-rollback
restore recover-mixed --confirm-rollback
restore recover-committed --confirm-completion
```

All use bounded private stdin; never put credentials in argv. Backup-only
product builds must continue denying all Restore execution. Typed OLD/NEW
release grants stay distinct; ordinary disconnected startup follows release.

## Next executable steps

1. Fetch, read current policy/contract and inspect this PR's latest test comment.
   Review the final CLI dispatch delta as well as the cgroup implementation.
2. Run focused `recovery_cgroup`, `recovery_manager`, `old_recovery`, historical
   stopped-owner and native recovery tests, then meaningful combined gates.
   Use one shared Cargo target directory and bounded build jobs.
3. Build an **optimized release** private candidate with
   `t4-manager-actor-service`, without `product-private-backup`. Package from a
   clean exact commit; verify installed binary/build identity and DNS/core pair.
   Debug builds previously exceeded activation timing; do not reuse that recipe.
4. On the PC's disposable VM, create its own fresh normal account/fixture and
   earn Backup → import → Intent pause → Stop → fresh OLD recovery → ordinary
   disconnected runtime. The ARM fixture is not transferable proof or data.
   Review/adapt the fixed account and evidence paths in
   `tests/restore_host/old_recovery.py`; its existing paths deliberately refuse
   reuse. Set the normal routing preset before Backup: an empty initialization
   preset was an independently diagnosed payload refusal.
5. Earn real installed NEW and mixed loss-of-owner cases through reviewed
   producer cuts, not fabricated receipts or hand-edited private records.
   Source tests cover NEW completion and mixed rollback/completion, not these
   installed scenarios. The current developer producer pauses only at Intent;
   additional cut support still needs implementation/review.
6. Implement/review fresh partial-retirement/disposition recovery if required
   by the supported contract. It is **not implemented** by the new CLI commands.
   Classify exact ordered prefixes, retain fresh owned authority, authenticate
   the selected pair and terminal records, then verify durable readback.
7. Cover wrong authentication, busy owner, foreign/substituted state, repeat
   attempts, loss cuts and ordinary-start failure. Do not rerun uncertain effects,
   delete fences to make startup pass, or equate source tests with host acceptance.

## Retained ARM state

The shutdown VM retains three isolated fixture accounts. Two earlier failed
activation/Backup scopes are not cleanup targets. The latest scope has the
original Intent/staged pair and private archive/request intact; subsequent
explicit recovery attempts refused before mutation. It is inactive with PID 0.
Original owner runtime was stopped before testing; no VPN connection or DNS
change was performed during this recovery session. Do not assume the installed
private test candidate is the ordinary product build after reboot.

## October 12 PC successor selection

The PC writer has taken the explicitly stopped branch. Its first case binds the
fixed fresh `ovrestore201` account and distinct `restore099-pc-positive` evidence
root. Do not select `recover-retained` for this first case. Source fixture
preflight requires at least 256 available descriptor-limit slots; this is not
low-limit admission acceptance or a replacement runtime envelope claim.

The first gate is **CLI-only**: a new optimized app and managed-DNS package with
the SAME source and verified actual build features/identities. It excludes
`product-private-backup`. Published frontend bootstrap pins stay unchanged;
do not force the frontend pairing tool past their mismatch, install an unpaired
frontend, or claim a full triple/UI beta gate. Existing guest UI is not evidence
for the new backend. Before any effect, select the exact inspected package pair,
review the fixed driver and administrative fresh-account preparation, and verify
known-Off guest state. Physical host services/network and historical failed
images/accounts remain out of scope.

Exact local paths, private artifacts and package copies stay on that VM, not in
this document. No private profiles need transfer to Git. A new PC fixture must
earn its own evidence. Existing original scopes remain preserved for a later
explicit recovery, not automatically retried on startup.

Prior committed checks: 44 focused recovery tests at `23fce418`, seven native
recovery tests plus bounded diagnostics and ten driver tests at `bd83fce1`.
These are source checkpoints, not full feature closure. The PR comment records
the new cgroup/CLI checkpoint's actual results and any checks still pending.

## First PC installed result and successor

The optimized CLI-only app/DNS pair at
`1dcd4c75b7367f0474f087c16d43d6a098d88227` was actually installed and inspected
in the x86_64 development guest. Normal fresh initialization, managed pair
selection, disconnected activation, packaged login preparation, bundled preset
selection and the fixed driver's read-only preflight passed.

The one-shot positive driver passed Backup, ordinary synthetic import, Intent
pause and normal Stop. Recovery authenticated and completed OLD retirement, but
the following consuming disposition refused; the original driver exited 2.
No ordinary Start was attempted. The stopped fixture retains its closure,
archive and private captures. It is not accepted, replayed or cleaned.

Source review and a new failing regression identify a pointer-planner mismatch:
ordinary import preserves an empty Last pointer, whereas disconnected pruning
invented a Last pointer to the first profile. Exact OLD preservation correctly
refused that proposed rewrite. The successor preserves an originally empty Last
pointer while retaining repair of nonempty stale/missing pointers. It does not
disable pruning or remove the release-time preservation check. Closed release
diagnostic categories distinguish the relevant checks without exposing inputs.

The fixed next positive driver selects a fresh `ovrestore202` scope. It must earn
its own optimized same-source pair and reviewed preparation; it must not reuse
the stopped first scope. This is still agent-operated CLI-only VM evidence,
not product/UI, human-attended or whole-Restore acceptance. Partial closure or
disposition recovery remains a separate unimplemented gate.

Resume discipline: after compaction/interruption, reconcile the newest trusted
assignment, stopped/active writer, sole VM operator and next executable gate.
An old pause/cleanup task is not the current task after an explicit owner resume.

## Corrected installed OLD row

The fresh successor completed the real OLD/Intent → ordinary-Off scenario at
`c6512ce2528f30c9a7c3a22f572cbc3fc7fb15b3`, with original exit 0 and independent
pair/boundary/history/client readback. See
[the exact PC-VM evidence](../testing/RESTORE_099_PC_OLD_2026-10-13.md).
This supersedes the pending successor gate only, not the retained first failure
or whole Restore matrix. The next practical gates are the genuinely generated
MIXED and committed NEW cuts; partial-disposition recovery remains unsupported.

## Controlled producer-loss rows

At integrated developer checkpoint
`b933bf7cc0813b504abd70d29e9b11e5cb4de345`, fresh real first-rename MIXED → OLD
and terminal-publication Committed → NEW both completed with original exit 0,
independent selected-pair/boundary/history and fresh ordinary-Off/client readback.
See [the exact loss-row evidence](../testing/RESTORE_099_PC_LOSS_2026-10-13.md).
This closes those two controlled rows only, not arbitrary loss or whole Restore.
The next finite milestone is freshly qualified partial final-output disposition;
the retained first closure is not automatically a retry or cleanup target.

## Fresh closure-only OLD row

At integrated source `5132305cc34657c688ff255f6a43e473cf3f42d8`, the separately
prepared fixed 205 account completed real PauseIntent → post-receipt Abort cut
→ fresh native final-OLD qualification/disposition → ordinary owned-Off, with
original exit 0 and independent readback. See
[the exact installed evidence](../testing/RESTORE_099_PC_FINAL_OLD_2026-10-13.md).
Only the controlled Aborted closure-only row is accepted. Existing
ticket/complete/history prefixes, NEW closure and torn state remain unsupported;
the original first failed closure stays untouched. Main/beta and publication
are unchanged, and the separate full Rust stale-test failure remains pending.

## Fresh closure-only NEW row

At integrated source `d58be727aabea8354b582d38875ee6abaf0460d1`, the separately
prepared fixed 206 account completed real authenticated committed-retirement
CutOnly → fresh FinalNew qualification/disposition → ordinary owned-Off, with
original exit 0 and independent NEW/history/client readback. See
[the exact installed evidence](../testing/RESTORE_099_PC_FINAL_NEW_2026-10-13.md).
No retired OLD/Stage/nonce provenance was claimed. Existing ticket/complete/
history-prefix and torn-state recovery remain unsupported; original 201 and
accepted 205 evidence remain preserved. The old stale import test was narrowly
corrected without changing production behavior. A subsequent whole wrapper's
SIGTERM143 remains incomplete despite separately successful missing-union and
parity continuations; see exact source/outcome boundaries in the evidence.
Main/beta/RC and publication remain unchanged.

## Recovery preserving genuine prior history

At integrated source `578387e92923657e451ded762fc66880c8b43157`, fresh207 first
earned a genuine normal completed Restore/history0, then a known ordinary
restart and newer disconnected Desired. A second real committed-retirement
cut/fresh final-NEW recovery appended history1 while preserving the original
history0 FD/bytes/metadata/xattrs. Driver and independent Off/NEW/two-history/
client readback both exited0. See
[the exact installed evidence](../testing/RESTORE_099_PC_PRIOR_HISTORY_2026-10-13.md).
Existing history remains audit DATA, not current recovery authority. Current
ticket/complete-prefix recovery and user-facing integration remain separate.
The independent ticket writer is not included in that installed source.

## Committed existing-ticket recovery

At combined source `d90553ca70c2258d4b2d0d2412499bc546758ee3`, native #761 and
fixed driver #765 earned one installed fresh208 row: actual Committed closure
and existing ticket, known Stop, freshly qualified ticket recovery, unchanged
NEW and history0 containing that exact ticket, then ordinary owned-Off/reopen.
Original driver and independent readback exited0. See
[exact installed evidence](../testing/RESTORE_099_PC_TICKET_NEW_2026-10-13.md).
The private matched package pair includes no frontend; this is agent-operated
x86_64 VM evidence, not public distribution or whole Restore acceptance.
Earlier unsupported-ticket prose remains the historical boundary of its
earlier source. Complete-present/later disposition/history-only and torn-prefix
recovery still require their own gates. Original201 was not read or retried;
accepted207 was normally stopped after readback with all data retained.

## Existing all-three Complete recovery

At combined source `82f8e02063e5f7570ad649d8b047adf18de6537c`, reviewed native
#767 and fixed209 #769 earned genuine Committed closure/ticket/Complete cut,
known Stop, fresh Complete recovery and SAME existing Complete inode renamed
to history0, with unchanged NEW/boundaries and ordinary owned-Off/reopen.
Original driver and separate independent readback exited0. See
[exact installed evidence](../testing/RESTORE_099_PC_COMPLETE_NEW_2026-10-13.md).
This accepts only the earliest all-three/no-prior-history row; later absent-
record shapes and fault/crash/product/distribution gates remain separate.
Accepted208 was normally stopped after readback with data retained; original201
was not read or retried. The private CLI pair includes no frontend; main and
publication remain unchanged.
