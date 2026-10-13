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

### Recovery manager qualification on installed systemd

The installed user manager can be nondumpable even to its own UID. Requiring
its `/proc/PID/exe` and namespace descriptors therefore blocks recovery on a
normal host. The new recovery path instead binds the fixed system-manager
`user@UID.service` attestation (including invocation and PID) to an original
pidfd, readable process start/status facts, and a retained authenticated peer
on the canonical `systemd/private` socket. Rechecks must retain that same unit
invocation, process and peer; disconnection never authorizes reconnection.
The owner-only socket modes `0600` and systemd's `0700` are accepted, not
group/other access or special permission bits.

Installed inspection also found the manager's nondumpable PAM helper and an
unrelated-process churn dependency in the historical global census. New
recovery therefore uses the agreed cooperative ownership boundary directly:
the original locks, authenticated manager, fixed packaged runtime unit with
known Stop completion and no jobs/PIDs, empty recursive service cgroup,
canonical no-listener check and actual Off observation. It does not grant
process-name exceptions or require every desktop process to expose its ELF.

The unit must use the fixed `app.slice`, `Delegate=no`, `KillMode=control-group`
and no drop-ins. Pin the verified cgroup2 hierarchy through that slice; each
observation must find the exact runtime child absent or read its bounded
`cgroup.events` with `populated 0`. Unreadable/substituted/unknown state refuses.
The empty service child may disappear across a known Stop; a replaced parent
is not adopted. User cgroups are delegated: this is **not** exclusion of a
malicious same-UID process moving itself or ignoring the shared locks. Supported
manual daemon entry points must obey those same locks.

Historical Abort retains its strict census. Own namespace/proc visibility checks
remain. Manager identity uses root-controlled system-service attestation, not
direct executable/namespace descriptor proof; OS ptrace/security settings are
not relaxed. Deterministic tests do not replace installed capture and recovery.

### Source-only finite developer producer loss cuts

The dependent `dev/099-restore-loss-cuts` source starts at #739's exact
`c6512ce2528f30c9a7c3a22f572cbc3fc7fb15b3`. Two closed private developer entries
select existing callbacks in the SAME retained native engine:

| CLI | Private method | Exact callback |
| --- | --- | --- |
| `developer cut-current-first-rename --confirm-private-first-rename-cut` | `developer.cut_current_first_rename` | `NativeStep::Renamed(0)` |
| `developer cut-current-terminal --confirm-private-terminal-cut` | `developer.cut_current_terminal` | `NativeStep::Terminal` |

Both use the existing bounded schema-1 archive/passphrase stdin and exact
instance/revision correlation. No step, index, manager command or phase comes
from input. Product/default builds deny these entries; the product restriction
also dominates the T4+product feature union and direct helpers before input or
acquisition. Owned revision admission, migration lease and original-slot vacancy
match CompleteCommitted/PauseIntent; the older FenceCommitted shortcut is not
exposed. Completion is always disabled for these two modes, even if a callback
were unexpectedly unreached. No new filesystem effect is implemented.

Only an exact reached callback plus its expected StillFenced result, AFTER the
engine's error/seal/retained-holder and transaction-fence handling, returns
`cutReachedStillFenced: true`. This is data, not completed Restore or resumable
Intent. Terminal publication precedes whole-pair verification and the engine's
committed state; it must not be reported as completion. Wrong authentication,
earlier refusal, busy/unreached callback or another error cannot return this data.
The SAME owner mutex spans execution and reply publication; broken/lost reply
is UNKNOWN, never permission to resend, Abort, clean or restart. Existing
private-input zeroization limitations for JSON temporaries remain unchanged.

Filesystem controls use independently authenticated China NEW and Roscom OLD
members that differ in BOTH store and template. MIXED must be produced by the
actual first rename, never hand-written journal/live state. Installed cuts and
fresh MIXED/committed NEW consumer recovery remain pending. No new cut VM
selection is authorized until the current OLD gate passes and Root/Astra review
the exact package, fresh fixture and recipe; preserved failed accounts/images
are not replay or cleanup targets.

### Fixed PC loss-driver preparation

The dependent driver delta is SOURCE preparation, not an effect grant. Root
reports the separate fresh 202 OLD row completed; its exact result remains in
the owning installed evidence. Neither that result nor the producer's original
`56a928970755bada78740726cac23a9df3971c29` source checks accepts a new cut row.
Select the exact app/DNS pair, these scripts and the fresh-account preparation
with primary and independent review before execution. Historical 201 closure
and all earlier scopes remain untouched. No source-unchanged native suite needs
to be relabeled or repeated for this Python/card delta.

| Fixed entry | Account / fresh evidence leaf | Actual producer and consumer | Independent required outcome | Status |
| --- | --- | --- | --- | --- |
| `loss_recovery.py positive-mixed` | `ovrestore203` / `restore099-pc-mixed` | first live store rename; `recover-mixed --confirm-rollback` | MIXED is staged NEW store + OLD template with Intent only; both members differ; exact OLD byte recovery; Aborted history phase 3; fresh ordinary Off and client reopen | SOURCE recipe; VM pending |
| `loss_recovery.py positive-committed` | `ovrestore204` / `restore099-pc-committed` | original terminal publication; `recover-committed --confirm-completion` | both live NEW members plus bound Committed terminal; both members differ; exact authenticated-consumer NEW readback; Committed history phase 2; fresh ordinary Off and client reopen | SOURCE recipe; VM pending |

Each account must be separately initialized with the reviewed managed pair,
packaged login receipt, startup disabled, and normal China preset before the
driver. Matching `preflight-mixed` / `preflight-committed` entries only observe.
The positive recipe exclusively creates its fixed evidence directory before
any command; its existence blocks reuse even after refusal. It runs Backup of
empty China NEW, then ordinary semantic Roscom preset (keep mode **on**) and
ordinary synthetic profile import to OLD. Desired/owner/login are pinned only
after those legitimate mutations complete. No Connect, TUN, DNS lease, shell,
caller step, PID, unit or arbitrary account/path input is introduced.

The exact CLI cut marker follows the client's positive-only
`cutReachedStillFenced` response check. It is not completed Restore, including
at the terminal cut. The driver independently pins the four actual staged
members, ready digest, Intent and applicable terminal and verifies every byte
binding before and after one known normal Stop. Stage NEW is comparison DATA;
the consumer authenticates the archive. Empty China counts/preset/startup and
semantic equality to the independently saved portable input are also checked,
allowing `restore_store_off` reserialization rather than inventing Python byte
normalization. MIXED rollback may replace both live inodes: only exact OLD
bytes, not old named-inode survival, are required. Both rows retain unchanged
Desired/owner/login identity and bytes through final readback.

The new fixed driver keeps original private capture descriptors, input files,
children and boundary/stage pins after attempted effects. Its per-command
budget is 150 seconds; poll, capture substitution/size, lost or malformed cut
marker, or post-effect readback uncertainty parks without kill, retry, unlink
or cleanup. A known completed nonzero exit remains recorded as that exit; it is
not assumed to prove no effect, and the outer unverified scope parks too.
Diagnostics and parking waits are best-effort/nonthrowing. The legacy 202
entry/default remains unchanged; shared account parameters accept only exact
202/203/204 username/home pairs, and history phase is data-only/default Aborted.

The driver delta's original SOURCE checks on 2026-10-13 exited 0: legacy 10
plus new 14 fixed-data/mock controls (24 total), then `./tests/run.sh` with 721
Python controls (2 skipped) and its normal JS/QML/navigation checks. Python AST
syntax, `bash -n tests/run.sh` and `git diff --check` also passed. The mocked
children are not VM effects; no native/runtime bytes changed and no installed
203/204, process-loss or host-network acceptance is claimed.

### First fresh final-output OLD route

The next isolated SOURCE implementation starts at the installed-row/evidence
checkpoint `db15189f16309608936b779c437e4f91adea3572`. It adds exactly
`restore recover-final-old --confirm-disposition`, with the existing bounded
schema-1 archive/passphrase stdin. This is **closure-only Aborted/OLD**, not a
general partial-retirement/disposition repair or Committed/NEW selector.
Historical 201 is excluded; an installed result must come from a separately
prepared fresh fixture after exact source, pair and recipe review.

The new fresh invocation authenticates NEW, acquires its own original existing
migration/singleton locks, and pins current OLD, owner, Desired, required
packaged Login, directories and canonical closure in the same native ledger.
Before fixed-service qualification/Stop it brackets the closed original
catalogue and requires stage, Intent, terminal, receipt, successor/next closure,
routing-preset pending, BOTH forward/rollback replacements, ticket/complete and
ALL eight disposition-history slots absent. No other `restore-*`/`.restore-*`
state entry or any reserved-prefix config entry is admitted; ordinary unrelated
config names remain preserved in the captured catalogue. Current OLD must match the Aborted closure receipt; current OLD plus
freshly authenticated portable NEW reconstructs its terminal stage identity
through the existing pure `output_binding` semantics. These records are DATA,
not former authority. The finite free-history target is reserved before effects.

Only the exact pinned Scratch3 closure gets a captured-final observation
exception. It is not completed origin, retirement or a completion grant. The
actual fixed manager/cgroup guard qualifies under the SAME leases; original
source/parent synchronization and whole-gate rechecks then earn a private
`FreshFinalOld` qualification. Missing receipt means a genuinely never-acquired
Scratch2 role plus repeated named absence, **not** an invented unlinked FD or
fake retirement flags. Dedicated FinalOld completion/disposition wrappers
reuse create-only ticket/complete, immutable history and the consuming OLD
release boundary. Pointer preservation and final pair/history readback precede
release; only after positive release may one normal Off Start be requested.
Uncertainty seals and retains the original nonfatal graph without retry/repair.

The fixed SOURCE producer is
`developer abort-current-after-retirement-cut --confirm-private-abort-retirement-cut`,
using the existing bounded schema-1-only Abort input and SAME original positive
PauseIntent/prepared archive/revision owner. Its private method is
`developer.abort_current_after_retirement_cut`. `NativeStep::RetirementComplete`
fires AFTER durable receipt unlink and final live/closure/origin checks but
BEFORE completed permission or disposition. The earlier `NativeStep::Closure`
still has its receipt and is not this shape. Exact reached expected sealed
error yields only `cutReachedStillFenced:true` DATA; reply loss under the SAME
owner mutex is UNKNOWN, with no resend, Abort, cleanup or startup permission.
Product restriction dominates the new CLI/method/direct helpers before input
or acquisition; no Cargo feature, archive/corpus or privileged API changes.

Required new SOURCE controls exercise actual private-file native IO and normal
retained Pause→Abort producer, unsupported phases/history/prefixes, boundary/
pair/closure/catalogue substitution, descriptor capacity, every qualification
sync and disposition publication/unlink/rename/sync prefix, retained error
custody, closed commands and product-union denial. These do not prove installed
manager/cgroup acceptance; fresh 205 and later partial/torn-prefix repair remain
pending. Earlier actual OLD/MIXED/Committed rows retain their exact receipts.

Initial original SOURCE gates exited 0: T4 `final_old` (9), the full native
engine filter (22, including the five new file controls), product-union
`final_old` (11), product dispatch denial (1) and the stopped-self matcher (1).
Strict T4 and explicit `product-private-backup,product-image-witness` all-target
Clippy passed; ordinary `tests/run.sh` passed 721 Python controls (2 skipped)
and its normal JS/QML/navigation checks. The isolated test executables stayed
unchanged while their self-reexec controls ran. Default non-test compilation
has a separately identified inherited optional-rustix cfg repair owned by Root;
it is not claimed passed from these T4/product gates. Exact source review,
combined candidate checks and fresh 205 installed acceptance remain pending.

### Distinct fresh final-output Committed NEW route

The next SOURCE slice starts at `400676cfd796f6370f5b85ed541bf2d45642e9a5`,
after the separately recorded actual fresh 205 OLD row. It adds only
`restore recover-final-new --confirm-disposition`, using the same bounded
private archive/passphrase input. `FinalNewOutput`, private `FreshFinalNew` and
the recovered/release NEW selectors are distinct from OLD; no fallback or
decoded-record grant is introduced. Current NEW must byte-equal authenticated
`restore_store_off`/template and the Committed closure's pair bindings, with
unchanged owner/Desired and required packaged Login, before fixed guard Stop.
Retired OLD lineage is unavailable and is not claimed or used for rollback.

The shared native helper preserves OLD semantics and all strict first-route
absence/catalogue rules: stage/Intent/terminal/receipt/successor/next, BOTH
replacement families, routing pending, ticket/complete, all eight history
slots and other reserved state/config prefixes refuse. Captured closure is
observation DATA only. The SAME original migration/singleton and concrete
manager/cgroup guard, ten held-source/parent syncs and final gates earn the
private NEW qualification. Missing Scratch2 still means never-acquired receipt
plus repeated absence, not a fabricated retired descriptor. Existing create-
only disposition/history and pointer-preserving consuming release precede a
single ordinary Off Start and independent NEW/history verification.

The fixed producer is
`developer cut-current-committed-retirement --confirm-private-committed-retirement-cut`
/ `developer.cut_current_committed_retirement`, with existing full schema-1
archive/passphrase input and strong owned revision/migration/vacancy admission.
Its dedicated typed `CutOnly` retirement permission is consumed BEFORE native
IO/Stage. The committed-chain validation is reused, but this mode structurally
cannot set completed, disposition or ordinary handoff even if its callback is
always OK/unreached. Actual `RetirementComplete` occurs only after receipt
removal and final original checks; exact reached expected sealed error may
yield only still-fenced CUT DATA. The SAME owner/quit mutex spans publication;
lost reply is UNKNOWN with no retry or cleanup.

An occupied previous holder is a not-submitted refusal: it is not sealed or
reclassified by the new cut. Revision/auth refusal before installation also
preserves that ordinary context. Any outcome after THIS attempt's new holder
admission unconditionally seals/blocks its graph, including an unreached cut.
Product/default/direct-helper denial remains before private input, acquisition
and effects. No features, pins, corpus/archive or privileged API changes.
Native file/auth/phase/pair/Login/catalogue/history/FD and every sync/disposition
fault controls, genuine producer/unreached/no-touch cases and broken reply are
SOURCE only. Exact authority review, combined packet and fresh 206 recipe/VM
acceptance remain pending; 201/205 are not reuse or cleanup targets.

Original bounded SOURCE checks passed: T4 `final_new` 10, full native filter 27,
product-union `final_new` 10, product dispatch 1, exact stopped-self matcher 1,
and the shared self-reexec FD-capacity check 1. Strict T4 and explicit
`product-private-backup,product-image-witness` all-target Clippy, bare/default-
TUI non-test compilation, formatting/diff checks and ordinary 739 Python
controls (2 skipped) with JS/QML passed. Initial private-test visibility and
product-only unused-test-helper failures remain original Source failures;
their narrow test fixes do not change runtime authority. Executables were held
unchanged through original/self-reexec tests. These are not fresh 206 effects.

The distinct controlled NEW row was subsequently installed and completed at
integrated `d58be727aabea8354b582d38875ee6abaf0460d1`, including the separately
reviewed fixed206 driver. Original one-shot cut/recovery and independent
NEW/history/ordinary-Off/reopen readback exited 0. See the
[exact installed evidence](../testing/RESTORE_099_PC_FINAL_NEW_2026-10-13.md).
This supersedes the pending fresh206 row only: product Restore integration,
ticket/complete/history-prefix recovery and other remaining gates are not
accepted by that result. Original201 and accepted205 evidence are preserved.

### Closure-only output with an existing valid audit prefix

The next bounded SOURCE slice starts at
`4fc317b987480e142812ea90bf10d9eb19ad2faf`. Both existing fixed final-OLD and
final-NEW selectors may admit zero through seven previous complete audit files.
Within the state catalogue's reserved-prefix predicate, only the current
closure and the exact eight `native_history::NAMES` leaves are eligible. This
is not the more permissive generic catalogue-name predicate. Reserved config
prefixes, current ticket/complete and all previous unsupported active shapes
still refuse; no new CLI, producer or input selector is introduced.

The SAME native history ledger is captured before guard qualification/Stop and
effects. It requires a contiguous immutable prefix, exact complete-record bytes,
owned UID/GID, mode0600, one link, no xattrs, held/named identity and a genuinely
free next target. Full capacity, sparse/torn/unsafe files or unknown suffixes
refuse; nothing is evicted, renumbered, repaired or recaptured. Existing history
is audit DATA only: syntax does not prove its original publication/fsync or
create current recovery authority. Its older owner generation, Desired and pair
may legitimately differ. Current closure/archive/Owner/Desired/required Login
binding and distinct freshly earned OLD/NEW qualifications remain unchanged.

All existing past files/metadata are checked across the original gates and
create-only disposition. The first unused target is published with the existing
`RENAME_NOREPLACE` operation and SAME Complete/Scratch1, not recaptured as another
past audit. Substitution, content/metadata/xattr drift or target collision seals
after effects; unknown outcomes never retry. Final current pair/history readback
and the unchanged typed consuming release still precede ordinary Off Start.

New real-file SOURCE controls cover both selections with every zero-to-seven
prefix, exhausted/sparse/torn/unsafe/xattr/unknown admission, historical and
target drift at qualification/publication/final gates, every existing sync and
disposition cut, and unchanged past bytes/inodes after positive fixture release.
Fixture release is not installed manager/quiescence or FreshRecovery acceptance.
Ticket/complete-prefix recovery remains a separate unsupported slice. Future
fresh207 must earn its previous history through a genuine completed Restore and
known ordinary restart, not fabricated records; 201–206 are not reused. Exact
SOURCE review, combined gates and that separate installed recipe remain pending.

Original focused SOURCE gates passed: four new history controls (130 matrix
cases), complete native engine filter31, retained/final-output filter18,
unchanged history ledger5, actual self-reexec FD-capacity1 and ordinary completed
Restore/replay1. Explicit `product-private-backup,product-image-witness` union
passed native final-output16, closed recovery CLI9, dispatch denial1 and history
ledger5. Strict T4 and that explicit union all-target Clippy passed; bare and
default-TUI non-test compilation, formatting/diff checks and normal755 Python
controls (2 skipped) plus JS/QML passed. Each original test ELF stayed unchanged
through its runs. These counts are SOURCE evidence, not fresh207 admission or
whole Restore/product acceptance.

The finite prior-history scenario subsequently completed at installed
`578387e92923657e451ded762fc66880c8b43157`: genuine normal Restore/history0,
known ordinary restart, newer disconnected Desired, second real committed cut
and final-NEW recovery/history1 with preserved prior FD/bytes/metadata/xattrs.
Original driver and independent Off/NEW/history/client readback exited0. See
[exact fresh207 evidence](../testing/RESTORE_099_PC_PRIOR_HISTORY_2026-10-13.md).
This accepts that controlled row only, not same-process repeated Restore,
eight-slot VM coverage, current ticket/complete-prefix or whole product gates.

### Distinct Committed NEW existing-ticket route (SOURCE)

The isolated successor starts at
`565815c1bf73cc3198903dacb0d6587008171f9c`; it is not included in the separate
fresh207 packet. Fixed consumer `restore recover-ticket-new --confirm-disposition`
uses the existing bounded archive/passphrase stdin and distinct NewTicket/FreshNewTicket
selection and typed NEW release. OLD, Complete-present and torn-prefix recovery
are not fallback branches. The inherited zero-to-seven immutable history policy
remains; no historical record supplies authority or current Desired/generation.

Before fixed guard Stop, current NEW must match authenticated portable NEW and
the Committed closure. The existing canonical ticket must match that exact
closure and current UID/owner generation/Desired, with required Login, original
locks/boundaries and the same closed catalogues/competing-prefix absences. Its
Scratch0 descriptor/bytes are captured observation DATA, not this invocation's
publication and not completed/disposition permission. An explicit observed-ticket
role permits only this narrow byte/named/pending check; it never sets disposition
or completed to bypass capture. Eleven same-guard held-source/parent syncs and
final gates earn the private proof, with genuinely absent never-acquired receipt.

Fresh consuming disposition checks that proof and consumes its one attempt
before the first Complete effect. It rechecks the SAME observed ticket, creates
Complete exclusively, and reuses the existing closure/ticket unlink, no-replace
history rename/sync and final readback tail. It never overwrites, recreates or
relabels the ticket as its own publication. Pointer preservation and typed NEW
release still precede ordinary Off startup; substitution/collision/error seals.

Fixed genuine producer
`developer cut-current-committed-ticket --confirm-private-committed-ticket-cut`
maps only to `developer.cut_current_committed_ticket`, with the existing full
schema1 archive request and strong owned revision/migration/vacancy admission.
NewTicketCutOnly is latched before native Stage. Original retirement/final
checks construct a non-Clone operation-local RetiredForTicket borrowing the SAME
engine/session; records or success booleans cannot construct it. Its only
continuation consumes it to publish/verify one ticket, then seals even if the
callback is always OK. No Complete/disposition/ordinary handoff branch exists.
The normal Complete wrapper retains its original final-check/callback/grant order.

Capability Drop only revokes flags, without manager calls, descriptor release,
diagnostics, allocation or locks. The outer admitted operation catches unwind,
seals its own retained (including poisoned) holder and blocks that transaction;
poisoned/unwound state never yields positive CUT DATA. Busy prior Pause and
authentication/queue refusal before installation are untouched. Exact fully
verified post-ticket callback plus retained blocked graph alone yields
`t4_current_committed_ticket_cut_reached_still_fenced`. SAME owner/quit custody
covers reply publication; lost/broken reply remains UNKNOWN with no retry.

Default/product/direct helper denial precedes private input/acquisition for
this route, including the strengthened product union. Actual Source controls
cover authenticated NEW/crossed ticket UID/generation/Desired/closure, unsafe
and competing shapes/history, all eleven syncs/disposition cuts and gates,
observed-role substitutions, SAME ticket FD/bytes and no re-publication, genuine
producer/no-op callback, panic before/after capability, actual partial ticket
write error/unwind, prior Pause/auth/queue and broken reply. These are synthetic
private-file Source checks, not installed manager or timed VM acceptance.
Exact immutable source review/combined gates and a separately reviewed fresh208
recipe/packet remain pending; no 201–207 account or evidence is reused.

Original focused SOURCE gates passed: whole native engine37, ticket native/
genuine producer10, previous final-output producers4, ordinary Complete/replay1,
actual self-reexec FD-capacity1, exact request/selector/self matcher1 each.
Explicit `product-private-backup,product-image-witness` union passed ticket10,
recovery CLI10, dispatch denial1 and history ledger5. Strict T4/union all-target
Clippy, bare/default-TUI non-test checks and normal755 Python controls (2 skipped)
plus JS/QML passed. Default's real dispatch denied the new method (1), and its
actual debug CLI refused both fixed commands with stdin held OPEN (2), before
private input/acquisition. The recovery module itself is intentionally absent
without T4; a zero-test selector filter is not labeled a denial result.

Earlier original failures are retained: one test wrongly accessed a sealed FD,
a test-only Slot comparison did not compile, and a default test wrongly called
the deliberately absent producer API. Narrow test fixes followed; the final
default-only cfg correction leaves effective T4/union code unchanged. A private
default CLI harness initially guessed the wrong fixed error category; its
original failure is preserved and corrected exact-error/read-block controls
passed. Test ELFs remained unchanged throughout original/self-reexec runs.
No Source result grants packet, timed VM, product or whole Restore acceptance.

The finite existing-ticket row subsequently completed at installed combined
`d90553ca70c2258d4b2d0d2412499bc546758ee3`, including the separately reviewed
fixed208 driver: actual empty NEW Backup, ordinary synthetic OLD, genuine
Committed ticket cut, known Stop and fresh ticket recovery to unchanged NEW,
history0 and ordinary owned-Off/reopen. Original driver and separate independent
readback exited0. See [exact installed evidence](../testing/RESTORE_099_PC_TICKET_NEW_2026-10-13.md).
This accepts only that no-prior-history ticket shape; Complete-present, later
disposition/history-only, torn-state and whole user-facing product gates remain
separate. Old207 data/evidence and failed201 were preserved; no publication or
main update follows from this row.
