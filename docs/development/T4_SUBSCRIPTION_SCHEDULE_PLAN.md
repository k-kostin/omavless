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

## Automatic owner and clock-driven execution checkpoint

The next independent Draft composes the existing preference, planner, journal
and typed batch receipts with the native owner. The clock-driven Rust driver
actually admits and executes the same bounded subscription batch transaction;
each tick does at most one provider step outside both owner and migration locks.
It receives the existing runtime fetch pool, whose clones share the manual
four-GET limit. A missing preference stays Off, and no production constructor,
timer, IPC method, CLI command or QML control registers the driver yet.

Admission reads exact committed native generation, owner revision, preference
revision and terminal attempt history. It reserves the existing global batch
slot and holds the still-private, non-runnable job until the journal start is
durably confirmed. Only then does it release runnable work to the driver.
The in-memory exact worker token is checked separately from the journal's
instance string. A Started journal without that live registry is uncertain,
including an accidental same-instance collision; a restart cannot guess failure
or automatically replay it.

An explicit owner preference change cooperatively cancels the exact automatic
batch. Before each provider step the owner rechecks ownership, revision,
preference and the worker token; final preference validation runs under the
same migration lease as the existing batch revision/member checks and store
publication. Off, changed preference, deleted/changed subscriptions, stale
revision and owner withdrawal cannot publish a late result. Driver stop cancels
before consuming unfinished work and prevents subsequent ticks from admitting.

The original batch registry mints the terminal receipt consumed by the journal.
A factual successful/empty commit, cancellation or ordinary refusal remains
distinct from uncertainty. If the store commit or terminal journal publication
is uncertain, or a worker is lost/panics, automatic replay remains blocked.
Neither an Off/re-enable choice nor a later manual batch clears that blocker.
No new recovery/reset capability is exposed.

Automatic maintenance never disconnects, restarts or overrides the selected
profile. Under the existing commit lease it reads durable desired state and
compares the selected profile's exact private URI, local name and present/
non-missing status in the validated current and candidate stores. If those
inputs change or disappear while connected, the entire automatic batch is
deferred as a fixed conflict with normal retry backoff; no partial store or
host action occurs. An unchanged selected proxy can receive subscription
metadata/other-row updates with zero lifecycle calls. Conservative refusal of
even a harmless URI spelling/name change is intentional. This proves unchanged
selected configuration inputs, not live controller health or network egress.
An admitted explicit Disconnect also cancels automatic work when an already
disconnected lifecycle returns NoChange without advancing the owner revision.
Manual batches retain their existing behavior.

The test-only runtime and production-observation fixtures use the shared short,
exclusive 0700 allocator. Descriptive scenario labels are not socket-path
components; a regression binds the actual nested `runtime/omavless/control.sock`.
This changes no runtime path or product behavior and weakens no assertions.
Compiler/fixture storage remains HOME-backed. Source gates may use the reviewed
offline namespace launcher to avoid unrelated HOME Git ancestry; they do not
establish installed timer or VM acceptance.

The preference writer now classifies every failed post-publication readback as
WriteUncertain. The automatic owner latches that uncertainty and cancels its
exact current worker, so a later apparently valid read or Off/re-enable cannot
clear the current context's blocker. Interrupted admitted work additionally
retains its durable Started journal across daemon restart. No implicit recovery
policy is added for either case.

| Scope | Deterministic evidence | Remaining acceptance |
| --- | --- | --- |
| Default Off, interval/backoff and injected clock | Actual driver and private file tests; production HTTP transport against synthetic loopback | Installed opt-in preference and trusted daemon timer wiring |
| One worker, shared fetch permits, disable/cancel and stale completion | Owner/driver concurrency and refusal tests | Combined native integration and exact installed head |
| Durable begin, typed terminal receipt, interrupted/uncertain state | Private journal and fault tests | Reviewed explicit interrupted-attempt disposition before product exposure |
| Network and lifecycle ownership | No core, TUN, proxy, service or package effect in this checkpoint | VM operator's separate integration; physical suspend/network cases if later claimed |

New work follows the [execution policy](EXECUTION_POLICY.md), copied exactly
from owner-approved policy commit
`b8c967f2039bdad8a385429a212b977318adc9dc` (#662). This changes development
procedure only; it does not weaken subscription/store guarantees or adopt the
separate T4 retained-manager actor implementation. The automatic maintenance
feature remains incomplete and unexposed until its own pending gates pass.
