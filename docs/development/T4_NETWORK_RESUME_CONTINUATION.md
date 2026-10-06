# T4 network-resume owner continuation

Status: executable, dormant developer owner composition; no installed event subscriber
or product recovery activation. It depends on the exact admission planner and
receipt/crash fixtures from #365, #374 and #419. Their historic outcomes and
production boundaries remain unchanged. The completion matrix is deliberately
narrow; real sleep/NIC acceptance is not inferred from deterministic tests.

## Completion matrix

| Scope | Required behavior | Evidence / remaining gate |
| --- | --- | --- |
| Owned local source | Authenticate Unix peer credentials to the fixture PID/UID; pin receiver boot and owner identity; strict bounded fixed events | Deterministic real Unix stream fixture; not host bus authority or PID lifetime proof |
| Event owner | Suspend pauses observation; resume/link bursts wait three quiet monotonic seconds; discard after 60 seconds from the first hint | Executable owner tests; no wall-clock input |
| Safety | Off, changed generation/revision/profile/mode/store, unavailable/mixed/foreign/unsafe ownership never reconnect; healthy state observes only | Original ownership-gated coordinator and injected binding evidence under real MigrationLock |
| One attempt | Existing exact Ready is reserved durably before coordinator invocation; failure/unknown completion never rearms | Real private files, pre/post publication errors, inherited process-death fixture |
| Shared startup/event barrier | One owner-installed admission and terminal eligibility; legacy startup paths refuse enrollment; both trigger orders recover at most once | Dormant original-coordinator/private-file fixtures; production enrollment remains unavailable |
| Restart | New owner instance refuses old Ready/Reserved/Finished; missing/lost receipt never initializes Ready | Both dormant trigger paths share refusal; no production identity/provisioning claim |
| Real integration | Canonical daemon source subscription, authenticated host event delivery and actual binding proof | Pending; no host bus, service, core or network was contacted |
| Hardware | Real suspend/resume and physical NIC transition | Pending separate bare-metal gate; VM evidence cannot close it |

## Concrete pipeline

`network_resume` is compiled under tests or the disabled-by-default
`network-resume-fixture` feature. An attributed owned Unix
stream sends only a sequence and Suspend/Resume/NetworkChanged enum. The source
rejects oversized, duplicate-field, malformed and gapped frames, channel loss
and timeout. Duplicate/reordered notifications are discarded. Receiver time is
monotonic synthetic fixture time; the sender cannot choose time, owner, epoch,
profile or commands. Boot/instance identity comes from fixture setup, never the
wire. No SSID, endpoint, URL, address or raw OS event payload is accepted.
One 100-ms monotonic deadline covers the whole frame; each blocking read is
limited by its remaining budget. A slow source cannot extend this budget by
delivering one byte just before successive read timeouts. The limit is an I/O
deadline, not a promise of scheduler latency on a stalled machine.

One owner-installed RecoveryBarrier retains immutable desired/store/owner
context, a fixed receipt binding, one EventOwner, at most one pending hint and
one non-clonable Admission shared by startup and events.
There is no source reconnect, automatic Ready provisioning, reset or rearm API.
The stable epoch is the already-established Ready fence; sequence advancement
does not prove a new network epoch. New-epoch provisioning remains separate.
The first hint bounds a burst's total age, so continuous notifications cannot
push the deadline indefinitely. Source availability and a fully drained channel
are rechecked at observation and the effect boundary. EOF or queued unprocessed
events invalidate the old action rather than skipping a possible newer Suspend.

The concrete `native_coordinator/network_resume` port borrows the original
OfflineNativeCoordinator under its actual MigrationLock. It checks the exact
committed Rust marker, revision, complete desired target, complete private-store
digest and live target existence. Off/busy does not enter the lifecycle host
observer. Incomplete observation remains uncertainty. The separate ResumeBinding
trait has no production host implementation: synthetic evidence is bound to the
exact desired target and cannot advertise real DNS/routes/protection safety.

After durable reservation and fresh revalidation, the port reserves a mutation
slot in the original coordinator and uses a narrow proved-empty lifecycle entry.
That entry refuses Off, adoption and stop decisions, preserves exact desired
intent and shares the existing bounded recovery implementation. Successful
verification repairs compatibility pointers and advances the original revision.
Any recovery/pointer error establishes the original manual-recovery barrier;
the event owner also terminalizes. Failed completion publication cannot repeat
an already completed coordinator operation. Healthy observations perform no
restart and make no claim of DNS, routes, Internet or leak protection.

The sole default-production-path code change extracts the existing startup recovery
body into a common private helper without changing its operations or error
handling. The event entry, source, receipt composition and coordinator port
remain dormant. No daemon IPC/CLI registration, settings, UI, package or
service change occurs.

## Shared dormant startup boundary

The original OfflineNativeCoordinator holds Absent, Installed, InFlight or
Blocked barrier state. Enrollment is private, once-only and before any general
startup reconciliation. Its receipt name is fixed below the original desired
state directory. Capture failure leaves a permanent blocked sentinel; extraction
leaves InFlight, so a panic cannot expose Absent or permit new enrollment. No
Arc/poison recovery, detach, reset, replacement or per-trigger Admission exists.

While enrolled, both original owner startup entry points refuse before host
calls. The lowest LifecycleExecutor general startup entry also refuses, including
the profile-preserving route into that entry. Normal factories do not enroll a
barrier, and their existing startup behavior is unchanged. Enrollment after an
unrestricted startup has already entered is refused and closes further bypass.

Guarded startup uses the same original migration lease and coordinator port as
events. It derives its quiet/deadline budget from actual monotonic enrollment
time; it never invents an old event Hint to pass the event planner. A queued or
paused event defers startup. Source loss, gap, clock regression, context loss,
manual recovery and any spent/uncertain attempt terminalize eligibility for both
paths, including a failed reservation which left durable Ready unchanged.

Healthy/settled-Off reconciliation uses a narrow observation-only lifecycle
method. A second observation becoming empty, mixed or residual cannot fall
through to general startup recovery or cleanup. Desired state is rechecked after
the last lifecycle observation before preparation or cache adoption. Observation
does not repair pointers, alter store digest, stop a residual Off owner, write
intent or increment revision. This is a cooperating original-lease contract,
not protection against an arbitrary malicious same-user filesystem writer.

The fixed fixture seeds Ready only in its fresh exclusive directory before
enrollment. Missing or mismatched receipts never initialize it. A restarted
fixture creates a new owner instance and refuses the old Ready, Reserved or
Finished record before any automatic effect. This closes the shared **dormant
fixture** trigger boundary, not production Ready provisioning or restart identity.
Supervised event/control-socket integration and actual runtime dispatcher hooks
are a separate review milestone; no new real process fixture is exercised here.

## Fixed developer entry

The feature enables one no-argument library function and a required-feature
Cargo example, `network_resume_fixture`. Types, source handles, journal, binding
trait and coordinator hooks stay crate-private. The function accepts no path,
identity, command, endpoint, profile, channel or callback. It creates a fresh
owned fixture directory, seeds synthetic Ready only there, drives a burst into
the original coordinator with the private synthetic host, verifies one recovery
and a duplicate refusal, then removes its owned fixture after successful
completion. A partial setup panic may leave owned synthetic files; it does not
claim cleanup on that failure or acquisition of any host resource. It returns only the
coarse `owned_fixture` status, effect count, revision and intent-preservation
boolean. This is not a production Ready initializer or host binding provider.

Compile preflight and the fixed scenario with `--features network-resume-fixture`
and `--example network_resume_fixture`; place Cargo and temporary storage in
dedicated HOME directories. There are no installed service/core/network calls.
Extra example arguments are refused before creating a fixture. Normal builds,
including normal daemon startup, do not contain or invoke the exported entry.
Enabling the feature still does not register an event subscriber or production
recovery path. Actual Ready provisioning and all activation gates remain pending.

Exact `5290891f1ff8499d35daa99ab4ef24490fb78312` records the original test-only
checkpoint: primary/independent review and GitHub test plus both package jobs
passed. Its local workspace/developer evidence and environment qualifications
are retained in #692. Evidence on that head is not transferred to this feature
and deadline successor; its changed code needs its own checks and review.

## Required activation decisions

Production activation must independently review authentic host source identity,
boot/owner lifetime provenance, loss/replacement handling and per-host behavior.
logind, NetworkManager and netlink notifications are observation hints, never
network trust. Network names cannot authorize recovery.

Ready provisioning must distinguish first use from loss/rollback and establish
the exact durable anchor; an absent file cannot authorize it. The fixture uses
trusted exclusive directories and the existing private atomic writer, not a
pinned-dirfd or power-loss/rollback-resistant production storage proof. It does
not inject faults inside write/fsync/rename. Source and effect operate serially
in one fixture owner; real scheduler cancellation/lease ordering remains part
of integration review.

The normal production factories still have no network barrier enrollment,
trusted Ready provisioner, host binding implementation or event source. The
shared dormant guard therefore cannot be advertised as product recovery. Actual
factory/dispatcher enrollment, supervised source cancellation, restart provenance
and durable production storage remain required activation gates. A later stable
epoch cannot bypass an unresolved reservation.

Primary and independent exact-head review must cover the changed recovery seam
and complete reached dependency boundary before any host activation. Dev VM
control belongs exclusively to its designated operator; this lane has only
source, synthetic owned files/streams and ordinary build/test authority.
