# OmaVLESS protocol roadmap

Status: 0.7.0 implementation baseline and forward roadmap, updated 2026-08-25.

This document records the protocol work included in the OmaVLESS 0.7.0
release candidate and the
remaining order for extending it as a Mihomo-native, multi-protocol client
without expanding the compact bar widget or weakening credential handling.
The current cross-feature PR order and Omarchy handoff gates live in
[`DEVELOPMENT_ROADMAP.md`](DEVELOPMENT_ROADMAP.md); this file remains the
protocol compatibility specification.
`Implemented` below means that the parser, redaction boundary, Mihomo mapping
and automated tests are in `main`; experimental protocols still need broader
independent server/provider interoperability before their label can be removed.

## Protocol status at a glance

| Family | OmaVLESS 0.7.0 status | Next gate |
| --- | --- | --- |
| VLESS | Implemented | Continue Mihomo/Xray compatibility review |
| Trojan, Hysteria2, TUIC v5 | Experimental implementation | Independent live server/provider validation |
| Standard WireGuard `.conf` | Planned P4a; no runtime importer | Structured private profile storage and one-peer parser |
| Native AmneziaWG `.conf` | Planned P4b; no runtime importer | P4a plus versioned v1-v3.1 field validation |
| AmneziaVPN guest `vpn://` | Planned P4c; no runtime importer | Bounded offline container decoding after P4b |

Mihomo supporting an outbound does not mean OmaVLESS already supports its key
or configuration format. Merged [PR
#15](https://github.com/k-kostin/omavless/pull/15) began the WireGuard and
AmneziaWG work at the design level only; it deliberately changed no runtime
code. The manifest and marketplace description must continue to list only
implemented protocols until the relevant P4 slice is merged and live-tested.

## Product direction

OmaVLESS keeps Mihomo as its primary core. Mihomo owns TUN, DNS, routing modes,
rule providers, the private controller and traffic accounting. Protocol support
is added through narrowly scoped import adapters which generate validated
Mihomo YAML.

The main panel remains protocol-neutral: choose a server, then use Full VPN,
Routing or Direct. Protocol names and experimental warnings belong in import
review, profile details and Settings, not in permanent top-level controls.

An experimental protocol must still pass the same security gates as VLESS:

1. bounded and strict parsing of untrusted input;
2. a redacted preview before storage;
3. no reusable credential in public status, diagnostics, argv or logs;
4. generated configuration validation with the installed Mihomo core;
5. an optional end-to-end probe before the user relies on the profile.

Experimental means that server and ecosystem compatibility is still being
measured. It never means accepting arbitrary YAML or skipping validation.

## 0.7.0 implementation baseline

The original stacked sequence is complete: PRs
[#1](https://github.com/k-kostin/omavless/pull/1) through
[#16](https://github.com/k-kostin/omavless/pull/16) are merged into `main`.
Together they delivered country routing and Settings, guided setup and login
autoconnect, custom routing tools, favorites, redacted import/diagnostics,
strict VLESS/REALITY/XHTTP handling, the protocol-neutral adapter boundary,
experimental Trojan/Hysteria2/TUIC imports, and the marketplace security
preflight. Later 0.7.0 hardening and UI fixes are documented in
`CHANGELOG.md` and the merged pull-request history.

The active delivery chain is tracked in `DEVELOPMENT_ROADMAP.md`. The
marketplace lifecycle/removal gate (S0) is merged; the next steps reconcile
documentation and diagnostics (D0/D1), then validate existing experimental
protocols (V0) before P4 starts below. Before implementing another protocol,
keep the existing experimental adapters honest: validate representative
generated configurations
against an installed current Mihomo core and collect independent real
server/provider results. Automated coverage is necessary but does not by
itself promote an experimental protocol to stable.

## Mihomo source and release gate

Every protocol or key-format PR must repeat this audit; knowledge copied from
an older OmaVLESS PR is not sufficient:

1. start with Mihomo's official proxy documentation for the emitted YAML;
2. inspect the corresponding option structures and parser at a pinned Mihomo
   tag or commit, because documentation can lag implementation;
3. read the latest stable release notes and every release since OmaVLESS's
   oldest supported Mihomo version for protocol changes, compatibility fixes
   and security advisories;
4. compare the original protocol/share-format specification; an independent
   importer such as v2rayN or dae may document an input dialect, but is never
   the authority for Mihomo's output schema;
5. classify every candidate field as share-controlled, local-only, unsupported
   by the installed core, or unsupported by Mihomo at all;
6. record the installed `mihomo -v` output and validate representative emitted
   configs with both the oldest supported and latest stable core whenever a
   feature is version-sensitive.

The PR body must link the official Mihomo documentation, a commit-pinned source
permalink, the latest release tag and date reviewed, and the original input
format specification. If the latest release changes the conclusion, add an
explicit compatibility test and user-facing version error. Never silently
downgrade or discard a field to make an older core accept a profile.

This audit is repeated on the Omarchy machine immediately before merge. The
reviewing agent must fetch the current release list rather than trust the date
in this roadmap, run the cloud test suite with its installed core, and perform
the PR's real TUN/server checks. A new upstream option is not automatically a
product feature: unknown, unsafe and provider-controlled local tuning remains
rejected until its user value and threat boundary are understood. A relevant
Mihomo security advisory blocks merge until the minimum supported version and
upgrade guidance are updated.

## Compatibility boundary

There are two different classes of compatibility gap.

### OmaVLESS gaps

Mihomo supports the feature, but the current importer does not yet parse,
validate or translate every share-link parameter. These gaps are in scope for
the VLESS phases below.

### Core gaps

Mihomo and Xray intentionally or structurally behave differently. OmaVLESS can
explain, detect or sometimes emulate the difference, but it must not claim that
translation makes the cores identical. The current REALITY compatibility
boundary with newer Xray servers belongs here. A future Xray transport backend
is reserved for this class of problem after Mihomo-native support is mature.

## Phase V1 — strict VLESS semantics

Status: implemented and merged in 0.7.0 through
[#6](https://github.com/k-kostin/omavless/pull/6) and
[#7](https://github.com/k-kostin/omavless/pull/7).

Goal: make the existing supported subset explicit and deterministic before
adding more fields.

Scope:

- validate `packetEncoding` as empty, `xudp` or `packetaddr`;
- validate XHTTP mode as empty, `auto`, `stream-one`, `stream-up` or
  `packet-up`;
- distinguish Xray `xtls-rprx-vision` from
  `xtls-rprx-vision-udp443` at import time while emitting the Mihomo-supported
  flow value;
- document and test the UDP/443 semantic difference instead of silently
  implying exact Xray behavior;
- treat Xray's `spx`/`spider-x` as an explained compatibility boundary rather
  than generating an option absent from current Mihomo, and reject Xray-only
  ML-DSA verification metadata explicitly;
- keep validation errors credential-free and visible before connection.

Cloud acceptance:

- parser and YAML unit tests cover every accepted value and nearby rejection;
- preview and diagnostics fixtures prove that new metadata cannot expose a
  complete credential;
- generated representative configs pass the opt-in current-Mihomo test.

Omarchy acceptance:

- connect one plain VLESS/TLS profile;
- connect one TCP + REALITY + Vision profile;
- exercise UDP/443/QUIC and record the observed behavior;
- connect one `packetEncoding=xudp` profile and verify UDP traffic;
- confirm old stored profiles migrate and reconnect unchanged.

## Phase V2 — bounded XHTTP `extra`

Status: implemented and merged in 0.7.0 through
[#8](https://github.com/k-kostin/omavless/pull/8) and
[#9](https://github.com/k-kostin/omavless/pull/9). Real-provider compatibility
remains narrower than the automated schema coverage.

Goal: translate the useful advanced XHTTP fields supported by Mihomo without
turning a share link into arbitrary configuration input.

Scope:

- accept a size- and depth-bounded URL-encoded JSON `extra` object;
- translate only a documented whitelist of headers, padding, session/sequence,
  upload, reuse/XMUX and download settings;
- map Xray-style camelCase names to Mihomo fields explicitly;
- reject unknown security-sensitive structures instead of copying them;
- preserve the existing simple `path`, `host`, `mode` and `alpn` path.

Implement and live-test the flat client/XMUX subset before adding
`downloadSettings`; upload/download separation is a distinct compatibility
slice because it introduces a second endpoint and a nested TLS/Reality policy.
The download slice must use Mihomo's inherited transport mode explicitly:
OmaVLESS rejects a different nested Xray mode because current Mihomo has no
separate download-mode field.

Omarchy acceptance requires real keys for each implemented XHTTP mode. At least
one test must combine XHTTP with TLS and one with a compatible REALITY server.

## Phase V3 — experimental VLESS Encryption and REALITY PQ metadata

Status: implemented and merged as an experimental 0.7.0 capability through
[#10](https://github.com/k-kostin/omavless/pull/10). It remains experimental
until independent compatible servers and fingerprints are verified.

Goal: expose Mihomo capabilities which the current importer intentionally
rejects, with an explicit experimental marker.

Scope:

- accept a bounded Mihomo-compatible VLESS Encryption value as opaque secret
  material rather than reimplementing its cryptography;
- add `support-x25519mlkem768` when represented by the imported profile;
- redact encryption and post-quantum key material everywhere;
- show the experimental capability in preview without showing the secret;
- retain the existing `encryption=none` path unchanged.

Cloud parsing validates Mihomo's current `mlkem768x25519plus` client grammar,
including mode, RTT, padding ranges and canonical 32/1184-byte public key
tokens. The GUI receives only fixed experimental feature names. REALITY PQ
also carries a fingerprint-dependent warning because the Mihomo option cannot
force a uTLS fingerprint to advertise a group it does not implement.

The parser must not infer that a failed new-Xray REALITY server is a malformed
key. Where the failure is ambiguous, the UI reports a compatibility hint rather
than a false diagnosis.

## Phase A1 — protocol-neutral profile adapters

Status: implemented and merged in 0.7.0 through
[#11](https://github.com/k-kostin/omavless/pull/11). The adapter boundary is
covered by the shared parser, preview, YAML, migration and identity tests.

Goal: remove VLESS-only assumptions before exposing a second protocol.

Introduce an internal adapter contract with protocol-specific implementations
for:

- extracting and strictly parsing a profile;
- producing a redacted public preview;
- generating one Mihomo proxy entry;
- returning a non-secret endpoint for probes;
- calculating stable subscription identity;
- redacting diagnostics and errors.

The private store may continue to hold the original URI, but it gains an
explicit protocol discriminator and a versioned migration. Existing VLESS IDs,
favorites, subscription membership, active selection and autoconnect choices
must survive the migration.

The implemented v3 migration is in-memory on read and becomes durable only on
the next user mutation. A v3 profile must declare a supported protocol matching
its link. Public status exposes only that discriminator and the existing
non-secret endpoint; it never exposes the URI.

This phase is deliberately user-visible only as clearer neutral wording such as
“profile” or “connection”. It must not add a partially implemented protocol.

## Phase P1 — experimental Trojan

Status: implemented and merged as experimental in 0.7.0 through
[#12](https://github.com/k-kostin/omavless/pull/12); independent
server/provider interoperability remains pending.

First additional protocol because its URI and Mihomo mapping are closest to the
existing VLESS path.

Initial scope:

- `trojan://` manual import and mixed-protocol subscription import;
- TCP, WebSocket and gRPC transports;
- TLS, SNI, ALPN, uTLS fingerprint and Mihomo-supported REALITY fields;
- password-safe preview, export, QR and diagnostics;
- existing routing, favorites, probes and autoconnect without special cases.

The implemented importer accepts the Xray-style fields which map directly to
current Mihomo: password, endpoint, TCP/WebSocket/gRPC, SNI, ALPN, certificate
verification, uTLS fingerprint, WebSocket host/path, gRPC service name and
REALITY public key/short ID/PQ flag. Unknown and ambiguous query fields are
rejected instead of being silently ignored. Current Mihomo's Trojan WebSocket
path does not apply its REALITY config, so that combination is rejected; TCP
and gRPC remain eligible for REALITY. The redacted preview contains no password
hint or complete URI.

Schema references: [Mihomo Trojan documentation](https://wiki.metacubex.one/en/config/proxies/trojan/),
[shared TLS/REALITY options](https://wiki.metacubex.one/en/config/proxies/tls/)
and the current [`TrojanOption` implementation](https://github.com/MetaCubeX/mihomo/blob/Meta/adapter/outbound/trojan.go).

Trojan can leave experimental status after successful tests against multiple
independent servers and one subscription provider.

## Phase P2 — experimental Hysteria2

Status: implemented and merged as experimental in 0.7.0 through
[#13](https://github.com/k-kostin/omavless/pull/13); independent testing on
both UDP-friendly and UDP-restricted networks remains pending.

Initial scope:

- official `hysteria2://` and `hy2://` URI forms;
- authentication, multi-port/port hopping, SNI, certificate pinning and
  `salamander`/`gecko` obfuscation;
- no Realm mode in the first slice;
- bandwidth values remain local client settings and are not trusted from a
  shared URI.

The implemented importer accepts the official `hysteria2://` and `hy2://`
forms, including percent-encoded auth/userpass, a default port of 443,
bounded individual/range port lists, `salamander` or `gecko`, SNI, strict
certificate verification preference, SHA-256 certificate pinning and a
bounded base64 ECH config. It maps only those values to current Mihomo fields.
Unknown and duplicate fields, overlapping/invalid ranges and URI-carried
bandwidth are rejected before storage. Authentication, obfuscation passwords,
certificate pins and ECH bytes are absent from public preview and diagnostics.

Realm links remain outside this slice because their rendezvous token, Hysteria
auth, Realm ID, STUN list and local port form a different connection model.
Likewise, hop interval and bandwidth remain local settings rather than hidden
provider-controlled query extensions.

Schema references: the official [Hysteria 2 URI
scheme](https://v2.hysteria.network/docs/developers/URI-Scheme/), official
[port-hopping format](https://v2.hysteria.network/docs/advanced/Port-Hopping/),
[Mihomo Hysteria2 documentation](https://wiki.metacubex.one/en/config/proxies/hysteria2/)
and the current [`Hysteria2Option`
implementation](https://github.com/MetaCubeX/mihomo/blob/Meta/adapter/outbound/hysteria2.go).

Omarchy testing must cover networks where UDP/QUIC works and where it is
restricted, with errors that distinguish server failure from blocked UDP.

Mihomo 1.19.30 also adds an optional integer `handshake-timeout` in seconds.
It is a local connection-tuning value, not part of the official Hysteria2 URI,
so OmaVLESS must not accept it from an untrusted share link. Keep Mihomo's zero
default in the initial adapter. A later advanced Settings control may expose a
bounded local value only if real UDP-restricted-network tests show that it
improves the error experience rather than merely making failures slower.

## Phase P3 — experimental TUIC v5

Status: implemented and merged as experimental in 0.7.0 through
[#14](https://github.com/k-kostin/omavless/pull/14); independent
server/provider interoperability remains pending.

Initial scope:

- TUIC v5 UUID/password profiles only;
- TLS/SNI, ALPN, UDP relay mode and bounded connection options;
- conservative defaults for 0-RTT and congestion control;
- no TUIC v4 token mode until a real compatibility fixture is available.

The implemented importer uses the interoperable `tuic://UUID:password@host`
dialect documented by dae and emitted by current v2rayN. It accepts SNI, ALPN,
strict certificate verification, explicit SNI disabling, `native`/`quic` UDP
relay and Mihomo's `cubic`/`new_reno`/`bbr` congestion controllers. Missing
relay/controller fields become explicit `native` and `cubic` values. QUIC
0-RTT (`reduce-rtt`) is never inferred from a share link and remains off.

Unknown and duplicate parameters, TUIC v4 token-only links, invalid UUIDs and
conflicting SNI settings fail before storage. UUID and password are absent from
preview, public status and diagnostics. Because current Mihomo implements
`disable-sni` by also disabling certificate verification, preview reports that
security consequence instead of presenting it as a harmless hostname option.

Schema references: the TUIC project's [v5 protocol
specification](https://github.com/tuic-protocol/tuic/blob/master/SPEC.md), dae's
[TUIC URI scheme](https://github.com/daeuniverse/dae/discussions/182), current
[v2rayN parser/exporter](https://github.com/2dust/v2rayN/blob/master/v2rayN/ServiceLib/Handler/Fmt/TuicFmt.cs),
[Mihomo TUIC documentation](https://wiki.metacubex.one/en/config/proxies/tuic/)
and the current [`TuicOption`
implementation](https://github.com/MetaCubeX/mihomo/blob/Meta/adapter/outbound/tuic.go).

## Phase P4 — experimental WireGuard and AmneziaWG import

Status: planned after the 0.7.0 release and broader live validation of the
existing experimental adapters. The standard WireGuard subset can work with
older Mihomo releases, but the target baseline for this phase is Mihomo 1.19.30
or newer: that release adds the WireGuard
`ip-stack` block, AmneziaWG v3.0/v3.1 and the current MIPS congestion controls.
OmaVLESS must read and compare the installed core version before accepting a
profile which requires those fields.

No P4 runtime code is present in 0.7.0. The merged roadmap PR #15 established
the security and compatibility design, not an importer.

This phase is split by input format instead of pretending that WireGuard has a
universal share URI.

### P4a — standard one-peer WireGuard `.conf`

Initial scope:

- explicit import of one bounded `[Interface]` plus one `[Peer]` from a
  standard `.conf` file or pasted configuration text;
- IPv4/IPv6 interface addresses, endpoint, public/private/preshared keys,
  keepalive, MTU, DNS and optional reserved bytes;
- strict rejection of duplicate sections/keys, extra peers and executable
  `PreUp`, `PostUp`, `PreDown`, `PostDown`, `Table` or `SaveConfig` directives;
- translation into Mihomo's documented `wireguard` outbound, using the full
  `peers` form internally so a later multi-peer slice has no second schema;
- no invented public `wireguard://` standard and no multi-peer editor in the
  first implementation.

Imported `AllowedIPs` describes the original tunnel intent, but it must not
become a system routing command. The Mihomo outbound receives all-address peer
coverage needed by the imported address families, while OmaVLESS Full VPN,
Routing and Direct policies continue to decide which traffic selects that
outbound. Import review must explain this translation.

WireGuard does not fit the current URI-only private store. P4a therefore needs
a versioned private profile payload for canonical configuration fields rather
than encoding a private key into an invented URI. The source path is never
stored. Private and preshared keys are absent from preview, public status,
diagnostics, errors and argv; explicit config/QR export must warn that the
result itself is a reusable private credential.

Schema reference: [Mihomo WireGuard documentation and standard-config
translation](https://wiki.metacubex.one/en/config/proxies/wg/#translating-from-a-standard-wireguard-configuration-file).

### P4b — native AmneziaWG `.conf`

The same parser can recognize Amnezia's additional `[Interface]` keys and
classify the profile as AmneziaWG without a second top-level protocol engine.
Map only the documented Mihomo `amnezia-wg-option` fields:

- v1.0: `Jc`, `Jmin`, `Jmax`, `S1`, `S2` and `H1`–`H4`;
- v1.5/v2: optional `S3`, `S4`, `I1`–`I5` and the version-appropriate header
  value/range rules; legacy v1.5-only `J1`–`J3`/`ITime` require a real fixture
  before they are accepted;
- v3: `HeaderProtectionKey`, `ContentPaddingAddition`, `RekeyAfterTime`,
  `RekeyTimeout`, `RejectAfterTime`, `KeepaliveTimeout` and
  `MaxHandshakeAttempts`;
- v3.1: `RandomTrailers` and `DisableCookies` in addition to the v3 fields.

Version inference must be deterministic from the accepted field family and
must fail on mixed or incomplete generations. Booleans accept only the native
AWG spellings which can be normalized without ambiguity. Numeric values,
ranges, special-junk templates and the decoded header-protection key receive
explicit count/size/range bounds before any YAML is written.

The generated Mihomo proxy remains `type: wireguard`; Amnezia is enabled only
through `amnezia-wg-option`. AmneziaWG v3.0 fields require `version: 3`, while
v3.1 adds `random-trailers` and `disable-cookies`; both generations require
Mihomo 1.19.30+. A profile using newer fields on an older core gets a precise
version error before storage or service restart.

Mihomo 1.19.30's optional `ip-stack` is a local advanced setting, not an
imported provider value. Default to `mode: auto`; expose `gvisor`/`mips` and
`cubic`/`reno`/`bbr`/`bbr3` only in Settings with an explanation that the
congestion controller is ignored by gVisor. Do not guess a faster mode from a
key.

References: the [Mihomo WireGuard/AmneziaWG
schema](https://wiki.metacubex.one/en/config/proxies/wg/), [Mihomo
1.19.30](https://github.com/MetaCubeX/mihomo/releases/tag/v1.19.30), its
[`ip-stack` commit](https://github.com/MetaCubeX/mihomo/commit/8b76447b7af8528a846e74dfbfe6776e6349a228),
[AmneziaWG v3.0 commit](https://github.com/MetaCubeX/mihomo/commit/8453e589df956192fbdd713ad1e1c239d3d805ac),
[AmneziaWG v3.1 commit](https://github.com/MetaCubeX/mihomo/commit/8c56780b56b82230e071551d7d471b045fa28dfa)
and Amnezia's current native-config field names in
[`InterfaceConfig::toWgConf`](https://github.com/amnezia-vpn/amnezia-client/blob/707266124452c566e8a723633759502221542901/client/daemon/interfaceconfig.cpp).

### P4c — AmneziaVPN `vpn://` connection keys

This is a later input adapter, not part of the first WireGuard PR. Amnezia's
`vpn://` value is a base64url/qCompress-wrapped JSON connection document, not a
WireGuard share URI. It may contain several protocol containers or an API
endpoint instead of an immediately usable native AWG configuration.

If implemented, decoding must be offline, size/depth/count bounded and select
only an explicit WireGuard/AmneziaWG container. OmaVLESS must never call an
embedded `api_endpoint`, import SSH/server-administration credentials or fall
back to another container implicitly. The decoded native configuration then
passes through the exact P4a/P4b validator; there is no privileged or lenient
second path. A `.vpn` file containing one `vpn://` line and direct clipboard
paste can share this adapter.

Official Amnezia documentation distinguishes the app-specific `vpn://` key
from the native `amnezia_for_awg.conf`, which is the interoperable first
target: [sharing formats](https://docs.amnezia.org/documentation/instructions/share-connection/).

WireGuard and AmneziaWG remain experimental longer than URI protocols because
their imports contain a reusable private key, because AWG has several field
generations, and because routing semantics must be verified against both
OmaVLESS policy and the peer's cryptokey routes on a real system.

## Later Mihomo-native candidates

Consider AnyTLS, Shadowsocks and VMess only after the first four protocol
adapters demonstrate that the model stays understandable. Demand and real test
fixtures decide their order. Supporting every Mihomo outbound is not a product
goal by itself.

## Future phase X1 — optional Xray transport backend

This is intentionally not part of the active implementation queue.

The possible architecture keeps Mihomo responsible for TUN, DNS and routing,
while a separately managed Xray process exposes a loopback-only SOCKS outbound
for VLESS profiles which Mihomo cannot serve. It requires two supervised
processes, private local IPC, failure coordination, loop prevention, combined
diagnostics and a second core lifecycle.

Start X1 only when all of the following are true:

- V1–V3 and the protocol adapter foundation are mature;
- incompatibilities are confirmed to be core limitations, not importer gaps;
- affected real-world profiles are common enough to justify the complexity;
- the two-process design has an explicit threat model and rollback path;
- the user experience can still explain which core owns a connection.

Until then, a Mihomo-incompatible profile receives a precise compatibility
error rather than an automatic hidden fallback.

## Marketplace security preflight

Before submitting or updating the marketplace entry, audit the exact commit
being submitted rather than only running manifest/Quattro validation:

- reusable credentials travel over stdin or an already-private file descriptor,
  never argv, shell source text or process environment;
- every credential file is created with its final private mode before the first
  byte is written, then atomically replaced; symlink and owner checks cover the
  private directory, store, generated config and temporary exports;
- no user-writable file is consumed by a root service and no passwordless
  sudo/Polkit rule can activate user-controlled privileged configuration;
- remote authenticated APIs require HTTPS, except an explicitly validated
  loopback HTTP endpoint;
- provider/profile names and all other untrusted metadata reach QML only via
  `Text.PlainText` or a sink-local AutoText sanitizer; include bar tooltips,
  hover tooltips, dialogs, notifications and error/status strings in the audit;
- normal `omarchy plugin disable/remove` stops the owned core and TUN, disables
  login integration and removes generated user units; live-test hot reload,
  already-disabled removal and failure paths rather than relying on a repository
  uninstall hook that Omarchy does not execute;
- normal removal names every private or privileged file it intentionally keeps,
  and a documented purge path removes credential-bearing state after the
  service is stopped;
- the PR includes adversarial tests for process inspection, creation-time file
  modes, root-service boundaries, rich-text-shaped names and removal residue.

Package installation, one-time capability setup and user-service management
will still trigger marketplace manual review. They are declared capabilities,
not automatic vulnerabilities. Keep them narrowly scoped, explain why each is
required, and provide a no-passwordless-policy design for the reviewer.

## Pull-request discipline for future agents

- One roadmap slice per PR; no protocol grab bags.
- Every PR body names its direct predecessor and its expected successor.
- Every PR lists cloud checks separately from Omarchy-only checks.
- Parser changes include malformed and adversarial fixtures, not only happy
  paths.
- New secrets require preview, status, diagnostics, log, argv, QR and export
  review in the same PR.
- Generated config is tested with the oldest supported and current Mihomo when
  the feature is version-sensitive.
- A protocol does not leave experimental status in the same PR that first
  implements it.
- Xray transport work must not begin as a shortcut around an unfinished
  Mihomo adapter.
