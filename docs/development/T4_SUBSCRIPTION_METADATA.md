# T4 subscription usage metadata foundation

Status: selected 0.9.5 beta implementation: explicit, transient private read.
Ordinary refresh still consumes only the body. No usage enters storage,
scheduling, ordinary status, support reports or the QML plugin.

## First working slice: explicit provider information

The TUI Subscriptions page uses `n/p` to select a subscription and `u` to
request its provider information. This is navigation/read, not Connect or feed
refresh. Opening the page, polling local state or choosing a row performs no
provider request. The runtime resolves one saved opaque ID through the canonical
private editor read, releases the owner/store lock for one bounded GET and
revalidates revision, ownership and the exact URL before returning a claim.
It shares the existing four-slot remote-fetch pool and three-second whole-request
budget, including validated redirects/body consumption. The existing supported
feed decoder must accept the body; arbitrary remote YAML/HTML cannot supply a
claim. Failures use fixed `subscription_unavailable`/ownership/conflict codes,
never provider-controlled prose.

The explicit private `subscriptions.usage` v1 response has scope
`private_provider_reported_usage`, `instanceId`, availability `reported` or
`not_provided`, and optional usage. Upload/download/total counters and optional
expiry use canonical bounded decimal strings, preserving u64 values without
cross-client floating-point loss. It has no names, IDs, URL, raw header, endpoint,
or reusable credentials. This is still private account information, not a
shareable support projection.

The client retains only the last explicit result on the selected page in this
window. Leaving the page, changing selection, changing owner/revision, failed
state reads or closing the TUI discards it. An older in-flight reply cannot
replace a newer explicit request. It never persists into the store, window
activity, diagnostics or ordinary lists. Therefore this slice needs no new
store migration, downgrade erasure policy or second writer; the unresolved
persistent quota candidates are deliberately excluded.

EN/RU labels explicitly attribute values to the provider, not local measurement
or VPN health. Remaining bytes use checked arithmetic and are suppressed for
zero total, overflow or usage exceeding total. Zero/missing expiry is unavailable,
not “never expires”; nonzero dates are unambiguous Gregorian UTC date/times,
never an entitlement verdict/countdown. Private synthetic EN/RU terminal review
and exact installed acceptance remain separate from deterministic tests.

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
failing a usable feed. Only the explicit transient read presents the value;
ordinary refresh discards it and no production store consumes it.

Active extraction keeps using all header values, not a single-value
`get`; bind only the final accepted response after every validated redirect.
The metadata parser's error must remain non-fatal to a usable subscription
feed rather than becoming a refresh or connection error. Decide private-store
retention and expiry presentation under a later contract, including staleness
after refresh, deletion, and server clock skew. Do not expose per-account usage
in shareable support output.

This is a new T4 feature, not an R-stage migration. The established Rust feed
transport remains the production owner; there is no Python parity or host
host networking configuration effect. Deterministic parser, transport, private
socket and TUI state/render tests are applicable. Live provider interoperability
and installed UI review cannot be inferred from those tests; record exact-head
results in the owning beta PR without copying private counters.
