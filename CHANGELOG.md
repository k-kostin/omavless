# Changelog

All notable changes to OmaVLESS are documented here.

## Unreleased

- Report file-picker availability during onboarding and installation, with an
  actionable Omarchy package command in Settings before the user tries file
  import.
  Picker discovery and launch now use a deterministic, shell-free backend
  boundary while retaining zenity, kdialog and yad support; clipboard import
  remains available without a picker.
- Published OmaVLESS 0.7.0 in the Omarchy plugin marketplace as a
  maintainer-verified listing at exact commit
  `69fe05b03129a23664fff3f8289821a7b7f80095`.
- Made normal `omarchy plugin remove kdk.omavless` fail safe despite Omarchy
  intentionally having no uninstall hooks. The generated main unit now runs a
  plugin-owned Mihomo supervisor, and QML starts a bounded removal guard when
  unloaded. Actual checkout deletion stops the TUN, disables autoconnect and
  removes both user units. An explicit plugin disable applies the same cleanup;
  hot reload and update preserve the selected connection state.
- Added a Settings-only, read-only Mihomo diagnostics page backed exclusively
  by OmaVLESS's private Unix controller. Loaded rules expose bounded type and
  payload fields plus only the terminal `VPN`, `DIRECT` or `REJECT` outcome;
  rule providers expose bounded identifiers, behavior, count, state and last
  successful update without configuration fragments or proxy-group names.
- Added local loaded-rule search, explicit loading/empty/error states and
  generation-based stale-response rejection. Static controller data is read
  once on page entry, on manual refresh and after a successful provider
  update; repeat requests coalesce instead of building a process backlog.
- Split status polling into the configured open-panel cadence and a light
  background cadence of at least 30 seconds, retaining bounded failure
  backoff. Traffic/details and latency sampling now stop when the main page is
  hidden unless compact bar throughput was explicitly enabled.
- Hardened rule-provider refresh validation against arbitrary API paths and
  removed provider identifiers from refresh failure messages while preserving
  the existing all-or-nothing success timestamp.
- Made the planned WireGuard and AmneziaWG work visible from the README and
  added an at-a-glance protocol matrix to the roadmap. The documentation now
  distinguishes Mihomo 1.19.30 core support from OmaVLESS runtime import
  support, records that AmneziaWG v3.0 and v3.1 both require that baseline,
  and keeps the current manifest description limited to implemented protocols.
- Added a delivery roadmap which separates merged, cloud-ready, experimental,
  planned and deferred work; records the exact Draft-PR handoff order; and
  names the privacy and live-validation gates for existing experimental
  adapters, WireGuard, AmneziaWG and later product tracks.

## 0.7.0 — 2026-08-24

- Decoupled pointer hover from the hero's persistent keyboard cursor so
  Settings, QR and Test no longer light the neighboring power-switch ring.
  Profile import actions now share the Subscriptions control height and
  optical centerline for stable hit targets and alignment.
- Hardened VLESS share-link parsing against duplicate fields, case-insensitive
  duplicates, conflicting aliases, malformed booleans and unknown options.
  Recognized provider-only metadata is bounded, explained during import and
  never copied into Mihomo configuration.
- Based VLESS subscription identity on supported connection semantics rather
  than labels, query ordering or provider priority hints. Legacy stored keys
  migrate during refresh while retaining profile IDs, favorites, active/last
  selection and autoconnect relationships.
- Sanitized provider-controlled profile metadata before every stock Omarchy
  text sink, including hero metadata and confirmation dialogs, so QML cannot
  interpret a profile label as rich text.
- Preserved option-like profile/subscription names and file destinations with
  explicit argument boundaries, and kept profile credentials on stdin rather
  than process command lines.
- Replaced raw Mihomo configuration-test output and malformed-link parser
  details with credential-free errors while retaining actionable recovery
  states for service operations.
- Added host-independent status tests, bracketed-IPv6 coverage, live procfs
  credential-boundary checks and migration regression tests. All supported
  generated profile families are also checked by the installed-core opt-in
  integration test.
- Added a protocol-neutral profile adapter for import, preview, Mihomo output,
  probes and subscription identity. Private stores migrate in memory to v3
  with an explicit protocol discriminator while preserving profile IDs,
  favorites, subscription links, active selection and autoconnect choices.
- Added experimental Trojan profiles for bounded `trojan://` manual import and
  mixed VLESS/Trojan subscriptions. TCP, WebSocket and gRPC map to Mihomo with
  TLS or the supported REALITY subset; unsupported share fields fail before
  storage and public preview never receives the Trojan password.
- Added experimental Hysteria2 profiles for official `hysteria2://` and
  `hy2://` manual and subscription import. Authentication, multi-port hopping,
  SNI, certificate pinning, ECH and `salamander`/`gecko` map to current Mihomo;
  Realm, bandwidth and unknown provider fields fail closed, while preview and
  diagnostics receive no authentication or obfuscation secrets.
- Added experimental TUIC v5 profiles for bounded `tuic://` manual and
  subscription import. UUID/password, SNI, ALPN, strict TLS, UDP relay and
  congestion fields map directly to Mihomo; v4 tokens and unknown options fail
  closed, 0-RTT stays off, and public surfaces receive no TUIC credential.
- Tightened VLESS compatibility boundaries: XUDP/PacketAddr encodings and
  XHTTP modes are now normalized and strictly validated, while both Xray
  Vision share-link variants map to Mihomo's single supported Vision flow
  without hiding their different UDP/443 semantics.
- Aligned Reality output with Mihomo's current schema: public keys and short
  IDs are validated, Xray-only `spx` is explained during import instead of
  emitted as an unsupported `spider-x` option, and ML-DSA-only links fail with
  a credential-free compatibility error.
- Added bounded XHTTP `extra` translation for client headers, padding,
  session/sequence placement, upload controls and XMUX/reuse. Duplicate,
  oversized, deeply nested, server-only and unknown fields are rejected; old
  stores remain readable even when a previously ignored field needs manual
  compatibility review before reconnecting.
- Added bounded XHTTP `downloadSettings` for a second Mihomo endpoint with
  independent path, host, headers, reuse and TLS/Reality metadata. Unmapped
  `sockopt`, certificate material, recursive downloads and mode conflicts fail
  before storage, while the redacted preview reveals no download endpoint or
  nested credential.
- Added experimental, bounded VLESS Encryption import for Mihomo's
  `mlkem768x25519plus` client grammar and experimental REALITY
  X25519-MLKEM768 metadata on upload/download endpoints. Preview reports only
  fixed capability names and a fingerprint-dependent compatibility warning;
  encryption strings and nested key material remain private.
- Added multi-server favorites for both local and subscription-managed
  profiles. Active and pinned servers stay easy to reach without adding a
  permanent toolbar; pins survive subscription refreshes for retained nodes.
- Added a redacted review step before file or clipboard import. Parsing remains
  in the backend and the GUI receives no complete VLESS UUID, Reality key or
  URI.
- Added a Settings export for private-permission JSON diagnostics built only
  from bounded counts and states, without profile/subscription identifiers,
  key material, names, endpoints, executable paths or subscription URLs.
- Added Settings-only custom routing rules for exact domains, domain suffixes
  and IPv4/IPv6 ranges, with VPN, Direct and Block actions evaluated before the
  selected country preset.
- Added `Where will this destination go?` with mode/custom-rule explanations
  and live Mihomo matched-rule detection. The live check opens one TCP
  connection to port 443 without loading a page and hides profile/group names.
- Added manual HTTP rule-provider refresh with an all-or-nothing success
  timestamp, plus compact indicators for the latest successful rule and
  subscription updates.
- Added a plugin-owned private Unix controller for live route inspection and
  rule updates. Adopted templates have inherited controller listeners removed
  before the runtime config is generated.
- Made Full VPN explicitly select and verify the active profile in Mihomo's
  `GLOBAL` group instead of inheriting a cached `DIRECT` choice. New bundled
  templates also ship a non-cyclic `GLOBAL` default, while existing private
  routing templates are fixed at runtime without being overwritten.
- Added a compact three-step first-run guide that checks Mihomo and required
  TUN capabilities, offers a skippable country Routing profile, and reuses the
  private VLESS import flow. Package and capability commands are copy-only and
  are never executed by the plugin.
- Added explicit login autoconnect settings for the last used or one fixed
  server in Full VPN or Routing mode. New installs start manually; existing
  installs retain their previous enabled-service behavior until this setting
  is saved.
- Split runtime and login responsibilities between `omavless.service` and a
  small user-level `omavless-autostart.service`, with transactional validation,
  rollback and uninstall cleanup for both units.
- Reordered the compact routing controls to `Full VPN`, `Routing`, `Direct`.
- Added a first-use Routing chooser for Russia, China and Iran. Existing
  profile stores retain their historical routing behavior instead of being
  interrupted by onboarding after an update.
- Added a general gear-shaped Settings page for routing profiles, source
  attribution, subscription management, monitoring context and privacy/panel
  preferences without expanding the main connection view.
- Kept RoscomVPN DEFAULT as the Russia baseline and added Mihomo-native China
  and Iran profiles backed by MetaCubeX and Chocolate4U MRS rule data.
- Made routing-profile changes persistent and transactional. A first-use
  selection activates Routing; a later Settings change preserves the current
  Full VPN/Routing/Direct mode, and a failed live reconnect restores both the
  prior template and preference.
- Extended the public routing status with credential-free preset/configuration
  state and added backend/QML contracts for the new templates and onboarding.

## 0.6.2 — 2026-08-23

- Finalized the pre-publication plugin ID as `kdk.omavless`, the maintainer
  identity as `kdk`, and the public repository root as
  `https://github.com/k-kostin/omavless`.
- Reworked the README around an Omarchy-native quick start using the current
  AUR `mihomo-bin` package, explicit TUN capabilities, direct GitHub plugin
  installation, and a separate path for users who already have Mihoro.
- Documented the standard `omarchy plugin add` lifecycle, explicit Mihomo and
  TUN-capability setup, and the required uninstall-before-remove sequence.
- Aligned network User-Agent metadata with version 0.6.2 and removed the
  placeholder project URL; release tests now guard that version relationship.
- Updated the QML contract test to reflect the intentionally mutable backend
  capability payload and pinned the CI checkout action to its verified commit.
- Extended the panel cap slightly again so the first profile's secondary line
  fits instead of being clipped at the bottom edge.
- Gave receive and send traffic stable saturated series colors and reused them
  in the live numeric legend; upload is dashed so equal RX/TX traces remain
  distinguishable instead of painting one another over.
- Added the normal control border to the hero QR action, making uptime, QR,
  Test and the power track consistently outlined at rest.

## 0.6.1 — 2026-08-23

- Moved the uptime pill onto the same vertical centerline as the hero's QR,
  Test and power controls instead of leaving it in the higher title row.
- Added breathing room between the traffic labels and sparkline.
- Raised the main panel's content cap to the standard large-panel height so
  the first saved-profile surface fits completely on a typical display; the
  shell's available-screen-height clamp still wins on smaller screens.

## 0.6.0 — 2026-08-23

- Added bounded plain-text rendering for every plugin-owned label and sanitized
  public profile, provider, endpoint and diagnostic strings before stock QML
  controls can interpret them as markup.
- Added connection uptime, an observed Exit IP with copy support and explicit
  routing-policy caveat, a 60-second receive/send sparkline and optional live
  bar throughput (off by default).
- Added profile search across display name, provider, country-like labels and
  endpoint hostname; matching provider groups expand without changing the
  user's saved disclosure state.
- Added concise bar tooltips and neutral best-effort warnings for concurrent
  Mihoro, V2RayN, Mihomo, Xray, sing-box, Clash or other TUN interfaces. These
  hints never stop processes or masquerade as fatal connection errors.
- Introduced a backend capability map and bound optional UI actions to it, so
  future protocols or alternative cores can expose only behaviour they
  actually implement.

## 0.5.1 — 2026-08-23

- Stream subscription probe results into the expanded server list as each
  DNS failure or successful end-to-end check becomes available; later test
  targets can refine the displayed median without waiting for the full run.
- Treat unavailable provider nodes as neutral availability information.
  Urgent red remains reserved for broken subscriptions, malformed probe
  output, configuration failures and other actionable plugin errors.
- Added a compact outlined `Test` action between the active-profile QR and
  power controls. It refreshes the live TUN-bound ping window immediately and
  uses the existing detail grid for latency and packet-loss feedback.

## 0.5.0 — 2026-08-23

- Made subscription testing self-explanatory in both panel views: it now shows
  elapsed time while running, a session-only last-tested age, available and
  failed totals, and an explicit end-to-end VLESS description.
- Added a working Cancel action for long server tests and ensured their
  temporary Mihomo core is terminated and cleaned up on cancellation or a
  graceful widget-process shutdown.
- Clarified subscription inventory with correct singular/plural labels and
  separate managed-profile totals instead of mixing in manual profiles.
- Promoted DNS failures to the same urgent visual state as unreachable nodes,
  added latency-sort and test tooltips, and kept subscription/provider rows
  visibly outlined at rest for clearer click targets.
- Let the subscription manager size itself to its actual content while still
  capping expanded server lists, removing the large empty panel below a short
  provider list.
- Reused one compact subscription status surface across the main and
  management pages, so a test started from the profile list no longer appears
  silent until completion.

## 0.4.4 — 2026-08-23

- Replaced TUN-intercepted TCP-connect timing, which could falsely report
  `1 ms` for every node, with full end-to-end Mihomo proxy health checks.
- Added a short-lived no-TUN probe core using a private Unix controller and
  the standard Mihomo routing mark so an active tunnel cannot intercept its
  measurements.
- Tests now pin every resolved endpoint candidate, perform real VLESS and
  TLS/Reality handshakes against three independent HTTP targets, and report
  the median successful delay without exposing subscription credentials.
- Treat an API-level `all proxies timeout` result as valid node unavailability
  instead of incorrectly failing the entire test operation.

## 0.4.3 — 2026-08-23

- Fixed subscription tests treating every domain-backed server as unresolved
  when the first policy DNS resolver was unavailable. Tests now health-check
  and fall back across configured DoH resolvers.
- Added IPv4/IPv6 address discovery, rejection of TUN fake-IP answers and up
  to four endpoint candidates per server.
- Replaced a single noisy sample with the median of at least three TCP
  connections to the server's actual VLESS port, while trying every bounded
  fallback IP at least once.
- Renamed the ambiguous `Not resolved` state to `DNS failed` in the panel.

## 0.4.2 — 2026-08-23

- Added a dedicated scrollbar gutter and bottom breathing room so long server
  names, latency states and the final row do not crowd the viewport edge.
- Stopped pointer hover from scrolling the list; keyboard navigation still
  keeps its selected row in view.
- Gave saved profile rows a persistent theme-native outline while preserving
  the existing outlined-and-filled hover treatment.

## 0.4.1 — 2026-08-23

- Added an explicit in-list ping sort control for expanded subscriptions.
  It toggles ascending and descending reachable latency while DNS-failed and
  unavailable servers remain at the bottom.

## 0.4.0 — 2026-08-23

- Grouped subscription-managed servers under collapsible provider rows in the
  main profile list and on the subscription management page.
- Added an explicit, concurrent TCP reachability test with per-server latency;
  reachable nodes sort fastest-first and unavailable nodes move to the bottom.
- Kept probe results ephemeral and credential-free so a transient network
  state cannot silently change future connection choices.
- Fixed a late external-drop notification leaving the bar's shield red after
  another profile had already connected successfully.

## 0.3.0 — 2026-08-23

- Added multiple VLESS subscriptions with a dedicated in-panel management
  page, private add/edit flow and per-provider or atomic refresh-all actions.
- Added raw and base64 VLESS-list parsing, stable node identities, source
  labels and bounded HTTPS fetching without new runtime dependencies.
- Added a version-2 private store with transparent v1 migration. Subscription
  URLs never enter argv or the public status payload.
- Updates preserve an active node removed by its provider as an explicitly
  stale profile until disconnect, rather than silently changing the exit IP.
- Kept network downloads outside the shared operation lock so a slow provider
  cannot block status, connection controls or an urgent disconnect.

## 0.2.0 — 2026-08-23

- Rebuilt profile storage and VLESS-to-Mihomo conversion around a bounded,
  private, transaction-safe Python backend.
- Added Reality/TLS transports, deterministic Rule-mode TUN configuration,
  live traffic and latency details, QR display, edit, rename and IPC actions.
- Added rollback verification, multi-monitor locking and status caching,
  external-drop notifications, bounded imports and strict store validation.
- Improved panel feedback, keyboard navigation, action tooltips, modal
  coordination, compact bar-state icons and explicit credential warnings.
- Added an always-visible effective-routing summary that distinguishes Rule,
  Global and Direct modes as well as RoscomVPN, custom and basic policies.
- Added persistent `Routing`, `Full VPN` and `Direct` controls to the panel,
  with transactional live switching and rollback on failure.
- Replaced the bundled LAN-only fallback with a deterministic RoscomVPN-based
  policy using 23 maintained GeoSite/GeoIP rule sets; existing private
  templates change only through the explicit `use-routing` command.
- Isolated OmaVLESS from Mihoro's configuration and service and added a safe
  removal helper.
