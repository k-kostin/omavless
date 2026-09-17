# OmaVLESS plugin UI/UX contract

Status: owner-directed local development contract, 2026-09-13. This document
does not declare R6 complete or publish the local candidate to main.

## Purpose and ownership

Prevent technically correct controls from creating misleading user actions.
`AGENTS.md` routes UI changes through the repository-local
[`omavless-ui-review` skill](../../skills/omavless-ui-review/SKILL.md).
This file owns product semantics; the skill owns the working procedure.
[Acceptance environments](ACCEPTANCE_ENVIRONMENTS.md) and
[localization policy](I18N.md) retain their existing authority.

Scope is the current Omarchy plugin, not a universal design system or a mandate
to redesign every screen. The owner accepted the corrected main layout at local
candidate `13717a75264aa2be350bdf741d57b4f1210bcbad` and considers Settings
acceptable. Preserve that baseline unless a requested change or demonstrated
defect requires a narrow adjustment. Later intentional redesigns must update
this contract rather than silently contradict it.

## Why the earlier process failed

During native UI restoration, backend parity became a proxy for product parity.
The implementation retained technical labels without a user task, used an
expansion-shaped arrow for selection, and placed secondary actions ahead of
connection. A subsequent patch clarified selection in prose but still put the
main action on a separate row. Function tests passed while the interaction was
misleading. Existing source audits had already warned about these categories;
the missing control was an obligatory semantic and rendered review before
calling the screen ready.

The [selection correction record](../testing/R6_PROFILE_SELECTION_UX_2026-09-13.md)
contains historical evidence and limits. Its first layout was rejected; the
fixed-dock follow-up supersedes it. Neither a past screenshot nor owner approval
of appearance establishes untested runtime or network health.

## Main-screen interaction invariants

| Element | User-facing contract |
| --- | --- |
| Profile name | Selects the target for management. Selection alone does not connect, disconnect or change mode. Its styling is not evidence of a tunnel. |
| Row connection action | Connect/Disconnect is directly opposite the corresponding profile name, visible without first selecting that profile. It acts on that row, not another selected or last-used record. |
| Management actions | Favorite, rename, edit, QR, export, details and delete occupy one separate fixed dock outside list scrolling. The dock explicitly identifies its selected target. Changing selection does not rearrange its buttons. |
| No valid selection | Show an instruction to select a profile and disable management actions. Removed/stale records never fall back to an unrelated profile. |
| Managed profiles | Retain existing edit/rename/delete restrictions and confirmation semantics. Disabled controls stay in their normal positions. |
| Subscription arrow | Means expand/collapse an actual child list. A leaf profile, inside or outside a subscription, has no expansion arrow. |
| Connected identity | Comes from current verified runtime state, not cursor, hover, selection or last-used configuration. Remains identifiable when its group is collapsed or filtered out. Unknown state must not assert a verified connection. |
| Mode choices | Preserve the noticeable main-screen selector. Do not add a second textual summary of the same mode without a distinct user need. Desired/configured mode is not proof of actual network transition. |
| Technical metadata | No protocol token beside Connect just because it is available in the model. Place details in the appropriate explicit details/diagnostics surface unless a concrete task requires them here. |

Selection, hover, keyboard focus and connection are separate concepts. Existing
explicit keyboard activation may connect; mere focus/movement may not. Its
target must agree with the visible cursor/action. Reusing a familiar glyph is
appropriate only when its familiar meaning matches the real behavior.

The dock rule is specific to this plugin's profile-management surface; it is
not a requirement to add fixed footers everywhere. Likewise, preserving existing
controls does not require adding a border to every label or equalizing unrelated
control roles. Keep the compact terminal-like style using shared shell tokens,
plain-text dynamic data and the existing scrollbar gutter. Longer translations,
selection and hover must not overlap adjacent controls or move their hit areas.

The owner-hidden main Test and latency sections remain hidden until explicitly
requested. Do not restore them as part of general polish.

### Required components (owner direction, 2026-09-15)

Missing OmaVLESS/Mihomo belongs in a bordered **Required components** block
below Profiles, not an unavoidable onboarding dialog. Show only missing
programs; use one sequential installer when both are absent. Hide the block
when neither is missing. Installed-but-unactivated or unknown application state
has separate setup/recovery guidance, never a reinstall offer. Without a ready
app, show a truthful unavailable-profile shell; do not invent an empty store or
working connection controls. Deferring onboarding does not dismiss component
facts. Presence is not permission/TUN/live-health evidence. A known missing core
blocks Connect but never prevents Disconnect. Preserve the accepted main and
Settings layout when the installation is ready.

## Three independent checks before calling a change ready

1. **Behavior:** does the real handler address the right record/state, honor
   admission and existing confirmation, and reject unavailable/stale inputs?
2. **Meaning:** can a user predict the target and effect from the visible label,
   icon and placement without reading implementation notes? Is a navigation cue
   falsely implying a mutation, or the reverse?
3. **Rendering:** does the installed candidate actually show that meaning, with
   reachable controls, consistent alignment and usable focus/scroll behavior?

No check substitutes for another. A screenshot proves only its captured state;
controller liveness or a Connected label does not prove internet/DNS health.

### Pick affected states, not a ceremonial full-product rerun

For profile selection/connection/action layout, cover these contrasts. For
other UI changes select the corresponding affected and adjacent states and
explain any exclusions briefly.

| State pair | Required question |
| --- | --- |
| Selected A / connected B | Does every control clearly identify its real target? Can B be found when hidden by grouping/filtering? |
| Local / subscription-managed profile | Do permitted and disabled actions remain understandable and stationary? |
| No selection / removed selection | Is an unrelated target ever substituted or implied? |
| Disconnected / connected / pending or unknown | Does the UI avoid premature success, double actions and false health claims? |
| Collapsed / expanded / filtered / long list | Is the selected management target named and are connection actions reachable? Does the dock stay outside scrolling content? |
| Pointer / Tab / Shift+Tab / activation / Escape | Is focus visible, is the action target correct, and can the panel be left normally? |
| English / Russian / long safe name / constrained height | Are labels legible, primary actions reachable, and scrolling/gutters free of overlap? |

Use synthetic metadata for deterministic tests and isolated rendered states
when possible. Label an isolated rendering as such; it is not live tunnel
evidence. Existing main-panel/presentation, action/IPC and localization tests in
`tests/` are the first regression entry points. Prefer behavior assertions over
tests which merely freeze exact source wording. Do not weaken an expected
behavior to accommodate a new layout.

### Installed review and safety

- Record source commit, installed frontend identity and relevant viewport/locale.
  Byte matching alone does not prove loaded QML: verify a distinctive changed
  behavior/visual after the supported reload. Do not restart the VPN core to
  refresh UI code.
- Inspect before/after captures after animation/loading settles. Review enough
  overlapping scroll positions to see affected content. Recapture fixes, not
  just the initial defect. Keep fixture-bearing images private and outside Git;
  publish only sanitized evidence under the existing privacy policy.
- Maintain the owner's requested network state. UI review never grants new
  authority to switch VPN, export credentials, confirm deletion or invoke
  privileged recovery. If a real transition is needed, apply the separate
  [host authorization procedure](../testing/HOST_AUTHORIZATION_ACCEPTANCE.md).
- Verify the focused surface before text entry or activation. Stale coordinates
  can send a shortcut to the panel instead of a text field. Stop an uncertain
  automation sequence rather than replaying it or creating repeated prompts.
- If the display, state or human interaction is unavailable, continue safe
  in-scope work and mark that exact check pending. Do not claim acceptance or
  require the owner to discover basic alignment/meaning mistakes first.

## Durable handoff, without unnecessary paperwork

Use the existing task report/PR record rather than creating a new report for
every small edit. Include: changed user scenario and target, intentional
differences, exact tested candidate, behavior tests, inspected rendered states,
live checks actually performed, remaining gaps and any environment restoration.
Keep accepted historical evidence distinct from superseded designs.

Commit useful contract/test/evidence changes on the authorized branch. Follow
the current owner instruction for remote writes; this contract never authorizes
push, merge or main publication. Docs/skill-only changes need link/skill/diff
validation, not a plugin reinstall or live VPN smoke.
