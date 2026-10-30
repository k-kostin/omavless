# Development documentation and retention policy

The product front door is [README](../../README.md); the developer front door is
[CONTRIBUTING](../../CONTRIBUTING.md). Root [AGENTS](../../AGENTS.md) remains the
small discoverable instruction entry point. It requires the complete
[detailed agent guide](AGENT_GUIDE.md), which preserves the former root rules and
historical continuity notes. These developer instructions travel with full source.
Since October16, main instead ships user/support-agent instructions and an
immutable source entry, not developer retrospectives, roadmaps or build tooling.
See the [deployed source/distribution map](STABLE_DISTRIBUTION_TRANSITION.md).

Main's user documentation belongs to its stable release snapshot. Current issues
and PRs carry day-to-day status; checked `dev/docs/*` changes belong in selected
full source. A versioned beta integrates development; a versioned RC freezes
release scope. Both retain these canonical source paths. Before any owner-authorized main update, apply the mandatory
[release reconciliation checklist](../roadmap/DEVELOPMENT_WORKFLOW.md#release-reconciliation-checklist)
so source roadmap, contracts, evidence and agent guidance are reconciled, while
only selected user-facing files are exported into main. Even
documentation-only main updates require explicit owner authorization.

## Canonical map — retain useful work

- [Delivery ledger](../../DEVELOPMENT_ROADMAP.md) and
  [protocol roadmap](../../PROTOCOL_ROADMAP.md): preserved at their established
  paths; no roadmap is deleted or reduced for cosmetic cleanup.
- [Architecture/roadmap index](../roadmap/README.md): feature contracts and status.
- [T3 bounded selected-set close](T3_SELECTED_SET_CLOSE.md): explicit Product
  source successor, per-member outcomes and preserved single-close boundaries.
- [Original-store preservation](ORIGINAL_STORE_PRESERVATION.md): unreleased
  fixed-source plaintext preservation before native startup; no repair or
  profile-admission claim.

- [Development workflow](../roadmap/DEVELOPMENT_WORKFLOW.md): canonical branch,
  review and acceptance process. Its existing path is retained for continuity.
- [Acceptance policy](../roadmap/ACCEPTANCE_ENVIRONMENTS.md) and
  [testing index](../testing/README.md): exact-source evidence, failures and limits.
- [UI review](../../skills/omavless-ui-review/SKILL.md) and
  [localization](../../skills/omavless-localization/SKILL.md): reusable skills.
- [Release preparation](../../packaging/release/README.md): source/artifact
  identity and publication gates, not a product README section.
- [Historical development/release journal](CHANGELOG_REFERENCE.md): preserved
  checkpoint history, separate from the short user-facing root release notes.

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

## Public guides follow main

Owner requirement, 2026-10-14: **public user guides must describe what is
actually supported by current main**, not an older release, an unmerged
candidate or a planned feature. This covers README, user installation/update/
removal guides, Setup help, onboarding help and every public page reached from
the shipped interfaces. A valid URL to outdated instructions is still a defect.

Before an owner-authorized main update:

1. Trace the affected help buttons/links to their actual destination pages.
   Review the contents, not only HTTP success or file existence. Equivalent
   entry points must give the same current instructions or clearly lead to the
   same canonical guide.
2. Compare component names, supported versions, package pins, commands, consent
   choices and visible actions with the proposed main snapshot and its selected
   native delivery. Do not infer installed compatibility from a version label,
   a newer GitHub tag, or source-only feature code.
3. Reconcile the affected first-use, existing-user update, partial-setup and
   removal/reopen instructions in the same update as their implementation.
   Describe reachable next steps and genuine limitations; never advertise a
   planned remover/updater as available or suggest resetting private data to
   bypass a refusal. Use the owning risk-based acceptance rules, not a new
   blanket VM rerun for every documentation edit.
4. Keep older instructions as clearly labelled historical/version-specific
   references with a prominent link to the current guide. Preserve established
   help URLs/anchors through a current landing page where needed. An old
   stock-core recipe must not become the current managed-pair instruction.

Development-branch drafts may describe the intended next snapshot only when
clearly labelled; they are not current public help before main integration.
User pages contain instructions for the user, never reminders to an agent about
merging, reconciling main, evidence gates or how to describe a candidate. Keep
those notes here, in a release ledger or in the owning PR. A candidate guide needs
only a short truthful availability note and a runnable next step. Support replies
lead with a short command/action sequence and link to the fuller explanation.
Follow the [end-user editorial rule](../roadmap/UI_UX_CONTRACT.md#end-user-editorial-rule).
Immutable release/tag documentation retains its original version and evidence:
do not overwrite old artifacts to make them look current. Keep main's current
guide and release notes explicit about that distinction and unrun acceptance.
Documentation/navigation tests protect links and rule discovery; they do not
prove that the installed user flow works. Record affected content/behavior
review in the PR. Main, release and issue/Marketplace-write authorization remain
separate; this rule grants none of them automatically.

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

Record final verified outcomes, substantive failures, decisions and remaining
limits—not every typo, cancelled prompt or routine retry. Keep such transient
details only when they explain a real defect, security concern or evidence gap.

### Compact operational checkpoints

Keep one small current index outside Git, separate from immutable chronological
receipts. Its job is to answer what is actionable now, not reproduce the chat.
Use this shape (normally one screen, at most about 80 lines):

```text
Updated / scope / operator
Outcome | prepared / checked / integrated / admitted | exact input | next action
Active original operations: identity, original completion, bounded next observation
Frozen / sealed boundaries and explicit evidence limits
Links to owning PR, canonical procedure and original evidence/archive
```

Update that index in place. Before replacing an overgrown index, preserve its
complete original bytes in a private dated archive and verify the copy/digest;
keep an explicit link back. Do not delete, rewrite or turn old failures into
successes. A historical ACTIVE/PENDING field is a snapshot, never current live
custody or permission to observe/restart that scope. Link completed outcomes
instead of copying their full prefixes into every later task or heartbeat.
Keep private paths and raw logs out of source/PRs, and do not add report-only
source commits or resume a completed monitor merely to update bookkeeping.

For repeated offline source-admission preparation, use the maintained
[source fixture helper](../../packaging/release/source-fixture.py):

```sh
python3 -E -B packaging/release/source-fixture.py FULL_40_CHARACTER_SOURCE_COMMIT
```

It requires an existing owned parent under HOME's `.cache/ovt`, creates
only a new private workspace, explicitly fetches the selected local Git object,
and checks a clean detached source, exact tracked bytes, actual 644/755 modes and
absence of bytecode/build outputs. It disables ambient Git overrides, global
filters/hooks, automatic maintenance and lazy object fetching; only local file
transport is allowed, with interactive authentication disabled. No old fixture
is reused or repaired.
The returned distribution directory stays EMPTY for the actual exporter. Use
the returned command-local Git identity options only when committing that
later export; do not change global author settings or initialize the output
before export. This helper does not import candidate code, export, authenticate,
run CI/bootstrap, install or operate a VM. The actual newly generated bootstrap
and all applicable source/native/installed gates remain separate obligations.

### Acceptance reports stay current

Maintain one current summary/matrix at the top of the owning report: exact
tested inputs, earned results, remaining gates and present ownership. Put older
attempts below an explicit historical boundary; their original failures and
identities must remain intact, but a superseded pending list must not look like
the current task. Source-only, installed and administrator-repair results stay
distinct. Retain only the repair's effect on acceptance, not a general host
repair diary or copied terminal session.

Batch report-only commits at a meaningful completed gate or a useful handoff.
Use the existing PR body for intermediate progress while expensive CI runs.
Raw logs, screenshots, private backups and detailed machine-repair checkpoints
stay local; a cross-machine handoff carries only the bounded facts needed to
continue. This does not prohibit preserving a substantive new failure promptly.

## Local cleanup safety

New short-lived HOME-backed test fixtures belong under `~/.cache/ovt/`, in an
individual short directory, not directly in HOME. Set a HOME-backed `TMPDIR`
and explicit `CARGO_TARGET_DIR` for large Rust runs; private-path fixtures that
deliberately reject `/tmp` use the shared test allocator's checked HOME parent.
It refuses symlink/writable parents and does not repair existing permissions.
Keep Unix-socket paths short. Do not change or move an active fixture/TMPDIR
until its operator has settled its original outcome, including child processes.
Retained binaries, private logs and recovery evidence are not disposable cache;
keep an indexed path map if completed material is archived outside Git.

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
