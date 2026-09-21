# Current delivery status

Updated 2026-09-21. This is the compact current-state entry point; the detailed
[delivery roadmap](../../DEVELOPMENT_ROADMAP.md) preserves the implementation
history. GitHub's actual main/PR state is authoritative for publication.

## Main and open work

- **0.8.2 preparation, #264:** #263 is merged. New versioned native packages
  for both architectures passed CI; exact frontend pins are recorded with owner authorization for a
  GitHub prerelease, not stable promotion or marketplace submission. The
  installed active connection is not changed by this build/release work.
  See the [0.8.2 artifact checkpoint](../testing/NATIVE_082_ARTIFACTS_2026-09-21.md).

- **Post-0.8.1 fixes, #263 (merged):** correct Full Quit's installation preflight and
  the installed-review findings for subscription keyboard focus and EN/RU
  name-search copy. The runtime fix has attended physical-PC disconnected
  Quit evidence; UI-only follow-up was checked without interrupting the
  owner's active connection. See the [follow-up record](../testing/R5_NATIVE_FULL_QUIT.md#september-21-release-follow-up-263).
  These fixes are not in the immutable public `v0.8.1` assets. A later release
  needs a new identity and matching packages/frontend; do not overwrite the
  old tag or claim fresh guided provisioning passed from this UI review.

- **0.8.1 published as a prerelease:** retain the already published `v0.8.0`
  prerelease unchanged. #258–260 and #261 are merged; the latter integrates setup and the
  corrected release candidate. Both native packages now pass CI at runtime source
  `98e0b275ae43d6e7d7901b58fab31457b542f878`; setup pins contain their actual
  hashes rather than the old 0.8.0 runtime. See the
  [0.8.1 artifact checkpoint](../testing/NATIVE_081_ARTIFACTS_2026-09-21.md).
  The public tag/frontend is `20b5e6c4f3ef466207d37306d4a70fe5876162f6`;
  all six release assets were downloaded anonymously and verified against the
  retained hashes. #249 and #257 are integrated and their branches are removed.
  Fresh guided provisioning remains the stable-promotion gate. Host evidence
  is not inferred from publication. Marketplace submission remains withheld.

- **Release finalization, September 21:** physical x86-64 found and corrected
  foreign-TUN recovery, connected-profile replacement and misleading subscription
  feedback in #258–260. The combined development candidate is now installed;
  attended package/update/reconnect, preserved profiles/foreign VPNs, fresh
  Connected/Rule and user-confirmed operation are recorded in the
  [PC continuation](../testing/PC_080_PREINSTALL_2026-09-21.md#corrected-installed-candidate--september-21).
  The matching QML required a graphical-shell restart to replace cached JS;
  this did not restart the native connection. Release integration reconciles
  #249's setup and #257's historical PC evidence with these fixes. Existing
  `v0.8.0` prerelease/tag/assets still identify the earlier runtime, not this
  installed candidate. Final corrected artifacts/pins and guided-install gates
  remain distinct. The owner authorizes main/release finalization but explicitly
  withholds marketplace submission until a separate instruction.

- **Historical release progress, September 16:** #250–253 are merged (user-facing content,
  preserved developer docs, runtime/frontend pairing and x86_64 package build).
  [v0.8.0](https://github.com/k-kostin/omavless/releases/tag/v0.8.0) is now public
  as a **prerelease / not latest**, with actual ARM64/x86_64 archives and the
  reviewed common frontend. Anonymous asset downloads and hashes PASS. #249
  has real pins and remains Draft for guided installation acceptance, not for
  unavailable release files. Physical x86_64 acceptance is planned, not PASS.
  Use the [current first-run report](../testing/MARKETPLACE_FIRST_RUN_2026-09-15.md)
  and [ready-package PC gate](../testing/PC_080_RELEASE_GATE_2026-09-16.md).
  Owner authorizes advancing release/marketplace after those gates; the old
  marketplace snapshot remains unchanged until its separate update approval.

- Owner-authorized final `0.8.0` source/package preparation follows the merged
  #244–#247 fixes and offline release tooling. This version preparation is not a
  tag or publication by itself; the actual later prerelease is recorded above.
  Use the [final candidate report](../testing/NATIVE_080_FINAL_CANDIDATE_2026-09-14.md)
  for actual ARM64 build/update results and outstanding x86_64/owner gates;
  old RC artifacts keep their original identity.
  Final ARM64 source `b7fd0a99b8b169f0933e5f43ea4389642015193a` now has
  full static/Rust/CI, real archive inspection, installed final binary/frontend
  identity, safe report and guarded Full VPN HTTPS/disconnect/restoration PASS.
  The update script's mistyped final acknowledgement remains qualified in the
  report; read-only checks prove its installed outcome and unchanged private
  data. Do not repeat package installation just to replace that transcript.

- Owner-approved [Python reference retirement](LEGACY_RETIREMENT.md) preserves
  the complete pre-retirement tree in frozen `archive/python-legacy` at
  `aa5873783c019edc303a732e55ea8c85f1f0b090`. Native RC delivery (#239) and
  native-only defaults (#242) are merged. [#243](https://github.com/k-kostin/omavless/pull/243)
  integrates the accepted fixture-backed legacy removal, completing the four-step
  checkpoint. Final-source preparation is now authorized; publication remains
  a separate owner decision. Use GitHub for the
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
- Native onboarding [#244](https://github.com/k-kostin/omavless/pull/244)
  merged at `9e8587847295a7a9a2dee64d7ab0ab004013cb73`; RC support-report
  parsing [#245](https://github.com/k-kostin/omavless/pull/245) merged at
  `6b9aa75dbe4ad76882b18d65895105ea85fbab93`. Their combined runtime/frontend
  bytes match the already installed Try Omarchy candidate. Clean same-account
  first-use onboarding and completion persistence passed; original private
  profile bytes were restored and verified. The missing final restoration
  helper marker and qualified owner recollection remain documented, not a
  reason to repeat destructive setup. See the
  [release handoff](../testing/NATIVE_080_RELEASE_HANDOFF_2026-09-14.md).

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

**Additional release gate, 2026-09-15:** installed-candidate R6/ARM64 acceptance
did not cover a fresh marketplace clone with no `/usr/bin/omavless`. The new
[first-run setup checkpoint](../testing/MARKETPLACE_FIRST_RUN_2026-09-15.md)
adds a runtime-independent setup page and explicit terminal provisioning.
Published architecture-specific package pins and download checks are now ready;
real clean package install → activation → onboarding acceptance remains required.
Do not tell a new
agent that only the final x86_64 runtime smoke remains. This does not reopen the
unchanged R6 migration or publish the marketplace snapshot.

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
2. Finalize the corrected runtime/setup release after the recorded PC update:
   retain the historical ARM64 evidence, build and inspect new matching artifacts,
   replace setup pins only with real reviewed hashes, and finish affected
   clean-provisioning checks. The old public prerelease cannot be promoted as
   though it contained the PC fixes. Resolve the new release identity without
   moving an existing tag or overwriting published bytes. Marketplace submission
   and the old 0.7.0 listing remain on hold for the owner's separate instruction.
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
