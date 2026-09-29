# T4 subscription usage metadata foundation

Status: inactive parser and pure store-model candidates. They do not change
current subscription fetching, production storage, IPC, UI, scheduling,
routing, or VPN behavior.

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

The follow-up loopback-only test uses the current locked `ureq` 3.4.0 client and
its existing redirect-disabled agent configuration. Two differently cased
`Subscription-Userinfo` response lines remain two values under `get_all` and
under `headers().iter()`; they are not silently folded into one value. The
pure parser rejects the latter iterator as a duplicate. A `302` response's
metadata is ignored, and only the separately requested final `200` contributes
usage. A malformed optional usage header still leaves the existing production
feed fetch and decode successful. These synthetic tests contact only a bound
loopback listener; they do not observe a real provider or prove future caller
composition.

Before active integration, keep using all header values, not a single-value
`get`; bind only the final accepted response after every validated redirect.
The metadata parser's error must remain non-fatal to a usable subscription
feed rather than becoming a refresh or connection error. The proposed
[presentation contract](T4_SUBSCRIPTION_PRESENTATION.md) records the separate
retention and UI decisions still required; it does not activate the feature.
Do not expose per-account usage in shareable support output.

An additional inactive pure store candidate now models one optional
`providerUsageV1` extension on a subscription record. A valid claim is bound
to both a domain-separated digest of the exact stored URL and the current
successful-refresh token (`updatedAt`); its provider counters and observation
time are private. A legacy or malformed extension is absent, not a reason to
reject otherwise valid profiles. Composing `None` into a not-yet-committed
successful refresh physically removes the field; replacement URL, a later
refresh token, or deletion also make an older claim unreadable, even if a
previous runtime preserved the unknown field. The existing production
refresh/store pipeline does not call this model.

Activation still requires one atomic composition of accepted feed and optional
usage before the existing compensated store commit, with concurrency snapshots
revalidated and old bytes restored on failure. Writing usage in a second
post-refresh transaction would create a stale-account window. Older v3 clients
can preserve unknown extension bytes after URL replacement or refresh; the
binding makes them semantically unavailable, but physical erasure by those
older clients is not guaranteed. A reviewed compatibility/migration policy is
required before release, especially if downgrade to an older runtime remains
supported.

The current canonical store parser uses `serde_json::Value`, which does not
preserve duplicate JSON member occurrences. This inactive model validates the
resulting bounded value and suppresses malformed shapes, but it does not prove
that a hand-edited document had no duplicate `providerUsageV1` keys. Before
activation, decide whether to reject duplicate members at the private-store
boundary or require a canonical re-read of writer-produced bytes; do not
advertise this model as strict raw-JSON duplicate detection.

This is a new T4 feature, not an R-stage migration. The established Rust feed
transport remains the production owner; there is no Python parity or host
network effect in this inactive foundation. Deterministic parser and store-model
tests are the applicable gate. Live provider interoperability and private UI
review remain unrun and cannot be inferred from these tests.
