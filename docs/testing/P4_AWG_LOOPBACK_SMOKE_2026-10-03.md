# P4 official-engine AWG 3/3.1 loopback smoke — 2026-10-03

**Scoped transport PASS, not product activation.** Two independent disposable
user/network-namespace invocations in the owner-authorized Omarchy Dev x86_64
VM each passed three rounds of genuine AWG 3 and three of AWG 3.1. Obfuscation
was not disabled or substituted with plain WG. This advances the transport
frontier only; installed-v4 and lifecycle gates remain closed.

## Immutable tested identities

| Artifact | Identity |
| --- | --- |
| Owning base | #548 `9a77207a835765d251753246bc3aad20a5d68798` |
| Actual AWG/WG harness + Go peer source | `cb6ebcc7c6c509d3951d4807b50869da1d441909` |
| AWG harness SHA-256 | `3881d327d4ed306dc3656dc2b3237c8db62601b6b1ecf6098e20dcaac41e2486` |
| Reused WG helper SHA-256 | `71edf75dfa38a0de94f0ade79977e84c0dc42bbea7642a4e4f050fba583fa625` |
| Go peer executable SHA-256 | `3a0bf0103c221eeec617b5b7a33f120ac2c16247404aada6239fa9f7336923ef` |
| Go wrapper source SHA-256 | `884ac2e0999cb57d653de914b9954c6d26bbcbd28f6beac2a60fdf6a61098fa8` |
| Go fixture go.mod / go.sum SHA-256 | `9ba5fc8af1f3763a91907b1e722d78faf53580f190dc759ea6fbd6ce9df344d7` / `b6e8c738a3eec160d0cc58d10ea6dbbf17886b122ed140a379bd9a29eec8df82` |
| Official peer engine source/tag | `b5928efb6ca19f0153958460c3d141f04abc5c2e` / `v3.1.20260828` |
| Official peer engine Go module checksum | `h1:D8d8gGvwXcTxUIsE4z6F6vjy4/VZddu95vMNtOygh1c=` |
| Downloaded pinned engine module ZIP SHA-256 | `14f378d365a612a60aa4352e257d06a02a0091023fd47cb0b4fa7a3e27a174dd` |
| Rust renderer executable SHA-256 | `8e79eb273ed0d08a245e7b5abc05afcd471e97e614c913a63e73730bc2a66dea` |
| Actual tested core SHA-256 | `ba7a74ed6bbc3098930e6e172fa9b7d4ef7e42a77f7a516046d61fbeb3bfcfe6` |

The Rust renderer was built from the example/Rust/lock inputs at
`8db23fe54adf13fd63a174a31a284c3762e488b4`, unchanged at the tested head.
Peer/harness bytes were unchanged by the later evidence-only follow-up.
Final-head ordinary/CI results belong to the owning Draft; a later head does
not retest different transport bytes by implication.

The official source was fetched into private HOME development scratch and
checked out detached at the full immutable SHA, not executed from a moving
installer. Cached module `device`, `conn`, `tun`, `rwcancel` and `ipc` trees
matched that checkout; `go mod verify` passed. The engine was **not patched**.
The [fixture notice](../../tests/fixtures/p4_awg_peer/NOTICE.md) retains the full
upstream MIT attribution and Go dependency BSD notices, and was staged with the
binary. `go version -m` attested official `amneziawg-go/v3 v3.1.20260828`,
`x/crypto v0.42.0`, `x/net v0.44.0`, `x/sys v0.36.0` and their locked checksums.

Peer build: `go1.27.0-X:nodwarf5`, linux/amd64, GOAMD64=v1,
`CGO_ENABLED=0 GOTOOLCHAIN=local`, `-trimpath -mod=readonly -buildvcs=false`.
Rust: `rustc 1.98.1 (48a229cea 2026-09-01)`, locked debug example build.
Targets, compiler cache and temporary build scratch were HOME-only; the Go
cache was approximately 155 MiB. An exclusively handed-off existing Cargo
target was reused, then explicitly released when all builds finished.

Guest observed tooling: kernel `7.2.5-3-omarchy`, Python 3.14.7,
iproute2 7.2.0, util-linux 2.42.3, curl 8.22.0,
wireguard-tools `1.0.20260223-1`. The existing core was
`mihomo-bin 1.19.31-1`, v1.19.31 linux amd64/go1.26.8/`with_gvisor`.
Package metadata is not an attestation of that core's exact upstream source;
this test is evidence for its exact binary, not a source-build/core-release
review or proof of the earliest/minimum supporting release.

## Actual protocol and negative evidence

The [fixture contract](P4_AWG_LOOPBACK_SMOKE_PLAN.md) describes the isolation,
private keys/UAPI and retained-namespace-descriptor guards. Each round used a
new official-engine peer, nonpersistent namespace-local TUN, separate fixed
HTTP fixture and fresh privately generated synthetic keys. The core's TUN and
DNS stayed disabled; the private selector contained only the AWG outbound.
Peer/core/HTTP children had NoNewPrivs=1 and zero effective/permitted/ambient
capabilities. The peer additionally checked its inherited capability and
typed-NSFS conditions itself before running the engine.

All nondefault fields were read back active from the actual peer's private UAPI.
The relay independently observed nonzero junk, all five distinct custom
signature packets (mask 31), and encrypted non-WG initiation/response/transport
headers at the configured prefix lengths. Accepted plaintext length and UDP
transport length histograms supplied bounded matches for the 37-byte content
padding. These are length correlations, not per-packet cryptographic attribution
or exhaustive MTU proof. Actual encrypted HTTP, fresh handshake/RX/TX and a
valid-but-wrong header-key refusal independently rule out mere syntax acceptance
or disabled header protection. The 3.1 flag additionally caused observed extended
handshake datagrams; generation 3 had none. A live same-core request after three
seconds advanced the handshake timestamp under `RekeyAfterTime=2`.

| Check across both generations | Invocation 1 | Invocation 2 |
| --- | --- | --- |
| Initial encrypted fixed-body HTTP + fresh handshake/transfer | 6 PASS | 6 PASS |
| Valid wrong client key: request fails/empty, fresh peer counters unchanged | 6 PASS | 6 PASS |
| Valid wrong header key through native/store renderer: same refusal proof | 6 PASS | 6 PASS |
| Correct key restored: encrypted HTTP/handshake/transfer | 6 PASS | 6 PASS |
| Same-core custom rekey, two successful HTTP requests per case | 6 PASS | 6 PASS |
| Private native/store/export/reimport config equivalence | 18 PASS | 18 PASS |
| Owned process/interface/namespace/scratch cleanup + outside state unchanged | PASS | PASS |

Aggregate: **48 successful encrypted HTTP requests, 24 negative refusals,
12 recoveries, 12 observed rekeys and 36 private roundtrips**. Every negative
began with a reset peer/session/timer state and zero counters; no autonomous
traffic from an old authenticated peer was used as evidence.

All twelve full-run rounds observed at least 48 junk packets and all signature
types; protected-header counts were 67–72 and content-padding size matches
20–23. Generation 3 handshake-trailer counts were zero throughout; generation
3.1 counted 14–16 extended handshake datagrams per round. Counts include bounded
retries/rekeys/negative traffic, not a throughput or wire-efficiency benchmark.
Released namespace inode numbers were reused between invocations; held outside
FDs, not globally unique/monotonic numeric inodes, establish isolation.

The changed helper's default plain-WG path was also actually rerun against the
same committed helper/renderer/core: 3 positives, 3 wrong-client refusals,
3 recoveries, 6 private roundtrips, cleanup and unchanged outside state PASS.
That is a regression check; it does not replace the earlier #548 exact evidence.

## Preserved failures, cleanup and ordinary gates

Earlier development probes were not counted as PASS. One malformed source-ID
argument refused before effects. The initial wrapper incorrectly treated
Linux NativeTun.Write's byte count as a packet count and panicked in the fixture
observer; the upstream engine was not changed to compensate. The corrected
wrapper preserves the returned count and observes only successful input packet
batches; an ordinary fake-TUN regression reproduces the byte-count contract.
One intermediate-source transport probe subsequently hit its positive deadline;
that failure is not retroactively relabelled PASS or attributed to the server/
core without evidence. The later diagnostic round and both exact committed
full suites passed. This small sample is not a claim of universal timing or
performance robustness. Refused probes cleaned their owned scratch and checked
unchanged outside state; no raw peer/core logs or key material was published.

For this AWG task **no** package/module installation, service reload,
sysctl/network/security/config change or private installed profile access was
performed. Stock key-generation tools already existed from the separately
authorized earlier WG preparation. Each namespace closed its owned nonpersistent
TUN FDs, proved only loopback remained, stopped/reaped exact child handles and
proved no remaining namespace process. Private key/config/log/binary scratch
was removed and both empty cache roots removed. Public-only staging (including
notice and bytecode) was recoverably trashed. No fixture peer/core process
remained; the installed native service stayed active/running at MainPID `86349`
and installed core file capabilities remained absent. VM ownership was explicitly
released before further host-only documentation/PR work. Cleanup proves paths
and processes, not secure erasure or absence of recovery snapshots.

Local ordinary gates: Rust workspace **1,055 passed / 0 failed / 12 existing
ignored**, formatting, strict all-target clippy and parity PASS; source suite
**294 tests / 2 existing skips** plus native/QML contracts PASS. Nine new AWG
guard tests are registered in normal `tests/run.sh`, without executing any real
namespace/network/installed action. Three Go fixture tests, `go vet`,
`go mod verify` and twenty race-enabled repetitions of the Go tests passed.
The Go fixture is a standalone opt-in developer module, not a new normal
application dependency or a claim that normal CI runs actual AWG transport.

## Deliberately unclosed gates

The exact pin's open upstream
[wide-header/RandomTrailers issue](https://github.com/amnezia-vpn/amneziawg-go/issues/186)
and [handshake-padding precedence issue](https://github.com/amnezia-vpn/amneziawg-go/issues/185)
remain relevant. This uses small traffic and singleton nondefault headers;
wide ranges, arbitrary MTUs and performance are not accepted. S3/H3 cookie
traffic and DisableCookies under load were configured/read back, not exercised
as a cookie challenge. Timer expiry/exhaustion permutations are not accepted.

P4 remains unavailable in normal product paths. The minimum-core/flavor matrix
must still refuse unsupported inputs **before quiescing** an active target.
Already-v4 host/startup/pointer wiring, active replace/delete compensation,
canonical restart/adoption, installed migration/old-reader rollback and all
private editor/export/backup/subscription consumers need their own acceptance.
Matching real servers, IPv6, Full/Routing/Direct, reconnect/autoconnect/update/
remove rollback, DNS/provider and core/privacy review remain mandatory. R6,
AUTO-1 and V0 are unchanged. The prior security scan remains preserved/incomplete;
no completion attempt or full-audit claim is made. No merge, release or
marketplace publication occurred or is authorized by this checkpoint.
