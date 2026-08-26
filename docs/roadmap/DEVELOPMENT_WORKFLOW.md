# OmaVLESS development workflow

Status: repository workflow policy, updated 2026-08-26.

OmaVLESS development is split between cloud GitHub agents and a real Omarchy
machine. The workflow must preserve one accepted source of truth while still
making it obvious which changes have only cloud evidence and which have passed
real TUN/Quickshell/systemd/server validation.

## 1. Do not use permanent `alpha` and `beta` branches by default

A permanent three-branch ladder (`alpha` -> `beta` -> `main`) looks attractive
but duplicates information already present in pull requests and acceptance
checklists.

For this project:

- a cloud-only feature branch or Draft PR already acts as the **alpha** state;
- a Draft PR whose cloud gates pass but whose real Omarchy checklist is still
  open already expresses **cloud-ready / local validation pending**;
- after the exact head passes its required real Omarchy checks and receives
  owner approval, it can merge directly to `main`.

Keeping permanent `alpha` and `beta` branches would create three moving bases,
extra rebases and ambiguity about which branch is authoritative. It would also
make stacked PRs harder to audit.

## 2. Branch roles

### `main`

`main` is the only long-lived development source of truth.

A runtime/network/security change reaches `main` only after all acceptance gates
declared by that PR pass. When a change requires a real Omarchy smoke test,
`main` therefore contains only the version which passed that test.

Documentation-only work may merge without an invented VPN smoke gate after
normal review and cloud checks.

`main` is not required to equal the currently published marketplace commit.
The marketplace/public release is an exact immutable SHA and, for formal
releases, should also receive a version tag. Documentation or later accepted
work may advance `main` while the marketplace still points to an older exact
snapshot.

### Short-lived feature branches

All implementation work happens on narrowly scoped branches such as:

```text
codex/<topic>
fix/<topic>
docs/<topic>
```

The existing `codex/...` convention is suitable for cloud agents and should
remain the default for agent-authored work.

A feature branch may contain code which has never run on the Omarchy PC. That
fact is recorded in its Draft PR body and checklist rather than encoded in a
permanent branch name.

### Stacked feature branches

A successor may temporarily target its unmerged predecessor when the work truly
depends on it. The PR body must state the stack explicitly and record the exact
base/head SHAs.

After the predecessor merges:

1. update/rebase the successor onto the new `main`;
2. resolve shared documentation/changelog edits without dropping either set;
3. rerun all cloud checks on the new exact head;
4. retarget to `main`;
5. perform the successor's own real Omarchy gates;
6. merge only with separate owner approval.

Do not treat testing a predecessor as implicitly validating a successor.

## 3. When a `beta` integration branch is justified

A beta branch is optional and **temporary**, not part of the normal ladder.
Create one only when the real Omarchy session needs to validate multiple
otherwise independent or already-reviewed PRs together before they can safely
merge separately.

Examples:

- two changes touch the same TUN/lifecycle state machine and interaction risk is
  meaningful even though each PR has independent tests;
- a migration must be exercised with several coordinated follow-up slices;
- a release candidate needs a fixed combined tree for a longer local soak.

Use an explicit scoped name such as:

```text
beta/0.8.0
beta/tunnel-coordinator
```

Rules for a temporary beta branch:

- it is cut from a known `main` SHA;
- it contains only named candidate PR heads;
- it is never the base for unrelated new feature development;
- fixes found during beta testing go back to the owning feature PR/branch and
  are then re-integrated, rather than living only on beta;
- successful beta evidence does not bypass the individual PR audit trail;
- delete the beta branch after its candidates merge or the experiment is
  abandoned.

In other words, beta is a **test assembly**, not another source of truth.

## 4. Why there is no permanent `alpha`

A permanent alpha branch would mostly duplicate Draft PRs while making cloud
agents coordinate a shared unstable branch. Shared unstable branches increase
accidental coupling, make rollback harder and obscure which commit belongs to
which task.

For OmaVLESS, `alpha` is better represented by:

```text
feature branch + Draft PR + cloud evidence
```

This preserves isolation and lets multiple cloud agents work in parallel.

If a future automated nightly/preview build needs a named aggregation point,
create an explicit automation branch for that distribution purpose. Do not make
it the development base by default.

## 5. PR maturity states

Branch names are not maturity labels. Use the following state vocabulary in PR
bodies and the development ledger.

### Draft / cloud work

Implementation is still changing or cloud validation is incomplete.

### Cloud-ready

The exact remote head has passed every available deterministic/static/security
check, but one or more declared checks require the real Omarchy machine.

Cloud-ready is not merge-ready for a runtime slice with a local gate.

### Local-validation pending

Often equivalent to cloud-ready, but useful when the PR is deliberately waiting
for one scheduled Omarchy session. The unchecked local list must remain visible
in the PR body.

### Merge-ready

The exact head that will merge has passed its declared cloud and local gates,
its diff has been re-reviewed after any rebase/fix, and owner approval has been
given.

### Merged

The accepted implementation is in `main`.

### Experimental product status

Independent from Git maturity. A merged protocol may remain **Experimental**
until enough real provider/server interoperability evidence exists. Do not keep
accepted code off `main` merely to encode an experimental product label.

## 6. Cloud agent responsibilities

A cloud agent may:

- research upstream code/docs/releases;
- implement bounded changes;
- add deterministic tests;
- run available CI/static/security checks;
- prepare Draft PRs and exact handoff instructions;
- rebase/retarget stacked branches after predecessors merge;
- update documentation and roadmaps.

A cloud agent must not claim that it validated:

- real Omarchy CLI behavior when the CLI is unavailable;
- Quickshell integration with installed imports;
- real TUN/routes/systemd ownership;
- live server/provider interoperability;
- censorship-network behavior which requires a real network;
- host firewall behavior which was not actually exercised.

Every such missing gate stays unchecked.

## 7. Omarchy-machine responsibilities

The local agent/session is the acceptance environment for behavior that depends
on the actual desktop/network host.

For each candidate:

1. fetch the exact remote head SHA recorded in the PR;
2. verify the diff and base before running anything;
3. run `omarchy plugin validate`, the full test suite, relevant shell/Python
   checks and `qmllint` with the installed imports;
4. execute the PR-specific live checklist;
5. keep credentials/endpoints/provider names out of public GitHub output;
6. report failures on the owning PR;
7. if a fix changes the head, repeat the affected checks on the new exact head;
8. merge only after explicit owner approval.

Local experiments made directly on the machine must be converted into a normal
branch/PR before they become accepted repository history.

## 8. Exact SHA discipline

Every handoff involving local validation records:

- base branch and base SHA;
- feature branch and head SHA;
- relevant dependency PRs;
- automated checks already completed;
- unchecked local gates.

If the branch is rebased or receives another commit, previous local evidence is
not automatically evidence for the new head. Repeat the tests materially
affected by the change and record the new SHA.

## 9. Release and marketplace model

Use separate concepts:

```text
main                 accepted development history
version tag          immutable project release
marketplace snapshot exact reviewed/published commit
```

For the current 0.7.0 marketplace publication, documentation should continue to
name its exact reviewed SHA even after `main` advances.

For future formal releases, prefer an annotated or otherwise intentional Git tag
such as `v0.8.0` on the exact release candidate that passed the release gate.
Marketplace submissions should reference/record that exact commit. Do not use a
moving `beta` or `main` branch name as the security identity of a published
build.

## 10. Recommended normal flow

The default workflow is therefore:

```text
main
  |
  +-- codex/feature-a ---- Draft PR ---- cloud-ready
  |                              |
  |                              +-- Omarchy smoke
  |                                      |
  |                                      v
  |                                  merge to main
  |
  +-- codex/feature-b ---- Draft PR ---- ...
```

For a real dependency:

```text
main
  |
  +-- A (Draft PR -> main)
       |
       +-- B (stacked Draft PR -> A)

A merges
B rebases to new main
B gets its own gates
B merges
```

Only for a justified multi-PR integration test:

```text
main + candidate A + candidate B
              |
              v
       beta/<scope>   (temporary)
              |
        combined Omarchy test
              |
     fixes return to A/B PRs
              |
       candidates merge
              |
        beta deleted
```

This keeps the cloud/local distinction explicit without creating permanent
parallel histories whose meaning drifts over time.
