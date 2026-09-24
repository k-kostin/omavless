# Probe semantics — #272

RC candidate, not a released change. Owner includes this work in the mandatory
0.9.0 gates in [RC_090](RC_090.md).

## Code-backed audit and decision

| Observation | Existing owner / evidence | Meaning, not inferred claims |
| --- | --- | --- |
| Connected | Rust lifecycle + `runtime.observation`; `NativePresentation.js` | Owned configured tunnel, not universal Internet/DNS health |
| ICMP reply/no reply | `tun_ping.rs`; `controller_attributed_tun_icmp` | Bounded sample through the attributed TUN; no reply neither proves filtering nor a broken VPN |
| Current-route HTTPS | `connection_test.rs`; `current_route_https` | Fixed-target request may route directly; success is not TUN-egress or leak proof |
| Profile HTTPS delay | Native probe executor, isolated no-TUN core | Test of that profile under current network conditions, not ICMP ping or the active connection's health |

Confirmed plugin gaps: profile HTTPS milliseconds used the ping catalog key,
sorting said Ping, failed HTTPS was generic Unavailable, and endpoint resolution
failure was labelled global DNS failure. The native subscription page now names
HTTPS and explains the scope without moving the main-page controls. Unmeasured
rows stay unmeasured. Job completion is distinct from successful measurements.
TUI already has an explicit isolated HTTPS scope and separate failed/unresolved
labels; no second probing subsystem or speculative rewrite is necessary.

The hidden ICMP client also retained old samples after an unavailable result,
allowing the next pending sample to re-expose them as observed. Clear that window
on unavailable; never insert a fabricated loss. Existing generation/revision,
instance, target and close fences still reject stale replies. Profile results
are invalidated on owner/revision changes; saved probe preferences are unchanged.

## Policy

- Keep the owner-hidden Test and latency sections hidden. Their background ICMP
  timer is already disabled by the presentation gate; no invisible retry loop
  needs a new backoff mechanism in this slice.
- Keep explicit subscription/TUI checks and existing target/TLS/time/concurrency
  bounds. Do not silently disable a saved target or alter runtime connection
  state because a measurement fails.
- Do not relabel current-route HTTPS as protected egress. Rust V0 adaptation
  owns the separate developer-only live Full VPN/attribution gate. A new ordinary
  public attributed-HTTPS API is deferred unless that work proves it necessary;
  it is not required to correct the misleading existing labels.
- No raw log, URL, endpoint, private ID or IP enters these public explanations.

## Acceptance

Focused JS tests execute production QML handlers/parsers with synthetic receipts:
healthy tunnel plus failed HTTPS, no ICMP reply plus successful current-route
HTTPS, unavailable versus measured loss, no stale sample revival, EN/RU result
labels, untested rows and unchanged lifecycle state. Existing tests retain
disconnect/profile/revision fencing and private-result bounds.

Try Omarchy ARM64 review of implementation `31c5cc9` passed: actual production
QML rendered in an isolated Quickshell instance with the real Omarchy imports,
theme and display. The read-only synthetic transport had no real home/store,
runtime socket or network access. EN/RU subscription captures covered HTTPS
success, timeout/failure and unresolved address together; adjacent main retained
its hidden Test/latency sections and fixed controls. No clipping/overlap found.
This is installed-stack rendering, not private-provider or live-probe evidence.

The same frontend was installed atomically into the real plugin, with Panel,
Service and catalog bytes verified. Plugin remained enabled; runtime revision 1,
connected Routing, one core/TUN, zero auxiliaries and no manual recovery were
unchanged before/after. No service restart, system authorization or VPN mutation
occurred. The actual private provider batch is not repeated for a presentation
change; its positive and negative measurements remain in T2 acceptance.

Local developer suite: 276 tests, two optional skips; focused probe/ping/HTTPS,
main-panel, localization, QML contracts, actual Quickshell compile, plugin
validation and diff checks passed. Final source/CI identity is in PR #286.
