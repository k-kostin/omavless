# Cloud R5 refresh-all handoff — 2026-09-04

## Source of truth and branch ownership

Starting `main`: `2f9f6bcdbafdfa6b34da50160e7a739618ab814b` (PR #153).
This session did not change `main`, marketplace submission, PR #30, PR #135,
installed services, private stores or tunnel ownership. Python remains the
installed production owner. No Omarchy host or private fixture was used.

The cloud writer stops after the final exact-head CI and PR-body update.
Before taking a branch, fetch again and use the head in its live PR, since a
report-only descendant may follow the implementation SHA below. Do not use an
old chat or reset a concurrently advanced branch.

## Reviewable work

- [PR #154](https://github.com/k-kostin/omavless/pull/154),
  `codex/r5-long-operation-protocol`, targets `main`.
  Preserved implementation: `ee1d97cd7c1d5780eb6f3031524921361ef014f2`.
  Existing pure start/get/cancel protocol and bounded per-instance registry are
  now visible as a Draft PR. No operation is registered or scheduled.
- [PR #155](https://github.com/k-kostin/omavless/pull/155),
  `codex/r5-subscription-fetch-deadline`, targets `main`.
  Accepted cloud candidate: `543f1b75bb70ef8f8cab5d685bd6cbd517bec3ff`.
  Fixes manual redirects resetting the full provider timeout. URL validation,
  every request and body consumption share one monotonic budget. Trusted
  enclosing work can shorten that budget, never extend it.
- [PR #156](https://github.com/k-kostin/omavless/pull/156),
  `codex/r5-refresh-all-bounded-worker`, targets the #155 branch.
  Implementation checkpoint: `d09fabf6759b10737599a53d897e41595c9c6a27`.
  Adds incremental private batch preparation and a shared four-fetch pool.
  Each step fetches at most one provider, observes cancellation and a fixed
  30-minute deadline, and never writes the store. Failure discards the complete
  prepared prefix while preserving completed-progress counts. Only a complete
  batch can be handed to the final owner transaction.

#154 is independent of #155. #156 genuinely depends on #155's total fetch
budget and does not duplicate #154. Future live refresh-all needs both tracks.
Preserve both `lib.rs` module additions and all roadmap paragraphs when the
branches converge. Keep the PRs Draft until their declared integration/review
gates and owner approval are satisfied.

## Evidence

- Local Python suite at the unchanged Python production tree: 270 run,
  267 passed, 3 expected skips. QML and i18n contracts passed.
- Local JSON validation, Python compilation, shell syntax and whitespace
  checks passed. No tracked symlinks were found.
- New worker/pool review found no process execution, privilege invocation,
  environment access, TCP controller, print/log surface or serialization of
  private batch state. Test URIs use synthetic documentation addresses only.
- #154 exact-head CI:
  [33913389102](https://github.com/k-kostin/omavless/actions/runs/33913389102).
- #155 exact-head CI:
  [33913851740](https://github.com/k-kostin/omavless/actions/runs/33913851740)
  and [33913855276](https://github.com/k-kostin/omavless/actions/runs/33913855276).
- #156's first formatted implementation passed workspace tests, formatting,
  strict Clippy and parity at `6a18848a38b01ecda23d4b1a6c9994eaa251f9dd`
  ([33914415022](https://github.com/k-kostin/omavless/actions/runs/33914415022)).
  The final candidate adds stale-store handoff coverage and makes the
  concurrency test release its barrier before asserting. Final exact-head
  results are recorded in #156's PR body and must be checked there.
- Rust was not available in this cloud container. Rust evidence above comes
  from GitHub Actions. Opt-in installed-Mihomo test names in a Cargo run do not
  prove that their environment variables/fixtures were supplied.
- No Try Omarchy, bare-metal, real-provider, TUN, NixOS or Python-absence
  acceptance is claimed by this session.

## Next native integration slice

Do not simply advertise `subscriptions.refresh_all` after merging these PRs.
The preparation worker and the protocol registry are deliberately not yet the
canonical owner's background scheduler. The next implementation must:

1. Admit start/replay under the single native owner lock, revalidate the exact
   committed Rust generation under `MigrationLock`, snapshot the whole private
   subscription set, and atomically share IDs with the ordinary mutation ledger.
   Check both collision directions, including unary preflight before remote I/O.
2. Bind the registry to the actual runtime `instanceId`. Keep one active batch,
   bounded terminal retention, and acknowledge start within the existing unary
   deadline. A spawn/admission failure must not leave a permanently queued job.
3. Run preparation outside owner/store locks with the runtime's existing pool.
   A busy step must yield through a bounded scheduler, not spin or block status
   and disconnect. The original job deadline includes time waiting for permits.
4. Publish progress through the registry. Accepted cancellation must prevent
   the first fetch or win over an in-flight fetch's later failure. The worker
   flag is only notification, not authority to commit or cancel after commit.
5. Reacquire the owner and migration locks for one final transaction. Check
   exact ownership/generation, global revision and every snapshot member; close
   cancellation atomically in the registry before calling the existing batch
   commit. A concurrent disconnect wins by revision. Empty batches perform no
   store write, timestamp read or revision increment.
6. Supervise shutdown and worker lifetime. No completion may mutate after
   ownership withdrawal, restart, manual-recovery entry or runtime shutdown.
   Preserve bounded failure/retry semantics if a response is lost after commit.
7. Only then register the three fixed methods and their semantic CLI commands.
   Do not add raw JSON/method execution or client-controlled timeouts/threads.

Use deterministic concurrent tests for these boundaries before an installed
integration pass. They are remaining implementation work, not merely unchecked
live tests of an already complete refresh-all service.

## Tomorrow's Omarchy work

Fetch current GitHub first. Review #154 and #155 independently; preserve #156's
stack relationship until #155 is integrated, then rebase/retarget and rerun the
materially affected checks. The source changes are separate from #135's UX fix.

For the current candidate, run normal Rust/Python/QML gates and available
installed-Mihomo opt-ins without changing the ownership marker. Check native
read-only startup/status, service packaging and private-socket behavior as
applicable. Do not manufacture a committed Rust marker to reach otherwise
inactive APIs. No user cutover command or QML bridge was added here.

PR #135 still needs its exact-head Try Omarchy authorization success/cancel/
failure/rollback/panel-reopen/shell-restart matrix; its earlier failed head is
not acceptance of the current fix. PR #30 remains fixture-constrained at
`a643db595ad5369b5fea200ebc601b4f0f70f18f`; do not ceremonially rebase it.

The production frontend bridge, controlled ownership cutover and host fault
matrix remain R5 work. R6 must prove normal operation with Python unavailable;
Ratatui product implementation remains after that gate. WG/AWG is still not
product-enabled by these changes.
