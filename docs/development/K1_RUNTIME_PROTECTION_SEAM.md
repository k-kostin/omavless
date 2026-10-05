# K1 unregistered runtime protection seam

Status: opt-in SOURCE candidate from docs checkpoint
`800b0a2bae71c993b5263b4dd683eb1685a1a421`, not activated or installed.
The [service-core evidence](K1_SERVICE_CORE.md) remains exact application
`d3b24a364c1fdc5b07a866edb54ec89994cd1086` and protocol v1. This successor
does not change those artifacts, their receipts, #663 or the remaining
[product/physical contract](../roadmap/KILL_SWITCH.md).

## Missing product callers and selected scope

The base runtime has no NetGuard dependency/client. Normal connect persists
connected before starting the core; explicit Disconnect proves owned-empty
but has no Disarm; startup/replacement/mode/profile paths lack protection
coordination. Native preflight checks the managed DNS pair, while existing
templates use Meta rather than reserved omavless0 and do not carry K1's mark.

The new `netguard-runtime-candidate` feature is not default. Its private
`lifecycle::protected_candidate` owns the SAME LifecycleExecutor and an injected
typed port, not a second host/desired owner. No ordinary executor extraction,
daemon/CLI/IPC registration, advertised capability or production constructor
is provided. Native protected preflight defaults to Unsupported. The port has
no concrete socket/backend: its injected replies test conformance, not original
peer, namespace, deadline or kernel authority. Future assembly must remain
inside the existing committed-owner/migration-lock/mutation serialization gate.

Scope is fresh disconnected -> Full/global -> explicit Disconnect. Connected
startup/adoption/recovery, replacement, Rule/Direct changes and profile/preset
quiesce remain unsupported. No root Recover, sudo/pkexec, nft, selectable
path/UID/mark/interface or shell operation is introduced.

## Same-producer v2 closed floor

The unreleased successor bumps only the protocol envelope VERSION to2;
POLICY_VERSION remains1. Disarmed requires `closed_generation`: explicit null
for verified Missing+Absent, exact unsigned N for verified Closed(N)+Absent.
The natural `transaction::observe` producer, completed Disarm(N), exact closed
retry and reconcile preserve that same fence. No effect ordering, root-state
schema, enrollment, locking, ownership admission, recovery policy, rule or unit
change occurs. The non-JSON root recovery token remains unchanged.

Existing strict duplicate/escaped-alias, unknown-field, type/depth/8KiB and
typed-reserialization checks reject omitted floor, malformed scalar and oldv1;
there is no compatibility fallback. Armed retains its typed generation;
Emergency/manual error never admits a connect. A disarmed response is not a
fixture-created floor proof: the runtime consumes the field from the same
healthy current-policy Status exchange. Real port authentication and whole
deadline enforcement are still prerequisites before activation.

## Executable ordering and retained failure boundary

Read/validate current desired state and owned emptiness, then trusted protected
core preflight and fixed Status. Choose checked
`max(desired.generation, closed_generation.unwrap_or(0)) + 1`, retaining extra
headroom for the later disconnected-intent generation. Exhaustion never wraps.
Prepare without core start, then durably reserve that generation in the SAME
schema while desired remains disconnected. Arm(full,N) must return exact
healthy Armed(N) before desired connected(N), core Start, owned verification
and commit. No cached state or mismatched reply authorizes progress.

Explicit Disconnect persists disconnected(N+1), then stops/discards and verifies
owned core/controller/managed-TUN emptiness BEFORE Disarm(N). Success requires
healthy Disarmed with exactly closed_generation=Some(N), not merely absence.

Each effect/callback consumes phase state BEFORE entry. Errors, late/channel
loss (port error), mismatched policy/generation/health and unexpected unwind
poison or leave an in-flight barrier. No resend, reconnect, Status-as-repair,
compensating Disarm, direct fallback or Drop recovery occurs. Post-Arm failure
reports manual recovery while retaining protection. The original executor/port
graph stays held while alive and is deliberately forgotten on uncertain or
abandoned-armed Drop; copied receipts do not replace owners. Fresh or positively
closed graphs may drop normally. This is not descriptor survival after process
death or a real-client capacity/lifetime acceptance result.

Only failed preparation or uncertain desired reservation BEFORE ANY Arm can
discard prepared local data. No generation rollback/reset follows an uncertain
write. A completed reservation remains consumed after unknown Arm; a later
fresh candidate cannot treat armed Status as a fresh disarmed floor.

## Focused gates and remaining work

Separate HOME target/TMP, at most4 jobs. Selected tests are pure protocol,
symbolic producer and injected lifecycle/filesystem controls; no real socket,
backend/core, namespace, ignored/native/kernel or VM test is selected.

| Gate | Required behavior |
| --- | --- |
| Natural floor producer | Missing/null, exact Closed/Disarm/retry/reconcile floor including0/MAX; no effect sequence change |
| Strict v2 wire | Explicit floor required; duplicates/escaped aliases/oldv1/overflow/wrong type/extra fields refused |
| Protected connect | Reserve disconnected first, exact Arm before connected write/core; greater than both counters |
| Failure cuts | Every post-Arm core/write/reply error retains protection, no compensation; original-owner Drop counters stay0 |
| Explicit Disconnect | Disconnected write then owned-empty, matching Disarm/floor; failed cleanup or lost reply refuses success |
| Ordinary lifecycle | Existing unprotected tests unchanged; no default candidate caller or native readiness grant |

Local checkpoint: 11 protected controls, 22 unchanged ordinary lifecycle
controls and 21 protocol/floor/transaction controls passed (zero ignored).
Candidate and unchanged default runtime checks, service-feature SOURCE check,
strict candidate all-target Clippy, package-scoped formatter, diff and relative
links passed. Cargo.lock changes only runtime's optional dependency on the
existing path crate; default runtime tree excludes NetGuard. These are source/
inert results, not a full workspace or real effect/transport gate.

Real authenticated client with fixed socket/peer/original deadline and bounded
resource lifetime, native fixed mark/omavless0 config and complete socket-path
coverage, coordinator registration/revision/replay, broader protected mutations,
restart reconciliation, provisioning and installed/physical gates remain open.
No working product kill switch or new orphan-adjudication guarantee is claimed.
