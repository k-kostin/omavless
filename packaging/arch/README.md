# OmaVLESS native package payload

This directory defines the inert filesystem payload for the future Arch
package. The payload contains the prebuilt `omavless` executable, its packaged
systemd user unit, license, third-party notices and this packaging note.

`stage-payload.sh` accepts an existing absolute package build root and an
absolute prebuilt executable. It does not build software, install onto a live
host, invoke a package manager, start or enable a service, or alter OmaVLESS
ownership and private state.

## Explicit local development package

After reviewing and committing a clean checkout, build the exact native binary
with the locked Cargo toolchain separately. Then create an empty build directory
outside the checkout and run:

```bash
bash packaging/arch/build-local-package.sh /absolute/empty-build-dir /absolute/prebuilt/omavless FULL_SOURCE_COMMIT_SHA
```

The helper requires an exact clean source HEAD and a native x86_64/aarch64 ELF,
then uses `stage-payload.sh` and the checked-in `PKGBUILD.local.in` to produce a
normal `omavless-0.0.0.rCOUNT.gSHA-1-ARCH.pkg.tar.zst`. This development version
does not declare a marketplace release or 0.8.0 readiness. Run as an ordinary
user with Arch build tools (`base-devel`, `binutils`, `git`, `tar`, `zstd`).
No Cargo build, remote source, download, dependency installation, root operation,
package installation, service enablement or ownership activation occurs.
`--nodeps` skips build-time runtime-dependency checks only; it is **not** advice
to bypass dependencies when an owner later explicitly installs the archive.

Runtime dependency `mihomo` can be supplied by a package such as `mihomo-bin`
that declares `provides=mihomo`. The core and its reviewed TUN capability setup
remain external. Picker/QR/clipboard helpers are optional package dependencies;
this package does not silently install or invoke them. No Python or Cargo runtime
dependency is declared.

`/usr/share/doc/omavless/build-identity.txt` records full source SHA, native
architecture and exact prebuilt binary SHA256. Stripping/debug splitting are
disabled so the reviewed binary bytes survive packaging. Source provenance is
explicitly **caller-supplied**, not a cryptographic claim that an arbitrary ELF
was compiled from that source. The owner must build the reviewed source, record
its hash and verify the archive before installing. The helper never runs the
supplied binary. Build output is retained for inspection, including on failure;
it contains no private store or profile fixture.

This follows Arch's [PKGBUILD](https://man.archlinux.org/man/PKGBUILD.5.en)
local-source/checksum/package conventions and
[makepkg](https://man.archlinux.org/man/makepkg.8.en); it is not AUR publication,
release signing, CI binary distribution, auto-update or a runtime installer.

Installing these files alone does not switch VPN ownership. Until the explicit
R5 cutover is accepted, the Omarchy plugin and Python compatibility backend
remain the production owner. Package activation, update, removal and rollback
remain separately reviewed host-integration work.
