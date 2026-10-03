# P4 synthetic IP-family / MTU characterization fixture

Additive developer-only continuation of #571 (sealed base
`14eeb6fc0207d4fb9c3facccd20d73e61fd1b614`), not a normal runtime or package.
The [WG](P4_WG_LOOPBACK_SMOKE_PLAN.md),
[AWG](P4_AWG_LOOPBACK_SMOKE_PLAN.md) and
[cookie](P4_AWG_COOKIE_TRANSPORT_PLAN.md) transport contracts still own P4.
No existing helper API, shared renderer, upstream engine, installed configuration,
service, default activation or Rust domain/profile API changes are introduced.
See the [partial actual checkpoint](P4_IP_FAMILY_MTU_MATRIX_2026-10-03.md) for
the two frozen 933 invocations: 2/24 measured cells each, remaining refusals,
and second-run strict-baseline NONPASS. Neither closes the planned matrix.

## Matrix and provenance

[Runner](../../tests/p4_ip_family_mtu_matrix.py),
[Rust private-store renderer](../../crates/omavless-domain/examples/p4_matrix_config.rs),
[observed peer](../../tests/fixtures/p4_matrix_peer/main.go) and
[offline builder](../../tests/fixtures/p4_matrix_peer/build.py) are separate
opt-in artifacts. The peer uses official AmneziaWG Go v3.1.20260828 at
`b5928efb6ca19f0153958460c3d141f04abc5c2e`, archive SHA-256
`716c0eec8a7557485555397f1217e0cac56317af8b41fefc287027d0c74ce00d`.
The builder reuses the unchanged verified-export/bounded-child functions, adds
only a new peer directory to a fresh export, verifies offline cached modules and
exact named JSON test counts, and checks source/export bytes before/after.
No upstream patch or module-cache rewrite is needed. Toolchain, input hashes,
binary SHA-256 and `go version -m` are recorded beside each new build output.

The 24 cells are three flavors × two outer families × two inner families × two
inner MTUs. The plain WG row is **the pinned official AWG engine in standard-WG
mode**, not independent kernel-WG or wireguard-go interoperability.
Every row and build receipt carries that engine provenance.

| Dimension | Values |
| --- | --- |
| Flavor | standard-WG mode, AWG3, AWG3.1 |
| Actual encrypted endpoint IP family | IPv4 loopback / IPv6 loopback |
| Actual decrypted payload IP family | IPv4 / IPv6 |
| Configured and read-back inner TUN MTU M | 1280 / 1420 |

Each cell has fresh synthetic keys, native positive/wrong-key configurations,
private-store serialization and native export/reimport rendering equality.
The AWG rows retain the existing non-default J/S/H/I/header-protection/padding/
timer settings and 3.1 trailer/cookie settings, checked through private UAPI.
No real keys, profiles, providers or DNS requests are needed.

## Measurements and refusal policy

Each positive phase checks **all 65,536 HTTP body bytes**, not just status or
length, through the literal-address SOCKS5 path. Handshake and both-direction
authenticated peer counters must advance. The single PROXY selector has no
DIRECT member. Fresh wrong-key peer reset must give no exact HTTP response or
UDP ACK/echo, unchanged authenticated peer counters, and unchanged service
request receipts; recovery must again give all 65,536 HTTP bytes.

UDP requests use a fresh nonce and an exact bounded synthetic payload. Each has
a small SHA-256 ACK followed by an attempted full exact echo. The ACK separates
forward receipt from the possibly oversized return path. Besides a small
baseline, requested **inner IP total lengths** are M−1, M and M+1. For ordinary
UDP/IP with no extensions, application length is IP total−28 for IPv4 or
IP total−48 for IPv6; SOCKS framing and encrypted outer overhead are excluded.

The peer records actual decrypted TUN IP versions, validated declared/actual
lengths, fixed UDP flow ports and addresses, complete UDP payload SHA-256 where
unfragmented, and fragment ID/offset/more/payload length. Per-request before/after
deltas prevent HTTP or earlier UDP packets satisfying a size bucket. Fragment
acceptance requires exactly one matching ID, nonoverlapping contiguous complete
coverage and exact service receipt/reassembly; an IPv6 atomic Fragment header
alone is not fragmentation. No packet bytes or synthetic credentials are emitted.
The relay records actual loopback source family and encrypted datagram lengths
in **both directions**, rather than treating rendered endpoint strings as proof.
Truncated UDP datagrams are refused.

At/below M, exact ACK/echo and matching unfragmented receipts are required.
Above M, categories deliberately remain separate:

- Exact unfragmented M+1 forward delivery is **observed oversize / not MTU
  compliance**, never fragmentation or core-boundary refusal.
- Complete fragment coverage plus exact reassembled service receipt is actual
  fragmentation evidence, not an assertion based on a fragment flag alone.
- The service uses a fresh connected response socket per request with explicit
  `IP*_PMTUDISC_DO`, reads its route MTU, sends the small ACK first and records
  immediate `EMSGSIZE` on the correlated full echo attempt. This proves only
  **fixture-kernel reverse-route refusal under that explicit socket policy**,
  not forward core refusal or native product DF/PMTU policy.
- Missing ACK/receipt, unrelated lengths/hashes, unknown refusal, ambiguous
  timeout or incomplete fragments are **NONPASS**, not expected refusal.

The runner attempts every independent cell once and retains fixed failure
classes and actual measured/refused counts. `MEASURED` is a characterization
result, not blanket MTU enforcement acceptance; mixed results are
`PARTIAL-NONPASS`. No parser/model/CPU result is actual wire evidence.

## Privilege and execution boundary

Only an explicitly leased disposable VM may invoke the live runner with
`--acknowledge-disposable-vm`. A non-root outer supervisor retains original user
and network namespace FDs and launches one fresh user+network namespace.
Before any interface action, the child verifies parent PID, retained-FD inode
anchors, distinct user/net namespaces, private owned scratch, only initial lo,
and empty IPv4/IPv6 route inventories. The namespace owner creates a nonpersistent
`IFF_TUN|IFF_NO_PI` FD and only fixed /32 or /128 addresses/return routes.
There is no VNET_HDR/GSO/GRO, DNS, default route, sysctl, package, module-loading
or installed-service operation. Missing prerequisites refuse, without repair.

Core, renderer, peer, HTTP/UDP service and transport clients run with all
capabilities dropped and no-new-privileges. Peer and service/client also verify
retained namespace anchors internally. New private binary copies have no file
capabilities. Logs/file output and completion deadlines are bounded. The outer
process keeps its process-group leader unreaped until owned-group cleanup, then
verifies no process remains in the captured child net namespace and removes only
its validated owned scratch directory. Existing running binaries are not rebuilt.

The runner's cleanup claim is **not a canonical host preservation comparator**.
An external reviewed guard must preserve private configuration fingerprints,
service state/PID/executable, parent namespace, core/TUN inventory, resolver/
resolv.conf, and all address/routes/rules IPv4/IPv6. Only numeric nonincreasing
`valid_life_time`/`preferred_life_time` at exact address[*].addr_info paths may
differ. No blanket expiry/stat normalization is allowed.

Freeze source/helper/binary hashes and review privilege/measurement guards
before an explicit VM lease. Run two independently invoked matrices with
independent keys/scratch, retaining both real result sets and canonical guards.
Loopback outer MTU is explicitly checked as **65,536**: this is not Internet
PMTU1500, provider, DNS, roaming or installed normal-bridge activation acceptance.

## Source gates so far

- Fifteen pure Python measurement/builder receipt guards pass in ordinary CI;
  `tests/run.sh` only registers these guards, never runs the live matrix.
- Five named observed-peer CPU guards: 50 repetitions = **250 executions PASS**;
  race-enabled 20 repetitions = **100 executions PASS**. Both build receipts
  explicitly say namespace execution is false. Earlier initial four-case
  200-execution build predates flow binding and is not the final peer artifact.
- Rust example: **two tests PASS**, including all 24 geometries × both phases
  = 48 private-store/native-render round trips and fixed-geometry refusal cases.
- Initial source suite: **321 reported, two existing skips, 319 executed, zero failures**;
  JS/native/QML contracts pass. Formatting/whitespace remain scoped static gates.
- Corrected source suite: **329 reported, two existing skips, 327 executed, zero
  failures**, plus JS/native/QML/navigation checks PASS. The linked checkpoint
  retains two older 2/24 partial runs and their strict-baseline NONPASS, then two
  independently invoked frozen ce4850a 24/24 loopback matrices with preserved
  strict baselines. This is bounded characterization, not broad P4, native-owner
  or Internet PMTU acceptance; general ce4850a Test CI remains failed separately.

Pre-VM review found a fixture-only variable overwrite: a renderer subprocess
result replaced the mutable cell record. A separate rendering helper now keeps
both subprocess receipts local, and a pure regression preserves the caller's
record identity/content across both phases. No VM execution or engine/binary
re-execution is inferred from that runner-only fix.
Further pre-VM review tightened negative attribution: local client startup,
privilege, authentication or malformed-framing failure cannot become a good
wrong-key result. Both literal SOCKS request attempts, an actual outbound relay
delta, expected post-request HTTP refusal and a sent but unacknowledged UDP
request are required. Reverse packet/fragment receipts cannot contradict a
missing exact echo or claimed EMSGSIZE; correlated service receipt publication
is polled with a bounded deadline rather than one-shot timing assumption.
Valid incremental HTTP/UDP receipts survive a later inconclusive UDP case, while
the independent wrong-key/recovery controls still run. That cell is
`PARTIAL-NONPASS`, not `MEASURED`. Terminal guards require the exact 24 ordered
geometries, four planned UDP outcomes per completed cell, and matching measured/
partial/refused and attempted/measured/NONPASS UDP counts. Missing/duplicated
cells, partial counts relabeled all-measured and incomplete cardinality refuse.
