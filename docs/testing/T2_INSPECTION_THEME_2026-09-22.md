# T2d/e — ARM64 terminal inspection and themes

Client source: `de13c17597d7c7f855973a252da02b30a07d8fe1` (#279), including
inspection source `32de3610071eab0dec9740793f3673d46ffb84b3` (#277).
Feature-enabled executable SHA-256:
`c34cf8c230c0711e3bd47046970c7ae27b1258c1a6c894fccb2cba3ea9b8acba`.

The development client attached to the unchanged installed stable 0.8.2
runtime. No package/frontend/unit replacement, release asset, main or marketplace
change occurred. This evidence is included in rc/0.9.0, not a published T2 MVP.

## Local and rendered checks

- Full Rust validation: 1,027 PASS / 11 existing opt-ins ignored; focused TUI
  47 PASS, including eight inspection and four theme cases.
- Ten synthetic PTY lifecycle checks, formatting, Clippy, feature checks and
  parity PASS. Existing developer/QML suite and plugin validation PASS.
- Twelve real Foot EN/RU inspection captures and four EN/RU light/dark captures
  reviewed. Synthetic fixture content only; screenshots stay outside Git.
- Initial theme captures inherited the agent's NO_COLOR environment and did not
  prove theme rendering. Captures were repeated with that variable removed in
  the capture subprocess; actual light/dark and selection colors were inspected.
  Product behavior continues to respect NO_COLOR.
- Source PR #277/#279 tests and both architecture package CI builds PASS.

## Attended start / real connection

The installed service was initially inactive, disconnected in Routing, with no
core/TUN and no recovery condition. It was not silently restarted to manufacture
evidence. After owner attendance confirmation, a real test terminal separately
guarded service start and the TUI connection with ready/settled acknowledgements.

The owner chose the actual private profile in the candidate TUI and confirmed
Connect. Afterward q closed the client successfully; settled was received.
No automatic reconnect, mode cycle, provider refresh or compensating disconnect
was performed. Private identifiers/names and screen buffers were not published.

## Read-only live matrix

| Check | Result |
| --- | --- |
| Header identifies actual connected profile, not selected row | PASS |
| Current TUN upload/download totals | PASS |
| Rate appears after two fresh samples | PASS |
| Selected-profile metadata / credential-exclusion explanation | PASS |
| Owned core/TUN diagnostics | PASS |
| Rule/provider counts match existing semantic diagnostics API | PASS |
| q leaves desired/actual state, revision and owner PIDs unchanged | PASS |

The read-only matrix passed again after the owner's client had closed. Final
observations: service active, Routing connected, one visible Mihomo, one managed
and visible TUN, zero auxiliary core, desired-profile/owned-config match,
owned controller configuration verified and manualRecoveryRequired false.
The same-user control socket is a Unix socket with mode 0600. A successful TCP
listener scan found no attributable Mihomo listener. This is not comprehensive
leak testing or a new provider/DNS acceptance claim; no new HTTPS probe was run.

## Remaining boundary

The package is still opt-in for TUI; no Open app/default distribution claim.
Subscription refresh, single/all latency workflows, richer capability details,
active-connection count, activity/settings and final combined MVP acceptance
remain. Earlier T2b connection/mode evidence retains its original exact head.
