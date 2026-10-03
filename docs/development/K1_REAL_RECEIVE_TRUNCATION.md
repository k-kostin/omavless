# K1 actual effect-reply receive truncation

This test-only slice starts at sealed creator/private-writer research
[#560](https://github.com/k-kostin/omavless/pull/560), exact source
`fc10fb39aa2ceb25bce251fa63ba55026ce88f33`. It preserves that branch and its
[evidence](K1_CREATOR_LIFECYCLE_RESEARCH.md). No production EffectPort,
namespace authority, service, installed path or dependency is added.

## Fault and barrier

The private `cfg(test)` one-shot fault binds the existing retained creator
socket. After its actual fixed batch send, the next successful real `recvmsg`
uses a **one-byte iovec**. EAGAIN does not consume the fault. The receive must
actually return one copied byte and `MSG_TRUNC`; otherwise the scenario fails.
No input MSG_TRUNC/PEEK flag, invented errno, synthetic ACK, altered sender or
fabricated receive flags are used. The exact actual byte, sender and flags then
reach the existing ACK collector, which refuses and permanently poisons.

Create, atomic full-policy replacement and same-inventory generation-conditioned
delete each get a fresh loopback-only disposable namespace. The ordinary delete
`consume()` interface remains unchanged; a private test-only receiver path
supplies this fault. There is exactly one additional attempted mutation in each
scenario, no resend, generation refresh, compensation or socket-reopened effect.
The original socket descriptor/inode/port and namespace remain retained through
the fault; the creator loses live causality and its session is poisoned.

Real LockedState persistence remains PendingCreate, PendingReplace retaining
the old handle, or PendingDelete retaining the old handle and durable Closed
generation respectively. Repeated Status/Arm/Disarm on the retained writer,
reopened private writer with the same creator, and then reopened writer with a
new creator all refuse with zero further effects and unchanged record bytes.
An independent **read-only** socket observes actual policy presence (create/
replace) or absence (delete). It verifies full rules and changed replacement
handle, but neither presence nor absence repairs a receipt or authorizes retry.

This is actual **receive truncation after send**, not raw kernel ACK dropping,
loss specifically of END, timeout, network packet loss, or #560's separate
post-ACK/readback adapter-result loss. In particular, no valid commit ACK is
inferred from independent readback. Pure tests separately require permanent
collector refusal for MSG_TRUNC, MSG_CTRUNC and both flags for all three effects.

## Gates

The opt-in gate is not run until an exclusive development-VM lease and frozen
artifact are verified. Ordinary parser checks are not actual kernel evidence.

```sh
OMAVLESS_K1_RECV_TRUNC_VM=1 frozen-netguard-tests --ignored --exact \
  kernel_observer::creator_lifecycle::tests::real_receive_truncation_in_disposable_vm \
  --nocapture
```

## Exact-source evidence, 2026-10-03

Immutable tested code:
`f9c919b302dc19dc9dab0fbad71d287e749562e3`. Frozen test executable SHA-256:
`2d83dea76efb5bf5d681d5cf41f2a4b79c63d7643e89d1bf6c73c96986c80f0a`.
It was copied outside Cargo, had no file capabilities, was verified again after
VM transfer, and was not rebuilt during self-reexecution. A separate new home
target/temp directory preserved #560's old artifacts.

Local netguard suite: 197 passed /31 opt-in ignored including doctests; two
focused collector tests passed, covering TRUNC/CTRUNC/both for all three effects.
Strict all-target crate clippy and formatting passed. Source suite: 503 passed
/two existing skips, native/QML checks and 93-link navigation passed. The full
current-RC Rust script passed with one test thread, including strict workspace
and TUI clippy, terminal fixtures and parity. Its repeated result lines total
1583 successful test invocations, zero failures and 43 ignored; this is not a
count of unique tests. All three applicable PR checks on that code head passed:
test, package and package-arm64. #560's separate final documentation Test
attempt initially failed an unchanged two-second core-helper readiness fixture;
its same-head rerun passed, as did its other four checks. That negative history
is retained and does not imply a K1 source fix.

Exclusive Omarchy development VM x86_64/KVM, kernel `7.2.5-3-omarchy`: ten
complete repetitions of all three actual truncation scenarios passed (30 real
MSG_TRUNC fault executions), plus all thirteen predecessor lifecycle scenarios
passed once on the new immutable executable. That predecessor regression
included its three actual SIGKILL cuts; the new receive faults are not SIGKILL
or fabricated transport errors.

All 43 scenarios ran in one private before/after preservation invocation. The
parent network namespace, canonical **user** `omavless-runtime.service`
active/running at PID 86349, runtime executable fingerprint, and private
profile/desired/ownership fingerprints remained equal. Deep JSON comparisons
preserved all interface IDs/names/flags/MTU, address/prefix fields, route tables,
rules and metrics. Only the independently confirmed decreasing numeric
`address[*].addr_info[*].preferred_life_time` and `valid_life_time` countdowns
were excepted; no other key, structure, type, process or route difference was
hidden. No private bytes, hashes or raw network snapshots were published.

Each namespace child was reaped and each fixture directory was empty after its
gate. The exact owned VM scratch, transferred executable/guard and raw private
snapshots were removed. Final no-process/no-scratch and unchanged user-service
PID/parent-namespace checks passed, then the exclusive VM lease was released.
The frozen host executable remains outside Cargo. No primary-PC network effect
or VM parent firewall/route/service/profile mutation occurred. Later
documentation-only changes do not claim execution of modified code.

Synthetic epoch/Canonical fixture vocabulary remains untrusted local history,
not authentication of the canonical host. Reviewed safe namespace API adoption,
trusted launch/no-switch authority, nft continuity, durable orphan adjudication,
installed service/package/runtime integration, power-cut/boot and physical-host
acceptance remain open under [KILL_SWITCH](../roadmap/KILL_SWITCH.md).
No main/RC merge, release, marketplace or upstream submission is implied.
