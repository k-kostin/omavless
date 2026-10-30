# Supported protocols

> Development guide for the proposed **0.9.9** snapshot. For the published
> release, use the [current main guide](https://github.com/k-kostin/omavless/blob/main/docs/user/NATIVE_INSTALL.md).

OmaVLESS parses supported share links into a bounded internal model. It does
not copy unknown link fields or arbitrary provider configuration into Mihomo.

## VLESS

Supported transports include TCP, WebSocket, HTTP, H2, gRPC and XHTTP with
None, TLS or REALITY security where the combination is valid.

The parser validates UUID, host, port, flow, packet encoding, SNI, fingerprint,
REALITY public key/short ID and all recognized transport options. Duplicate
fields, conflicting aliases, malformed booleans and unknown connection options
fail before storage.

Both Xray Vision share-link values are recognized. Mihomo exposes only
`xtls-rprx-vision`; OmaVLESS preserves the imported variant for review without
claiming identical UDP/443 behavior between cores. Vision is accepted only
with TCP plus TLS or REALITY.

Xray `packetEncoding=xudp` and `packetEncoding=packetaddr` are supported.
Unknown packet encodings are rejected.

### XHTTP

Supported modes are `auto`, `stream-one`, `stream-up` and `packet-up`.
A bounded XHTTP `extra` object can represent the shared client subset for
headers, padding, session/sequence placement, upload sizing and XMUX/reuse.
Depth, count, keys, strings, numeric ranges and header syntax are constrained.

Upload/download separation may use one bounded secondary endpoint with its own
path, host, headers, XMUX and TLS/REALITY settings. Recursive download config,
certificates, `sockopt` and unsupported overrides are rejected.

### VLESS Encryption and REALITY PQ metadata

Experimental Mihomo `mlkem768x25519plus` Encryption values are validated for
grammar, client-key encoding, mode, RTT, padding, token count and total size.
The complete value stays private.

Experimental `supportX25519MLKEM768` metadata maps to Mihomo's
`support-x25519mlkem768` field and produces an honest device-verification
warning. The selected uTLS fingerprint still has to offer the required group;
the flag alone is not a connection guarantee.

Xray spider-path metadata is disclosed as unmapped because current Mihomo has
no corresponding client field. Xray-only ML-DSA verification metadata is
rejected rather than silently ignored.

## Trojan — Experimental

Trojan URI import supports TCP, WebSocket and gRPC with TLS or Mihomo's
supported REALITY subset. Password, SNI, ALPN, certificate-check preference,
uTLS fingerprint, WebSocket host/path, gRPC service name and bounded REALITY
fields map explicitly. Unknown or unrepresentable fields are rejected.

## Hysteria2 — Experimental

Official `hysteria2://` and `hy2://` links support authentication/userpass,
multi-port hopping, SNI, certificate verification, SHA-256 pinning, ECH and
`salamander`/`gecko` obfuscation where representable by Mihomo.

Realm mode and a local handshake-timeout control are not offered. Provider
links cannot set local bandwidth or handshake tuning.

## TUIC v5 — Experimental

`tuic://` links require strict UUID/password parsing and support SNI, ALPN,
certificate verification, `native`/`quic` UDP relay and bounded Mihomo
congestion-control choices. TUIC v4 tokens and unknown connection options are
rejected. 0-RTT remains disabled.

## Subscriptions

One HTTPS subscription may contain plain-text or standard/URL-safe base64 lists
of supported links. Unrelated protocols are ignored; arbitrary Clash/Mihomo
YAML is not parsed. Refresh saves a validated replacement atomically. Manual
refresh is the default; an explicit Daily schedule is available through the
local CLI. Refresh never connects a VPN.

## WireGuard and AmneziaWG — Experimental

The native importer accepts bounded single-peer `.conf` files and supported
offline AmneziaVPN guest inputs containing such a configuration. Review the
redacted preview before saving. Native private export produces a `.conf`;
WG/AWG QR export is not offered. The supported input/export limit is 32 KiB.

The connectable subset is narrower than the imported data: standard WireGuard
and the supported AWG3 configuration shape, with IPv4 interface, peer
AllowedIPs, endpoint transport and imported DNS addresses. Dual-stack/IPv6
records can remain private data but are not IPv6 connection support. Other
AWG generations are not generally guaranteed.

A scalar keepalive, including explicit zero (disabled), is representable.
Variable ranges remain private data and refuse connection rather than silently
choosing a value. Unsupported AWG headers, padding and junk expressions also
refuse connection. MTU and DNS must fit the supported core contract; imported
AllowedIPs never become arbitrary host route commands. OmaVLESS's Full VPN,
Routing and Direct modes still determine its routing policy.

Multiple peers, duplicate or conflicting settings and executable directives
such as `PreUp`, `PostUp`, `PreDown`, `PostDown`, `Table` and `SaveConfig` are
rejected. Keep private/preshared keys, configuration and guest inputs private.
An accepted import is not a successful connection or server compatibility test.

## Not yet supported

OmaVLESS does not currently import:

- signed/API-based AmneziaVPN guest or subscription keys;
- Amnezia server-management/full-access keys;
- arbitrary remote Mihomo/Clash YAML.

Mihomo core support alone is not sufficient. These formats require bounded
parsing, structured private storage, redacted preview, installed-core checks
and representative server acceptance before they can be advertised.

Embedded server-management credentials and remote provider APIs are never
contacted to make an otherwise unsupported key work.

Protocol roadmap details live in [PROTOCOL_ROADMAP.md](../../PROTOCOL_ROADMAP.md).
