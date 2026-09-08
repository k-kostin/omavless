# Native rule-provider refresh boundary

This checkpoint implements inactive, bounded provider discovery/update work.
Python `refresh_rule_providers` still owns the installed product. The Rust
adapter is not advertised through IPC/CLI, creates no worker thread, writes no
store and cannot by itself retire Python or claim completed refresh migration.

## Reference and intentional differences

The actual Python `refreshable_rule_provider_names` and `_refresh_rule_provider`
functions are the discovery/path oracle. The actual `refresh_rule_providers`
function is exercised with service/controller/store/time effects isolated.
Synthetic inputs pass over stdin; output contains only digests, classifications
and counts. No actual private store or provider is used.

Both implementations validate the entire provider map (at most 256 entries),
reject invalid names before any PUT, and refresh only case-insensitive HTTP
vehicles. Provider names are at most 256 UTF-8 bytes, exclude controls and
`/\\?#%`, and cannot be `.` or `..`. Validated private target types have no
public string constructor, Debug or serialization. Update paths percent-encode
UTF-8 bytes exactly as Python `quote(name, safe="")`; no caller method, URL,
headers, body, command or controller endpoint is accepted.

Python schedules up to four PUTs. Native work deliberately performs one per
step, using the runtime's existing shared four-permit pool. This narrows
concurrency while preserving all-target attempt and all-success semantics.
Discovery has a three-second cap, each PUT 60 seconds, and the complete job
including permit wait a 30-minute deadline. Queue saturation performs no I/O.
The whole-job bound may prevent completion of a worst-case 256-provider batch;
the outcome is failure, never success for a truncated prefix.

The native controller is fixed `mihomo.sock` beneath a trusted runtime
directory, with exact same-user `0700` directory / `0600` socket and peer-UID
checks. Nonblocking connect and remaining-budget read/write loops bound Unix
backlog and slow responses. Discovery inherits the 512-KiB controller cap;
update replies are capped at 32 KiB. Exactly HTTP 200 is accepted for discovery
and 204 for updates. Raw core messages never become public errors.

## Partial effects and cancellation

A successful PUT changes Mihomo/provider cache immediately. Later provider
failure, cancellation, timeout or stale ownership cannot undo that effect.
Native work attempts remaining targets after ordinary individual failure but
returns no all-success completion count. The legacy `rulesUpdatedAt` stamp is
permitted only when every discovered target succeeded, never after a partial
failure. Cancellation is cooperative between bounded PUTs, not a promise to
cancel Mihomo's underlying fetch. An accepted cancellation wins over a later
in-flight failure; it prevents subsequent PUTs and final stamp eligibility.

## Required registration checkpoint

Do not create another scheduler or registry. Extend the accepted #161
long-operation registry with a fixed `routing.refresh_providers` discriminant,
its 256-provider count bound, and the same one-active-operation policy. Start
must accept only current `instanceId`, `operationId`, optional
`expectedRevision`; `operations.get/cancel` remain the existing fixed methods.

The one committed owner must require connected Routing state, capture exact
ownership generation, revision, desired state, private store and active config,
and recheck before each remote step without holding owner/migration locks over
controller I/O. Disconnect remains responsive and stops future work. Completion
must close cancellation under that same owner and revalidate the exact snapshots
before an atomic compensated `rulesUpdatedAt` write. Stale completion must not
stamp current state. Remote effects remain irreversible; no rollback success
may be inferred from retaining the old timestamp. Public projections contain
counts and fixed error codes, never target names or URLs.

The present work primitive does not provide those ownership fences or stamp
transactions and must not be registered directly as a unary method.

## Local gates

Deterministic tests cover complete discovery validation, UTF-8/path escaping,
boundary caps, partial failures, shared permits, cancellation, terminal reuse,
private socket permissions/UID, non-204 refusal and slow-body deadline.
The installed-Mihomo opt-in uses an isolated no-TUN core and a synthetic
loopback HTTP rule provider. It proves loaded rule count changes after the
actual fixed Unix PUT, checks no core-owned TCP listener or TUN descriptor,
then stops the owned child. This is controller/adapter evidence on Try Omarchy
ARM64, not installed native-owner, physical network or frontend acceptance.

Exact tested head and counts belong in the acceptance PR after tests complete.
