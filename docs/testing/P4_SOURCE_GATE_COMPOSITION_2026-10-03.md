# P4 source-gate composition

Development-only follow-up to the [family/MTU matrix](P4_IP_FAMILY_MTU_MATRIX_2026-10-03.md).
Executed source: `1c26f574775d80837958885c35593c9cea3b5d01`.
This narrow older-stack composition is not the complete current RC, a release,
installed WG/AWG capability or a main/RC update.

## Inputs and retained negatives

The matrix and its two measured wire runs remain at `ce4850a`. This composition
does not transfer whole-head VM acceptance from those artifacts. Counts and
strict preservation limits stay in the linked report.

Two independently reviewed inputs were omitted from the old stack:

1. Current RC commit `727682f6de03e1cd7ae7051e132624a155b8ba56` changes only
   the auxiliary cleanup comment and bounded STOP600ms→3s/DRAIN900ms→4s.
   It is an ancestor of current RC/frontier, not the original P4 matrix.
   This is not a newly invented timeout workaround. The old `ce4850a` Test
   CI Cleanup failure's cause remains unestablished; this input does not prove
   its remediation.
2. The [concurrent first-lock correction](SETUP_CONCURRENT_REFUSAL_2026-10-03.md),
   commit `6168a0c`, retains exact Busy/UnsafePath fail-closed outcomes, both
   owned children and verified successful retry/no-op metadata. Its production
   CLI remained byte-identical at its original checkpoint.

The matrix's report-only `6e268c2` failed Test37104635284 on precisely that
second omitted contract: actual UnsafePath versus expected Busy. Nine other
first-use cases passed. This is distinct from the older auxiliary Cleanup
failure. Both raw receipts remain privately retained; neither is overwritten,
waived or replaced with a blind unchanged CI rerun.

## Checks on the new composition

- Complete `tests/run-rust.sh`: exit0; runtime693 PASS/7 ignored.
  Across all76 summaries,1056 successful test invocations/12 ignored;
  these are invocations, not unique guarantees. Workspace, isolated owned-group
  helper, strict clippy and parity PASS.
- Source329 reported/two existing skips:327 executed, exit0.
  Frontend/navigation/QML contracts PASS.
- All shipped QML installed-import `qmllint`, manifest JSON, plugin validation,
  tracked Bash syntax and diff checks PASS; no tracked symlinks.
- Exact-code Test37105606866 PASS; x86_64/ARM64 packages37105606853 PASS.
  Later report-only checks are separate and visible on
  [Draft #577](https://github.com/k-kostin/omavless/pull/577).

No VM call, installed service, profile, DNS, TUN, route or firewall mutation was
made by this composition. The primary system remains untouched. It closes the
omitted source-gate composition work, not broad P4 user acceptance or the
unproven old auxiliary failure diagnosis.
