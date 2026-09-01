# OmaVLESS development delivery roadmap

Status: active delivery ledger, updated 2026-08-31.

This file is the compact source of truth for **what happens next**, dependency
order and acceptance state. Detailed contracts live under `docs/roadmap/`.

Canonical references:

- [`docs/roadmap/RUST_MIGRATION.md`](docs/roadmap/RUST_MIGRATION.md) — selected
  Rust implementation direction, Python parity/retirement gates and R0-R6;
- [`docs/roadmap/ARCHITECTURE.md`](docs/roadmap/ARCHITECTURE.md) — shared Rust
  runtime/domain/core layers, lifecycle/exclusivity and fail-closed direction;
- [`docs/roadmap/CONTROL_PLANE.md`](docs/roadmap/CONTROL_PLANE.md) — v1 IPC,
  ownership, migration and T1 runtime cutover contract;
- [`docs/roadmap/TUI_APP.md`](docs/roadmap/TUI_APP.md) — Rust/Ratatui TUI product
  and client contract;
- [`docs/roadmap/PLATFORM.md`](docs/roadmap/PLATFORM.md) — Arch + NixOS host and
  distribution boundary;
- [`docs/roadmap/NIX_PORTABILITY.md`](docs/roadmap/NIX_PORTABILITY.md) — Nix
  immutable-path/capability/service consequences and acceptance matrix;
- [`docs/roadmap/KILL_SWITCH.md`](docs/roadmap/KILL_SWITCH.md) — K0/K1
  fail-closed threat model and privileged boundary;
- [`docs/roadmap/I18N.md`](docs/roadmap/I18N.md) — localization ownership and
  remaining surface batches;
- [`docs/roadmap/DEVELOPMENT_WORKFLOW.md`](docs/roadmap/DEVELOPMENT_WORKFLOW.md)
  — branch/PR/evidence workflow;
- [`PROTOCOL_ROADMAP.md`](PROTOCOL_ROADMAP.md) — protocol/share-format
  compatibility contract.

Where an older document still describes Python versus compiled runtime as an
open choice, `RUST_MIGRATION.md` is authoritative: **Rust is selected for the
future runtime/domain/CLI/TUI; Python is the current plugin reference and
migration oracle.** Where older wording says Arch/AUR is the only future host,
`PLATFORM.md` is authoritative: Arch and NixOS are the initial host families.

## 1. Delivery principles

### One source of truth

`main` is the only long-lived development source of truth. Runtime/network/
security/migration changes merge only after their declared exact-head gates.
Documentation-only changes do not invent a live VPN test.

The published marketplace 0.7.0 snapshot remains exact reviewed commit
`69fe05b03129a23664fff3f8289821a7b7f80095`. Moving `main` is not automatically
a new marketplace release.

### Product maturity, migration maturity and host maturity are separate

- A protocol may be merged and still be **Experimental**.
- A Rust implementation may have parity tests but not yet own production
  behavior.
- A shared runtime may work on Arch while NixOS packaging remains unaccepted.

Do not collapse these axes into one "done" label.

### No big-bang rewrite

The current Python backend remains the reference production behavior until a
bounded Rust slice passes its parity and host gates. Rust replaces Python
incrementally. Do not create a permanent dual implementation.

### No premature TUI

Rust + Ratatui is the selected full TUI stack, but **production TUI work starts
only after R6**, when the normal runtime/application path no longer requires
Python. UI mockups/research may happen earlier; product TUI implementation may
not be used to hide unfinished migration.

## 2. Current repository snapshot

Already accepted:

- S0 / PR `#29` marketplace lifecycle/removal hardening, merged at the published
  `69fe05b03129a23664fff3f8289821a7b7f80095` baseline;
- D0 / PR `#28` documentation/protocol queue;
- D1 / PR `#27` advanced Mihomo diagnostics and selector-readiness fix, merged at
  `5a50d59ae9fada9c61ee3ed9037f72e35bf09853`;
- file-picker onboarding / PR `#41`;
- unified top-level subscription import / PR `#42`;
- panel-local Tab/Shift+Tab navigation / PR `#43`;
- onboarding/startup localization / PR `#50`;
- routing-tools and routing-preset localization;
- advanced diagnostics localization;
- Arch + NixOS platform planning / PR `#51`, merged at
  `ec78bd6bc2dec74cbeec80121aca878e2ade9412`.

V0 / PR `#30` remains Draft at exact tested head
`a643db595ad5369b5fea200ebc601b4f0f70f18f`. The representative VLESS XHTTP
case passed; VLESS Encryption/REALITY PQ, Trojan, Hysteria2 and TUIC v5 fixtures
remain unavailable. Do not fabricate them or call V0 complete.

Missing V0 fixtures block claims about those protocol families and P4 protocol
expansion. They **do not block R0/R1/R2 Rust migration work** on already-defined
semantics.

## 3. The new global sequence

Development now has two active lanes which intentionally converge.

### Lane P — finish the plugin-version product surface

Continue work that is valuable in the current Omarchy plugin and does not create
large new Python runtime surfaces merely to be rewritten:

- remaining QML/localization batches;
- current UI correctness/accessibility/navigation fixes;
- current routing/import/subscription presentation and diagnostics polish;
- opportunistic V0 live validation when legitimate fixtures become available;
- P4 WireGuard/AmneziaWG **after both V0 and R2**, so new protocol parsers are
  Rust-first while still becoming usable by the plugin through the migration
  bridge.

"Plugin-version complete" does **not** mean squeezing every future application
feature into the bar. Full active-connections workspaces, App proxy host
ownership, kill switch runtime, Xray backend and the large TUI remain later
runtime/application tracks.

### Lane R — migrate backend/runtime to Rust

Start now and continue in bounded parity-gated slices:

```text
R0 workspace/parity harness
 -> R1 control protocol
 -> R2 profile/protocol adapters
 -> R3 store/subscriptions/routing
 -> R4 Mihomo/probes/diagnostics/host readiness
 -> R5/T1 canonical Rust runtime cutover
 -> R6 Python runtime retirement
 -> T2 Ratatui TUI
```

R0/R1 can run while plugin QML work continues and while V0 fixtures are missing.

### Features deliberately moved behind Rust runtime

Do not build these as deep new Python-plugin subsystems first:

- C1 full privacy-aware active-connections workspace/mutations;
- S1 App proxy state ownership;
- K1 fail-closed kill switch implementation;
- X1 Xray compatibility backend;
- background scheduling/recovery features which need a persistent owner;
- TUI application implementation.

Their design contracts remain useful, but implementation should attach to the
Rust runtime so they are not immediately rewritten.

## 4. Immediate work queue

The practical next sessions may therefore be heavily Rust-focused without
abandoning the plugin.

### P-I1 — finish remaining plugin localization/surface batches

State: **active independent plugin work**.

Current foundation is merged. Routing tools and the first-use routing-preset
confirmation and advanced diagnostics are now localized. Remaining batches
include secondary dialogs and legacy/backend-facing prose where it is actually
presented to users. Keep provider/profile/core-controlled data untranslated and
preserve English fallback.

These QML/i18n slices can merge independently of R0/R1 when their exact-head
visual/contract gates pass.

### V0 — existing experimental protocol validation

State: **partially live-validated; fixture constrained**.

- PR: `#30`, Draft.
- Exact tested head: `a643db595ad5369b5fea200ebc601b4f0f70f18f`.
- XHTTP representative case: PASS.
- Missing: VLESS Encryption/REALITY PQ, Trojan, Hysteria2 and TUIC v5 fixtures;
  safe UDP-restricted Hysteria2 half also unavailable.
- Do not ceremonially rebase solely because unrelated `main` docs/Rust work
  advances. Rebase only when the harness/runtime relationship materially needs
  it or remaining evidence becomes available.
- V0 continues in parallel with R0-R2.

### R0 — Rust workspace and differential infrastructure

State: **merged in PR #55; production runtime unchanged**.

Accepted implementation head: `52d9c28fb87320ae65a5e37afa6c312f2a614083`.
Merge commit: `f88a148c7146281fdda25249bae96bea5ed82930`.

Scope:

- introduce Cargo workspace without changing production tunnel ownership;
- commit application `Cargo.lock`;
- CI for build/test/clippy/format as appropriate;
- credential-free golden/differential fixture conventions;
- bounded parity runner and privacy rules;
- crate/dependency direction documented;
- no QML cutover, daemon or TUI.

The first checkpoint provides the pinned Cargo workspace, application lockfile,
strict sanitized result envelope, bounded comparator and CI gates. It does not
yet move a production subsystem; R1 supplies the first Python/Rust differential
adapter against the existing control-protocol corpus.

Merge gate: deterministic cloud/static checks plus proof that current plugin
runtime behavior is unchanged.

### R1 — Rust control-protocol parity

State: **merged in PR #56; Rust is canonical for future control-protocol work,
Python retained as migration oracle**.

Accepted implementation head: `227c6cc05dfad6f2e954b7c5461c479f592ba380`.
Merge commit: `105aae681e12bae955b8fdbfcf017d8e493b72a8`.

The existing Python T1a implementation and `tests/control_protocol_cases/`
become the first language-neutral parity corpus.

Scope:

- strict UTF-8/NDJSON frame mechanics;
- duplicate-key/size/depth/version/id/revision/operation validation;
- success/error envelopes and bounded stream helpers;
- stable safe errors;
- differential Python/Rust tests;
- fuzz/property coverage where useful.

After parity, Rust is the canonical future control-protocol implementation;
Python remains temporarily as migration oracle, not a second runtime.

## 5. Plugin protocol expansion now depends on Rust adapter boundary

### R2 — Rust profile/protocol adapter parity

State: **protocol classification foundation merged in PR #58, bounded VLESS
authority/public-preview parity merged in PR #60, and strict VLESS query/coarse
transport-security metadata parity merged in PR #62. Vision-flow and
packet-encoding parity merged in PR #64, REALITY key/short-ID/PQ parity in PR
#66, VLESS Encryption parity in PR #70, transport-option parity in PR #72,
XHTTP `extra` decoder/shape parity in PR #74, normalized top-level XHTTP
option parity in PR #78, nested `downloadSettings` parity in PR #80 and
canonical profile/identity/preview/rendering parity in PR #83, followed by the
R2j pure-VLESS consolidation/privacy audit in PR #86. Canonical Trojan,
Hysteria2 and TUIC v5 adapters, shared profile-URI dispatch and credential-safe
rendering parity merged with the accepted R2-R4 foundation in PR #90; R2
adapter parity is complete without a production runtime cutover**.

Accepted classification head: `0c11682284d451e5f43bd1ffc4c116a67b13fce9`.
Classification merge commit: `1b481f60710fd84342aa5c01c4f1a73a76e88af1`.

Accepted VLESS authority head: `be9668d34aab3ca26aaedd25896c913feb0a7910`.
VLESS authority merge commit: `9d6d12f582689d75c19ed4ab823250bd2668b929`.

Accepted VLESS query-metadata head: `5d477ce655a93db2343ecf412ad2b30d36c5ad54`.
VLESS query-metadata merge commit: `02535146586146fbe20a73a024673c1296a0089a`.

Accepted VLESS flow/packet head: `d90252b2dc92bbbc34dbd481fc86278b6573b950`.
VLESS flow/packet merge commit: `6c688b848db97cfaf4b51f5dc8de97d41a13e450`.

Accepted VLESS REALITY head: `22c60fae517b8f521d5c311273ce2ec25517cd26`.
VLESS REALITY merge commit: `6e69bd7a32427de045938911702665accc67d7f1`.
Canonical-Base64 correction head: `76d7132d55504dbedbab822c23160b7483ec1b52`.
Canonical-Base64 correction merge: `89a4d4e3ac77ac538bf884c79b308e974a03e710`.

Accepted VLESS Encryption head: `bb4f4aeb653cc9f133538041af2663cb3dcf4d39`.
VLESS Encryption merge commit: `bf16228e71fc48307c3beeed89927c79250c17ea`.

Accepted VLESS transport-option head:
`8f65628f57b8a7caaa801beb6b44a3a7894bbed6`.
VLESS transport-option merge commit:
`256669d7155766c9d1a0e05fe41a186cbd639458`.

Accepted XHTTP `extra` shape head:
`bf8aa50e3b3c1fa67cc9baf0924fe38d2a5da469`.
XHTTP `extra` shape merge commit:
`a97871703c27c403f8be715889daa82ee755821e`.

Accepted normalized XHTTP-option head:
`50ae48f73448c720c41d89defe7d843ac0ed35ba`.
Normalized XHTTP-option merge commit:
`fb08101e7d15b5484a1c4d729cf715e171b87161`.

Accepted XHTTP `downloadSettings` head:
`58781daebb728a12d2639a5621d698982cfd03db`.
XHTTP `downloadSettings` merge commit:
`f4c9d9eccfd0b76439c66265b700c19847b7f445`.

Accepted canonical VLESS head:
`7b186f79349b7d81101b55945154db44b3e1c8f3`.
Canonical VLESS merge commit:
`0b2a7d5f147c1c373ae491f1206695532e8c07d7`.

Accepted R2j VLESS-consolidation head:
`d82bf63bec44b259e67f422f967509d417fad4bc`.
R2j VLESS-consolidation merge commit:
`5ecbd7d13c1fb0ae1fb73bced61b40bd4e1bf184`.

Accepted R2-R4 foundation head:
`8efd96a3a8fe34d8765224bc1a9a66c9a19bc86b`.
R2-R4 foundation merge commit:
`010938294f89c034392fa3cc6836bed67d4795d9`.

Continue as `codex/rust-profile-adapters-*` with narrow slices rather than one
giant conversion. Rust now owns the future bounded VLESS authority model,
scheme/UUID/host/port validation, suggested-label decoding and credential-safe
public preview/fact projection. It also owns the future strict query envelope,
allowed-field/alias/provider-metadata checks and coarse transport, security,
certificate-verification and XHTTP-mode vocabulary, plus source-preserving
Vision-flow/Mihomo normalization, packet-encoding semantics and bounded REALITY
key/short-ID/PQ validation. It now also owns the future bounded VLESS Encryption
grammar, canonical client-key validation, mode/RTT vocabulary and padding
limits, plus established transport-option facts for path normalization,
host/service-name/fingerprint presence, ALPN splitting/trimming, alias
conflicts and TCP-header validation. Rust also owns the future bounded XHTTP
`extra` decoder/shape contract: 12-KiB input, duplicate rejection at every
object depth, depth/value/key/string bounds, accepted Python numeric forms and
fixed credential-safe errors. Rust now also owns normalized top-level XHTTP
options: allowlists, host/path/mode compatibility, recursive-extra rejection,
server-only defaults, bounded headers, Python-compatible scalar/boolean/enum/
token handling, session constraints and `xmux`/reuse semantics. Rust now also
owns the future private normalized XHTTP `downloadSettings` model and its
Python-order validation of the secondary endpoint, network/security/TLS/REALITY,
nested transport, headers, defaults, aliases and recursive-download rejection.
Private endpoint, SNI, REALITY key/short ID, spider path and header values remain
redacted. Rust now also owns the future canonical private VLESS model, redacted
preview semantics, normalized subscription identity and semantic Mihomo proxy
rendering. Synthetic Python/Rust differentials cover 31 authority, 47
query-metadata, 19 flow/packet, 43 REALITY, 42 Encryption, 34 transport-option,
62 XHTTP-shape, 115 XHTTP-option, 188 XHTTP-download and 49 canonical VLESS
cases without emitting private input or key material. Python remains the
production owner and migration oracle; neither PR #83 nor the R2j audit cut over
runtime behavior. The R2j audit kept all 49 canonical differential cases green,
fixed the last credential-bearing authority-preview `Debug` representation and
finished with 71 Rust and 259 Python/QML tests passing on Try Omarchy ARM64.

R2 is complete at the adapter/parity boundary. Python remains the installed
production owner and migration oracle until R5/T1; completing R2 does not
authorize new protocol-family work or claim a runtime cutover.

Migrate existing validated semantics:

- protocol classification;
- VLESS parsing and transports/security metadata;
- Trojan;
- Hysteria2;
- TUIC v5;
- canonical profile model;
- redacted import preview;
- endpoint extraction;
- subscription identity inputs;
- Mihomo profile/YAML rendering semantics.

Each protocol may be a separate PR if that produces clearer parity evidence.
The Python implementation remains oracle until the corresponding Rust adapter
is accepted.

### P4a — standard one-peer WireGuard `.conf`

State: **planned after V0 + R2**.

This dependency is deliberate. Do not implement P4a as a large new Python-only
parser simply because the current plugin is Python-backed.

Scope remains:

- bounded one-interface/one-peer parser;
- versioned structured private representation;
- redacted preview;
- Mihomo WireGuard generation;
- endpoint/probe compatibility;
- safe explicit export;
- reject executable hooks, extra peers, ambiguous keys and arbitrary YAML.

The adapter itself is Rust-first. The current QML plugin receives the same
semantic behavior through the migration/runtime bridge.

Acceptance still requires a real matching server, IPv4/IPv6 where available,
Full/Routing/Direct, reconnect/autoconnect/update/remove rollback and proof that
reusable keys never reach public UI/logs/diagnostics/argv/environment.

### P4b — native AmneziaWG `.conf`

State: **planned after P4a accepted**.

Reuse the R2/P4a Rust storage/parser boundary; accept only validated AWG
families and emit documented Mihomo fields. Real matching server fixtures and
old-core compatibility errors remain mandatory. Retain Experimental product
label initially.

### P4c — AmneziaVPN guest `vpn://`

State: **planned after P4b**.

Bounded offline guest-key decoding only; extract one WG/AWG container through
the exact Rust P4a/P4b validators. No embedded API call, SSH/root/admin
credential handling or implicit protocol fallback.

## 6. Continue migration beneath the finished plugin surface

### R3 — store, subscriptions and routing semantics

State: **accepted in PR #90; production ownership cutover remains R5/T1**.

Migrate with differential tests:

- private store schema validation and migrations;
- active/last/favorites/startup semantic state;
- subscription parse/merge/identity/atomic commit model;
- import/export transformations;
- routing preset/custom-rule semantic model;
- deterministic config assembly around Rust adapters.

Remote fetch/process ownership may remain Python until the R5/T1 runtime
cutover. State semantics must already be language-neutral.

The accepted Rust foundation now covers private-store schema/migration and
active/last/favorite/startup state, subscription refresh planning and identity
preservation, strict unified-import classification, routing rules and modes,
deterministic config assembly and bounded same-user atomic `0600` store
replacement. Remote fetching remains in the Python compatibility runtime.

### R4 — Mihomo operations, probes, diagnostics and host readiness

State: **accepted in PR #90; lifecycle ownership cutover remains R5/T1**.

Migrate:

- stable Mihomo entry-point discovery;
- config validation subprocess behavior;
- private controller client;
- provider/rule/route diagnostics;
- isolated latency/probe orchestration;
- traffic/status sampling;
- owned process/user-service observation;
- host capability/doctor model for Arch and NixOS.

Tokio/asynchronous Rust is appropriate for bounded I/O, but must not silently
change timeout, retry, cancellation or concurrency semantics.

Real Try Omarchy integration gates supplement differential tests for every
OS/network-facing cutover.

The accepted Rust foundation now covers fixed-argv Mihomo discovery and config
validation, bounded private Unix-controller HTTP/JSON, credential-safe
diagnostics and traffic projections, deterministic isolated-probe scheduling,
process/service/TUN observation and Arch/NixOS host readiness. Acceptance used
Mihomo 1.19.30 on Try Omarchy ARM64 with 115 Rust and 259 Python/QML tests; see
`docs/testing/TRY_OMARCHY_R2_R4_RUST_ACCEPTANCE_2026-09-01.md`. Python still
owns the live service and tunnel.

## 7. Canonical runtime cutover

### T0 / T0p — design contracts

State: **accepted**.

The v1 semantic control plane, one-owner model, staged migration and Arch/NixOS
host boundary are established. Implementation language is no longer open: Rust
owns the target runtime.

### R5 / T1 — Rust shared runtime / daemon foundation

State: **in progress; foundations accepted, production ownership cutover not
started**.

Accepted incremental checkpoints:

- PR #96: primary `omavless` binary, private same-UID Unix control socket,
  singleton owner lock and read-only hello/status/capability surface;
- PR #97: bounded private desired-state schema, pure restart reconciliation,
  read-only preflight and hardened package-owned user-unit contract;
- PR #98: one private canonical profile dispatcher and explicit Mihomo renderers
  for every currently supported profile family;
- PR #99: read-only v1-v3 private-store projection/config preparation, a
  Python/Rust differential corpus and real existing-store + installed-Mihomo
  preflight evidence;
- PR #100: bounded mutation queue, expected-revision checks, urgent-disconnect
  ordering and credential-free operation-ID replay cache.
- PR #102: fixed-argv, parent-owned Mihomo child supervision with bounded
  private-controller readiness, graceful termination and forced-stop fallback.

These checkpoints deliberately report `runtimeOwnership: false` and do not
expose lifecycle mutations. The current QML -> `backend.py` path remains the
only production owner. R5 is not complete until the explicit transactional
cutover, transactional lifecycle actions, restart reconciliation, plugin bridge
and rollback gates below pass.

This is one track, not a Rust daemon plus a separate T1 daemon.

The primary `omavless` executable gains `daemon` and semantic CLI behavior and
becomes the single canonical owner of:

- desired/actual connection state;
- profile/subscription/store mutations;
- routing mode;
- owned Mihomo lifecycle;
- background work;
- mutation serialization;
- bounded diagnostics/events.

Required cutover work:

- private `0600` Unix socket below a `0700` runtime directory;
- same-UID peer verification and singleton owner lock;
- `system.hello`, `status.get`, capabilities and mutation dispatch;
- desired/actual restart reconciliation;
- serialized connect/disconnect/mode/profile mutations;
- plugin compatibility bridge through semantic CLI/socket;
- service state control separated from package/service provisioning;
- no package-manager or raw systemctl API exposed to clients.

The existing `backend.py` becomes explicit disconnected rollback/reference only
during the compatibility window. It may not remain a second active owner.

### R5a — Arch host/package path

State: **part of T1 acceptance**.

- package primary `omavless` binary and runtime user unit;
- explicit Mihomo/TUN privilege setup;
- plugin -> Rust runtime migration from existing private store/service;
- Full/Routing/Direct lifecycle parity;
- update/remove/rollback and restart reconciliation.

### R5n — NixOS host path

State: **second host implementation once binary/service contract is stable**.

- Nix package/module/wrapper integration;
- stable executable paths across generations;
- no direct Arch-style `setcap` on immutable store binaries;
- user-service lifecycle;
- generation update/rollback;
- garbage-collection/stale-store-path regression;
- privilege-negative cases;
- same private store/control-plane semantics.

Nix support is not inferred from Arch/Try Omarchy evidence.

## 8. Native-runtime gate before TUI

### R6 — Python runtime retirement

State: **hard prerequisite for T2**.

R6 is complete only when:

- all production backend operations required by the plugin are Rust-owned;
- normal connect/routing/subscription/diagnostic/startup/lifecycle behavior works
  with Python unavailable;
- `backend.py` is not spawned by the normal application/plugin path;
- no venv, pip or Python package setup is required;
- the primary package runs the Rust `omavless` executable;
- existing private stores migrate without credential loss or duplicated owners;
- rollback/recovery was tested before legacy owner code is removed;
- language-neutral regression corpora survive removal of the Python oracle;
- final removal does not weaken current plugin functionality.

Python can remain in repository history or narrowly scoped developer tooling only
when there is a concrete reason, but it is not a product runtime dependency.

## 9. TUI / application track

### T2 — Rust + Ratatui TUI MVP

State: **planned only after R6**.

The first full application UI is Rust + Ratatui, using a reviewed terminal
backend such as Crossterm. It is a client of the already accepted Rust runtime;
it does not call Python or start Mihomo directly.

MVP:

- dashboard with connection/core/policy/protection status;
- profiles and grouped subscriptions;
- connect/disconnect and supported modes;
- search/favorites;
- one/all latency tests;
- live rates/totals and active connection count;
- profile/core details;
- subscription refresh;
- settings/diagnostics entry points;
- bounded local activity/log view;
- responsive keyboard-first navigation and help.

Omarchy plugin gains `Open app` launch-or-focus and remains the compact quick
control surface. Standalone Arch/Nix use the same TUI/runtime with a default
non-Omarchy theme when needed.

Acceptance includes concurrent plugin/TUI mutations, terminal close/reopen,
resize, theme changes on Omarchy, runtime restart and proof of one canonical
owner/core/TUN.

### T3 — operator/observability workspace

State: **after T2 and Rust runtime prerequisites**.

Integrate deeper D1/C1 concepts into TUI views:

- rules/providers/route diagnostics;
- privacy-aware active connections;
- later separately reviewed close-connection mutation;
- richer traffic history;
- bounded local logs/activity;
- read-only doctor/health output.

The full connection table belongs naturally here rather than being forced into
the bar during plugin closure.

### T4 — background/management quality of life

State: **later independent Rust-runtime slices**.

Candidates: subscription schedules, provider quota/expiry metadata,
backup/restore, suspend/network recovery, structured DNS controls and service
routing templates. Each keeps its own threat/host acceptance gate.

### G1 — optional desktop GUI research

State: **deferred until the Rust runtime + TUI are stable**.

GPUI is a preferred research candidate, not a daemon/TUI dependency. A future
GUI must be another semantic client. Prefer a separate GUI binary/package so a
headless/TUI installation does not pull in a graphics stack solely to run the
VPN runtime.

## 10. Networking/security tracks after Rust ownership

### C1 — privacy-aware active connections

State: **backend/operator implementation deferred to Rust runtime/T3**.

D1 already provides useful controller/diagnostic foundations. The full read-only
connection/process/chain view should be implemented in Rust and presented
primarily in TUI. Closing a connection remains a separate mutation/security
slice.

### S1 — App proxy without TUN

State: **design retained; implementation after Rust runtime ownership**.

Preserve/restore exact prior proxy state transactionally. On Omarchy cover both
desktop proxy state and systemd/UWSM user-manager environment; on standalone
NixOS define the equivalent host contract separately. No privileged helper and
no generic reset-to-default behavior.

### K0 — fail-closed threat model

State: **accepted design/security checkpoint**.

Root NetGuard + fixed nftables policy remains the selected privileged boundary.
The normal runtime/UI gets no arbitrary firewall/root command surface. Arch and
NixOS provisioning remain separate host implementations.

### K1 — Full VPN fail-closed kill switch

State: **implementation-ready by design, but implementation waits for Rust
canonical runtime**.

Initial scope remains opt-in Full VPN only. Protection follows desired tunnel
state through core/runtime failure; explicit disconnect disarms it. Deliver as
separate privileged protocol/service/runtime/acceptance slices, not as a large
Python-plugin addition.

### X0 / X1 — core backend abstraction and optional Xray

State: **deferred until Rust runtime and a real compatibility need**.

Mihomo remains primary. Extract only the Rust `CoreBackend` surface needed by a
confirmed Xray-only semantic gap. First Xray production slice is Full VPN only;
Routing/Direct parity is not implied.

### R-routing — backend-neutral Xray routing research

State: **deferred**.

Only consider after exact RU/CN/IR/custom-rule/DNS/route-inspection/leak-safety
semantics can be preserved rather than approximated.

## 11. Implemented but still Experimental

The following current features remain Experimental until broader V0 evidence is
available:

- VLESS Encryption and REALITY PQ metadata;
- Trojan supported TLS/REALITY combinations;
- Hysteria2, especially UDP-restricted behavior;
- TUIC v5;
- representative advanced XHTTP modes/split settings.

Rust parity does not by itself promote their product maturity. Migration
correctness and provider interoperability are separate evidence.

## 12. Deferred compatibility research

Keep outside active protocol/runtime work unless real demand/fixtures justify:

- Hysteria2 Realm;
- local Hysteria2 `handshake-timeout` without demonstrated value;
- TUIC v4 / TUIC 0-RTT without fixtures and replay-risk review;
- AnyTLS, Shadowsocks and VMess solely for protocol-count completeness;
- manual core-selection UX without operational need;
- permanent Lockdown before K1 kill-switch behavior is proven;
- arbitrary raw YAML merge/patch systems or provider-controlled runtime config.

## 13. Branch and evidence policy

Do not maintain permanent `alpha`/`beta` branches. Use narrow branches + Draft
PRs; a temporary beta is only a named integration/soak assembly.

For every runtime or Rust migration PR record:

- base/head exact SHA;
- roadmap stage and owning subsystem;
- Python owner before / Rust owner after, when applicable;
- differential/golden evidence;
- intentional differences;
- cloud/static checks;
- exact local host gates still required;
- privacy treatment of fixtures;
- rollback/oracle status;
- production path change.

A Rust PR whose tests only exercise Rust is not sufficient migration evidence.
Use the existing behavior or a pre-established language-neutral contract as the
comparison source.

For current plugin/runtime work, Try Omarchy ARM64 remains the normal local
integration environment. Bare-metal becomes mandatory only for concrete
hardware-sensitive cases. Future Arch/Nix packaging evidence is recorded
separately and one host never proves another.

## 14. Current priority in one sentence

**Finish bounded plugin-facing work while immediately building R0/R1/R2; add
new protocol parsers only after the Rust adapter boundary; complete R3/R4 and
Rust T1 cutover; retire Python at R6; then build the Ratatui TUI on the already
native Arch/Nix-ready runtime.**
