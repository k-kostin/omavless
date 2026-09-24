# T2 subscription overview — ARM64 read-only acceptance

Candidate: `dev/t2-subscription-overview`, based on RC
`38e627e3a620acd57c201683ca147a90cef1a7eb`. The PR records the final source head.
Client SHA-256: `ac7e470f99c9b3b07499ee5419c69f6ee7971129092594a350588bd75e828506`.

- Seven new cases / 61 focused TUI tests PASS: empty feeds, counts, timestamp
  bounds/clock rollback, privacy, read-only keyboard behavior, 64 feeds and
  wrapped End/Up/resize navigation. Catalog: 109 bounded EN/RU keys.
- Full Rust baseline: 1,041 PASS / 11 opt-ins ignored. Developer suite: 276 tests
  / 2 skips; QML, 10 PTY tests, formatting, Clippy, parity and plugin validation PASS.
- Real Foot synthetic EN/RU top/bottom/Up/back screens inspected. A saved age
  is not provider health; unknown/future timestamps do not claim success.
- Candidate attached read-only to installed 0.8.2: displayed subscription
  metadata matched the private snapshot; IDs/raw timestamps were absent.
  Reload and q preserved exact desired/actual state, revision, runtime instance,
  process identities and saved metadata. No service start, provider refresh,
  install, tunnel transition or authorization prompt occurred.

No runtime/QML/package changes, new crate dependencies, credentials or captures
are committed. Ratatui's existing locked rendered-line-count API is enabled for
correct wrapped scrolling. Empty-feed refresh, refresh-all and operation history
remain outside this read-only slice; T2 is not complete. See the
[TUI contract](../roadmap/TUI_APP.md) and [RC ledger](../development/RC_090.md).
