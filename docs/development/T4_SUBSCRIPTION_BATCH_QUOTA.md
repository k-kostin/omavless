# T4 all-member quota candidate

Status: inactive, test-only batch composition stacked on the single-feed
transaction candidate. No worker, IPC, UI, package or persistent schema path
uses this code.

The batch candidate reuses the established all-member refresh snapshot and
planner. It validates raw store members before JSON normalization, captures all
subscription IDs and URLs privately, and rejects a missing, extra, reordered
or stale member before returning any replacement. Every fetched feed is decoded
before the planner runs. A single synthetic ID source spans all feeds, and the
planner gives every accepted member its actual common monotonic refresh token.

Old usage claims are removed from the in-memory candidate before feed size
validation; the latest original bytes remain the atomic writer's comparison and
rollback baseline. Optional claims are then bound to the final token and exact
stored URL. Missing or invalid metadata removes that member's old claim.
Aggregate metadata growth falls back deterministically to the complete
feed-only batch instead of privileging whichever provider was processed first.
An empty batch preserves the exact input bytes and invokes neither clock nor ID
generation. None of these outcomes are VPN-health claims.

Synthetic tests cover distinct claims on two feeds, absent/invalid metadata,
default discard, stale membership and feed failure, concurrent renames and
unrelated fields, aggregate size fallback, and empty no-op. One isolated test
passes the complete candidate to the existing private atomic writer, verifies
one publication, restores both old feeds and claims byte-for-byte after a
modeled downstream rejection, and refuses to overwrite unexpected bytes.
There is no real lifecycle or owner completion callback in this slice.

The older body-only runtime can retain unknown `providerUsageV1` account bytes
on downgrade. Binding hides a stale claim but does not erase it. Production
retention remains blocked on a reviewed downgrade and physical-erasure policy,
plus all-member worker integration, add/edit/delete semantics, owner/revision/
cancellation fencing, private presentation, and installed acceptance. The
single-feed and batch candidates do not authorize one another's activation.

Applicable local check:

```sh
cargo test --locked -p omavless-runtime quota_refresh_candidate --lib
```

Only synthetic data is permitted in test output and review evidence. No live
provider, VPN, desktop proxy or host firewall state is touched.
