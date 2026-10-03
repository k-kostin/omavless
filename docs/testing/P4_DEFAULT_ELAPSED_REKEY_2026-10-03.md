# P4 default elapsed data-triggered rekey source-engine gate

Developer-only continuation from sealed #585
`a29f1d1ad34bf2361f9f302c49f9fc1b7b384b66`. This new case is **not yet
executed**. Compilation and ordinary receipt/source guards are not elapsed
engine evidence. The earlier [default retry result](P4_DEFAULT_ELAPSED_RETRY_2026-10-03.md)
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

No VM, namespace, network/TUN FD, installed profile, private key input, primary
network change, Rust runtime change or normal P4 activation is in scope. There
is no whole P4 closure, performance/stress, default key expiry, retry exhaustion,
Mihomo embedded-engine or installed product acceptance claim. Main/RC merges,
release and marketplace publication remain unauthorized. Exact compiled and
executed source/artifact hashes and outcomes belong in subsequent reviewed
evidence; this plan supplies none by inference.
