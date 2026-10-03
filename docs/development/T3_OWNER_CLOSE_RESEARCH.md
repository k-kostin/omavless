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
refresh UX. The three-second discovery/effect I/O budgets are separate; confirmation starts the
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
An exact-session proof-flight reservation begins under that gate before the
durable lease is acquired. Cancellation then drains only those short proof
reservations for at most 50 ms, releasing the gate while waiting. The actual
migration lease is dropped before the flight acknowledgement. No new proof
starts after cancellation. A drain timeout or unrelated external lease produces
honest Busy without a POST; a later explicit Disconnect can succeed. Socket
read/reply waits never hold a proof reservation. Cleanup of both Session and
worker capacity precedes bounded result publication.

## Evidence and remaining gates

The focused actual-owner checkpoint covers same-display reordered selection,
snapshot/prepare/confirm/typed replay, expired authority-independent replay,
post-write lost reply without VPN recovery, urgent real ordinary Disconnect
during pre-write controller/read and post-write reply stalls, original expiry,
ticket replacement, desired/store/config/ownership/pending drift, executable
path replacement, display/token/selection drift, and zero extra POSTs.
Focused tests also cover 128 actual lost-reply effects with retained
Unknown receipts, refusal of a concurrent pending effect and the 129th effect,
old exact replay at saturation, close-ID collision with ordinary/batch/probe/
provider families, ordinary/batch/probe-ID collision with close, and actual
batch/probe/provider/shutdown/recovery hooks cancelling a stalled close before
attempting a contended migration lease.
Additional actual-owner cases cover the reverse provider-reservation collision,
late batch/probe/provider completion refusal without durable-state restoration,
passive bytes/ABI refusing MissingAttestation, confirmation after 3.1 seconds
without extending the original expiry, and same-PID exec to another actual image
permanently revoking the old binding. Deterministic proof-lease barriers cover
successful cancellation drain, bounded Busy then explicit Disconnect retry,
and a real 16-byte partial effect followed by cancellation yielding Unknown with
no resend. The transport's publication barrier admits a successor even while
the cleaned-up prior thread is deliberately still alive; known display/identity
JSON duplicates are rejected before Value collapse.

Host fixtures create only private directories, owned subprocesses and Unix
controllers. They observe real process/executable and sysfs inventories and use
a canonical disabled-TUN config; unrelated host interfaces are neither adopted
nor modified. This is not connected VPN or privileged host acceptance.

The composed-core owner opt-in test requires the exact separately frozen
`3b1da75d3c9fd8440216f9c256c6c59da812faae88debc936f3c72fef9724544`
binary. It runs actual snapshot/prepare/confirmation/detached-worker/typed replay
through the same owner with two private no-TUN/no-DNS loopback tunnels, proving
exactly one terminates while the other echoes. Without the opt-in environment
variable it does not execute and must not be counted as composed-core evidence.

Full Rust/developer/CI and exact frozen composed-core VM receipts belong to the
tested SHA recorded in the PR, not to this evolving text. The VM requires an
explicit exclusive lease. Product adoption still requires reviewed package
attestation/distribution, authorization/expiry wiring and public UI/IPC work.
