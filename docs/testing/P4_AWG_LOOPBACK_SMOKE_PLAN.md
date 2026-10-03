# P4 isolated AWG 3/3.1 transport fixture

Developer-only opt-in fixture stacked on the [WG smoke](P4_WG_LOOPBACK_SMOKE_PLAN.md),
not product activation. Sole disposable-VM ownership must be explicitly handed
off before a run. Never run on the physical PC or against an installed/private
profile. Missing tooling, TUN, user namespaces, immutable artifact identities or
safe cleanup refuse; there is no package/sudo/module/sysctl fallback.

## Exact independent peer

`tests/fixtures/p4_awg_peer` uses the **unmodified official**
[Amnezia userspace engine](https://github.com/amnezia-vpn/amneziawg-go/tree/b5928efb6ca19f0153958460c3d141f04abc5c2e)
at `v3.1.20260828` / `b5928efb6ca19f0153958460c3d141f04abc5c2e`.
Its standalone fixture module has pinned `go.mod` / `go.sum` and
[dependency notices](../../tests/fixtures/p4_awg_peer/NOTICE.md). It is not linked
into the Rust runtime, exposed through application IPC or installed as a
service. No upstream protocol implementation is copied or patched.

The optional [cookie overlay](../../tests/fixtures/p4_awg_peer/run_cookie_overlay.py)
is a separate **source-engine-only** CPU test, not a transport run or a Mihomo
cookie-path acceptance. It needs no VM, socket, real TUN or namespace. It verifies
a clean exact upstream checkout/source-export hash and cached dependencies,
executes a fresh bounded hash-verified export (never checkout files), virtually
adds one tagged test, and removes its overlay/export even on refusal.
Fake Bind/TUN implementations cannot touch OS networking. A private atomic
under-load deadline and one real handshake queue element exercise the actual
worker with synthetic keys in memory; no queue flood, unsafe/reflection or
production patch is permitted. The test selects H3=303/S3=40 vs H2=202/S2=32,
checks protected classification, corrupt/wrong-context/truncated cookie refusal,
source-bound MAC2 retry and authentic Noise response with DisableCookies both
false and true, with RandomTrailers both false and true. Always discard/recheck
the overlay before rebuilding any peer binary; normal fixture builds exclude
the test tag. Missing cached metadata refuses offline; prepare only dependencies
pinned by the exact upstream go.mod/go.sum before the offline invocation.

A separate [cookie/MAC2 transport contract](P4_AWG_COOKIE_TRANSPORT_PLAN.md)
uses an explicitly tagged add-only developer instrument and unchanged Mihomo.
It must not be confused with this normal peer or the source-only CPU overlay.

Build only the reviewed module with a locally available compatible Go compiler,
`GOTOOLCHAIN=local`, HOME cache and HOME temporary directories. Use
`CGO_ENABLED=0 go build -trimpath -mod=readonly -buildvcs=false`; record the
exact compiler, source, checksum and `go version -m` attestation. `go mod verify`
and comparison of downloaded engine source with the immutable upstream checkout
are prerequisites. Stage its notice with the binary. Never fetch/execute a
moving installer, use `curl | shell` or download a binary without attestation.

## Namespace and credentials

The Python developer glue reuses the WG fixture's bounded private-file,
subprocess, controller/no-DIRECT and outside-state helpers. It does not parse
user protocol sources: the existing Rust strict native parser/store/renderers
still own import -> private v4 bytes -> config -> native export/reimport.

The ordinary-account supervisor retains outside user/network namespace FDs
before `unshare --user --map-root-user --net`. Its exact child proves different
namespace identities, correct parent PID, only loopback and no route before
creating a nonpersistent TUN inside that namespace. The parent configures only
the synthetic peer address and narrow client return route. The peer receives
this owned TUN FD; its exported unmonitored-TUN API avoids privileged setup in
the executable. It starts via `setpriv` with all capabilities dropped and
NoNewPrivs=1, rechecks those conditions and typed retained NSFS descriptors,
then uses the official engine's device/bind/UAPI implementation.

No AWG kernel module, external interface, veth, forwarding, firewall, default
route, resolver/provider lookup, service change or installed config access is
needed. The peer's private Unix UAPI lives only in scratch, not `/var/run`.
Every request checks SO_PEERCRED against the exact owned peer PID.

Each round privately generates fresh server/client/wrong-client/header/wrong-
header keys plus PSK through captured pipes **inside the namespace**. No key is
in argv, environment, Git or shared output. Directories are 0700 and credential,
native config, prepared YAML and bounded log files are 0600 before writing.
The tested core, renderer and peer are copied to fresh non-file-capability
inodes with required checksums. Core, peer, HTTP and request children have no
capabilities; core TUN and DNS stay off with one selector and no DIRECT fallback.

## Active non-default generation proof

The fixture requires `Jc=4`, `Jmin=64`, `Jmax=66`, distinct `I1`–`I5`,
`S1/S2/S3/S4=24/32/40/48`, `H1/H2/H3/H4=101/202/303/404`, fresh nonzero shared
header-protection key, `ContentPaddingAddition=37`, `RekeyAfterTime=2`,
`RekeyTimeout=1`, `RejectAfterTime=30`, `KeepaliveTimeout=1`, and
`MaxHandshakeAttempts=4`. AWG 3.1 additionally sets both `RandomTrailers` and
`DisableCookies` true; AWG 3 omits those fields and remains generation 3.
Every declared field must be read back active through the engine's private
UAPI, not merely accepted by a config checker.

A namespace-loopback UDP relay between core and peer records only bounded
counts/packet-length histograms. It independently decodes the protected message
type using the shared synthetic header key, nonce and configured S/H values.
No packet/key bytes escape. PASS additionally requires observed nonzero junk,
all five distinct custom signatures, protected initiation/response/transport
headers, and transport-length/accepted-plaintext-length matches for the configured
37-byte content padding. These are bounded length correlations, not a claim of
one-to-one cryptographic attribution for every packet or arbitrary MTU behavior.
AWG 3 requires no handshake trailers; 3.1 requires actual extended handshake
datagrams. A live same-core request after three seconds must advance the peer's
handshake timestamp, exercising the configured two-second rekey.

Each generation/round also requires fresh handshake + RX/TX advancement and
fixed WG-side HTTP response, wrong client key refusal, wrong header key refusal,
and correct-key recovery. Negative controls use valid imported/rendered inputs,
not an invalid-config rejection. Reset the authenticated peer/session/timers
before each negative so old autonomous traffic cannot masquerade as refusal or
contaminate unchanged counters. All phases are deadline-bounded.

## Upstream limits and remaining gates

The chosen pin includes the official cookie-trailer and DisableCookies fixes.
Its open [wide-header/RandomTrailers issue](https://github.com/amnezia-vpn/amneziawg-go/issues/186)
and [handshake/content-padding precedence issue](https://github.com/amnezia-vpn/amneziawg-go/issues/185)
limit interpretation. Narrow singleton headers and small namespace-only traffic
do not establish broad range, MTU or performance compatibility. Cookie headers/
padding and the DisableCookies flag are active/read-back checks, **not** a
cookie challenge/under-load test. Other timer bounds are configured/read-back,
not an exhaustive retry/exhaustion/expiry matrix. Do not weaken the fixture to
Jc=0, default headers or absent header protection and call that AWG proof.

Stop/reap only owned process handles, close the nonpersistent TUN FDs, prove
only loopback remains and no process retains the namespace, delete exact owned
scratch and compare outside network/native runtime/core facts. Remove/trash
public staging separately. Cleanup is path/process cleanup, not secure erasure.

This smoke leaves installed-v4 refusal, before-quiesce core/flavor admission,
host/startup/pointer wiring and active replace/delete compensation unchanged.
Real-server, IPv6, Full/Routing/Direct, DNS/provider, autoconnect, lifecycle,
restart/adoption, migration/rollback, core review and privacy acceptance remain
separate mandatory gates. R6/AUTO-1/V0 and incomplete security scans are not
reclassified. No merge/release/publication is authorized by a passing fixture.
