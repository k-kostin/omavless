# P4 actual Mihomo AWG cookie/MAC2 transport — 2026-10-03

**Scoped transport PASS, not activation.** The unchanged installed Mihomo binary
completed genuine AWG 3 and 3.1 cookie challenges, cryptographically validated
source-bound MAC2 retries and fixed WG-side HTTP through TUN-off SOCKS. Two
independent disposable user/network namespace invocations each passed three
rounds per generation. This advances the source-only cookie gate in #552; it
does not convert its earlier source-only evidence into transport evidence.

## Exact tested identities

| Artifact | Identity |
| --- | --- |
| Owning base | #552 `8064334451aaf5afe948af5160c593e7446a49b3` |
| Frozen actual fixture/harness code head | `2b3762fdf3c24e2fec369a3ff6a3d98e0ce0c893` |
| Cookie harness SHA256 | `e717ddd9e27e7845c54bd2d7ab2a1606255145bdf74937ba4b18820483791cf6` |
| Independent official peer source | `b5928efb6ca19f0153958460c3d141f04abc5c2e` / `v3.1.20260828` |
| Exact fresh source archive SHA256 | `716c0eec8a7557485555397f1217e0cac56317af8b41fefc287027d0c74ce00d` |
| Add-only tagged device hooks SHA256 | `6203b5f2643ded3acab74f1c07e82be850edc9fe2beb285629974a5333c3bd7a` |
| Tagged wrapper SHA256 | `40a544c7ebcbd995a173f7d85b9ac58f60bc545273c446ff20f8c8f6310f7d73` |
| Builder / reused bounded exporter SHA256 | `8454b7b64118f641b0adeb0d955e2366cd4733429cfae1ed9b844f5375622b3d` / `55fa8597b183a5b8deb0b0aabf25740ee12dfc221054e04e62ac017d1b86cc6e` |
| Tagged peer executable SHA256 | `4febf0ad60bc9d8a11ba935c42b59571b2bbdec921dcdb5c10f305a130ccce73` |
| Staged normal50 build receipt SHA256 | `1e814c9fe14095532e15f04b0c7494888dabd7885d0597b43bd21d30988ba13b` |
| Independent race20 receipt SHA256 | `125d58ba734bf6d4c4f0688af9b69a494eb973b5eb29772a2de34c4c17bbdd1e` |
| Reused strict Rust renderer SHA256 | `8e79eb273ed0d08a245e7b5abc05afcd471e97e614c913a63e73730bc2a66dea` |
| Actual unchanged core SHA256 | `ba7a74ed6bbc3098930e6e172fa9b7d4ef7e42a77f7a516046d61fbeb3bfcfe6` |

The reused AWG and WG helper hashes remain respectively
`3881d327d4ed306dc3656dc2b3237c8db62601b6b1ecf6098e20dcaac41e2486`
and `71edf75dfa38a0de94f0ade79977e84c0dc42bbea7642a4e4f050fba583fa625`.
The renderer retains the exact #552 source/build provenance; it was copied from
the explicitly released artifact after regular-owner/no-capability and before/
after digest checks, not rebuilt or newly source-attested. Later documentation
heads do not retest different transport bytes by implication.

The peer is a deliberate `p4_cookie_transport` developer instrument: fresh
hash-verified bounded archive execution, add-only virtual device hooks and a
default-inert wrapper seam. No existing upstream implementation was replaced;
the module cache and checkout stayed unchanged. A separately built untagged peer
had no P4Fixture/cookieObserver symbols after overlay removal. The tagged binary
attests `github.com/amnezia-vpn/amneziawg-go/v3/p4-peer`, main module `(devel)`,
not a falsely versioned cached dependency. Its receipt pins every Go fixture/
overlay file, exporter/builder, source archive, compiler and binary byte hash.
Pinned cached modules verified offline: x/crypto v0.42.0, x/net v0.44.0 and
x/sys v0.36.0 with their exact go.sum checksums. Complete MIT/BSD
[notices](../../tests/fixtures/p4_awg_peer/NOTICE.md) were staged with the binary.

Compiler: `go1.27.0-X:nodwarf5`, linux/amd64/GOAMD64=v1, CGO=0,
`-trimpath -mod=readonly -buildvcs=false -tags=p4_cookie_transport`.
Race instrumentation applies to CPU tests only; the two final builders reproduced
the same CGO=0 peer hash. Explicit clean GOENV=off/GOWORK=off/GOTOOLCHAIN=local,
empty GOFLAGS, proxy/sumdb off and fixed private HOME caches/scratch prevented
inherited compiler/test-wrapper/download overrides. No shared Cargo build ran.

The actual core reports `mihomo-bin 1.19.31-1`, go1.26.8, `with_gvisor`.
Read-only `go version -m` of an exact copied public executable confirms embedded
`github.com/metacubex/amneziawg-go v0.0.0-20260908071407-0c1c6f40ecd7`, checksum
`h1:OJa/EzqUNScl8X4UCUfG5DDqRdEds89k2XwzBZrnqcE=`. This is distinct from
the official peer. Its build metadata records revision
`ab405bad5beeeac8b003bb01f60f134f6df54471`, vcs.modified=true and a `+dirty`
main module: the metadata is **not** a clean upstream source-build/release
attestation. Acceptance belongs only to the exact core binary above.

Guest tooling: kernel 7.2.5-3-omarchy, Python 3.14.7, iproute2 7.2.0-1,
util-linux 2.42.3-1, curl 8.22.0-1, wireguard-tools 1.0.20260223-1. No package
or system-configuration preparation was performed for this slice.

## Actual nondefault cookie chain and negative proof

The [contract](P4_AWG_COOKIE_TRANSPORT_PLAN.md) records the endpoint-role,
provenance and cleanup boundaries. Each phase starts a new official peer/server
cookie secret, core and session. A round's correct synthetic native credentials
are reused for the corrupt-cookie refusal and fresh recovery, never a previous
session. The Rust strict native import/private-v4/native-export-reimport/render
path produces every tested config; no private installed profile is read.

All declared AWG fields were read back active. H3=303/S3=40 actual protected
cookie classification uses the device's classifier/header cipher and decoded
structure, not datagram-length inference. The fixed private under-load deadline
activates the existing real handshake worker without flooding or injecting a
production queue. The real UDP Bind's untouched buffers/endpoints/count/error
continue to that worker; the observer checks actual MAC1 and actual source-bound
MAC2 with the server's cookie checker, not nonzero-byte guesses. No raw keys,
packets/configs or private logs are emitted.

Positive and recovery phases each observed MAC1 without MAC2, exactly one
classified protected cookie, then one valid MAC2 after that cookie and no MAC2
before it, followed by authentic response/transfer and exact HTTP response.
Each negative retained the **correct** client and header keys but flipped the
actual nonempty cookie authentication field in every classified challenge:
three of three challenges corrupted, three MAC1/no-MAC2 retries, zero valid
MAC2, zero response/transport, zero handshake/RX/TX, failed empty HTTP. The same
selector had no DIRECT fallback; HTTP refusal was not an import/key rejection.

The AWG 3.1 client retained native RandomTrailers=true and DisableCookies=true.
That flag gates an endpoint sending server challenges, not consuming received
cookies. The independent peer/server used false for cookie phases; its separate
under-load true bypass phase produced actual HTTP/handshake/transfer with zero
cookies and zero valid MAC2. Generation 3 had no handshake trailers; 3.1 cookie
success phases observed three extended handshake datagrams and bypass phases
four. All successful phases observed Jc activity, all five signatures (mask31),
protected initiation/response/transport and nonzero 37-byte padding length
correlations. These are not per-packet cryptographic padding attribution.

| Gate across both generations | Full invocation 1 | Full invocation 2 |
| --- | --- | --- |
| Initial authentic cookie → validated MAC2 → fixed HTTP | 6 PASS | 6 PASS |
| Correct-credential corrupt-every-cookie refusal | 6 PASS | 6 PASS |
| Fresh same-key cookie/MAC2 HTTP recovery | 6 PASS | 6 PASS |
| AWG3.1 under-load server DisableCookies bypass | 3 PASS | 3 PASS |
| Strict native/private-store/export-reimport roundtrip | 21 PASS | 21 PASS |
| Namespace/TUN/process/scratch cleanup and outside state unchanged | PASS | PASS |

Aggregate full runs: **30 successful HTTP requests** (24 cookie/MAC2 chains plus
six intentional bypasses), **12 refusals / 36 corrupted challenges**, 12 fresh
recoveries and 42 private roundtrips. A separate one-round-per-generation probe
also passed four cookie/MAC2 HTTP chains, two refusals/six corrupted challenges,
one bypass and seven roundtrips; it is excluded from those aggregate full-run
counts. No actual failed VM run occurred in this slice. Earlier #552 failures
remain preserved there and are not retroactively relabelled PASS.

## Cleanup, ordinary gates and remaining limits

Ordinary-account supervisors proved fresh retained user/net namespace identities,
only loopback/no routes, owned nonpersistent TUN and exact peer/HTTP/core child
identity. Copied executables had no file capabilities; runtime children had
zero capabilities and NoNewPrivs. Every phase left only loopback; every invocation
proved no owned namespace process, removed private keys/configs/bounded logs and
scratch, and checked unchanged outside network, installed core PIDs and native
service. The empty owned cache root was removed; public-only staging and bytecode
were recoverably trashed. No fixture peer remained. Installed native service
stayed active/running at MainPID86349 and core bytes/capabilities unchanged.
VM ownership was explicitly released before host-only evidence/PR work. Cleanup
is not a secure-erasure or snapshot-absence claim.

Frozen-code ordinary gates: source suite **310 tests / 2 existing skips** plus
JS/native/QML contracts PASS; ten new pure refusal/privacy/receipt/result guards
are registered in normal tests/run.sh. Normal50 and race20 each passed exact
named worker/hook tests (300+120 subcases) and tagged wrapper tests (200+80).
Normal untagged Go tests, go vet, formatting, offline module verification, source/
export immutability, overlay removal and normal symbol exclusion PASS. Rust app,
frontend, packaging and production IPC code are unchanged; final owning-head CI
is recorded in the Draft rather than borrowing an older head's result.

This is a forced-under-load loopback protocol smoke, not realistic overload/
rate-limiter performance, broad header ranges, production hooks, a minimum core
release review, IPv6/provider/network-leak/Full/Routing/lifecycle acceptance,
installed-v4 activation or completed security audit. The installed v1–v3 owner
and all existing P4 activation fences remain unchanged. No host-PC VPN, package,
service, network/sysctl/resolver or private installed configuration was changed.
