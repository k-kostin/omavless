# Acceptance evidence index

## Start here: accepted native integration

The integration merged in #238; #240 adds the accepted subscription-row
refresh. See [current status](../roadmap/CURRENT_STATUS.md) for main and open
release work. Dated reports below retain their original pre-merge wording.

- [R6 local closure](R6_LOCAL_CLOSURE_2026-09-13.md): exact installed identities,
  accepted migration scope and deliberately unclosed product/host evidence.
- [Publication candidate](R6_PUBLICATION_CANDIDATE_2026-09-13.md): final local
  checks, integration of earlier Draft work and owner-controlled publication.
- [AUTO-1](../roadmap/LOGIN_AUTOCONNECT_FOLLOWUP.md): enabled Last/pinned login
  follow-up; default Off is not enabled-autoconnect acceptance.
- [Network investigation](R6_NETWORK_DIAGNOSIS_2026-09-13.md): preserved failed
  probes and limits on attributing a failure to DNS, provider, VM or runtime.
- [Main UI acceptance](R6_PROFILE_SELECTION_UX_2026-09-13.md) and the
  [UI/UX contract](../roadmap/UI_UX_CONTRACT.md): accepted interaction semantics
  and required rendered checks for future changes.

## Evidence groups

- [0.9.8 RC1 installed x86_64](RC_098_VM_2026-10-08.md): exact artifact identities,
  ordinary entry/navigation and preserved-Off warm/cold relaunch; retained
  failures and explicit ARM-installed, fault, human and public-release limits.
- [T4 root-observed private Abort process loss](../development/T4_ROOT_OBSERVED_PROCESS_GUARD.md#exact-head-execution--2026-10-04):
  exact-source five-case synthetic SIGKILL/re-entry and bounded whole-invocation
  preservation evidence; not installed restore, power-loss or product T4 PASS.
  The earlier [inner PASS / outer NONPASS](../development/T4_FIRST_ABORT_PROCESS_REENTRY.md#exact-head-execution-and-retained-nonpass)
  remains independent retained evidence.
- [0.9.7 RC ledger](../development/RC_097.md): maintenance selection and
  explicit package, installed-rendering and publication gates; not a new
  installed or public acceptance claim.
- [0.9.6 RC ledger](../development/RC_096.md): selected read-side scope,
  prior installed beta evidence and explicit final distribution gates.
- [0.9.5 RC ledger](../development/RC_095.md): scope-frozen T3/T4 selection,
  installed ARM64 beta counterpart, exact-artifact checks and retained limits.

- [0.9.5-beta.1 x86_64 Omarchy Dev VM](BETA_095_PC_VM_2026-09-30.md):
  exact offline triple, disconnected upgrade and installed T3/T4 read-side
  smoke, working Rule/TUN HTTPS, stale-row/revision fences and synthetic claims;
  owner-attended/public provisioning evidence remains separate.
- [0.9 managed-DNS release-pair VM migration](DNS_RELEASE_VM_MIGRATION_2026-09-28.md):
  exact-source x86_64 agent-run migration and DNS/TUN/HTTPS mode cycle; not a
  fresh installer or formal owner-attended gate.
- [0.9.0 PC continuation](../development/RC_090_PC_CONTINUATION_2026-09-25.md):
  current #270 ownership transfer, remaining DNS host gates and frozen release
  boundary; [installed ARM64 evidence](DNS_BROKER_TRY_OMARCHY_2026-09-25.md)
  and [PC x86_64 pre-install evidence](DNS_BROKER_PC_PREINSTALL_2026-09-27.md).
- [0.8.2 fresh x86_64 VM installation](NATIVE_082_FRESH_VM_2026-09-21.md):
  actual plugin-first setup with both application and Mihomo initially absent,
  onboarding/reopen and exact-source marketplace baseline; no new VPN claim.
- [0.8.2 artifacts and public downloads](NATIVE_082_ARTIFACTS_2026-09-21.md):
  immutable native packages and frontend pairing.
- [0.8.1 artifacts and public downloads](NATIVE_081_ARTIFACTS_2026-09-21.md):
  native x86_64/ARM64 builds, exact frontend/runtime pairing, public prerelease
  checksums and the separate remaining fresh-provisioning gate.
- [Physical x86-64 PC pre-install checkpoint](PC_080_PREINSTALL_2026-09-21.md):
  verified 0.8.0 package/frontend pair, exact-head local tests, preserved legacy
  state and explicit remaining attended installation/network gates.

| Scope | Entry point |
| --- | --- |
| Native-only installation and no hidden fallback | [Native payload](R6_NATIVE_ONLY_FRONTEND.md) |
| Installed Python-inaccessible domain/QML bridge | [Bridge matrix](R6_INSTALLED_NO_PYTHON_BRIDGE_2026-09-12.md) |
| Installed Python-inaccessible lifecycle | [Attended cycle, including failed HTTPS](R6_ATTENDED_PYTHON_ABSENCE.md) |
| Fresh package and default startup Off | [Fresh activation](R6_FRESH_PACKAGE_ACTIVATION_2026-09-11.md), [login Off](R6_INSTALLED_NO_PYTHON_LOGIN_OFF_2026-09-11.md) |
| Package recovery | [Attended archive recovery](R6_INSTALLED_PACKAGE_RECOVERY_2026-09-12.md) |
| Full Quit / removal | [Quit](R5_NATIVE_FULL_QUIT.md), [removal](R5_NATIVE_PLUGIN_REMOVAL.md) |
| Save As and support data | [Save As](R6_SAVE_AS_UI_CHECKPOINT_2026-09-12.md), [support schema](R6_NATIVE_SUPPORT_COMPOSITION.md) |
| Earlier chronology | [R6 ledger](R6_CLOSURE_LEDGER_2026-09-12.md), [dependency audit](R6_PYTHON_DEPENDENCY_AUDIT.md), [September 10 UI checkpoint](TRY_OMARCHY_UI_CONTINUITY_2026-09-10.md) |

Historical reports stay in place so commit links and negative evidence remain
auditable. An older "pending" sentence does not supersede a later recorded
acceptance or owner-approved deferral. Conversely, a current closure does not
turn a historical failed probe into PASS.

## Safe local test entry points

`./tests/run.sh` runs reference/launcher/JS/QML contracts;
`./tests/run-rust.sh` runs Rust formatting, workspace tests, Clippy and R0 parity.
Installed-core opt-ins use synthetic configurations, not private live fixtures.
The [QML component gate](../../tests/qml-load/README.md) compiles without
instantiating the plugin and is explicitly opt-in on an installed desktop.

Attended installed lifecycle, package recovery, login and interpreter-masking
scripts are **not** generic unattended test commands. Read each report's scope,
authorization and restoration requirements before running them. Never turn
this index into a bulk runner that restarts the user's VPN or asks for repeated
passwords. Passing contracts/compilation do not constitute visual acceptance.
