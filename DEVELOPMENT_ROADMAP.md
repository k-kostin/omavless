# OmaVLESS development delivery roadmap

Status: active delivery ledger, updated 2026-08-25.

This document connects the code currently merged in `main`, open cloud PRs,
future implementation branches and the checks which can run only on a real
Omarchy machine. Protocol-level compatibility details remain in
[`PROTOCOL_ROADMAP.md`](PROTOCOL_ROADMAP.md).

## Status vocabulary

- **Merged** means runtime code and automated tests are in `main`.
- **Cloud-ready** means a Draft PR has passed available CI/static tests but
  still needs its listed Omarchy smoke tests.
- **Experimental** means the importer exists, but independent server/provider
  interoperability evidence is not yet sufficient to call it stable.
- **Planned** means no runtime support is present; documentation or design is
  not counted as implementation.
- **Deferred** means the item is intentionally outside the active merge chain.

## Current repository snapshot

The marketplace release candidate is `0.7.0` at
`69fe05b03129a23664fff3f8289821a7b7f80095`. Marketplace submission
[`#2311`](https://github.com/HANCORE-linux/omarchy-plugin-marketplace/issues/2311)
has been revalidated at that exact commit: Quattro validation passes, the
automated security baseline has no findings, and the earlier `needs-fixes`
label is removed. The remaining `security-review-required` label represents
the expected human review of the documented privilege, service-management and
installer capabilities.

Two OmaVLESS Draft PRs are active while that exact marketplace snapshot remains
unchanged:

1. [`#28`](https://github.com/k-kostin/omavless/pull/28) documents the
   WireGuard/AmneziaWG status and this delivery queue. It changes no runtime
   behavior.
2. [`#27`](https://github.com/k-kostin/omavless/pull/27) implements advanced
   read-only Mihomo diagnostics and adaptive polling. Cloud checks pass; the
   PR remains incomplete until its local Omarchy checklist passes.

Merged [`#29`](https://github.com/k-kostin/omavless/pull/29) completed the S0
lifecycle gate and produced the current marketplace candidate. Draft branches
may be rebased and tested in the cloud, but `main` should stay at the candidate
until submission `#2311` reaches a maintainer decision.

Everything else described as merged in the protocol roadmap is already in
`main`. In particular, routing presets and custom rules, onboarding and core
setup guidance, login autoconnect, favorites, redacted import preview, safe
diagnostic export and destination-route inspection are not future work.

## Active merge chain

### S0 — marketplace lifecycle and removal fix

- Branch: `codex/removal-runtime-cleanup`
- PR: `#29`
- State: **merged** at `69fe05b03129a23664fff3f8289821a7b7f80095`.
- Reason: Omarchy intentionally runs no repository uninstall hook. A generated
  full-route service must therefore stop and remove its runtime integration
  when its owning plugin is disabled or removed, without treating hot reloads
  and updates as removal.
- Evidence: exact-tree CI, 156 cloud tests including fake Omarchy/systemd
  end-to-end cleanup, fail-closed unit removal and credential-boundary checks.
- Local regression checklist: active and inactive login autoconnect, explicit
  disable, normal remove, already-disabled remove, hot reload preservation,
  private-data retention and unit/TUN residue.
- Marketplace gate: complete. Submission `#2311` is validated at the merged
  exact SHA and awaits only the expected human security review.
- Successor: D0.

### D0 — documentation and progress ledger

- Branch: `codex/amneziawg-roadmap-visibility`
- PR: `#28`
- State: cloud-ready, documentation only, rebased from the S0 candidate.
- Dependency: satisfied. The branch preserves S0's `Unreleased` lifecycle
  entry and removal documentation.
- Review: verify that README and both roadmaps distinguish Mihomo capability
  from OmaVLESS import support. No live VPN check is required.
- Hold as Draft until marketplace submission `#2311` reaches a maintainer
  decision, then merge before D1 so later PRs reference one canonical queue.

### D1 — advanced Mihomo diagnostics and adaptive polling

- Branch: `codex/advanced-mihomo-diagnostics`
- PR: `#27`
- State: cloud-ready, Omarchy smoke pending.
- Dependency: after D0 merges, update the branch from `main` and resolve the
  shared README/CHANGELOG `Unreleased` additions without dropping either set.
- Acceptance: complete every unchecked `Local Omarchy smoke required` item in
  the PR body, including private Unix-controller verification, stale-response
  handling, provider refresh semantics and background polling behavior.
- Successor: V0.

### V0 — existing experimental protocol validation gate

- Future branch: `codex/experimental-protocol-live-gates`
- State: planned after D1 merges.
- Goal: turn the remaining “works in generated YAML” evidence into a repeatable
  privacy-safe validation procedure before adding another credential family.
- Scope: an opt-in harness and a checked-in results template for VLESS XHTTP
  modes, VLESS Encryption/REALITY PQ, Trojan, Hysteria2 and TUIC v5. Real keys
  enter only through private files or stdin and are never committed, printed,
  placed in argv/environment or copied into CI artifacts.
- Cloud gate: parser/config fixtures, redaction tests and sanitized result
  schema.
- Omarchy gate: record `mihomo -v`; run installed-core config validation;
  exercise representative independent servers/providers; test Hysteria2/TUIC
  on UDP-friendly and UDP-restricted networks; record only protocol, feature,
  pass/fail category and a credential-free note.
- This gate may document failures without changing an adapter. Any fix found
  during live testing gets its own narrowly scoped follow-up PR.
- Successor: P4a.

### P4a — standard one-peer WireGuard `.conf`

- Future branch: `codex/wireguard-conf-import`
- State: planned; no runtime code exists.
- Dependency: V0 merged, current Mihomo source/release audit repeated.
- Scope: versioned structured private-profile storage, bounded one-interface /
  one-peer parser, redacted preview, Mihomo `wireguard` YAML, endpoint probe,
  safe export/QR behavior and store migration. Reject executable hooks,
  duplicate sections/keys, multiple peers and arbitrary YAML.
- Omarchy gate: real standard WireGuard profile, IPv4 and IPv6 where
  available, all three OmaVLESS routing policies, reconnect/autoconnect,
  update/remove rollback and proof that private/preshared keys do not reach
  public UI, logs, diagnostics, argv or environment.
- Successor: P4b.

### P4b — native AmneziaWG `.conf`

- Future branch: `codex/amneziawg-conf-import`
- State: planned; no runtime code exists.
- Dependency: P4a merged and live-tested; Mihomo 1.19.30+ capability gate.
- Scope: reuse P4a storage/parser boundaries, classify native Amnezia fields,
  validate generation-specific v1-v3.1 families and emit only documented
  `amnezia-wg-option` fields. Reject mixed/incomplete generations.
- Fixtures: redacted native profiles for at least one legacy AWG generation,
  v3.0 and v3.1. Legacy v1.5-only `J1`-`J3`/`ITime` remain rejected until a
  real fixture exists.
- Omarchy gate: validate the generated config and connect to real matching
  servers; verify exact old-core errors for v3/v3.1; leave the protocol marked
  experimental.
- Successor: P4c.

### P4c — AmneziaVPN guest `vpn://`

- Future branch: `codex/amnezia-vpn-key-import`
- State: planned after P4b.
- Scope: bounded offline decode of guest keys and explicit extraction of one
  WireGuard/AmneziaWG container through the P4a/P4b validators.
- Security boundary: never call an embedded API endpoint, import SSH/root or
  full-server-access credentials, choose a different protocol container
  implicitly, or retain the raw decoded document in public output.
- Omarchy gate: use a revocable guest key, then revoke it after testing; verify
  `.vpn`, clipboard and import-preview behavior without exposing the key.

## Implemented but not yet stable

The following items are runtime-complete in 0.7.0 but retain experimental
labels until V0 collects independent evidence:

- VLESS Encryption and REALITY PQ metadata;
- Trojan, including the supported TLS and REALITY transport combinations;
- Hysteria2, especially UDP restriction behavior and failure reporting;
- TUIC v5, including relay and congestion-controller combinations.

XHTTP modes and split upload/download settings also require representative
real TLS/REALITY keys even though their bounded mapping and current-core config
validation are already automated.

## Product tracks after the active protocol chain

These are valuable, but starting them before P4c would create overlapping
QML/backend/storage changes and make the evening merge sequence harder to
audit.

### I1 — i18n foundation

- Future branch: `codex/i18n-foundation`
- Centralize user-facing strings; English remains default/fallback and Russian
  is the first additional locale.
- Provider/profile names and backend/controller data are never translated.
- Prefer stable backend status/error codes localized in QML over complete
  backend-generated sentences.

### C1 — privacy-aware active connections

- Future branch: `codex/active-connections`
- Depends on D1's diagnostics page and controller boundary.
- Read-only host/process/chain/rule/traffic comes first, with strict bounds and
  redaction. Closing a connection is a later mutation with a separate UX and
  security review.

### S1 — App proxy without TUN

- Future design branch: `codex/system-proxy-design`
- Threat model and reversible state design precede implementation.
- User-facing name: “App proxy — only applications that honor proxy settings”.
- Preserve and transactionally restore the exact previous proxy settings,
  mark ownership, exclude localhost/controller/private ranges and never use a
  privileged helper. Do not present it as a full-device VPN or as another
  Full VPN / Routing / Direct routing policy.

## Deferred compatibility research

Do not put these items into P4 PRs:

- Hysteria2 Realm links;
- local Hysteria2 `handshake-timeout` unless live UDP tests prove user value;
- TUIC v4 and TUIC 0-RTT without representative fixtures and an explicit
  replay-risk decision;
- AnyTLS, Shadowsocks and VMess until demand and real fixtures justify them;
- the two-process Xray transport backend until Mihomo-native coverage is
  mature and a common core-level incompatibility justifies its lifecycle and
  threat-model cost.

## Evening Omarchy handoff procedure

For each PR, the cloud agent provides the URL, exact head SHA, changed files,
automated checks and an unchecked local-smoke list. The Omarchy agent then:

1. treats merged S0 as the baseline, then follows D0 → D1 → V0 → P4a → P4b →
   P4c and never tests a successor before its predecessor is merged;
2. fetches `origin` and verifies the PR head SHA;
3. checks that the diff contains only the named roadmap slice;
4. runs `omarchy plugin validate`, `./tests/run.sh`, `qmllint` with Omarchy
   imports, shell syntax checks and the PR-specific installed-core tests;
5. completes the live checklist without pasting real keys, profile names,
   endpoints or subscription URLs into GitHub;
6. reports failures on the same PR instead of fixing them in the successor;
7. merges only after the live gate passes, then starts the successor from the
   new `origin/main` rather than from a stale pre-merge branch.

No empty successor branch should be created in advance. Its base SHA is the
merged predecessor, so creating it early only increases rebase conflicts and
makes the audit trail ambiguous.
