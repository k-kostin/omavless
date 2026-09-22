# T2f — ARM64 subscription refresh acceptance

Source: `b40927415a4e6dbebfba5d0f68dbcd8fbf32dc0a`, #280.
Feature-enabled client SHA-256:
`f449613e939c1fdd95180bda4d6c7848b1fa7c2eef2e26baf4c00764d4a50f6d`.
Attached to unchanged installed 0.8.2, not a replacement runtime/package.

## Checks

- Full Rust: 1,034 PASS / 11 existing opt-ins ignored; focused TUI: 54 PASS.
  Seven new refresh cases plus the expanded feature-enabled canonical parser
  case cover the existing subscription-refresh wire shape.
- Ten PTY tests, Clippy, formatting, parity, plugin validation and whitespace PASS.
- Full developer suite: 276 tests / 2 skips; QML contracts and 99 bounded EN/RU
  catalog keys PASS. An overlong help string initially failed the catalog gate;
  text was shortened, not the limit increased, and affected gates rerun.
- Real Foot synthetic EN/RU confirmation screens inspected; private actual
  screens were not published. Source test and ARM64/x86_64 package CI PASS.

## Actual provider action

The first attended invocation stopped after a mistyped acknowledgement. It is
not counted as success. No automatic retry or compensating network action ran.
The owner confirmed the terminal was closed, no authorization dialogs remained,
and explicitly requested another attempt.

The new real terminal required ready, then the owner selected a profile from the
intended subscription, pressed s and confirmed Enter. After the result, q closed
the client and settled completed the existing human barrier. Its connect-phase
guard covered possible active-session reconciliation; this was a subscription
refresh, not a requested Connect command.

Private before/after metadata and observation were compared internally. Public
results contain only booleans and the executable identity:

| Check | Result |
| --- | --- |
| Exactly one subscription's saved update timestamp advanced | PASS |
| Subscription count unchanged; no duplicate subscription created | PASS |
| Active profile, connection request and Routing mode preserved | PASS |
| Runtime instance preserved | PASS |
| One Mihomo / one managed TUN, zero auxiliary core | PASS |
| Owned controller verified; desired profile matches core | PASS |
| No manual recovery condition | PASS |
| Client q exit successful, connection remains healthy | PASS |

No profile names, record IDs, endpoint/URL, provider response, keys or screen
buffers are included. This proves actual server-list refresh, not a new
provider/DNS/HTTPS connectivity certification. The UI requested only the already
implemented semantic action; no runtime production or privileged path changed.

Included by ancestry in rc/0.9.0 with matching roadmap/status updates. Main,
installed package, release assets and marketplace snapshot remain unchanged.
This does not complete T2 or refresh-all/empty-feed/last-success presentation.
