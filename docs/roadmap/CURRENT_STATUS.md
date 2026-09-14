# Current delivery status

Updated 2026-09-14. This is the compact current-state entry point; the detailed
[delivery roadmap](../../DEVELOPMENT_ROADMAP.md) preserves the implementation
history. GitHub's actual main/PR state is authoritative for publication.

## Main and open work

- Owner-approved [Python reference retirement](LEGACY_RETIREMENT.md) preserves
  the complete pre-retirement tree in frozen `archive/python-legacy` at
  `aa5873783c019edc303a732e55ea8c85f1f0b090`. Native RC delivery (#239) and
  native-only defaults (#242) are merged. [#243](https://github.com/k-kostin/omavless/pull/243)
  integrates the accepted fixture-backed legacy removal, completing the four-step
  checkpoint. Leaving RC remains a separate owner decision; use GitHub for the
  final merge SHA, not historical preparation-only status below.

- Native integration [#238](https://github.com/k-kostin/omavless/pull/238)
  merged at `778647215deb1cb27e66e10e628fd0e78beee1af`. Its PR and main CI
  passed; #214–#237 are merged or explicitly superseded and their remote
  source branches were cleaned up. This integration is not still waiting to
  be published to main.
- Subscription-row refresh [#240](https://github.com/k-kostin/omavless/pull/240)
  merged at `3b82c4f66ca624d88a6d28b6bd68ddd96c11155a`, after exact-head CI
  and Try Omarchy ARM64 UI/live-refresh acceptance. Its compact main-page
  action reuses the existing Rust operation; VPN ownership is unchanged.
- Native **0.8.0-rc.1** assembly merged in
  [#239](https://github.com/k-kostin/omavless/pull/239) at
  `87844c1ffc24320285427a740ec43ffc6e68bee8`, following the
  [release assembly guide](../../packaging/release/README.md). Local artifacts and
  archive checks and the attended ARM64 installed update from source
  `4549f6921e908a951698617027b4971057278920` are recorded there. The new pair
  includes #240; earlier archived pairs retain their original identities.
  This is not a stable release or marketplace update.
- #30 and #135 remain separate Drafts. Do not merge or discard their evidence
  as part of repository cleanup. Marketplace text/screenshots and publication
  remain owner-controlled.

## Accepted native checkpoint

| Track | Current state |
| --- | --- |
| R0–R4 | Rust protocol, profile/domain/store and core foundations accepted; do not restart their migration |
| R5 / T1 | Rust owns the activated native application's state, mutations, background work and Mihomo lifecycle; restored QML is a client of that owner |
| R6 | Native installed path closed under the owner-approved scope, with deliberate installed Python-absence evidence |
| Enabled login autoconnect | AUTO-1 open; Last/pinned fresh-login acceptance is deferred, not PASS; default is Off |
| Network/DNS | Separate unresolved investigation; failed probes remain failures and are not hidden by migration closure |
| Native distribution | Reviewed local Arch package and explicit native-only frontend install accepted; no automatic marketplace migration or release implied |
| V0 / #30 | Draft; accepted exact XHTTP evidence preserved; missing-family/UDP-restricted evidence remains unavailable |
| T2 | Migration prerequisite satisfied for the accepted native path; client implementation is a separate task, not part of this integration |
| P4, K1, NixOS and other host tracks | Own design/fixture/security/host gates still apply; not promoted by R6 |

The [R6 closure](../testing/R6_LOCAL_CLOSURE_2026-09-13.md) states the exact scope,
tested runtime/frontend/package identities and exceptions. The
[integration evidence](../testing/R6_PUBLICATION_CANDIDATE_2026-09-13.md) records
the final local suites and the mapping of intermediate Drafts #214–#237 to
this tree. It is a dated preparation report, not a permanent publication block.
Do not merge those obsolete Draft implementations over the integrated result.

## What users install

The immutable published marketplace 0.7.0 snapshot remains
`69fe05b03129a23664fff3f8289821a7b7f80095`.
Neither a main merge nor the presence of Rust sources installs a native binary,
runs Cargo, grants capabilities, changes an ownership marker or enables VPN
startup on a user's machine.

The [native guide](../user/NATIVE_INSTALL.md) is the supported reviewed-candidate
path: install the package, explicitly activate once, then install its matching
frontend with `./install.sh`. The source/default installer is now native-only;
`--native-only` is an alias. The launcher refuses absent/legacy/unknown native
ownership without Python. Omarchy's clone-based installation does not install
the package or run this installer. The old Python implementation is archived;
remaining Python is independent developer/test/build tooling and frozen-reference
replay, not a supported fallback or second native lifecycle owner.

## Next work, in order

1. Retain the completed four-step reference retirement, native-only default and
   scoped R6 evidence. Do not reintroduce the Python runtime or rerun unchanged
   migration gates merely because test/docs cleanup merged.
2. Prepare stable release/package metadata
   and owner-reviewed plugin-page screenshots. Do not change the published
   0.7.0 marketplace identity without separate approval.
3. Address [AUTO-1](LOGIN_AUTOCONNECT_FOLLOWUP.md) and the
   [network investigation](../testing/R6_NETWORK_DIAGNOSIS_2026-09-13.md) with
   their actual reproducible host scenarios; no repeated password/dialog loops.
4. Scope a small T2 client checkpoint or another explicitly selected roadmap
   task on the existing Rust owner. Preserve accepted UI unless the task
   deliberately changes it under the [UI/UX contract](UI_UX_CONTRACT.md).

Historical acceptance reports retain their original heads and outcomes. Their
old "Python still owns production", "R5 incomplete" or "publication withheld"
sentences describe those earlier checkpoints, not the current native state.
Do not erase negative evidence, advertise untested host behavior or convert
deferred work to PASS while synchronizing documentation.
