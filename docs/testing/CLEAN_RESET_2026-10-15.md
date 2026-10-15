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
- No changes to the native app/DNS producer or immutable fix.7 packages.

The helper is a terminal reset for supported public 0.8.2/0.9.8 installations
which **never connected a VPN**. The REMOVE screen explicitly states this limit
and deletion of this account's own profiles/subscriptions/settings. It is not
the all-state, data-preserving 0.9.9 remover planned in #774. Connected-ever,
unknown/experimental builds, custom XDG locations and unknown ownership remain
unsupported; no forced kill, guard bypass or journal/socket deletion is added.

The ordinary entry downloads the exact helper into a newly created
`.cache/omavless-maintenance/reinstall-098` subdirectory and verifies its hash.
It does not depend on a development checkout or an existing download directory.
After REMOVE consent it detects the running application, asks STOP separately
and uses existing authenticated/fenced native shutdown. Enabled widgets use
the original hello fence plus full Quit; removed/disabled widgets use the fixed
removal consumer. A watcher no-op never earns shutdown by itself. Uncertain
outcomes stop once and retain the original setup exclusion.

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
- Production validator/shutdown/parser/classification fixtures: 57 PASS.
- Fixed root-branch fixtures with isolated paths/inert commands: 33 PASS,
  including empty-directory cleanup, hidden entries, unsafe metadata/symlinks
  and failing `rmdir`. The absent-DNS early exit is isolated before asserting
  filesystem deletion; it cannot skip that assertion.
- Guide: four Bash blocks, exact helper/hash, ordinary main add, short reset
  scope, no external UI Quit prerequisite: PASS.
- Anonymous HTTPS helper bytes match the pinned SHA-256: PASS.
- README addition and all four support-draft command blocks match branch
  documentation byte-for-byte: PASS; the reply remains local and unsent.
- Local combined `tests/run.sh`: 776 Python tests PASS, two existing skips,
  all JS/QML suites PASS. This is not a Rust rebuild or installed removal.
- Hosted first implementation checkpoint `0baf317f` failed the expected old
  guide/hash consistency check; retain FAILURE. Documentation was corrected in
  `570ffe2e`; later hosted CI is independently recorded in #790.
- Independent architecture/delta review accepted bounded VM testing and
  subsequently approved code/docs at `8e9dd884`, with installed gates separate.
- Independent review approved successor helper SHA-256 `33eabf6c` for bounded
  VM testing; successor installed gates remain separate from original results.

## Installed acceptance matrix

Sole operator: VM master. Only the dedicated PC x86_64 development guest is in
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
| Public 0.8.2 without DNS companion | Source implemented; installed gate pending. |
| Packages installed before first DNS enrollment | Source implemented; installed gate pending. |
| Successor helper; root directory cleanup after enrolled/pre-enrollment or absent DNS | Pending installed gate; earlier PASS results do not earn the new cleanup effect. |
| Reinstall through ordinary main add after reset | Pending; no tag/candidate substitution or fixture repair inside the user journey. |
| Connected-ever, lost ownership or experimental Restore build | Excluded; not an accepted generic uninstall. |

Installed ARM64 acceptance of this changed remover is not claimed. The owner's
Mac first-use result and shell-reload result concern main/fix.7, not this helper.

## Practical retrospective

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
