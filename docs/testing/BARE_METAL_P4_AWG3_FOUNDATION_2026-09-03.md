# Bare-metal P4 AWG3 foundation acceptance — 2026-09-03

This records a credential-safe parser and installed-core configuration check on
the owner's x86_64 Omarchy machine. It is **not** connectivity, routing-mode or
product-enablement evidence.

## Scope

- Rust `omavless-profile` one-interface/one-peer WG/AWG parser;
- deterministic AWG generation recognition;
- private canonical identity and redacted fact/debug projections;
- Mihomo WireGuard + `amnezia-wg-option` rendering;
- installed-core configuration validation only.

The private fixture stayed outside the repository, was restricted to mode
`0600`, and was passed by file path only to an opt-in local test. No key,
endpoint, address, DNS value, subscription URL or rendered YAML was emitted in
the test report.

The reusable test is intentionally ignored by the ordinary workspace suite and
runs only with both owner-provided environment variables and Cargo's
`--ignored` flag.

## Evidence

- Host: bare-metal Omarchy, x86_64.
- Core: Mihomo Meta `v1.19.30 linux amd64`.
- Fixture family: native AmneziaWG 3.
- Shape: one Interface, one Peer, IPv4 local address, numeric DNS entries and
  425 bounded split-tunnel `AllowedIPs` prefixes.
- Generation markers: AWG3 header protection, padding/rekey/timeout fields and
  the common junk/header fields were present and complete.
- Result: strict parse passed.
- Result: the same private native config passed a Qt-compatible `qCompress`
  `vpn://` envelope round trip into the strict parser.
- Result: generated full Mihomo configuration passed `mihomo -t`.
- Network side effects: none; no service/TUN start, stop or restart occurred.

The provider's AWG3 peer keepalive is a range. Mihomo `v1.19.30` exposes
`persistent-keepalive` as one integer, so the renderer selects the lower bound
deterministically and exposes `keepalive_range_normalized` in safe facts. This
compatibility choice still requires a real connection test.

## Upstream basis

- Amnezia Client `dev` commit
  `398697c101d27d9d4fb2f9a25ebe7749925f0d61` for `vpn://` Qt `qCompress`
  encoding, AWG keys and generation markers.
- Mihomo tag `v1.19.30` for WireGuard and `amnezia-wg-option` field mapping.

## Remaining P4 gates

- expose the Rust importer through the compatibility/runtime bridge;
- store/migrate WG/AWG profiles in the private schema;
- add bounded redacted import preview and UI capability gating;
- test the owner fixture against its matching server;
- exercise Full VPN, Routing and Direct plus reconnect/autoconnect;
- cover IPv6 where available, older-core rejection, update/remove rollback and
  credential-leak review;
- implement exact structured Amnezia JSON container selection for P4c without
  API, SSH or server-admin behavior.
