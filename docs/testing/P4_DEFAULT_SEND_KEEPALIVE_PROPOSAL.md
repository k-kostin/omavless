# P4 ordinary real default send-keepalive proposal

SOURCE only, from docs-only #744 checkpoint
`89c74e6e07331bd428315e46ccd094f323c8493c`. The accepted rejection/exhaustion
rows belong to exact [e674 ordinary evidence](P4_DEFAULT_ORDINARY_V4_2026-10-13.md),
not this new source. No new Go compiler, test ELF, keepalive engine case, VM,
OS socket/TUN/service, provider/profile or normal P4 activation is selected.
The sole actual Go-tool operation by the writer was explicitly authorized
source formatting of this new public overlay with fixed `/usr/bin/gofmt`,
private telemetry OFF/config/cache and a 10-second command bound. It returned
known zero, changed source formatting only and created no telemetry data.

## Concrete supported row

One ordinary selector `TestP4DefaultSendKeepalive`, one fresh BUILD then
separately selected one-shot EXECUTE. Body including both device closures <=30s,
Go emergency timeout 40s, original supervisor 50s. The
[new overlay](../../tests/fixtures/p4_awg_peer/upstream-tests/default_send_keepalive_test.go)
is add-only on official AWG `b5928efb6ca19f0153958460c3d141f04abc5c2e`,
archive `716c0eec8a7557485555397f1217e0cac56317af8b41fefc287027d0c74ce00d`.
Old support, residue cases, supervisor, export helper, tool admission, Go1.27
frame adapter and ordinary runner remain byte-identical, independently pinned
inputs. The old residue test functions compile as support but are never selected.
There is no timer/callback/key-age/attempt override, direct SendKeepalive call,
original replay-window ValidateCounter call, unsafe/reflection or live network.

| Phase | Actual behavior / bound |
| --- | --- |
| Configured baseline | Two devices/one peer each; real configured Read readiness, fresh silent synthetic identities, actual handshake and exact nonce-bearing bidirectional IPv4 bytes; <=5s setup |
| Real arms | Original client idle-loss arm observed then cancelled by authentic responder data; responder's15s idle-loss arm and client's10s send-keepalive arm observed; sender-authenticated send cancels responder's pending keepalive |
| Genuine default expiry | No subsequent TUN input or callback injection; actual protected zero-plaintext client transport at Bind Send must occur10–13s after the original response's ReceiveFunc delivery |
| Actual consumption | Original receiver index/counter correlate with one ReceiveFunc delivery and next entry; exact existing receiver keepalive log occurs only after real AEAD/replay checks; <=2s, responder idle-loss now cancelled |
| Corrupt-tag puncture | One original ciphertext clone with one tag bit flipped; independent pure AEAD Open refuses; actual channel/ReceiveFunc consumes that exact same index/counter second delivery; <=1s, no valid receiver event/timer refresh inferred |
| Quiet and closure | Full1.5s quiet window without fake-TUN data, extra keepalive or H1; original indexed current keys retained; both actual closes share <=5s and finish within30s; final counts rechecked after closure |

The small Bind wrapper and logger wrapper are installed before NewDevice starts
workers. They are not replaced afterward. Independent AEAD Open retains the
actual receiver keypair alias under its real keypair RLock and reads immutable
cipher state only; it does not mutate the replay filter or retain plaintext/key
bytes. Original protected packets for the bounded puncture remain silent memory.
Logger observation recognizes one fixed existing receiving-keepalive literal
without inspecting/formatting its peer argument. Delivery alone, log absence,
empty fake-TUN output or a zero-length malformed frame is not the positive
receipt: exact authentication, unchanged key/index, correlated actual delivery,
post-replay receive event and real pending→cancelled idle state are required.

The source bounds are inherited from the accepted configured fixture: NumCPU1..32
before construction, <=222 fixture/engine goroutines plus Go runtime/background,
fixed channel/Read/event/packet capacities, two device/close owners. Additional
wrappers add no goroutine/FD/OS networking owner; silent empty-transport and
keepalive-log histories each cap128. Engine pool allocation has no intrinsic
heap cap. On failure the existing one-close-attempt and supervisor unknown-owner
rules apply; no successful body/closure or physical secure-erasure result is
inferred. Scheduled10s duration and observed emission/consumption timings are
separate fields; scheduler overshoot is bounded NONPASS.

## Fixed executable recipe and finite results

The [new fixed ordinary entry](../../tests/fixtures/p4_awg_peer/run_default_send_keepalive.py)
is a narrow successor of the accepted ordinary runner, not an extensible
registry. Four exact add-only overlays and one new
[pure keepalive parser](../../tests/fixtures/p4_awg_peer/default_keepalive_receipt.py)
are bound with the unchanged graph; all 11 inputs must be committed and match
the selected fixture HEAD. Only case=keepalive/ordinary CGO0 is admitted.

GOROOT=/usr/lib/go and fixed Go symlink/signed Arch package/full lexical compiler
and stdlib/ELF provenance checks, independently reviewed offline modules, owned
0700 HOME roots, private telemetry OFF, NPROC<=2048 preserving stricter soft/all
hard limits, other resource bounds and no cleanup path remain as in #744.
Module verification30s and explicit zero-case cmd/test2json compilation120s
precede device compilation120s. The latter's fixed tags are
`p4_cookie_overlay,p4_default_residue_overlay,p4_default_keepalive_overlay`.
Both physical private static ELFs/hashes and all source/Go/telemetry state are
rechecked around the single frozen-helper execution. No lazy compiler, race,
caller-selected environment/tool/command, timing or diagnostic override exists.

The original exclusive attempt precedes child creation. Complete known original
status/captures are recorded before status/format validation with non-authority
flags; an exception/unknown never produces a completed-command result. Raw
stdout/stderr/result bytes are checked before final receipt. Exact native
Go1.27 frame fields normalize into the closed eight-event grammar. The parser
requires one selected case/package PASS, exact causal flags, scheduled10s,
emission10–13s, consumption<=receive<=2s, full quiet1.5–3s, teardown<=5s and
body<=30s including the complete causal chain and both closures. Unknown,
missing, duplicate, early/late/default-overridden or inconsistent claims refuse.

ROOT must review the immutable candidate, changed fixture/parser/runner and
complete concrete private environment/resource recipe before any compile.
Then ROOT selects one fresh BUILD with zero engine cases, inspects the original
known-zero terminal and exact generated ELF/receipt identities, and separately
selects one EXECUTE. No existing failed or accepted artifact is reused or renamed
as this new attempt. First failure stops that snapshot for classification; no
automatic retry or unknown-scope query/signal/reap/cleanup follows.

## Current checks and remaining limits

The [ordinary controls](../../tests/test_p4_default_send_keepalive.py) fabricate
native-shaped keepalive events and mock all Go/export/admission/limit paths.
They exercise strict causal/frame/order/timing/field punctures, preconstruction
pure-observation source guards, exact tags/selector/bounds, pinned inputs and
failed/unknown command refusal. An inert exact-tag check found and corrected
an initially omitted new build tag before any compiler operation. These are
SOURCE checks, not parser-generated engine acceptance.

Race/CGO1/C tool closure, default idle-loss/autonomous recovery as a separate
trigger, shutdown stress, broad wire/header/MTU/IPv6, independent servers,
Mihomo's distinct embedded engine and normal Rust P4 lifecycle/core/privacy
admission remain outside this one row. No whole P4, host/VM, main/beta/release or
Marketplace acceptance is inferred. The source PR records the immutable
candidate, focused/full source results and explicit unrun compiler/engine gates.
