# Internal beta: agent-assisted human acceptance

Owner decision, 2026-10-07. Applies to temporary `beta/<version>` assemblies,
not release/main promotion. Read with the [workflow](../roadmap/DEVELOPMENT_WORKFLOW.md),
[execution policy](EXECUTION_POLICY.md) and
[host procedure](../testing/HOST_AUTHORIZATION_ACCEPTANCE.md).

## Inclusion before final hands-on acceptance

A feature may enter internal beta with **manual-validation pending** when its
declared exact-head programmatic checks and affected review pass, and its
remaining gap is an explicitly described, bounded installed interaction.
Do not demand completed human acceptance before making that feature available
for the very beta session intended to obtain it. Keep it experimental/opt-in
where appropriate; do not advertise release readiness or silently change defaults.

Eligibility requires:

1. A real end-to-end client/runtime path, not only a model or simulated button.
   For a plugin/TUI session, the actual action is reachable in that interface.
2. Matching reviewed artifacts and a defined activation/preparation procedure.
   The agent prepares prerequisites before handing the user a short action list.
3. Applicable deterministic, integration-fixture, build/lint and combined checks
   passed; ignored tests are not counted as passed. Record exact source/results.
4. Affected ownership, privacy, privilege and failure boundaries are reviewed.
   No unresolved unsafe effect, unknown prior operation or fabricated authority.
5. A concrete human test card: action, visible expectation, agent observation,
   direct question if useful, stop condition and final restoration expectation.

Unfinished UI/API wiring, missing production event registration or a missing
ownership/recovery mechanism is **implementation pending**, not a human-only
gap. Unknown kernel protection, cold/fatal recovery or incompatible artifacts
cannot be settled by clicking a button and seeing a green icon. Separately
required machine/fault/hardware evidence remains required; no roadmap guarantee
is weakened by this policy.

## Division of work

The human performs the declared ordinary plugin/TUI interactions and describes
what was visible or confusing. The agent first inspects code/tests, prepares the
exact build and safe test data, then observes only the separately reviewed
bounded read-only semantic/status/log sources. Logs never manufacture authority.
The agent diagnoses defects, captures sanitized evidence and repairs code;
the user is not asked to discover missing implementation or debug raw protocols.

One step at a time: explain its target/effect, wait for that step's outcome, then
compare display and independent facts. Ask direct questions such as “After
Escape, is the confirmation gone and is the connection still active?” instead
of a generic “does it work?”. Do not require terminal `ready`/`settled` chains
for ordinary navigation. Actual privileged operations retain their separate OS
authorization procedure; passwords and bearer input never belong in chat/logs/Git.

For each tested batch retain privately: source/native/frontend/helper/core
identities, environment/architecture, starting state, exact interactions,
observer scope/bounds and results. Share only sanitized counts/codes and safe
labels. A screenshot, a human “looks fine”, or an open website alone is not
evidence of the whole routing/protection policy.

## Test card and outcomes

Each card contains:

| Field | Required content |
| --- | --- |
| Candidate | Exact source/build and actual client entry; capability enabled or absent |
| Preparation | Known starting state, synthetic/test data, observer selected and operator |
| Human action | One or a short sequence of ordinary visible controls; exact target |
| Visible result | What should appear, remain unchanged, or be disabled |
| Agent observation | Bounded permitted facts confirming target/outcome, not a whole raw log |
| Question | One concrete UI/result question, only when useful |
| Stop/restoration | Failure/unknown rules and agreed ending state |

Use **PASS**, **FAIL**, **UNRUN** or **INCONCLUSIVE** per scenario. A passed
scenario is not whole-feature acceptance. Reproduce/fix an ordinary deterministic
failure after classification; do not blindly repeat an operation with uncertain
effects. Unknown/pending authority or ownership stops dependent mutations.
Retain evidence and use the existing recovery design or a separately authorized
disposable-VM administrative action; reset is not product recovery acceptance.

Hands-on sessions are operator-scoped. The VM master retains the single VM
until explicit handoff; two agents/humans must not drive it concurrently.
Permission to include a feature/test plan is not permission to change the
physical PC's VPN, stop another VPN, restore private data, provision privilege
or publish. Agree the actual environment/actions before execution.

Main/RC/release gates stay distinct. Successful beta clicks do not authorize
main merge, tag/assets or Marketplace. Carry this policy and actual evidence
through the next candidate's normal documentation reconciliation.

Current example: [0.9.8 hands-on plan](../testing/BETA_098_MANUAL_PLAN.md).
