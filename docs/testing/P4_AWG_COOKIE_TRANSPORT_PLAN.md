# P4 isolated AWG cookie/MAC2 transport gate

Developer-only continuation of [the AWG transport fixture](P4_AWG_LOOPBACK_SMOKE_PLAN.md),
stacked on exact #552 `8064334451aaf5afe948af5160c593e7446a49b3`.
This is not installed-v4 activation, Full/Routing acceptance, provider evidence
or a completed security audit. A sole disposable-VM lease is mandatory; physical
PC networking and installed services/configuration remain outside scope.

## Immutable, separate engine identities

The independently instrumented official peer remains
`b5928efb6ca19f0153958460c3d141f04abc5c2e` / `v3.1.20260828`. Its verified
`git archive` SHA256 is
`716c0eec8a7557485555397f1217e0cac56317af8b41fefc287027d0c74ce00d`.
The unchanged Mihomo `v1.19.31` outbound embeds a different engine revision:
[`0c1c6f40ecd7`](https://github.com/MetaCubeX/mihomo/blob/v1.19.31/go.mod).
These are not equivalent revisions or source-build attestations of the installed
binary. Record the actual core SHA and build information independently.

The exact [official receive worker](https://github.com/amnezia-vpn/amneziawg-go/blob/b5928efb6ca19f0153958460c3d141f04abc5c2e/device/receive.go)
checks MAC1, challenges an under-load initiation without valid MAC2, and processes
the valid retry. Receiving/consuming a cookie is independent of the endpoint's
DisableCookies server-challenge flag in the
[embedded client engine](https://github.com/MetaCubeX/amneziawg-go/blob/0c1c6f40ecd7/device/receive.go).
Thus the 3.1 client preserves its native DisableCookies=true; only the independent
server selects false for the cookie gate. A separate 3.1 server-true bypass case
must respond with no cookie and no valid MAC2.

## Bounded instrument, not a production patch

`build_cookie_peer.py` executes a fresh bounded regular-file-only hash-verified
source export. The checkout stays read-only. Go's add-only virtual overlay adds
one `p4_cookie_transport` hook file; it replaces no existing implementation.
The fixed 90-second private atomic under-load deadline avoids queue floods,
unsafe/reflection and new wire semantics. The real UDP Bind, actual endpoint
bytes, cookie checker, classifier, handshake queue and worker remain in use.
Only fixed mode `pass` or `corrupt` is read from an owned regular single-link
0600 file inside private fixture scratch, never through product IPC.

The relay handles only fixed synthetic loopback ports. It classifies and decodes
the actual protected cookie, locates the nonempty cookie authentication field
from the decoded structure with exact bounds, then optionally flips one encrypted
authentication byte in **every** challenge. It does not guess cookies by length,
alter headers/index/nonce/trailers or export packets/keys. Numeric counters report
MAC1 without MAC2 and cryptographically valid, source-bound MAC2 observed by the
server's actual checker, including ordering after a classified challenge.

The clean offline Go environment disables inherited GOENV/GOFLAGS/workspace/
toolchain download/proxy/sumdb overrides. Verify pinned cached modules, exact
named normal/race tests and no skips/failures. Receipts include every fixture,
overlay, builder/exporter, source archive, toolchain and binary hash. Remove the
overlay and build an untagged peer separately; its symbols must contain no tagged
hooks. Stage [the complete notices](../../tests/fixtures/p4_awg_peer/NOTICE.md)
with the instrument. No fixture binary is installed or shipped.

## Actual acceptance sequence

Each AWG 3 and 3.1 round generates synthetic keys privately inside a proven fresh
user/network namespace. Each phase starts a new peer, server cookie secret,
core and session; reusing the round's correct credential does not reuse a session.
Use the existing strict Rust native import/private-v4/export-reimport/render path,
TUN-off unchanged Mihomo SOCKS, fixed WG-side HTTP response and selector with no
DIRECT fallback. Require field readback, H3=303/S3=40 actual protected-cookie
classification, nondefault wire fields and 3.1 trailer activity.

1. Positive: fresh MAC1/no-MAC2, actual cookie, valid source-bound MAC2 after that
   cookie, authenticated response/transfer and exact HTTP response.
2. Corrupt-cookie negative: correct native credentials; every classified challenge
   corrupted; HTTP refuses with empty response, zero valid MAC2, zero handshake,
   zero peer transfer/response. No alternate credential/import refusal is a PASS.
3. Fresh same-key recovery repeats the positive chain.
4. AWG 3.1 only: active under-load server DisableCookies=true bypass responds and
   transfers without a cookie or valid MAC2; client native fields stay unchanged.

Repeat bounded rounds and a separate full invocation. Credentials/raw logs stay
0600, private and payload-free on output; JSON accepts only exact bounded numeric
fields. Copy attested executables into fresh no-capability inodes; peer/core/HTTP/
request children have zero capabilities and NoNewPrivs. Owned processes, TUN,
keys/config/log scratch are removed, namespace is empty, and outside network,
installed core processes and native service state are unchanged. No package,
module, sysctl, service, resolver, interface or installed-profile operation is
allowed. Missing support refuses honestly; source-only tests cannot replace the
actual transport sequence. Release the VM lease explicitly after cleanup.
