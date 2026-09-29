# T4 subscription usage metadata foundation

Status: inactive parser and transport-extraction candidate. Current refresh
still consumes only the body; no metadata enters storage, IPC, UI or scheduling,
and routing/VPN behavior is unchanged.

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

The follow-up loopback-only tests use the current locked `ureq` 3.4.0 client and
its existing redirect-disabled agent configuration. Two differently cased
`Subscription-Userinfo` response lines remain two values under `get_all` and
under `headers().iter()`; they are not silently folded into one value. The
pure parser rejects the latter iterator as a duplicate. A `302` response's
metadata is ignored, and only the separately requested final `200` contributes
usage. A malformed optional usage header still leaves the existing production
feed fetch and decode successful. These synthetic tests contact only a bound
loopback listener; they do not observe a real provider or prove future caller
composition. An additional body-plus-optional-usage transport seam parses only
the final accepted response, preserving the existing body-only caller. Invalid,
non-UTF-8 and duplicate usage values become unavailable metadata rather than
failing a usable feed; no production store or presentation consumes the value.

Before active integration, keep using all header values, not a single-value
`get`; bind only the final accepted response after every validated redirect.
The metadata parser's error must remain non-fatal to a usable subscription
feed rather than becoming a refresh or connection error. Decide private-store
retention and expiry presentation under a later contract, including staleness
after refresh, deletion, and server clock skew. Do not expose per-account usage
in shareable support output.

This is a new T4 feature, not an R-stage migration. The established Rust feed
transport remains the production owner; there is no Python parity or host
network effect in this inactive foundation. Deterministic parser tests are the
applicable gate. Live provider interoperability and private UI review remain
unrun and cannot be inferred from these tests.
