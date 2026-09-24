# T2 MVP completion candidate

Status: development candidate on `dev/t2-mvp-completion`, based on the accepted
checkpoints in [rc/0.9.0](RC_090.md). This is not a main update, release, or a
completed installed-acceptance claim. Record the final tested source, package,
frontend and remaining gates in the owning PR before marking T2 complete.

## Product boundary

The terminal workspace uses the existing canonical Rust owner. The QML plugin,
CLI and TUI share its private semantic API; the terminal owns neither a tunnel
nor a second desired-state machine. Closing the terminal does not disconnect,
stop the runtime, cancel a background job or undo an accepted command.

This candidate finishes the selected T2 surface: status, grouped profiles,
search/favorites filtering, explicit connection/mode controls, traffic/counts,
saved profile categories, diagnostics, subscription refresh, bounded profile
checks, session activity/settings, and Omarchy Open app. Import/edit/routing
management remains available in the plugin. Full connection tables, raw logs,
persistent TUI preferences, scheduling and a graphical application are not T2.

## Keyboard paths

| Surface | Keys and effects |
| --- | --- |
| All pages | `Tab` / `Shift+Tab` changes page; `?` shows help; `q` / Ctrl+C closes only the client |
| Profiles | Arrows or `j` / `k` selects; Home / End jumps; `/` searches; `f` filters favorites without changing them |
| Profile actions | `c` Connect, `d` Disconnect, `1` Full VPN, `2` Routing, `3` Direct; Enter confirms the named action, Escape cancels |
| Profile checks | `t` checks the selected non-missing profile; `T` checks every non-missing profile, not merely the filtered rows; both require confirmation |
| Subscriptions | `n` / `p` chooses the visible action target, including empty feeds; `s` refreshes it; `S` confirms refresh-all |
| Profiles → subscription | `s` refreshes the selected profile's parent subscription; standalone profiles cannot target a different feed |
| Checks and updates | `g` reads the current operation receipt; `x` requests cancellation after confirmation; cancellation is not complete until the owner confirms it |
| Settings | `,` opens Settings; `l` cycles automatic/English/Russian, `t` toggles automatic/default palette, `0` restores automatic choices |
| Read-only pages | Arrows / `j` / `k`, Home / End scroll; `r` reloads local observations; Escape returns to Profiles |

Search and confirmation overlays keep priority over global shortcuts. Resize
must preserve selection; below the action minimum of 70×24 cells, actions are
disabled rather than hidden behind an unsafe confirmation. Selection is never
Connected state. The verified connected identity stays visible when a selected
row differs or filtering hides the connected row.

## Refresh and operation outcomes

Single refresh uses the existing subscription mutation, including validation,
atomic replacement and active-profile reconciliation. Refresh-all uses the
existing owner-side start/poll/cancel scheduler, not a TUI loop over providers.
Empty saved subscriptions remain selectable. Saved-list age is a persisted
last-success timestamp, not evidence that the latest request succeeded. The
latest single-refresh attempt per current subscription is window-local; it is
not a persistent provider/error journal. Refresh-all retains its own receipt
on Checks and updates.

The TUI pins starts to instance, revision and the displayed target. Only one
local job is active; ordinary mutations are blocked while its outcome is
unresolved, except the explicit urgent Disconnect path through the same owner.
Disconnect still requires its normal confirmation and freshness/fences; the
runtime owns cancellation and cleanup of conflicting auxiliary work.
Lost/malformed replies remain unknown: no new operation ID,
automatic restart, silent cancellation or inferred rollback. Receipts are
validated for method, identity, revision, progress and state monotonicity.
Polling is at most once per second, with a 30-minute-plus-10-second / 1,810-poll
watch ceiling. Expiry stops watching, not the runtime job; an explicit receipt
read is still available. Daemon restart cannot reuse an old operation handle.

## What a profile check measures

`profiles.probe` reuses the native probe scheduler, resolver and isolated
no-TUN auxiliary Mihomo executor. One explicit ID checks one profile; omitted
ID checks the runtime's current non-missing set, at most 256. The client never
receives credentials or constructs probe configurations. There is at most one
owned auxiliary probe child, no second production tunnel, and no new TUN.

The result is a median of successful HTTPS delay samples through the tested
profile using the existing three fixed public probe URLs and pinned endpoint
addresses. It is not ICMP, packet-loss measurement, exit-IP proof or complete
VPN/system-DNS health. DNS resolution failure remains distinct from a resolved
profile whose probes failed/timed out. A successful job means results were
collected, not that every profile passed. Progress can legitimately stay at
zero until final results; the UI says so.

The existing bounds remain: 256 profiles, four pinned addresses per endpoint,
64 generated targets per chunk, one auxiliary core at a time, three fixed
HTTPS rounds per chunk, bounded controller work, and a 30-minute whole-job
ceiling. Cancellation is cooperative; uncertain auxiliary cleanup is a hard
manual-recovery condition, not an ordinary failed measurement. Results are
volatile and tied to the exact runtime/store/config snapshot. Changed state
must not attach old measurements to current rows.

## Read-side privacy and compatibility

- Traffic includes rate/totals plus only the current owned core's active
  connection **count**. Unavailable is not zero. There are no destination,
  process, chain or browsing-history rows; full C1 inspection remains separate.
- Profile Details retains only allowlisted protocol/transport/security
  categories from the existing explicit private details method. Endpoint,
  SNI, raw profile link and credentials are neither retained nor rendered.
  Capability booleans mean API support, not server compatibility or health.
- Status/observation/read pages negotiate methods and reject mixed-instance or
  stale-revision data. Older packages lack the new optional methods; those
  actions/readouts remain unavailable instead of silently using another path.
- Session activity is at most 32 fixed typed events, without private names,
  IDs or raw errors. Language/theme changes stay in memory, work offline and
  do not change plugin/OS preferences or pending network outcomes.

## Launch and package boundary

Normal candidate builds now include the `tui` Cargo feature. A deliberately
headless build remains possible with `--no-default-features`; it must not claim
to contain the TUI. `omavless tui --available` reports the fixed local token
`omavless.tui.v1` without accessing private state, contacting the runtime or
starting a service. This detects the installed binary, not the frontend version.

Settings offers Open app only after that feature check. The supported fixed
Omarchy adapter is:

```text
omarchy launch or focus tui --app-id=org.omarchy.omavless omavless tui
```

All arguments are literals because the upstream adapter constructs a shell
command. No profile, subscription, path or user text is interpolated. Reopening
focuses the existing app window where supported by Omarchy. With a stopped or
incompatible daemon the TUI opens its unavailable/remediation state; this
candidate does not implicitly start the service. Missing/older application
packages show update guidance instead of a broken Open app action. There is
no download, Cargo build, package-manager invocation or privilege prompt on
this navigation path.

Local acceptance packages may use the existing `0.0.0.rCOUNT.gCOMMIT` developer
label. That is not a published 0.9.0 artifact; released 0.8.2 assets/pins and
stable main remain unchanged. Preserve a byte-verified stable package and
frontend for an explicit attended rollback. A normal local update must preserve
private data and the existing service enablement state rather than enabling
login behavior for the test.

## Completion gates

Candidate source: `02a5a13b807aab8d984f37cc49e20eab71374942`, [PR #284](https://github.com/k-kostin/omavless/pull/284).
Local gates passed: 1,098 Rust tests / 11 ignored, strict workspace/all-target
clippy, formatting, developer suite 276 tests / 2 skips, QML contracts, ten PTY
cases, 183 EN/RU keys, plugin/manifest/shell/docs checks and a headless-feature
build. Source test and both architecture package CI jobs passed.

The attended ARM64 package switch passed using developer version
`0.0.0.r626.g02a5a13b807a-1`. Installed/running binary SHA-256:
`7b95a4131e892c808efb571015622b2ac74adf1453c1e10731149a18887527f7`.
Private store bytes, existing disabled service enablement and startup Off were
preserved. Matching frontend runtime files were compared byte-for-byte, and
the plugin is enabled. The previous stable package/frontend are retained
locally for an explicit attended restoration; no automatic rollback occurs.
Installation alone does not establish the combined gates below. Final results
and exact CI head belong in the PR, without a diary of individual prompts.

Deterministic tests cover client target/revision fences, missing capabilities,
job start/poll/cancel/unknown results, probe bounds/privacy and count-only
controller projection. They do not substitute for the following combined
installed gates, which remain pending for this candidate:

1. Exact candidate package/frontend identity, feature discovery and Open app
   launch/focus, including old-package unavailable and daemon-down behavior.
2. English/Russian wide/narrow rendering, all new confirmations, scrolling and
   stale/error states, reviewed using safe synthetic metadata.
3. Plugin and TUI agree on live state; controlled conflicting requests serialize;
   Connect/Disconnect/modes work through the same owner without duplicates.
4. Selected/all probes, subscription refresh/refresh-all, cancellation/results
   and cleanup on the actual runtime, distinguishing provider failures from
   client bugs and from absent fixtures.
5. Live traffic/count/details/diagnostics, terminal close/reopen, runtime restart
   and stale-client behavior; private Unix controller, no TCP controller.
6. Exact-head local suites/CI and an explicit sanitized result matrix. OS effects
   follow [human authorization](../testing/HOST_AUTHORIZATION_ACCEPTANCE.md).

Preserve earlier accepted checkpoint evidence where unchanged. T2 completion
does not close AUTO-1, DNS/provider follow-ups, V0 or NixOS host acceptance, and
does not authorize main/release/marketplace publication.
