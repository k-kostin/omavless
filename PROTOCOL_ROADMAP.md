# OmaVLESS protocol roadmap

Status: 0.7.0 product baseline and forward compatibility roadmap,
updated 2026-09-03.

This document specifies protocol/share-format support and compatibility
boundaries. Delivery order lives in [`DEVELOPMENT_ROADMAP.md`](DEVELOPMENT_ROADMAP.md).
Implementation-language/migration authority is
[`docs/roadmap/RUST_MIGRATION.md`](docs/roadmap/RUST_MIGRATION.md).

`Implemented` means the parser, redaction boundary, current core mapping and
automated tests are in accepted history. `Experimental` means implementation
exists but broader independent server/provider interoperability evidence is
still being collected.

## Product direction

Mihomo remains the primary/preferred core. Current Full VPN, Routing and Direct,
country policies, rule providers, private controller, route inspection and
traffic diagnostics remain reference behavior.

Protocol support is added through strict profile adapters above runtime/core.
A profile is never accepted merely because an upstream core can parse a similar
option. OmaVLESS must understand, bound, redact and validate every
share-controlled field it stores or emits.

Existing adapters and the canonical runtime are Rust-owned after completed
R2/R6; QML is their client. Differential/golden parity retains the earlier
adapter semantics. **New large protocol adapters remain Rust-first.**

P4 WireGuard/AmneziaWG requires the completed R2 Rust protocol/profile adapter
boundary. V0 continues as an independent evidence track for already implemented
VLESS/Trojan/Hysteria2/TUIC families; missing V0 provider fixtures no longer
block P4 implementation. P4 has its own private-fixture, installed-core,
security and live-host gates. Its current development candidate uses the same
Rust owner but does not inherit those independent interoperability results.

Do not revive a Python-only P4 implementation. The current QML plugin consumes
the registered Rust-owned protocol/data path; its installed/live qualification
is separate from source registration.

A future Xray backend remains a narrow compatibility mechanism for common
semantics Mihomo cannot represent without loss. It is not a replacement for
Mihomo or initially a user preference. Its first production slice, if justified,
is Full VPN only.

Detailed architecture: [`docs/roadmap/ARCHITECTURE.md`](docs/roadmap/ARCHITECTURE.md).

## Protocol status at a glance

| Family | Status | Next gate |
| --- | --- | --- |
| VLESS | Implemented | R2 Rust parity + continuing XHTTP/PQ interoperability evidence |
| Trojan | Experimental implementation | R2 Rust parity + independent validation |
| Hysteria2 | Experimental implementation | R2 Rust parity + UDP-friendly/restricted validation |
| TUIC v5 | Experimental implementation | R2 Rust parity + independent validation |
| Standard WireGuard `.conf` | Development native-data/runtime candidate | Exact installed matching-server/mode/lifecycle gates |
| Native AmneziaWG `.conf` | Experimental managed IPv4 candidate | Exact AWG interoperability and lifecycle gates |
| AmneziaVPN guest `vpn://` | Bounded direct/structured Rust adapter in the development candidate | Same native-data and narrower managed-execution gates |

Mihomo supporting an outbound does not mean OmaVLESS supports the provider
key/config format. Manifest/marketplace description lists only implemented
product support.

## Universal security gates

Every protocol/key-format slice preserves:

1. bounded strict parsing of untrusted input;
2. redacted preview before storage;
3. no reusable credential in public status/diagnostics/logs/argv/environment;
4. private files use final restrictive permissions before secret bytes;
5. generated configuration validation with installed core;
6. explicit rejection of unknown/ambiguous security-sensitive fields;
7. optional end-to-end testing without credential leakage;
8. real host/TUN/server validation where runtime behavior depends on host;
9. for Rust migration/new Rust adapters, language-neutral fixtures and parity
   with accepted semantics where an older implementation exists.

Experimental status does not relax these gates.

## Upstream source/release gate

Every protocol/key-format PR repeats an upstream audit rather than relying on an
old roadmap conclusion.

For Mihomo-native support:

1. review official Mihomo proxy documentation;
2. inspect corresponding option structures/parser at pinned tag/commit;
3. review latest stable release and relevant releases since minimum core;
4. compare original protocol/share specification;
5. classify fields as share-controlled, local-only, version-gated or
   unsupported;
6. add explicit minimum-version gating where needed;
7. validate representative generated config against installed core before
   runtime merge.

A relevant security advisory blocks merge until minimum version/upgrade guidance
is reconciled.

Future Xray-specific slices repeat equivalent official Xray source/docs/release
audit. Third-party client dialect docs are evidence, not authority for another
core's runtime schema.

## Compatibility boundary

### OmaVLESS importer gap

The selected core supports a feature but OmaVLESS does not yet safely parse,
validate or represent its share input. This is an adapter task; it does not
justify changing cores.

### Core gap

Mihomo and Xray intentionally/structurally differ. OmaVLESS may explain the
difference but must not imply translation makes them identical. Xray remains
reserved for confirmed common core-level gaps after Mihomo-native support is
mature.

Until an Xray path is explicitly implemented/live-tested, a profile requiring
unsupported Xray-only semantics gets a precise compatibility error.

## 0.7.0 implementation baseline

The initial protocol sequence through PR #16 established:

- VLESS/REALITY/XHTTP handling;
- country routing and Settings;
- login autoconnect/custom routing tools;
- favorites, redacted import/diagnostics and safe export boundaries;
- protocol-neutral adapter/store discriminator;
- experimental Trojan, Hysteria2 and TUIC v5 import;
- marketplace security preflight.

Later lifecycle/removal/diagnostic/UX hardening is in merged history. Current
sequence is maintained in `DEVELOPMENT_ROADMAP.md`; missing V0 fixtures do not
block R0-R2 migration of already accepted adapter semantics.

## Phase V1 — strict VLESS semantics

Status: implemented/merged.

Scope includes:

- bounded `packetEncoding` (`xudp`, `packetaddr` or absent);
- Mihomo-supported XHTTP modes;
- explicit distinction between Xray Vision share variants while documenting
  Mihomo actual flow behavior;
- explicit Xray-only `spx`/spider-path compatibility note rather than invented
  Mihomo option;
- rejection of Xray-only ML-DSA verification metadata not safely representable;
- credential-free errors/compatibility notes.

R2 must preserve these accepted semantics in Rust. A Rust parser is not allowed
to become more permissive merely because a crate/library can parse additional
URI fields.

## Phase V2 — bounded XHTTP `extra`

Status: implemented/merged; broader provider evidence remains required.

Scope:

- bounded URL-encoded JSON object;
- whitelist-only headers/padding/session/sequence/upload/reuse/XMUX/download
  structures representable by current Mihomo;
- explicit camelCase -> Mihomo mapping;
- rejection of unknown/unsafe nested structures;
- explicit split upload/download and inherited transport/security limitations.

R2 parity includes representative accepted/rejected XHTTP fixtures and redacted
preview/generation behavior.

## Phase V3 — experimental VLESS Encryption and REALITY PQ

Status: implemented/merged as Experimental.

Scope:

- bounded Mihomo-compatible VLESS Encryption material treated as opaque secret;
- supported post-quantum REALITY metadata;
- strict encryption/PQ redaction;
- explicit experimental/fingerprint warning;
- no silent downgrade.

Independent compatible servers/fingerprints remain V0 evidence. Rust migration
parity does not promote this feature from Experimental.

## Phase A1 — protocol-neutral profile adapters

Status: implemented/merged in Python; **Rust migration scheduled as R2**.

The stable profile layer owns:

- strict extraction/parsing;
- redacted preview;
- non-secret endpoint extraction;
- stable subscription identity;
- diagnostic/error redaction;
- core generation behind adapter boundary.

The private store has explicit protocol discriminator/versioned migration while
preserving IDs, favorites, subscription membership, active selection and
autoconnect choices.

R2 ports this semantic boundary to Rust before P4 expands it. Do not perform a
separate speculative multi-core rewrite; migrate the accepted adapter contract.

## Phase P1 — experimental Trojan

Status: implemented/merged as Experimental.

Supported subset:

- `trojan://` manual/mixed-subscription import;
- TCP, WebSocket, gRPC;
- TLS/SNI/ALPN/uTLS;
- supported REALITY combinations;
- strict password redaction;
- existing Mihomo routing/favorites/probes/autoconnect behavior.

Unknown/ambiguous fields reject. Unsupported security semantics reject rather
than degrade.

Promotion needs independent server/provider evidence. R2 needs Rust parity for
accepted/rejected/redacted/generated behavior.

## Phase P2 — experimental Hysteria2

Status: implemented/merged as Experimental.

Supported subset:

- official `hysteria2://` / `hy2://` forms;
- bounded auth/userpass;
- port hopping;
- SNI/certificate verification/pinning;
- bounded ECH;
- supported `salamander`/`gecko` obfuscation.

Realm remains out of initial slice. Provider-carried local tuning is not trusted
as hidden share metadata.

Live validation compares UDP-friendly with safe UDP-restricted environment when
available. R2 migration preserves error/redaction/generation semantics without
claiming missing live interoperability evidence.

## Phase P3 — experimental TUIC v5

Status: implemented/merged as Experimental.

Supported subset:

- TUIC v5 UUID/password;
- TLS/SNI/ALPN;
- native/quic UDP relay;
- bounded supported congestion controls;
- conservative 0-RTT policy.

TUIC v4 token mode and 0-RTT remain deferred without representative fixtures
and explicit replay-risk decision.

R2 ports current v5 semantics to Rust before adding unrelated TUIC dialects.

## Phase P4 — experimental WireGuard and AmneziaWG import

State: **native-data/runtime development candidate after completed R2**.
This is not a stable release or completed matching-server acceptance; do not
advertise wider support than the actual accepted experimental scope.

P4 is the first new major credential-family expansion after the Rust adapter
boundary and is therefore Rust-first. The current QML plugin remains the user
surface; it consumes P4 through the accepted Rust migration/runtime bridge.

Do not invent a universal `wireguard://` share scheme.

### P4a — standard one-peer WireGuard `.conf`

Initial Rust adapter scope:

- exactly one bounded `[Interface]` + one `[Peer]`;
- IPv4/IPv6 addresses, endpoint, keys, keepalive, MTU/DNS and documented
  representable fields;
- reject duplicate sections/keys, extra peers and executable
  `PreUp`/`PostUp`/`PreDown`/`PostDown`, `Table`, `SaveConfig` directives;
- structured private profile storage;
- explicit Mihomo WireGuard generation;
- never treat imported `AllowedIPs` as arbitrary OS route command;
- private/preshared keys absent from public output/argv/environment.

Full VPN/ Routing/ Direct remain OmaVLESS policy choices rather than imported
host routing commands.

Required gates include real matching server, installed-core validation,
Full/Routing/Direct, reconnect/autoconnect/update/remove rollback, IPv4/IPv6
where available and credential-leak review.

The foundation accepts large but bounded provider-generated `AllowedIPs` lists
without treating them as host route commands. It keeps at most one IPv4 and one
IPv6 local address because Mihomo's one-peer outbound has one field per family.

### P4b — native AmneziaWG `.conf`

Reuse Rust P4a parser/storage boundaries. Accept only documented deterministic
AWG generations; mixed/incomplete generations fail.

Map only Mihomo `amnezia-wg-option` semantics supported by installed version.
Newer generations get precise minimum-core errors rather than field loss.

The 2026-09-03 source audit used Amnezia Client `dev` commit
`398697c101d27d9d4fb2f9a25ebe7749925f0d61` and Mihomo tag `v1.19.30`.
The initial Rust model distinguishes AWG 1, 2, 3 and 3.1 markers, rejects
incomplete/mixed generations and rejects the unvalidated 1.5-only `J1`/`J2`/
`J3`/`Itime` family. A private AWG3 config maps successfully through
`amnezia-wg-option` and passes installed-core config validation. Mihomo 1.19.30
exposes `persistent-keepalive` as one integer while AWG3 configs may carry a
range. The historical foundation renderer used the lower bound; that behavior
is superseded. Current managed execution refuses nonconstant ranges rather
than changing their meaning, while private storage/export preserves them.
Credential-safe local evidence is recorded in
[`docs/testing/BARE_METAL_P4_AWG3_FOUNDATION_2026-09-03.md`](docs/testing/BARE_METAL_P4_AWG3_FOUNDATION_2026-09-03.md).

Rare/legacy families remain rejected until real fixtures justify support.

### P4c — AmneziaVPN guest `vpn://`

Bounded local Rust decoding, with no provider/admin API access:

- decode Base64URL + Qt `qCompress` with declared-size, compressed-stream and
  decompressed-size bounds;
- pass a directly decoded native `.conf` through P4a/P4b unchanged;
- select exactly one supported explicit WG/AWG container matching
  `defaultContainer` from structured Amnezia JSON keys;
- recognize signed Amnezia API/subscription guest keys and reject them with a
  credential-safe compatibility result rather than contacting the embedded
  provider service;
- never call embedded API endpoint;
- never import SSH/root/server-admin credentials;
- never silently choose another protocol container.

Native WG/AWG config remains first interoperable target.

### Current development candidate: native data path and managed subset

The current development port carries the bounded structured guest adapter from
`08c80c3b3d91482e9d1a413e22cbb6ddc07b937a` and strict versioned private-record
codec from `b0af70a447e88288321b3de5cc184861bdb6a92b`. Exactly one supported
WG/AWG container must match `defaultContainer`; its embedded native config is
revalidated by the existing strict parser. Conflicting endpoint, credential,
address, DNS, MTU, keepalive and AWG sibling metadata reject. Original-byte
duplicate JSON keys reject before normalization. API/admin shapes do not cause
remote access. The explicit private record contains canonical credentials only,
revalidates on read and has redacted Debug/errors; it is not a share scheme.

The development candidate integrates these credentials into the existing Rust
`PrivateStore`, ordinary CLI/IPC owner and QML/TUI clients. Standalone `.conf`
and supported guest inputs use redacted Preview, confirmed import/replace,
details/editor reads and explicit private `.conf` export. WG/AWG QR export is
refused. Product input/canonical export is bounded to32 KiB and actual serialized
request/response limits; broader private parsing does not grant execution.

The first confirmed native insertion or replacement prepares one atomic
legacy-to-v4-plus-profile transaction under the existing lock, ownership,
revision and replay guards. URI-only stores retain their existing versions;
v4 is sticky. Mixed credentials, subscriptions, local bundles and closed Backup
v3/v4 handling use the shared graph, including cross-profile/subscription ID
rejection. Structured rows cannot hide URI credentials, provider ownership or
unknown extensions. An intentional empty Last selection remains empty. There
is no startup migration, parallel runtime owner or Python production fallback.

Execution is narrower than private-data support. The credential-derived
predicate runs before intent/quiesce and again at configuration/start
boundaries. The initial managed subset requires IPv4 interface, AllowedIPs and
endpoint transport, plus losslessly representable parameters for the pinned
core. See the [precise managed subset](docs/roadmap/CONTROL_PLANE.md#experimental-managed-wgawg-subset).
Variable keepalive, incompatible legacy headers/padding and unsupported junk
expressions refuse execution while remaining private data. Dual-stack parsing
and export are not IPv6 connection support.

These source capabilities are not installed/live acceptance. Exact-candidate
matching-server traffic, wrong-key/wrong-header controls, Full/Routing/Direct,
reconnect and owned cleanup remain required. Fresh-login/autoconnect,
update/remove/rollback, IPv6, other AWG generations, ARM installation and public
DNS/internet egress need their own evidence or explicit scope disposition.
Parser tests and `mihomo -t` cannot prove lazy UAPI initialization or traffic.
The old unbounded caller-selected opt-in host-core test is not an acceptance
substitute. Main/release support is unchanged.

The port also refuses unvalidated AWG 1.5-only `J1`/`J2`/`J3`/`Itime` sibling
settings in structured guests, consistently with native parsing; they cannot be
silently ignored alongside an otherwise valid WG/AWG native payload. Negative
controls cover case/order variants and both credential families. Historical
source acceptance and original failed checks remain separate from this port's
exact-head evidence in its owning PR.

The October19 upstream recheck read the official
[Amnezia importer at the pinned source](https://github.com/amnezia-vpn/amnezia-client/blob/398697c101d27d9d4fb2f9a25ebe7749925f0d61/client/core/controllers/selfhosted/importController.cpp),
the [Mihomo WG documentation](https://wiki.metacubex.one/en/config/proxies/wg/)
and [pinned v1.19.30 option implementation](https://github.com/MetaCubeX/mihomo/blob/v1.19.30/adapter/outbound/wireguard.go).
Official [v1.19.31](https://github.com/MetaCubeX/mihomo/releases/tag/v1.19.31)
and [v1.19.32](https://github.com/MetaCubeX/mihomo/releases/tag/v1.19.32) notes
were checked; v1.19.31 includes AWG v3 and WG initialization corrections. This
does not select a new core, lower a minimum version or qualify current binaries.
No published advisory was listed on the official advisory page at that check;
this is not a claim that no relevant bug exists. In particular the open
[WG reload report #3255](https://github.com/MetaCubeX/mihomo/issues/3255) describes
old devices surviving `PUT /configs`, including an attributed Linux/AWG
observation. It is not an OmaVLESS reproduction or a proven fix. Before product
activation, qualify the actual selected core and lifecycle replacement/cleanup,
rather than inferring that config reload closes the previous WG device.

## Later Mihomo-native candidates

AnyTLS, Shadowsocks, VMess are demand/fixture-driven only. Supporting every
Mihomo outbound is not a product goal.

New large adapters after R2 are Rust-first unless an explicit roadmap exception
explains why a temporary Python implementation is unavoidable and how it will
be retired.

## Future X0/X1 — optional Xray Full VPN backend

State: deferred until Rust canonical runtime plus real compatibility need.

Do not use hidden Mihomo TUN + loopback SOCKS + Xray chain as default design.
If justified, Xray is a real optional `CoreBackend` with its own validated Full
VPN capture path and centralized Rust runtime ownership.

Start only when:

- adapter behavior is mature/Rust-owned;
- incompatibility is confirmed in Mihomo rather than importer;
- real profiles justify another core;
- exact Xray semantics have strict parsing/redaction/config tests;
- lifecycle/rollback/conflict contract is explicit;
- real Xray TUN behavior is validated.

Initial capability contract:

```text
Mihomo:
  Full VPN  yes
  Routing   yes
  Direct    yes

Xray initial:
  Full VPN  yes
  Routing   no
  Direct    no
```

Core selection is initially automatic/capability-driven. One OmaVLESS-owned
full-tunnel core at a time. Foreign VPN/TUN remains detect/refuse, not kill.

If K1 exists, Xray integrates with the same core-independent leak-protection
boundary.

## Future backend-neutral Xray routing research

State: deferred, not implied by X1.

Consider only after preserving rather than approximating:

- RU/CN/IR policy semantics;
- custom rules;
- DNS;
- rule-data lifecycle;
- route inspection;
- IPv4/IPv6 leak safety;
- reconnect/failure behavior.

Until then Routing is unavailable for Xray-backed profiles.

## Marketplace/security preflight

Before marketplace/package update audit exact submitted commit:

- credentials never through argv/shell/environment;
- private paths reject unsafe ownership/symlinks and use restrictive modes;
- no user-writable config consumed by unrestricted root service;
- no silent passwordless sudo/Polkit;
- controller/remote APIs bounded/private;
- untrusted strings render as plain text;
- disable/remove stops owned networking and cleans generated integration;
- privileged changes get adversarial crash/removal tests;
- Rust migration does not introduce secret-bearing differential logs or a second
  runtime owner.

Future fail-closed helper remains optional/bounded and never interprets arbitrary
user firewall config as root policy.

## Pull-request discipline

- One protocol/migration slice per PR.
- Every runtime PR names dependencies and exact host gate.
- Existing-adapter Rust ports name Python reference + parity corpus.
- New adapters after R2 are Rust-first.
- Cloud/parity and host interoperability evidence remain separate.
- Parser changes include malformed/adversarial fixtures.
- New secrets trigger preview/status/diagnostics/log/argv/export review.
- Version-sensitive generation is tested against relevant core versions.
- A protocol does not become Stable in the same PR that first implements it.
- Rust parity does not imply provider interoperability maturity.
- Xray is not a shortcut around unfinished importer work.
- Follow `docs/roadmap/DEVELOPMENT_WORKFLOW.md` and `AGENTS.md`.
