# OmaVLESS development delivery roadmap

Status: active delivery ledger, updated 2026-08-28.

This file is the compact source of truth for delivery order and acceptance
state. Detailed design rationale lives under [`docs/roadmap/`](docs/roadmap/):

- [`docs/roadmap/README.md`](docs/roadmap/README.md) — roadmap/document map;
- [`docs/roadmap/ARCHITECTURE.md`](docs/roadmap/ARCHITECTURE.md) — shared
  runtime, core, lifecycle/exclusivity and fail-closed direction;
- [`docs/roadmap/TUI_APP.md`](docs/roadmap/TUI_APP.md) — evolution from a
  bar-only plugin to one runtime with compact bar, full TUI and CLI/IPC control
  surfaces;
- [`docs/roadmap/DEVELOPMENT_WORKFLOW.md`](docs/roadmap/DEVELOPMENT_WORKFLOW.md)
  — cloud/Omarchy Git workflow and branch policy;
- [`PROTOCOL_ROADMAP.md`](PROTOCOL_ROADMAP.md) — protocol and share-format
  compatibility specification.

## Status vocabulary

- **Merged** — accepted code/documentation is in `main`.
- **Cloud-ready** — exact Draft PR head passed every available cloud/static
  gate but still has declared Omarchy-only checks.
- **Merge-ready** — exact head passed its declared cloud and local gates and is
  waiting only for final owner merge approval.
- **Experimental** — product maturity label; implementation may already be
  merged but broader independent interoperability evidence is still required.
- **Planned** — designed/scheduled, no production runtime support yet.
- **Deferred** — intentionally outside the active merge chain.

Git maturity and product maturity are separate. An experimental protocol may
live in `main` after its implementation gates pass; it does not need a permanent
`alpha` or `beta` branch merely to preserve the label.

## Repository and release model

`main` is the only long-lived development source of truth. Runtime/network/
security changes merge only after the exact candidate head passes every
required real-Omarchy gate. Documentation-only work does not invent a live VPN
gate.

The currently published OmaVLESS 0.7.0 marketplace snapshot remains exact
commit `69fe05b03129a23664fff3f8289821a7b7f80095`, published and
maintainer-verified through marketplace submission `#2311`. `main` is allowed
to advance beyond that immutable public snapshot; documentation must continue
to name the exact marketplace SHA rather than imply that the moving `main` tip
is already published.

For future formal releases, prefer an explicit version tag such as `v0.8.0` on
the exact release candidate which passed the release gate.

### Branch policy

Do **not** maintain permanent `alpha` and `beta` branches by default.

- A short-lived feature branch + Draft PR is the practical **alpha** state for
  cloud work.
- A cloud-ready Draft whose real Omarchy checklist is still open represents the
  next maturity state without requiring a second branch.
- After the exact head passes its real gate, it merges directly into `main`.
- A temporary `beta/<scope>` branch is allowed only as an integration/test
  assembly when several named candidate PRs genuinely need combined Omarchy
  or soak testing. Fixes found there must go back to the owning feature PRs;
  beta is deleted afterward and never becomes a parallel source of truth.

Full rules: [`docs/roadmap/DEVELOPMENT_WORKFLOW.md`](docs/roadmap/DEVELOPMENT_WORKFLOW.md).

## Current repository snapshot

D0 / documentation PR `#28` is merged into `main` at
`751bb31b6c478e53aea6c0e9647e60fdb80bce6a`. It preserved the S0 lifecycle
work from merged PR `#29` and established the P4 WireGuard/AmneziaWG queue.

D1 / PR `#27` is complete and merged. V0 / PR `#30` was reconstructed directly
on `main` rather than remaining stacked on D1. Its static gates are green and
one representative VLESS XHTTP live case passes, but the Draft remains open
because VLESS Encryption / REALITY PQ, Trojan, Hysteria2 and TUIC v5 fixtures
are unavailable. V0 is partially validated, not complete.

Fixture absence blocks claims about those protocol families; it does not block
independent work which needs no new protocol key. The following Try Omarchy-
validated UX fixes are now merged:

- issue `#39` / PR `#41`: out-of-box GTK file-picker fallback plus actionable
  picker onboarding, merged at `8ebf1049bb7135569b7ec89673f5e0f7fde3cb58`;
- issue `#40` / PR `#42`: unified top-level profile/subscription import,
  merged at `2e10b7e8f707f7d91f3d8c18e9ffd0dc20da44ba`;
- issue `#37` / PR `#43`: page-local Tab/Shift+Tab navigation with preserved
  Enter/Escape and shell handoff after panel close, merged at
  `d2a58589ed1eef5938fbce888f1e43a8d4c90fd0`.

## Active merge chain

### S0 — marketplace lifecycle and removal fix

- PR: `#29`.
- State: **merged** at `69fe05b03129a23664fff3f8289821a7b7f80095`.
- Result: disable/removal cleanup, generated user-service ownership and private
  state retention/removal semantics were hardened and became the published
  marketplace snapshot.

### D0 — documentation and progress ledger

- PR: `#28`.
- State: **merged** at `751bb31b6c478e53aea6c0e9647e60fdb80bce6a`.
- Result: canonical protocol/delivery queue plus explicit WireGuard/AmneziaWG
  planning.

### D1 — advanced Mihomo diagnostics and adaptive polling

- Branch: `codex/advanced-mihomo-diagnostics`.
- PR: `#27`.
- State: **merged** at `5a50d59ae9fada9c61ee3ed9037f72e35bf09853`.
- Accepted exact head: `2c2615247d32e7ac4b2f7ada055d1028586cc639`.
- Result: advanced private-controller diagnostics and adaptive polling merged;
  the Full VPN selector-readiness race was reproduced and fixed with bounded
  retry semantics.
- Acceptance record:
  [`docs/testing/TRY_OMARCHY_D1_ACCEPTANCE_2026-08-27.md`](docs/testing/TRY_OMARCHY_D1_ACCEPTANCE_2026-08-27.md).
- Successor: V0.

### V0 — existing experimental protocol validation gate

- Branch: `codex/experimental-protocol-live-gates`.
- PR: `#30`, Draft, based directly on `main` at its reconstruction point rather
  than stacked on D1.
- Exact V0 head: `a643db595ad5369b5fea200ebc601b4f0f70f18f`.
- State: **partially live-validated; additional fixtures unavailable**.
- Goal: turn generated-config evidence into repeatable privacy-safe live
  evidence before another credential family is added.
- Scope: opt-in local runner for existing VLESS XHTTP modes, VLESS Encryption /
  REALITY PQ, Trojan, Hysteria2 and TUIC v5. Real URIs, passwords, protocol
  UUIDs, subscription URLs and keys never enter its shareable result matrix.
- Evidence: exact-head cloud/static and installed-Mihomo gates pass. The
  privacy-safe `vless-xhttp-stream-one-provider-1` case passed Full VPN/TUN,
  HTTPS probe, restoration and privacy checks on Try Omarchy ARM64.
- Remaining gate: representative VLESS Encryption / REALITY PQ, Trojan,
  Hysteria2 and TUIC v5 fixtures are unavailable. The safe UDP-restricted
  Hysteria2 half is also unavailable. Do not fabricate fixtures, call V0
  complete or discard the accepted XHTTP evidence merely because unrelated
  no-fixture UX/docs work advances `main`.
- Successor: P4a.

### P4a — standard one-peer WireGuard `.conf`

- Future branch: `codex/wireguard-conf-import`.
- State: **planned**.
- Dependency: V0 merged plus a fresh Mihomo source/release audit.
- Scope: bounded one-interface/one-peer parser, versioned structured private
  storage, redacted preview, Mihomo WireGuard generation, endpoint probe and
  safe explicit export. Reject executable hooks, extra peers, ambiguous keys
  and arbitrary YAML.
- Omarchy gate: real WireGuard server, IPv4/IPv6 where available, Full VPN /
  Routing / Direct, reconnect/autoconnect/update/remove rollback and proof that
  reusable keys do not reach public UI/logs/diagnostics/argv/environment.
- Successor: P4b.

### P4b — native AmneziaWG `.conf`

- Future branch: `codex/amneziawg-conf-import`.
- State: **planned**.
- Dependency: P4a merged/live-tested and installed-core capability gate.
- Scope: reuse P4a storage/parser boundaries, accept only validated native AWG
  generation families and emit documented `amnezia-wg-option` fields.
- Omarchy gate: real matching AWG servers/fixtures, precise old-core errors and
  independent live evidence; retain Experimental label initially.
- Successor: P4c.

### P4c — AmneziaVPN guest `vpn://`

- Future branch: `codex/amnezia-vpn-key-import`.
- State: **planned after P4b**.
- Scope: bounded offline guest-key decoding and explicit extraction of a single
  WG/AWG container through the exact P4a/P4b validators.
- Security boundary: no embedded API call, no SSH/root/server-administration
  credentials, no implicit fallback to another protocol container.

## Implemented but not yet stable

The following are runtime-complete in 0.7.0 but remain Experimental until V0
collects broader independent evidence:

- VLESS Encryption and REALITY PQ metadata;
- Trojan supported TLS/REALITY combinations;
- Hysteria2, especially UDP-restricted behavior;
- TUIC v5;
- representative advanced XHTTP modes/split settings.

## Product tracks after the active protocol chain

These tracks are independent of protocol expansion and should remain narrowly
scoped PRs. Their relative ordering can be changed deliberately after P4c; the
roadmap does not require implementing every product track before another can
start.

### I1 — i18n foundation

- Future branch: `codex/i18n-foundation`.
- English remains default/fallback; Russian is the first additional locale.
- Provider/profile/core-controlled data is never translated.
- Prefer stable backend status/error codes localized on the UI side so the same
  codes can later be rendered consistently by QML and the TUI.

### C1 — privacy-aware active connections

- Future branch: `codex/active-connections`.
- Depends on D1's diagnostics/controller boundary.
- Read-only host/process/chain/rule/traffic comes first; closing a connection
  is a separate mutation/security slice.
- The eventual TUI is the natural full view for this data; the bar may retain a
  compact summary rather than duplicate the complete connection table.

### S1 — App proxy without TUN

- Future design branch: `codex/system-proxy-design`.
- User-facing concept: applications which honor proxy settings, explicitly not
  another Full VPN / Routing / Direct policy.
- Preserve/restore exact prior proxy settings transactionally and introduce no
  privileged helper.
- On Omarchy, design must cover both desktop proxy state and the systemd/UWSM
  user-manager environment inherited by newly launched applications. Merely
  switching a desktop proxy to `none` or unsetting variables is not sufficient
  when the machine had pre-existing proxy state.

## Full application / TUI track

Detailed design: [`docs/roadmap/TUI_APP.md`](docs/roadmap/TUI_APP.md).

The target is **not** a second VPN application next to the plugin. It is one
canonical OmaVLESS runtime with a compact Omarchy bar surface, a full TUI
workspace and a bounded CLI/IPC integration surface.

### T0 — control-plane and distribution design

- State: **planned design phase**.
- Define bar/TUI/CLI responsibilities and the migration path from the current
  QML -> `backend.py` control model.
- Define a versioned bounded semantic IPC protocol and separate small state from
  high-volume local logs/connections.
- Define daemon/systemd ownership, package/update/remove lifecycle and exact
  compatibility requirements for the existing plugin.
- Preserve the current distinction between closing a panel, disconnecting the
  VPN and disabling/removing the plugin. Defer `Quit OmaVLESS` until T1 owns the
  tunnel independently of UI lifetime; do not alias it to popup close or plugin
  disable.
- Define an explicit distribution path for a future native TUI/runtime. The
  marketplace plugin must not silently download/build a binary.
- Define `Open app` launch-or-focus behavior using the supported Omarchy
  terminal launcher and a stable app-id.

### T1 — shared runtime / daemon foundation

- State: **planned after T0 when scheduled**.
- One headless runtime becomes the canonical owner of desired/actual connection
  state, store mutations, core lifecycle and background work.
- Expose a private `0600` Unix-socket contract plus a small machine-readable CLI
  bridge for QML/scripts; ordinary state contains no reusable credentials or
  core-controller secret.
- Keep current bar behavior working through the new boundary before adding a
  full TUI.
- A panel/TUI/terminal close must not stop a healthy requested tunnel.
- This phase should satisfy/subsume the relevant N0 coordinator extraction;
  do not create a parallel daemon state machine and a separate coordinator.

### T2 — TUI MVP and plugin `Open app`

- State: **planned after T1**.
- Build an Omarchy-themed keyboard-first TUI with a responsive layout.
- Wide dashboard direction: traffic summary at top, profiles/subscriptions
  sources plus live activity/logs in the main workspace, and persistent
  connection/mode/core/protection/policy status at the bottom.
- MVP operations: profiles/subscriptions, connect/disconnect, supported modes,
  search/favorites, one/all latency tests, traffic rate/totals, active
  connection count, profile/core details, subscription refresh, settings and
  diagnostics entry points, help overlay.
- `j/k` + arrows are baseline navigation; `h/l`/Tab, search, first/last and
  mouse support are added where they improve rather than complicate semantics.
- Follow the active Omarchy theme while running.
- Add `Open app` to the plugin. It must launch or focus one TUI window, attach to
  the existing runtime and never create a second tunnel/core.
- Concurrent bar and TUI actions must serialize against the same canonical
  state; closing/reopening either surface must not disturb the tunnel.

### T3 — operator and observability integration

- State: **planned after T2 and prerequisite feature tracks**.
- Integrate D1 rules/provider diagnostics into dedicated TUI views.
- Integrate C1 active connections when available, including a later explicit
  close-connection mutation after its own security review.
- Add bounded local core/application activity/log views with session-aware
  navigation/selection/copy, richer traffic context and route inspection.
- Add a read-only `doctor`/health view or CLI output with remediation hints.
- Keep local operator detail distinct from shareable redacted diagnostic
  export; raw logs are never silently bundled for sharing.

### T4 — background and management quality-of-life candidates

- State: **later, split into separate PRs**.
- Candidate slices: per-subscription automatic refresh schedules; provider
  quota/usage/expiry metadata under strict bounds; transactional private-state
  backup/restore; reconnect after suspend/selected network transitions; richer
  batch latency workflows; bounded structured DNS controls; named
  service-routing templates layered on existing custom rules.
- These are not permission to accept arbitrary provider YAML/configuration or
  to broaden protocol support solely for feature-count parity.

## Networking/core architecture tracks

The accepted design direction is documented in detail in
[`docs/roadmap/ARCHITECTURE.md`](docs/roadmap/ARCHITECTURE.md). These tracks do
not authorize a speculative rewrite of the current Mihomo implementation.

### N0 — tunnel coordinator boundary

- State: **planned when a concrete networking or multi-surface feature needs
  it**.
- Centralize desired versus actual connection state, owned-core transitions,
  operation serialization and conservative foreign-VPN conflict handling.
- Serialize commands from every OmaVLESS control surface so bar/TUI/CLI cannot
  race separate core transitions.
- OmaVLESS may stop cores it owns; it must not kill/reconfigure foreign VPNs or
  arbitrary TUN interfaces.
- Preserve current Mihomo behavior while extracting the smallest useful
  lifecycle boundary.
- T1 may subsume this track if the shared runtime lands first.

### K0 — fail-closed threat model and host integration design

- State: **planned design/security phase**.
- Goal: design a real kill switch which remains effective after the proxy core
  or UI crashes.
- Cover IPv4, IPv6, DNS, route replacement, suspend/resume, network changes,
  reconnect traffic, stale-rule cleanup, plugin/app disable/remove and emergency
  recovery.
- Determine the smallest privilege boundary before implementation. Do not add
  arbitrary `sudo`/`pkexec`/firewall commands to the normal backend/runtime.

### K1 — Full VPN fail-closed kill switch

- State: **planned after K0**.
- Initial scope: opt-in **Full VPN only**.
- Protection follows desired VPN state rather than core process liveness.
- If the owned core dies/restarts while the user still expects a connection,
  ordinary direct egress stays blocked until recovery or explicit disconnect.
- First slice is a kill switch, not permanent Lockdown.
- Likely requires an external OS networking policy; if a privileged helper is
  required, it must be optional, separately reviewed and accept only bounded
  fixed-purpose operations rather than arbitrary firewall rules.
- Forced core death, failed restart, network changes and removal cleanup are
  mandatory Omarchy tests.

### X0 — core-backend contract extraction

- State: **deferred until a real second-core need exists**.
- Extract only the Mihomo-specific lifecycle/config/capability surface required
  to add another backend. Do not perform a big-bang rewrite while Mihomo alone
  satisfies current product requirements.

### X1 — optional Xray Full VPN compatibility backend

- State: **deferred compatibility work**.
- Trigger: common real profiles require Xray semantics that supported Mihomo
  cannot represent without loss and the gap is confirmed to be core-level, not
  an OmaVLESS importer bug.
- Mihomo remains preferred/default and fully featured.
- Core choice is initially automatic/capability-driven, not a normal user
  preference toggle.
- **First production Xray slice supports Full VPN only.** Routing and Direct are
  unavailable; there is no RU/CN/IR/custom-rule/route-inspection parity claim.
- Xray is intended to own its own validated Full VPN capture path; do not use
  the older proposed hidden Mihomo-TUN + loopback-SOCKS + Xray two-process
  transport chain as the default architecture.
- One OmaVLESS-owned full-tunnel core may be active at a time.
- Every control surface renders the same selected-core/capability state from
  the runtime instead of implementing core selection independently.
- If K1 exists by then, Xray integrates with the same LeakProtection boundary
  instead of growing a second kill-switch implementation.

### R1 — backend-neutral routing research

- State: **deferred; not implied by X1**.
- Consider Xray Routing only when RU/CN/IR policies, custom rules, DNS,
  rule-data lifecycle, route inspection and leak-safety semantics can be
  preserved rather than approximated.

## Deferred compatibility research

Keep these out of P4 and the networking/application refactors unless real
demand/fixtures justify them:

- Hysteria2 Realm links;
- local Hysteria2 `handshake-timeout` without demonstrated value;
- TUIC v4 / TUIC 0-RTT without representative fixtures and replay-risk review;
- AnyTLS, Shadowsocks and VMess solely for protocol-count completeness;
- manual core-selection UX before a real operational need exists;
- permanent Lockdown semantics before K1 Full VPN kill-switch behavior is
  proven;
- arbitrary raw YAML merge/patch systems or untrusted provider-controlled
  runtime configuration as a shortcut around structured adapters.

## Cloud -> Omarchy handoff procedure

For every runtime PR the cloud agent records URL, base/head SHA, changed files,
completed cloud checks and every unchecked real-host gate. The Omarchy agent
then:

1. fetches and verifies the exact candidate head;
2. checks the diff and dependency base;
3. runs `omarchy plugin validate`, `./tests/run.sh`, shell/Python checks and
   `qmllint` with installed imports as applicable;
4. executes the PR-specific live TUN/systemd/core/server/network checklist;
5. keeps real credentials, profile/provider names and endpoints out of GitHub;
6. reports failures on the owning PR rather than patching only an integration
   branch;
7. reruns affected gates whenever the head changes;
8. merges only after the declared gates pass and explicit owner approval is
   given.

For TUI/control-plane work, the local gate additionally covers concurrent
plugin/TUI control, launch-or-focus, terminal close/reopen, theme changes,
resize behavior and confirmation that one UI cannot stall or replace the
canonical tunnel owner.

A stacked successor is rebased/retargeted only after its predecessor merges,
then receives its own checks. Do not create empty successor branches merely to
reserve names.
