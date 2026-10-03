# T4 retained historical-context foreign-owner refusal — 2026-10-03

This is a `cfg(test)` research correction, not normal historical-policy adoption
or a new restore/backend API. Draft #578 is stacked on
`652898c3c571a8189caff4d1d69000fa79360ced`. Admission fix code is
`6d9efa70a1b6e6ba7b1a6591e524b96f6ab44094`; final test-expectation-corrected
code is `50d41ee8b341d5a466eb78b6c34cc813c528e561`.

## Defect and narrow admission boundary

Historical Profile and Connection contexts retain the identity of the actual
coordinator that first binds them. Previously, a valid submission to a different
coordinator called the destructive binding check: refusal poisoned the context
belonging to the original coordinator. Connection admission additionally latched
the receiving coordinator into manual recovery despite no receiving-owner effect.

Both typed research admissions now refuse an already foreign-bound Arc before
calling `bind_owner` or the transaction-blocking proof checks. This borrowed
identity comparison neither rebinds nor supplies authority. Unbound and same-owner
contexts still undergo every existing lease/source/epoch check. Direct lower-level
`bind_owner` misbinding still permanently poisons the context; that primitive and
its negative regression are retained.

The receiving owner's early unknown-new-intent close invalidation remains first,
before typed kind/proof admission. Known-ID retry semantics are unchanged. Foreign
refusal does not validate private facts for the original owner: its next admission
still performs the original complete proof checks and must poison on actual loss.

## Actual counterexamples, without original-owner recapture

Four separately named tests cover Profile/Connection multiplied by Commit/Abort
history. Each creates two actual coordinators over the same synthetic private paths
and generation. Original A succeeds in an actual Favorite or Connect, binding the
retained context. B's valid new operation refuses with private file bytes and
identities, revisions and lifecycle calls unchanged. Without replacing or
recapturing A's context, A then exact-Replays and performs a new actual
Favorite/Disconnect. Connection cases also prove B can subsequently use its own
independent Off context rather than remain latched by the foreign refusal.

Profile lifecycle methods panic if called. Connection uses the existing faulting
synthetic host, not a real core or network. These fixtures do not manufacture
installed-manager/package authority or claim private-profile acceptance.

The independent test-only pre-fix checkpoint
`d7699ad2db551151e19520f669f90475706af86d` leaves inherited admission code
unchanged. All four cases fail at A's exact Replay with `ManualRecoveryRequired`,
after the nonmutating B-refusal assertions: **0 passed, 4 failed, 0 ignored**, 12.28s.
The same four cases pass on fixed code: **4 passed, 0 failed, 0 ignored**, 8.79s.
That original negative remains evidence, not a superseded passing result.

The full frozen suite on `6d9efa7` then reported **1117 passed, one failed,
33 ignored**, 617.28s. Its sole failure was the inherited Connection DNS/collision
test still expecting a foreign caller to poison the context. Test-only followup
`50d41ee` preserves its actual DNS/collision/refusal assertions, asserts original
Replay and unchanged private bytes/identities/revision/calls, and then explicitly
misbinds the lower-level context to prove permanent poison/reuse refusal. That
corrected aggregate case passes (5.39s); the four independent counterexamples
also pass on `50d41ee` (15.13s). The earlier full-suite failure is not relabeled PASS.

## Immutable evidence and checks

Dedicated offline/locked HOME builds produced immutable test executable copies
outside Cargo; the full suite and focused tests never rebuild a running self-exec
artifact. Build caches and branch writes remained task-local.
After all local builds and test invocations settled, only the dedicated task
Cargo target was cleaned (3.0 GiB reported). Immutable executables/source archives
and private successful/failed receipts remain retained outside Cargo.

| Artifact | SHA-256 |
| --- | --- |
| Pre-fix executable | `154a196b947e59d8f670d42e019cf225c6687be8a59c0e003bfbcdef8c3da26a` |
| Pre-fix source archive | `5ae52bd52b33c42619aac01aeeeae9d107fb27a48a10c1f499706d67e9aed64e` |
| Admission-fix executable (`6d9efa7`) | `22adaea3ba044e9c0806ad8d6bd46022c77dd6c2f38e9108021e2fa3d0aaeb96` |
| Admission-fix source archive | `35e8e1c895f2f62d40650575b9c9c4eec0005858810285d80328c2323ebe041d` |
| Final executable (`50d41ee`) | `076bb63eda8079ff8eaf940b7a819d0f39803ef19aaf66394c5ff21d9153e751` |
| Final source archive | `4ae30669d68884b116c581252a772f522b60f9aa20220fb8508894ed80083c59` |
| Independent VM guard | `0962465753a801f19bb5fe328acf91452e203cbfef05ddd7e36da44f7b8f1a2f` |

The direct lower-level poisoning regression passed once; ten existing actual
typed-entry revocation/known-ID checks passed. Strict workspace/all-target and
explicit TUI clippy, formatting/diff checks and no-default-features runtime check
passed on the final test-corrected head. Final frozen runtime at eight threads:
**1117 passed, zero failed, 33 ignored, one filtered**, 491.58s. The sole filtered
case is the existing resource-cleanup deadline fixture, run separately as the
workflow requires: **one passed, zero failed, zero ignored**, 12.34s. Together
these invocations execute all 1118 non-ignored runtime library tests; the 33
ignored internal-worker/explicit opt-in tests are not claimed as acceptance.

Independently built and frozen normal `omavless` executables (local default-feature
debug build) on `6d9efa7` and `50d41ee` are byte-identical (`cmp`), SHA-256
`15957e48fe232dd25dbfe496e3aa23d4cdf0f048f03fc3a20148b40d61e65458`.
This confirms the test-only followup did not change that normal binary; it does
not turn an earlier test/VM result into execution of a different source head.

Source/frontend checks passed: **503 tests, two skipped**, plus JS/QML/navigation
gates. The first HOME-temp invocation had three unchanged native V0 private-path
guard errors: an existing home-root `.git` marker makes HOME descendants ineligible
as private evidence paths. That refusal is retained. An explicitly authorized,
private task-only `/var/tmp` fixture directory outside Git enabled the independent
passing retry; it was empty and removed afterward. No marker was deleted, guard
relaxed, or Cargo build/cache/temp moved outside HOME.

## Two short independent Dev-VM invocations

The immutable `6d9efa7` executable ran under an exclusive disposable Dev-VM
lease in two separate private stages with a fresh strict baseline each:
**4 passed, 0 failed, 0 ignored** in 13.84s and 16.58s. Both guards passed all
canonical categories and all IPv4/IPv6 address/route/rule non-timer fields. Only
numeric, individually non-increasing address valid/preferred lifetimes were
accepted, at the exact address-entry paths. No lifetime increase was waived.

Installed service active/running and MainPID 86349, executable, parent network
namespace, installed private-file fingerprints, core/TUN inventories, resolver
and resolv.conf were unchanged. No real core, sudo, package/service/config,
installed private-profile, DNS or network mutation was requested or exercised.
Private raw snapshots/logs were archived and their hashes validated on the host
before exact task-owned ELF/guard/log/snapshot staging and empty fixture directories
were removed. The lease was explicitly returned.

Private archive SHA-256 values:

- Run 1: `5399c2e5572dbfedc32d9fe9ac439ce306f019b73a37102a93f07f767e575ec4`.
- Run 2: `06a0ad989370c576a6868063a6a5b2d53dc93cbb677936cd7e28d7bede5e86fa`.

Exact-head CI remains separate. These bounded synthetic owner tests do not close
T4, approve historical restore policy, establish installed System-proof authority,
or authorize merge/release/marketplace publication.
