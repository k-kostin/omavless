# T3 bounded selected-set close

Source specification under implementation; no installed, default or public
activation is claimed. This adds a separately authorized set operation to the
[Product epoch](T3_PRODUCT_WITNESS_EPOCHS.md) boundary. The existing single-close
API and its one-effect Session contract stay unchanged.

The Product-feature-only entry is
`omavless tui --developer-conditional-close --selected-set`. The ordinary TUI
and the existing single-close entry retain their behavior.

The explicit set workspace selects 1–128 exact opaque members from one inert
preview. Select all means those previewed members, never future arrivals or the
ID-free Connections table. Navigation and membership changes are local. The
entire ordered set is reviewed locally with scrolling before Enter requests
preparation. A second explicit Enter confirms; preparation starts the unchanged
five-second deadline. Final discovery must freshly match every
member's original incarnation, display and owner before any write.

Only Product registration may expose
`development.connection_sets.snapshot`, `.prepare`, `.confirm` and `.receipt`.
They use scope `development_owned_connection_set_close`, the existing private
authenticated framed socket and its bounds. Prepare accepts an exact ordered
`handles` array and returns matching `rows` plus one `ticket`. Confirm binds that
array, ticket, instance, expected revision and original operation ID. Empty,
duplicate, oversized, foreign, stale or malformed sets refuse.

One typed set permit, one original Session and one shared worker reservation
authorize the complete set. Existing conditional ABI1 requests address each
immutable member at most once. The whole operation retains the original
three-second budget and per-chunk durable/image/lifetime proofs. It stops at
the first non-Closed outcome, uncertainty, cancellation or expiry. It cannot
renew a deadline, reset Attempted, call Single Confirm repeatedly or substitute
targets. Attempt flags are set under the write gate and survive unwinding.

There is one final original helper Finish and one whole Session Drop before
publication. Per-member replies are provisional until the final proof succeeds.
A failed or late final proof makes attempted members Unknown; members with no
POST attempt remain NotAttempted. A typed set completion may renew only after
all-Closed plus the existing identity/Finish/zero-flight/after-Drop conditions.
Known Partial must never be converted into Single Closed. Non-Closed retains
the current conservative Product factory fence; ordinary Disconnect and config
operations remain available.

Single and Set share the existing non-evicting 128-entry ledger and operation-ID
namespace. The ledger stores only a semantic digest, count, ordered outcome
metadata and original receipt revision, not handles, displays or controller
identifiers. Exact replay precedes fresh authority/capacity checks; wrong-family
receipt reads refuse. Completion advances revision once if any member is Closed
or Unknown, including Partial, while retaining the successor-revision guard.

Finished receipts have exactly one indexed outcome per held member. Aggregates
distinguish all-Closed, known Partial, Unknown and before-write refusal; Pending
never publishes provisional successes. The client validates cardinality, order
and aggregate consistency. Pending or ambiguous work permits only original
receipt reads and client exit. Verified Unknown is sticky. A known terminal
result is spent; no automatic retry or rollback follows a partial result.

EN/RU review, result scrolling, exact target selection, 70×24 minimum/60-column
guard, cancel, expiry, private plain text and no hidden activation follow the
[UI contract](../roadmap/UI_UX_CONTRACT.md). Synthetic source evidence remains
distinct from installed Product acceptance. A new image cannot reuse or repair
an existing create-only root enrollment; legitimate image upgrade is a separate
admission dependency.

## Explicit client Confirm-response loss

The Product feature additionally admits exactly
`omavless tui --developer-conditional-close --selected-set --discard-first-confirm-reply`.
This is an opt-in development client cut, absent from default builds. It does
not exist as a CLI mode in a Backup-only build, even when the Product feature
is also compiled; that combination refuses the selector and omits it from help.
It does
not change the daemon, wire schema, helper, permits, native deadlines or normal
Single/Set clients. No environment variable activates it. Extra, reordered or
non-selected-set uses of the flag refuse before opening the terminal or socket.

The normal workspace still obtains and reviews the preview, prepares the exact
members and submits its original Confirm. The cut is consumed before the first
Confirm's fallible directory, socket, peer, framing or write checks. It uses the
same authenticated transport and bounded frame writer. Only after the complete
request write and successful write-half shutdown, within the existing transport
deadline, does it close that original connection without reading a response.
It returns Unavailable to the normal workspace. It never reconnects, resends,
rearms or substitutes an operation. A second Confirm through that cut refuses
before any I/O; unrelated methods, including the original receipt read, keep
their normal transport. A partial write or shutdown failure remains uncertain.

The normal UI keeps the submitted operation and blocks refresh, new preparation,
Enter and Esc. Only an explicit `u` for the original receipt or client exit is
available. Transport ambiguity does not manufacture a verified Unknown receipt:
a valid known receipt may resolve it, while an actually verified Unknown remains
sticky under the existing contract. Backend response loss and transport-only
unresolved UI are distinct evidence from native worker uncertainty.

After terminal teardown, the CLI writes one fixed public line to stderr:
`omavless.reply-loss.v1 stage=reply_discarded full_request_written=true`.
The other closed stages are `not_attempted`, `attempted` (both with
`full_request_written=false`) and `request_written` (true). No target, ticket,
operation ID, endpoint or exception is included. False is a lack of full-write
proof, **not** proof of no effect. Only `reply_discarded` proves this cut reached
its intended boundary; neither this line nor CLI exit proves worker completion
or a Close result. Early client exit may report an earlier stage. The diagnostic
is at most 70 ASCII columns and is never printed into the active alternate screen.

Source controls cover the shared owned AF_UNIX path, one complete Confirm and
same-operation receipt, no response read, no second Confirm, default/unrelated
calls, path/peer/framing refusal, short/incomplete writes, half-close failure,
timeout and strict CLI selection. EN/RU workspace controls preserve the original
operation across lost response, expiry and constrained width, then validate the
indexed receipt. These controls do not establish installed acceptance.

The complete installed journey requires the new runtime package/image admission
through the [normal explicit upgrade](T3_PRODUCT_IMAGE_UPGRADE.md), actual held
selected originals, one timely Confirm, unresolved UI with mutation keys blocked,
one original receipt-only resolution, original selected EOF/unselected fresh
traffic, next-epoch readmission, ordinary Disconnect/Off and protected-state
preservation. The exact post-teardown diagnostic and original CLI status must be
captured through a qualified normal terminal completion route: inner CLI stderr
belongs to the terminal PTY and is not implicitly the outer Foot launch's SSH
stdout. A fixed public terminal wrapper may retain only the restored terminal
long enough to inspect the closed line and original status, then exit normally;
no private UI/payload log, raw RPC harness or diagnostic file is required.
New source, package, installed, own-PR/merged-PUSH and final distribution gates
remain separate. No actual lost-reply, sticky-Unknown or whole-T3 PASS is claimed
by adding this capability.

## Explicit acceptance image: failed final proof

`acceptance-connection-close` is a separate default-off runtime feature, not a
public/default activation. Its closed selectors are `daemon --product-image-witness
--acceptance-close-before-attempt` and `daemon --product-image-witness
--acceptance-close-final-proof`, and `daemon --product-image-witness
--acceptance-close-between-members`, with exact argument order and no extras.
Backup-only builds refuse all three. Normal Product selection remains unchanged.
The local packager selects one of three fixed acceptance unit files; it records
`featureSelection=product-image-witness-acceptance` and the closed cut name in the
private schema4 build identity. No helper wire field, endpoint or privilege changes.

The same current Product owner shares one arm across its captures. Inert preview
retirement cannot consume it; the first set-authorization transfer consumes it
before fallible authorization. The before-attempt cut shuts down only the original
bound helper channel before the first POST's proof. The final-proof cut does so
at finalizing `effect_lease`, after its fresh image/package checks and before the
ordinary Client Finish. The ordinary Observe/Finish error and unchanged settlement
path determine the result. No fake outcome, attempt bit, native response, receipt,
retry, rearm, sleep or deadline extension is supplied by the cut.

Source controls use real SEQPACKET lifetime checks and the existing owned
controller/owner fixture with explicitly memory-only image callbacks. They are
not real Product authority or installed failure evidence. Installed acceptance
requires this declared image's normal package/root admission, exact cut-reached
marker, original selected TCP witnesses and unaffected control, the same indexed
receipt remaining Unknown through explicit receipt reads, no renewal/resend,
original worker/helper/client completion and protected ordinary Off return.
A before-attempt falsifying control must instead preserve every original flow
and return NotAttempted. Instrumented evidence proves the real failed-final-proof
handling; it does not assert an organically occurring failure in an unchanged
runtime image, default/public readiness or whole T3 closure.

The between-members scenario shares the same one-shot set arm. After the first
real Closed member of a multi-member set, the worker waits on one owned nonblocking
socket pair released by its selected owner's fixed SIGUSR2 handler. The pair is
created once for that owner, not per preview; its registration is dropped with
its owner. There is no pathname, arbitrary FD, message payload, effect permission,
sleep or renewed deadline. The wait uses the original absolute native deadline,
holds no proof/owner/migration lock and fails closed on preloaded input, timeout,
EOF/error or repeat entry. Normal proofs run again before the next write and at
Finish. The caller must bind the exact original daemon pidfd and wait for the
closed `between_members barrier=entered` marker before the single release signal.
A release authorizes no operation; the original normal guards still decide.
This declared instrumentation can align real member disappearance and a competing
normal client while the original worker is held. Source fixture Partial is not
installed real-flow Partial or shared-owner client acceptance.

Acceptance markers use one nonblocking send to the existing stderr socket, with
no Rust stderr lock and no global descriptor-policy change. A full or unsupported
output drops the marker; this never blocks a native proof/operation. Missing or
partial marker delivery is not cut qualification: the installed driver must bind
the exact full journal line to the current original owner invocation. Source
controls cover saturated owned output and contended Rust stderr separately.

Before binding the instrumented owner, the explicit acceptance entry requires
stderr to be an AF_UNIX stream socket accepting an empty nonblocking probe. It
emits no cut marker during preflight and cannot guarantee later delivery. This
refuses unsupported terminal/file/pipe redirection before creating that owner;
the fixed packaged units retain ordinary journal routing. Default/Product
entries do not gain this requirement.

### Installed Product guard probes

Ignored test entries exercise the existing Product primitives on their fixed
installed paths; they are not runtime commands, helper wire methods or authority
exports. The parent/UID test requires the actual enrolled core and its daemon,
plus one owned inert normal selected-set TUI. It holds original pidfds, proves
the positive daemon/core binding, independently checks the TUI's enrolled image,
four UIDs and namespaces, then requires exactly `Refused` for its foreign parent.
Its real root SEQPACKET peer control checks the same UID admission primitive;
this alone is not a packaged helper-listener rejection or helper-exit result.

The read-only maintenance test acquires the actual exclusive lock before and
after its own Product lease, requiring exclusive refusal while that lease exists.
It requires no other live leases, creates no record and issues no upgrade.

The retained-object test consumes real Product Roots into a private diagnostic
holder and drops the lease before ordinary managed package/root upgrade. It
cannot produce Roots, a Binding, session or permit. Original runtime/enrollment
objects must then refuse; an unchanged core is explicitly a positive control.
A separate normal-UID test retains the actual managed core/broker/source-receipt
Chain/Member objects and checks their independent package-object guards across
that upgrade. These waits accept one fixed stdin inspection byte within 180s;
they are diagnostic lifetimes, not extensions of product deadlines.

These actual retained-object results, exact source composition/poisoning tests
and normal installed Product refusals have different evidence layers. They do
not claim a live authority session survived an upgrade, nor waive an unexecuted
probe. Expiry, unavailable observation, absent originals or ambiguous processes
cannot count as the required identity refusal.
