# T4 subscription usage metadata foundation

Status: selected 0.9.5 beta foundation: explicit, transient private read.
Ordinary refresh still consumes only the body. No usage enters storage,
scheduling, ordinary status or support reports. The original TUI slice is
retained below; the separate development QML slice is explicit and transient too.

## Development QML slice: explicit subscription usage

On one browsed subscription, the bordered **Check usage** action opens a private
**Subscription data** dialog and makes one explicit request. Ordinary page
opening, selection, local polling and locale changes do not request provider
data. **Check again** starts one new read only when the previous child has ended;
there is no automatic retry or feed refresh. The fixed CLI command is
`omavless subscription usage SUBSCRIPTION_ID`, forwarded by the launcher's
`native-subscription-usage` command. Only the saved opaque ID crosses argv;
the existing runtime resolves the URL privately. No generic RPC escape hatch,
new provider parser, persistent field or runtime ownership policy is introduced.

The dialog distinguishes loading, provider-reported data, metadata not supplied
and unavailable reads. It identifies the current subscription and attributes
values to the provider, not locally measured VPN traffic or verified entitlement.
Closing/Escape, page/target changes, lost admission and owner/revision changes
erase the result. The single active child handle plus generation, opaque ID,
instance and revision fences reject late replies, including copied QML Process
contexts; completion cannot schedule another request. The child has a bounded
client watchdog without weakening the runtime's existing request budget.

Counters remain canonical u64 decimal strings through checked sum/subtraction
and binary-unit formatting. The presentation marks approximate byte values,
uses an English decimal point or Russian comma, and never coerces a whole u64
counter to a floating-point Number. Missing/zero limits and expiry remain
unavailable, not unlimited service or no expiry. Gregorian date/time is explicit
UTC; no entitlement verdict or countdown is added. Provider labels stay plain
text and untranslated. Data scrolls while Close and Check again remain fixed.

This changes the compiled CLI, so final integration requires a newly matched
both-architecture native pair and its applicable installed journey, not assumed
equivalence with the preceding packages. Source/function tests, isolated actual
QML rendering and installed new-pair acceptance are separate evidence classes;
the feature's owning PR must state their exact candidate and unrun gaps.

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
