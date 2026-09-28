# Unpublished managed-DNS companion candidate

This directory stages a **normal-name package candidate** from a separately
reviewed, native, offline source pair. It does not install, enroll, enable a
service, alter DNS/TUN, publish an asset or make 0.9 ready. The public setup
path, runtime selection, cross-architecture artifact pairing and owner-attended
acceptance remain separate work. See the
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

The package is named `omavless-dns`, conflicts with (but does not silently
replace) `omavless-dns-experimental`, and uses distinct core, broker, guard,
receipt and license paths. It installs a dormant system unit and an ALPM
PreTransaction removal/upgrade guard. Its scriptlet grants the reviewed core
file capabilities only; it neither grants the broker file capabilities nor
enrolls, starts, enables, connects or edits the firewall. The release broker
looks for `/etc/omavless-dns/release-enrollment.json` with the separate
`meta-ipv4-release-v1` policy. An old experimental enrollment is therefore not
authority for this package. A migration still requires an attended old-policy
revocation at a proven clean boundary; the package does not perform it.

Do not install this staged archive over an active experimental lease or use
pacman override flags. This branch does not yet make `omavless` select the new
package. Package build/inspection alone is not installed or release acceptance.
