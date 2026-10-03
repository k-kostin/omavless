# T3 inactive close-connection admission model

This checkpoint is a synthetic Rust model compiled only by unit tests
(`connection_close_admission`). Existing `runtime.connection_rows` still discards
core IDs. No method, capability, dispatcher, CLI/TUI action, HTTP DELETE, host
adapter, persistence or package change is added.

## Identity and confirmation

One owner-memory snapshot holds at most 128 private identities: bounded core ID,
synthetic incarnation fingerprint and binding to the private displayed target.
Identical destinations are not identity; row index/order never selects a target.
Duplicate IDs invalidate the whole candidate. The conservative synthetic ID
grammar is not a claim about Mihomo's schema.

Only independent opaque 32-byte handles leave the registry model. Preparation
creates a separate one-use ticket for that handle. There is one pending
confirmation; another preparation revokes it. A future UI must visibly identify
the target and explain that closing this connection does not disconnect the VPN
or prevent applications reconnecting. No UI or human-intent proof is implemented.

Exact context comprises daemon instance, ownership generation, desired generation
and private desired/profile/mode digest, canonical revision, store/config digests,
child incarnation, authenticated peer PID and controller device/inode. Connected
committed ownership, fresh owned-core/config/profile/TUN facts, idle mutation
admission and clear recovery barriers are required. These are injected synthetic
owner facts, never client-selectable proof. PID reuse cannot preserve incarnation.

Expiry is five seconds of injected monotonic time; equality refuses. Preparation
does not extend expiry. Backward time or a changed clock epoch permanently
invalidates the registry. Refresh, including failed refresh, invalidates old
handles/tickets. Leaving/cancelling a view uses invalidation; stale context and
unavailable facts also clear it. Production entropy must be owner-controlled
CSPRNG output; deterministic bytes here are fixtures only. Missing, zero or
duplicate entropy refuses. The lifetime ledger refuses after 1,024 tokens without
eviction or reuse. A reviewed wire representation remains future work.

Confirmation compares exact pending handle/ticket and fresh private identity,
then consumes handle/ticket even if the target disappeared or its incarnation or
display changed. At most 128 final refusal receipts are retained without eviction.
Exact retries replay only the same refusal; different target/ticket with the same
operation ID conflicts. Context/freshness still gate replay. Private identity and
context have no serialization/formatting implementation; registry/token Debug is
fixed redacted text. No core ID, private digest or ticket enters existing output.

## Execution remains blocked

Even fully matching confirmation returns `CoreIdentityGuaranteeMissing`; there
is no success or executable permit type and no bypass flag. GET then DELETE of
an ID cannot exclude disappearance and reuse between calls. Activation needs
verified per-core-incarnation ID non-reuse or a core-side conditional close that
atomically checks incarnation. Neither guarantee has been established here.
The synthetic incarnation is a test input, not an implemented Mihomo guarantee.

### Bundled-core source audit, September 30, 2026

The corresponding source inside the paired x86_64 `omavless-dns` 0.9.5-beta.1
package (package SHA-256
`1e99069336b092a258f7040507ad0e0e23f3f631a9400160e649ddfc57fef027`)
identifies Mihomo v1.19.31 at
`ab405bad5beeeac8b003bb01f60f134f6df54471`. This is an audit of that
specific bundled core, not a guarantee about later or system-installed cores:

- `tunnel/statistic/tracker.go` creates both TCP and UDP tracker IDs through
  `utils.NewUUIDV4()` and presents them as UUID strings.
- `common/utils/uuid.go` fills the UUID from `crypto/rand.Read`, but does not
  inspect its result. More importantly, neither this constructor nor the
  tracker manager records all previously issued IDs to enforce non-reuse.
- `tunnel/statistic/manager.go` stores trackers by ID; `Join` stores under that
  key and `Leave` deletes by that key. The code does not establish a monotonic
  per-core incarnation token or collision rejection.
- `hub/route/connections.go` handles GET through `Snapshot()` and DELETE of
  `/{id}` through a separate `Get(id)` followed by `Close()`. The DELETE accepts
  no expected tracker incarnation and contains no atomic comparison with an
  earlier GET result.

Random UUIDs make accidental reuse improbable under normal conditions, but
probability is not the per-incarnation non-reuse or atomic conditional-close
*guarantee* required by this security-sensitive mutation. An extra owner-side
GET, matching displayed destination, matching creation time or a short timeout
does not close the intervening replacement window. The new synthetic ABA test
records that even identical pre-DELETE readback yields no permit. Keep the
existing read-only connections view; do not route a TUI button to Mihomo's
DELETE endpoint or quietly treat this source audit as a passed activation gate.

A future core-side solution needs an atomic conditional close tied to an
opaque per-tracker incarnation (or a reviewed, enforceable non-reuse contract),
with an explicit mismatch/not-found result and a matched-core package receipt.
Its API, failure semantics, upstream/fork maintenance and installed privacy
review are separate slices; none is implemented here.

Before activation, separately review private action projection/wire parsing,
owner-lock/shared operation-ID collision integration, trusted observations,
monotonic clock/entropy acquisition, revocation/cancellation wiring, bounded
detached controller work and immediate pre-write peer/socket checks. Permit only
a fixed semantic single-connection effect with owner-resolved identity: no raw
caller ID, path, HTTP method/header, deadline or close-all fallback. Lost replies
after future effects remain outcome unknown; later absence does not prove that
request caused closure. Never blindly resend an uncertain effect or change
desired VPN state as a consequence of closing one connection.

## Evidence boundary

### Inactive peer-bound conditional transport continuation

`dev/t3-conditional-owner-adapter` adds a separate review-only Mihomo patch and
Rust transport candidate; the normal test-only admission registry above still
returns `CoreIdentityGuaranteeMissing`. No production effect permit constructor,
public method, client-provided controller ID or TUI close button is added.

The exact upstream source remains v1.19.31 at
`ab405bad5beeeac8b003bb01f60f134f6df54471`. The core assigns non-reused
manager-lifetime incarnation tokens and conditionally closes the retained object
under atomic ID/token comparison. A fixed typed ABI/readiness endpoint reports
Running explicitly. This matters because upstream publishes config/rules and
listeners before Running, and CONNECT 200 precedes actual tunnel admission.
The handler also refuses effects while suspended/loading. Protocol readiness
is not immutable package attestation or concurrent reload serialization.

The inactive Rust session retains a revocable parent-owned child identity and
directory/socket descriptors, exact UID/mode/inode and per-request peer PID/UID.
Each selected target retains a non-reusable Arc session identity; even a new
session for the same numeric PID/socket rejects an older selection. Discovery
strictly parses bounded raw JSON, duplicate keys/IDs/tokens and canonical tokens;
private IDs/tokens never have a public formatting or serialization path.
Only fixed capability/snapshot reads and one semantic conditional POST exist.

Pre-write child/socket/session/ABI failure sends no effect. After the first POST
write attempt, lost/partial/malformed responses are Unknown and never resent.
Empty exact typed status receipts distinguish Closed/Missing/Changed/Unsupported;
a generic 404 page does not establish a missing target. No DELETE, close-all,
desired-state change or inference from later absence is used.

Synthetic subprocess tests cover stale session, waitable child death, replaced
socket, ABI mismatch/duplicate fields/suspension and lost/partial replies. The
opt-in real patched-core test uses a separate loopback-only core (TUN/DNS off)
and two echo tunnels: wrong token preserves both; matching token closes only
the selected original while the other remains usable. Exact source, patch and
binary hashes and host/VM repetitions belong to the owning PR. They do not
activate installed packages or establish normal owner-lock/revision/operation
admission, detached scheduling, ABI package attestation or installed EN/RU UI.

The [managed composition gate](../../tests/core_connections_adapter/README.md#exact-managed-dns-composition-gate)
exports exact managed-DNS/sing-tun patch snapshots and compiles them together
with this conditional protocol using production `with_gvisor`/CGO-off tags.
Its Go matrices and real synthetic loopback close check do not replace DNS
broker/interoperability acceptance or a root-owned matched-package receipt.
Local archive attributes are excluded through an isolated export repository;
the test-only short-socket overlay is reversed before the binary build.
Exact toolchain/binary and optional Rust transport repetitions belong to the
owning PR, not the accepted package allowlist.

### Inactive detached child-lifetime continuation

`dev/t3-detached-core-research` replaces the transport's mutable owner borrow
with owned descriptors and a retained, non-reusable `Arc` lifetime. `Child`
remains exclusively in `OwnedCore`. The optional research lifetime is revoked
under a short gate before the existing stop/drop signal and reap sequence.
Revocation shuts down the registered socket, releases the gate, then performs
bounded child cleanup. An old lifetime can never become Live again, including
after a new child binds the same controller path. Only a Live gate permits
`waitid(WNOWAIT)`; a revoked worker never probes or signals the old numeric PID.
An exit or child-proof failure permanently revokes the lifetime immediately,
whether observed by the detached transport or `OwnedCore.running`; unexpected
external reaping cannot allow a later reused PID to restore eligibility.

The same gate holds one session reservation and serializes cancellation,
the first nonblocking effect write attempt, each partial write and final
receipt acceptance. All sockets remain nonblocking. Read/write waits occur
outside the gate and owner, with one three-second monotonic session budget
which discovery and close cannot renew. This budget is research transport
lifetime, not proof of the separate five-second human confirmation contract.
Before the first effect attempt, cancellation/refusal sends no effect. From
the first attempt onward (including EAGAIN), cancellation, timeout, panic,
teardown, lost/invalid reply or proof loss is Unknown and never resent.
Restoring a failed identity proof cannot turn an attempted effect into a
pre-write refusal. A complete typed receipt plus final proof must atomically
finish before cancellation; a later cancellation cannot rewrite that receipt.
The deadline is rechecked inside the write gate after identity proof and
again during final acceptance, so delayed admission never sends an expired
effect or accepts a late receipt.
Each session admits only one close and cannot attribute an earlier receipt to
another selection.

An inactive one-worker scheduler reserves under a short scheduling section
and moves only the owned transport outside it. A bounded result channel never
calls back into the owner from the lifetime gate. Spawn failure and unwind
release their exact reservation. Dropping a result handle cancels without
joining an unfinished worker; capacity stays held until worker completion.
The real subprocess barrier tests compose the existing mutation coordinator
with the owned child and prove status/revision access and prioritized urgent
disconnect during capability/reply stalls. They also cover pre/post-attempt
cancellation, parsed-204/cancel ordering, transient socket restoration,
actual partial-write identity loss followed by restoration,
stop/drop/new-child refusal, unexpected external reaping, deadlines and
spawn/panic/drop cleanup. These are actual owned child/Unix-controller effects,
not installed production admission or private-provider evidence.

Normal operation-ID collision/replay retention, owner/revision/store/config/
desired-state admission, opaque row/confirmation registry, immutable matched
core package attestation, registered scheduling/IPC and installed EN/RU UI
remain separate activation gates. The registry still issues no permit; the
candidate permit still has no production constructor. No package pin,
production method, desired VPN state or user-facing control is changed.

Focused synthetic tests cover every context component, admission/recovery gates,
expiry/clock discontinuity, refresh and identical displayed targets, disappeared
or reused IDs, wrong/cancelled/replaced confirmation, malformed/duplicate IDs,
token/receipt bounds, entropy failures, replay conflicts and privacy. This is new
Rust-only contract work, not Python migration or live controller/UI acceptance.
Full static/Rust results and exact head belong to the PR. No installed/VM/host
or network changes are made. Live identity and installed interaction acceptance
remain required before any activation.
