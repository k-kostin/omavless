# PC x86_64 DNS broker pre-install evidence — September 27, 2026

This is a bounded pre-install checkpoint for Draft #295, not acceptance of
password-free DNS or of the 0.9.0 release. The source was
`e9073b574c7c2aec5fe5d9e0caeed60162b4e162` on
`dev/dns-transaction-foundation`. The outer PC's VPN, routes, DNS and installed
OmaVLESS were not changed. The separate Omarchy x86_64 VM was initially running
stock OmaVLESS 0.8.2, disconnected with startup off, no profile and no TUN.

## Exact local candidate

The review-only Mihomo patch from this source was applied once to upstream
`ab405bad5beeeac8b003bb01f60f134f6df54471`; the reviewed sing-tun patch
was applied to `b50ae28a1409c7bce8e96e6c6966cf57d8ace754` with a
scratch-only local module replacement. Go 1.27.0 built the x86_64 core with
`CGO_ENABLED=0`, `with_gvisor`, locked modules and `-trimpath`. Focused Go
config/listener tests passed. Rust release runtime and broker built with locked
dependencies from the exact repository source. These are locally built review
inputs, not signed/public artifacts or proof of upstream authenticity.

| Input | SHA-256 |
| --- | --- |
| Patched production-tag Mihomo | `dc0732957deed3382bdb69ae693d631e822d550c18107d504a326bd193e40626` |
| Rust DNS broker | `55f77cc57dde588f84c277d5b5877026b4afbcbf097970fe9892066c0a9bcf32` |
| Rust runtime | `7e3c9047530b4383d9da10ce44f9521ace40042a886dc5d847376d52b6a0ba7a` |

The experimental package staged and built without installing or enabling a
service: `omavless-dns-experimental-0.9.0rc1-1-x86_64.pkg.tar.zst`, SHA-256
`2fcc027173dd698b806ea5c27efc50840c33c8f2bb680a531e61e43997ad536f`.
The archive contains the two pinned binaries, fixed unit, ALPM guard/hook and
source receipt; it contains no enrollment, stock core override, user service or
private profile. Archive-extracted binary hashes match the table. A separate
unpublished normal RC package and frontend were assembled from the same source:
package SHA-256 `e99cc10bb26c98169279d248176ce1bfea33c24cd9ddc4a7945aa004f7290758`,
frontend SHA-256 `ba554db727744c2b2955270ba5908280914f0c3b35e8c1314c4af3d880fde6ad`.
The pair was copied into the isolated VM and its package hashes read back; neither
package was installed at this checkpoint. Build outputs remain outside Git.

## Isolated evidence and a corrected test assumption

The production-tag x86_64 core passed all eleven synthetic whole-core/channel
facts and thirteen DNS ownership facts. The standalone kernel TUN lease probe
passed all fifteen fixed facts. The packet probe initially showed
real TUN counters and correct marked/unmarked routes, but all four TCP/UDP echo
cases failed on both the PC and VM. A fresh network namespace on these systems
inherits `net.ipv4.conf.all.rp_filter=1`; Linux uses the effective maximum of
`all` and the interface setting, so disabling only the test TUN's filter did
not disable reverse-path filtering. Disabling `all.rp_filter` **inside the
already-verified disposable namespace only** made the unrestricted and
restricted TCP/UDP echo cases pass. The corrected source probe then passed all
ten facts on both the outer PC and VM using the exact production-tag core.
This changes no host-level sysctl. The earlier failure must not be treated as
either a core regression or a passing test.

The full Python/QML suite and Rust suite passed after the test correction;
targeted acceptance/packet tests reported 53 passed after the final dual-pin
runner check. The installed-owner DNS runner requires independent hashes for
both the core and the running root broker and has mock safety tests, but has
not run a real Connect or applied DNS.
The older isolated-home runner now refuses managed mode before private/host
access because current production login admission requires the installed owner.

## Still open

The VM has no usable private VLESS fixture; no installed DNS broker/core/runtime
cycle was attempted here. The human-attended installed mode sequence, actual
resolved readback, same/cross-UID admission, crash/retention, ALPM
removal/upgrade refusal, clean removal and recovery remain outstanding as
specified by the [PC handoff](../development/RC_090_PC_CONTINUATION_2026-09-25.md).
Synthetic Ready/Released messages and package metadata are not substitutes for
those installed results. #270/#132 and Draft #295 stay open; RC, main, release
and marketplace remain unchanged.
