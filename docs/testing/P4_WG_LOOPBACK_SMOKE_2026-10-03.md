# P4 isolated standard-WG transport smoke — 2026-10-03

**Scoped PASS, not product activation.** Two independent invocations of the
developer harness each created a disposable user/network namespace in the
owner-authorized Omarchy Dev x86_64 VM. Each ran three positive, three wrong-key
negative and three restored-key recovery requests. This supplies an actual
standard-WG outbound/peer transport checkpoint, not Full/Routing/Direct,
AWG/server/provider, installed-v4, startup/autoconnect or lifecycle acceptance.
The physical PC's VPN state was not changed. VM ownership was explicitly
released to the root agent after cleanup; no subsequent guest actions belong
to this checkpoint.

## Immutable tested identities

| Artifact | Tested identity |
| --- | --- |
| P4 domain base | #545 `868bd4db4b8c619545a8b1a80df557304431a649` |
| Actual transport harness source | `819e2f7db982b95792ce3aceaa63206793b0e376` |
| Python harness SHA-256 | `6ed89a307367e63da2ce55ecd08c4a0b5fe2896a3d49a07e84214757773e1ccf` |
| Rust fixture build tree | `712b69d16e30c7625c54c5ca51e59f5f84f217b7` (Rust/example/lock inputs unchanged at the tested harness head) |
| Rust fixture executable SHA-256 | `2618711e0f0df03648bcfe2cba2a5a1092313a451f61449dc84c894352d7ca62` |
| Installed tested core SHA-256 | `ba7a74ed6bbc3098930e6e172fa9b7d4ef7e42a77f7a516046d61fbeb3bfcfe6` |
| Observed core package/version | `mihomo-bin 1.19.31-1`; Mihomo Meta v1.19.31 linux amd64, go1.26.8, `with_gvisor` |
| Guest kernel/tooling | `7.2.5-3-omarchy`; `wireguard-tools 1.0.20260223-1` |

The core's package/version metadata does not prove an exact upstream source
commit or an unchanged upstream distribution. This checkpoint tests the exact
binary above, not source-build provenance, release/advisory review or a
flavor-specific minimum-core/version matrix. Exact final-source CI/checks belong
to the owning Draft; documentation/test-runner follow-ups do not retest a
different harness or core by implication.

## Authorized preparation and preserved installed state

Initial read-only preflight found the stock kernel module file present but
unloaded, no `wg` executable, and working unprivileged user/network namespaces.
The owner explicitly amended the plan to allow stock VM tooling preparation.
Following the Omarchy package workflow in a real SSH terminal installed
`wireguard-tools` (one package, approximately 0.26 MiB), then loaded the stock
kernel WireGuard module. The package hooks reloaded **system-manager
configuration** and armed `ConditionNeedsUpdate`; these are guest-wide actions,
not namespace-local fixture actions. No package/core/runtime update, persistent
network/sysctl/security-policy change or private profile access was performed.
Stock tools/module remain available and were not removed or unloaded as if
they could not be shared.

The installed `omavless-runtime.service` remained active/running at MainPID
`86349` before preparation and after both suites/cleanup. No installed service
Start/Stop/Connect/Disconnect, frontend action or private runtime IPC operation
was invoked. This is preserved observable state, not a claim about unavailable
private desired-state/provider data or deferred host authorization acceptance.

## Actual fixture and proof

The outer ordinary-account supervisor captured outside interface/address/route
facts plus installed runtime/core process facts, retaining only comparison
results. It opened parent user/network namespace descriptors **before** unshare
and passed them into its exact child. The namespace child verified those held
identities, its actual parent PID and different current user/network identities
before effects. Namespace-root privileges never authorize an initial-namespace
network effect. The HTTP child retains the same namespace proof across its
privilege drop, without forbidden parent `/proc` namespace rereads.

Inside the new namespace only, the harness created `wg-p4`, assigned peer
`10.203.0.1/32`, and installed the narrow client return route
`10.203.0.2/32`. A separate bounded HTTP fixture served a fixed synthetic body.
There was no default route, physical/veth interface, forwarding, DNS/provider
lookup or external endpoint. WG's UDP endpoint stayed on namespace loopback.

Every round generated fresh ephemeral server/client/wrong-client keys and a
preshared key through captured private pipes/files. No key value entered argv,
environment, Git, public output or raw shareable logs. Scratch directories were
`0700`; key/native/YAML/log files were created `0600` before payload writes.
The Rust developer fixture consumed only these synthetic files, passing native
import -> strict complete private v4 store -> config preparation, and proving
native export/reimport produced identical private runtime bytes. No installed
store was read, migrated or written.

The tested core was copied to a new plain `0500` inode, checked for the exact
checksum and absent file capabilities. TUN and DNS were disabled, no provider
or geodata option was present, and SOCKS bound namespace loopback. Core and HTTP
children were observed in the expected namespace with NoNewPrivs=1 and effective,
permitted and ambient capabilities zero; request helpers use the same privilege
drop. Core output stayed in bounded private logs, never public diagnostics.
The private Unix controller's peer PID had to equal the owned core child; its
PROXY selector had exactly the one WG outbound and no DIRECT alternative.

| Actual check | Invocation 1 | Invocation 2 |
| --- | --- | --- |
| Fixed HTTP response through WG plus fresh handshake and increased peer RX/TX | 3 PASS | 3 PASS |
| Wrong client key: bounded request fails, empty response, peer handshake/transfer unchanged | 3 PASS | 3 PASS |
| Correct key restored with a fresh core child: response/handshake/transfer prove recovery | 3 PASS | 3 PASS |
| Private native/store/export/reimport configuration equivalence | 6 PASS | 6 PASS |
| Owned child/interface/namespace/scratch cleanup and outside snapshot unchanged | PASS | PASS |

Aggregate: 12 successful encrypted HTTP requests and six wrong-key refusals,
with 12 private roundtrips. The negative control distinguishes actual WG
transport from a hidden direct/fallback request to the locally reachable
fixture. The kernel reused released namespace inode numbers between the two
invocations; numeric inodes are not monotonic/global generation tokens. Each
invocation proves a newly created namespace relative to its retained outside
descriptors and verifies no residual process before deleting scratch.

## Refusals, cleanup and static gates

Earlier harness iterations refused, rather than being counted as PASS: clean
environment initially lacked explicit read-only user-manager addressing;
parent namespace `/proc` dereference was denied after user-namespace/capability
changes. The retained-descriptor corrections fixed the fixture contract, not
the installed runtime or Linux security policy. No failed earlier iteration
is matching-server or core incompatibility evidence. Every refused invocation
removed its scratch and verified the outside snapshot remained unchanged.

Both successful invocations deleted their owned WG interface, stopped/reaped
owned child handles and verified no remaining process in the fixture namespace.
Ephemeral key/config/log/binary scratch was removed; the now-empty cache root
was removed too. Public harness/executable staging was recoverably moved to
trash; it contained no generated credentials. Final process inventory had no
fixture core or Mihomo process, and the installed native PID remained unchanged.
This proves path/process cleanup, not secure erasure, memory zeroization or
absence of filesystem recovery snapshots.

The normal source suite now includes nine synthetic guard tests for opt-in,
missing prerequisites, exclusive private-file creation, binary identity,
fixed-safe errors, exact controller PID/no-DIRECT selection, parent namespace
refusal and dropped child capabilities. They never execute a real namespace,
network, module operation or installed acceptance action. Local full Rust gates
passed 1,055 tests with zero failures and 12 existing ignored; formatting,
strict all-target clippy and parity passed. Final source-suite/CI totals belong
to the owning PR after registering the guard tests in `tests/run.sh`.

## Remaining gates

P4 remains unavailable in the product. Before user exposure, finish explicit
already-v4 host/startup/connection-pointer wiring, shared active replacement/
deletion compensation and canonical restart/adoption proof; review installed
migration/old-reader rollback, all subscription/backup consumers and bounded
authenticated editor/export framing. Precise core/flavor compatibility must
refuse **before quiescing** an active target. Matching real WG/AWG servers,
applicable IPv4/IPv6, Full/Routing/Direct, reconnect/autoconnect/update/remove
rollback and credential-leak/core review remain mandatory. This smoke is
standard WG IPv4 only and does not change AUTO-1, DNS/provider, V0 or R6 results.
The preserved earlier security scan remains incomplete; no completion/retry or
complete-audit claim is supplied. No merge/release/marketplace action is implied.
