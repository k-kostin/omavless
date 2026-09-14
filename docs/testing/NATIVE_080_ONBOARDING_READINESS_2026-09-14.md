# Native onboarding release-readiness check — 2026-09-14

Status: bounded fixes prepared in [PR #244](https://github.com/k-kostin/omavless/pull/244).
Not a stable release, marketplace publication or new R6 closure.

## Exact scope and identity

- Base: `1f6814557d1d0cc943ef4f600f4a46bc5e639c5b`.
- Helper-readiness implementation: `4b2c07895f3803b2f5b4cdc2f4c614491b5ec4cd`.
- Completion-response correction: `1549a1bc5216a663c072814ba6385bb29b26848e`.
- Fresh-install geometry/keyboard correction: `db079c80aafa2e754c581ec7d28f0174710e4054`
  and `cba5a4d326ed0222e961350fafb981c73c9679f7`.
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
5. Owner-approved same-account clean-state acceptance exposed a real first-use
   layout defect: wizard height inherited the empty main inventory's short
   popup, hiding navigation below the fold. Only the visible wizard now asks
   for the normal 600-unit panel budget, still capped by the shell's available
   screen geometry. Ordinary main/Settings dimensions remain unchanged.
6. Wizard controls now use the shell's focusable Button, explicit modal-local
   Tab/Shift+Tab traversal and scroll-to-focus. Hidden/disabled steps are skipped,
   Enter retains the existing action, and Escape retains dismissal. Tests cover
   empty-inventory height, scroll bounds, inactive targets, local wrapping and
   non-Tab key preservation. Installed keyboard navigation confirms the result;
   merely adding activeFocusOnTab did not establish working shell integration.

## Evidence and limits

| Check | Result |
| --- | --- |
| Full local suite on corrected code | 228 Python tests: 226 PASS, 2 SKIP; all JS/QML contracts PASS |
| Focused onboarding | 19 PASS on final code: helper/admission/completion fences, Service cleanup, fresh-height and keyboard/scroll regressions |
| Settings readiness | 9 PASS; read/write clipboard facts remain independent |
| QML production import graph | Compile PASS using installed Omarchy imports; not a rendered-UI substitute |
| Shell syntax, manifest, diff, plugin validation | PASS |
| Isolated real wizard rendering | EN/RU ready/missing/partial/unknown helpers, core ready/missing/permissions, routing; constrained-height top/bottom inspected |
| Corrected unknown-helper wording | EN/RU recaptured and inspected offscreen on final code |
| Installed source update | PASS through `./install.sh`; no core restart |
| Loaded-code verification | A shell restart was required: matching files initially still rendered cached old QML. New installation-guide control then verified visibly |
| Installed wizard EN navigation | Core, routing and import steps inspected; pre-fix Finish failure recorded above |
| Corrected installed Finish | PASS on already-configured account: owner traversed Setup assistant and pressed Finish; completion flag true, pending false, unknown outcome false, no manual recovery |
| Clean-state first use in the owner's account | PASS for the executed supported initialize/activate/install and real wizard completion flow below; distinct from an empty OS/package installation |
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

## Same-account clean-state acceptance

The owner explicitly chose a reversible reset in this VM instead of creating
another account. Byte-verified private config/state/frontend backups stayed
outside Git in a same-user 0700 directory. Existing services were stopped with
separate real-terminal ready/settled acknowledgements under the
[host authorization barrier](HOST_AUTHORIZATION_ACCEPTANCE.md). Runtime
enablement was disabled and the plugin removed through Omarchy's supported
removal command, which retained its own frontend backup.

Whole original config/state/cache and same-session runtime/login receipt were
retained, not patched to fabricate completion or ownership. Relocation held the
existing migration lock without replacing its inode. The supported installed
`omavless setup initialize` created two default files with onboarding false,
startup Off and no ownership activation. One separately attended
`omavless cutover activate` then established an empty native owner, followed by
the ordinary frontend `./install.sh`. No package reinstall, dependency removal,
new OS account, OS-policy change or VPN connection was involved.

Final UI candidate: detached integration
`413521e82861036ff5b069d2ae954f66913d0668`, combining the existing #244/#245
integration with the two geometry/keyboard corrections above. All 24 installed
runtime-relevant files matched; the new loaded height and working keyboard
behavior were verified after a shell restart. Full suite on final owning-branch
code: 228 Python tests (226 PASS, 2 SKIP), all JS/QML contracts PASS; focused
onboarding 19 PASS. EN installed steps and RU real-widget constrained captures
were inspected. The Rust binary remained unchanged.

| Executed clean-state scenario | Result |
| --- | --- |
| Empty inventory and first opening | 0 profiles / 0 subscriptions, onboarding false, automatic wizard opening |
| Core step | Installed Mihomo/setup facts and guide visible; no grant or install invoked |
| Continue, then Skip for now | Real focused buttons activated with Enter; no routing preset mutation |
| Import step | Existing clipboard/file entry points visible; no test profile persisted |
| Finish later | Actual button activation changed canonical completion false to true, dismissed wizard, pending/unknown false |
| Close/reopen after shell restart | Completion remained true and wizard did not reopen automatically |
| Network/startup | Disconnected Routing, no Mihomo/TUN/auxiliary, no recovery, startup Off |
| Restore original private account data | Whole original files restored and byte-verified before start; independent post-start profile-store byte comparison PASS, 37 profiles / 1 subscription restored |
| Restore runtime | Independently observed disconnected Routing, no recovery/core/TUN, runtime active/enabled, plugin enabled, startup Off |
| Attended restoration completion record | Owner tentatively recalls seeing PASS; durable final success marker remains absent. This is human recollection, not verified script completion. No host effects repeated |

This closes the exercised graphical first-use/default completion question on
the installed ARM64 package. It does not claim a newly provisioned OS, absent
desktop-helper installation, all routing-preset choices, enabled autoconnect or
fresh VPN interoperability. Earlier [fresh package activation evidence](R6_FRESH_PACKAGE_ACTIVATION_2026-09-11.md)
retains its own exact identity and scope.

The restoration helper recorded successful whole-file restoration before its
attended service start. Subsequent independent reads confirm the original
inventory and healthy disconnected state, but the helper's overall completion
marker is absent and its terminal/process has exited. It is not known whether
the final human acknowledgement was interrupted or a subsequent verification
failed. Do not claim an entirely green guarded restoration invocation, fabricate
an acknowledgement, or rerun host effects merely to obtain its missing marker.
Asked what the terminal displayed before closing, the owner subsequently
reported that they thought it was PASS. Preserve that qualified recollection
separately from the independently verified restored state; it does not establish
the missing final ready/settled record or explain why the marker is absent.
Private original and test snapshots are both retained for recovery. Final #244
implementation/evidence CI at `4676a0ec617e65897555f469fe214149133a97b9` passed;
this subsequent clarification changes documentation only.

## Combined RC support-report check

The independent [PR #245](https://github.com/k-kostin/omavless/pull/245),
implementation `3bc48a43d5933292f630d38cfd4a4127ac7cef06`, corrects rejection
of the installed `0.8.0-rc.1` report version. It accepts only stable versions
and bounded positive-number RC versions, not arbitrary version text.

A detached local integration worktree combined #244 at
`211de881837bbd6cbe77d85ca9ea0d743d73e0a7` with that commit, producing
`f7d8ac8e28af7e7213cc622818220dcbb13a8ec5`. The combined candidate was
installed with `./install.sh`, all 24 runtime-relevant files
matched byte-for-byte, and the shell was restarted without touching the Rust
service or network. Focused support tests (18), onboarding tests (13) and QML
contracts passed. Each independent implementation also has green GitHub CI.

Actual installed Settings navigation used the existing keyboard controls,
visually verified focus on Copy report, and activated that button. The visible
success message appeared. The clipboard contained bounded schema-3 native
support JSON (2160 bytes) with the expected RC version and without private
identifier/credential fields. It also matched the candidate parser's strict
projection of a fresh actual native response. No raw report or clipboard
content is published.
The report remains in the clipboard as the explicit result of this smoke.
No Save-file dialog, VPN transition or host authorization was required.

This is combined UI acceptance, not authorization to merge, publish stable
artifacts or update the marketplace. The two PR branches remain independent.

## Release boundary

Final x86_64 package/install evidence must come from the Omarchy PC, not this
ARM64 VM. Stable artifact/version and owner-approved release/marketplace gates
remain those in the [release guide](../../packaging/release/README.md).
AUTO-1, DNS findings and V0 fixture gaps remain separate and unclosed.
