# T4 quota candidate: private-store transaction evidence

Status: test-only adapter stacked on Draft #371 at
`932c45aff44776ed046380a5f618764e99874f8d`. No runtime, store schema activation,
IPC, UI, provider request, timer, package or installation change.

## Decision and compatibility boundary

The [metadata contract](T4_SUBSCRIPTION_METADATA.md) requires accepted feed and
optional usage to enter one existing compensated store transaction. #371
proved candidate composition but not actual publication or restoration. This
slice connects that candidate to `PreparedPrivateStoreWrite` in tests, using
the existing migration lock, same-user file checks, atomic replacement and
verified restore. It introduces no second writer in the production path.

Production activation remains blocked by a concrete unresolved policy: older
v3 runtimes preserve unknown `providerUsageV1` bytes after URL replacement and
refresh. Digest/token binding suppresses a stale claim, but does not physically
erase that account information. The existing downgrade counterexample remains
executable. New persistent private data therefore requires a reviewed downgrade
and migration policy, plus all-member refresh/add/edit/deletion handling,
before it can be enabled. Current production body-only behavior is preserved.

## Adapter and tested outcomes

`quota_refresh_candidate/transaction.rs` is reachable only through the existing
`cfg(test)` module. Its synthetic refresh has fixed test times and IDs. It holds
the matching migration lock while preparing one combined candidate from the
actual latest store read by the established writer. In-memory claim removal
never replaces the original compare/rollback bytes.

The adapter consumes the prepared transaction once. A test-supplied acceptance
callback models a rejection after publication; it is not a real lifecycle
observation. Final readback must agree before success. Failed publication or
rejected completion restores both old feed and old usage together only when
the established writer recognizes the current bytes. Unknown bytes or unsafe
files yield `RecoveryRequired`, never an unconditional overwrite. This is a
test outcome, not an implemented durable recovery receipt or owner barrier.

Synthetic temporary-directory cases cover:

- One combined feed/token/usage replacement, private `0600` mode, repeat-commit
  refusal, exact original whitespace and claim restoration, idempotent restore.
- Successful missing-metadata/default-discard publication physically removes
  the old claim; rejected completion restores it with the feed.
- Concurrent unrelated edits and subscription rename survive publication and
  restoration to the exact latest baseline.
- Wrong migration lock, invalid feed and stale snapshot refuse preparation
  without replacing bytes.
- Unexpected bytes before commit or during compensation remain untouched and
  require recovery; a callback cannot claim success after corrupting readback.
- Unsafe file permissions before publication or before restore refuse success.

Only synthetic stores under dedicated test directories are touched. No raw
headers, provider URLs, account data, profile data or private path is emitted
by adapter errors. The adapter has no production exports or CLI activation flag.

## Gates and limits

Run `cargo test --locked -p omavless-runtime quota_refresh_candidate --lib`,
`./tests/run.sh`, `./tests/run-rust.sh` and `git diff --check`. Exact commit and
CI outcomes belong in the stacked Draft PR. This is new T4 contract evidence,
not a Python migration or a replacement for earlier reference fixtures.

The new tests prove real single-file publication and synchronous compensation,
not crash/power-loss durability, native generation/global revision/cancellation
fencing, actual lifecycle compensation or production owner recovery. Those
already established owner protections must surround any eventual integration;
the test callback grants none of them. Batch refresh, URL-edit/deletion physical
erasure, retained-history semantics, bounded private IPC projection and EN/RU
presentation remain separate work. No UI availability is claimed.

No installed Try Omarchy, physical-PC, live-provider or network-transition gate
is applicable to this inactive synthetic adapter. Before activation, require
exact-head installed manual-refresh/store rollback acceptance and the separate
privacy/presentation gates. Existing AUTO-1, DNS/provider and V0 deferrals stay
open; this slice neither reruns nor completes them.
