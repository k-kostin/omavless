# OmaVLESS protocol roadmap

Status: planning and integration guide, 2026-08-24.

This document is the canonical order for extending OmaVLESS from its current
VLESS focus into a Mihomo-native, multi-protocol client without expanding the
compact bar widget or weakening credential handling. It also tells the agent
running on the Omarchy machine which stacked pull requests must be verified and
integrated first.

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

## Current integration queue

The open work is a linear stack. Test and integrate it in this exact order:

1. [#1 — country routing presets and general Settings](https://github.com/k-kostin/omavless/pull/1)
2. [#2 — guided setup and login autoconnect](https://github.com/k-kostin/omavless/pull/2), based on #1
3. [#3 — custom routing tools and rule refresh](https://github.com/k-kostin/omavless/pull/3), based on #2
4. [#4 — favorites, import review and safe diagnostics](https://github.com/k-kostin/omavless/pull/4), based on #3
5. The roadmap PR containing this document, based on #4
6. VLESS and later protocol PRs listed below, each based on the preceding
   unmerged PR until the stack is integrated

For each stacked PR on the Omarchy machine:

1. read its `Depends on` and live-test checklist;
2. check out its head branch and run `bash tests/run.sh`;
3. perform the listed Omarchy, systemd, TUN and real-Mihomo checks;
4. leave failures on that PR instead of compensating in a later branch;
5. merge its predecessor first;
6. retarget the PR to `main` after its predecessor is in `main`;
7. confirm that the retargeted diff contains only the intended isolated slice;
8. merge it, then repeat for the next PR.

Do not merge a later PR merely because its combined tip works: doing so would
hide which slice introduced a live regression. Draft status is removed only
after the Omarchy checks for that PR pass.

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
- audit current REALITY aliases and the generated `spx`/`spider-x` field
  against the installed Mihomo version;
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

Goal: translate the useful advanced XHTTP fields supported by Mihomo without
turning a share link into arbitrary configuration input.

Scope:

- accept a size- and depth-bounded URL-encoded JSON `extra` object;
- translate only a documented whitelist of headers, padding, session/sequence,
  upload, reuse/XMUX and download settings;
- map Xray-style camelCase names to Mihomo fields explicitly;
- reject unknown security-sensitive structures instead of copying them;
- preserve the existing simple `path`, `host`, `mode` and `alpn` path.

Omarchy acceptance requires real keys for each implemented XHTTP mode. At least
one test must combine XHTTP with TLS and one with a compatible REALITY server.

## Phase V3 — experimental VLESS Encryption and REALITY PQ metadata

Goal: expose Mihomo capabilities which the current importer intentionally
rejects, with an explicit experimental marker.

Scope:

- accept a bounded Mihomo-compatible VLESS Encryption value as opaque secret
  material rather than reimplementing its cryptography;
- add `support-x25519mlkem768` when represented by the imported profile;
- redact encryption and post-quantum key material everywhere;
- show the experimental capability in preview without showing the secret;
- retain the existing `encryption=none` path unchanged.

The parser must not infer that a failed new-Xray REALITY server is a malformed
key. Where the failure is ambiguous, the UI reports a compatibility hint rather
than a false diagnosis.

## Phase A1 — protocol-neutral profile adapters

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

This phase is deliberately user-visible only as clearer neutral wording such as
“profile” or “connection”. It must not add a partially implemented protocol.

## Phase P1 — experimental Trojan

First additional protocol because its URI and Mihomo mapping are closest to the
existing VLESS path.

Initial scope:

- `trojan://` manual import and mixed-protocol subscription import;
- TCP, WebSocket and gRPC transports;
- TLS, SNI, ALPN, uTLS fingerprint and Mihomo-supported REALITY fields;
- password-safe preview, export, QR and diagnostics;
- existing routing, favorites, probes and autoconnect without special cases.

Trojan can leave experimental status after successful tests against multiple
independent servers and one subscription provider.

## Phase P2 — experimental Hysteria2

Initial scope:

- official `hysteria2://` and `hy2://` URI forms;
- authentication, multi-port/port hopping, SNI, certificate pinning and
  `salamander`/`gecko` obfuscation;
- no Realm mode in the first slice;
- bandwidth values remain local client settings and are not trusted from a
  shared URI.

Omarchy testing must cover networks where UDP/QUIC works and where it is
restricted, with errors that distinguish server failure from blocked UDP.

## Phase P3 — experimental TUIC v5

Initial scope:

- TUIC v5 UUID/password profiles only;
- TLS/SNI, ALPN, UDP relay mode and bounded connection options;
- conservative defaults for 0-RTT and congestion control;
- no TUIC v4 token mode until a real compatibility fixture is available.

## Phase P4 — experimental WireGuard file import

Initial scope:

- explicit import of a standard WireGuard `.conf` with one peer;
- IPv4/IPv6 interface addresses, endpoint, public/private/preshared keys,
  allowed IPs, keepalive, MTU and optional reserved bytes;
- translation into a Mihomo WireGuard outbound while OmaVLESS remains the
  owner of traffic splitting;
- no invented `wireguard://` standard and no multi-peer editor initially.

WireGuard remains experimental longer because it has no single interoperable
share-link format and because imported `AllowedIPs` must not silently override
OmaVLESS routing policy.

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
