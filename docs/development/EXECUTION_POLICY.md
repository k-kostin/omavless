# Development execution and failure policy

Status: owner-approved direction, 2026-10-05; documentation candidate, not a
runtime implementation or release. This is a durable policy for all agents,
not a one-session exception.

## Authority and scope

Read this policy with the [agent guide](AGENT_GUIDE.md),
[development workflow](../roadmap/DEVELOPMENT_WORKFLOW.md),
[acceptance policy](../roadmap/ACCEPTANCE_ENVIRONMENTS.md) and owning feature
contract. It governs new development work and review. It does not change
production ownership, privilege, input schemas or acceptance results by itself.

Historical reports and frozen experiments retain their exact source, original
contract and outcome. Do not edit a failed receipt into success, run a sealed
old recipe under new assumptions, or treat this document as a reset capability.
If an executable enforces the older policy, prepare and review a changed
successor before using it. Record the changed guarantee and affected tests.

The development-process rules below supersede older procedural requirements
for blanket full-graph re-review and blanket refusal of diagnostics in NEW
work. They do not silently override product safety guarantees or grant new
authority to mutate an uncertain old scope. The T4 service boundary below is
the specific owner-approved architectural alternative; legacy ancillary
receiver requirements remain separate.

Main merges (including documentation), release/tag/assets and marketplace
actions still require their applicable explicit owner authorization. Approval
to prepare these rules or start a long session is not publication permission.

## 1. Plan around executable outcomes

For each active feature write a short completion matrix: supported scope,
required behavior, selected exact head, deterministic checks, installed/VM
checks and genuinely unavailable host cases. Broad roadmap headings are not
permission to add every future feature or declare an entire track finished
from one passing slice.

Prefer the smallest real end-to-end scenario early, then add fault and recovery
cases. Run compilation and environment preflight before privileged execution.
Synthetic controls complement, but never replace, the real integration gate.
Do not create prerequisite research unless a concrete scenario needs it.

After two unsuccessful diagnostic cycles without narrowing the failure,
reassess the hypothesis or design with an independent reviewer. This is a
checkpoint to change method, not a retry allowance or a reason to stop all work.
Unlimited session duration means persistence toward outcomes, not unlimited
scope or an obligation to keep generating intermediate artifacts.

## 2. Classify failures before choosing continuation

| Failure class | Allowed next step | Must not imply |
| --- | --- | --- |
| Source, formatter, build or ordinary deterministic test failure, with no uncertain external/resource-owning effect, privileged or unprivileged | Read bounded relevant diagnostics, fix the source/environment, rerun affected gates | That a previous failed gate passed, or that compilation is privileged acceptance |
| Known completed operation with a proven refusal/no effect | Diagnose the refusal; use a separately admitted new attempt after correction | That a nonzero exit alone proves no effect |
| Effect may have occurred; original ownership/completion is uncertain | Stop dependent mutations; preserve available evidence/resources; use separately scoped observation or designed recovery | That a later observation repairs the original operation or permits blind resend |
| Actor/channel loss, fatal death or missing evidence | Report unavailable/unknown; block dependent operations | Live custody, absent resources or successful rollback without proof |

Classify the actual reached operations, not merely the exit code or filename.
An outer build script may have staged a privileged unit; it is not automatically
a pure compiler failure. Preserve the original failed result in either case.

No automatic retry of uncertain effects, guessed ownership, arbitrary privileged
IPC, broad firewall cleanup or permissive parser is introduced. A new directory
name alone does not isolate effects in a shared kernel, manager or network.

## 3. Useful diagnostics without false authority

Diagnostic reads and effect authorization are distinct. New experiments should
provide bounded phase/error output from the start so routine failures do not
require a new bespoke observer implementation every time.

Define the diagnostic surface before the run: exact permitted captures or
service queries, maximum bytes/time, privacy classification and independence
from mutation paths. Prefer a reusable reviewed observer with narrow fixed
inputs. No caller-selected command/path or producer-defined authority token.

For ordinary public-source compilation/tests, bounded relevant error messages
are acceptable after privacy inspection; raw logs are not categorically banned.
Private configuration, credentials and provider content must not enter Git,
argv or shareable output. Private runtime diagnostics remain private and their
public projection is allowlisted. Do not dump a whole raw log as a shortcut.

Read-only does not mean harmless: observation must not run lazy initialization,
change state, trigger cleanup/reaping, drain a protocol needed by a retained
owner or access a secret through an unreviewed parser. If safety is unknown,
review or isolate the observer first. Observation can explain a failure but
cannot create original ownership, reclassify an uncertain result or authorize
compensation. Frozen old scopes keep their observation restrictions until a
separately authorized observer is selected.

## 4. Risk-proportionate review and exact-head gates

Full primary and independent review is required for a NEW privileged boundary,
authority/ownership protocol, side-effecting recovery, descriptor acquisition
or changed security/fault guarantee. Review reached failure paths and dependency
contracts, not just the happy-path diff.

For a change within an already reviewed boundary, review the complete delta
plus its affected call sites/invariants and run affected tests. Mechanical
formatting, documentation and fixed path/pin updates do not require repeatedly
reading an unchanged graph. A path or pin can change executable selection or
authority: classify such a change by its effect, not by its small size.

Retain the reviewed baseline SHA/artifact identities and review result; inspect
the transitive impact when they change. Do not borrow acceptance across a
changed boundary. Focused gates run during iteration; full combined and
installed gates run on meaningful integration checkpoints and the final exact
candidate. An unchanged gate need not be rerun for unrelated prose.

## 5. Development VM and cleanup

One explicitly designated operator owns the only development VM at a time.
Other agents perform source/test work without controlling it. The physical
owner's network/services are outside VM authorization.

Prefer reproducible disposable VM fixtures and reviewed reusable launchers to
many ad-hoc privileged wrappers. Bound process lifetimes and collect useful
diagnostics by design. Ordinary owned build processes may use normal timeout
and cleanup supervision; that is not an authorization to signal an unknown
resource-owning worker.

Cleanup of a known owned completed fixture can follow its pre-reviewed recipe.
Uncertain effect-bearing state requires the feature's explicit recovery path,
or an owner-authorized disposable-VM reset with the recovery boundary recorded.
A VM snapshot/reset is not product rollback acceptance. Preserve needed private
profiles/backups and evidence first; never reset or broadly delete the host.

Place build and temporary storage under HOME, not the quota-limited /tmp. Reuse
compatible build caches; periodically inspect and remove only classified
disposable outputs. Stopped scopes and unique commits are not routine cache.
Follow the [retention policy](README.md) for exact-target cleanup.

## 6. T4 service fault boundary

For the NEW retained-manager actor SERVICE direction, the approved guarantee is
availability-oriented, not descriptor survival after fatal process death:

- Capture/recheck and proof-consuming operations remain inside one authenticated
  original actor operation; do not export copied live authority or arbitrary FDs.
- While a nonfatal actor is alive, uncertain operations consume their capability,
  stop dependent work and retain resources that it actually owns. Do not claim
  ownership of descriptors the backend installed but did not report.
- Actor/channel loss or fatal death makes the service unavailable. No fallback,
  guessed raw-FD adoption, stale success or unsafe automatic retry is permitted.
  Permanently revoke that original context and every outstanding capability;
  late replies cannot reactivate it. No claim of descriptor custody after fatal
  death is made.
- Later availability requires a separately admitted fresh service/context with
  defined reconciliation of any persistent effects. It cannot inherit authority
  from, reconnect or reactivate the lost actor. Product recovery remains a
  separately tested operation.
- Before activation define aggregate bounds for live quarantined actors,
  descriptors and other retained resources. Reserve the required capacity before
  acquisition. At the bound refuse new allocation/service admission; do not
  evict uncertain owners to make room. Fresh contexts cannot bypass the bound.

This is not a drop-in relaxation of the old caller-local ancillary Bundle
contract. Existing receiver research may remain blocked under its own stronger
requirements; it is not marked accepted by the service alternative. Implement
and independently review the new interface, origin/credential binding,
resource bounds and fault tests before activating it. No implementation or
product acceptance follows from this documentation decision.

K1's kernel fail-closed, dedicated-table ownership and crash/reboot guarantees
are unchanged. The T4 boundary cannot be used to weaken kill-switch protection.

## 7. Agents and model roles

The primary agent owns integration, scope, VM scheduling and evidence. Assign
independent writers per feature/worktree, with explicit no-overlap boundaries,
expected deliverable, acceptance scenario and immutable handoff head.

Use a capable implementation model (normally Sol) for feature code and tests.
Use Astra for bounded difficult architecture/security questions and independent
review where a second perspective changes the decision. Do not require an
Astra call for routine edits or delegate all integration responsibility to it.

When available and permitted by the calling tool, Luna or Terra can handle
closed low-risk tasks: documentation/link checks, public metadata inventories,
finite fixture generation from a fixed specification or mechanical migrations.
The primary agent verifies their output. Do not make a smaller model the sole
authority for firewall/ownership decisions or give it unsupervised VM control.
These are routing guidelines, not model-performance or cost benchmarks.

Respect actual concurrency slots. Prefer primary + two independent writers +
one reviewer/helper; replace or finish the helper before starting another.
Parallelize independent work, not overlapping edits or simultaneous VM control.
Delegation is optional when coordination would cost more than the task.

Distinguish delivery of a message from starting work. Before assigning a new
deliverable to an existing agent, check whether it is running or completed.
Use the available follow-up/resume operation for an idle or completed agent;
a queued message alone may not trigger another turn. Confirm a short start or
progress checkpoint before scheduling dependent integration work. Reuse that
agent and its owned branch rather than spawning a duplicate writer.

## 8. Checkpoints, branches and session completion

Continue existing feature PRs; create a new PR for an independently reviewable
change, not every diagnostic token or formatting step. Reuse known-good tooling
and explain supersession without deleting useful historical evidence.

Report working behavior, current failure, concrete blocker and next experiment.
Test/PR counts and progress tokens are not product readiness. Complete a long
session when its agreed outcome is achieved or a genuine external/owner decision
prevents remaining in-scope work; save an exact-head handoff. Do not stop merely
because several steps ran, and do not fabricate closure to avoid a blocker.
New scope needs its own applicable owner direction.

## 9. Lightweight orchestration retrospectives

The primary agent reassesses the working process at a meaningful integration
checkpoint, after the same bottleneck recurs, or approximately every two to
three hours of active work if neither event has occurred. This is an in-session
checkpoint, not a scheduled extra agent, permission to keep an idle session
alive, or a reason to rerun unchanged reviews and tests.

Use existing observations and ask three questions: what delayed an executable
outcome, what smallest reusable change removes the cause, and how will the
next actual attempt demonstrate improvement? Normally spend a few minutes;
do not produce another full audit or speculative optimization backlog.

Record a short cause/change/check entry in the owning PR or handoff. When a
solution is reusable, keep one maintained procedure, helper or narrowly scoped
project skill and link it from the agent guide. Inspect existing skills and
other active agents' workflow PRs first; reuse them rather than duplicating
them. Local machine paths, secrets and private captures stay outside Git.
Do not create a new branch for every retrospective: use an existing owned
scope, or one separate workflow PR when the change is independently useful.

Examples worth fixing are repeated VM unlock rediscovery, an incompatible
package/bundle discovered only after installation, repeated socket/temp-root
setup, full unchanged graph re-reviews, and serial VM waits while independent
source work is available. Prefer preflight, compatible cache reuse and explicit
single-operator VM scheduling. Password entry, a reset or a successful source
test never substitutes for boot, original completion or product acceptance.

For short-lived UI confirmations, preflight the complete capture/inspection/
input latency before starting the effect-bearing scenario. A model/tool round
trip may exceed a five-second dialog even when each local command is fast.
Do not widen a protection deadline or send blind keys to make the test pass.
When automation is appropriate, separately review a bounded controller which
checks the actual rendered target, original focused window and conservative
deadline before its one confirmation. Missing, changed, truncated or late
observations refuse; input delivery is not the effect result. Keep the original
expired run distinct from the later corrected scenario.

Ask an independent reviewer when the proposal changes a difficult ownership,
privilege or fault boundary; routine process notes do not require Astra.
Failure classification and applicable owner authorization remain unchanged.

Before carrying a shared fix across feature branches, check that each target
actually contains the affected implementation/test and its owning wiring.
Do not import an absent research precursor just to make a cherry-pick apply.
A source-only conflict is a normal diagnostic: preserve unrelated work,
resolve or abort the carry, and retain each tested runtime's exact identity.

For a sequential acceptance harness, mock only records earned by preceding
completed phases. Add a first-observation control that refuses every other
lookup: a convenient pre-populated mock can hide a self-dependency and waste
an otherwise valid VM run. Diagnose harness failure separately from product
failure, preserve the failed scope, and verify the narrow successor before
selecting a fresh scope. Do not continue past an unresolved observation merely
because the underlying command returned success.
