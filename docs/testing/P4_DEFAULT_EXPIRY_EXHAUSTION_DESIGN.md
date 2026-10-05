# P4 default rejection, residue expiry and retry exhaustion proposal

Source/design only, based on exact #589
`466755d0ddb0ac408e01328816444d80b2b7a523`. No new Go build, test executable,
engine case, VM or native networking has run. The owning
[rekey report](P4_DEFAULT_ELAPSED_REKEY_2026-10-03.md),
[retry report](P4_DEFAULT_ELAPSED_RETRY_2026-10-03.md) and
[timer boundary report](P4_AWG_TIMER_SOURCE_2026-10-03.md) retain their original
executions and failures. This proposal does not close their remaining gates.

## Pinned source facts

Use only official AWG engine `b5928efb6ca19f0153958460c3d141f04abc5c2e`
(`v3.1.20260828`), archive SHA-256
`716c0eec8a7557485555397f1217e0cac56317af8b41fefc287027d0c74ce00d`.
The read-only source checkout matches that revision. An add-only tagged overlay
may reuse the configured channel Bind/TUN support from #589. No implementation
file, timer callback, constant, timing/attempt option or key timestamp changes.
Fresh generated secrets and complete protected/plain packet bytes remain in
silent memory. Python remains developer build/receipt tooling only.

| Official boundary | Source and consequence |
| --- | --- |
| Transport key rejection | `device/receive.go` looks up the header receiver index, then rejects a key whose `created + keychainExpireTime()` is before actual `time.Now()`, before creating an inbound decryption element. Default is180 seconds. |
| Residue destruction | `timersSessionDerived()` arms the real zero-key-material Timer for `3 ×180 =540` seconds. The callback calls `ZeroAndFlushAll()`, removing current/previous/next keys, handshake index/ephemeral state and staged packets. |
| Retransmit | Default is5 seconds plus integer jitter0–333 ms. Initial H1 has attempt count0. At counts0–18, expiry increments and sends; count19 exceeds max18 on the next expiry and exhausts without a21st H1. Thus one chain has20 H1 emissions and20 timer expirations. Nominal exhaustion is100 seconds plus20 jitter samples, not90 seconds. |
| Existing residue timer | Exhaustion arms540 seconds only if `zeroKeyMaterial.IsPending()` is false. A genuinely session-derived pending timer must survive exhaustion without renewal. |

These are source facts, not executed elapsed receipts. Observe scheduled
durations separately from monotonic Send/Receive/callback events; scheduler
delay is a bounded NONPASS. A stored duration540 alone cannot prove an unchanged
absolute timer deadline or actual expiry.

## Case A:180-second rejection followed by540-second destruction

Proposed selector: `TestP4DefaultElapsedRejectAndResidue`. Establish a genuine
channel handshake through actual receive/Noise/AEAD/sequential workers, with
configured Read readiness and exact bidirectional nonce-bearing IP bytes.
Capture the actual keypair objects, indices and key creation epochs privately;
verify the real session-derived pending540-second timers and unchanged default
helpers before the observation window.

Before120 seconds, let the actual sender create two distinct protected
transport packets with different, unused counters and nonce-bearing payloads.
The channel Bind retains their original bytes without delivering them. Complete
an authenticated reverse exchange after observing the actual sender's idle
timer arm so that the15-second idle-loss trigger is cancelled. Retaining a
packet is a channel fixture action, not changing a key's age or timer state.

Deliver the first retained packet at receiver key age177–179 seconds. Its exact
payload must reach the fake TUN under the original index. The receiving peer is
the responder, avoiding its initiator-only receive refresh. Complete a genuine
reverse exchange to cancel the responder's newly armed keepalive timer. The
initiator may then legitimately request rekey because its receive-refresh
threshold is165 seconds; record and withhold all subsequent H1 packets at the
channel boundary. Do not disable, cancel or shorten that real engine behavior.
No new response/session may derive and reset either540-second timer.

Deliver the second retained protected packet at receiver key age181–183 seconds.
It must still name the original retained/indexed key and unused counter. The
actual ReceiveFunc records delivery, then its next entry proves the single-packet
receive loop returned after consuming that packet. Require no corresponding
fake-TUN payload during a full bounded quiet window. A previously captured valid
protected sender packet, early positive receiver control, still-live original
index and real expired key distinguish this from malformed-header, absent-index
or replay rejection. This is an observed source-engine consumption/refusal
chain; there is no new production branch hook or real TUN.

At539 seconds from each key creation epoch, require the same keys and indices
still present and no zero-key callback event. Avoid observations that race the
timer's unguarded duration reset near its firing edge. A silent logger hook may
recognize only the existing fixed zero-key callback format and retain its
monotonic timestamp, never its arguments. Its event occurs before key removal,
so it cannot itself be a cleanup receipt. Separately observe locked key slots,
handshake state/index, captured indices and staged-channel drain after callback.
Require the actual540-second callback duration and removal at age540–550 seconds,
with no session derivation or earlier callback. Do not assert secure physical
erasure of Go cipher memory: upstream explicitly does not provide that guarantee.

## Case B: genuine default exhaustion with an already armed residue timer

Proposed selector: `TestP4DefaultElapsedExhaustionKeepsResidue`. Start with a fresh
genuine channel handshake and exact indexed bidirectional payloads, which arm
the real540-second residue timer. At actual key age181–183 seconds, submit one
genuine nonce-bearing TUN packet. The actual outbound worker must stage it and
request a new handshake through the unchanged expired-key branch of
`SendStagedPackets()`. Record the real staged-container presence and withhold
all H1 requests from the responder after the baseline handshake. No manual
expiry callback, attempt-counter write or directly injected handshake worker
element substitutes for the timer chain.

Submit no further TUN packet during this chain. `SendHandshakeInitiation(false)`
resets the attempt counter before its minimum-time guard, so injecting a second
expired-key packet during retry would change the attempt history even if no
additional H1 were emitted. The single initial packet remains staged until
real exhaustion; this avoids manufacturing queue contents or resetting retries.

Record every H1 inside Bind Send. Its private decoded MAC1/Noise/header shape
and changed local index must validate. Require exactly20 H1 in this new chain
(21 client H1 including the baseline); each of the19 retries must occur at least
5 seconds after its preceding send and within a bounded8-second observation
limit. Inspect the actual rearm duration within that cycle, separately from
emission elapsed time. The silent fixed existing exhaustion logger event must
arrive at least100 seconds after the chain's initial H1 and by130 seconds.
It is an entry marker, not proof that flushing already completed.

Require the single genuinely staged packet to remain present before exhaustion,
then require exhaustion's actual
drain, attempt count19, no pending retransmit/keepalive and no21st H1 throughout
an additional full5.5-second window. Read shared handshake state under its real
mutex and key slots under their actual locks. Do not manufacture queue contents
or call `FlushStagedPackets()` from the test.

The original session keys and latest genuine partial-handshake index must remain
retained after exhaustion. The real prearmed residue timer must still have its
540-second duration. More strongly, observe its actual callback/removal at the
original session age540–550 seconds, rather than540 seconds after exhaustion.
This distinguishes the preserved pending timer from a renewed timer with the
same stored duration. Check the last partial handshake's index/ephemeral state,
all actually populated key slots and staged packet removal; report unpopulated
slots honestly. No unrelated peer or native network is involved.

## Immutable recipe and execution boundary

Both selectors are proposed separately, each with a570-second body/case bound,
600-second Go emergency timeout and620-second whole-command supervisor cap.
The signed-off recipe must define finite per-phase deadlines, event/packet caps,
index/nonce correlation, exact receipt fields and teardown completion. The
longest wait runs inside a supervised test command; the coordinating agent
must remain able to report progress without a blocking tool wait over60 seconds.

Use a new opt-in runner, preserving #589's frozen inputs. Build and execute are
separate phases; the former yields zero engine cases. Pin the exact retained-
graph supervisor from #589 (SHA-256
`00fa64cacdf72d65fcdd772208eca1bc4ca40ca954ef94444ff11eec65b729ec`),
unchanged export helper and all overlay/support bytes. Require clean committed
source, offline verified modules, tool hash and owned0700 HOME scratch/cache/
artifact directories; no `/tmp`, cache download or shared-cache mutation.
Freeze each physical test ELF outside build caches. Bind selector, ordinary/race
mode and exact input hashes in the build receipt, and consume an exclusive
one-shot attempt marker before execution. First failed/uncertain execution stops
that snapshot; do not retry it or close/query/signal/reap a stopped unknown graph.

Exact finite JSON receipts must require one case PASS, one package PASS and one
complete causal observation for the selected case; reject malformed, missing,
extra, duplicate, skipped or failed events and unbounded elapsed fields.
Settled complete output and failed diagnostics remain private. A supervisor
exception is uncertainty, not a complete output or cleanup receipt. Case elapsed
must include teardown, not only the timed body.

ROOT must review the complete new source, immutable graph and concrete recipe
before any Go build or execution. Initially propose one ordinary case per
selector; an independent fresh race artifact is a separate reviewed execution,
never a retry of a failed artifact. Inert Python receipt/source guards and Go
formatting may run before that review. No real sockets/TUN FDs, namespace,
installed profile, Rust/runtime caller, normal P4 activation, main/RC merge,
release or marketplace publication follows from these channel-only cases.
