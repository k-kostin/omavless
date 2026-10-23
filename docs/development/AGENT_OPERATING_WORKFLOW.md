# Agent operating workflow

Owner-directed process correction, October21, 2026. Use for NEW work and
continuation; frozen attempts retain their original contracts and outcomes.
This sequences the [execution policy](EXECUTION_POLICY.md), not a new gate,
privilege, retry allowance or publication permission. Full-source guidance is
distributed only after integration; a local checked candidate is not integrated.

## 1. Resolve authority once per scope

Read current owner decisions, task ownership and source instructions. Record
the target, allowed actions, exclusions and decision reference, without secrets.
Reassess only when the target/scope changes, authority is revoked or actual
evidence conflicts. A compaction or unchanged heartbeat is not revocation.

| Situation | Next action |
| --- | --- |
| Existing owner authority covers the exact target/action | Continue under that authority; do not ask for it again. |
| Authority exists, but an input/tool/prerequisite is genuinely unavailable | Diagnose the technical dependency; request only the missing input once. Do not relabel it as missing permission or invent a human-only requirement. |
| New target/action or explicit owner choice is required | Explain the precise scope change and obtain that decision before effects. |
| Prior effect or original completion is uncertain | Stop dependent mutations; follow the execution policy's observation/recovery path. |

Where guest sudo entry is authorized, use only its verified normal guest prompt
with legitimately available input. Keep credentials out of argv, environment,
files, logs, memory pages and automation prompts; never search or guess them.
Unavailable input remains a real dependency. Agent-entered authorization is not
physical-human acceptance. Guest authority does not include the physical host.

## 2. Keep one current plan

Keep the operational checkpoint normally within 80 lines. Each requested outcome
has exact inputs, operator, earned gates, remaining dependency and next action.
Use explicit states: actionable, external-wait, owner-choice, complete. A
terminal failure is actionable diagnosis, not unchanged waiting.

Replace superseded current text. Keep original receipts and historical limits
behind links under the [retention policy](README.md#compact-operational-checkpoints).
Archive the complete prior index before compaction; historical ACTIVE/PENDING
is not present custody or an instruction to restart it. Read the applicable
history when a dependency or diagnosis needs it, not on every heartbeat.

## 3. Measure the baseline before effects

Use the existing owned observer/driver and one minimal read-only preflight.
For an installed scenario bind the actual guest/process identity, executable
and shell/config root, native/frontend inputs, service active AND enabled state,
requested network state and the state-preservation categories the scenario needs.
Record safe values/digests only; do not inspect private contents unnecessarily.

Derive expectations from that measured baseline. A different shell path or
enabled unit is an applicability mismatch, not permission to change the machine
to fit the script. Revalidate volatile identities immediately before their
effect; reuse unchanged static evidence only within its proven applicability.
Separate files-installed proof from distinct loaded-feature proof.

## 4. Qualify the harness before the live journey

Prefer the maintained procedure and the smallest real end-to-end scenario.
An affected-boundary review is enough for a narrow correction; full independent
review remains required where the execution policy changes authority or safety.
Do not manufacture a new wrapper/observer merely to rename each refusal.

Derive sanitized positive fixtures from the actual selected interface/schema.
Validate envelope fields and types exactly. For preservation comparisons,
separate state from documented per-request metadata such as a correlation ID.
Exclude ONLY the demonstrated volatile field; reject every other changed field
and retain independent private-byte checks where required. Prove the old
comparison RED and the corrected one GREEN with inert controls before live use.

Exercise expected enabled/disabled baselines, wrong identity/root, malformed
and bool-versus-int receipts, child-success/outer-refusal, late/incomplete output
and absent prior-phase records as applicable. Verify nonzero selected cases and
the project's actual pinned toolchain. A model fixture is not installed proof.

For a finite server, reserve time for the journey and owned cleanup before
starting. Aggregate success follows original child/server EXIT0 and required
listener/resource closure, not an early success marker. Uncertain mutation
completion stops dependents; it does not authorize a compensating delete,
automatic restart or extending the original deadline.

## 5. Classify each original result

Retain both the product child outcome and the harness qualification separately.
Design bounded privacy-safe stage diagnostics before execution: closed stage
and failed-predicate codes, reached-effects class, original child exit/late flag
and identity. Preserve UNKNOWN when the first failed predicate was not captured.
Do not print exception strings, raw remote bodies, headers or private snapshots
to improve a generic refusal. Use sanitized exception classes/numeric status
only where that diagnostic surface is already admitted.

| Observed result | Continuation |
| --- | --- |
| Original child and required postconditions succeed | Admit that exact gate and continue. |
| Child succeeds, outer verifier refuses | Keep the gate failed/unqualified; diagnose the harness separately. Later observations do not retroactively pass it. |
| Known refusal before effects | Correct the demonstrated applicability/harness defect; qualify a separately admitted successor. |
| Effect/completion unknown | Preserve the scope and stop dependent mutations; no blind resend/cleanup. |
| Hosted infrastructure transport failure | Preserve the original; use only the applicable bounded retry authority. A green retry is a new attempt, not a rewritten failure. |

## 6. Publish meaningful checkpoints, not bookkeeping pushes

Before final producer/pin publication, reconcile ALL selected-tuple consumers:
metadata, frontend/DNS expectations, inventory, update/remover allowlists and
their existing positive/negative tests on both declared architectures. Run the
actual focused selectors and confirm the hosted runner discovers new tests.
Keep assertions, provenance and the reviewed native-input projection intact.

Discover each exact-head CI dependency once; retain run/event/attempt/job IDs.
Follow only that running dependency to all required terminal jobs/steps. Stop
polling a completed gate. Reopen only gates affected by new inputs or a declared
contract; report-only changes belong in the owning checkpoint, not source pushes.
Required own PR and independent exact merged-push checks, export/source-CI and
first generated bootstrap remain separate under existing contracts.

Record original queue/compile/test/step durations when available; UNKNOWN is
better than inventing a breakdown. Optimize only a measured bottleneck, keeping
assertions, fault/configuration coverage, crypto, trust and deadlines. No
automatic full local duplicate matrix or speculative cache/CI redesign.

### Early feedback and timings

The maintained Rust runner executes all existing format/type/lint configurations
before long tests. Its `OVT_CHECK_START` / `OVT_CHECK_END` records identify each
command, elapsed seconds and original exit status without echoing arguments.
Use these command wall times with Cargo's build/test output; a command duration
alone does not distinguish compilation, fixture setup and test execution.
Preserve the complete recipe and serial deadline-sensitive fixtures when changing
scheduling. Inert recipe tests prove command preservation and fail-fast behavior,
not a hosted speedup. Measure total CI time on the next ordinary candidate;
start a separate profiling experiment only for a remaining specific question.

## 7. Wait quietly, close explicitly

Keep a continuation compact, normally within 25 lines, with links to the current
checkpoint and canonical procedure. Store only scope/boundaries, current exact
dependency, original operation/attempt IDs, next admissible action, real missing
input and completion disposition. Do not copy old receipts or credentials.
Use the product's scheduler/wait mechanism; retain one observer per dependency.

### Hosted CI completion

For a NEW exact public-source CI dependency, prefer one supported completion wait
over coarse scheduled wakeups when the current tool session can remain active.
After binding its repository, full head, workflow, event and attempt, an existing
GitHub CLI session can use [the documented run wait](https://cli.github.com/manual/gh_run_watch):

```sh
timeout 30m gh run watch RUN_ID --repo OWNER/REPO --compact --exit-status --interval 30
```

Replace the three placeholders with the already discovered dependency. Keep the
original local child/session and consume output through waits of at most 60s;
stay quiet on unchanged progress. Start no second observer or custom service.
The 30-minute bound ends only this read-only observer, never the hosted run.
An observer timeout, transport/auth refusal or lost result is not a CI outcome:
retain it, apply stalled-operation triage and use the existing scheduler if a
live wait is unavailable. Keep existing authentication unchanged; unsupported
credentials are a tool limitation, not a reason to obtain different credentials.

Completion is a signal to read back the bound run/attempt and ALL actual jobs
and steps once through the existing evidence path. CLI exit success alone does
not admit a gate. An unexpected head/event/attempt invalidates this wait's
applicability; preserve the observation rather than adopting the new attempt.
Stop at terminal state; completed gates stay closed. Frozen attempts retain their
original observation restrictions. PR, independent merged-push and distribution
admission remain separate. Record actual completion-to-observation lag; a shorter
poll interval is an expected improvement, not measured time saved.

### Stalled-operation triage

Before waiting, record the original operation/attempt, queued/running/unknown
state, observable progress signal, last material progress time, next check and
the exact outcomes it blocks. In particular, queued is not started, and a
still-present folder proves neither execution nor failure. Never invent a
completion API, operation ID or proof of absence.

When two consecutive observations show no progress, or after 30 minutes without
an observable start/progress signal, reassess instead of repeating the same
poll. These are review checkpoints, NOT operation timeouts: they do not permit
kill, resend, compensating cleanup, deadline changes or a late PASS. A known
running compile/test may legitimately take longer; compare its actual phase
and recorded durations before calling it stalled. Preserve the original result.

1. Draw a small resource/dependency map: operation, potentially affected paths,
   refs/registrations, required completion proof and blocked next actions. For
   an unknown mutation, block only actions whose safety depends on that outcome.
   A shared Git directory can be a real dependency; document it rather than
   assuming every other task is either safe or blocked. Independent reads,
   preservation planning and separately authorized source work remain eligible.
2. Use one bounded supported status/diagnostic path if available. A missing
   status/completion API is a concrete tool blocker, not evidence of success,
   failure or ownerlessness. Keep the original operation fenced. Any new recovery
   or remaining authority must come from the owner, not this triage.
3. Replan all requested outcomes. Execute authorized independent work; list
   exhausted alternatives and the exact unavailable input/tool where none remain.
   Preserve useful future work with its source carrier and next task before
   retiring a working folder; branch deletion is a separate decision/check.
4. Report once when productive work stops: last actual result/time, what is
   waiting, what cannot safely proceed, and the smallest required decision or
   external change. An unanswered question is not repeated on each pulse. A
   scheduled observation is not development progress. Keep an explicitly
   requested monitor active, but back off unchanged observations (normally up
   to hourly) and notify only a meaningful change. Respect an owner's explicit
   cadence; absence of that cadence is not a reason for frequent empty polls.

### Outcome reporting

Lead with the plain-language outcome, last actual change and time, then the
blocker and next action. If no development or cleanup occurred since the last
checkpoint, say so when asked; do not substitute tool-call counts or "continuing"
for progress. Report GitHub branches, local branches and working folders
separately. Explain a working folder as a local project copy when that distinction
matters; a reviewed catalogue, saved future-work plan, removed folder and deleted
remote branch are different outcomes. Avoid internal identifiers unless they
help the owner make the next decision; retain exact technical receipts below.

For unchanged external waits, do no VM probes, closed-CI replay or memory imports.
Notify only material completion, a new real error or a needed decision, subject
to higher-priority required progress updates. A scheduled wakeup is not renewed
effect authority. An available authorized next step must not be silently dropped.

Apply the [whole-task closure check](EXECUTION_POLICY.md#delivery-closure-check).
Settle original owned children, restore the declared state and classify cleanup
under existing ownership/retention rules. Save the material checkpoint through
the existing memory procedure. Pause only the completed continuation; a later
task does not reactivate it. Report local adoption, checked source and integrated
distribution separately. Documentation tests protect discovery, not actual
agent compliance or measured performance improvement.
