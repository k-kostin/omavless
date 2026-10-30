# OmaVLESS: agent entry point

These instructions apply to the entire repository. Detailed rules are retained,
not replaced by this short entry point.

## Before work

1. Fetch/prune remote metadata; compare main, current branch, open PRs and local
   changes. Preserve other agents' work; one branch has one active writer.
2. Read the [detailed agent guide](docs/development/AGENT_GUIDE.md) completely.
   Also read the owner-approved [execution policy](docs/development/EXECUTION_POLICY.md)
   for failure classification, risk-based review and agent/VM coordination.
   Execute new work through the [operating workflow](docs/development/AGENT_OPERATING_WORKFLOW.md).
   For resumed work and cost retrospectives, apply its
   [outcome and cost budget](docs/development/AGENT_OPERATING_WORKFLOW.md#outcome-and-cost-budget)
   before selecting the next complete item.
   Before allocating a checkout/cache/fixture and at phase closure, pause or
   handoff, complete the [retention gate](docs/development/README.md#proactive-retention-gate).
   The primary owns retirement; do not wait for an owner cleanup request.
3. Read [delivery roadmap](DEVELOPMENT_ROADMAP.md),
   [current status](docs/roadmap/CURRENT_STATUS.md),
   [development workflow](docs/roadmap/DEVELOPMENT_WORKFLOW.md) and
   [acceptance policy](docs/roadmap/ACCEPTANCE_ENVIRONMENTS.md).
4. For backend/runtime/protocol/packaging/TUI work, read
   [Rust migration](docs/roadmap/RUST_MIGRATION.md) and the owning feature contract.
   For new execution and failure handling, also read the owner-approved
   [execution policy](docs/development/EXECUTION_POLICY.md).
5. For UI work, use the [UI review skill](skills/omavless-ui-review/SKILL.md).
   For localization, use the [localization skill](skills/omavless-localization/SKILL.md).
   For user-facing wording, also read the shared [glossary](GLOSSARY.md).

## Current repository boundary

Since October16, `main` is the generated stable user distribution, including
help for the user's agent, not the full-source development checkout. Follow the
[source/distribution map](docs/development/STABLE_DISTRIBUTION_TRANSITION.md).
Developer rules and retrospectives stay in full source; never merge a full-source
PR into user main. The release's exact source remains immutable provenance.
The owner selected `development` on October16 as the full-source integration
home, starting at released source5edf4eb2, not an old beta. Read the
[development inventory](docs/development/DEVELOPMENT_INVENTORY.md) before carrying
old work; its old tests do not qualify a new integrated candidate.

## Non-negotiable boundaries

- Normal runtime ownership is Rust; QML is the frontend. Do not revive Python
  production fallback. Preserve `archive/python-legacy` as a frozen reference.
- Scoped R6 is accepted. Do not restart its unchanged acceptance or call deferred
  AUTO-1, DNS/provider or V0 checks PASS. Evidence belongs to exact tested heads.
- No private profiles, credentials, provider URLs or raw private logs in Git,
  command arguments or shareable output. No arbitrary privileged/shell IPC.
- Preserve the owner's requested network state. Host authorization and recovery
  follow the separate [procedure](docs/testing/HOST_AUTHORIZATION_ACCEPTANCE.md).
- New task branches use `dev/<topic>`; temporary development assemblies use
  `beta/<version>`, and scope-frozen release candidates use `rc/<version>`.
  Historical `dev/098-stable-preparation` (#726), `rc/0.9.8` and immutable
  `v0.9.8-fix.1` are retained delivery history, not current package selectors.
  Historical accepted artifact baseline is `rc/0.9.5`; preserve `rc/0.9.0`
  and its assets as superseded acceptance history until exact-ref disposition.
  Earlier #732/#735/#736 and Marketplace permissions are not standing authority
  for future main/release/submission actions. Beta/task branches are not release
  readiness. No permanent beta/rc lane or direct implementation on main;
  `development` is the separately owner-selected full-source home.
  `main` is the stable user release snapshot, including user documentation. Keep
  ongoing decisions/status in issues, PRs and the next candidate's docs.
  Internal RC freeze uses declared risk-based gates, not repeated public
  provisioning per version; private restore still needs installed integration.
- Merge and release/marketplace publication need their own applicable owner
  authorization; a green test or a cleanup task is not that authorization.
  This includes documentation-only merges: the former standing authorization
  is revoked. Follow the [documentation merge rule](docs/roadmap/DEVELOPMENT_WORKFLOW.md#agreed-documentation-only-updates)
  and include roadmap/docs reconciliation in every proposed main update.
- README is a product page, not an agent diary. Follow the
  [documentation and retention policy](docs/development/README.md). Preserve
  useful roadmaps, contracts and evidence; keep disposable session files outside Git.
- Public guides and in-app help must match current main; apply the
  [public-guide rule](docs/development/README.md#public-guides-follow-main).

For developer commands and navigation, see [CONTRIBUTING.md](CONTRIBUTING.md).
