# P4 default elapsed data-triggered rekey source-engine gate

Developer-only continuation from sealed #585
`a29f1d1ad34bf2361f9f302c49f9fc1b7b384b66`. Original source `5599a5d`
executed one ordinary case PASS; its independent race case was NONPASS at the
initial forward fake-TUN receipt deadline (5.01 seconds). No race causal receipt
or race-detector warning occurred. Both original binaries/events are retained;
the failed attempt was not retried. Configured-read source
`de9752acc972a111eebefdc97b131d3255b08bec` subsequently executed **one separate
read-entry diagnostic PASS, one ordinary rekey case PASS and one independent
race rekey case PASS**, each with fresh artifacts and explicit renewed review.
Compilation and ordinary guards are not elapsed engine evidence. The earlier
[default retry result](P4_DEFAULT_ELAPSED_RETRY_2026-10-03.md)
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
Each reviewed rekey snapshot permits at most one ordinary and one independent
race case; the first failure stops execution for diagnosis, with no blind retry.
The separate diagnostic and corrected snapshot received renewed approval after
the original failure; no original artifact was rerun or overwritten.

## Build, execution and preservation

The [new opt-in runner](../../tests/fixtures/p4_awg_peer/run_default_rekey_overlay.py)
separates `--phase build` (zero engine cases) from `--phase execute`. Ordinary
tests register only pure receipt/source guards and never invoke this runner.
All build, module-cache and temporary paths must be owned HOME subdirectories.
Source export, add-only overlay and offline module verification use the sealed
export helper; the existing runners and their APIs remain unchanged.

The executed runner admitted the exact sealed #585 supervisor bytes by SHA-256
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
seeded or native TUN opened. This diagnostic passed: actual entries were 16 then
64, the pending entry remained 16 after configuration, one empty read occurred
and no Send call occurred. This establishes the worker ordering, not
retrospectively the failed attempt's cause or a fix to normal native
configuration/startup. Subsequent rekey receipts characterize only the
stable configured in-memory fixture, not a normal startup/no-first-packet-loss
guarantee.

## Exact immutable executions and source gates

Toolchain: `/usr/bin/go`, `go1.27.0-X:nodwarf5 linux/amd64`. Inherited Go
configuration/workspace/flags are disabled; module downloads and sum lookups
are disabled. Already-cached dependencies were copied into a separate owned
HOME module cache, and offline `go mod verify` passed before each compilation.
Both source versions add absent test files to a fresh verified official source
export only. None of the official implementation bytes are changed. The
diagnostic and corrected ordinary binaries are reproducible with the same
hash, but occupy separately compiled physical files, each regular/nlink1, with
different selectors and independently bound one-shot artifact receipts.

| Executed input | SHA-256 |
| --- | --- |
| Official engine source archive | `716c0eec8a7557485555397f1217e0cac56317af8b41fefc287027d0c74ce00d` |
| Original 5599 fixture source archive | `4cc19c0c8ec9fba37cf668a194371c86b723918985036dfb2fb9d27b49938aec` |
| Configured de975 fixture source archive | `72403a521631c88262ee7e0b38abba261a6fa4de0d51c7f36400440b9050ca5a` |
| Original runner / Go overlay | `f5564793be1a2b835dac27d7deb8245582f18566e490dde40e6b1bb5706b849c` / `426d2136c5b0e1dee6a41facd4d18c0f7c151a19daa298ed4b037df7f1e374d7` |
| Configured runner / Go overlay | `3c4575357a86b567cf82cf588d42b721488a01c40073747cce055517c4e1aea1` / `6722869be1b098603966eb0df564579789146a15c1340bc121e4ac96c5d2cb6f` |
| Unchanged export helper | `55fa8597b183a5b8deb0b0aabf25740ee12dfc221054e04e62ac017d1b86cc6e` |
| Go toolchain executable | `3144268876ba974458f06ab523581fabac9e967855e2a1bb9de9ba950167b542` |
| All-phase private exact evidence archive | `ff12b307f54ca2d45cf4f29587a029a6246228e54741a4ecb112cb150859eec0` |

| Exact execution | Frozen test ELF SHA-256 | Raw Go JSON SHA-256 | Execution-receipt SHA-256 |
| --- | --- | --- | --- |
| 5599 ordinary: one PASS | `46e17e48d53f24003c3a9422c95429c24f2e2fc2433d4343de559b605c734d56` | `5467183cdf9931e0d32aceee409fa043b91df10f4ff962c4df16c7e7feed1233` | `d393ce46f9ea72c34ec8a83e25abf40618ed256c2e44a4a19b3f11693ed6b8e0` |
| 5599 race: one FAIL / zero PASS | `bf8e447c1afb4ddd2459eb0432edfc9f563fad6ac23721b0a9f62e0f21d14864` | `83d37665d2ea78e7437881f699545f27bb5f29cc453c6fa9d4c0a95078474fb8` | None; failure retained |
| de975 read diagnostic: one PASS | `2b753a086ccee5ea729de8f25a6f666d7ca299dfcb7d46e535b0fed8146c5be7` | `718ba9caaf420bebd3eabbd478b1949bf0c7fd178ad8420823aed3e33394c23f` | `a4ab7abbdf8a7ead7c0a988e0c2faf0bbac1510f105020aa14985f5896171829` |
| de975 configured ordinary: one PASS | `2b753a086ccee5ea729de8f25a6f666d7ca299dfcb7d46e535b0fed8146c5be7` | `c007c5f7e1806a4ccc153dac1cb8ee7c32987abbcedacf973873ab57918febc9` | `a64328e9a03c71e80d8e7e1b334d823a91678c44476a5c9f6a93a71faa3357cb` |
| de975 configured race: one PASS | `6f4a883c133669e5fa621ed4de57524a1fe91857baa63f81f7714e7cb64ad72d` | `6deb92c0dd0d53e52d1879460b274bff33b54b7836fd38c51a1891c40dcb61bd` | `88e4c025abca8dd33ac43320dc5d3e7a7b717677f4dba4326565202360a1c610` |

| Configured execution | Actual pre-threshold data age | Actual trigger-data age | Actual subsequent H1 age | Body / Go case elapsed |
| --- | --- | --- | --- | --- |
| Ordinary | 117.098990364 s | 121.001965177 s | 121.002067358 s | 121.003913689 s / 121.00 s |
| Race | 117.099367988 s | 121.001314418 s | 121.002391320 s | 121.018420774 s / 121.02 s |

Both configured cases require the complete four-phase causal chain, exact
bidirectional nonce-bearing IP bytes under old then new authenticated indices,
client H1=2, server H1=0 and server H2=2. Each independently yields one case
PASS, one package PASS and one exact causal receipt; the separate diagnostic
is not counted as rekey evidence. Original and corrected private artifacts,
raw stdout/stderr, attempt markers and frozen inputs are retained. All stderr
files were empty. Evidence archive mode0600 was explicitly applied/read back
inside its private0700 parent, and archive members were byte-compared.

Nine focused pure guards pass. Source suite at de975: **334 reported, two
existing skips, 332 executed, zero failures**; JS/native/QML/documentation
navigation, Go formatting, Python compile, shell syntax and whitespace gates
pass. No local Cargo build is represented: Rust/Cargo inputs are unchanged.
Final report-head checks belong to their exact later heads and cannot imply
new Go executions, rebuilds or changes to these recorded execution bytes.

### Later supervisor-only uncertainty hardening

Review after the actual executions found a latent uncertainty path in the
sealed supervisor: a first main-loop wait-state error could enter its generic
cancellation path and query wait-state again; Popen's successful final wait
could also treat ECHILD as zero. Neither condition occurred in the recorded
engine cases. Their actual original runner/supervisor bytes remain frozen and
all phase results above remain attributed to them, not to this follow-up.

The first supervisor-only follow-up runner hash is
`f0115ef30587c82c4e6c34d1fd3b1260e54342f500d9b8d007d74889723f734a`.
It pins a new [rekey-only supervisor](../../tests/fixtures/p4_awg_peer/run_default_rekey_supervisor.py),
SHA-256 `1235ba47be848f356beb86af259d2e305bd3e86cc5766f745b0140c2f0cb0c47`;
the old #585 helper and its callers remain unchanged. The new helper latches
the first main-loop wait uncertainty before any subsequent wait/query/signal/
reap, quarantines that anchor and blocks another launch within the supervisor.
After WNOWAIT, EOF and nonleader absence, successful exit code comes only from
an exact raw nonblocking waitpid result for the owned PID and an actual exited/
signaled status. A final-reap exception, wrong PID, missing exited child or
unsupported status also quarantines without a second query, signal or reap.
Bounded cancellation uses the same raw-status discipline; there is no Popen
ECHILD-to-zero fallback. Export-helper bytes and directory/save/inventory bodies
are unchanged, with dedicated source comparison guards.

Deterministic mocked first-wait error followed by a would-be second success,
final-reap OSError/ECHILD, wrong-PID/no-child/invalid-status controls prove no
false PASS, no later ownership operation and no second launch. Existing actual
Python-child timeout/setup/output/orphan controls also exercise the new helper.
**No Go case or binary is rebuilt/reexecuted for this wrapper-only change.**
Recorded successful JSON streams pass the current receipt-only validator;
the original failed race stream remains refused.

Wrapper follow-up source `e6b83b998b329e773b5780ceaeea4b06dfc9b1dc`:
**20 focused guards PASS; 345 suite tests reported, two existing skips, 343
executed, zero failures**, plus JS/native/QML/navigation and formatting/syntax
gates PASS. All local build/execution processes settled before any cache
cleanup; frozen binaries and original failed/successful evidence remain outside
the owned build cache. These source-only checks do not add an engine execution.

### Complete channel graph retention follow-up, 2026-10-05

Further source review found that the supervisor's `finally` block closed its
selector and original stdout/stderr even after a wait/reap uncertainty latched
`UNSETTLED`. Two inert regressions reproduced this on the previous source
(four subcase failures, `ef5809`). Neither bug was observed in the retained Go
engine runs, and those historical successes are not transferred to this fix.

The returned child plus selector now have a process-lifetime retained graph;
the graph is published before selector setup and the selector retained before
subsequent channel operations. A quarantined original causes no selector or
channel close. A close error on the ordinary settled path also seals the scope,
retains the remaining graph and stops before another close or launch. This
does not prove retention of partial resources inside a constructor that raises
before returning, nor hard syscall/allocator cancellation. It does not add a
privileged operation, change engine bytes or revive a stopped invocation.

The successor supervisor SHA256 is
`00fa64cacdf72d65fcdd772208eca1bc4ca40ca954ef94444ff11eec65b729ec`;
runner SHA256 is
`29617c9d5287289206ea824711846f74f31ad8e5c82d13b6f60b93365a72e4fa`.
The runner admits only this new exact supervisor; previous frozen runners and
inputs remain unchanged. Fourteen focused source/receipt/mocked-ownership
controls passed (`acb29b`), including each first wait/reap failure and each
selector/stdout/stderr close failure. The ordinary source suite (346 reported,
two existing skips) plus JS/QML gate passed (`8da729`). No Go build, elapsed
case, native networking or VM
operation was performed for this follow-up; independent review remains needed
before any new slow engine execution.

## Remaining gates

No VM, namespace, network/TUN FD, installed profile, private key input, primary
network change, Rust runtime change or normal P4 activation is in scope. There
is no whole P4 closure, performance/stress, default key expiry, retry exhaustion,
Mihomo embedded-engine or installed product acceptance claim. Main/RC merges,
release and marketplace publication remain unauthorized. Default rejection and
residue expiry, retry exhaustion, isolated shutdown stress, independent server
interoperability and normal P4 Rust activation remain separate gates. No
transport, MTU, IPv6, HTTP or real native startup claim is inferred here.

The separate [default expiry/exhaustion design](P4_DEFAULT_EXPIRY_EXHAUSTION_DESIGN.md)
proposes actual180/540-second source-engine gates from this exact source base.
It is unexecuted design, not additional elapsed evidence or P4 acceptance.
