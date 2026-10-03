# P4 default elapsed data-triggered rekey source-engine gate

Developer-only continuation from sealed #585
`a29f1d1ad34bf2361f9f302c49f9fc1b7b384b66`. Original source `5599a5d`
executed one ordinary case PASS; its independent race case was NONPASS at the
initial forward fake-TUN receipt deadline (5.01 seconds). No race causal receipt
or race-detector warning occurred. Both original binaries/events are retained;
the failed attempt was not retried. **The configured-read follow-up and its
read-entry diagnostic are not yet executed.** Compilation and ordinary guards
are not elapsed engine evidence. The earlier [default retry result](P4_DEFAULT_ELAPSED_RETRY_2026-10-03.md)
remains unchanged and does not prove the default 120-second refresh below.

## Fixed causal chain

The [add-only tagged Go overlay](../../tests/fixtures/p4_awg_peer/upstream-tests/default_elapsed_rekey_test.go)
uses official AWG engine `b5928efb6ca19f0153958460c3d141f04abc5c2e`
(`v3.1.20260828`), without changing its source, constants, options, callbacks or
key creation timestamps. Fresh synthetic identities, header-protection key and
nonce-bearing IPv4 packet bytes stay in memory. Channel Bind ReceiveFuncs feed
unchanged protected packets into the actual receive, Noise, AEAD/replay and
sequential TUN workers; decrypted exact payloads are correlated with protected
Send timestamps using receiver index and transport counter. This is **not** a
socket, real TUN, independent peer interoperability or network transport test.

Before payload injection, the follow-up requires an actual configured TUN Read
entry at offset 64 (16-byte transport header + S4=48). If the first reader entry
cached the constructor's zero padding, exactly one empty read advances the
unchanged worker to its next iteration; unexpected offsets or missing readiness
refuse. This emits no payload, seeds no handshake and changes no timer/key fact.

1. A genuine worker handshake establishes indexed initiator/responder sessions;
   exact nonce-bearing payloads arrive in both directions. No direct handshake
   consume/session/refresh callback is used by the test.
2. At an observed session age of at least 117 seconds and strictly below 120,
   another bidirectional exchange must retain the original session and produce
   no new H1. The test observes the actual sender's new-handshake timer arm
   before returning the authenticated echo, then proves its cancellation.
   This excludes the distinct 15-second idle-loss trigger as the later cause.
3. After a 121-second scheduling target, actual old-session data Send must
   precede the new H1 at an observed age of at least 120 seconds. The nonce is
   below the message-count threshold, the old session still exists, and no
   idle-loss timer is pending before that injection. The genuine data sender
   invokes the unchanged engine refresh logic; the test never invokes it.
4. Real Noise response and confirmation workers must establish distinct current
   sessions/indices, advance the handshake epoch and carry another exact
   bidirectional payload under the new indices. Exactly two client H1 and two
   server H2, with no server H1/client H2, are required after the final exchange.

Scheduling targets and monotonic observations are separate. Slow scheduling is
a bounded NONPASS, not a fabricated on-time receipt. The test body and actual Go
case elapsed including cleanup must be at most 150 seconds; Go's emergency
timeout is 180 seconds, and the outer command supervisor is bounded at 200.
Maximum authorized actual scope is one ordinary case and one independent race
case; the first failure stops execution for diagnosis, with no blind retry.

## Build, execution and preservation

The [new opt-in runner](../../tests/fixtures/p4_awg_peer/run_default_rekey_overlay.py)
separates `--phase build` (zero engine cases) from `--phase execute`. Ordinary
tests register only pure receipt/source guards and never invoke this runner.
All build, module-cache and temporary paths must be owned HOME subdirectories.
Source export, add-only overlay and offline module verification use the sealed
export helper; the existing runners and their APIs remain unchanged.

The new runner admits the exact sealed #585 supervisor bytes by SHA-256
`dcec0ed6b5bbde5fc6015f8a66dd1a3255cdfa2f3c2d6b8802d046edacfde165`.
It retains a WNOWAIT group anchor until bounded pipe EOF and nonleader absence.
Unknown cancellation quarantines the anchor/export rather than inferring
cleanup. Executable/input objects are hashed, owner/mode/link/xattr checked,
frozen outside build caches and read back before and after execution. This is
trusted fixed-fixture pathname execution, not adversarial retained-FD execution
attestation. An exclusive attempt marker consumes the artifact's one execution
slot before launch, including on failure. Ordinary settled child output is
retained; a supervisor exception records failure/uncertainty but does not claim
that its incomplete live output was retained as a complete receipt.

## Retained initial failure and separate diagnostic

Original ordinary source `5599a5daedd149222c2eb552f4579236f0838339`
observed pre-threshold old-session data at 117.099428587 s, late data at
121.002033792 s and new H1 at 121.002134350 s; the body completed at
121.004038588 s. Its exact post-rekey bidirectional indexed AEAD chain passed.
The independent race attempt failed its baseline before this chain. The private
original archive SHA-256 is
`12fdc8826619685b71d1dec14247e118831df6821ba305a48b5d324707e7bde4`;
its 0600 mode and byte comparison were verified. These are not two PASS.

Official `NewDevice` starts `RoutineReadFromTUN` before returning. That worker
loads padding before calling the blocking TUN Read and later retains that
iteration's padding in the outbound element. The original fixture configures
S4 only after construction. This is a source-backed startup-order hypothesis
for the failed first payload, not recovered instrumentation proving the cause
of that historical attempt.

The separate fixed `--diagnostic` case forces actual initial Read entry at 16
before IpcSet, sets only S4=48, observes that the pending read is still at 16,
returns exactly one zero-length read, and requires the next actual entry at 64
with zero Bind Send calls. It has a five-second body/case bound, ten-second Go
emergency timeout and fifteen-second outer bound. Its own fresh artifact and
one-shot attempt marker cannot be mistaken for the 120-second case; the runner
binds the exact selector and diagnostic flag in each receipt. No handshake is
seeded or native TUN opened. A diagnostic PASS would establish this worker
ordering, not retrospectively prove the failed attempt's cause or fix normal
native configuration/startup. Subsequent rekey receipts characterize only the
stable configured in-memory fixture, not a normal startup/no-first-packet-loss
guarantee.

No VM, namespace, network/TUN FD, installed profile, private key input, primary
network change, Rust runtime change or normal P4 activation is in scope. There
is no whole P4 closure, performance/stress, default key expiry, retry exhaustion,
Mihomo embedded-engine or installed product acceptance claim. Main/RC merges,
release and marketplace publication remain unauthorized. Exact compiled and
executed source/artifact hashes and outcomes belong in subsequent reviewed
evidence; this plan supplies none by inference.
