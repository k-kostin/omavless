# Guided reset after incomplete setup

Owner-requested follow-up to #772 and the fix.7 user-path review. Owning
implementation/reconciliation PR: [#790](https://github.com/k-kostin/omavless/pull/790).
This report does not authorize main integration, a release or an issue reply.

## Scope and identities

- Main baseline: `ed0346c5b224c503ec4871f43d8e65fe367a632d` (fix.7).
- Original tested helper: `0baf317f1eef09847f254a66e08268dd32155157`, SHA-256
  `86c238f4a9f7b2d15fb0d530782fd890c635cd8c20eac20c1fb55cfbc38a39c1`.
- Successor helper: `621502689b112760dc20b378ac2da7f3545d8e9b`, SHA-256
  `33eabf6c5e08e03df181d555a22ea7d29e9b9d4e4f0dd463fd521dbbb0794b8a`.
- Documentation reconciliation: `570ffe2e7f8e3eddd93e0a48741ff49660f36576`;
  successor also removes only the proven empty root enrollment directory with
  `rmdir`. Original installed results below retain their original helper identity.
- Owner-expanded guided successor: `7c65847156756b25f1c0097f713e75535383d59e`,
  SHA-256 `2f1ffd56e49ad0d8c174d5e5af84006564d5ef32d9a3297c7e13097aa40417ba`.
  Zero arguments select the guided journey, not the compatibility phase CLI.
- Final metadata/editorial delta: `73c41cf4b8d6dc4675a50c643bdab82d00e05c0a`,
  SHA-256 `1bc9d1361dadeb76963a9cdf6bf79a4374f1329bcd7b708ad2c870d6e1fe02e3`.
  It additionally requires the pinned unit's own `root:0644:nlink1` metadata
  and simplifies progress wording; existing guided sessions retain their source.
- No changes to the native app/DNS producer or immutable fix.7 packages.

The original two-phase helper is a terminal reset for public 0.8.2/0.9.8 installations
which **never connected a VPN**. The REMOVE screen explicitly states this limit
and deletion of this account's own profiles/subscriptions/settings. It is not
the all-state, data-preserving 0.9.9 remover planned in #774. Connected-ever,
unknown/experimental builds, custom XDG locations and unknown ownership remain
unsupported in that original scope; no forced kill, guard bypass or journal/socket deletion is added.

The owner then requested a single terminal entry, permanent data deletion with
one short warning, simpler editing and lightweight waiting indicators. The
guided successor retains the original partial/never-connected lanes and adds
normal live managed 0.9.8 removal only after this invocation's authenticated
Full Quit or actual watcher `runtimeStopped:true`. A watcher no-op cannot earn
that lane. Original broker release proves captured DNS restoration/readback
before journal/FD release; independently verify that fact in the connected gate.
Already stopped used installations and connected-ever 0.8.2 remain excluded:
missing current original cleanup proof is not repaired by deleting profiles.

A bounded same-account record outside purged data selects preparation/restart/
removal screens, never root authority. It is flushed before dependent effects.
Same-boot continuation only offers restart; uncertain, malformed, wrong-helper
or changed records refuse. No automatic login execution/reconnect is added.
The ASCII spinner observes the original child job and cannot replace its exit
status. Legacy explicit phase commands retain their never-connected restriction.

The original two-phase entry downloads its exact helper into a newly created
`.cache/omavless-maintenance/reinstall-098` subdirectory and verifies its hash.
It does not depend on a development checkout or an existing download directory.
After REMOVE consent that original phase CLI detects a running app and asks STOP separately
and uses existing authenticated/fenced native shutdown. Enabled widgets use
the original hello fence plus full Quit; removed/disabled widgets use the fixed
removal consumer. A watcher no-op never earns shutdown by itself. Uncertain
outcomes stop once and retain the original setup exclusion.
The new guided entry uses REMOVE to confirm closing the application as well;
it does not ask STOP. Its reboot/continuation stays inside the one entry flow.

Both phases hold the existing exclusive setup directory. The root operations
check trusted helper/unit bytes, the intended enrollment, no owned TUN/native
process, empty journal/FD store and ordinary package guards. Absent DNS and
pre-enrollment are separate classifications, not ignored guard failures.
The original broker's clean stop is proved before the deliberate reboot;
the reboot is not evidence of DNS cleanup. After logging back in, REMOVE
confirmation, guarded revocation and ordinary removal precede deletion of only
the three checked private application directories. Shared stock Mihomo, other
VPNs/accounts, unrelated dependencies and firewall rules are not reset.

## Source and documentation checks

- Bash syntax and `git diff --check`: PASS.
- Production validator/shutdown/parser/classification fixtures: 61 PASS.
- Fixed root-branch fixtures with isolated paths/inert commands: 39 PASS,
  including empty-directory cleanup, hidden entries, unsafe metadata/symlinks
  and failing `rmdir`. The absent-DNS early exit is isolated before asserting
  filesystem deletion; it cannot skip that assertion.
  Correct unit bytes with a foreign owner, writable mode or hardlink also refuse
  before stop/revocation effects.
- Guided phase/record/consent fixtures and inert real-PTY spinner: 32 PASS.
- Guide: three Bash blocks, exact helper/hash, ordinary main add, short reset
  scope, no external UI Quit prerequisite: PASS.
- Anonymous HTTPS helper bytes match the pinned SHA-256: PASS.
- README addition and support-draft command blocks must match the final branch
  documentation; the reply remains local and unsent.
- Local combined `tests/run.sh` at guided checkpoint `44cc208f`: 776 Python tests PASS, two existing skips,
  all JS/QML suites PASS. This is not a Rust rebuild or installed removal.
- Final combined source run used checkout `fe355a4ade1087757c914ea82862bea47a378469`
  (helper `73c41cf4`, tests 61/39/32), plus the reviewed README/guide/evidence
  wording delta: the same 776 Python tests/two skips and all JS/QML suites PASS.
  Final committed-head and hosted Rust status are recorded in #790, not inferred
  from this source-only run.
- Hosted first implementation checkpoint `0baf317f` failed the expected old
  guide/hash consistency check; retain FAILURE. Documentation was corrected in
  `570ffe2e`; later hosted CI is independently recorded in #790.
- Independent architecture/delta review accepted bounded VM testing and
  subsequently approved code/docs at `8e9dd884`, with installed gates separate.
- Independent review approved successor helper SHA-256 `33eabf6c` for bounded
  VM testing; successor installed gates remain separate from original results.

## Installed acceptance matrix

One VM operator at a time: initially VM master, then the explicitly delegated
acceptance operator. Only the dedicated PC x86_64 development guest is in
scope; no physical-PC or Mac/ARM mutation is authorized by this VM handoff.
Preserve development/private material before fixture administration. Initial
fixture construction is separate from the actual user helper journey; no
manual repair or pre-shutdown may replace a missing edge inside that journey.

| Starting state / actual entry | Result |
| --- | --- |
| Fresh main/fix.7, running, disconnected, never-connected; cancel REMOVE | PASS: original packages, registry, service PID/enablement, data and network unchanged; helper's setup lock absent. |
| Same baseline; REMOVE then cancel STOP | PASS: same unchanged-state comparison and no remaining helper setup lock. |
| Same baseline; REMOVE → STOP → preparation → guest reboot → remove | PASS: helper itself completed native shutdown; intended packages/widget/three private directories/enrollment removed; unrelated packages/network unchanged and HTTPS available. |
| Original public 0.9.8, enrolled never-connected partial setup | PASS: exact guide bootstrap downloaded/verified the immutable helper anonymously; preparation, reboot and removal completed without a bypass or manual repair inside the journey. |
| Ordinary widget remove, then helper with checkout absent | PASS with original helper: preparation, reboot and removal completed. Normal unload may already stop the app; this does not claim installed live-watcher shutdown. |
| Public 0.8.2 without DNS companion | PASS with 62150268: prepare/reboot/remove captured exit 0; separately installed stock Mihomo binary/config/units and unrelated packages/network unchanged. |
| Packages installed before first DNS enrollment | PASS with 62150268 on separately restored fixture; original 0baf sudo timeout remains unsuccessful and its disk preserved. |
| 62150268; enrolled original 0.9.8 running/disconnected, empty root directory cleanup | Behavioral completion and independent postconditions PASS. Prepare printed completion and original owners/lock were gone; the PTY supervisor had timed out at gum before continuing that same channel, so its outer exit 1 is not a captured helper exit. Reboot/remove exit 0; guarded revoke and root directory absence independently confirmed. |
| Guided 7c658471; 0.8.2 cancel REMOVE | PASS in actual guest terminal: exit 1, no progress record/setup lock, packages/data unchanged. |
| Guided 7c658471; 0.8.2 restart-later/same-boot resume | PASS: prepare exit 0; same invocation cancelled reboot; same-command resume showed only restart with unchanged record identity/bytes. Actual graphical REBOOT worked without administrator substitution. |
| Guided 7c658471; 0.8.2 same-command removal after reboot | PASS: original exit 0, progress record cleared, packages/private state/runtime absent; stock core and unrelated packages/network unchanged. |
| Guided 7c658471; original public 0.9.8 actually connected | PASS: real owned core/TUN and broker FD store 1, DNS differs from pre-Connect baseline. Full Quit inside the helper removed runtime/core/TUN and released FD store to 0; exact DNS comparison matched the original baseline before root stop or reboot. Actual spinner frames inspected; no VPN Internet-health claim. |
| Guided 7c658471; same connected fixture through reboot/remove | PASS: original prepare/remove exits 0, real GUI REBOOT; DNS remained at baseline, intended app/DNS/private/runtime/receipt/lock/session state absent and unrelated packages unchanged. |
| Final 73c41cf4; actual root metadata admission and guided removal | PASS on a separate current fix.7 running/disconnected fixture: literal checked bootstrap, actual Full Quit and root metadata admission, original prepare/remove exits 0, genuine GUI REBOOT, same-command removal and independent absence of intended packages/private roots/enrollment/units/session. DNS, stock core and unrelated packages unchanged. |
| Reinstall through ordinary main add after reset | Installation/setup PASS: ordinary add exit 0, exact main ed0346c5, actual INSTALL/pacman/DNS, terminal completion and closure acknowledgment, Core ready, onboarding completed without a profile. No preseed, tag/candidate substitution, shell reload or fixture repair inside the journey. Actual GUI setup exit was not separately captured; visible completion and postconditions are the evidence. Import is not accepted: no supported guest file picker; empty/unreadable clipboard produced an explicit refusal. |
| Connected-ever 0.8.2, already stopped used installation, lost ownership or experimental Restore build | Excluded; not an accepted generic uninstall. |

Installed ARM64 acceptance of this changed remover is not claimed. The owner's
Mac first-use result and shell-reload result concern main/fix.7, not this helper.
The final installed native producer was `d4d4310be6ade7e7e166f8b672cf90c7ad3e5b3b`;
the installed app/broker payloads matched the public fix.7 pair. Final independent
state was disconnected, startup Off, zero profiles/subscriptions, onboarding
complete, broker FD store 0 and no owned TUN. The original guest idle preference
was restored and exclusive VM custody released. Private original receipts and
actual screenshots remain outside Git; no private logs or profiles are attached.

## Practical retrospective

### Owner rejection: incomplete product scope and redundant success controls

The owner rejected the scoped remover as completion of a full deinstaller.
Its old installed PASS rows remain valid only for their named fixtures, not
for the requested normal already-used/stopped and interrupted-removal journeys.
The primary let implementation limits redefine the goal, then counted safe
refusals as an acceptable product boundary. Correction: execution policy now
requires actual used/running/stopped, widget-absent and interrupted-phase
continuations before declaring a full remover ready. User history attestation
and generic support referral cannot substitute for those implementations.

The same review missed Check again/Installation guide on the successful Core
ready screen: callbacks and rendering were tested, not action relevance.
Correction: the UI contract requires explicit allowed/absent actions per state,
production-expression regression tests and installed ready/non-ready review.
These are changes to the existing policy/tests, not a new ceremonial audit.
Successor implementation and affected installed results must be recorded
separately below; earlier source/helper identities are not silently promoted.

### Guided terminal decisions

Goal: remove the selected supported old installation, not the OS or another VPN.
REMOVE confirms permanent own-data deletion and the required graceful shutdown;
REBOOT is a separate computer restart with a save-work warning. A same-boot
record selects only the restart screen. Waiting frames observe the original
job; original completion plus independent state checks decide success.

The primary inspected actual private English guest-terminal captures for REMOVE,
preparation and same-boot restart-only continuation at 1890×2080, plus connected
shutdown spinner and completion at 1890×1333. The warning,
empty-line grouping and next input are readable; no dense authority/enrollment
essay is shown. The final delta replaces misleading DNS-preparation wording
when no DNS package exists and introduces the helper purpose at first mention.
This standalone maintenance terminal remains English; QML/Russian catalog and
native package prompts are unchanged, not newly claimed localized here.
The primary also inspected the final actual preparation/restart offer and literal
main Core ready captures. Their text matches the advertised path; final removal
completion is original PTY output with exit 0, not a claimed GUI screenshot.

The first live-connected design was unnecessarily excluded while the original
protocol had not been re-examined. Distinguish missing installed evidence from
an intrinsic product limit: review the actual original release/restore contract,
then earn a positive gate. Do not infer that deleting saved profiles supplies
missing DNS restoration proof in unsupported legacy or stopped-used states.

The earlier reset was exercised from an expert-prepared stopped baseline, then
advertised with an external Quit prerequisite. That did not earn a useful
shutdown offer for a blocked first-use user. Correction: put that offer inside
the helper and test both cancellations plus the original running entry. The
execution policy now explicitly requires this acceptance unit.

The same session also exposed a separate identity/workflow problem: a private
Restore development package on the Mac carried the public `0.9.8-1` label.
Widget remove/add preserved it. This helper rejects unrecognized app bytes
before any removal instead of trusting that label. Distinct experimental
package versions remain a producer-workflow follow-up, not something this
shell change or the existing immutable packages already fixed.

Finally, matching files on disk did not establish loaded QML: the owner saw the
old verbose onboarding until a normal shell restart. Preserve the existing
loaded-screen check/reload guidance rather than reinstalling native packages
to refresh a frontend. README, guide and support draft now name the same
ordinary shell command and its bar/panel reload consequence.

## Full-removal replacement and ready-wizard correction

The owner rejected the earlier helper's scope as a complete deinstaller.
`tools/clean-reinstall-098.sh` and its rows above are retained historical source,
not the advertised replacement. `tools/uninstall.sh` is the new zero-argument
entry. It detects the installed public version and state, never asks whether
the owner used VPN before, and warns once that own saved data will be deleted.

The replacement stages a verified digest-named continuation command outside the
widget, uses the existing fenced native shutdown where available and suppresses
only future own activation. It retains a currently quarantined owner/journal
until an explicit restart. The changed boot is a declared recovery boundary,
**not** proof that an interrupted original DNS rollback succeeded. Fresh package,
intended-UID, process, resolver, TUN and rule/route checks precede guarded
revocation and ordinary package removal. No foreign core, VPN configuration,
network rule or unknown journal is forcibly deleted.

Private progress selects reconciliation, not root authority. Root owns one fixed
flock through the complete transaction; delayed old authorizations are fenced
by attempt, phase and boot. A new user invocation crosses that barrier before
reclaiming its exact setup directory. Setup/mask identities are saved before
publication, and final shared-lock release precedes deletion of the receipt.
Preparing, prepared and partially removing are separate resumable states.

Source checkpoint `24b3f800d7e60456c2307a4b90c76fd466c1fb37`:
helper SHA-256 `6fd3c72c49ded0ecdf17dd8a4807ec036e243c8e5a5573a2bd335d7401bc494f`.
Independent Astra review admitted bounded installed testing, not product PASS.
66 inert production-function/custody fixtures passed without real root, service,
package, resolver or network effects. They include queued old authorization,
an already-admitted root worker retaining its flock after frontend replacement,
durable continuation, partial mask/setup publication, final-release interruption,
redirected ancestors, foreign enablement links and malformed resolver inventory.
Combined source run: 776 Python tests passed, two existing skips, all JS/QML
suites passed. New remover installed gates remain separate below; 24b3f800
preflight found a real resolver-schema mismatch before any removal effect, so
that checkpoint is not accepted as an installed remover.

Successor `2630b025a9101f22afa186f2cf3e57a056a549fa`, helper SHA-256
`a8e99d291b876268ee0ab00989e6140204fb5968c7603dda1e62c9bc655effe6`,
corrects only that resolver global-object schema. The positive fixture now uses
the actual measured field/types with empty synthetic address/domain/scope values;
the fabricated `servers`-only global object is an explicit negative case.
Independent delta review admitted bounded VM execution. 82 inert cases now
include actual preparation/removal branch source, guard-before-revoke ordering,
cancelled pacman, already-revoked continuation and root-owned mtree/core tampering.
These extra source cases do not replace the installed matrix.

The first actual `2630b025` invocation stopped before REMOVE/root authorization:
the old literal bootstrap had created an owned `0755` maintenance parent, while
the new script incorrectly required `0700` for both parent and private child.
No manual chmod/repair or removal effect was used to pass it. Successor
`b4b394e8b9d27099d56d679fd24a15f3fb4bf22b`, helper SHA-256
`628f2ac5fd61706a8fbfa14d2e4598d3b3ceb8da31c8ef3430d1c60e19fc6ae1`,
admits that same owned, non-redirected, non-writable-by-others parent and still
requires a private `0700` child. Independent delta review admitted VM testing;
86 inert cases include the literal old bootstrap metadata, unsafe/redirected
parents and incorrectly public child. The original refusal remains a failed
entry, not a successful clean removal.

### Ready wizard: installed bounded PASS

Candidate `656be0654d1700daba25c896d5df16ac17346982`, installed
`OnboardingWizard.qml` SHA-256
`6bfbc2c2e51983019b00e5abfcf2f21b95e0e80d52f4915b9a5f13290189a644`.
Actual Settings → Setup assistant → settled Core ready was reviewed in English
and Russian after a normal guest shell reload. Only heading/step, ready/version,
one next-step sentence and Close/Continue remain. Continue reached routing,
Back returned to ready and Close returned to Settings in both locales. Native
revision, disconnected state, startup Off, core/broker and DNS were unchanged.

The same installed QML was also rendered with synthetic EN/RU ready, missing,
attention, terminal and stale-delivery facts: ten reviewed captures, six blocked
Continue checks. Non-ready recovery/recheck/help remained; optional updates did
not leak into ready. Actual pixels were personally inspected at 1890×1333;
synthetic adjacent-state rendering is not live setup/fault-recovery acceptance.
30 onboarding production-expression tests passed, including the explicit
allowed/absent action matrix. No new string catalog/native package was needed.

The retrospective changes require both state relevance and actual user-reached
prerequisites, not merely working callbacks. A safe refusal is a negative check,
not completion of a normal uninstall; missing evidence must not redefine the
owner's requested product scope. The later resolver preflight also reinforces
using a sanitized actual installed schema as the positive parser fixture,
instead of a hand-invented response that happens to satisfy its own predicate.
