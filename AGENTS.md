# OmaVLESS: agent entry point

These instructions apply to the entire repository. Detailed rules are retained,
not replaced by this short entry point.

## Before work

1. Fetch/prune remote metadata; compare main, current branch, open PRs and local
   changes. Preserve other agents' work; one branch has one active writer.
2. Read the [detailed agent guide](docs/development/AGENT_GUIDE.md) completely.
   Also read the owner-approved [execution policy](docs/development/EXECUTION_POLICY.md)
   for failure classification, risk-based review and agent/VM coordination.
3. Read [delivery roadmap](DEVELOPMENT_ROADMAP.md),
   [current status](docs/roadmap/CURRENT_STATUS.md),
   [development workflow](docs/roadmap/DEVELOPMENT_WORKFLOW.md) and
   [acceptance policy](docs/roadmap/ACCEPTANCE_ENVIRONMENTS.md).
4. For backend/runtime/protocol/packaging/TUI work, read
   [Rust migration](docs/roadmap/RUST_MIGRATION.md) and the owning feature contract.
5. For UI work, use the [UI review skill](skills/omavless-ui-review/SKILL.md).
   For localization, use the [localization skill](skills/omavless-localization/SKILL.md).

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
  Current assembly is `beta/0.9.8`, based on the internal `rc/0.9.7` source
  checkpoint. Publicly accepted artifact baseline is `rc/0.9.5`; preserve `rc/0.9.0` and its assets as
  superseded acceptance history. Beta/task branches are not release readiness. No permanent
  develop/beta/rc lane or direct implementation on main.
  `main` is the stable release snapshot, including its documentation. Keep
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

For developer commands and navigation, see [CONTRIBUTING.md](CONTRIBUTING.md).
