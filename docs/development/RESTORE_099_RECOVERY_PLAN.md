# Restore 0.9.9: one consumer recovery outcome

Status: **design proposal / implementation agreement pending**, 2026-10-08.
Owner selected public 0.9.8 Backup-only and required full Restore/recovery in
0.9.9. The 0.9.8 producer/dispatch/capability restriction has its own writer;
this document grants no implementation, host operation or release permission.

Source anchor: corrective RC
`c64dad234b1f152f9b13212430ceeffde4e03488`, not the template-defective older
source. The pending 0.9.8 trusted restriction must be recorded as an additional
baseline before implementation. No whole historical stack is a merge vehicle.

## Observed obstacle, not another ARM checkbox

The [preserved fatal cut](../testing/RC_098_VM_2026-10-08.md#fatal-lost-owner-cut-preserved-data-recovery-not-usable)
used original `5e7d4258`: Intent reached, runtime killed in an explicitly
selected disposable VM, OLD bytes retained, startup repeatedly refused exit2,
normal Abort returned original exit2/stopped-precondition. The failed unit was
stopped administratively; no socket/marker/journal was deleted. This is retained
data and fail-closed fencing, **not usable recovery**, and is not final-source
fatal acceptance. The fenced image stays untouched.

Actual code at the source anchor:

- `restore_abort_cli::StoppedRuntime::recheck` refuses every socket pathname,
  including stale names. It does not establish ownership by failed connect.
- `restore_first_abort_owner` finishes `AbortedStillFenced`; OLD readback alone
  cannot admit a normal daemon.
- `FreshRecovery` has private completion/disposition machinery and a new
  installed-boundary reservation, not a shipped end-to-end consumer path.
  `reconcile` latches `attempted` before fallible work: do not call its one-shot
  modes successively on one handle or serialize its live proof graph.

## First executable result

**One explicitly confirmed stopped-service recovery to verified OLD**, initially
for authenticated Intent/Abort cases. A user supplies the retained encrypted
archive/passphrase through bounded private input, not argv/environment/logs.
The operation must finish with independent OLD pair/Desired readback **and** a
fresh ordinary owned-Off runtime. Exit0 or an “OLD restored” label is insufficient.

This first slice is not whole 0.9.9 acceptance. Committed NEW and remaining
publication/crash cuts retain mandatory follow-up coverage before full Restore
is public; rejecting them safely does not mark that coverage PASS.

| Point | Required fact / result | Existing gap or next check |
|---|---|---|
| Explicit entry | User sees replacement/recovery distinction; private input bounded/authenticated | Select a fixed consumer CLI/terminal entry; no generic shell or raw RPC |
| Quiescence | Fixed service inactive, no restart/control jobs, old runtime/core children or listeners; owned Off inventory | Design supervised stop/absence admission; MainPID0 alone is not proof |
| Fresh recovery admission | Existing exclusive singleton/migration locks, original pathname/FD identities, current package/user/namespace facts | Newly earn a recovery context; never adopt dead-process descriptors or copied tokens |
| Classification | Authenticate archive and correlate exact supported transaction/OLD/current members | Wrong archive, substitutions, malformed/unsupported/torn evidence refuse unchanged |
| OLD rollback | Exact pre-transaction pair and unchanged Off intent independently verified | Reuse reviewed authentication/rollback primitives, not a whole new engine |
| Durable completion | Journal/fence disposition survives crash; normal admission is earned, not assumed | Define one causal phase progression instead of reusing a spent one-shot handle |
| Socket retirement | Only a positively pinned canonical dead-runtime socket, under exclusive custody and identity rechecks | Separately reviewed fixed operation; no wildcard cleanup, arbitrary path or unlink on failed connect |
| Availability | Normal Start, new owned instance, verified Off and preserved pair/Desired/history | Keep configuration recovery separate from startup availability; no autoconnect |
| Failure or loss | Truthful fenced/unavailable result, necessary copies/evidence retained | No blind resend, guessed compensation, epoch reset or invented descriptor survival |

The exact ordering of socket retirement relative to rollback/completion must be
chosen with the stopped-owner boundary: the current reader forbids a socket
before any recovery. Do not bypass that predicate ad hoc; add and review a
distinct admitted phase with its own ownership proof. A crash between phases
must stay fenced or enter a specifically supported recovery state.

## Minimum implementation agreement

Before code, primary and independent review must settle:

1. Supported transaction phases and OLD/NEW policy; no rollback of Committed NEW
   merely because it is convenient. Initial unsupported cases remain refusals.
2. Authority for fixed service stop/start, exact stale-socket retirement and
   durable fence disposition. No caller-selected PID/path/command or blanket
   sudoers privileges; preserve unrelated VPNs/services.
3. One continuous owned operation versus typed internal phase transfer, with
   aggregate resource bounds, irreversible uncertainty and crash/re-entry rules.
   Fatal loss creates a newly admitted context, never old FD survival.
4. Exact runtime/API/contract writer ownership after 0.9.8 restriction freezes.
   The runtime writer may reuse fixed existing inputs but must not silently
   broaden the ordinary IPC, package authority or old sealed experiments.

Candidate owners: `restore_abort_cli.rs`, `restore_abort_stopped_owner.rs`,
`restore_first_abort_owner.rs`, `native_coordinator/restore_native_recovery.rs`,
the existing disposition/startup consumers and fixed CLI/service handoff.
This is a call-site map, not permission to edit every module or weaken their
guards. TUI readiness [#720](https://github.com/k-kostin/omavless/pull/720) remains
a separate future-client slice; it does not implement recovery.

## Bounded acceptance and delivery

Start with one ordinary consumer path, then its immediately relevant negatives:
authenticated Intent/Abort → OLD → restart; wrong archive; busy/live owner and
restart race; foreign/replaced socket; repeated invocation; loss during durable
completion/socket publication/start. Check every required crash point without
calling administrative reset recovery. Later cover NEW/Committed and the
remaining mixed/torn/publication states before whole 0.9.9 closure.

Use synthetic private pairs and immutable failure baselines. Record exact source,
bundle and original outcomes; independently verify data, history and ordinary
availability. Bound diagnostics before mutation, keep secrets/raw logs outside
Git, and never ask the owner to debug internal receipts. A green pure model or
fixed-unit command does not establish filesystem/manager/VM acceptance.

Next finite cycle after agreement: implement/gate **one explicit supported OLD
recovery** in its owning dev branch, including a runnable client entry and a
short VM card. Do not build more observer infrastructure unless that actual
consumer scenario demonstrates a concrete missing diagnostic. Main, release,
marketplace, 0.9.8 activation and fenced-image administration stay separate.
