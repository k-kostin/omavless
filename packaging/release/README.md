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
The root `manifest.json` intentionally remains the separate 0.7.0 compatibility
payload; neither it nor the default repository installer is silently switched.

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
Neither schema is a signature or proof that caller-supplied bytes were built
from the declared source.
