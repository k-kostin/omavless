# T3 operator workspace — isolated VM rendering checkpoint

Disposition: **synthetic rendering and source checks passed; installed T3
acceptance remains open**. This does not approve a release, close the
connection-mutation gate or establish VPN/network health.

## Identity and environment

- T3 TUI source rendered: `5ec930c643d6f8d7fc6d0bef853eb5db04a6ca99`
  (`dev/t3-route-evidence-age`, PR #429).
- The credential-free `fixture_preview operator` executable built from that
  source had SHA-256
  `2fbeb6e5e830f4583ffab5d2ffe187205cf51a7d7c65a77b1e9288eeb3d5c1ec`;
  the copied VM executable matched byte-for-byte.
- Environment: isolated x86_64 Omarchy Dev VM, Foot and English/Russian TUI
  session setting. A forced 90×36 PTY size exercised the constrained view in
  addition to the wide terminal. The VM's installed `omavless 0.9.5beta1-1`
  package was **not** replaced. The preview has no runtime socket or mutation
  adapter. Only invented fixture names and reserved example addresses appeared.

## Checked behavior and rendering

| Surface | Observation |
| --- | --- |
| Traffic and Diagnostics | Unavailable counters remain unavailable; local core-log categories and explicit Internet/DNS-not-tested warnings were legible. No raw log was displayed. |
| Connections | The private two-row fixture appeared after its asynchronous page load. Direct and proxy-chain rows, totals and the limit-of-proof notice remained readable in English and Russian, including at 90×36. A first capture taken before loading completed was not treated as a defect or positive table evidence. |
| Route check | The initially empty page identified `/` as the explicit input action. One `example.invalid` fixture query showed its source, outcome and age without suggesting continuous monitoring. Leaving and returning cleared both destination and result. |
| Navigation and locale | Tab/Shift-Tab, Escape and the window-local English→Russian setting were exercised. The TUI remained read-only; no profile or VPN mutation was requested. |

After replacing two old timestamp-based test-directory builders with the
existing short, private, collision-safe allocator, `./tests/run-rust.sh` and
`./tests/run.sh` passed with `TMPDIR` and Cargo output under the home directory.
Before that correction, 11 Rust tests failed at Unix-socket binding because
the fixture paths exceeded Linux's socket-path bound; those failures did not
measure T3 behavior. The correction touches test fixtures only, not runtime
or socket naming in a user's installation. Raw captures stayed outside Git.

## Remaining T3 boundary

This run did **not** install the T3 candidate as the VM's application package,
attach it to a live owner, restart that owner, verify live stale/late private
reads, or test real route/provider/controller data. Those checks require a
matched exact-head installation and separately scoped host acceptance. The
connection-close mutation stays inactive: the current Mihomo GET and DELETE
operations do not provide an atomic incarnation guarantee (PR #395). A
synthetic table, connected label or route outcome cannot prove Internet/DNS
health or the path of all system traffic. T3 is not marked accepted by this
checkpoint.
