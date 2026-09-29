# T4 subscription schedule planning boundary

This checkpoint adds an unused pure Rust planner for a future opt-in automatic
subscription refresh. The existing manual `subscriptions.refresh` and
`subscriptions.refresh_all` operations remain the only production entry points.
No preference is persisted, timer or service is registered, provider GET is
started, host state changes, or QML control appears from this change.

The preference defaults to Off. A future explicit enablement may select a
whole-second interval from 6 hours through 7 days. A successful attempt waits
the full selected interval. A failed attempt retries after 5 minutes, doubling
to at most 24 hours. The planner takes caller-supplied time and an exact native
owner generation/revision fence. It refuses stale fences and timestamp overflow;
clock rollback waits rather than triggering an early fetch. The planner's Due
result is advisory. A future scheduler must recheck owner and revision under
the same serialized owner immediately before admission and again at completion.

Activation requires a separate review of durable preference and attempt-state
schema, trusted time/restart behavior, one-worker registration, shared fetch
permits, cancellation/shutdown, provider failure handling, and privacy-safe
status. Installed ARM64 integration must check explicit enable/disable,
disconnect and ownership races; physical suspend/network transition behavior
requires its own host evidence. None of those gates is claimed here.

## Stacked private preference checkpoint

The next Draft adds an inactive, private `subscription-refresh-preference.json`
beside the ownership marker. A missing file means Off. Only an explicit call to
the new Rust setter may enable a 6-hour to 7-day interval. It acquires the
shared migration lock, proves the exact committed Rust generation, compares a
monotonic preference revision, and publishes a complete `0600` replacement.
Reads reject malformed, oversized, symlinked, public or wrong-generation state.
An ownership transition cannot silently resume a schedule saved by a prior
generation. An explicit Off choice with revision zero rebinds a valid stale
preference to the current generation; enabling it then needs a second explicit
choice against the returned revision. The setter has no socket, CLI or QML
registration.

The persisted preference file contains only schema, owner generation, preference
revision and interval. There is no URL, profile ID or attempt result. Before
enabling a timer, later implementation must bind scheduler admission to the
serialized owner revision, share the existing four-provider fetch permits and
recheck state before commit. This checkpoint has no provider request or
installed behavior.

## Stacked private attempt journal checkpoint

The next Draft adds an inactive, single-record private
subscription-refresh-attempt.json journal. Its start record is written before
any future provider fetch. It carries only schema, owner generation/revision,
preference revision, monotonic attempt sequence, daemon instance, timestamps,
consecutive failure count and fixed outcome. It contains no subscription URL,
profile ID, endpoint, response or raw failure detail. The same shared migration
lock and exact committed Rust generation protect reads and writes; the file
must be owned by the user, regular, non-symlinked and mode 0600.

The journal composes the existing planner at admission: Off refuses, a successful
attempt waits the selected interval, and failure uses bounded exponential
backoff. An unfinished attempt blocks another start in the current instance.
When a new daemon instance sees an unfinished start, its read projection says
uncertain and automatic retry remains blocked. No guessed failure or success is
published after a crash. Completing an attempt requires the in-memory ticket
from its exact start, unchanged preference revision and a fresh supplied owner
fence. A failed write is reported as uncertain, never as a confirmed result.
Malformed, oversized, duplicate-key or unsafe files also block.

The journal is not yet a production scheduler or a proof that a provider commit
succeeded. Its owner-revision input must come from the authoritative serialized
owner. A future executor must atomically coordinate its operation identity,
global fetch permit, cancellation, provider deadline, final store commit and
terminal journal publication under that owner. In particular, it must define
manual disposition of interrupted attempts and uncertain file writes before
enabling any timer, CLI, IPC or QML control. Restart or wall-clock changes cannot
be treated as evidence of a completed refresh. No live GET, VM network action or
installed behavior occurs in this checkpoint.

The current daemon instance string is a process-ID/time identifier, not proof
that a worker is still running. Production activation must check an in-memory
worker registry as well as the durable journal, and must either establish a
fresh per-start identity or conservatively classify a collision as uncertain.

## Stacked batch receipt and terminalization checkpoint

An additional inactive Draft binds a journal start to an owner-minted
subscription batch ticket: daemon instance, private operation token and exact
base revision. Its version-2 private journal stores only the numeric token,
not the client operation ID, URL or provider identity. The serialized owner
can mint a typed terminal receipt only from the matching registry token. This
distinguishes an actual committed refresh, an empty batch with no store write,
an accepted cancellation, an ordinary failure and an uncertain store write.
The runtime adapter now returns that typed receipt to its batch scheduler and
reports a missing terminal receipt using only a fixed error code. There is no
automatic journal consumer yet; the current manual batch registry remains
the source of status for existing callers.

The journal consumes the exact receipt and marks a cancellation or failure
after an Off/preference change as superseded. A cancellation with an unchanged
preference has its own terminal state. These non-success outcomes use the
bounded retry backoff, so an explicit re-enable cannot create an immediate
fetch loop. A factual committed or empty result remains factual even if the
preference changed before the journal write. A failed batch after an unrelated
owner revision can also be terminalized from its receipt. A post-rename
uncertain store write remains an unfinished, blocked attempt: no failure,
success or retry is inferred. A prior-instance unfinished attempt likewise
remains uncertain and requires a separate reviewed recovery policy.

This is still a contract, not activation. Before any background GET or timer,
the scheduled owner must coordinate batch admission, durable journal start,
worker registration, result receipt and durable terminal write under the
authoritative serialized owner. A failure between a committed store write and
its journal write must block automatic replay rather than guess completion.
The future UI must expose only bounded privacy-safe states and provide an
explicit reviewed way to resolve interrupted attempts. No host/VM network
state is changed by this checkpoint.

## Stacked inactive one-shot scheduler composition checkpoint

The next Draft adds a private one-shot seam on the existing BatchScheduler,
without registering any timer, IPC method or production caller. It is exercised
only with synthetic transports and private temporary fixtures. Read-only
preflight checks Off, interval/backoff, owner generation and an unfinished
attempt before reserving a batch operation. The scheduler admission guard is
acquired before the serialized owner, matching manual dispatch lock order.
The journal path, user ID and ownership generation are derived from that
owner, never supplied by a timer or client. The serialized owner then creates
an internal operation ID and batch ticket. The durable Started record is
rechecked and written before the first transport step; if that write fails,
the owner operation is aborted without a fetch. The attempt retains a clone
of the runtime-supplied shared four-permit fetch pool, so waiting for capacity
does not perform a provider request. The worker check and transport step are
separate calls: a future supervisor must release the owner lock around I/O.

Synthetic tests cover default Off without an owner reservation, durable start
before the first fetch, exhausted shared permits without provider I/O,
successful owner receipt and journal completion, cancellation before fetch,
Off/abort supersession, an unrelated owner-revision race that refuses a late
refresh commit, and an interrupted attempt that remains uncertain across daemon
instances. A running manual batch also refuses the scheduled reservation.
This seam intentionally does not claim live worker registration, shutdown
joining, panic/spawn-failure recovery, suspend/network
timing, or a manual disposition for prior-instance Started records. Dropping
the one-shot handle is not terminalization; it blocks later automatic retries.
Those boundaries must be implemented and accepted before background execution
can be enabled, even if this synthetic checkpoint is green.

## Stacked pre-step and commit-fence checkpoint

The next inactive Draft tightens the one-shot seam without activating a timer,
worker, IPC method or provider GET. Each synthetic provider step now requires a
fresh owner-locked admission token: the exact committed generation, current
owner revision, enabled preference revision and durable Started identity are
re-read before progress. The token is consumed by exactly one step, including
a Busy step, so a subsequent step cannot silently reuse an earlier check.
This reduces avoidable work after sequential Off, preference, marker or owner
changes; it is not a promise that an Off write cannot race an already admitted
in-flight request.

More importantly, scheduled completion now checks the exact enabled
preference inside the existing migration-lock section that fences and commits
the store. If Off or another preference revision was published before that
section, the fetched result cannot update profiles. If a preference read is
unsafe/ambiguous, no store write occurs and the durable Started record remains
blocked rather than being labelled a retryable failure. Synthetic tests cover
Off/changed preference after a completed fetch, malformed preference before
commit, no provider I/O after sequential pre-step refusal, and prior-instance
uncertainty. Manual refresh uses its unchanged completion path.

This is deliberately a smaller safety slice, **not supervised execution**.
The scheduler still has no registered scheduled worker handle. A future Draft
must couple the persisted Off setter with exact active-token cancellation
under the serialized owner; register and join a supervised worker with safe
spawn/panic/shutdown lock order; consume the exact owner receipt before
registry eviction; retain Started on absent/uncertain receipt; and prove
interleavings with manual batches. Until those gates and an explicit manual
disposition for prior-instance Started are complete, no live timer or network
path may call this seam.

The next worker composition must preserve `scheduler.worker → dispatcher`
admission order, release `dispatcher` before spawning, and register the handle
in `scheduler.worker` before releasing that reservation. A failed spawn must
recover the not-yet-run attempt and terminalize it only after no dispatcher
guard is held; dropping a captured supervisor while holding that guard would
deadlock. The worker takes only `dispatcher` between transport steps, never
`scheduler.worker`; shutdown revokes under the owner, releases both locks and
then joins. Panic before a proved terminal receipt may abort the exact active
token, but a panic after a possible store write must leave Started uncertain
unless the same owner can still mint and durably settle its exact receipt.

## Stacked supervised synthetic one-shot checkpoint

The next Draft composes the inactive one-shot with the existing BatchScheduler
worker slot. It has **no production caller, timer, IPC method, CLI, provider GET
or package/host effect**. Only deterministic tests inject a synthetic
transport. Admission reserves `worker` before the serialized dispatcher,
persists Started, releases dispatcher, then spawns and stores the join handle
before releasing the worker reservation. A failed spawn drops the captured
supervisor only after dispatcher is unlocked; it aborts and settles the exact
ticket without trying to re-lock an owner already held by admission.

Before each transport step the worker rechecks the exact owner, preference and
Started record through the owner. Transport runs outside dispatcher. Shutdown
revokes the active owner ticket, releases admission/dispatcher locks and joins
the worker; a returned in-flight result cannot commit afterward. The worker
guard settles a proved shutdown receipt or aborts only its own active ticket.
A panic before commit does the same if the owner remains available. A panic
after completed receipt/journal publication cannot erase the factual success.
If ownership, the receipt or the journal write becomes uncertain, Started
remains durable and blocks another automatic attempt across daemon instances.

Synthetic tests cover successful commit, spawn refusal, panic before/after
completion, stop/join during an in-flight step, Off after fetch admission,
manual/scheduled admission conflict, owner revocation and failed journal
settlement after store commit. They do not prove real provider timing or a
network transition. Activation still requires an owner-serialized preference
setter that persists Off and cancels the exact active scheduled token, a
reviewed manual disposition for prior-instance Started, trusted clock and
timer/suspend handling, privacy-safe UI/IPC controls, and installed host
acceptance. A generic injected transport seam is not authorization to call it
from the daemon before those gates are resolved.

## Prior-instance Started disposition: owner decision pending

This section is a **proposal, not an implemented recovery action or approval
to activate T4**. A `Started` journal entry left by another daemon instance,
or left after a possible post-rename store write without a durable terminal
receipt, cannot be classified by the next process. Store revision, profile
contents, timestamps and a fresh provider response cannot prove whether that
specific earlier attempt committed. It remains `UncertainFromPreviousInstance`
and blocks all automatic admission, regardless of the saved interval or how
much wall-clock time passes. Existing manual refresh is a separate operation,
not a replay or a terminal receipt for the old attempt.

The recommended recovery candidate, subject to explicit owner/product review,
is **acknowledge uncertainty, turn the schedule Off, then require a separate
explicit re-enable**. Acknowledgment would record a distinct
`UncertainAcknowledged` state, not `Succeeded`, `Empty` or `Failed`; it would
never reuse the interrupted batch ticket or perform a provider GET. It must be
available only to a same-user, authoritative Rust owner after a clear manual
confirmation, with no background or silent confirmation path. The confirmation
must exact-match the journal sequence, original daemon instance, batch token,
owner generation/revision and preference revision observed when the prompt was
shown. A stale prompt, still-running current-instance worker, active batch,
ownership transition, changed preference or replaced journal must refuse.

Two private files cannot be atomically replaced as one filesystem transaction.
The proposed fail-closed order under the serialized owner and shared migration
lock is: revalidate those exact fences; persist a new Off preference revision;
then persist the distinct acknowledgment tied to that Off revision. A failure
or crash after Off but before acknowledgment leaves `Started` blocked and the
schedule Off. A failed/uncertain acknowledgment write must not be called a
successful disposition. A failure while persisting Off must not write an
acknowledgment. A later enable action must separately compare the new Off
revision and acknowledged sequence; it must not cause an immediate GET merely
because the old interval elapsed. A conservative candidate is a fresh full
interval measured from that explicit re-enable, with clock rollback or overflow
remaining blocked. The exact restart timing and whether a manual refresh may
clear the pending warning require owner approval before schema or writer code.

Other choices remain visible for review: permanent block until explicit
reinstall/recovery is safe but leaves poor usability; a dedicated operator
reconciliation workflow could provide stronger evidence if it is designed and
tested. Automatically converting Started to success/failure, deleting the
journal, inferring a result from store revision/mtime, or offering a one-click
"retry now" are **not** acceptable: they guess an outcome or allow an
unacknowledged duplicate fetch. Changing the interval or toggling Off alone
must not silently clear the uncertainty.

Privacy-safe UI copy should say, for example, “Automatic refresh paused: the
previous attempt's result is unknown” / “Автообновление приостановлено:
результат предыдущей попытки неизвестен”. The detail view may show only bounded
state, time and a generic explanation; it must not include subscription URLs,
profile names, response bodies or raw errors. Any future acknowledgment
control should clearly state that it does not roll back or verify profile
changes and that automatic refresh remains Off until separately enabled.

Before implementation, synthetic negative tests must cover the exact stale
fences above; no authorization from a read-only projection; simultaneous
manual/scheduled work; a crash or injected write failure at each step of the
Off-then-ack sequence; post-rename uncertainty; concurrent enable/disable;
malformed, symlinked, wrong-owner/mode or replaced private records; clock
rollback/overflow; restart after partial disposition; and proof that no old
batch is replayed or GET starts before a separately accepted re-enable delay.
Only after the owner accepts the policy should a new schema, writer, semantic
IPC/UI and installed-host acceptance be designed and reviewed independently.

## Inactive clock-continuity arithmetic checkpoint

The independent pure `subscription_schedule_clock::assess_elapsed` helper
compares caller-supplied wall and elapsed whole-second samples. It performs no
clock reads and has no runtime caller. Missing anchors, explicit restart,
resume or unknown continuity, either clock regressing, unequal deltas and
unrepresentable deadlines refuse. Matching deltas may return an advisory wait
or elapsed result; they do not prove trusted time or authorize a refresh.

`SameProcess` is an explicit caller assumption of uninterrupted clock provenance,
not an identity check or a suspend detector. Exact delta equality is deliberately
conservative; real sample bracketing, drift tolerance, clock-source selection and
host resume detection remain separate adapter decisions. This stateless helper
does not clear a prior refusal or reseed an anchor. A future owner must retain
unresolved discontinuity rather than infer recovery from a later matching pair.

Synthetic tests cover delay boundaries, absent anchors, repeated discontinuity,
clock steps and integer limits. There is no journal/preference schema change,
terminal timestamp repair, Started disposition, re-enable timing decision,
timer, IPC/UI, provider request or host effect. Existing planner/journal callers
are unchanged. In particular, wall-clock rollback after a committed receipt
still refuses terminalization rather than fabricating a finish timestamp; the
interrupted-attempt policy above remains pending. Physical suspend and installed
host acceptance are not established by these arithmetic tests.
