# Cloud implementation handoff — 2026-09-04

Main/base remains `2f9f6bcdbafdfa6b34da50160e7a739618ab814b`.
No merge, marketplace update, installed-host mutation or production cutover.
Python remains the installed production owner. Cloud writer stops after the
final exact-head CI/PR-body update; fetch live heads before continuing.

## PR #158: profile-search recovery

Branch `codex/profile-search-recovery`, head
`6abfcd66c385a1ffe0ef571d21e3554aeac51b08`, targets main.

Fixes U1 from #157: active queries stay editable when profile count drops below
eight; / reveals search for small/empty stores; Escape clears and returns focus
when clearing hides the field. New production-JavaScript tests exercise list
sizes, shrinking/empty stores, explicit keyboard discovery and focus requests.

Local 270 Python tests passed (3 expected skips), Node search behavior and
QML/i18n contracts passed. JSON/Python compile/shell syntax/diff checks passed.
Exact-head CI passed: push run 33917959893 and PR run 33917963724. An initial
publication lost tests/run.sh's executable bit; the final head restores it.

Omarchy still needs actual / and Escape, visible focus and refresh/delete/
close/reopen checks. A Node focus stub is not rendered QML evidence.

## PR #159: native batch/owner integration

Branch `codex/r5-batch-owner-integration`, targets #156's branch. It converges:

- #156 at `68a1dcc481a0d89e8e8a61b2ba0deecc1914b51c` (first parent);
- #154 at `ee1d97cd7c1d5780eb6f3031524921361ef014f2` (second parent);
- #156 already includes #155's bounded subscription transport.

The PR diff against #156 includes #154. Preserve both predecessors, then rebase/
retarget only after their integration. Do not squash away unique predecessor
work or claim this PR is an independent change against main.

New implementation lives in native_coordinator/batch.rs and the coordinator's
ordinary admission/external-fetch preflight. It provides:

- one bounded operation registry initialized once with a trusted instance ID;
- shared ordinary/batch operation IDs in both collision directions;
- replay before store access and stale expected-revision rejection;
- private worker handoff, safe counters, cancel notification and terminal state;
- owner/migration-lock reacquisition, ownership/revision and store-snapshot
  checks before one atomic commit; empty batches skip writes/clock/revision;
- shutdown revocation, plus a private supervisor ticket for spawn/panic cleanup
  without permanently stopping the owner; stale tickets cannot revoke successors.

Ten added owner tests cover commit/replay, early/late cancellation, disconnect,
ID collisions including fetch preflight, empty batches, changed private store,
shutdown, ownership withdrawal, concurrent fetch/cancel/failure, and lost-worker
cleanup. The preparation/pool and pure registry suites remain intact.

The first eight-test integration at `2f26dfa823e72dd24b01e9691c7f6c2a3aa0c366`
passed full workspace tests, formatting, strict Clippy and parity in CI run
33918025492. The final candidate adds concurrency and supervisor-ticket tests;
its exact head and final CI links are recorded in PR #159's body. Use those
results, not this earlier implementation SHA, for review.

Local Python/QML/i18n, JSON, compilation, shell syntax and whitespace checks
passed. Rust tools are unavailable in this cloud container; Rust validation and
canonical rustfmt output come from GitHub Actions. No host or private provider
fixtures were used. Synthetic test files live only in temporary test roots.

## Remaining implementation, not merely local tests

The batch APIs are not wired to RuntimeServer/ProductionOwner or advertised over
IPC. No threads are created in production by this change. The next bounded
scheduler slice must:

1. Bind the real runtime instance ID during accepted owner construction.
2. Retain a supervisor ticket before spawning. Abort on spawn failure/panic;
   never lose the only ticket while a registry record remains active.
3. Run preparation outside owner locks using the existing shared fetch pool.
   Busy must yield without spinning; preserve the job's original deadline.
4. Publish counters, return complete/failed work, and serialize final completion
   with cancellation/disconnect. Invoke revocation on shutdown/owner withdrawal.
5. Add fixed start/get/cancel dispatch and semantic CLI mappings only with
   deterministic concurrent and installed-host acceptance.

The APIs do not make an arbitrary offline owner production-authorized. Production
must use the existing ownership-gated constructor; tests also exercise the
socket-independent owner with synthetic host/store state.

Actual frontend bridge/cutover, host lifecycle/fault gates and R6 Python-absence
remain separate. Production Ratatui remains after R6. QML visual consolidation
from #157 remains a plugin lane; do not translate presentation into Rust.

## Omarchy continuation order

Fetch all PRs and read AGENTS/roadmaps first. #135 retains its own mode-change/
authorization acceptance; #30 retains missing protocol-fixture gates. Neither
branch was modified. #157 records the broader UI capture matrix.

Review/test #158 on the exact installed head. Review #154/#155/#156 dependencies
and #159's integration diff before any owner-approved merges. Run available
normal Rust/Mihomo opt-ins, but do not fabricate a production ownership marker
or call synthetic tests an executed cutover. Review the scheduler remainder
above before advertising refresh-all as an available Rust runtime feature.
