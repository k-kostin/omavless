# Managed DNS pair: fresh x86_64 VM setup diagnostic — 2026-09-28

Scope: agent-run, disposable, headless Omarchy VM derived from the clean
post-OS baseline. This is not owner-attended acceptance, an actual marketplace
install, or proof of a published GitHub download. The physical PC and its VPN
were untouched. The VM started without OmaVLESS packages, store, plugin or TUN;
no private profiles, endpoints, logs or screenshots are retained here.

## Inputs and transport boundary

- Application and DNS packages were inspected artifacts of Draft #307 source
  `6b672f9c6e6fd5ff53f4df37568932e8d151d697`, version `0.9.0rc1-1`,
  x86_64. The application declares an exact dependency on
  `omavless-dns=0.9.0rc1-1`; both embedded source identities matched.
- Application package SHA-256:
  `477309dddf18b405258b664ff1a0998e8dbe934b73cc70dc9d5a93de58f12dca`.
  DNS package SHA-256:
  `5d3d5b4d1204ab233c22ab3f5f9f1e02eaef8d63e2dc168faa9939917cdbb8e8`.
- The plugin's two release pin maps remain intentionally empty. To test the
  unpublished first-use source, a VM-only fixture supplied those exact hashes
  and substituted only the fixed GitHub asset transfer with local copies.
  The original installer still performed both hash/identity checks, normal
  `pacman -U` in one transaction, separate enrollment consent, Rust commands
  and final readiness checks. This does **not** verify the real CDN path.

## Failure discovered and correction

An older Draft #306 application artifact still depended on stock `mihomo`;
ordinary pacman correctly refused that pair without changing packages. With
the matching Draft #307 packages, the first source sequence installed both,
initialized the empty store, but attempted Rust cutover before selecting the
managed core. The candidate started and the transaction restored Legacy; no
connected state was claimed. A manual empty-state sequence proved that
enrollment, broker start, bundled-template preparation and pair selection
**before** cutover allowed Rust ownership to commit. Draft #308 was amended to
use that order and gained a synthetic regression test.

## Corrected fresh run

The clean baseline was booted again from a new disposable overlay. The amended
installer reported `needs_package`, then accepted `INSTALL`, installed both
matching packages with normal pacman confirmation, and separately accepted
`DNS` for numeric-UID enrollment. It prepared the bundled template, selected
the fixed pair, committed Rust ownership and enabled the user runtime. Final
read-only facts: target `rust`, component status `ready/present`, both packages
at `0.9.0rc1-1`, selected pair true, runtime unit enabled/active, broker
enabled/active with FD store zero, initial private store present, installer
lock absent, and TUN absent. No VPN connection was started.

## Remaining gates

Actual immutable release assets and download pins, GUI-first marketplace
installation, ARM64 fresh setup, interrupt/recovery matrix, default-deny
firewall guidance, clean removal, live traffic on the final release pair and
formal owner-attended acceptance remain separate. These exact artifact hashes
are diagnostic inputs, not publication pins or a claim that the current Draft
head has been installed.
