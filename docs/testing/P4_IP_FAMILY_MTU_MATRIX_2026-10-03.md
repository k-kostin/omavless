# P4 IP-family / MTU matrix: bounded wire measurements and retained negatives

Two independently invoked corrected matrices executed exact source
`ce4850ac1191463cf9aea1e416db59b8dc60c4e3`: each measured **24/24 cells and
96/96 UDP cases**, with its own fresh strict external canonical baseline
preserved. This is bounded developer loopback characterization, not normal
activation, Internet PMTU1500, provider/DNS or broad P4 closure. The older
frozen933 partial runs and strict-baseline NONPASS below remain unchanged.

## Corrected ce4850a execution

The HTTP fixture sets SO_REUSEADDR before the first and every subsequent bind;
new readiness/diagnostic guards are described below. No peer, renderer, Mihomo
or upstream implementation bytes changed. The exact copied binary and inherited
helper hashes are those in the frozen933 artifact table below.

| Corrected execution artifact | SHA-256 |
| --- | --- |
| Frozen source archive | `fcd18c7b439c2123ab011bff87d1ff3f64b63999076ba7801f3d7b534d8d949f` |
| Matrix runner | `d098f72c9d28c29678a939039df44fb51d511aa854de0879594f315472aa3540` |
| Single-invocation canonical guard 1 | `c571d545af75680b54cfd258e3773b94bfa7a741e44bdf42940935df9d020db7` |
| Single-invocation canonical guard 2 | `d5a19e561e0f3372be29de3c1a3c523ccaef78a33e6cb8d75b4d3e1e1b4a00d4` |
| Invocation 1 raw result JSON | `5158d251a2a7ffcf20dfb3eaf9c1d4385153d710411854dd4b4ecad9307db590` |
| Invocation 1 private archive | `535ec9d17b817da44210377f5bd9cfd539424c0c93dbfb10cee613da8a1c054c` |
| Invocation 2 raw result JSON | `b5e9f8585d255ca48eb12990761a1df857e0f55988ad09f3e60c5fa02db950d3` |
| Invocation 2 private archive | `dc29063ba332e5778d24eb01a12ed1e62e2e8129239ecd1e747ef3950b3ec3cf` |

Both reviewed guards retain the identical strict canonical comparator; only
the frozen source/runner pins, new private stage and invocation number differ.
Each invocation independently measured all standard-WG-mode/AWG3/AWG3.1 ×
actual outer IPv4/IPv6 × actual inner IPv4/IPv6 × MTU1280/1420 cells. The
standard-WG-mode row uses the pinned official AWG engine, **not** independent
kernel-WG/wireguard-go interoperability. Actual relay datagram source families
in both directions and decrypted fixed-flow IP/UDP receipts determine families;
configuration strings or parser round trips are not wire evidence.

| Corrected invocation | Cells measured / partial / refused | UDP measured / NONPASS | HTTP / wrong-key / recovery cells | Canonical baseline |
| --- | --- | --- | --- | --- |
| 1 | 24 / 0 / 0 | 96 / 0 | 24 / 24 / 24 | Preserved |
| 2 | 24 / 0 / 0 | 96 / 0 | 24 / 24 / 24 | Preserved |

Each HTTP and recovery receipt proves the exact 65,536-byte body. Each wrong-key
control requires an authenticated literal SOCKS connection attempt for HTTP and
an actual UDP send, outbound relay advance, unchanged authenticated peer/service
counters and no DIRECT path. It does not claim a GET reached the wrong-key peer.
Per invocation, 72 baseline/M−1/M UDP cases had exact unfragmented forward and
reverse payloads. All 24 M+1 cases had matching complete forward fragment
coverage, exact service reassembly and a nonce/hash ACK, followed by **fixture
reverse local EMSGSIZE under explicit PMTUDISC_DO** for the full echo. The
reverse socket policy is a fixture choice, not forward core refusal or native
product MTU enforcement. No oversize-unfragmented or ambiguous timeout case was
promoted to compliance. M denotes total inner IP length; loopback outer MTU is
65536, not an Internet path MTU.

This is 48 measured cell executions and 192 measured UDP cases across the two
corrected invocations, with 48 separately attributed M+1 fragment/refusal cases.
Every external category passed independently: private-file fingerprints,
active/running user service/PID/executable, parent namespace, core/TUN inventory,
resolver/resolv.conf and all IPv4/IPv6 addresses/routes/rules. Only approved
nonincreasing numeric address lifetime countdowns differed. No comparator
relaxation or canonical repair was used. Both owned process groups/namespaces
were reaped and scratch cleanup succeeded. No NONPASS diagnostic archive was
needed. Private archives were copied and rehashed before removing only the new
owned guest stage; old933 evidence remains privately retained. The exclusive VM
lease was explicitly released. Raw private baselines/logs are not in Git.

Source ce4850a gates: 15 focused pure guards PASS; **329 reported / two existing
skips / 327 executed / zero failures**, plus JS/native/QML/navigation and scoped
static checks PASS. Its [x86_64 and ARM64 package CI](https://github.com/k-kostin/omavless/actions/runs/37103364103)
passed. Its [general Test CI](https://github.com/k-kostin/omavless/actions/runs/37103364171/job/111147071347)
**failed** in the unmodified runtime test
`auxiliary_core::tests::admission_is_exclusive_and_quiesce_revokes_before_spawn`
at `auxiliary_core.rs:493` (`Cleanup` unwrap; 691 passed, one failed, seven
ignored). Cause is unestablished; no rerun or runtime fix is represented as PASS.
Frozen933's earlier green CI belongs only to933. These VM results belong only
to the exact frozen ce4850a code/artifacts, not a later documentation head.

## Retained frozen933 partial execution

Two independently invoked matrices executed exact source
`933e4028427a21f6678923c31e0c700dfd4490c1` from Draft #574. This is developer
synthetic private-user+network-namespace characterization under the
[matrix contract](P4_IP_FAMILY_MTU_MATRIX_PLAN.md), not normal activation,
Internet PMTU1500, provider/DNS or broad P4 closure.
Both matrices are **PARTIAL-NONPASS**, not 24-cell wire PASS.

## Exact executed artifacts

| Artifact | SHA-256 |
| --- | --- |
| Frozen source archive | `fa235c6a8647a661953eabf783a64f09a9a8eb6a9ac2a73e47eddcdca0ceea92` |
| Matrix runner | `cf6086e7324ffb1c1f4e51a8aa379061075bdc2490c1bc88df8197fde336e825` |
| Existing WG helper | `71edf75dfa38a0de94f0ade79977e84c0dc42bbea7642a4e4f050fba583fa625` |
| Existing AWG helper | `3881d327d4ed306dc3656dc2b3237c8db62601b6b1ecf6098e20dcaac41e2486` |
| Copied unchanged Mihomo | `ba7a74ed6bbc3098930e6e172fa9b7d4ef7e42a77f7a516046d61fbeb3bfcfe6` |
| Developer peer | `5aad17e961b7127c01f00d4d6cd0e545351bb3b15a9a944c6e0606a380c43603` |
| Developer Rust renderer | `34b300639ce04b5d2e76f36361af5c2650142b2e8ca4a7c8e8aefeff41c9cd66` |
| Single-invocation canonical guard 1 | `891154fa1b043901aa70254bfa6fb172f24bb63d3e2887090b2f2db6e832a8b3` |
| Single-invocation canonical guard 2 | `2e83362173ccccdc536c11f6ab1e24e20662c2eba3845c42131f88397acebfb7` |

Guards were mechanical single-iteration copies of the reviewed guard
`deca3bd48e8871ba3a667a3fbc0751de21a9066f8407590c3c43c05e003b8fd4`,
so the complete strict canonical comparison ran separately around each matrix.
The exact official pinned AWG engine and offline source/build receipts retain
the provenance in the plan. The plain WG row means **official AWG engine in
standard-WG mode**, not independent kernel-WG/wireguard-go interoperability.

## Actual counts and limits

| Frozen invocation | Measured cells | Refused cells | Measured UDP cases | Canonical host guard |
| --- | --- | --- | --- | --- |
| 1 | 2 / 24 | 22 `child_readiness` | 8 | Preserved, approved address countdown only |
| 2 | 2 / 24 | 22 `child_readiness` | 8 | **NONPASS: address lifetime increases** |

Both runs measured only these geometries, with the identical artifact set:

- standard-WG mode, actual outer IPv4, actual inner IPv4, inner MTU1280;
- standard-WG mode, actual outer IPv4, actual inner IPv6, inner MTU1280.

Each of those two cells in each invocation had exact 65,536-byte HTTP body and
recovery, authenticated handshake/bidirectional counter advance, actual
outer-family receipts, wrong-key/no-DIRECT negative attempted-request evidence,
and baseline/M−1/M/M+1 UDP outcomes. Baseline/M−1/M were exact unfragmented
ACK+echo. M+1 forward delivery had matching complete fragment coverage and
service reassembly receipt (IPv4 and IPv6 respectively); the small ACK arrived,
while full reverse echo produced **fixture-kernel local EMSGSIZE under explicit
PMTUDISC_DO**, not forward core refusal or native product MTU compliance.

Across the two runs this is four measured cell executions and 16 measured UDP
cases, including four M+1 forward-fragment/reverse-fixture-refusal cases.
All other geometries are refused/unestablished: no AWG3/3.1, outer IPv6 or
MTU1420 wire PASS is inferred from configuration/CPU results.

The 22 refusals in each run occur after the first successful use of each inner
address family. The frozen service binds the same private HTTP address/8089
without SO_REUSEADDR in a namespace shared across cells. This is consistent with
TCP TIME_WAIT preventing repeated binds; Python's
[socket documentation](https://docs.python.org/3/library/socket.html#socket.create_server)
documents reuse for this situation. **This is a source-backed hypothesis, not
an observed errno from those old runs.** The old runner deleted ephemeral raw
child logs on cleanup and reported only generic `child_readiness`, so exact
refusal attribution cannot be reconstructed or silently changed to PASS.

## Strict canonical failure and stop

Invocation 2's saved before/after comparison differs only at four numeric
address lifetime fields. One entry counts down from 25232 to 25209 as permitted;
the other entry's valid lifetime increases **86231→86400** and preferred lifetime
increases **14231→14400**. The increasing entry retains IPv6/dynamic/site /64
metadata. Address values, object/key/list ordering and all IPv4/IPv6 routes/rules
remain unchanged. Private file fingerprints, user service state/PID/executable,
parent namespace, core/TUN inventory and resolver/resolv.conf comparisons pass.

The increase correctly violates the **nonincreasing-only** policy. No comparator
field is removed or broadened, and invocation 2 does **not** get canonical PASS.
Address lease/router refresh is plausible but **cause is unestablished**; the
systemd-networkd journal has no entries in the captured 06:20:35–06:20:58 UTC
window. No service/configuration/network repair or further VM mutation followed
the refusal. Byte-exact existing receipts/snapshots were copied read-only into
private host evidence, the guest stage was retained privately, and the exclusive
VM lease was explicitly released to its owner.

Private evidence is not uploaded to Git:

| Retained private receipt | SHA-256 |
| --- | --- |
| Invocation 1 raw result JSON | `d60706757143260ebc8b4f39fac102871f1b2715194d59f0980171ef7edfe63c` |
| Invocation 1 archive | `1f54749502ef81f92a6b60b308aa06cd5c8f586b54fce6cf219f450e9190530e` |
| Invocation 2 raw result JSON | `69582ffa4b1cd311d0fff363f875762407a9dc74afa374479ccdbe51e71df198` |
| Invocation 2 archive | `253fd7c983b5623a228b0bb87f325cceadf9ddec73326219b2da994ea1236bc9` |

## Fixture follow-up (CPU gates before corrected execution)

The subsequent test-only runner change sets HTTP SO_REUSEADDR **before bind**
for the first and every later cell. It distinguishes peer/service/core readiness
and emits a fixed HTTP bind-in-use class when that errno is directly observed.
A model-only repeated-bind regression checks the option ordering and safe fixed
failure class; it is not a live kernel or transport counterexample.

On NONPASS, after owned-group reap and namespace-process checks, new bounded
private diagnostics retain only approved per-cell `.log` files in a private
0600 HOME archive. Symlinks/non-private parents/hardlinks/special/oversized files
refuse; logs are capped at 2MiB each, 512 files and 64MiB total. Configuration and
synthetic key files are excluded, and public metadata contains only archive
leaf/hash/count/bytes. This prevents a new refusal losing its diagnostic bytes,
without sharing raw private core logs or weakening the strict canonical guard.
No new peer/renderer/core binary or upstream implementation change is needed.
The corrected ce4850a executions above followed a new frozen source/guard review
and separate explicit VM lease with a freshly checked strict baseline for each
invocation. Old 933 results remain partial and invocation 2's baseline remains
NONPASS; success after the source fixture fix does not establish the old errno.
