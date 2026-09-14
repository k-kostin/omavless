# Native 0.8.0-rc.1 artifact preparation

Try Omarchy ARM64, 2026-09-13. Release preparation in PR #239, **not a stable
release, marketplace update or new installed R6 acceptance**. The accepted
runtime/UI was left running; no package installation, service restart,
authorization prompt or VPN transition was performed.

## Exact source and artifact pair

Base: R6 integration `778647215deb1cb27e66e10e628fd0e78beee1af` (#238).
Artifact source: `ac4bd30825c43912b72c7e6cb3332f3b95db0680`.

| Artifact | SHA-256 |
| --- | --- |
| Optimized aarch64 `omavless` ELF | `e88f83496d4e301d41d02e1f53c4e19a21c02731dd8b5803d8cbdae8155d4807` |
| `omavless-0.8.0rc1-1-aarch64.pkg.tar.zst` | `42312f7a1418af1a25e4eca3cc04c2eb77ea11ec488683b14b513ff73b433b15` |
| `omavless-0.8.0-rc.1-frontend.tar.xz` | `2cd083d77e761d3d535baddcf285b5db56e998a74514961dbc7d6dc10c1fbaab` |

The clean source was built locally with the pinned Rust 1.98.0 toolchain:
`cargo build --release --locked --offline -p omavless-runtime --bin omavless`,
two build jobs, incremental compilation disabled. The release assembler then
used that binary and the exact source SHA. It did not execute a caller-selected
ELF or fetch/install anything. The identity record remains explicitly
`caller-supplied-prebuilt`: hashes and a source label are not a signature or
independent proof of compilation provenance.

The artifacts and `release-candidate.json`/`SHA256SUMS` are retained outside Git
in the VM's build-artifact directory. No binary assets were uploaded to GitHub.
This report preserves their identity, not a promise that a future rebuild with
different package tools/environment has the same archive hash.

## Checks

| Gate | Result |
| --- | --- |
| Focused release/package/attended-inspector tests | 36 run, 35 PASS, one root-only skip |
| Full Python suite on artifact source | 472 run, 468 PASS, four existing opt-in/root-only skips |
| JS presentation and QML contracts | PASS |
| Rust workspace | 957 PASS, zero failures, ten existing ignored |
| Rust fmt / all-target Clippy / R0 parity | PASS |
| Optimized locked offline ARM64 build | PASS |
| Actual candidate Arch archive inspection | PASS: exact allowed members/modes, no install hooks, matching architecture/source/binary hash and schema-2 RC version |
| Both archives and identity file checksum verification | PASS |
| Extracted RC frontend `omarchy plugin validate` | PASS; manifest `0.8.0-rc.1` |
| Fresh/update/refused frontend installation in isolated HOME/PATH | PASS; no Python binary available, no activation or real shell/service action |
| Repository plugin validation / shell syntax / Python compilation / manifest / diff | PASS |
| Checked documentation links before evidence append | 93 local links resolved |
| GitHub CI | First implementation head `2323db5` PASS; final exact PR-head status recorded on #239 |

Rust gates ran at `2323db5fc44684bdabfef2614addaee6730f541c`. Cargo files and
every crate are byte-identical at the artifact head; the subsequent adjustment
only extends package identity and its acceptance tooling. Rust test execution
used the established VM settings: dev/test debug information disabled,
`CARGO_INCREMENTAL=0`, two build jobs and two test threads. No production
deadline, cleanup assertion or security boundary was weakened.

The new version changes the build-derived support version and subscription
User-Agent; the lifecycle/IPC/UI implementation is unchanged. This is not new
server-interoperability evidence for that User-Agent.

## Packaging correction found during preparation

The existing attended package inspector admitted only development versions
`0.0.0.rCOUNT.gSHA`. Silently widening that regex would lose its source-prefix
guard. Candidate packages instead carry schema 2 with `productVersion`; the
inspector requires the exact RC-to-Arch version mapping, bounded complete
metadata and the existing binary/source/architecture/mode/payload checks.
Schema 1 development packages retain the original source-prefix requirement.
Unknown schemas, stable versions through the RC route, duplicate/extra fields
and mismatched package versions are rejected. No pacman/authorization method
or installed path was changed by this test-tool adjustment.

## Distribution and remaining gates

The delivered frontend is assembled only from allowlisted committed regular
Git blobs. It excludes Python, legacy uninstall, tests, agent skills and
untracked/private content. Its default entry point always selects the existing
native-only installer; missing/legacy/ambiguous ownership refuses before writes.
The source repository's compatibility manifest/default installer remain distinct
and unchanged. The published marketplace 0.7.0 snapshot remains immutable.

These archives have **not** been installed over the running VM package. Final
artifact package installation/update and 0.7.0-to-native migration acceptance
remain a release step where new packaging materially changes that path. Existing
[R6 closure evidence](R6_LOCAL_CLOSURE_2026-09-13.md) is retained for unchanged
runtime/UI; it is not relabelled as new RC installed acceptance. ARM64 artifact
inspection does not prove x86_64/NixOS distribution or pristine-OS onboarding.

AUTO-1 enabled login and recorded DNS/provider failures remain open. V0 remains
Draft and fixture-constrained. No stable 0.8.0 tag/release is published; a
**marketplace update additionally requires the owner present and explicit
approval**. See the [release checklist](../../packaging/release/README.md).

## Later branch maintenance — 2026-09-14

PR #239 was rebased onto main `aa5873783c019edc303a732e55ea8c85f1f0b090`,
which includes subscription-row refresh #240 and documentation cleanup #241.
Documentation overlaps were reconciled with the shorter user README and current
delivery ledger. The two non-documentation patches compare unchanged in
`git range-diff`; the aggregate non-Markdown patch ID before/after is
`22e2ee02579fff0e7f7511edfe3e177b0d7f0918`. Cargo, crates, packaging and package
acceptance implementation bytes are unchanged from the previously tested RC.

The archived pair above remains built from `ac4bd30825c43912b72c7e6cb3332f3b95db0680`.
It does **not** contain #240 or the new user documentation. No archive was
regenerated, relabelled, installed or uploaded during this maintenance. A final
delivery must regenerate the matching frontend/package identity from its actual
reviewed source. Current branch checks belong in #239; they do not turn these
historical artifacts into a newer build or fresh installed-release acceptance.
