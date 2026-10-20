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
| Open app | Global navigation below the Profile actions dock on the main panel, outside list scrolling; not in Settings and not tied to the selected profile. Opens the TUI or focuses its existing window. Closing that window does not disconnect VPN. |
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

## Control roles and visual grammar

Owner-directed clarification, October19: choose controls by their effect, then
apply a consistent presentation. "Secondary" describes priority, not a license
to make an interactive control look like ordinary text. These rules govern new
and materially affected surfaces; they do not order a whole-product redesign or
retroactively qualify an existing screen. The specific profile dock, setup and
ready-wizard contracts below remain in force.

### Choose the role before the style

For each control, identify the user's goal, exact target, effect and return path.
Classify the effect as navigation, disclosure, local read, persistent mutation,
external navigation or destructive/recovery action. Then choose priority and
presentation in this order:

1. The state-specific contract decides whether the control exists and is enabled.
2. Its effect chooses the affordance and cues in the table below.
3. Priority chooses emphasis within that affordance, not a different effect.

Normally one action per active decision group is primary; use the shared accent
treatment for its button. Related alternatives are secondary and neutral.
Ordinary command buttons remain bordered at either priority. Effect-specific
link/disclosure treatments take precedence over that generic button default:
a secondary web link is still a link, not a second instruction to frame it as a
button. State-specific forward navigation such as Continue may be primary;
local Help is secondary. Independent profile rows retain their own connection
actions. Navigation is not mutation; a secondary action can still be dangerous.
Neither its color nor its position grants authorization.

| Role | Presentation | Behavior and examples |
| --- | --- | --- |
| Command/action | Bordered button; accent when primary, neutral when secondary. | Install components, Save, Check again or Copy request. Keep a visible boundary even when not hovered. Do not accent an optional update over required setup. |
| In-product navigation | Bordered button, neutral by default, no external-link marker. | Setup help, Settings Help, Back, Close or Open app; a state-specific forward step may use primary emphasis. Name the destination; opening a surface does not confirm, save, install, settle a terminal or change VPN state. |
| External navigation | A visibly identifiable link ending in `↗`, separated from the primary decision. | Read the guide or Report a problem. Borderless presentation is allowed for this link role, but it still needs an identifiable link treatment and visible keyboard focus; plain status-like text is insufficient. |
| Disclosure | Compact expandable control with an expansion cue and explicit Show/Hide label or equivalent accessible name. | Show request, Hide request or Other options. Borderless is allowed when the cue, hover and keyboard focus distinguish the control. Expansion itself never submits the revealed action. |
| Value/selection control | The shared switch, selector, tab or selectable row for that value. | On/Off, locale, mode or profile selection. Show the selected value independently of hover/focus and actual connection. Do not invent a link-shaped substitute or use an expansion arrow for a leaf selection. |
| Destructive or recovery action | Explicit bordered action with consequence-specific wording, visually separated from routine navigation. Use shared warning treatment where applicable, never warning color alone. | Delete, Full Quit or Restore access. Retain the owning confirmation, prerequisite and unknown-outcome fences; an ordinary Close is not Full Quit. |
| Information | Plain text, no button hover, focus stop or clickable hit area. | Status, version and explanatory copy. Do not rely on users discovering that an apparent label is actionable. |

For a multi-role control, keep the effect explicit and use the stronger safety
contract; do not disguise a fetch-and-save as a harmless local read. An icon-only
button is allowed for an established compact action when its target and familiar
meaning are unambiguous, it has an accessible name/tooltip and a visible boundary
or established selector treatment. Do not use icon-only styling to hide a new
or consequential action.

### Concrete Help decision

Setup help and Settings Help are in-product navigation: use a neutral bordered
button without `↗`. They open the same local Help surface, not a website.
Keep Setup help and Check again in a coherent secondary group when both are
eligible; neither becomes the primary setup action. Inside Help, Copy request
is an action, Show/Hide request is disclosure, and the guide/issue destinations
are external links. A change from a web destination to local Help must change
both the handler and its visual role; removing the arrow alone is insufficient.
Preserve origin/focus on return and the existing terminal-closure barriers.
This decision does not restore Help controls on a confirmed-ready wizard or
component-review state where the state-specific contract hides them.

### Composition, states and interaction

Group controls by the user's decision, not by the order their code was added.
Keep the primary outcome prominent, related neutral actions together and
destructive/recovery actions separate. Use one compact related row when it fits;
stack in the same logical order at constrained width. Back/Close and forward
wizard navigation retain their established left/right order. Do not force every
role to the same width, frame color or emphasis, and do not create duplicate
primary actions. Preserve the main-screen fixed management dock.

Use shared Omarchy components, palette, typography, spacing, borders and focus
tokens rather than per-screen guessed colors or dimensions. Neutral framing
must be visible at rest; hover and focus may strengthen it without moving the
control or adjacent content. Long EN/RU labels wrap or stack without clipping
their text, focus indicator or hit area. Grouping must not turn several actions
into one ambiguous clickable surface.

| State | Required visible meaning and interaction |
| --- | --- |
| Enabled / idle | Recognizable action or navigation before hover; label identifies its target. |
| Hover / keyboard focus | Distinct feedback with stable geometry. Focus must remain visible without a pointer; focus alone never activates or changes a selection's product effect. |
| Pressed / activated | Activate only the focused/pointed enabled control, once per explicit activation; preserve the owning asynchronous/pending fences. |
| Disabled / busy | Distinguish unavailable from ordinary text and explain a blocking reason nearby when needed. No keyboard activation, duplicate submission or apparently successful completion. |
| Hidden | No hit area or keyboard stop; do not leave an invisible action over the surface below it. |
| Error / unknown outcome | Retain truthful failure and the supported next action; do not manufacture success, auto-resubmit or make recovery look like ordinary navigation. |

Tab/Shift+Tab order follows the visible decision order and skips unavailable
controls. Enter/Space activation follows the selected shared control's supported
semantics; text editors retain their own newline/submit contract. Escape closes
the current cancellable surface and returns focus to its origin, without
silently discarding committed work, settling a system prompt or disconnecting
VPN. A modal overlay owns input and scroll; its keys must not activate controls
behind it. Test both forward and reverse traversal, not only callback invocation.

### Applying and challenging the rules

Keep one small action/state matrix in the owning PR: control, role/priority,
target/effect, visible/enabled states, presentation and return/focus. Do not
create a separate design diary. Apply the
[affected-surface UX gate](#premerge-affected-surface-ux-gate) to behavior,
meaning and actual rendering, including EN/RU normal/constrained states.
Navigation tests protect discovery, not visual conformance. A docs-only rule
update does not mean its referenced UI has already been fixed or inspected.

If a role is genuinely missing, record the exact ambiguity and proposed
extension here before implementation; do not improvise a one-off exception in
QML. Deviations need an explicit rationale and review, with an owner decision
when they change an agreed interaction. Retain known relevant findings as
premerge blockers unless explicitly owner-deferred; a border rule is not a
replacement for judgment about the user's actual task.

### Explicit exit during connection recovery

Recovery/unavailable controller facts block Connect, mode changes and ordinary
mutations, but must not hide the explicit Disconnect and confirmed Full Quit
request paths. These require a current parsed native snapshot, no failed snapshot
read, no pending/running mutation, no unresolved outcome and no concurrent Quit.
The request retains instance/revision/operation fencing; only the native owner
can decide whether cleanup is safe. Admission does not turn a recovery warning
into Disconnected or claim shutdown success. General metadata/ownership recovery
can still refuse cleanup; the UI must keep that failure visible. No layout,
confirmation, selection or ordinary panel-close semantics change.

### Configured but stopped application (owner direction, 2026-10-20)

Installation readiness, application availability and VPN connection are three
different states. A compatible installed/configured pair with a component-proved
`needs_runtime_start` belongs in the ordinary panel, not Finish setup or a new
error page. Show a neutral Application stopped status and one primary bordered
Start application command; keep familiar controls and the fixed profile dock.
Check again is a secondary local read, Help is secondary local navigation with
origin/focus return. Unavailable dependent controls remain disabled.

No RPC snapshot means profiles are unavailable, not an empty database. Discard
the stopped process's cached connection identity and require fresh metadata
before enabling actions. Starting is progress, not Connected; a failed Start
stays visible on the ordinary panel with Check again/Help, never an automatic
resend. Reuse the native session-only app Start: no VPN connection, enablement,
preference reset or package effect. Real missing/mixed/incomplete installation,
pending/unknown outcomes and genuine failures retain their existing fences.
Test cold boot without a snapshot, prior Connected cache, ordinary stop/start,
failed/refused Start, duplicate activation, ready and truly incomplete setup,
plus affected EN/RU normal/constrained and keyboard/return states.

### Application-login and VPN-autoconnect settings

Show application launch at login separately from the saved VPN autoconnect
preference. An explicit Check reads only the existing bounded local support
report; it neither enables a service nor connects a VPN or copies a report.
Use the report's runtimeService (omavless-runtime.service), not its static
loginService preparation dependency. Report the service setting when checked, not guaranteed persistence or
successful future login. Missing/older host coverage, stale replies and failed
reads are unavailable, not Off. Invalidate displayed facts across visits,
runtime/availability epochs and snapshot revisions. Keep the VPN Edit action
and its existing mutation/confirmation admission separate and unchanged.

### Local profile protocol filter

Profile protocol filtering is session-local presentation, intersecting the name
query without changing selected or connected identity. Use the present-only
collapsed disclosure specified below, preserving the fixed dock and the
connected-profile identity on both profile-list pages.
Active filters expose matching subscription children without overwriting saved
expansion. Reset filters clears both local filters; Details reveals its exact
selected target through both filters. Counts refer to matching profiles within
the current page scope, not rendered group headers or unavailable metadata.

### Profile protocol filter (owner direction, 2026-10-20)

Offer only protocols present in the user's added profiles, not every protocol
the application can import. Main uses the whole profile inventory; a browsed
subscription uses its own profiles. Derive choices before applying name or
protocol filtering, so typing a search never removes its own filter options.
With zero or one distinct protocol, omit the protocol selector. If an inventory
or scope change removes the selected option or the need for a selector, clear
only that local protocol filter; never retarget selection or the connection.

Use a compact, initially collapsed disclosure next to the search area instead
of an always-visible protocol strip. Its label retains the current value when
collapsed. Expanding only reveals choices; selecting filters the list and
returns focus to the disclosure. Escape first collapses an open chooser; hidden
choices are not keyboard stops. Reset remains reachable for an active search
even when no protocol selector is needed, clears both local filters and returns
to search. Keep the fixed management dock, connected identity, scoped counts
and temporary group expansion; no runtime, provider or persistent-store effect.

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

The native onboarding core step distinguishes confirmed desktop core setup,
missing components, a supported package update and unverified/failed facts.
Confirmed setup shows Core ready, the installed version and Continue without
suggesting reinstall; this is not service/TUN-creation/network-health evidence.
In the ready wizard step, show only that state/version, one short next-step
sentence and Close/Continue. Hide Check again, Installation guide, component
review/update navigation and setup prerequisite prose. Optional delivery updates
remain accessible outside this completed wizard step. The separate component
review/error surfaces retain their relevant recheck/help controls.
Other states navigate to the existing guarded component controller, not a new
installer. Missing/unknown facts never masquerade as ready Continue. Component
review retains Check again, Setup help and return navigation when core readiness
is unconfirmed, including when package presence is known but core permissions
are not confirmed. Once package/core readiness is confirmed, no terminal or
operation is pending and effects are not blocked, hide recheck/help on that
review page too; keep Core ready and return navigation. Test this second ready
surface as well as the wizard, rather than treating one successful screen as
coverage of every entry. Optional component-update notices retain their own
relevant controls. Refresh
means refreshing facts; it must not imply a package update. Only an actually
supported update state offers the admitted app/DNS paired update. A missing
companion beside an existing app is not automatic repair authority.
An opened setup terminal blocks ready Continue even after programs become
present. Review must retain the explicit terminal/prompts-closed acknowledgement
when the ordinary missing-components block disappears; navigation/recheck alone
does not settle authorization or authorize another effect.

Component delivery freshness is independent of operational readiness. A
verified compatible older pair keeps Connect, ready Continue and the ordinary
Start application / Start existing DNS broker continuations; an optional update
notice cannot replace those facts. Missing, mixed, modified or unknown members
remain blockers. A shared package version is not delivery identity: compare
published bytes/source per architecture, not only `pacman -Q` or a receipt's
self-declared hash. Managed Mihomo belongs to the app/DNS pair; do not offer an
independent update of an unrelated stock core.

For a component-update change, cover current, same-version older, qualified
0.8.2, missing companion, mixed/modified delivery and stopped broker/runtime.
Separate inventory reads, navigation, download, consent, package transaction
and post-update continuation. Advertise an update only if the exact pinned
native helper actually supports it. Fixture rendering does not prove that
delivered packages contain that helper or that their installed path works.

For setup/maintenance, the acceptance unit is the affected **user journey**,
not a helper phase. Record its actual entry, prerequisites the user must reach,
button/terminal/consent, original outcome, closure acknowledgement and next
reachable action. An expert-only fixture stop, socket cleanup, preloaded archive,
seeded enrollment or manually repaired state is an unearned journey edge unless
it is itself the documented, reviewed and tested user procedure. Mark offline
candidate/cache evidence separately from anonymous published downloads.
Verify the prerequisite edge before final artifact assembly; package-phase PASS
cannot conceal a missing preparation/return path. Keep an update advisory from
disabling ordinary Start/Continue, and make preparation explicit inside the
update flow rather than promising immediate replacement. Its entry is named
Update components…; the ellipsis indicates subsequent steps and confirmations.

## Three independent checks before calling a change ready

### Agent-first help (owner direction, October16)

One local Help surface serves setup, non-ready onboarding and Settings. The
first option offers Copy request with the public prompt hidden by default.
Show request/Hide request reveal or collapse a separate selectable text block;
opening Help must not present the full template as another instruction to read.
The prompt names
the installed user-assistance guide directly and also AGENTS.md if present. It
ends at the problem entry with a blank line. Keep diagnostics/version/consent rules in
the agent guide, not repeated in the user's prompt. Do not collect private facts
or read the clipboard merely by opening help. Copy writes only the public
template and reports success only after the clipboard writer succeeds; failure
must reveal selectable text and an honest manual-copy action. Hidden text is
excluded from keyboard traversal. Copy failure does not steal focus from a
different action, and a late result cannot reopen text in another Help visit.

Read the guide and Report a problem are independent secondary web links marked
with ↗. Neither using an assistant nor reading documentation is required before
reporting an issue. Help must work before the native app is installed and during
an unsettled setup terminal without bypassing its barriers. It never runs an
agent, installs components, authorizes effects or changes a VPN. Closing it
returns to the originating screen without resetting the wizard or treating a
terminal as settled. Preserve the ready-wizard Close/Continue-only contract.
Review EN/RU, Tab/Shift+Tab/activation/Escape, copy success/failure and constrained
height; the overlay owns input/scroll instead of activating underlying controls.

### End-user editorial rule

The shared [glossary](../../GLOSSARY.md) owns component names and action
terminology across QML, shell, native prompts, user guides and support copy.
Read it before changing user-facing wording. It does not replace runtime
contracts, exact delivery checks or rendered review.

Owner correction, October 14: onboarding, ordinary status cards, public guides
and support replies are for the person using the product, not its implementing
agent. Start with the user's goal, the current state and one reachable next
action. Normally use one or two short sentences and an explicit button label.
Keep optional updates clearly optional; preserve real failures and necessary
warnings, but do not explain an internal authority protocol in the main panel.

Words such as broker, enrollment, matched pair, ownership, joined observation,
delivery provenance and acceptance gates belong in diagnostics/development
records unless the user needs that exact term to act. Necessary confirmation
tokens such as INSTALL/PREPARE/UPGRADE belong at the actual terminal prompt,
not in a paragraph describing every future step before a button is clicked.
Explain consequences when relevant (profiles kept/deleted, VPN disconnect,
password request, reboot), not every action which is *not* performed.

Introduce a required helper at its first user-facing mention with its short
purpose, then use the same name in statuses, buttons and standalone terminals.
The DNS helper manages OmaVLESS connection DNS; it is not a public DNS provider
choice or the Mihomo VPN core, even though the same package contains both.
Initial setup may recommend turning off other VPN connections, without stopping
them automatically or repeating the warning during ordinary ready-state review.
An optional update notice must not hide the operational reason for a separate
Start/setup/recovery action, including the limits of Restore access.

For setup/onboarding actions, show current state, short explanation and the
reachable next action in that order. Required operational setup comes before
an optional-update card; optional freshness does not acquire primary styling.
Keep ordinary DNS-helper Start primary and place Restore access behind an
explicit secondary disclosure with its clean-reinstall prerequisite. Disclosure
is navigation only and never grants the restore effect. Preserve all existing
busy, terminal-closure, pending and unknown-outcome barriers.

Wizard navigation places Back/Close on the left and Continue/Finish on the
right, stacking rather than clipping long localized labels. Refresh and web
help are secondary controls separate from forward navigation. All web-link
labels end with the same `↗` marker; local URL reveal, package installation and
in-product navigation do not. Completion without a profile names that outcome,
not an ambiguous "later" promise. Progress prose names only the observed phase
(opening a terminal, starting the app, saving routing), never a fabricated
package transaction or successful connection.

A readiness label names its scope (core installed/ready versus VPN connected)
without an essay about test coverage. Keep version information useful, and
move low-level TUN/capability facts to explicit diagnostics/help. Unknown state
still blocks unsafe actions; shortening copy never changes admission or hides
an actual error. Avoid claiming internet health from installation metadata.

Support replies include a short ordered sequence of **actual commands and
visible actions**, then an optional link to more detail. If checks are complex,
use a reviewed bounded helper; do not make the user assemble an engineer's
runbook. Respect the chosen reset scope: an expressly requested fresh reset
deletes only that application's settings after clear notice, without adding an
unrequested backup ceremony. Normal updates still preserve user data.

Before handoff, read each surface as a first-time user. Remove agent-directed
policy, repeated caveats and redundant explanations; verify the shortened
EN/RU rendering and the next action on the exact installed candidate. Function
or wording-length tests are guardrails, not editorial/visual acceptance.

Treat the widget, its setup shell and native helper as one editorial journey.
Follow every referenced button through terminal consent, cancellation/failure,
closure acknowledgement and the offered next action. Compare references to the
actual catalog, not remembered labels. Tests check these cross-surface names
and the controller-selected locale; the glossary is checked against the catalog.
Return text describes the completed operation, not unobserved global VPN state.

Record where each reviewed string actually comes from. In particular, the
setup shell extracts its native helper from the pinned application archive;
editing Rust source or installing only the widget does not update those prompts.
An inert preview compiled from the production formatter can earn text/rendering
review, but not privileged update, public download or installed-delivery
acceptance. A changed native formatter requires new matching packages and pins
before claiming that the shipped journey contains the change. Preserve system
password/package prompts and exact consent words when simplifying prose.

1. **Behavior:** does the real handler address the right record/state, honor
   admission and existing confirmation, and reject unavailable/stale inputs?
2. **Meaning:** can a user predict the target and effect from the visible label,
   icon and placement without reading implementation notes? Is a navigation cue
   falsely implying a mutation, or the reverse?
3. **Rendering:** does the installed candidate actually show that meaning, with
   reachable controls, consistent alignment and usable focus/scroll behavior?

No check substitutes for another. A screenshot proves only its captured state;
controller liveness or a Connected label does not prove internet/DNS health.

For state-dependent screens, list the allowed visible actions and the controls
which must disappear in success, loading, missing and failure states. Test both
presence and absence against production visibility expressions; then inspect
the installed ready screen and an adjacent non-ready state. A working callback
or legible button does not establish that the action belongs on that screen.

### Premerge affected-surface UX gate

Owner clarification, October17: this is mandatory before integration into
development, including a narrow wording or read-state fix. Review all controls
on the affected surface, not only changed labels. An inherited control can make
the new state misleading even when its own source line did not change.

Record primary, secondary, navigation and recovery roles before selecting the
composition. Use shared Omarchy controls/tokens and compact coherent action
groups; do not leave unrelated actions in an arbitrary vertical stack merely
because that was the old layout. Different roles need not have identical
widths, borders or emphasis. Preserve the accepted main-screen profile dock
and unrelated screens; this rule does not authorize a whole-product redesign.

For each affected loading, unavailable/error, confirmed-empty and populated
state (plus cached, pending or unknown states where relevant), check visibility,
enabled state, target and actual handler. Distinguish navigation and local
metadata reads from provider/store mutations. Do not hide a required explicit
Disconnect/recovery/exit path to simplify appearance, enable a blocked action,
remove confirmation or weaken native authority/unknown-outcome fences.

Before merge, give separate behavior, meaning and rendering dispositions on
the final exact candidate. Include adjacent states, EN/RU narrow/normal layout
and relevant keyboard traversal; fix findings and repeat materially affected
checks/captures after a change. Text legibility alone is not final UX acceptance;
neither green CI, a correct callback nor a count of screenshots closes this gate.
Known relevant UX findings block merge unless the owner explicitly defers them.
Record that decision and remaining limits rather than calling an unfinished
surface final or silently scheduling its correction after integration.

The owning PR carries the bounded action/state matrix and disposition. Link
this one canonical rule from the mandatory agent entry and protect discovery
with the navigation guard; such a test cannot prove actual agent judgment or
rendered acceptance. Unrelated baseline findings may be separately scoped with
an applicability reason; do not turn every small change into a full UI audit.

### Pick affected states, not a ceremonial full-product rerun

Changing a global terminal/input mode requires checking every existing editor,
not just the new dialog. Bracketed paste must reach only the active field and
must not replay shortcuts, Enter or confirmations. Verify advertised modified
keys through real terminal encoding/PTY input; synthetic key objects alone can
hide an unsupported key combination. Trace action copy to the actual semantic
runtime dispatch: a save-shaped label must disclose a subscription fetch when
that existing operation fetches before committing.

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

When a transition depends on asynchronous property notifications, model their
actual publication order, not just a manually invoked settled callback. Include
a refusal with an already-matching cached value and an unknown result; neither
may advance the journey as if the new action succeeded.

First-run completion additionally covers Finish -> native acknowledgement ->
persisted completion -> close to desktop -> reopen. A submitted command is not
completion. Repeat close/reopen after each terminal non-success too: a retained
Finish retry must not silently return to step one. Model Process.running
becoming false before result publication and
an old in-flight read before a queued post-action read. Bound local waiting;
failed reads, restart and acknowledged unknown outcomes must leave a reachable
explicit continuation without clearing native operation fences or resubmitting.
Initial component reads show checking, not missing/setup-required actions.
An expected metadata observation refresh must not flash a fatal VPN indicator;
its neutral presentation is bounded and never masks actual failure/recovery.

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
