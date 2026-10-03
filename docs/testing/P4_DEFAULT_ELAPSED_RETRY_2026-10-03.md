# P4 default elapsed source-engine retry and worker cancellation

Developer-only CPU continuation of [#571's timer boundary evidence](P4_AWG_TIMER_SOURCE_2026-10-03.md), based on sealed
`14eeb6fc0207d4fb9c3facccd20d73e61fd1b614`. Actual executed source:
`1bd0efbed515881250e5ebcee99395792c7fecae`. One ordinary invocation and one
independent race-enabled invocation each executed **one case PASS**. This adds
no normal runtime caller, installed profile, transport or VM acceptance.

## Why this is a different gate

#571 checks selected default/range values, directly invokes retry exhaustion,
and lets an actual cleanup Timer expire at a deliberately shortened 1 ms.
Those results remain unchanged. The
[separate #574 matrix](https://github.com/k-kostin/omavless/blob/6e268c2400da595d7109dc18304096956b349541/docs/testing/P4_IP_FAMILY_MTU_MATRIX_2026-10-03.md)
characterizes actual loopback family/MTU traffic, not elapsed default retry
scheduling. Neither earlier result proves the new chain below.

The [new tagged overlay](../../tests/fixtures/p4_awg_peer/upstream-tests/default_elapsed_retry_test.go)
uses the unmodified official AWG engine at
`b5928efb6ca19f0153958460c3d141f04abc5c2e` / `v3.1.20260828`.
Fresh synthetic identities and the existing fake TUN/endpoint support remain
entirely in memory. A separate fake Bind timestamps packet emission **inside
Send**, not when the observing test wakes. There are no socket/real-TUN FDs,
namespaces, unsafe/reflection operations or host network/configuration changes.

1. Actual `SendHandshakeInitiation(false)` emits the first protected H1 and
   arms the official default retransmit Timer. The test withholds this request.
   No timing/range option, constant, Timer implementation, callback or key age
   is changed. The selected default target is 5 seconds plus 0–333 ms jitter.
2. The actual Timer expires and emits a fresh, valid-MAC1 Noise initiation with
   a different local index and retry-attempt count one. Its in-Bind monotonic
   timestamp must be at least 5 seconds and no more than 8 seconds after the
   initial send. Scheduled target and observed elapsed emission are separate
   receipts: scheduler and engine work may delay emission beyond the target.
3. The retry reaches the actual normalized handshake-worker boundary, which
   produces a real response. A full-size copy has one Noise Empty authenticator
   bit flipped, with its outer MAC1 recomputed by the actual server generator.
   The actual client response worker reaches its fixed existing invalid-Noise
   response log event. A silent, format-only logger hook records that stage;
   it retains/emits no arguments. No session is derived, attempts remain one
   and the retry timer remains pending. This is not a fixture length rejection
   or an invalid outer MAC1 masquerading as Noise refusal.
4. The original valid response reaches the same actual worker, derives a real
   indexed session, resets attempts and cancels retransmission. Across a full
   additional 5.5-second window, no H1 may appear. Exactly one worker-generated
   H4 keepalive is classified separately, rather than mistaken for a retry.

The test body has an 18-second deadline, with bounded phase deadlines. The
receipt-only follow-up also requires the actual Go case elapsed field, including
cleanup, to be positive and at most 18 seconds. This is bounded source-engine
behavior, not an exact-latency guarantee under arbitrary scheduling load.

## Exact execution

Toolchain: `/usr/bin/go`, `go1.27.0-X:nodwarf5 linux/amd64`. A clean offline
environment disables inherited Go configuration/workspaces/flags and downloads.
Pinned already-cached dependency bytes/metadata were copied into a separate
owned HOME module cache; no shared cache was written. `go mod verify` passed.
The source checkout stays read-only; the freshly exported official archive is
verified before and after execution. The overlay adds absent files only.

The [new fixed opt-in runner](../../tests/fixtures/p4_awg_peer/run_default_elapsed_overlay.py)
does not change the old helper or runner API. Its local command supervisor drains
bounded output while retaining the unreaped WNOWAIT group leader until pipe EOF
**and** all same-group nonleaders disappear. Setup failure, timeout, excessive
output and closed-stdio orphan controls cannot imply quiescence. Unknown
cancellation retains the anchor/export and forbids another launch or automatic
scratch deletion. Each test binary is compiled once into a fresh private
artifact directory outside the build cache, hashed, executed through test2json,
rehashed and retained. Nothing overwrites a running binary.

| Executed input/artifact | SHA-256 |
| --- | --- |
| Official source archive | `716c0eec8a7557485555397f1217e0cac56317af8b41fefc287027d0c74ce00d` |
| Executed fixture source archive | `025db4aae3656b96b31af4e8e6f92c2778b2c68c0216ee8a1d4d6c3cc360d37f` |
| Executed new runner | `5b0980ba3312055dee1a9f69680678e0dbc1cc696226b2847cb21d24cb1f4dff` |
| Unchanged export helper | `55fa8597b183a5b8deb0b0aabf25740ee12dfc221054e04e62ac017d1b86cc6e` |
| Unchanged in-memory support | `40983d9632f4f92d3e157d25e17c91e5703b53e49aad39f559332d5e423a1e36` |
| Executed Go overlay | `257f4e3b85944cb32cba38127e1a2c87ea920233ee403e74f7153a8555081360` |
| Ordinary frozen test ELF | `0459e8be5d666cc19be1b78e91350a3f4102db57b143403be218390a9d2fde78` |
| Race frozen test ELF | `1c74195728f610b4a6d19a1a971f19d15fd42d748b8515f716f23affe9d6fd39` |
| Ordinary raw Go JSON | `02482309d522e16d7c9788b28cee5423f831217a5f8962995046c94c727c486c` |
| Race raw Go JSON | `dcbb75c2a44d456583290597ffbcf5b8d160aa0876cec4d8cb07cb4afb2304ae` |
| Private exact evidence archive | `065d690a98eba9186e247197fef9ceb6ac1858266dff9798d7d30ab4d45a07c5` |

| Independent execution | Scheduled retry target | Actual monotonic retry emission | Full no-H1 window | Go case elapsed / result |
| --- | --- | --- | --- | --- |
| Ordinary | 5.141 s | 5.145254769 s | 5.504056021 s | 10.65 s / one PASS |
| Race | 5.190 s | 5.195229947 s | 5.504347851 s | 10.71 s / one PASS |

Both actual cases include the full-size malformed-Noise negative and valid
response recovery. Exactly one case PASS and one package PASS, plus one exact
numeric observation receipt, were required independently per invocation.
Frozen ELFs, raw receipts and negative diagnostics remain private. The archive
was created/read back as 0600 inside its 0700 owned parent and byte-compared
against its retained members.

The later runner-only follow-up hash is
`dcec0ed6b5bbde5fc6015f8a66dd1a3255cdfa2f3c2d6b8802d046edacfde165`.
It adds the actual-case elapsed-field bound and preserves cancellation
uncertainty when wait-state inspection fails or an unproven anchor is lost.
**No new Go execution is inferred** for that changed runner. Both retained
original JSON streams pass its receipt-only check, and dedicated mocked
unknown/lost-anchor controls refuse without signals/reaping. Go overlay/support
bytes and the two exact original execution heads/hashes remain unchanged.

## Retained failures and ordinary source gates

Initial source `f2c4fe9` had two prerequisite/build refusals, **zero engine
executions**: the first private copied module cache lacked transitive public
module metadata (offline lookup correctly refused); after copying already-cached
metadata only, compilation found a missing fixture `tun` import. The import-only
fix is the executed `1bd0efb` source. Original stdout/stderr and inputs were
retained, not overwritten or counted PASS. No download, protocol/timer fix,
global cache modification or blind unchanged-head retry was used.

Eleven focused CPU supervisor/receipt guards pass. Final local source suite:
**325 reported, two existing skips, 323 executed, zero failures**, plus
JS/native/QML and documentation-navigation gates PASS. Go formatting and diff
whitespace pass. The ordinary suite only
registers these guards; it never invokes Go or the opt-in engine runner.
Actual Go execution requires explicit clean pinned source, owned HOME scratch,
cache/module-cache and **empty fresh artifact directory**. The runner permits
only `--count 1` or `--count 2`; race execution is separate.

No local Cargo build is represented: Rust/Cargo inputs are unchanged. Final
source/CI results belong to their exact final heads and are recorded in the PR;
the two Go executions above belong only to `1bd0efb` and their recorded bytes.

## Remaining gates

This does not wait the default 120-second data-triggered refresh, default full
retry exhaustion, 10-second keepalive or key rejection/residue expiry. Shutdown
concurrency, performance, autonomous recovery and Mihomo's different embedded
engine remain separate. Normal P4 admission/activation, lifecycle, Full/Routing/
Direct, provider/DNS, independent server interoperability and installed rollback
remain fenced. #574's transport receipts and earlier negatives are unchanged.
No main/RC merge, release or marketplace publication is authorized.
