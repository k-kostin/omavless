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
