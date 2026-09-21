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
