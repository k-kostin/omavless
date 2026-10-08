# Restore 0.9.9: one consumer recovery outcome

Status: **OLD-first SOURCE implementation under independent review**, 2026-10-08.
Owner selected public 0.9.8 Backup-only and required full Restore/recovery in
0.9.9. The 0.9.8 producer/dispatch/capability restriction has its own writer;
the primary/coordinator agreed the OLD-first phase progression and cooperative
same-UID ownership boundary. Implementation does not grant live operation,
product activation, main merge or release permission.

Source anchor: frozen Backup-only restriction #723
`26beeb34948fb92f481db12c48a5345f422e1fd1`, on corrective RC
`c64dad234b1f152f9b13212430ceeffde4e03488`, not the template-defective older
source. Preserve `product-private-backup`, `product_scope.rs`, Cargo feature
selection and the 0.9.8 producer unchanged. Development recovery is T4-only;
product deny must precede input, actor allocation and manager effects. No whole
historical stack is a merge vehicle.

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
short [installed VM card](RESTORE_099_VM_CARD.md). The card is a proposed matrix,
not an admitted effect-bearing driver. Do not build more observer infrastructure unless that actual
consumer scenario demonstrates a concrete missing diagnostic. Main, release,
marketplace, 0.9.8 activation and fenced-image administration stay separate.

## Agreed OLD-first phase progression

The first scope is **already-OLD live pair + authenticated Intent or Aborted**,
not MIXED rollback. `OldIntent` already checks OLD and preserves its live inodes.
Committed NEW, mixed/torn/unsupported states refuse; they remain required future
0.9.9 rows, not PASS. The historical Abort command keeps its strict socket-absent
reader and its still-fenced result. Proposed fixed new CLI entry is
`restore recover-old --confirm-rollback`, using the existing bounded private
stdin object, no new generic RPC, caller-selected PID/unit or recovery path.

One private non-Clone phase owner holds the SAME fresh migration lease and
engine ledger from admission through positive completed release. Phase names
below are design types, not current APIs or serialized effect capabilities.

| Phase / transition | Exact permission / existing primitive | Crash or uncertain result |
|---|---|---|
| `InputVerified` | Trusted product deny first; bounded parse; authenticate retained archive without admitting effects | No recovery mutation; preserve original input/result, no automatic retry |
| `ExclusiveLostOwner` | New `FreshRecovery` reservation and existing migration lease; engine `capture_recovery_singleton` acquires the SAME existing `owner.lock` once; root/lock paths and optional `Scratch6` inert socket remain held | Busy/live old owner refuses; never call CLI `StoppedRuntime::acquire` plus engine acquisition |
| `ServiceQuiescent` | Under those locks, prove lost old owner; fixed user-manager stop only `omavless-runtime.service` cancels queued restart; verify fixed runtime/legacy inactive, zero PID/control/job, no supported daemon/core or canonical listener and actual Off inventory | Unknown stop or remaining actor blocks dependent effects; no PID kill, broker cleanup or unrelated VPN stop |
| `OldQualified` | SAME origin authenticates exact archive/stage/Intent and binds current OLD/Desired/generation; classify optional valid Aborted terminal; reserve history capacity before first publisher effect | Wrong archive, unsafe members, committed/mixed/unknown evidence refuses without data replacement |
| `AbortVerified` | Intent branch uses existing `publish_recovery_abort` to fsync and verify Aborted; Aborted branch verifies SAME retained terminal. No unchanged-live rename | OLD/Intent or OLD/Aborted remains fenced. A fresh explicit invocation may requalify supported state; old context is not resumed |
| `EndpointRetired` | Only this engine's exact Root/Lock/optional `Scratch6` + `ServiceQuiescent` + `AbortVerified` admit fixed socket retirement; separately earned absence needs no unlink | Any uncertain effect seals the original scope. After death a fresh invocation requalifies socket-present or socket-absent plus authenticated Aborted; no copied FD custody |
| `TransactionCompleted` | Continue SAME engine through `retire_native_aborted`/`retire_native_terminal`, using retained terminal bytes and original members; earn native completion once | Partial receipt/stage/closure retirement remains fenced/unsupported in this first cut; do not call generic cleanup or mark whole 0.9.9 done |
| `DispositionDurable` | Existing positive completion/disposition publishes and fsyncs immutable history, verifies no pending active fences, and independently verifies OLD/Off | Recognize a durable completed result without replaying Abort. Partial disposition is unsupported in this first cut; after history rename there may be no active presence fence. Never infer completed durability or replay Abort from that absence |
| `StartableReleased` | NEW consuming boundary proves ordinary startup admits the durable state, all owned activity is Off, and releases ONLY known-completed fresh resources; it is not `dispose_and_transfer_completed`'s in-process transfer alone | Original availability is not inferred from release. A lost release outcome is unknown; no drop-to-success or guessed second owner |
| `OwnedOffAvailable` | After that boundary, one fixed normal Start and independent fresh instance/owned-Off + pair/Desired/history readback | Start failure means configuration recovered but runtime unavailable. Never replay rollback or autoconnect; startup has its own explicit retry rules |

### What authorizes the endpoint operation

No socket path comes from private input. Resolve only current canonical runtime
paths; retain root directory, original existing empty0600 singleton flock and
original migration lease. The optional socket is captured with `O_PATH` under
the engine's current validated catalogue, exact type/socket, caller UID/GID,
0600/single-link and held/named identity. Quiescence adds original manager/unit,
process/namespace and listener facts; PID0 or failed connect alone is inadequate.
Every supported writer must obey these SAME cooperative locks. Manager stop
must be known complete and restart/control jobs absent before mutation; a new
daemon cannot acquire the held locks. The packaged unit's
`RuntimeDirectoryPreserve=yes` must be verified, not assumed for another host.

Proposed endpoint state is `ExistingSocket → Retiring → SocketAbsent`, or a
separately earned initial `SocketAbsent`. Consume before first unlink; bracket
only the fixed leaf with same-origin/held/named checks, `unlinkat` relative to the
held parent, held unlinked-node metadata update for this operation's nlink/ctime,
parent fsync and exact named absence/catalogue checks. Retain the original
unlinked socket FD. Any uncertainty permanently seals this original invocation;
it cannot accept a later corrected name. Existing singleton rechecks must
understand this explicit owned transition, not pretend the old name remains.

**Threat-model decision:** this grants newly qualified retirement of the inert
canonical endpoint under **cooperative same-UID exclusivity**, not proof of the
former process's creation or protection against malicious same-UID replacement.
`O_PATH` and pre/post stat do not make `unlinkat` conditional on inode identity;
a hostile writer can swap a name between check and effect. Stronger protection
requires another ownership boundary, not more postchecks. Unsupported/foreign
types, modes, catalogues or observed substitutions refuse. Do not weaken an
existing stronger guarantee silently; this assumption needs explicit agreement.

Retirement occurs AFTER verified OLD/Aborted and BEFORE persistent fence
disposition. Thus a socket-retirement crash preserves the actual transaction;
normal startup is not accidentally enabled while endpoint ownership is unknown.
This is product recovery proposed for a new context, not permission to repair
the preserved fatal5e7 image or to remove its markers/sockets.

### One-shot engine integration and final release

Add one private `RecoverOldToCompletion` progression. Refactor only the shared
qualification prefix so Intent and Aborted are selected once; for Intent,
publish and verify Abort, then pass the SAME retained terminal into retirement.
Do not invoke `FreshRecovery::reconcile_old_intent` followed by
`complete_aborted` on the spent handle or reopen its terminal as a new origin.
Preserve existing modes. Reuse archive authentication, stage matching,
OLD-inode verification, retained publication, terminal retirement and durable
history/disposition; do not replace the entire engine with a decoded journal.

`dispose_and_transfer_completed` installs an in-process owner retaining leases.
It does NOT authorize dropping that graph and starting a service. Implement and
review a distinct consuming completed-release boundary after independent
ordinary-startup eligibility; then release known-positive owned resources and
request fixed normal Start. Pending/nonfatal-failed resources stay retained;
fatal death never promises their FD survival. No general new startup bypass,
login receipt fabrication, generation reset or carried authority token is added.

Primary/coordinator agreed this graph after independent design review. SOURCE
review must separately examine actual reached acquisition, timeout, child-loss,
release and startup paths; design approval is not implementation clearance.
Minimal owning changes include
`manager_actor_stage/native.rs` for SAME-engine progression/endpoint transition,
`native_coordinator/restore_native_recovery.rs` for one consumer/release,
and new fixed CLI/stopped-service modules. `product_scope.rs`/producer remain
unchanged; verify their denies also dominate the new development entry.
