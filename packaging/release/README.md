# Native 0.8.0 release preparation

Current candidate: **0.8.0-rc.1**. This directory does not publish a GitHub
release/tag, upload an artifact, update marketplace metadata or install software.
Marketplace changes require the owner present and explicit approval.
The [ARM64 preparation report](../../docs/testing/NATIVE_080_RC_PREPARATION_2026-09-13.md)
records the actual built pair, checksums, isolated installer tests and remaining
installed-release gates. It is not a stable-release acceptance claim.

The build tuple is one reviewed source commit, locked Rust workspace version,
prebuilt native ELF SHA-256, Arch package and matching native-only frontend.
`Cargo.toml` supplies the RC version; Cargo workspace members inherit it.
Arch spells `0.8.0-rc.1` as `0.8.0rc1` (package release `1`), so it sorts before
stable `0.8.0`. The candidate frontend manifest gets the exact Cargo spelling.
This follows Arch's [pkgver restrictions](https://man.archlinux.org/man/PKGBUILD.5.en)
and [version ordering](https://man.archlinux.org/man/vercmp.8.en); local tests
also exercise `vercmp` when installed.
The root `manifest.json` and source installer now describe the same native RC.
Plain source `./install.sh` is native-only; `--native-only` remains an alias.
The historical marketplace snapshot is unchanged.

## Offline artifact assembly

On the matching Arch architecture, first run the normal gates and build the
reviewed clean commit with the locked toolchain. For example:

```sh
cargo build --release --locked -p omavless-runtime --bin omavless
```

Cargo may obtain locked build dependencies if they are not already cached; use
`--offline` when a complete cache is available. Build tools are developer-only.
Then create an empty absolute output directory **outside the checkout**, and run:

```sh
python3 packaging/release/build-candidate.py /absolute/empty-output /absolute/prebuilt/omavless FULL_SOURCE_COMMIT_SHA
```

The assembler requires a clean exact Git head and an RC version. It invokes the
existing offline Arch packager with `--candidate`, then builds the frontend from
an allowlist of **committed regular Git blobs**, not a recursive worktree copy.
No private/untracked file, backend.py, test, agent skill or legacy uninstall
script is included. The wrapper always calls the accepted installer with
`--native-only`; arbitrary arguments cannot select a compatibility path.

Output:

- `omavless-0.8.0rc1-1-ARCH.pkg.tar.zst`;
- `omavless-0.8.0-rc.1-frontend.tar.xz`;
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
python3 tests/installed_native_package.py --run --upgrade-only --current-package /absolute/new-rc.pkg.tar.zst --rollback-package /absolute/installed-old.pkg.tar.zst
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

Candidate packages use build-identity schema 2: the existing source, binary,
architecture and provenance fields plus `productVersion`. The attended package
checker requires its exact RC-to-Arch version mapping and unchanged payload
safety checks. Schema 1 development packages retain their SHA-in-version guard.
None of these identity schemas is a signature or proof that caller-supplied bytes were built
from the declared source.
