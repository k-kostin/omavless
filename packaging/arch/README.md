# OmaVLESS native package payload

This directory defines the inert filesystem payload for the native Arch
package. The payload contains the prebuilt `omavless` executable, its packaged
systemd runtime and login-preparation user units, license, third-party notices
and this packaging note.

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

`bubblewrap` is required for fail-closed offline startup validation. Runtime
startup is ordered after the fixed login-preparation oneshot; both preserve the
private owner-lock directory across service stops. Installation still does not
enable either unit. After reviewed ownership migration, explicit enablement of
`omavless-runtime.service` is required for login activation; saving startup
preferences does not enable it. Startup Off means an enabled runtime starts
disconnected. Actual packaged login acceptance is a separate R5 host gate.

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
activation transaction is performed, the existing owner remains authoritative.
The native R6 path is accepted; use the
[installation/recovery guide](../../docs/user/NATIVE_INSTALL.md) and retain its
exact host evidence and limitations.

## 0.8.0 candidate assembly

The optional fourth argument `--candidate` selects only the checked-in Cargo
RC version, rendered in Arch's compatible `0.8.0rc1` spelling. The ordinary
three-argument development package identity is unchanged. Stable versions and
arbitrary version input are rejected by this RC path. Prefer the
[release assembler](../release/README.md) to pair the package with a matching
native-only frontend and integrity record. No publication is performed.

The separate `--stable` mode accepts only a checked-in three-part stable Cargo
version (at most 32 characters). It emits schema-3 build identity and exact
`VERSION-1` package spelling; it never promotes or rewrites an RC version.
The RC/default development modes and their schemas remain distinct. This is
offline assembly, not stable-release approval, installation or publication.
