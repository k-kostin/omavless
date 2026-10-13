# Unpublished managed-DNS companion candidate

This directory stages a **normal-name package candidate** from a separately
reviewed, native, offline source pair. It does not install, enroll, enable a
service, alter DNS/TUN, publish an asset or make 0.9 ready. The public setup
path, immutable release assets and owner-attended acceptance remain separate
work. Stacked drafts add runtime selection, offline release-triple inspection
and guided first use, but their pin maps remain empty. See the
[release-distribution contract](../../docs/development/DNS_RELEASE_DISTRIBUTION.md).

The existing `tests/dns_broker_host/package` fixture stays experimental. To
build a production-flavor pair from exact local pinned sources, use its
`build_pair.py` with `--flavor release`; it compiles the broker with the fixed
release guard path and records that feature in the bounded receipt. Then run
`packaging/dns/stage.py --pair ABSOLUTE_PRIVATE_PAIR --revision FULL_COMMIT
--arch x86_64 --output NEW_ABSOLUTE_PRIVATE_DIRECTORY` (or `aarch64` on a
native ARM64 builder). The strict staging validator rejects an experimental
broker receipt, mismatched ELF architecture, source, patch, Go/toolchain,
license, symlink, checksum or occupied/unsafe destination. The staged
`PKGBUILD` has only fixed local inputs and exact SHA-256 pins; `makepkg` is a
separate ordinary-user action after review.
The ordinary `release` producer embeds Mihomo's pinned upstream version
`v1.19.31` in `github.com/metacubex/mihomo/constant.Version`, following the
[upstream linker variable](https://github.com/MetaCubeX/mihomo/blob/ab405bad5beeeac8b003bb01f60f134f6df54471/Makefile).
That exact commit is the upstream `v1.19.31` tag; without the linker value,
[the source default](https://github.com/MetaCubeX/mihomo/blob/ab405bad5beeeac8b003bb01f60f134f6df54471/constant/version.go)
reports `1.10.0`. This is core version metadata, not a claim of runtime health,
an OmaVLESS product version, or upstream endorsement of the local patches.
No wall-clock build time is injected. Existing `experimental`, `release-close`
and `release-close-k1` recipes, patches, tags and ABI receipts stay unchanged;
their previously recorded qualification is not transferred to newly built bytes.
The corrected ordinary release must be rebuilt and inspected, and its installed
`mihomo -v`/desktop setup facts checked before new package hashes are selected.
The native CI builder explicitly sets `PKGEXT=.pkg.tar.zst` on both
architectures and rejects a different output suffix. Arch Linux ARM's
default `.pkg.tar.xz` must not be silently renamed or published as the
`.pkg.tar.zst` expected by the bootstrap and offline assembler.

The package is named `omavless-dns`, conflicts with (but does not silently
replace) `omavless-dns-experimental`, and uses distinct core, broker, guard,
receipt and license paths. It installs a dormant system unit and an ALPM
PreTransaction removal/upgrade guard. Its scriptlet grants the reviewed core
file capabilities only; it neither grants the broker file capabilities nor
enrolls, starts, enables, connects or edits the firewall. The shared guard
uses a neutral refusal message; the production package does not present
itself as experimental. The release broker
looks for `/etc/omavless-dns/release-enrollment.json` with the separate
`meta-ipv4-release-v1` policy. An old experimental enrollment is therefore not
authority for this package. A migration still requires an attended old-policy
revocation at a proven clean boundary; the package does not perform it.

The native runtime recognizes the new private `managed-dns-release-v1` selector
and verifies the release core, broker and receipt before use. Its ordinary
`omavless dns-pair select` now creates **only** that release selector; it does
not fall back to the experimental package when the release pair is absent.
An existing experimental marker is refused by the new runtime rather than
silently authorizing the release package. One isolated x86_64 VM migration
with fresh enrollment and release-pair selection was checked in the
[scoped diagnostic](../../docs/testing/DNS_RELEASE_VM_MIGRATION_2026-09-28.md);
it is not the public first-use installer or formal owner-attended acceptance.

The staged package version is derived from matching `Cargo.toml` and
`manifest.json` product versions, rather than a frozen RC literal. The
candidate/stable application package declares an exact-version dependency on
this companion; ordinary development snapshots retain their separate Mihomo
dependency. Neither package authorizes runtime setup merely by being present.

Do not install this staged archive over an active experimental lease or use
pacman override flags. A guided first-use UI and normal two-package bootstrap
are staged in this draft stack, not yet published or
formally accepted. Package build/inspection and unit tests alone are not
release acceptance.
