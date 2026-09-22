# OmaVLESS development workflow

Status: repository workflow policy, updated 2026-09-22.

This workflow applies to current plugin work, incremental Python -> Rust
migration, standalone Arch/NixOS packaging and later TUI work.

`AGENTS.md` is the mandatory entry point. Backend/runtime/protocol/TUI work must
also read `RUST_MIGRATION.md`.

## 1. One long-lived branch

`main` is the only long-lived release source of truth. It preserves the stable
product snapshot, including README, roadmap and agent documentation. Do not
advance it between owner-authorized updates, even for documentation-only work.
Day-to-day development continues in task branches and versioned candidates;
issues and PRs provide the live operational status without changing release SHA.

Do not maintain permanent `develop`, `rc`, `alpha` or `beta` branches. A normal feature branch
+ Draft PR already expresses an alpha state. A cloud-ready Draft with local
gates pending expresses the next maturity state.

A temporary `rc/<version>` branch is allowed for a named release/integration
candidate (for example `rc/0.8.0`). Record its exact constituent heads and pending
gates. Assemble completed, independently checked roadmap checkpoints there,
including their docs; do not collect every intermediate edit. During final
acceptance, allow only fixes needed for that release, returned to owning PRs.
It carries the same documentation and agent rules as main, not a separate
user-facing tree. It is not a release tag or permission to merge/publish.
Delete it after the release or recorded supersession. Existing `beta/<scope>`
scratch work is grandfathered until safely retired, not a new branch convention.

The owner-approved `archive/python-legacy` exception is a frozen full-repository
snapshot at `aa5873783c019edc303a732e55ea8c85f1f0b090`. It preserves the Python
reference and its tests, not a supported parallel release or development branch.
Retain it during cleanup; new work still targets `main`. See the
[retirement sequence](LEGACY_RETIREMENT.md).

## 2. Short-lived branch roles

New task branches use `dev/`, regardless of whether a human or agent writes them:

```text
dev/<topic>
dev/fix/<topic>
dev/docs/<topic>
```

Use bounded feature names which make ownership obvious. Do not rename existing
open/evidence branches solely for cosmetics or break their exact-head handoffs.
The `dev/` prefix is a namespace, not a permanent integration branch. Scoped R6
is accepted; historical migration branch names are not a new work queue.

### Concurrent agent ownership

A remote branch plus its Draft PR is the visible ownership record for active
work; chat and handoff files are supporting context, not locks or sources of
truth.

Before starting a branch, every local or cloud agent must fetch/prune and check:

- open PRs for the same roadmap stage or subsystem;
- recently updated remote branches with related names or changed files;
- whether a handoff's base/head still matches GitHub;
- whether the proposed diff overlaps another active branch.

One branch has one active writer. A handoff transfers the exact remote head and
states that the previous writer has stopped. If another agent advanced the
branch unexpectedly, fetch and reconcile those commits before doing more work;
do not overwrite them. Independent agents may work in parallel on independent
branches.

Push a meaningful checkpoint and open a Draft PR early enough that other agents
can discover the active scope. Do not use empty commits or placeholder PRs as
branch reservations. Before any history rewrite or merge, fetch again and
compare the observed remote SHA. Reconstructed history may use
`--force-with-lease` against that exact SHA; unguarded force pushes are not
allowed.

### Agreed documentation-only updates

Owner-approved replacement, 2026-09-22: the September 17 standing authorization
to merge agreed documentation automatically is **revoked**. A request to record
a decision authorizes preparing/checking/pushing its documentation PR, not
updating `main`. Main merges, including docs-only merges, require the owner's
explicit instruction to update main. Release/tag/assets and marketplace changes
retain their separate applicable authorization.

Use narrow `dev/docs/*` PRs; run documentation/navigation checks and normal CI.
Mark checked work ready when appropriate, but keep it outside main until the
authorized update. Readiness and integration into RC do not mean publication.
Preserve canonical paths on every branch: main documents its released snapshot,
the candidate documents the intended next snapshot, and issues/PRs expose the
current queue. Do not put roadmap/agent rules exclusively in a separate branch.

Every proposed main update must complete the [release reconciliation checklist](#release-reconciliation-checklist).
There is no docs-only exception for bypassing that checklist or the main hold.

### Branch cleanup lifecycle

After inclusion in an authorized main update, delete the source branch and
prune local remote-tracking refs. RC integration alone is not grounds to delete
the only independently reviewable source/evidence branch.
After closing a superseded PR, delete its branch once its unique commits have
been classified. Temporary integration, recovery and CI-automation branches
must be removed when their durable result is merged or recorded elsewhere.

A cleanup audit classifies every retained branch as one of:

- active open PR or intentionally preserved evidence branch;
- fully reachable from `main` / merged PR;
- closed and demonstrably superseded by accepted work;
- unmerged work with unique commits requiring preservation or explicit owner
  disposition;
- disposable automation with no unique durable result.

Never delete merely because a branch is old or has no PR. Inspect its unique
log and diff first. Never delete the source branch of an open evidence PR solely
to make the branch list tidy. If a concurrently produced PR describes stale or
contradictory state, close it with a reason rather than merging misleading
documentation.

## 3. Stacked branches

Stack only for a real dependency.

After predecessor merges:

1. update/rebase successor onto new `main`;
2. resolve shared docs/changelog without dropping either decision;
3. rerun materially affected cloud/parity checks;
4. retarget to `main`;
5. perform successor's own local gates;
6. merge only with separate owner approval.

Do not ceremonially rebase a long-lived evidence PR such as V0 solely because
unrelated docs/UI work advanced `main` if its exact tested patch remains valid.
Rebase when its runtime relationship materially changes or remaining evidence
becomes actionable.

## 4. PR maturity states

### CI event policy

The full Test workflow runs for every pull request (including Drafts), pushes
to `main`, and all tag pushes. Feature-branch pushes do not also run a duplicate
workflow: open the Draft PR at the first meaningful checkpoint to obtain cloud
evidence. Until then, run the same `./tests/run.sh` and `./tests/run-rust.sh`
gates locally. Every later PR synchronization runs both gates again.

No path filters or Draft skips are used, so documentation and workflow changes
retain the same checks. Keep the existing `test` job name and read-only token
permissions; do not replace `pull_request` with privileged
`pull_request_target`. Merged `main` and release-tag checks remain independent
of the PR check. This reduces duplicate runs, not acceptance coverage or
exact-head host gates. Branch/tag filter semantics follow the
[GitHub workflow contract](https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-syntax#onpushbranchestagsbranches-ignoretags-ignore).

### Draft / cloud work

Implementation or cloud/parity validation incomplete.

### Cloud-ready

Exact remote head passed all available deterministic/static/parity checks but
one or more declared host/integration checks remain.

### Local-validation pending

Useful when explicitly waiting for Try Omarchy, bare-metal, Arch or NixOS
acceptance. Keep unchecked list visible.

### Merge-ready

Exact merge candidate passed declared cloud/parity/local gates, diff was
re-reviewed after any rebase/fix and owner approval is given.

### RC-integrated / not released

A completed checkpoint is included at recorded exact heads in `rc/<version>`.
Its source PRs remain discoverable; remaining stage/release gates are explicit.
This does not make it merged into main, finish its whole roadmap stage, or
authorize a tag, package publication or marketplace request.

### Merged

Accepted implementation is in `main`.

### Experimental

Independent product maturity label. A merged protocol can remain Experimental
while broader provider/server evidence is missing.

### Migrated

Do not use this word loosely. For a subsystem it means Rust is the accepted
production owner and any retained Python code is explicitly oracle/rollback,
not another live owner. Full project migration is not complete until R6.

## 5. Two active development lanes

### Plugin completion lane

Current QML/plugin work may continue while Rust migration proceeds:

- localization;
- presentation/accessibility/navigation;
- current diagnostics/routing/import/subscription UX;
- bug/security fixes;
- V0 evidence when fixtures exist.

Do not freeze a current bug because the backend will eventually be Rust.

### Rust migration lane

R0-R6 proceeds according to `RUST_MIGRATION.md`.

Avoid expanding large Python backend areas which are about to move. Once R2 is
available, new large protocol adapters are Rust-first. P4 WG/AWG depends on R2
and its own protocol-specific private/core/live/security gates. V0 remains an
independent maturity track and does not globally block P4 implementation.

## 6. Rust migration PR template requirements

Every R-stage PR body must answer:

1. What exact subsystem is moving?
2. Who owns it before this PR?
3. Who owns it after this PR?
4. What language-neutral contract/fixtures existed before replacement?
5. What Python/Rust differential or golden result passed?
6. Are any differences intentional? Why are they contract-correct?
7. How are credentials/private fixtures protected?
8. Does production plugin/runtime path change?
9. What host gate is required?
10. Can Python code be removed, or is it still oracle/rollback?
11. What exact head SHA was tested?

A PR which only shows `cargo test` against newly written Rust tests does not
establish migration parity.

## 7. Establish reference behavior before replacement

Before rewriting a subsystem, capture the behavior worth preserving in a
language-neutral form whenever practical:

- accepted/rejected fixture corpus;
- stable machine errors;
- canonical normalized model;
- redacted preview;
- generated core config semantics;
- migration outputs;
- routing outcomes;
- lifecycle state transitions.

Do not snapshot secrets into Git.

Known Python bugs are handled by explicit contract correction + regression test,
not blindly duplicated in Rust.

## 8. Differential test safety

- credentials never in argv;
- private fixtures untracked/mode `0600` where applicable;
- CI uses only credential-free fixtures;
- failure output does not dump raw profile/subscription input;
- shareable parity reports use IDs/categories, not endpoints or keys;
- bounded stdin/private files preferred over ad-hoc environment variables for
  sensitive comparison input.

## 9. Cloud agent responsibilities

Cloud agents may:

- research upstream behavior;
- implement bounded QML/Python/Rust changes;
- add language-neutral fixtures;
- run deterministic differential/static/security checks;
- prepare Draft PRs;
- rebase genuine stacks;
- update canonical docs.

Cloud agents must not claim unrun:

- Quickshell integration;
- real TUN/routes/systemd ownership;
- live provider/server interoperability;
- physical network behavior;
- Nix generation/package behavior;
- Python-absence R6 product behavior unless actually exercised in an installed
  environment.

## 10. Try Omarchy responsibilities

For applicable current plugin/R4/R5 candidates:

1. fetch exact candidate SHA;
2. verify base/diff;
3. run `omarchy plugin validate`, tests, installed QML checks and relevant
   Rust/Python checks;
4. execute PR-specific TUN/systemd/core/server checklist;
5. keep credentials/endpoints/provider names out of public output;
6. report failures on owning PR;
7. rerun affected gates if head changes;
8. merge only after explicit approval.

Try Omarchy is ARM64/virtualized evidence; label it honestly.

## 11. Bare-metal responsibilities

Bare-metal is secondary confidence unless concrete hardware-sensitive behavior
requires it. Required examples include physical NIC transitions, suspend/resume,
x86_64-only behavior or host firewall/kernel behavior not faithfully represented
in Try Omarchy.

Do not demand bare-metal merely because old roadmap text did.

## 12. Standalone Arch/NixOS responsibilities

### Arch

When R5a begins, test native package/runtime path independently of plugin-only
acceptance: install/update/remove, service, Mihomo/TUN readiness, CLI/runtime,
store migration, restart/rollback and Python-free operation when R6 applies.

### NixOS

When R5n begins, test package/module/wrapper semantics, stable entry points,
user service, privilege negative case, generation update/rollback, garbage
collection/stale store path and shared state migration.

Never reuse Arch evidence as Nix package proof.

## 13. R6 special procedure

Apply the 2026-09-13 owner scope amendment in
[acceptance environments](ACCEPTANCE_ENVIRONMENTS.md#3-r6-python-absence-gate):
enabled Last/pinned acceptance remains AUTO-1, while proven Off/default/native
operation closes the local migration gate. Keep the
[closure record](../testing/R6_LOCAL_CLOSURE_2026-09-13.md) and its failed/unrun
checks separate from publication readiness. Do not repeat completed host gates
just to update documents, or claim an unrun gate passed.

R6 must intentionally make Python unavailable to the installed normal
OmaVLESS path and exercise all required production operations. Merely not
observing a Python process during one happy-path connect is insufficient.

Record:

- package/head SHA;
- how Python was made unavailable to OmaVLESS;
- plugin bridge result;
- status/import/subscription/routing/connect/diagnostic/startup coverage;
- process tree showing no normal `backend.py` owner;
- recovery/rollback result;
- no venv/pip/runtime-Python installation requirement.

Only after this gate may T2 production implementation begin.

## 14. TUI workflow

Before R6: research/design/prototypes only.

After R6:

- Ratatui client is developed in narrow T2 PRs;
- it consumes only semantic Rust control API;
- no Python compatibility calls;
- concurrent plugin/TUI actions must serialize;
- terminal close/reopen must not disturb tunnel;
- Omarchy launch/focus/theme checks are distinct from standalone Arch/Nix TUI
  behavior.

## 15. Exact SHA discipline

Every handoff involving runtime/migration/local validation records:

- base SHA;
- feature head SHA;
- dependency PRs/stages;
- cloud/parity checks;
- unchecked host gates;
- current owner language/path.

If head changes, previous evidence applies only where unaffected; repeat
materially affected checks.

## 16. Release and marketplace model

Keep concepts separate:

```text
main                 stable owner-approved release snapshot, docs included
dev/<topic>          short-lived task branch + PR
rc/<version>         temporary exact release candidate
version tag          immutable project release
marketplace snapshot exact reviewed plugin commit
native package       exact built/tagged application release
```

Marketplace verification covers an exact commit, not all future commits with
unchanged runtime bytes. The current catalog implementation compares observed
upstream SHA with the verified SHA: a difference produces `update-unverified`
while retaining `verificationSnapshotStatus: verified` for the old snapshot.
There is no Markdown-only exemption. A pending exact-commit request is not
invalidated by another commit, but does not cover that new commit. See the
[marketplace projection](https://github.com/omacom/omarchy-plugin-marketplace/blob/main/scripts/catalog-verification.mjs)
(checked 2026-09-22). Consult actual request/registry state, not historical
release prose, before claiming verification or publication.

Keep main unchanged while preparing the next candidate. Batch applicable docs
and code into the owner-authorized update, then submit the final exact SHA only
when marketplace submission is separately authorized. A serious documentation
or security correction may justify an earlier owner-authorized update; never
hide a necessary fix merely to retain a badge.

### Release reconciliation checklist

The RC PR body must record these items before proposing an update to main:

- Starting main SHA and all included source PR/head identities; no silent
  overwrite of another agent's work. List excluded/deferred PRs and reasons.
- Reconcile `DEVELOPMENT_ROADMAP.md`, `docs/roadmap/CURRENT_STATUS.md`, affected
  feature contracts, agent rules and user guides with the actual candidate.
  Explicitly review pending `dev/docs/*` PRs and accepted issue decisions.
- Preserve historical exact-head evidence and failed/unrun gates. Label new
  work RC-integrated, not released/complete, until the applicable event occurs.
- Record resolved doc conflicts and remaining decisions; do not cherry-pick
  implementation while forgetting its acceptance or roadmap changes.
- Run combined checks; repeat host gates only where integration materially
  changes tested behavior. Record the exact RC head and remaining release gates.
- Obtain explicit owner authorization for the main update. Afterwards fetch,
  verify remote main, reconcile source PRs/issues and clean up only safely
  included branches. Retain independent evidence PRs and the Python archive.
- Keep tag/assets and marketplace authorization separate. When authorized,
  submit the final main SHA and record the actual request/outcome in its PR or
  issue without another ceremonial main commit that immediately changes SHA.

The marketplace plugin never silently builds Cargo sources or installs the
standalone package.

The root README is a product page, not the development ledger. Keep detailed
agent instructions discoverable through root AGENTS on every branch; use the
[documentation and retention policy](../development/README.md) for their
placement. Repository cleanup must preserve useful roadmaps and evidence, not
hide them in a separate develop branch or ship them as runtime payload.

## 17. Normal flow examples

Plugin/UI feature:

```text
main -> dev/<topic> PR -> declared gates -> rc/<version> + reconciled docs
     -> combined gates -> explicit owner main authorization -> main
```

Rust migration slice:

```text
main
  -> establish/reference fixtures
  -> Rust candidate + differential gate
  -> host gate if applicable
  -> Rust ownership cutover for slice
  -> merge
  -> later PR removes Python oracle when safe
```

T1/R5:

```text
Python plugin owner
  -> Rust runtime package present but inactive
  -> migration preflight
  -> explicit one-owner cutover
  -> plugin semantic bridge
  -> compatibility window
  -> R6 Python retirement
```

This workflow exists to avoid two failure modes: a giant unverifiable rewrite,
and a permanent hybrid product which requires both Python and Rust forever.
