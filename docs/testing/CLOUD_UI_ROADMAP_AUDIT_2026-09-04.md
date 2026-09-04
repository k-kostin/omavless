# QML interaction and roadmap source audit — 2026-09-04

Audited main: `2f9f6bcdbafdfa6b34da50160e7a739618ab814b`.
Scope: current plugin presentation, navigation/scrolling, and delivery claims.
This is a source audit, not rendered visual acceptance or a full security audit.

## Evidence and limitations

- Read current GitHub main, all six remote branches and five open PRs; checked
  PR #123's merged state and recorded acceptance.
- Inspected production QML, QML contracts, delivery/workflow/acceptance/I18N/Rust
  roadmaps. The inspected QML is identical to the audited main.
- Reproduced the hidden-filter failure with production JavaScript functions
  extracted from Panel.qml and executed in Node, using synthetic profile names.
- No Quickshell, qml/qmlscene or qmllint executable is available in this cloud
  environment. No current screenshots were captured. The repository preview
  image is not evidence of current hover, focus, geometry or scrolling.
- qs.Ui and qs.Commons implementations come from the installed Omarchy shell.
  Their exact sizes, painting and default focus behavior were not inspected here.
  Host acceptance must record shell revision, theme, font and display scale.
- The owner reports inconsistent button sizes/outlines/hover and an out-of-place
  scrollbar. Source findings below explain plausible causes; pixel-level claims
  remain unverified. Token dimensions below are Style.space values, not measured
  physical pixels or an assertion that all buttons should have identical size.

Priority P1 means blocked/misleading interaction; P2 means consistency or
acceptance gap; P3 means documentation/maintainability. These are audit priorities,
not protocol roadmap stage names.

## Confirmed source findings

### U1 — P1: an active profile filter can become inaccessible

Evidence: `plugin/Panel.qml`, `profileFilter`, `buildProfileRows`, profileSearch
and onOpenedChanged. Search is visible only with at least eight profiles; the
model always applies profileFilter. Neither count changes nor close/reopen clear
that value. `/` also tries to focus the search field without checking visibility.

Reproduction: start with eight synthetic unmanaged profiles, search for the only
one called `match`, then remove that profile (or refresh a subscription to a
smaller set). Seven profiles remain, search hides, and the model returns zero
rows. Clearing the filter returns all seven. Node assertions using the extracted
production profileMatches/buildProfileRows functions passed. This establishes the
model/visibility contradiction, not the rendered focus behavior.

Fix: retain an accessible search/clear control while a query is active, even
below the discovery threshold. Do not silently discard user input during refresh.
Make `/` reveal/focus a visible search surface. Test 8 -> 7, 1 -> 0, refresh,
close/reopen, empty query and keyboard-only recovery. Keep this a narrow QML fix.

### U2 — P2: control roles have no plugin-wide presentation contract

Evidence: `Panel.qml` hero and importActions; settings/subscription back buttons;
profile/subscription row actions; `AdvancedDiagnostics.qml` controlHeight;
`StartupPrompt.qml` ChoiceRow; `Panel.qml` editChooser.

Hero actions explicitly use bordered=true and bright foreground, with glyph
multipliers 1.35 and 1.5. Import icons use dim -> foreground and derive size from
subscriptionsButton. Back actions use foreground -> accent. Row actions use
several dim/foreground/accent/urgent transitions and visibility-on-hover rules.
Diagnostics forces a 32-unit height; uptime uses 28; custom edit choices use 34;
other controls inherit shell defaults. Some differences are justified by role,
but the role-to-style rules are only scattered comments and property overrides.

Fix: first agree a small role/state contract using existing Omarchy tokens and
controls. Separate primary/secondary/icon/destructive actions and selection from
keyboard focus. One shared size per density class, aligned icon/text hit areas,
and documented exceptions. Do not put a persistent border on every button or
remove every border. Do not fork/copy third-party plugin components.

### U3 — P2: scrollbars have no local theme/geometry contract

Evidence: attached ScrollBar declarations in Panel, AdvancedDiagnostics,
OnboardingWizard, StartupPrompt and RoutingToolsPrompt. They select policy but
provide no local contentItem/background/width/state styling. Main/settings/
subscriptions reserve 16 units; diagnostics/routing tools duplicate that gutter.
StartupPrompt's outer and inner profile list keep content at full viewport width.

Confirmed: inconsistent gutter reservation and reliance on platform Qt Controls
styling. Not confirmed: exact thumb radius/color/overlap on the owner's shell.

Fix: prefer a suitable existing shell scrollbar if present; otherwise one small
plugin scrollbar component using shell theme tokens. Reserve a stable trailing
lane with an explicit gap; keep its position consistent across pages and sheets.
For the requested console character, review a restrained rectangular thumb,
quiet track and no unrelated rounding/glow. A thin visual thumb must retain a
usable drag target. Test wheel, drag, short/long content and nested scrolling.

### U4 — P2: modal height policy is inconsistent

Evidence: ImportPreviewPrompt, SubscriptionPrompt, RoutingPresetPrompt and
NamePrompt calculate full content height without a viewport cap or scrolling.
StartupPrompt and RoutingToolsPrompt cap height and use a Flickable. Panel itself
caps its content at 600 units or available screen space; its sizing expression
does not account for the active modal's content.

This is a confirmed missing bound and a potential clipped-action bug, not a
measured overflow on the current host. Reproduce on a short panel, larger font,
Russian labels and long safe validation text. Import/preset dialogs must keep
Cancel/Confirm reachable. Use one modal shell policy with a bounded body and
reachable actions; avoid independent wheel movement beneath it.

### U5 — P2: hover changes the persistent edit-choice selection

Evidence: Panel.qml editChooser MouseArea.onEntered assigns selectedIndex;
there is no matching pointer-exit reset. That index also drives the accent
border/fill and the Enter action. This differs from hero hover, which clears the
keyboard cursor, and couples pointer hover to keyboard selection.

Confirmed state coupling; whether its persistence is undesirable needs a host
interaction decision. Prefer distinct hovered and keyboard-focused states.
Verify hover -> leave -> Enter, keyboard navigation -> pointer movement, and
that a pointer departure cannot make a destructive or unexpected action appear
implicitly selected. Do not confuse this finding with the existing mode bug.

### U6 — P2: UI contracts assert source shapes, not presentation behavior

Evidence: tests/test-qml.sh greps for exact gutter expressions, two icon-size
assignments and three modal scrollbar policies. These are useful structural
regressions but cannot detect clipped buttons, theme-incompatible scrollbars,
unreachable focus or U1's changing-profile behavior.

Keep privacy/contract checks. Add small behavioral tests for model changes where
possible and a compact exact-head visual matrix for shared controls. Do not call
more grep assertions visual acceptance or build a huge screenshot ceremony.

## Risks requiring rendered reproduction

### U7 — P2: keyboard focus may leave the diagnostics viewport

AdvancedDiagnostics links searchField -> providersRefreshButton, which occurs
after up to 300 rule delegates. There is no local ensure-visible handler, unlike
Panel.scrollPanelControlIntoView. With many rules, test Tab from search and
Shift-Tab from Back: focused controls must be visible and operable. The actual
shell Button implementation may influence behavior, so this is not yet a
confirmed rendered defect. If reproduced, use shared focus-to-scroll handling.

### U8 — P2: action density can squeeze titles and long localized labels

The hero accumulates uptime/settings/QR/test/power; the Profiles title and its
action Row are independently left/right anchored. There is no explicit width
negotiation between that title and the action cluster. The edit chooser uses
three equal fixed-height cells with centered text and no wrapping/elision.

Test a constrained width, larger font, Russian and active/idle/busy variants.
Do not claim an overlap from source alone. Prefer predictable spacing and
responsive overflow policy over individually shrinking glyphs or fonts.

## Roadmap audit

### R1 — P3: secondary-dialog localization is incorrectly still queued

DEVELOPMENT_ROADMAP P-I1 and docs/roadmap/I18N list secondary dialogs as upcoming.
PR #123 is merged: accepted head `384e31960e4bbbd8277a7127f1bd76819249f5a7`,
merge `fca795c57a08db29faca484ad9f05b60daa9a1c9`. Its body records Try Omarchy
English/Russian secondary-dialog, QR and editor acceptance. Mark this batch as
accepted; remaining dynamic prose is separate. Examples still present include
probe ages/Unavailable/DNS failed and legacy validation messages. Do not translate
provider data or claim localization complete merely because #123 merged.

### R2 — P2: UI consistency has permission but no deliverable or exit gate

The plugin lane permits UI/accessibility fixes but has no explicit shared-control
contract, owner or acceptance checklist. Repeated local tuning can therefore
pass CI without converging. Add a bounded P-U1 plugin work item, independent of
R5, with U1 and the existing #135 tracked separately from visual consolidation.

### R3 — P3: chronological foundation lists obscure the cutover remainder

AGENTS' dated checkpoint ends at #142 while main includes #153. The dated text
is historical, not proof of current state. More importantly, the R5 ledger mixes
already implemented foundation requirements with still-required integration.
Keep accepted checkpoints, but expose a short remaining queue:

1. Review #154 protocol and #155 deadline, then #156's genuine dependency on #155.
2. Integrate worker/registry/scheduler, owner-generation and cancellation commit
   fence; pass shutdown, restart, stale-member and competing-operation tests.
3. Connect the frontend semantic bridge and execute controlled host cutover gates.
4. Prove R6 with Python unavailable; only then start production T2.

#154/#155/#156 remain Draft; their cloud checks do not activate Rust ownership.
The detailed integration handoff is in #156. This report does not mark R5 done or
request blanket approval to merge the stack.

### Boundaries that remain correct

- D1 is already accepted; do not restart #27 from the old chat handoff.
- #135 remains the owner of mode confirmation/authorization rollback. Its current
  head is `bf3af3618488d72536791a88f358d54922e9d472`; local transition/cancel gates
  remain. Do not duplicate its fix in the visual work.
- #30 remains fixture-constrained at `a643db595ad5369b5fea200ebc601b4f0f70f18f`.
  XHTTP representative evidence does not prove all experimental protocols.
- P4 Rust-first WG/AWG foundations may proceed under their own gates. Old blanket
  V0 -> P4 prohibition is superseded. Parser/mihomo -t is not connectivity.
- Marketplace 0.7.0 is still a reviewed snapshot, not an automatic main release.
- Try Omarchy is an accepted ordinary integration host; bare-metal is mandatory
  only for a concrete host-sensitive gate. NixOS needs independent evidence.

## Proposed implementation sequence

1. Narrow U1 behavior fix with regression coverage. Independently finish #135's
   declared local gates; do not restyle its pending mode logic.
2. Capture current screens on Omarchy and inspect installed shell controls.
   Agree button role/state/size rules and scrollbar geometry from that evidence.
3. One bounded shared-control/scrollbar PR; migrate representative main/settings/
   diagnostics usages first, with no runtime behavior changes.
4. Separate bounded modal/focus work for reproduced U4/U7/U8, then remaining
   surface migration. Retain localization and privacy contracts.

## Tomorrow's Omarchy matrix

Record exact installed plugin SHA, shell revision, theme/font, scale and viewport.
Use sanitized fixtures for shareable evidence; private screens stay off GitHub.

- Main: disconnected/connected/busy; mouse enter/leave and keyboard focus on hero,
  imports and row actions; verify selected, hovered and disabled are distinct.
- Search: 0/1/7/8 profiles, active filter while count drops, close/reopen, `/`/Escape.
- Scrolling: main/settings/subscriptions/diagnostics/startup/routing tools; short
  and long content, wheel and thumb drag, all content reachable in its own lane.
- Diagnostics: enough rules to put provider refresh below the viewport; Tab and
  reverse Tab must reveal the target. Empty/busy/disabled states also matter.
- Sheets: import/subscription/preset/edit/rename; short viewport, English/Russian,
  longest safe text; Cancel/Confirm remain reachable; no background scrolling.
- Review all captured states, including overlapped portions of long surfaces.
  Record pass/fail per finding; source hypotheses stay open until reproduced.

No production code changed by this audit. No main/marketplace mutation or merge;
no new runtime acceptance claimed. This is a bounded QML/roadmap review, not an
exhaustive Rust/Python security or performance certification.
