# T4 subscription usage metadata foundation

Status: inactive parser candidate. It does not change current subscription
fetching, storage, IPC, UI, scheduling, routing, or VPN behavior.

`Subscription-Userinfo` is a de facto provider response header, not an HTTP or
Mihomo networking guarantee. The official MetaCubeXD project describes its
`upload`, `download`, `total`, and `expire` fields in its [profile manager
documentation](https://github.com/MetaCubeX/metacubexd#profiles--config-editor-desktop--server).
The [3x-ui subscription documentation](https://github.com/MHSanaei/3x-ui/blob/main/docs/content/docs/en/config/subscription.mdx)
describes the first three as byte counts and `expire` as Unix seconds. These
sources establish an interoperability vocabulary, not provider truth or a
universal promise that every feed supplies metadata.

The pure Rust parser in `omavless-domain` takes a final status and response
headers. It ignores redirect/error responses and absent metadata; for a present
header it accepts one case-insensitive header name and exact ASCII lower-case
field names separated by semicolons, with or without surrounding spaces.
`upload`, `download`, and `total` are required; `expire` is optional. Duplicate,
unknown, partial, oversized, control-character, non-decimal, overflowing, and
out-of-calendar-range data fails with a fixed category. Values are untrusted
provider assertions. In particular `total=0`, an expiry of zero, and an expiry
in the past are not interpreted as unlimited service, valid access, or VPN
health. No raw value is included in errors, ordinary diagnostics, or `Debug`.

Before any active transport integration, separately review how the HTTP library
exposes duplicate headers after each manually validated redirect. Only the
final accepted response may contribute metadata, and invalid metadata must
never turn a usable subscription feed into a failed refresh or a connection
error. Decide private-store retention and expiry presentation under a later
contract, including staleness after refresh, deletion, and server clock skew.
Do not expose per-account usage in shareable support output.

This is a new T4 feature, not an R-stage migration. The established Rust feed
transport remains the production owner; there is no Python parity or host
network effect in this inactive foundation. Deterministic parser tests are the
applicable gate. Live provider interoperability and private UI review remain
unrun and cannot be inferred from these tests.
