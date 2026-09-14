# Native onboarding release-readiness check — 2026-09-14

Status: bounded fixes prepared in [PR #244](https://github.com/k-kostin/omavless/pull/244).
Not a stable release, marketplace publication or new R6 closure.

## Exact scope and identity

- Base: `1f6814557d1d0cc943ef4f600f4a46bc5e639c5b`.
- Helper-readiness implementation: `4b2c07895f3803b2f5b4cdc2f4c614491b5ec4cd`.
- Completion-response correction: `1549a1bc5216a663c072814ba6385bb29b26848e`.
- Environment: Try Omarchy ARM64 VM, native `0.8.0rc1-1` package.
- Installed native executable SHA-256:
  `e88f83496d4e301d41d02e1f53c4e19a21c02731dd8b5803d8cbdae8155d4807`.
  No Rust executable, runtime unit or host policy was changed by this work.

User goal: finish initial setup without discovering missing import tools only
after clicking Import. Import still targets the existing preview/confirmation
flow. The installation guide is fixed public navigation, not an installer.
Finish acknowledges onboarding; it does not activate login autoconnect, grant
network permissions, connect a tunnel or assert internet health. Main/Settings
layout and all profile/subscription content remain unchanged.

## Findings and corrections

1. Native onboarding hid the existing missing-picker guidance. Helper facts
   were refreshed by Settings, not first-use opening, and the validated
   clipboard-read capability was dropped from the QML projection.
2. The wizard now refreshes helpers on opening, enables clipboard/file import
   independently, and shows fixed installation commands plus Check again.
   Unknown facts are not treated as availability. Copy requires a working
   clipboard writer; a terminal-entry explanation remains when copying cannot
   work. Missing-picker text no longer promises that clipboard import works.
3. Native core setup lacked an actionable path to its instructions. It now
   opens the fixed installation guide; legacy path-based capability commands
   remain hidden. No sudo/pkexec/package execution was added.
4. Real installed Finish at `4b2c078` exposed an existing response-parser gap:
   `onboarding-complete` was submitted successfully but omitted from the
   action-response allowlist. QML therefore retained pending/unknown outcome.
   `1549a1b` recognizes only that already-supported action with the existing
   instance/operation/revision fences. New tests failed before the correction
   and pass afterward, including the actual Service exit handler.

## Evidence and limits

| Check | Result |
| --- | --- |
| Full local suite on corrected code | 228 Python tests: 226 PASS, 2 SKIP; all JS/QML contracts PASS |
| Focused onboarding | 13 PASS: helper combinations, admission, completion success/no-op/error/fences and Service pending cleanup |
| Settings readiness | 9 PASS; read/write clipboard facts remain independent |
| QML production import graph | Compile PASS using installed Omarchy imports; not a rendered-UI substitute |
| Shell syntax, manifest, diff, plugin validation | PASS |
| Isolated real wizard rendering | EN/RU ready/missing/partial/unknown helpers, core ready/missing/permissions, routing; constrained-height top/bottom inspected |
| Corrected unknown-helper wording | EN/RU recaptured and inspected offscreen on final code |
| Installed source update | PASS through `./install.sh`; no core restart |
| Loaded-code verification | A shell restart was required: matching files initially still rendered cached old QML. New installation-guide control then verified visibly |
| Installed wizard EN navigation | Core, routing and import steps inspected; pre-fix Finish failure recorded above |
| Corrected installed Finish | PASS on already-configured account: owner traversed Setup assistant and pressed Finish; completion flag true, pending false, unknown outcome false, no manual recovery |
| Fresh-account first use | NOT RUN: the existing account already had completion true; repeated Finish does not prove the first false-to-true transition |
| Actual clipboard import pipeline | PASS: installed plugin IPC invoked the same Service import path, synthetic profile preview rendered, Escape canceled, inventory unchanged |
| Actual file chooser and profile preview | PASS: installed importPick opened GTK portal through the native helper, synthetic file selected, profile preview inspected and canceled |
| Actual file chooser and subscription confirmation | PASS: synthetic subscription-URL file selected in the chooser list, subscription-specific confirmation rendered with URL hidden, canceled without fetch/add |
| Literal wizard import-button activation | Isolated real-widget callback/admission tests PASS; installed imports above entered through existing production IPC, not an automated wizard-button click |

The [synthetic rendering harness](../../tests/onboarding-visual/README.md) never
creates Service or accesses the store. Its Finish originally recorded a signal
without dismissal, confusing a human observer. It now dismisses and carries an
explicit test title/banner. This test-window defect and the independent actual
native response defect above must not be conflated.

All captures stay outside Git. Actual-user captures are private, even when a
path happens to contain `shareable`. Only sanitized classifications are recorded
here. No real profile/subscription was added, removed or renamed.

Import smoke used only synthetic TEST-NET/.invalid inputs outside Git. Both
file types reached explicit confirmation; neither was persisted. The portal
initially returned the previously selected profile after automated location
entry for the subscription file. That attempt is not subscription acceptance:
the repeat navigated to the directory, visually verified the selected file in
the list, and then inspected the correct subscription confirmation. No product
parser change was made to compensate for uncertain desktop input.

The clipboard test temporarily replaced clipboard text. Its initial test-only
wrapper timed out because the forked clipboard writer retained captured pipes;
the previous clipboard contents were not confirmed restored. Subsequent calls
used detached output correctly. At cleanup the clipboard reader returned no
text (exit 1), so restoration/explicit clearing is not claimed; no unrelated
clipboard content was overwritten during cleanup. This is a test-harness side
effect, not evidence of an application clipboard failure.

Last verified runtime: Routing, disconnected, native runtime active, no pending
action or manual recovery, Mihomo/TUN/auxiliary 0/0/0, plugin enabled, startup Off.
This is not evidence that optional enabled login autoconnect works.

## Remaining stable-release gates

Finish fresh-account first-use acceptance; retain earlier
[fresh package activation evidence](R6_FRESH_PACKAGE_ACTIVATION_2026-09-11.md)
without relabelling it a fresh graphical first-use pass. A disposable-account
check must preserve the owner's store and follow the
[host authorization barrier](HOST_AUTHORIZATION_ACCEPTANCE.md) for account/
service effects. Do not replay the old unguarded package/account wrapper.

The independent RC support-report parsing defect is tracked in
[PR #245](https://github.com/k-kostin/omavless/pull/245).
Final x86_64 package/install evidence must come from the Omarchy PC, not this
ARM64 VM. Stable artifact/version and owner-approved release/marketplace gates
remain those in the [release guide](../../packaging/release/README.md).
AUTO-1, DNS findings and V0 fixture gaps remain separate and unclosed.
