# 0.9.0-rc.1 offline package checkpoint

September 24, RC development only. No installed update, host authorization,
service/VPN effect, main update, tag, public asset, download pin or marketplace
change was performed in this unattended preparation.

Subsequent attended work is recorded separately in the
[installed pair and DNS report](RC_090_DNS_AUTHORIZATION_2026-09-24.md).
The local ARM64 pair is now installed; the historical preparation-only claims
below describe this earlier checkpoint, not the final VM state.

## Source and artifacts

Reviewed source: `5b5ed848e2464d1c4594788a490299dd8d17ee8c` (PR #292).
Cargo, lockfile and frontend use `0.9.0-rc.1`; Arch uses `0.9.0rc1-1`.
Installed `vercmp` confirms `0.8.2 < 0.9.0rc1 < 0.9.0`.

Both native builds in [CI run 36040744365](https://github.com/k-kostin/omavless/actions/runs/36040744365)
passed compilation, CLI loader/TUI capability checks and strict native package
inspection. Downloaded artifacts were checked locally against their SHA256SUMS,
source/version records, embedded binary digests and expected ELF machine.
ARM64 also passed the local strict archive inspector. x86_64 execution/inspection
belongs to its native CI job, not to emulation or a claimed installed PC pass.

| Artifact | SHA-256 |
| --- | --- |
| CI ARM64 `omavless-0.9.0rc1-1-aarch64.pkg.tar.zst` | `508de152172b3f9ef0829226f281ead4bcdae801175c8eae236d93b60551020d` |
| CI x86_64 `omavless-0.9.0rc1-1-x86_64.pkg.tar.zst` | `1833d284acce767fd4aeece0b3e4b20ae5395ab020b6897cc6e52a713911dc98` |
| Local Try Omarchy ARM64 `omavless-0.9.0rc1-1-aarch64.pkg.tar.zst` | `e44595335c69de58bbd1c29a0d43d6ea4df6dfd0202eb9f61f4a8cb0aa0b629a` |
| Common `omavless-0.9.0-rc.1-frontend.tar.xz` | `743d2744bf4af3607c36b6f8a3fc1dd2d5d75634680392b1e7fd19b73e44ddb5` |

The common frontend was reconstructed locally from the exact clean source using
the existing allowlisted Git-blob assembler. Its hash matches **both** native CI
build records. No untracked/private/developer Python file is shipped. Packages,
build provenance and frontend are retained outside Git; CI artifacts are also
available on the linked run for its retention period, not on GitHub Releases.

The local locked/offline build used Rust/Cargo 1.98.0, ARM64, one build job and
the same clean source. Its ELF hash is
`3c0f96acc6a48687bc2f48e95c3e0678a453be49e2e8539a4ad0b2ece1dcb3c0`;
strict archive inspection, source/version/binary identity, SHA256SUMS and
`tui --available` passed. The frontend hash is the same as both CI builds.
The local package is a distinct artifact from CI ARM64: do not substitute one
hash or installed acceptance record for the other. Select and pin the actual
archive used for the next attended gate; none has been installed here.

Bootstrap pins are deliberately empty. An absent-runtime RC frontend must report
release unavailable, not download public 0.8.2 as a substitute. The fixed future
stable setup version is 0.9.0; RC packages are installed only via explicit
reviewed local acceptance. No latest-version lookup or arbitrary URL is added.

## Validation and limits

[Exact candidate CI test](https://github.com/k-kostin/omavless/actions/runs/36040744422)
passed the combined developer suite (301 tests), ten terminal-fixture tests,
Rust workspace and focused rerun (1,103 successful test executions, 11 ignored),
strict Clippy, format and parity. The two native package jobs passed separately.
Local setup contracts, version mapping/order, manifest/plugin validation and
format checks passed. The first full local developer run hit two packaging
subprocess timeouts under VM load; it is not recorded as PASS. The parallel local
Rust test compile was stopped to release resources, not called a completed gate.
The local locked/offline ARM64 release build subsequently succeeded.
The full local developer suite then passed without the parallel Rust test
compile: 277 tests, two expected skips, Node/QML contracts PASS. The combined
CI count is 301 because it also includes #290's 24 native V0 tests, absent from
the independent packaging source branch. No test timeout was increased or
runtime safeguard bypassed to obtain this result.

The prior installed T2/#271 and native XHTTP evidence remains valid at its exact
developer runtime identities. It is **not** acceptance of these versioned package
bytes. DNS #270/#132, the unexplained first admission refusal and unavailable
V0 families are not fixed by this build. Stable 0.8.2 stays unchanged.

## Remaining attended gate

1. Confirm the actual installed rollback package/binary and preserve private state.
2. In a real terminal, use the existing per-effect `ready`/`settled` guard for
   disconnect, package/service update and restoration. No scripted passwords,
   acknowledgements, unattended retry or speculative cleanup.
3. Install the exact reviewed architecture package and common frontend; verify
   running byte identity, startup Off, retained profiles, Open app/TUI and the
   affected short connect/disconnect/restoration path. Reuse unchanged T2 evidence
   instead of repeating every manual screen.
4. Resolve the independent DNS/security decision and required host evidence before
   declaring the owner-required RC gates complete. Prepare stable packages/pins
   only after a separate release decision; main needs explicit owner instruction.
