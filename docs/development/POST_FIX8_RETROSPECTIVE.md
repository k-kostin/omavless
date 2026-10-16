# Fix.8 delivery retrospective — October16, 2026

Scope: the last setup/removal/Help/onboarding and user-distribution finalization.
This is a full-source developer record, not a user guide or another release.
The primary agent owns the missed journey checks and premature completion
reports. They were not caused by the owner failing to specify enough detail.

## Outcome and limits

Fix.8 and user-only main were delivered through #795 from full source5edf4eb2.
The current [source/distribution map](STABLE_DISTRIBUTION_TRANSITION.md#current-map--after-fix8-delivery)
records exact identities; #794/#795 retain gate receipts. Old failures remain
failures. The last frontend delta has installed x64 evidence; additional
installed ARM evidence was waived, not earned. Aryan's response is pending.
No new tests below can turn a past unrun installed check into PASS.

Local verification: `./tests/run.sh` passed 814 Python tests (two existing opt-in
skips), JS/QML suites and documentation navigation. Focused remover, onboarding
and CI-isolation checks also passed. No Rust workspace rebuild, VM/network
operation or new installed acceptance was needed for this docs/discovery delta.
Remote PR CI remains a separate check, not inferred from this local result.
The earlier 813-test local pass did not predict the first PR's cleanup failure.

## Findings, prevention and checks

| Finding and cause | Canonical prevention | Check / disposition |
| --- | --- | --- |
| A reset restricted to never-used installs was described as a full remover. Safety refusal was mistaken for delivering the requested journey. | Execution policy §1: used/running/stopped/widget-absent and interrupted phases; actionable recovery, no connection-history attestation. | `test-uninstaller.js`, `test-uninstall-guided.js`, `test-clean-reinstall-root.js`; installed lifecycle reports keep their exact scope. The restricted tool is not the advertised remover. |
| The new app pair was admitted by setup but not its remover. An unchanged helper was treated as unchanged compatibility despite new inputs. | Execution policy §1: selected delivery must be removable on each architecture; test the actual classifier against current pins and retained predecessors. | `test-uninstaller.js` derives both architectures' positives from selected identities, with modified/development/unsafe negatives. Helper0250 fixes admission without weakening runtime guards. |
| Core-ready screens kept repair/help actions and initial reads briefly looked like missing components. Callback correctness was reviewed without action relevance or temporal states. | UI contract state/target review and initial-checking rule; review every equivalent entry, not one wizard screenshot. | Existing setup/core-review and onboarding expression tests plus installed EN/RU review; rendering evidence is not source-only or live-network evidence. |
| Finish appeared complete before native persistence; ordinary close/reopen repeated onboarding. A refusal also lost the retained final step. | UI contract: acknowledgement → persisted state → desktop close/reopen; test real asynchronous publication order and non-success reopen too. | `test-native-onboarding.js` covers queued reads, refusal, unknown, restart and bounded waiting. Exact installed x64 fresh first-use used the ordinary setup path, not a manually edited completion flag. |
| Heavy duplicate/superseded CI and report-only pushes delayed a stable candidate. | Execution policy §4: inspect exact runs, batch checkpoints, retain one required exact-head result; cancelled is not PASS. | `test_rust_ci_isolation.py`; original watcher/API outcomes retained separately. Queue duration does not justify gate bypass or relabelling. |
| Session answers stopped with authorized work remaining; local handoff chronology looked current after supersession. | Delivery closure check and report-retention policy: one operational summary, all outcomes disposed, continue actionable work, name an actual external hold. | Apply the rule to the next real outcome report. Navigation tests protect discovery only, not agent compliance. Completed release and this audit are separate outcomes. |
| Main became user-only, but source rules still said the migration was pending and developer rules belonged on every main update. | Deployed source/distribution map; workflow and mandatory source AGENTS distinguish user help from developer guidance. | This follow-up reconciles entry points and current summaries. Navigation guards check discovery and reject the stale transition assertion. Main/release source bytes remain unchanged. |
| Hundreds of Draft PRs and stacked branches made historical experiments look like current delivery work. | Cleanup lifecycle and development inventory: inspect exact heads, dependencies, source preservation and custody; reachability is not feature acceptance. | Owner-selected development starts at released full source, not an old beta. Old work must earn affected integration checks. No automatic merge/closure/deletion or blanket frontier integration. |
| This retrospective's first CI failed while deleting a synthetic Git objects directory. Temporary Git commands could launch auto-maintenance beyond command completion. | Execution policy §1: disable automatic/detached maintenance per fixture invocation, never change global Git config or ignore cleanup errors. | Trace regression failed on old fixture helper and passed after the scoped correction; pairing/release/export suites passed. Original CI race was not traced or reproduced and is not relabelled PASS. |

Existing canonical rules are reused rather than copied into another checklist.
UI publication/persistence and remover-current-input regressions already shipped
with fix.8; this follow-up reruns their inert checks for retained-rule confidence,
not a new installed acceptance claim. VM-specific Qt/package-manager repair and
artifact-download trouble remain environment/transport observations; they do
not justify product fixes or another general policy without evidence.

## Distribution and remaining integration

The deployed product corrections are in fix.8. The developer-rule reconciliation
in this branch is prepared/locally checked, not yet integrated into the selected
shared full-source home. Do not call it project-wide or copy this report into
main. Mandatory source AGENTS → agent guide → execution policy discovers the
rules and this record. Before resuming an older branch, read this applicable
source guidance without importing unrelated runtime code or rewriting evidence.

The full-source carrier and open dependency/evidence branches remain retained.
The owner selected development from the released baseline; integrating #796 and
authorizing exact-ref cleanup remain separate from green product CI. Raw logs, screenshots,
per-branch inventory, machine paths and private fixtures remain outside Git.
