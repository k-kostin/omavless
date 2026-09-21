# Native 0.8.2 release preparation

Current candidate: **0.8.2**, incorporating #263's Full Quit and UI fixes.
Its [GitHub prerelease](https://github.com/k-kostin/omavless/releases/tag/v0.8.2)
is published and [public downloads are verified](../../docs/testing/NATIVE_082_ARTIFACTS_2026-09-21.md#public-prerelease-verification).
This is not stable or a marketplace update.
Earlier public `v0.8.0` and `v0.8.1` prereleases and their assets remain immutable.
The [0.8.1 artifact record](../../docs/testing/NATIVE_081_ARTIFACTS_2026-09-21.md)
is historical evidence, not proof of a 0.8.2 build.
Use explicit `--stable` for this source. This directory does not publish a GitHub
release/tag, upload an artifact, update marketplace metadata or install software.
Marketplace changes require the owner present and explicit approval.
The [fresh-marketplace checkpoint](../../docs/testing/MARKETPLACE_FIRST_RUN_2026-09-15.md)
is also required: the installed package acceptance alone does not prove that a
marketplace user can obtain and initialize the application.
The [ARM64 preparation report](../../docs/testing/NATIVE_080_RC_PREPARATION_2026-09-13.md)
records the actual built pair, checksums, isolated installer tests and remaining
installed-release gates. It is not a stable-release acceptance claim.

The build tuple is one reviewed source commit, locked Rust workspace version,
prebuilt native ELF SHA-256, Arch package and matching native-only frontend.
`Cargo.toml` supplies the product version; Cargo workspace members inherit it.
Arch spells `0.8.0-rc.1` as `0.8.0rc1` (package release `1`), so it sorts before
stable `0.8.0`. The candidate frontend manifest gets the exact Cargo spelling.
This follows Arch's [pkgver restrictions](https://man.archlinux.org/man/PKGBUILD.5.en)
and [version ordering](https://man.archlinux.org/man/vercmp.8.en); local tests
also exercise `vercmp` when installed.
The root `manifest.json` and source installer describe the same native version.
Plain source `./install.sh` is native-only; `--native-only` remains an alias.
The historical marketplace snapshot is unchanged.

## Offline artifact assembly

### Native architecture builds without a personal Omarchy host

The `Native packages` workflow builds its exact checked-out Git HEAD
in an official, dated Arch Linux
base-devel container on an x86_64 runner. Cargo uses the locked dependencies and
pinned 1.98.0 toolchain. Compilation and the existing strict native packager run
as an unprivileged disposable build user. It retains the package, build log,
toolchain/source identity, ELF/payload inspection and checksums as CI artifacts.
No release token, upload to Releases, installed service, TUN or private fixture
is used. An actual CLI `--help` invocation proves loader execution, not VPN health.

The ARM64 job uses GitHub's native `ubuntu-24.04-arm` runner and the official
Arch Linux ARM generic rootfs in a disposable Docker container, not emulation.
The 2026-08-05 image was downloaded from the official DE3 mirror and verified
against the build-system signature with fingerprint
`68B3537F39A313B3E574D06777193F152BDBE6A6`, published on the
[official downloads page](https://archlinuxarm.org/about/downloads).
The workflow pins its SHA-256
`42a4eeaa038994ffd31fa173256ef2f0ef511358eeb41b9ea1f8626391b9b319`;
an upstream image replacement fails closed and requires a reviewed pin update.
Distribution dependencies use normal signed pacman repositories. Both jobs run
the same unprivileged `build-native-ci.sh` and strict native package inspector.

The workflow runs manually or when its build/runtime/package inputs change.
It does not replace the normal Test workflow or make installed acceptance pass.
The source must be clean; package version comes from committed Cargo metadata,
and archive inspection verifies that same source/version. The former workflow
was pinned to `b7fd0a99b8b169f0933e5f43ea4389642015193a`; it must not be reused
to claim a build of later fixes. CI artifacts are named by workflow SHA and
remain unpublished. These two explicitly allowlisted build-tool paths do not
change runtime input comparison; other unknown CI/build files still fail closed.
The generated same-source frontend is
not the final release frontend: pair the reviewed package with the later setup
pin commit using the existing tool.

On the matching Arch architecture, first run the normal gates and build the
reviewed clean commit with the locked toolchain. For example:

```sh
cargo build --release --locked -p omavless-runtime --bin omavless
```

Cargo may obtain locked build dependencies if they are not already cached; use
`--offline` when a complete cache is available. Build tools are developer-only.
Then create an empty absolute output directory **outside the checkout**, and run:

```sh
python3 packaging/release/build-candidate.py /absolute/empty-output /absolute/prebuilt/omavless FULL_SOURCE_COMMIT_SHA --stable
```

The assembler requires a clean exact Git head and the matching explicit version
mode. For current `0.8.2`, it invokes the offline Arch packager with `--stable`;
historical RC sources use the no-flag assembler and `--candidate` packager.
It then builds the frontend from
an allowlist of **committed regular Git blobs**, not a recursive worktree copy.
No private/untracked file, backend.py, test, agent skill or legacy uninstall
script is included. The wrapper always calls the accepted installer with
`--native-only`; arbitrary arguments cannot select a compatibility path.

Output:

- `omavless-0.8.2-1-ARCH.pkg.tar.zst`;
- `omavless-0.8.2-frontend.tar.xz`;
- `release-candidate.json`: full source, version, architecture and binary/archive
  hashes; explicitly caller-supplied prebuilt provenance;
- `SHA256SUMS`: both archives and the identity record;
- `arch-build/`: retained intermediate evidence; **not a release asset**.

Check hashes from the output directory with `sha256sum --check SHA256SUMS`.
Inspect package contents/dependencies and the frontend before installation.
Assembly does not execute the supplied ELF or prove it came from the named
source: retain the actual build command/toolchain, clean head and build log.
Do not present unsigned checksums as authenticity or cross-architecture proof.

Python is required only by this developer assembly tool and reference tests,
never by either delivered artifact. No new crate or runtime dependency is added.

## Reuse a reviewed package with a newer frontend

A frontend-only correction or bootstrap checksum pin need not rebuild the already
accepted native package. In particular, a package's hash cannot be embedded in
the same source commit that the package identifies: finalize the package first,
then commit its pins in the newer frontend. Keep **both** source identities.
The single-source assembler above retains its original strict checks.

On the package's architecture, use a clean descendant checkout and an empty,
current-user-owned absolute directory outside the checkout, under non-writable
by-others parents (for example a private build-artifacts directory, not `/tmp`):

```sh
python3 packaging/release/pair-frontend.py /absolute/empty-output /absolute/reviewed/omavless-0.8.2-1-ARCH.pkg.tar.zst FULL_FRONTEND_COMMIT_SHA REVIEWED_PACKAGE_SHA256
```

This offline developer tool:

- verifies the supplied archive hash before parsing, then reuses the existing
  bounded package inspector for ownership, paths, payload, architecture, version,
  embedded ELF hash and source identity;
- compares committed Git paths, modes and blob IDs between package source and
  frontend source, requiring ancestry and exact equality outside an explicit
  frontend/documentation/test/release-tool allowlist;
- protects all crates, Cargo/toolchain/build configuration, templates, systemd
  units, Arch packaging, licenses/notices and unknown new paths;
- checks populated `plugin/runtime-release.json` pins against this exact
  architecture/package/source; absent or empty pins are recorded as **not ready**
  for guided public installation, not fabricated;
- copies the original package bytes and creates the committed native-only
  frontend archive; neither artifact contains this developer Python tool;
- writes `frontend-pair.json` and `SHA256SUMS`, retaining runtime and frontend
  source commits, input-tree digest, ELF/archive hashes and unpublished status.

Review the allowlist when build inputs change. This is not a general compatibility
detector: API/runtime changes require a new build and their affected acceptance.
The exact root documentation entry `CONTRIBUTING.md` is allowed alongside
`AGENTS.md`; adding developer navigation must not invalidate an unchanged runtime
package. This is not a wildcard for new root files: unknown paths still fail closed.
Original build provenance and host evidence remain required; checksums are not
signatures. Input equivalence does not prove frontend correctness, release asset
availability, guided download/install/activation or another architecture's behavior.
Use trusted local artifacts in private directories, not an adversarial same-user
workspace. A failure may leave partial files in the new output directory; no
existing archive is overwritten or cleaned up automatically.

The tool never builds, executes the ELF, installs packages, accesses the private
store, controls services, downloads, creates tags, uploads or publishes. Actual
published pins, clean guided E2E and owner release/marketplace approval remain
separate gates even when offline pairing succeeds.

## Release gates and deliberate stop

### Explicit stable assembly (offline preparation only)

The default assembler remains RC-only. Once the owner approves a clean source
commit with the stable Cargo/lock/manifest versions, use the same assembler
with an explicit `--stable`:

```sh
python3 packaging/release/build-candidate.py /absolute/empty-output /absolute/prebuilt/omavless FULL_SOURCE_COMMIT_SHA --stable
```

This mode requires a three-part stable version from committed `Cargo.toml`;
it rejects RC/beta/build suffixes and versions longer than 32 characters.
Conversely, the default mode continues to reject stable versions. No CLI
argument supplies or rewrites the version. The native binary must be built
from that exact source separately; the assembler inspects but never executes it.

Stable Arch packages use build-identity schema **3** with `productVersion`,
mapped exactly to `VERSION-1`; schema 2 remains RC-only, schema 1 remains
development/source-prefixed. The attended inspector understands all three and
retains exact source, hash, architecture, payload and permission checks. Older
inspectors reject schema 3: use the checker from the reviewed release source.
No installed runtime reader or service policy changes with this test-tool schema.

Outputs retain `release-candidate.json` and `publication: unpublished-candidate`
even when the version is stable. An assembled archive is not publication,
release approval, a signature or installed acceptance. No tag/upload/package
installation/service action occurs. Tests build RC and stable pairs around a
synthetic system ELF, not a falsely labelled OmaVLESS binary.

The [VM-to-PC checklist](../../docs/testing/NATIVE_080_RELEASE_HANDOFF_2026-09-14.md)
separates completed VM gates from the remaining exact-artifact/owner gates.

For an already activated native installation, the developer package gate can
perform only the required update (no downgrade/removal rehearsal):

```sh
python3 tests/installed_native_package.py --run --upgrade-only --current-package /absolute/new-final.pkg.tar.zst --rollback-package /absolute/installed-old.pkg.tar.zst
```

Run in a real interactive terminal, after startup Off, verified disconnect and
settled OS authorization. `--rollback-package` must match the running installed
package; `--current-package` is the strictly newer candidate. Both retained
archives are inspected before effects. Each Stop/Install/Start requires a human
`ready` before and `settled` after. Passwords go only to the normal OS/pacman
prompt, never to these acknowledgement prompts. A failed step stops without
automatic compensation. Successful output verifies the actual running binary,
units, private-state preservation, enablement and disconnected core/TUN state.
Without `--upgrade-only`, the existing explicit recovery rehearsal is unchanged.

- [ ] Exact final candidate: full deterministic/parity/CI gates.
- [ ] Matching native architecture package and frontend inspection/checksums.
- [ ] Fresh and existing-native frontend installation/refusal tests, without
  accidental Python fallback or ownership activation.
- [ ] Attended package install/update and 0.7.0 migration on the final artifact
  where changed packaging/version behavior requires it; retain existing R6
  evidence where implementation is unchanged, rather than repeating all UI.
- [ ] Record architecture coverage: ARM64 evidence cannot manufacture x86_64
  package acceptance; publish only tested artifacts.
- [ ] Final release notes/install/upgrade instructions and known limitations.
- [ ] Owner approves final stable version/tag/release publication.
- [ ] **Separate owner-present approval for marketplace update.** Never infer
  this from a green PR, a tag or a completed R6 migration.

AUTO-1, DNS/provider findings and V0 fixture gaps remain explicitly open. The
release scope must not advertise them as validated features. No AUR/NixOS
publication, silent Cargo download, generic privileged helper, automatic
cutover or seamless connected package upgrade is introduced here.

Historical RC candidate packages use build-identity schema 2: the existing source, binary,
architecture and provenance fields plus `productVersion`. The attended package
checker requires its exact RC-to-Arch version mapping and unchanged payload
safety checks. Schema 1 development packages retain their SHA-in-version guard.
Current stable-format packages use schema 3, as described above; the RC mapping
does not apply to them.
None of these identity schemas is a signature or proof that caller-supplied bytes were built
from the declared source.
