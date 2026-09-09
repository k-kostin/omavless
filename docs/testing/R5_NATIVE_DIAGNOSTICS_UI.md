# R5 native diagnostics presentation

Draft frontend slice, based on native subscription UI (PR #216). Main and the
published plugin are not changed by this branch. Rust diagnostic collection
already exists; this slice changes only the fixed launcher and QML/test layer.

## Boundary

Settings opens the existing AdvancedDiagnostics page in native mode. One
fixed `native-diagnostics-summary` launcher executes `omavless diagnostics
summary`; arguments are refused. The full response is checked before reuse of
the existing plain-text rule/provider presentation. Limits: 256 KiB UTF-8
frame, 2,048 rule rows, 256 provider rows, bounded fields/counts. Unknown fields
and malformed output fail closed with a fixed localized unavailable notice.

This is an independent read-only sample. The existing response does not carry
instance identity; it must not update connection health or establish atomic
coherence with another request. A locally observed daemon change invalidates
the sample. Page-lifetime tokens discard late reads; disposable collectors
release output on every completion. Provider mutations remain hidden/disabled
and their signal path remains guarded under native ownership.

The original page retains search, rule filtering, its 300-visible-rule cap,
inset scrollbar and back/refresh controls. Native settings no longer overlap
the diagnostics page. English/Russian copy explains the independent-sample
scope; native stable provider status enums are localized, never provider data.

## Initial Try Omarchy checks (ARM64, 2026-09-09)

- Real Quickshell disconnected read: fixed unavailable state, no stale rows.
- Synthetic bounded result rendered through the production parser/callback:
  English/Russian top and bottom, twelve rules, one provider; plain markup
  remains literal, no provider-update button, only one inset scrollbar.
- Close clears the native sample; reopen issues a fresh request.
- Focused parser/Service and launcher contracts, full reference suite and
  ordinary syntax/manifest/plugin checks are recorded against the PR head.

Screenshots remain private; synthetic foreground does not make the desktop
background shareable. No real subscription/profile identity enters this report.

## Discovered installed-core blocker

On the installed Rust binary from PR #216, connection and ownership observation
passed (one owned core and one TUN), but the existing diagnostics collector
returned `capability_unavailable`. Metadata inspection found Mihomo's socket
mode was 0666 inside the private 0700 directory. The readiness selector accepts
that combination, whereas diagnostic reads correctly require 0600.

Do not weaken diagnostic permission checks. An owning startup normalization
fix and installed-core regression are tracked separately. Every live attempt
here disconnected afterward with zero core/TUN. Until the owning fix and
exact-head connected check pass, this is not successful connected diagnostics
acceptance or completed R5/R6.
