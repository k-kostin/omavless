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
