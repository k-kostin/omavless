# T4 subscription usage presentation proposal

Status: inactive design candidate. No fetch, persistence, IPC, TUI/QML,
localization, or VPN behavior is changed by this document. Any activation
requires a separate implementation and acceptance review.

## Purpose and surface

The user goal is to inspect a provider's reported subscription usage and
expiry while managing subscriptions. The proposed primary interaction is
navigation to a selected subscription's details; it does not connect, switch
servers, refresh a feed, or mutate routing. If a compact indicator is later
added to the subscription list, its meaning must remain distinct from VPN
connection state, server reachability, and feed-refresh success. The existing
`subscriptions.list` and `ui.snapshot` contracts have no quota fields: the
details surface cannot be populated by the current runtime or TUI.

The provider controls `Subscription-Userinfo`. Treat its `upload`, `download`,
`total`, and optional `expire` values as *reported*, not measured or verified.
Do not label a VPN as healthy, usable, expired, or disconnected based on them.
An unavailable or malformed optional header must not turn an otherwise usable
feed into a failed refresh or a red fatal-state shield.

## Candidate states

| Input / event | Candidate private presentation |
| --- | --- |
| No accepted metadata on the latest successful refresh | “Usage not provided by provider”; no progress or expiry claim. |
| Valid metadata on the latest successful refresh | “Provider-reported usage”, with the observation time, bounded values and any eligible expiry date. |
| Malformed or duplicate metadata on a successful refresh | No usage claim. Keep the feed usable; do not show parser contents or account identifiers. |
| Refresh failure | Retain a previous claim only if private retention is explicitly implemented, and mark it “Last reported” with its observation time. Never imply freshness. |
| Subscription URL replaced or subscription deleted | Remove associated claims and observation time. Do not transfer them to a new account/feed. |
| Restart without an implemented private persistence contract | Usage unavailable until a new successful refresh. |

This table is a design proposal, not a statement that retention or any UI is
implemented. A future storage design must define an atomic binding between
accepted feed identity and its optional metadata; a successful refresh with no
metadata must clear an older current claim. The field must not survive URL
replacement merely because the visible subscription name is unchanged.

`total=0` is not evidence of “unlimited”, and `expire=0` is not evidence of
“never expires”. Do not create an expiry badge from zero or show a countdown
until wall-clock reliability is established. If a future UI derives remaining
bytes or a progress ratio, use checked arithmetic and suppress the derived
figure when upload plus download exceeds total, total is zero, or the values
cannot be represented safely. Raw reported counters may still be shown with
clear attribution and bounded formatting. A past date is a reported date,
not proof that an active connection must be stopped.

## Privacy, localization and review gate

Provider URLs, raw headers, account identifiers, exact quota values, and
expiry dates are private account information. Do not place them in shareable
support bundles, ordinary logs, analytics, Git fixtures, screenshots, or PR
descriptions. Any future UI should use synthetic data in review artifacts.
Provider-supplied text is data, never rich text or a translation key; bound and
render it as plain text. English and Russian labels, numbers, units, and dates
need locale-aware review. UTC Unix seconds are the interchange value; a local
date/time label must state its timezone or use an unambiguous localized form.

Before UI activation, separately review rendered English/Russian states for
missing, valid, malformed, stale, zero/ambiguous, clock-unreliable, and long
safe-name cases at the real dock/panel size. Inspect hover, focus, scrolling,
and subscription-list alignment. Verify that informational usage never masks
an actionable connection/recovery error and that a refresh does not briefly
flash a fatal VPN state. Acceptance must be tied to an exact implementation
head; these docs and loopback parser tests are not visual or live-provider
evidence.
