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
For a parser admitting installed system-command output, derive its positive
fixture from that installed command's measured schema (sanitize private values).
An invented object satisfying the predicate is not compatibility evidence;
retain malformed/older-schema cases as separate negative tests.

For inert subprocess tests, verify that the selected mock actually ran.
A shell function mock does not automatically cross an external timeout/env
process boundary. Refuse unmocked effect leaves by default and retain a bounded
fixture witness; a passing assertion against a real desktop query is not inert
evidence. Preserve any accidental live read separately from the corrected run.

Dependent commands require a fail-fast admission boundary. Check checkout/head,
selected directories and permissions before launching a build or test; create
and verify the HOME-backed TMPDIR first. Keep dependent shell steps in one
fail-fast script, or explicitly check each tool result's exit code before the
next call. Printing only output drops the completion status. A running session
without an exit code is not success: retain and wait for that original command.
On failure stop dependent steps, classify reached effects and preserve the
original outcome before selecting a corrected successor.

Unix-socket fixtures must use the existing short, private shared allocator,
not clock-based names appended to an arbitrary build TMPDIR. A working default
/tmp path can hide this defect when HOME build paths become longer. Exercise
the affected socket controls with a long HOME-backed TMPDIR; do not shorten a
production endpoint, relax a socket deadline or alter the parser to fix tests.

Resolve helper APIs and Rust test namespaces in the selected owning snapshot,
not a newer main checkout or shared build cache. Source review does not replace
compilation. For an unfamiliar focused selector, inspect the module mapping or
the actual test list first; record the selected case count. Exit 0 with zero
selected tests is UNTESTED, not evidence that the requested behavior passed.

For socket-response fixtures, accepting the last peer is not completion of its
request. Keep fixture teardown separate from the behavior under assertion:
signal server shutdown only after the final reply is checked, with unwind-safe
cleanup. A teardown which takes the same mutex as the tested nonblocking read
can manufacture `busy` before its intended ownership check. Preserve exact
response assertions; do not accept `busy`, add sleeps/retries or weaken runtime
admission to hide that race. A controlled contention test proves precedence,
not the unobserved thread interleaving of an earlier CI failure.

For installer/updater work, include user-reached preparation, return after
Quit/terminal closure and continuation in that early scenario. Record every
fixture-only intervention and resulting proof limitation in the existing task
matrix. Do not replace a missing product step with administrator fixture work
and later call the helper result a complete journey. Before requesting release
publication, close its prepublication user-path gates; explicitly separate later
public-asset/download gates which cannot exist before authorized delivery.

For a user-facing reset/remover, start at the advertised installed state,
including a running application and a widget already removed through Omarchy.
An expert's earlier Quit does not earn the remover's shutdown prompt/consumer.
Exercise cancellation before removal and at the shutdown offer; compare the
actual packages, service/startup state, data and network, not only its exit code.
Put executable prerequisites inside the guided tool where possible, and retain
an explicit unsupported/unknown boundary rather than publishing a prerequisite
which the reporter's broken UI cannot reach.

For a full deinstaller, its completion matrix must include a previously used
installation, both running and normally stopped, widget already absent, and
continuation after each persistent removal phase is interrupted. Do not replace
these ordinary entries with a never-connected fixture or ask the user to attest
connection history. Infer the next operation from actual supported state and
retain tested recovery for partial effects. A safe refusal is a negative test,
not completion of that user path. If a required path remains unavailable,
report the remover as incomplete rather than narrowing its product goal after
implementation. Network ownership checks remain mandatory; missing recovery
must be implemented, not bypassed or hidden behind a generic support referral.
For restart-resumable file ownership, do not treat kernel-assigned device IDs
as persistent across boots. Use a retained object reference or an independently
qualified persistent identity, and test actual restart plus partial cleanup.
An interrupted staging create must not leave an unrecorded fixed-name blocker.

When changing package pins, exercise the latest actual published predecessor
through the complete advisory inventory and native update entry. A registry
lookup or local-rebuild refusal alone does not prove that the positive update
path is reachable. Preserve the old admission fences; use explicit tested
producer/payload identities rather than making an unknown installation qualify.

The selected delivery must also be removable by its shipped remover on every
supported architecture. Derive positive admission tests from the current pinned
payload identities and retained public predecessors, and run the actual successor
classification function; tests of an older helper or old hashes alone do not
qualify the new delivery. Retain modified-hash, development-label and unsafe-file
negative controls. Include matching DNS/service gates when their inputs change.

When changing staged package content or the source of installed help, run the
owning Rust packaging-payload integration tests as well as the Python/shell
staging checks before freezing artifacts. Keep exact bytes, inventory and modes;
update the intended user-help source rather than weakening a stale assertion.
An integration-test change can still affect a package's corresponding-source
archive even when it does not compile into the executable. Follow the reviewed
input-equivalence contract; do not exclude that file merely to reuse old assets.

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

When a handled-failure path promises to retain its owned graph, its diagnostics
must be bounded, best-effort and non-throwing. A broken output stream must not
turn parking into an unwind/exit that releases the graph; include panic hooks
and the parking wait itself in that review. Use inert broken-output and
interrupted-wait controls where applicable. This does not require a new logging
framework or promise resource survival after fatal process death.

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

Before starting or retrying expensive CI, inspect this PR's existing runs and
their exact heads. Keep the latest full check; cancel only confirmed superseded
PR runs, not another agent's work or independent main/tag checks. Cancelled is
not PASS. Batch report-only pushes at meaningful checkpoints rather than
restarting a heavy unchanged matrix for each receipt edit. Transfer an immutable
review candidate without a new push when practical. Never remove tests or
borrow an old head's green status to compensate for queue time; the final
remote candidate still needs its own required checks.

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

At a significant integration boundary the independent reviewer must also check
that the test's initial state/interventions and shipped artifact selection match
the advertised user scenario. Source safety approval and user-journey approval
are different conclusions. Do not delegate the primary's evidence classification.

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

Before each Git write, bind the tool's explicit working directory and check
the actual checkout root and expected branch. A preceding worktree-add does
not change the next tool's cwd. Refuse an unexpected main checkout rather than
assuming a dev branch was selected. If an own write was misdirected, stop,
preserve the exact own commit and user index/untracked state, then use a
separately checked exact-ref/path recovery; no broad reset or inferred permission
to alter the release branch. Record original/final refs and prevent recurrence.

For a feature that replaces persistent user data, earn one real loss-of-owner
and supported recovery-to-ordinary-availability result before selecting public
RC scope. A happy path and a same-live-holder Abort do not close that gate.
If unavailable, record the release decision explicitly and enforce exclusion
in the trusted producer/runtime, not just hidden client controls. Preserve the
research feature and its failed evidence for the owning next milestone.

Continue existing feature PRs; create a new PR for an independently reviewable
change, not every diagnostic token or formatting step. Reuse known-good tooling
and explain supersession without deleting useful historical evidence.

Report working behavior, current failure, concrete blocker and next experiment.
Test/PR counts and progress tokens are not product readiness. Complete a long
session when its agreed outcome is achieved or a genuine external/owner decision
prevents remaining in-scope work; save an exact-head handoff. Do not stop merely
because several steps ran, and do not fabricate closure to avoid a blocker.
New scope needs its own applicable owner direction.

### Delivery closure check

Before an outcome-reporting final answer, compare each owner-requested outcome
with the current task matrix, not just the last completed subtask. Keep that
operational summary above detailed receipts: outcome, done/actionable/external,
next action and responsible operator. Bind evidence to the selected input below;
prepared source, installed behavior and public delivery are separate outcomes.

Continue any authorized actionable step. For an external hold, name the missing
access, running operation or owner decision and the specific outcome it blocks;
do not let one unavailable environment stop independent work. A terminal CI
failure changes waiting into actionable diagnosis, not another unchanged wait.
Reconcile the summary before reporting; do not announce whole-task completion
from a passing subtask. An answer to an inline question does not close the task.

## 9. Lightweight orchestration retrospectives

For delayed lifecycle actions, read the actual grace/deadline and hold the
trigger beyond it before observing the intended outcome. A rapid disable/enable
cycle proves cancellation during grace, not sustained-disable behavior. For
wire fixtures, close the responding peer after its original response and bound
the client read; an EOF-waiting test must not hang indefinitely or be mistaken
for a product deadlock. Keep the first incomplete result separate from the
corrected test, and do not use completion markers as state proof.

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

### Retrospective closure and distribution

A retrospective is not complete merely because its findings were written down.
For each reusable finding, update the applicable project rule, contract or
maintained skill and add a regression or tooling correction where practical.
Keep one canonical rule, linked from the mandatory agent entry path; a private
note or report is supporting evidence, not its replacement. A finding which
needs no durable rule/test must have a short applicability reason, rather than
generating a ceremonial policy change.

Use the existing PR or handoff for a short finding → change → check → distribution
record. Distinguish analysis complete, implemented/checked, and integrated.
Record the rule/test location, exact available commit/PR and any remaining
integration owner. Do not call the change project-wide while its only copy is
local or in an unmerged feature branch. Until integration into the selected
full-source guidance, mark
distribution pending and give affected active agents the exact accessible
handoff; do not silently assume they read this chat. Since the October16
source/distribution separation, developer-rule distribution does not mean
copying those rules or a retrospective report into user main. Retain the
immutable released source; use the current developer entry for process updates.
Local reminders must point
back to the canonical rule and be reconciled after integration.

Before starting or resuming an older feature branch, check current remote main
agent guidance and follow its source entry for current developer guidance, not
only that branch's historical rules. User main's support-agent instructions do
not replace full-source development policy. Read changed applicable
instructions and carry the process corrections without importing unrelated
runtime code or rewriting old acceptance. Reuse the reviewed guidance identity
while unchanged; this does not require repeated full audits or blanket rebases.
Close a reusable finding with a check of the changed behavior or discovery, not
only the presence of a paragraph. Main/release/marketplace authorization remains
separate; a request for retrospective work is not an automatic merge permission.

For a repeated session-closure failure, check the next real outcome report
against the [delivery closure check](#delivery-closure-check): all requested
outcomes have a disposition, terminal results are current, and no authorized
actionable step was silently dropped. A navigation test can protect discovery
of this rule; it cannot prove that the primary agent actually followed it.

For an editorial journey review, include all text owners (frontend catalog,
shell and the actually selected native binary). Check names against the shared
[glossary](../../GLOSSARY.md) and actual controls, not memory. Extract local
review snippets by owning function/semantic marker, not fixed line offsets;
fail generation if a marker disappears and retain source hashes. Distinguish
production-formatter preview from installed effects and public delivery. The
[UI contract](../roadmap/UI_UX_CONTRACT.md#end-user-editorial-rule) owns this gate;
do not create a second copy or another standing reviewer to enforce it.

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

Apply the same admission rule to VM/host drivers: after each effect-bearing
phase inspect its original exit/result schema and independent required state
before launching the next effect or authorization prompt. A compound command's
final success, JSON-null projection, keys-sent marker or zero-test filter cannot
earn an earlier gate. Prefer separate tool calls or an explicit fail-closed
driver over semicolon-chained mutations. Do not use fixed sleeps as completion
proof; bound observation against actual state/phase and retain the failure.

Define a negative fixture's trigger at the product's actual observation boundary.
For example, TCP EOF in one direction does not prove that a bidirectional relay
has left its connection catalogue. Close only the fixture-owned endpoint's
required direction, retain its original identity, and earn bounded catalogue
absence through the ordinary data API before selecting the negative operation.
That observation is not a new effect permission: retain the original preview and
let the product's fresh conditional check decide the result. Keep a previous
legitimate success distinct from the intended refusal scenario; fix the fixture,
not the product's safety deadline or authorization boundary, when the trigger was
incomplete.
