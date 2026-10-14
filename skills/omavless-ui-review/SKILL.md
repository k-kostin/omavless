---
name: omavless-ui-review
description: Implement or review OmaVLESS plugin UI changes with explicit action targets, preserved interaction semantics and installed-screen verification. Applies to layout, controls, labels, selection, focus and scrolling; not backend-only work.
---

# OmaVLESS UI review

Read the repository `AGENTS.md` and the complete
[`UI_UX_CONTRACT.md`](../../docs/roadmap/UI_UX_CONTRACT.md). That contract owns
product decisions and the risk-based acceptance matrix; do not copy it here.

## Before editing

Inspect the current screen, the owning QML handlers and available prior accepted
evidence. If no rendered environment is available, say so; source review cannot
establish what is currently visible. Historical screenshots are references,
not proof of the current installed build.

For the changed flow, write a short decision note in the work update or existing
task record: user goal, primary action, exact target, navigation versus mutation,
and the expected visible result. For layout changes, identify what stays fixed
when selection or content changes. Do not add technical metadata or familiar
icons without explaining their actual user-facing purpose.

For setup/update, use the contract's user-journey unit: verify the actual entry
and user-reached prerequisites before assembling final artifacts. Identify each
fixture-only intervention; do not let an expert-prepared broker/cache or a direct
helper call silently replace a missing product edge. Challenge the return after
Quit/terminal closure as well as the happy-path button.

Preserve the owner's current accepted baseline. Make the bounded correction;
do not start a redesign, backend rewrite or broad audit merely because this
skill applies. Reuse installed Omarchy controls/tokens and inspect their actual
API when changing their use.

## Implement and challenge

Test the behavior most likely to be misunderstood, not just the happy-path
callback. Use the contract's relevant state pairs, synthetic identifiers and
existing production-function/QML tests. Include wrong-target and unavailable
cases for action changes. Structural assertions may protect layout boundaries;
they cannot establish clarity, accessibility or appearance.

Before handoff, review the affected screen as a user: without reading the code,
can you identify what is connected, what is selected, and what each action will
change? A misleading cue is a defect even when its callback is correct.

## Verify and report

Follow the contract's installed-review and safety rules. If localized strings or
layout are affected, also use the repository's
[`localization skill`](../omavless-localization/SKILL.md). Use the local Omarchy
instructions when available for installation/capture; do not assume cloud
agents have a shell/display or prescribe stale command syntax.

Inspect captures yourself, fix findings and repeat affected states before asking
the owner for final judgment. Request human interaction only where automation
cannot safely establish the result. Never type into an unverified field, blindly
replay coordinates, or switch a tunnel merely to complete a screenshot matrix.

Report behavior, rendered UI and live integration evidence separately, with
exact candidate identity and unrun checks. Store durable sanitized decisions
and evidence in the repository; keep private captures outside Git. A local
commit is not permission to push, merge or publish. No VPN/locale/runtime state
change is required for a docs-only update to this skill or its contract.
