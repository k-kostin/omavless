# Native 0.8.0: VM closure and final PC release gates

**Added 2026-09-15:** the [marketplace first-run checkpoint](MARKETPLACE_FIRST_RUN_2026-09-15.md)
is an additional distribution gate. Prior package-first acceptance did not test
a normal plugin clone without the native application. Preserve the evidence
below, but do not infer that only x86_64 runtime smoke and publication remain:
reviewed package pins and clean guided installation must also be accepted.

Owner direction, 2026-09-14: merge technically accepted work here on Try
Omarchy ARM64, finish applicable local release preparation, then run final
acceptance on the x86_64 Omarchy PC before marketplace publication. Marketplace
updates still require separate owner-present approval. This is not a stable tag
or a claim that every optional feature is complete.

## Integrated implementation

- Starting main: `1f6814557d1d0cc943ef4f600f4a46bc5e639c5b`.
- #244 onboarding merge: `9e8587847295a7a9a2dee64d7ab0ab004013cb73`.
  Tested PR head `a19e0a20565cc2a47b4da2df2a189f71465d7605`, CI PASS.
- #245 RC report merge: `6b9aa75dbe4ad76882b18d65895105ea85fbab93`.
  Tested PR head `3bc48a43d5933292f630d38cfd4a4127ac7cef06`, CI PASS.
- The combined merge's plugin, launcher, manifest, installer, templates,
  Cargo files and crates compare byte-identical to installed integration
  `413521e82861036ff5b069d2ae954f66913d0668`. No reinstallation or repeated
  manual acceptance is required just to substitute the merge SHA.
- Focused checks on combined main: 19 onboarding, 18 support-report,
  9 Settings readiness; QML contracts PASS.

## VM evidence retained, not rerun ceremonially

| Gate | Evidence / limitation |
| --- | --- |
| Rust-only normal path / scoped R6 | [Accepted closure](R6_LOCAL_CLOSURE_2026-09-13.md); not a new migration queue |
| Reference retirement | Frozen archive retained; native-only default and independent fixtures integrated |
| Installed native ARM64 RC update | [Actual package/binary/unit identity and private-state preservation](NATIVE_080_RC_PREPARATION_2026-09-13.md#attended-rc-update--2026-09-14) |
| Clean application first use | [Real same-account initialize/activate/frontend reinstall](NATIVE_080_ONBOARDING_READINESS_2026-09-14.md); existing native package and desktop dependencies, not a pristine OS |
| Wizard completion | Actual Continue / Skip / Finish later; completion persisted after shell restart; startup stayed Off |
| Missing helper UI | Deterministic readiness/admission and real-widget EN/RU rendering; optional dependencies not silently installed |
| Import / confirmation | Production clipboard/file paths and real chooser reached explicit profile/subscription confirmation; no persistent test subscription added |
| RC support report | Installed Copy report PASS with strict schema-3 safe projection; RC version no longer rejected |
| Original-account restoration | Original profile bytes independently verified, inventory restored; healthy disconnected runtime, enabled plugin, no recovery/core/TUN |
| Restoration helper's final acknowledgement | Durable final marker absent; owner tentatively recalls PASS. Not promoted to fully verified guarded-script completion; no retries to manufacture evidence |

Private backups and captures are outside Git. Preserve both original and test
snapshots until the owner no longer needs recovery. No private names, record IDs,
URIs, credentials or screenshots belong in the public release report.

### Post-merge CI harness finding

Main run `34849402594` at `f11ed070845c4650f4dec851114c4391800321a5`
failed in `desktop_dialog_cancellation_retains_exit_three_without_error_output`.
The panic was the test helper's stdin `write_all(...).unwrap()` returning
BrokenPipe, not a failed cancellation/cleanup assertion. The negative case
deliberately sends an extra CLI argument, which is rejected before stdin is
read; writing an unused locale raced that correct early process exit.

The narrow correction sends empty stdin for that argv-rejection case only.
Valid chooser calls still receive their locale and must return cancellation
code 3 without output; the invalid call still requires code 2, empty stdout and
no private argument in stderr. No production code, timeout, error suppression,
test skip, GUI interaction or host behavior changes. The failed run remains
visible; subsequent exact-head CI must pass before this correction merges.

## Remaining release preparation

The offline tooling checkpoint adds explicit stable assembly without changing
Cargo/lock/manifest versions, Rust code, runtime units or the installed package.
Local full suite: 235 tests, 233 PASS / 2 existing SKIP; all JS/QML contracts
PASS. Focused package/release/inspector suite: 45 tests, 44 PASS / one root-only
SKIP. Real offline RC and stable Arch archive assembly/strict inspection and
isolated extracted-frontend fresh/update tests ran on this VM. Python was absent
from those installers' PATH; no real installation, activation or service action
occurred. Shell syntax, Python compile, manifest, plugin validation and diff
checks passed; 74 local documentation links resolved. GitHub records the exact
PR/main CI results. These are developer-tool tests, not a stable binary gate.

1. Retain the tested explicit stable-assembly boundary: the default RC route
   still refuses stable versions; `--stable` refuses RC input. Schema 3 identifies
   stable package metadata, not an installed-state migration.
2. Approve one final source/version commit for `0.8.0`, updating Cargo/lock and
   frontend manifest coherently. The owner subsequently authorized this
   preparation; follow the [final candidate report](NATIVE_080_FINAL_CANDIDATE_2026-09-14.md)
   for its exact source/artifacts and actual installed results. Earlier RC
   artifacts are unchanged, not relabelled.
3. Build each architecture from that clean source with the pinned locked
   toolchain, retain build provenance and inspect both package/frontend archives.
   Existing RC binary/archive hashes must never be relabelled as stable builds.
4. Test the real final version's installed package/update and support response
   where version/packaging materially changed. The offline synthetic archive
   test is not actual OmaVLESS installed acceptance. Preserve unchanged R6/UI
   evidence rather than restarting the entire migration matrix.
5. Finish user release notes, native installation/0.7.0 migration instructions,
   known limitations and owner-reviewed page/screenshots. `plugin add` alone
   does not install the Rust package or activate ownership; the marketplace
   must not advertise an automatic seamless migration.

## Final x86_64 Omarchy pass

ARM64 final preparation is now executed from source
`b7fd0a99b8b169f0933e5f43ea4389642015193a`: actual final package/frontend
inspection, installed ELF/unit/frontend identity, data preservation, safe
support response and separately guarded Full VPN HTTPS/disconnect/restoration
PASS. See the [final candidate report](NATIVE_080_FINAL_CANDIDATE_2026-09-14.md)
for hashes, counts and the qualified package-script acknowledgement. Later
evidence-only commits do not change that artifact source. Do not reopen the VM's
unchanged onboarding/R6 matrix; build the matching x86_64 package and run the
host-specific checklist below. Both architectures use the same QML frontend.

Fetch first and select the exact reviewed source and matching x86_64 artifact,
not a private stale handoff or an ARM64 binary. If testing RC first, record it
as RC; a later stable build still requires its affected exact-artifact checks.

1. Preserve a private backup and compatible recovery archive. Identify current
   native versus legacy ownership through supported read-only commands.
2. Build/inspect package and matching frontend, verify architecture, versions,
   checksums, real running ELF and user units. Record toolchain/Mihomo versions.
3. Use [native install/update guidance](../user/NATIVE_INSTALL.md) for the actual
   starting state. Do not initialize over existing data, edit ownership markers,
   skip dependencies, or blindly repeat activation. Fresh-account testing may
   preserve/relocate complete data only with explicit owner scope and recovery.
4. Confirm plugin loads, EN/RU main/Settings/import/guide are usable, picker and
   clipboard imports reach correct confirmation, and Copy report succeeds.
   Verify no duplicate persistent subscription is created for a smoke test.
5. With an existing valid private fixture, check connect/disconnect and actual
   selected profile/mode; distinguish Routing from Full VPN egress. Verify one
   runtime/core/TUN, responsive private Unix controller and no attributable TCP
   controller. Run a bounded benign HTTPS/DNS check; preserve failure categories
   rather than declaring success from an enabled toggle.
6. Check current release's startup-Off default, shell restart neutrality,
   confirmed Quit/reopen and cleanup where changed or required by this host's
   release gate. Do not enable Last/pinned just to close this checklist.
7. Record exact source/package/frontend identity and a redacted result matrix.
   A physical NIC/suspend claim requires its own actual test; it is not inferred
   from routine PC smoke or VM evidence.

Use the [human authorization barrier](HOST_AUTHORIZATION_ACCEPTANCE.md) for
each potentially authorizing host effect: real terminal, ready before, settled
after, no automated acknowledgements, forced deadline or compensating retry.
Passwords belong only in the system/terminal prompt. Stop on unresolved
authorization or manual recovery.

## Publication decision

Only after the exact final artifact checks and owner approval: publish the
reviewed version/tag/assets, then separately approve the marketplace revision.
No automatic upload, AUR/NixOS publication, new protocol, firewall helper or
security-policy relaxation is part of this checkpoint.

AUTO-1 (optional enabled login, default Off), recorded DNS/provider findings,
V0 missing-family evidence and #135 remain separate, explicitly unresolved.
Do not merge their stale branches as release cleanup or advertise their gaps
as validated. A reproducible failure of normal default connectivity on the
final PC candidate blocks its release; scoped deferrals do not excuse it.
