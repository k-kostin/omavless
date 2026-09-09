# R5 native plugin read bridge

This checkpoint connects the real `backend.sh` launcher and QML consumer to
`ui.snapshot`. It is a **read-only frontend checkpoint**, not complete R5/R6,
not login activation and not permission to delete the Python reference.

## Dispatch and failure behavior

When the native package is present, `omavless plugin target` reads the existing
canonical marker/selector without creating directories, repairing permissions,
calling the daemon or changing ownership. Only committed, consistent Legacy or
Rust state permits dispatch; preparing, rollback, stale and unsafe state refuse.
Repeated reads detect intervening changes but are not an atomic reservation.
The existing Python mutation lock and native method admission remain the final
owner checks. This is not a hostile same-user filesystem containment claim.

`backend.sh` preserves exact argument boundaries. Legacy dispatch retains the
existing Python command surface. Rust dispatch accepts only exact `status`,
mapping it to `plugin snapshot`. No unsupported operation or daemon error falls
back to Python. Exit 70 means native read-only/unavailable; 71 means ownership
cannot be selected safely. QML uses these codes, not raw errors, to enter the
blocked/read-only view even before the first successful snapshot.

Marketplace installs still need no Rust package. When `omavless` is absent,
legacy dispatch is allowed only after proving marker/selector absence through
accessible non-symlink directories. Existing artifacts, unreadable paths,
symlinks and invalid XDG paths refuse rather than guessing. Conservative path
refusals are intentional; the launcher does not parse ownership JSON itself.

## UI meaning and privacy

The native view is separate from legacy status. It shows private display names,
protocol labels, desired intent and explicitly cached lifecycle state. It never
turns selected intent into a live-connected claim, invents core/TUN readiness or
asserts that the tunnel is disconnected. Live health is visibly unavailable.
Connection/import/edit and legacy telemetry/notification paths are gated.
Loss of the daemon retains a stale-metadata warning; only a fully valid legacy
status can restore the legacy view after a legitimate rollback.

The JS parser accepts only the declared bounded snapshot fields. Unsupported
versions, extra fields, unsafe strings, duplicate records, out-of-order same-
instance revisions and malformed metadata refuse without echoing input. It
rejects integer tokens above JavaScript's exact-integer range rather than
rounding a revision/generation. Rust protocol limits remain unchanged. Existing
plain-text sinks render names without markup interpretation. Names/IDs remain
private UI data, not a general secret-scrubbed support report.

English/Russian labels cover the native read-only and failed-refresh views.
Technical desired/actual enum tokens remain identified as runtime metadata;
provider and profile content is never translated.

## Acceptance

Deterministic gates include real launcher execution with synthetic executables,
real `plugin target` CLI tests against isolated XDG files, and the production JS
parser and Service function tests. Full Python/Rust/parity/QML/i18n checks apply.

Try Omarchy acceptance uses an isolated temporary HOME, XDG state/runtime,
synthetic store and a real Rust daemon with disconnected intent. A separate
Quickshell config loads the exact plugin sources against installed Omarchy
Commons/Ui imports. This exercises the real launcher, native Unix socket and
QML consumer without modifying the user's real ownership marker, fixtures or
native service policy. Required states: successful snapshot, failed daemon
refresh, restart/new instance, blocked mutations, English/Russian rendering,
scrolling with synthetic records, and close/reopen. The ordinary installed
legacy path must also remain healthy on the same candidate.

This fixture does not prove installed native package activation, live VPN
health, connect/disconnect through native QML, login triggering, or Python-
absence. Those are still required before full frontend cutover and R6. Exact
candidate SHAs and executed results belong in the PR acceptance record.
