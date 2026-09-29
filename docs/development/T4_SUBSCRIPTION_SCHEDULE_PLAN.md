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
