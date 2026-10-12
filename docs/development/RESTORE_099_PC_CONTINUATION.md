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

Exact local paths, private artifacts and package copies stay on that VM, not in
this document. No private profiles need transfer to Git. A new PC fixture must
earn its own evidence. Existing original scopes remain preserved for a later
explicit recovery, not automatically retried on startup.

Prior committed checks: 44 focused recovery tests at `23fce418`, seven native
recovery tests plus bounded diagnostics and ten driver tests at `bd83fce1`.
These are source checkpoints, not full feature closure. The PR comment records
the new cgroup/CLI checkpoint's actual results and any checks still pending.
