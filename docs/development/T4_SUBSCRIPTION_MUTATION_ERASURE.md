# T4 subscription mutation and private quota erasure

Status: **test-only design evidence** on a Draft branch. Nothing here changes
the production subscription editor, private store, IPC or live VPN behavior.

The optional `providerUsageV1` claim is bound to a subscription URL digest and
refresh token. That binding prevents a different URL from reading the claim,
but it does not erase the bytes. The current body-only subscription update
preserves unknown JSON fields. A URL A → B → A edit with a repeated token
(same-millisecond updates or a clock rollback) can therefore expose A's old
account claim again. Treating a digest mismatch as revocation is insufficient.

`quota_refresh_candidate::mutation` exercises this counterexample against the
real store mutation function with synthetic URLs. It then models a future URL
edit that clears the old claim *inside the same complete candidate* before one
private atomic publication. The tests check that an A → B → A roundtrip cannot
recover the claim, the published file retains mode `0600`, and a downstream
rejection restores the exact original bytes. They do **not** register a live
mutation owner or prove host-authorized recovery.

Production activation still needs an owner-reviewed policy for all mutation
paths, including delete, import, URL replacement and older-version downgrade.
The owner must ensure the latest store is read under the proper lease, the old
claim is physically erased on any URL change, the resulting feed and claim are
committed together, and stale bytes are never overwritten during compensation.
An older release that preserves unknown fields can retain private claims;
supporting downgrade requires an explicit migration or removal policy. Until
that policy and private-data review are complete, persisted provider usage and
its UI/IPC exposure remain disabled.
