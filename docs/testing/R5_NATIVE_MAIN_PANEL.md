# Native familiar-panel restoration

## Purpose

The temporary Rust controls panel proved individual commands but did not provide
the established product interface. This slice restores the familiar main-panel
structure over the accepted native commands, without returning ownership to
Python or presenting unavailable telemetry as live data.

The intended first checkpoint includes the OmaVLESS hero and power control,
mode selection, searchable grouped profiles, imports and selected-profile
actions, plus settings navigation and locale choice. Verbose migration metadata
belongs in settings rather than the primary view. Subscription management,
traffic/probe/diagnostic and other unfinished native UI connections must remain
explicitly unavailable, not silently routed through the legacy backend.

## State contract

`NativePresentation.js` is a pure projection of already validated snapshots and
observations. Connected presentation requires matching instance, revision,
desired generation/mode/state and actual state, plus observed owned core,
matching desired profile, verified private controller configuration and one
core/TUN. This is local connection evidence, not an external Internet check.
Stale, absent or contradictory observations are unavailable, never a healthy
connection. The UI continues using the established fenced commands; it does
not own or duplicate the lifecycle state machine.

Private names remain unmodified plain text, never localization keys or rich
text. Search is bounded, local-only and does not mutate profile metadata.
Recovery/error controls remain accessible even when normal actions are disabled.

## Required acceptance

- Projection tests: observed versus requested state, stale identity/revision,
  missing/duplicate resources, bounded search and unchanged private names.
- Existing native import, QR, editor and action suites plus QML contracts.
- EN/RU full scrolling main/settings, grouped search, action visibility,
  pending/unknown/recovery states, keyboard focus and close/reopen.
- Installed exact candidate: familiar header, one scrollbar/gutter, equal-size
  header controls with no outer power box; import/QR/editor regression and
  ordinary connect/disconnect without new controller/lifecycle behavior.

Work in progress. Full historical UI parity and R5/R6 completion are not claimed.

## Installed checkpoint

Frontend `a19e6fc9548d7d5d0f93daf511b5723c5b248f85` installed in Try Omarchy
ARM64. Panel and NativePresentation compare byte-identically with checkout.
The existing combined Rust binary is unchanged; crate/launcher/package source
matches integration candidate fe8c57f. No runtime restart was needed for the
frontend update; the shell was reloaded. Plugin enabled, desired/actual
disconnected with Full VPN mode preserved and zero visible Mihomo/TUN.

Local full Python suite: 342 run, 341 passed, one root-only skip. Existing
native suites plus six projection and six main-panel checks, localization and
QML contracts passed. Isolated exact-production QML loaded on real Quickshell;
EN/RU main/settings captures exercised header, long names, scrolling and locale
labels. Long profile rows were compacted and recaptured. Synthetic captures
remain outside Git. The temporary runtime was stopped after acceptance.

Human installed header/power/search/group/action/focus and connect/disconnect
checks remain pending. Subscription management and other unavailable native
surfaces are explicitly disclosed in Settings; this is the main-panel
restoration checkpoint, not full historical feature parity.
