# Development documentation and retention policy

The product front door is [README](../../README.md); the developer front door is
[CONTRIBUTING](../../CONTRIBUTING.md). Root [AGENTS](../../AGENTS.md) remains the
small discoverable instruction entry point. It requires the complete
[detailed agent guide](AGENT_GUIDE.md), which preserves the former root rules and
historical continuity notes. These instructions travel with the code on every
branch; they are not hidden only in a permanent develop/rc branch.

## Canonical map — retain useful work

- [Delivery ledger](../../DEVELOPMENT_ROADMAP.md) and
  [protocol roadmap](../../PROTOCOL_ROADMAP.md): preserved at their established
  paths; no roadmap is deleted or reduced for cosmetic cleanup.
- [Architecture/roadmap index](../roadmap/README.md): feature contracts and status.
- [Development workflow](../roadmap/DEVELOPMENT_WORKFLOW.md): canonical branch,
  review and acceptance process. Its existing path is retained for continuity.
- [Acceptance policy](../roadmap/ACCEPTANCE_ENVIRONMENTS.md) and
  [testing index](../testing/README.md): exact-source evidence, failures and limits.
- [UI review](../../skills/omavless-ui-review/SKILL.md) and
  [localization](../../skills/omavless-localization/SKILL.md): reusable skills.
- [Release preparation](../../packaging/release/README.md): source/artifact
  identity and publication gates, not a product README section.

Moving a document requires updating relative links and checking readers in
tests/scripts/skills. Keep a useful stable navigation path where an established
external handoff depends on it. Do not duplicate mutable canonical policies in
several places; concise summaries link to their authority.

## README editorial contract

README serves a person arriving from the marketplace. Keep:

1. a plain explanation of the VPN application and its benefit;
2. one reviewed, credential-safe screenshot;
3. useful everyday capabilities;
4. truthful installation instructions/link for the actual availability state;
5. compatibility/limitations a user needs before installing;
6. help, license and attribution, with a small contribution link at the end.

Do not append session transcripts, agent handoffs, PR progression, R-stage
checklists, hashes, test counts or architecture essays. Completion of a coding
task is not a reason to edit README. Those details belong in owning docs or the
PR. During an unpublished candidate, keep a short honest availability warning
and a guide link, not the engineering history of why release is pending. Do not
replace the warning with a released installation claim before the actual gate.

Screenshots may use neutral invented display names to protect privacy, but
cannot fabricate Connected/health or claim fixture rendering is live evidence.
Only owner-selected, inspected assets belong in a product gallery. Raw captures
remain outside Git. Preserve attribution and useful limitations during editing.

## Retention by purpose

| Material | Destination / rule |
| --- | --- |
| Durable architecture, roadmap, security or agent rules | Keep in their canonical docs/skills; preserve decision history. |
| Significant acceptance/failure evidence | Keep a bounded report under `docs/testing/`; date/source/environment/results, not private raw logs. |
| Cross-machine handoff needed to finish a release | Keep one current indexed prompt; mark older chronology as historical. Existing useful handoffs are not deleted merely for age. |
| Per-turn plan, scratch transcript, temporary screenshot or intermediate report | Outside Git or in the relevant PR discussion; do not create another permanent session diary. |
| Build/package artifacts and recovery archives | Outside the checkout in retained artifact directories; never remove them as worktree cleanup. |
| Private fixtures, stores, backups, exports, results | Private local storage outside Git; never publish. |

Do not purge existing reports to meet a file-count target. Separate current
navigation from historical evidence with indexes. If later archival is useful,
preserve content and provenance and explicitly update discovery links.

## Local cleanup safety

Remote cleanup and local worktree cleanup are distinct. An old checkout may
contain a unique commit, ignored build/recovery output, untracked private data
or an active process. Before removing one, inspect its identity, Git changes,
ignored/untracked content, branch/PR ownership and reachability or documented
equivalence. Preserve anything uncertain. Never force-remove a dirty worktree.
The frozen Python archive and active/evidence PR branches are retained.

For a clean worktree without unique local artifacts whose HEAD is reachable from main
(only inspected, disposable compiler caches may remain),
remove only that exact checkout with ordinary `git worktree remove`; keep a
public path/head disposition record outside Git or in the cleanup PR. Delete a
merged local branch with `git branch -d` only after confirming no active PR or
owner uses it. Squash/cherry-pick equivalence requires explicit review, not a
force-delete shortcut. Prune missing worktree registrations only after checking
their exact targets. No broad recursive delete of a workspace or artifact root.
