# Main-panel connection checks

Owner direction, September11: these sections were temporarily hidden until
their presentation improved. On October23 the owner selected their restoration
for0.9.9; the October29 gap review confirmed both sections as the next outcome.
The current candidate restores them with separate HTTPS and Ping results below
traffic, before Profiles. Installed EN/RU and integration evidence remain required;
source visibility alone is not acceptance.

| Section | Gate in `plugin/Panel.qml` | Preserved implementation |
| --- | --- | --- |
| Connection Test / HTTPS result | `showMainConnectionTest: true` | `nativeTestButton`, result/scope UI, Service connection test and fixed Rust probe |
| Ping / Packet Loss | `showMainLatencySection: true` | `nativePingTest`, latency/loss/status UI, Service rolling samples and fixed Rust ping |

The latency gate still controls the adjacent `traffic.native_note` explanation at
the owner's request. Traffic rates/totals and sparkline remain visible; their
scope remains TUN-interface traffic, including both direct and VPN paths, not
a count of exclusively proxied traffic. Do not change that meaning later.

Disabled gates remove their controls from keyboard navigation. With restoration,
Tab follows mode → HTTPS → Ping → Profiles; unavailable controls are skipped.
Ping monitoring remains limited to an open native main panel and the existing
connected/eligible conditions. Closing or leaving it admits no new samples. Backend/CLI functionality, translations
and deterministic tests remain intact. No connection/lifecycle change is made.

## Delivery checklist (October29 candidate)

1. Owner scope: both sections, selected October23 and retained October29.
2. Improve the owning QML/catalog surface; do not reimplement the backend.
3. Set the corresponding gate to `true`; update the default-hidden assertions
   in `tests/test-native-main-panel.js` / `tests/test-native-ping.js` deliberately.
4. Run main-panel, ping, traffic, connection-test, localization and QML gates.
5. Install the exact candidate and check English/Russian connected and
   unavailable states, layout, Tab order, close/reopen and bounded sampling.
6. Record actual evidence and update this document. Unavailable measurements
   must not be presented as proof that the user's VPN connection has failed.

## User meaning and boundaries

HTTPS Test makes one explicit read-only request through the current routing
mode. Show Not checked, Checking, response timing, failed request or unavailable
truthfully; none changes the connection status or diagnoses credentials. The
optional observed address retains the existing privacy preference. The short
scope explanation remains visible before the first test.

Ping retains the bounded ten-sample window and its independent ICMP result.
Unavailable/stale data shows dashes, not zero loss; observed missing replies
retain their real loss count. The Measure button resets/joins the existing
request path without creating a second worker. The layout uses ordinary neutral
bordered actions and shared shell tokens; it preserves the fixed profile dock.
No runtime, timeout, endpoint, network policy or ownership change is selected.
