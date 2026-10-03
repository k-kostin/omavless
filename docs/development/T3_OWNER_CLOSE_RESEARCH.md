# T3: actual-owner detached-close research

This inactive development cut starts from current RC `c4e800425243c1b02165f82153e4bf418fe465e6`.
Commit `3c639cb` selectively ports #559's detached transport and retained child
lifetime. The integration does not replace current-RC native-owner, managed-DNS
package selection, or ManagedPair validation with older research ancestors.

## Boundary

No public IPC method, TUI action, runtime registration, normal conditional-close
permit constructor, matched package attestation, package pin, release, or main/RC
merge is enabled. Passive retained executable/source descriptors and digests
are evidence preparation only. Normal confirmation refuses missing attestation.
Only a fixed `cfg(test)` actual parent-owned subprocess constructor supplies the
internal fixture permit. Interpreter image and fixture script are distinct
retained objects, not a claim that the interpreter is an adopted core package.

## Actual composition

`OfflineNativeCoordinator` captures its actual instance, revision, Rust ownership
fence, desired state, private store/config bytes, recovery/pending/batch state and
parent-owned child session. Blocking discovery, executable hashing and fresh
controller observation run after releasing the owner and migration lease. A
nonreusable Arc binds a detached discovery completion to that exact capture.

Strict snapshot parsing retains ID/token and bounded display from the same row.
Opaque 256-bit handles and confirmation tickets use the fixed checked kernel
random device; entropy is never reused. There is one pending ticket, at most
128 rows, a 1,024-token nonreuse ledger per owner instance, and a five-second
original, nonrenewable confirmation lifetime. Exhausted entropy returns Busy
permanently for that instance; this bounded research limit is not production
refresh UX. The
three-second discovery/effect I/O budgets are separate; confirmation starts the
effect budget without extending the original snapshot expiry.

The common mutation coordinator reserves bounded, non-evicting typed close
receipts separately from its ordinary lifecycle slot. Operation IDs conflict
across ordinary and long-operation families. Exact typed replay precedes fresh
authority/revision checks, including Unknown, and never resends an effect.
Unknown alone preserves VPN desired/mode/profile/ownership and does not latch
whole-VPN manual recovery. A completion advances the canonical revision once
only if its base revision is still current; it never restores older owner state.
The 128-close receipt capacity is intentionally non-evicting: exhaustion refuses
new operations before any effect while retaining exact retries, including
uncertain results. It likewise needs a reviewed product-lifetime design.

Before each nonblocking POST chunk, the worker tries the cooperative migration
lease and rechecks actual durable owner bytes. Then the retained lifetime gate
checks cancellation, child/proc-exe/source/socket identity, original expiry and
I/O deadline, and marks Attempted before one syscall. No lease or gate spans a
readiness wait. Central owner hooks cancel before competing admission/effects,
including urgent Disconnect. No worker calls back into the owner.

## Evidence and remaining gates

The focused actual-owner checkpoint covers same-display reordered selection,
snapshot/prepare/confirm/typed replay, expired authority-independent replay,
post-write lost reply without VPN recovery, urgent real ordinary Disconnect
during pre-write controller/read and post-write reply stalls, original expiry,
ticket replacement, desired/store/config/ownership/pending drift, executable
path replacement, display/token/selection drift, and zero extra POSTs.
Eight focused tests also cover 128 actual lost-reply effects with retained
Unknown receipts, refusal of a concurrent pending effect and the 129th effect,
old exact replay at saturation, close-ID collision with ordinary/batch/probe/
provider families, ordinary/batch/probe-ID collision with close, and actual
batch/probe/provider/shutdown/recovery hooks cancelling a stalled close before
attempting a contended migration lease.

Host fixtures create only private directories, owned subprocesses and Unix
controllers. They observe real process/executable and sysfs inventories and use
a canonical disabled-TUN config; unrelated host interfaces are neither adopted
nor modified. This is not connected VPN or privileged host acceptance.

This is a work-in-progress research checkpoint. Full Rust/developer/CI gates,
provider-reservation-to-close collision and all-family completion regressions,
retained executable same-PID exec drift,
and exact frozen composed-core VM evidence are still pending. The VM requires an
explicit exclusive lease. Product adoption still requires reviewed package
attestation/distribution, authorization/expiry wiring and public UI/IPC work.
