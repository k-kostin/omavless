# T2d: read-only inspection pages

Development candidate on top of T2a–c, not the full T2 MVP or a published
package. The installed runtime/QML and stable main remain unchanged.

## Interaction and privacy

Tab/Shift+Tab cycle Profiles, Traffic, selected-profile Details and Diagnostics.
Esc returns to Profiles without changing selection; arrows/j/k scroll inspection
pages. Search, help and confirmation keep their own keyboard ownership. Network
action shortcuts are available only on Profiles, not hidden behind inspection
content. The verified active identity remains in the header on every page.

- Traffic requests existing `runtime.traffic` only while its page is visible.
  RX/download and TX/upload match the accepted QML convention. Rates need two
  fresh, non-regressing samples from the same counter identity/runtime/revision;
  restart, gaps, counter reset, unavailability and page changes reset the series.
  Integral IEC formatting avoids locale-dependent decimal punctuation. These
  are TUN totals including direct traffic, not VPN-only usage or leak evidence.
- Details uses already admitted private UI metadata: exact selected name/source,
  favorite, feed presence and independently observed connected identity. It does
  not request endpoint/credential-bearing `profiles.details` or export methods.
- Diagnostics requests existing `diagnostics.summary` while visible; only
  bounded rule/provider counts survive into the model. Core/controller/TUN facts
  come from the fresh observation. Raw rule rows, destinations, provider names,
  controller paths/secrets and remote errors are never rendered by this page.
- Unavailable is not zero, Connected is not an Internet/DNS test, and the UI
  explicitly makes no kill-switch claim. Active connection count and detailed
  protocol/capability inspection remain follow-ups, not invented measurements.

## IPC and lifetime

The existing authenticated adapter and bounded capacity-one read worker are
reused. The ordinary Profiles/Details refresh still uses four fixed reads.
Traffic/Diagnostics adds one capability-gated fixed empty-parameter read between
metadata and observation. Revision, runtime instance and desired/actual checks
fence the combined result. Optional failure leaves metrics unavailable without
turning a healthy independent observation into a fake disconnection.

Freshness still expires six seconds after work starts, even if an optional read
is slow. Tab, repaint and exit never wait for that I/O. No runtime code, new
daemon methods, controller access, service start, store write or package behavior
is added. There is no automatic network probe.

## Evidence and remaining gates

- 43 TUI tests (8 new inspection cases): on-demand/capability reads, coherent
  fences, unknown metrics, numeric/schema bounds, reset/rate direction, private
  field exclusion, exact selection, keyboard ownership, EN/RU and resize.
- Full local Rust/format/Clippy/parity and existing developer/QML suites pass;
  ten synthetic PTY lifecycle tests pass. Catalog: 95 EN/RU keys, shared QML
  translations unchanged.
- Twelve real Foot captures of synthetic EN/RU profiles, traffic, details,
  diagnostics/scroll and return were inspected. They remain outside Git and
  prove presentation only, not a live tunnel.
- Live read attachment was attempted, but the installed runtime was already
  inactive with zero OmaVLESS/Mihomo processes at the first host check. No
  service/network change was made to manufacture evidence. Live counters/count
  comparison with an active runtime remains pending; T2b's earlier attended
  lifecycle evidence keeps its original exact source identity.

T2 still requires subscription refresh/probes, remaining details/activity,
theme following, launch/focus/default packaging and combined host acceptance.
Main/release/marketplace updates remain owner-controlled.
