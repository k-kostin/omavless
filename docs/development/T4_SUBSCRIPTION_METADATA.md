# T4 subscription usage metadata foundation

Status: inactive parser, transport-extraction and pure store-model candidates. Current refresh
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
preserve duplicate JSON member occurrences. The inactive usage model now runs
a bounded, recursive raw-JSON pass *before* that parser: repeated decoded keys
at any depth, including escaped-equivalent and Unicode keys, return only the
fixed `InvalidStore` category. This check is local to optional usage read and
candidate composition; it does not change the production private-store reader
or reject an otherwise usable subscription feed. A future fetch caller must
continue to discard optional metadata errors without failing a successful
feed. The raw-duplicate gate is covered by synthetic tests, not host data.

This is a new T4 feature, not an R-stage migration. The established Rust feed
transport remains the production owner; there is no Python parity or host
network effect in this inactive foundation. Deterministic parser and store-model
tests are the applicable gate. Live provider interoperability and private UI
review remain unrun and cannot be inferred from these tests.

## Atomic refresh composition: executable proposal

`omavless-runtime/src/quota_refresh_candidate.rs` is compiled only in tests.
It joins the final `FetchedSubscription` response with the strict store model
and existing feed decoder/refresh planner. It returns one private candidate
containing the accepted feed and its optional usage; no intermediate feed-only
payload is returned, and no file, network, IPC or lifecycle action is performed.
Synthetic ID generation is deliberately confined to that test module.

The proposal validates raw members before either snapshot or completion parsing,
rechecks the captured URL/refresh token against the latest store, preserves
concurrent name and unrelated changes, and uses the actual resulting monotonic
refresh token rather than the wall-clock input. The target's old claim is removed
in memory before feed normalization so it cannot consume the new feed's budget.
Invalid observation time or optional-metadata size growth discards the metadata;
it does not fail an otherwise usable feed. Missing metadata also clears the old
claim. A feed or stale-snapshot failure returns no replacement at all.

The test model defaults to discarding persisted usage. Its alternative
`ModelPrivatePersistence` exercises the proposed schema; the enum is not a
production capability or an approval mechanism. An actual existing body-only
refresh is used as a downgrade counterexample: it retains unknown claim bytes
while advancing the token, making them unreadable but not physically erased.
The next modeled discard refresh removes them. Release still needs a reviewed
downgrade/migration policy before private persistence may be enabled.

This is **payload composition**, not proof of a durable transaction. A future
production caller must keep the original latest bytes (before claim removal)
as its exact compare/rollback baseline and use the existing single atomic writer
under the owner/migration lease. Its native generation, global revision,
cancellation and lifecycle compensation checks must surround that one commit.
No metadata write may follow a separately committed feed. Failed or uncertain
publication must restore both old feed and old usage together or retain the
existing recovery barrier. Batch refresh needs equivalent all-member composition;
this single-feed model does not implement batch, new subscription, URL-edit,
UI/IPC exposure, or a live worker. Those are explicit activation gates.
