# P4 IP-family / MTU matrix: partial wire measurements and retained refusals

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

## CPU-only fixture follow-up

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
Any follow-up actual matrix needs a new frozen source/guard review and separate
explicit VM lease with a freshly checked strict baseline. Old 933 results remain
partial and invocation 2's baseline remains NONPASS.
