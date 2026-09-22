# T2c: grouped profile browsing and favorites filter

This opt-in client slice builds on [T2b actions](T2_CONNECTION_ACTIONS.md).
It adds **no IPC methods, mutations, runtime lifecycle or package changes**.
Main/marketplace remain under the owner's explicit publication hold.

## Interaction contract

- Local profiles appear first, followed by subscription groups in canonical
  metadata order. Within a group, feed order is preserved. Duplicate display
  names never replace internal record identity.
- Headers show the group name and number of visible profiles. They are labels,
  not selectable rows, connected indicators or expansion controls. No fake
  arrows are added to leaf profiles. Collapsing groups and subscription
  management remain separate work.
- Arrows/j/k/Home/End move among profile records, skipping group headers.
  Selection and the existing Connect confirmation retain the exact profile ID.
- `f` toggles a **local view filter** for favorites; it does not change any
  favorite flag. It does not act inside search or a confirmation. Held repeat
  events do not repeatedly toggle the filter.
- `/` searches profile and subscription display names. Favorites and search
  compose; counts describe the current view, not a fabricated empty store.
- Esc exits search/confirmation first; in ordinary browsing it clears both
  filters and selection. A refresh that hides the selected record clears that
  selection; Connect cannot target a filtered-out profile.
- The actually connected profile remains in the header when its row is outside
  favorites/search or below the viewport. Group labels never acquire Connected.

Names are bounded/sanitized plain text and remain untranslated. English/Russian
chrome distinguishes **Local profiles** (group) from **Local** (source label).
The same 256-profile/64-subscription input caps apply; grouping adds at most one
header per nonempty group and never fetches remote subscription content.

## Validation

Eight new deterministic tests cover interleaved feed ordering, duplicate names,
navigation skipping headers, favorites without writes, combined source search,
search-letter/confirmation handling, active identity outside filters, external
unfavorite hiding a pending target, empty states and Russian plain-text names.
The full client suite has **35 tests** (14 actions, 13 read-only, 8 browsing).

Local complete Rust script: **1015 PASS / 11 existing opt-ins ignored**;
10 terminal PTYs, formatting, Clippy, feature-enabled canonical parser and parity
passed. Existing developer suite: **276 tests / 2 skips**, JavaScript/QML and
plugin validation passed. The final Local-profiles label refinement was followed
by another focused client/catalog run: **67 bounded EN/RU keys**.

Rendered Foot review used only synthetic data: EN/RU normal grouped list,
last-profile scrolling, favorites, combined subscription search, filtered-target
confirmation and empty favorites. Twelve captures remain outside Git. All were
reviewed; the changed group label was recaptured in both locales.

A separate feature-enabled client attached to installed 0.8.2 without replacing
it. Private comparisons confirmed actual active identity, favorite count, empty
filter retaining active identity and the first grouped profile's confirmation
target. The confirmation was **cancelled**, never submitted. Closing/reopening
with q/SIGTERM preserved desired/actual/revision and runtime/core process IDs.
No new authorization or VPN transition was needed; T2b's attended network
evidence retains its own exact head rather than being relabelled.

Still outside this slice: favorite mutations, collapsible groups, subscription
refresh, traffic/probes/details, theme following, Open app and default packaging.
T2 MVP is not complete merely because browsing and lifecycle controls work.
