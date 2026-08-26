# OmaVLESS protocol roadmap

Status: 0.7.0 implementation baseline and forward compatibility roadmap,
updated 2026-08-26.

This document specifies protocol/share-format support and compatibility
boundaries. Delivery order and real-host gates live in
[`DEVELOPMENT_ROADMAP.md`](DEVELOPMENT_ROADMAP.md). Broader core/lifecycle and
Git workflow rationale lives under [`docs/roadmap/`](docs/roadmap/).

`Implemented` means the parser, redaction boundary, current core mapping and
automated tests are in `main`. `Experimental` means implementation exists but
broader independent server/provider interoperability evidence is still being
collected.

## Product direction

Mihomo remains the primary and preferred core. Current production Full VPN,
Routing and Direct modes, country policies, rule providers, private controller,
route inspection and traffic diagnostics are Mihomo-native and remain the
reference behavior.

Protocol support is added through strict profile adapters above the runtime
core. A profile is never accepted merely because an upstream core can parse a
similar option. OmaVLESS must understand, bound, redact and validate every
share-controlled field it stores or emits.

A future Xray backend is a narrow compatibility mechanism for common profile
semantics that supported Mihomo cannot represent without loss. It is not a
replacement for Mihomo and not initially a user-selectable preference.

The first production Xray slice, if justified, is **Full VPN only**. Routing
and Direct remain unavailable until later work can preserve the observable
semantics and leak-safety guarantees of the Mihomo implementation.

Detailed architecture: [`docs/roadmap/ARCHITECTURE.md`](docs/roadmap/ARCHITECTURE.md).

## Protocol status at a glance

| Family | Status | Next gate |
| --- | --- | --- |
| VLESS | Implemented | Continue Mihomo/Xray compatibility review and real XHTTP/PQ evidence |
| Trojan | Experimental implementation | Independent server/provider validation |
| Hysteria2 | Experimental implementation | UDP-friendly and UDP-restricted validation |
| TUIC v5 | Experimental implementation | Independent server/provider validation |
| Standard WireGuard `.conf` | Planned P4a | Structured private storage + one-peer parser |
| Native AmneziaWG `.conf` | Planned P4b | P4a + generation-aware field validation |
| AmneziaVPN guest `vpn://` | Planned P4c | Bounded offline extraction through P4a/P4b validators |

Mihomo supporting an outbound does not mean OmaVLESS already supports the
provider key/configuration format. The manifest and marketplace description
must list only actually implemented product support.

## Universal security gates

Every protocol or key-format slice must preserve these invariants:

1. bounded strict parsing of untrusted input;
2. redacted preview before storage;
3. no reusable credential in public status, diagnostics, logs, argv or process
   environment;
4. private files created with final restrictive permissions before credential
   bytes are written;
5. exact generated configuration validation with the installed core;
6. explicit rejection of unknown/ambiguous security-sensitive fields instead
   of arbitrary pass-through;
7. optional end-to-end connection testing without leaking the imported
   credential into public artifacts;
8. real Omarchy/TUN/server validation before the implementation slice merges
   when runtime behavior depends on the host/network.

Experimental status does not relax these gates.

## Upstream source/release gate

Every protocol/key-format PR must repeat an upstream audit rather than relying
on an older roadmap conclusion.

For Mihomo-native support:

1. review official Mihomo proxy documentation;
2. inspect the corresponding option structures/parser at a pinned tag/commit;
3. review the latest stable release and relevant releases since the oldest
   supported core;
4. compare the original protocol/share-format specification;
5. classify each candidate field as share-controlled, local-only,
   unsupported by the installed version, or unsupported by Mihomo;
6. add explicit version gating where the emitted semantics depend on core
   version;
7. validate representative generated configs against the current installed
   core on the Omarchy machine before merge.

A relevant upstream security advisory blocks merge until minimum-version and
upgrade guidance are reconciled.

For a future Xray-specific slice, perform the equivalent audit against official
Xray documentation/source/releases for the exact semantics that require Xray.
Third-party clients may document dialects but are not authoritative for another
core's runtime schema.

## Compatibility boundary

There are two distinct classes of gap.

### OmaVLESS importer gap

The selected core supports a feature, but OmaVLESS does not yet safely parse,
validate or represent the corresponding share input. This is an OmaVLESS task;
it does not justify changing cores.

### Core gap

Supported Mihomo and Xray intentionally or structurally differ. OmaVLESS may
explain/detect the difference, but must not claim that translation makes them
identical. A future Xray backend is reserved for **confirmed, common core-level
gaps** after Mihomo-native support is mature.

Until an Xray path is explicitly implemented and live-tested, a profile which
requires unsupported Xray-only semantics receives a precise compatibility
error rather than a hidden fallback.

## 0.7.0 implementation baseline

The initial protocol sequence through PR #16 is merged. It established:

- VLESS/REALITY/XHTTP handling;
- country routing and Settings;
- login autoconnect and custom routing tools;
- favorites, redacted import/diagnostics and safe export boundaries;
- a protocol-neutral adapter/store discriminator;
- experimental Trojan, Hysteria2 and TUIC v5 import;
- marketplace security preflight.

Later lifecycle/removal hardening is in the merged history/CHANGELOG. The active
D1 -> V0 -> P4 delivery order is maintained in `DEVELOPMENT_ROADMAP.md`.

## Phase V1 — strict VLESS semantics

Status: implemented/merged.

Scope includes:

- bounded `packetEncoding` (`xudp`, `packetaddr` or absent);
- Mihomo-supported XHTTP modes;
- explicit distinction between Xray Vision share variants while documenting
  Mihomo's actual flow behavior;
- explicit handling of Xray-only `spx`/spider-path compatibility rather than
  inventing a Mihomo option;
- rejection of Xray-only ML-DSA verification metadata which current Mihomo
  cannot represent safely;
- credential-free errors and compatibility notes.

The product must not infer that a profile failing against a newer Xray server
is malformed when the failure may be a core compatibility difference.

## Phase V2 — bounded XHTTP `extra`

Status: implemented/merged; broader real-provider evidence remains required.

Scope:

- bounded URL-encoded JSON object;
- whitelist-only headers/padding/session/sequence/upload/reuse/XMUX/download
  structures which current Mihomo can represent;
- explicit camelCase -> Mihomo mapping;
- rejection of unknown or unsafe nested structures;
- explicit limitations around split upload/download settings and inherited
  transport/security semantics.

Real Omarchy evidence must include representative TLS and REALITY profiles and
implemented XHTTP modes.

## Phase V3 — experimental VLESS Encryption and REALITY PQ

Status: implemented/merged as Experimental.

Scope:

- bounded Mihomo-compatible VLESS Encryption material treated as opaque secret;
- supported post-quantum REALITY metadata;
- strict redaction of encryption/PQ material;
- explicit experimental/fingerprint warning;
- no silent downgrade to an older core or weaker semantics.

Independent compatible servers/fingerprints remain part of V0 evidence.

## Phase A1 — protocol-neutral profile adapters

Status: implemented/merged.

The stable profile layer owns:

- strict extraction/parsing;
- redacted preview;
- non-secret endpoint extraction;
- stable subscription identity;
- diagnostic/error redaction;
- current core-specific generation behind an adapter boundary.

The private store has an explicit protocol discriminator and versioned
migration while preserving profile IDs, favorites, subscription membership,
active selection and autoconnect choices.

The current implementation still generates Mihomo entries. The eventual core
contract should be extracted only when a concrete second-core implementation
requires it; do not perform a speculative rewrite solely for architectural
symmetry.

## Phase P1 — experimental Trojan

Status: implemented/merged as Experimental.

Initial supported subset:

- `trojan://` manual and mixed-subscription import;
- TCP, WebSocket and gRPC;
- TLS/SNI/ALPN/uTLS fields;
- supported REALITY combinations;
- strict password redaction;
- existing Mihomo routing/favorites/probes/autoconnect behavior.

Unknown/ambiguous query fields are rejected. Combinations which current Mihomo
cannot implement with the required security semantics are rejected rather than
silently degraded.

Promotion from Experimental requires multiple independent servers plus provider
subscription evidence.

## Phase P2 — experimental Hysteria2

Status: implemented/merged as Experimental.

Initial supported subset:

- official `hysteria2://` / `hy2://` forms;
- bounded auth/userpass;
- port hopping;
- SNI/certificate verification/pinning;
- bounded ECH;
- supported `salamander`/`gecko` obfuscation.

Realm mode remains outside the initial slice. Provider-carried bandwidth/local
connection tuning is not trusted as hidden share metadata.

Live validation must explicitly compare a UDP-friendly network with a
UDP-restricted network and keep server failure distinguishable from likely UDP
blocking.

## Phase P3 — experimental TUIC v5

Status: implemented/merged as Experimental.

Initial supported subset:

- TUIC v5 UUID/password dialect;
- TLS/SNI/ALPN;
- native/quic UDP relay;
- bounded supported congestion controller values;
- conservative 0-RTT policy.

TUIC v4 token mode and 0-RTT remain deferred without representative fixtures
and an explicit replay-risk decision.

## Phase P4 — experimental WireGuard and AmneziaWG import

State: planned after V0 evidence. P4 adds new credential formats; current
0.7.0 runtime does not import them.

The phase is split by actual input format rather than inventing a universal
`wireguard://` scheme.

### P4a — standard one-peer WireGuard `.conf`

Initial scope:

- exactly one bounded `[Interface]` plus one `[Peer]`;
- IPv4/IPv6 addresses, endpoint, keys, keepalive, MTU/DNS and documented
  representable fields;
- reject duplicate sections/keys, extra peers and executable
  `PreUp`/`PostUp`/`PreDown`/`PostDown`, `Table` or `SaveConfig` directives;
- translate to Mihomo WireGuard using structured private profile storage;
- never treat imported `AllowedIPs` as an arbitrary OS route command;
- private/preshared keys absent from public output/argv/environment.

Full VPN, Routing and Direct remain OmaVLESS/Mihomo policy choices rather than
blindly importing host routing directives from the file.

### P4b — native AmneziaWG `.conf`

Reuse the P4a parser/storage boundary and accept only documented, deterministic
AWG field generations. Mixed/incomplete generations fail validation.

Map only the Mihomo `amnezia-wg-option` semantics explicitly supported by the
installed version. Newer AWG generations require a precise minimum-core error
rather than silent field loss.

Legacy/rare field families remain rejected until representative real fixtures
exist.

### P4c — AmneziaVPN guest `vpn://`

Later bounded offline adapter only.

- decode size/depth/count-bounded guest connection data;
- select one explicit WireGuard/AmneziaWG container;
- feed that native configuration through the exact P4a/P4b validators;
- never call an embedded API endpoint;
- never import SSH/root/server-administration credentials;
- never silently choose a different protocol container.

Native WG/AWG config remains the interoperable first target.

## Later Mihomo-native candidates

AnyTLS, Shadowsocks and VMess are demand/fixture-driven candidates only.
Supporting every Mihomo outbound is not a product goal by itself.

## Future X0/X1 — optional Xray Full VPN backend

State: deferred, not in the active P4 chain.

The old design in which Mihomo kept TUN/routing while a hidden Xray process sat
behind a loopback SOCKS hop is no longer the preferred architecture. If Xray
support becomes justified, OmaVLESS should treat it as a real optional core
backend with its own validated Full VPN capture path and centralized
OmaVLESS-owned lifecycle.

Current upstream Xray-core includes a Linux TUN inbound and automatic route /
outbound-interface controls, so a self-contained Xray Full VPN path is
technically plausible. This remains an upstream capability until OmaVLESS
implements and validates it on the real Omarchy host.

Start X0/X1 only when all are true:

- VLESS/protocol adapter behavior is mature;
- the incompatibility is confirmed to be in Mihomo rather than OmaVLESS;
- affected real-world profiles are common enough to justify another core;
- exact Xray semantics are covered by strict parsing/redaction/config tests;
- lifecycle ownership, rollback and conflict behavior have an explicit design;
- real Xray TUN behavior is tested on Omarchy.

Initial capability contract:

```text
Mihomo:
  Full VPN  yes
  Routing   yes
  Direct    yes

Xray (initial):
  Full VPN  yes
  Routing   no
  Direct    no
```

Core selection is initially automatic and capability-driven. Mihomo is chosen
whenever it can represent the validated profile without loss. Xray is selected
only for an explicitly supported Xray-only semantic. The UI may explain the
selected core but should not force ordinary users to choose between cores.

Only one OmaVLESS-owned full-tunnel core may own the active capture path at a
time. Foreign VPNs/TUNs remain detection/refusal territory, not automatic
termination territory.

If fail-closed K1 exists by the time X1 lands, Xray must integrate with the same
core-independent leak-protection boundary rather than introducing a second
backend-specific kill switch.

## Future R1 — backend-neutral routing research

State: deferred and explicitly **not** implied by X1.

Xray Routing may be considered only after the project can preserve rather than
approximate:

- current Russia/China/Iran policy semantics;
- custom rules;
- DNS behavior;
- rule-data lifecycle/update semantics;
- route inspection behavior;
- IPv4/IPv6 leak safety;
- reconnect/failure behavior.

Until that is proven, the Routing control is unavailable for Xray-backed
profiles.

## Marketplace/security preflight

Before any marketplace update, audit the exact submitted commit:

- credentials never travel through argv/shell text/environment;
- private files/directories reject unsafe ownership/symlinks and are created
  with final restrictive modes;
- no user-writable configuration is consumed by an unrestricted root service;
- no passwordless sudo/Polkit policy is silently installed;
- controller/remote APIs remain bounded and private where intended;
- untrusted strings render as plain text;
- disable/remove paths stop owned networking and remove generated runtime
  integration while documenting intentionally retained private state;
- privileged/networking changes receive dedicated adversarial tests and real
  removal/crash validation.

A future fail-closed helper is not exempt from this model: it must be optional,
narrowly scoped and must not interpret arbitrary user-controlled firewall
configuration as privileged policy.

## Pull-request discipline

- One roadmap slice per PR.
- Every runtime PR names its predecessor/dependencies and exact local gate.
- Cloud evidence and Omarchy-only evidence stay separate.
- Parser changes include malformed/adversarial fixtures.
- New secrets trigger preview/status/diagnostics/log/argv/export review.
- Version-sensitive core generation is tested against relevant minimum/current
  versions.
- A protocol does not become Stable in the same PR that first implements it.
- Xray must not be used as a shortcut around an unfinished Mihomo importer.
- Branch maturity does not encode product maturity; follow
  `docs/roadmap/DEVELOPMENT_WORKFLOW.md`.
