# Current-candidate frontier owner composition

The later batch/root-object coexistence cut has its own
[exact-source acceptance report](../testing/FRONTIER_BATCH_OBJECT_COMPOSITION_2026-10-03.md).
Its evidence does not retroactively change the executed heads recorded below.

Development-only compatibility audit based on `rc/0.9.7` at
`c4e800425243c1b02165f82153e4bf418fe465e6`. This is not an RC scope change,
normal activation, release, marketplace request or main-update proposal.

## Source selection

- T4 #561: code `9f96b1e071e1e13b5613bebd7183089171c006ff`,
  evidence-only head `fa8a613b3891d582f6534befa50b0695f684e692`.
  The inherited inactive backup/restore chain is part of this substantial
  composition; it is not replaced by a small stand-alone patch.
- K1 #560/#563: code `f9c919b302dc19dc9dab0fbad71d287e749562e3`,
  evidence-only head `a6137e96d5ac5369af75491a7ef702e992b1c42d`.
  The separate netguard crate and its actual namespace fixtures are retained.
- T3 #562: code `9a5e132d18bf33d5dae69ac12c18969710e9b309`.
  This includes the later test-fixture ordering correction: construct/drop a
  stopped predecessor adapter before spawning the successor core/socket, then
  retain that adapter. No production cleanup, time budget or grant changes.
  The intermediate `129250e86676b28c763a271e268473a7993e73bf` is not the
  accepted integrated input.

All three writers retain their independently reviewable PRs. This branch has
one separate owner and must not rewrite their branches. Standalone test counts,
CI and VM results remain associated with their original exact heads; they do
not automatically establish acceptance of this combined tree.

## Integration question

T3 invalidates its retained capture/confirmation/detached effect before a new
ordinary admission and before long-job publication. T4's test-only historical
Favorite and Connect/Disconnect alternatives use the shared scheduler directly,
without ordinary admission. The composition must prove that these alternatives
cannot bypass T3 cancellation, and that exact replay does not invalidate a newer
unrelated capture. Ordinary admission still needs its early cancellation before
trying the migration lease; moving cancellation only to the scheduler is not
sufficient.

The integration review must additionally retain normal C1/pending refusal,
single coordinator operation-ID/revision semantics, late-result fencing and
detached worker cleanup. No new Boolean grant, unconditional controller DELETE,
privileged IPC or ownership reconstruction from netguard rule shape is allowed.

## Gates and limits

The first combined full Rust/source/frontend and strict-clippy run passes on
`eba24b957ebb7eaab452156a51201583a87c7470`: runtime 1,084 PASS, 31 ignored,
plus its separately isolated helper test; source 503 PASS and two opt-in skips.
These results are not acceptance of the subsequent late-fence correction.
The later frozen acceptance below supersedes that intermediate result. Any
combined VM fixture has its own frozen
artifact, exclusive VM lease and before/after preservation checks; prior
standalone VM results are not carried forward silently.

Normal conditional-core package attestation/adoption, historical restore-policy
adoption and canonical netguard manager/namespace authority remain their
separate product gates. None is enabled by importing these research modules.
Managed DNS and normal runtime package validators, release pins, QML/TUI actions
and the installed host service remain unchanged.

## Initial integration correction

Three actual-owner regressions independently fail on the composed source
before its new correction: shared scheduling retains an old capture, it fails
to cancel a stalled detached effect before publication, and the newly typed
startup caller retains capture authority through pending-transaction refusal.
Each uses an actually captured private-controller session rather than a Boolean
permission fixture. They do not simulate a successful historical restore proof.

The correction keeps ordinary admission's early pre-lease invalidation, repeats
new-operation-aware invalidation at the shared scheduler and invalidates before
typed startup reconciliation. Exact known-operation replay preserves a newer
confirmation; the tests also assert no conditional POST, unchanged connected
intent and timely detached refusal. The three focused tests now PASS locally.
That focused result is not the remaining combined full-suite or VM gate.

## Late private-transaction fence correction

An independently serialized restore transaction can publish an existence fence
after the close worker captured its immutable desired/store/config/ownership
context. Checking only the routing-preset marker then permits a conditional POST
through a C1/restore fence. A real captured and started private-controller
worker reproduces this defect before correction: the first closure-marker case
returns `Closed`, not `RefusedBeforeWrite`.

Both initial capture and each detached effect lease now consult the canonical
`pending_private_transaction` predicate. No historical exception applies to
connection close. Thirty real-worker cases cover all ten fixed members as a
malformed file, directory and dangling symlink without changing context bytes.
Each refuses before POST, keeps revision zero and preserves the fence/private
state. Three further cases publish a disposition fence after an actual POST;
they retain `Unknown`, advance the revision once, and replay the exact receipt
without a second effect. Both focused aggregates PASS. This is not normal
conditional-package adoption or permission to retry an ambiguous operation.

## Frozen combined acceptance

Executed source: `a4dabb202768d73e0dbc36d33ca62b19aa1a5d69`. Later changes to
this report are documentation-only; do not silently move its executed SHA.

- Complete `tests/run-rust.sh`: PASS, including runtime 1,086 PASS/31 ignored
  plus the separately isolated helper, workspace tests, strict clippy, TUI
  feature checks, terminal tests and parity. Inactive opt-in returns in ordinary
  tests are not real-core acceptance; the selected VM opt-ins run separately.
- Source suite: 503 PASS/two existing opt-in skips. Navigation 93, frontend
  contracts, all shipped QML with installed Omarchy imports, manifest/plugin
  validation and tracked shell syntax: PASS. No tracked symlinks.
- Independent read-only Astra review of the scheduler/startup and canonical
  late-fence corrections found no additional actionable defect. It ran no
  tests or VM effects and does not replace the execution results.
- Exclusive x86_64 Omarchy Dev VM: 20 actual-owner tests, 23 transport tests,
  nine historical connection tests, one ordinary separate-path refusal and one
  actual no-TUN core rollback/cleanup test PASS. Both actual composed-core
  opt-ins execute, rather than returning because the environment is absent.
- The combined netguard artifact repeats actual creator/lifecycle (13 scenarios)
  and real receive truncation (three scenarios), each aggregate PASS.
- Twenty repetitions each of late fences, shared scheduling, admitted startup,
  proof-lease drain, publication ordering and actual composed-core owner close
  PASS: 160 further test results. Together the invocation has 216 PASS test
  results; scenario aggregates are not miscounted as independent unit tests.

Frozen SHA-256:

| Artifact | SHA-256 |
| --- | --- |
| Runtime test executable | `e5fca7a32f187b693ed3994a247d898407e2e0c84bbaa6c63c60786ef9dcd6e8` |
| Netguard test executable | `d7b09b9f01bcb44c765652b2f3981adbe37f83a6e9ac65740f5e3652bfe83274` |
| Developer composed core | `3b1da75d3c9fd8440216f9c256c6c59da812faae88debc936f3c72fef9724544` |
| Executed-source archive | `d1f725b3fbb4c06573aa84151ba252026b53c3351e81aaa554deb245ea7b7fd0` |

Actual normal-core cleanup uses installed Mihomo hash
`ba7a74ed6bbc3098930e6e172fa9b7d4ef7e42a77f7a516046d61fbeb3bfcfe6`.
Artifacts have no file capabilities. These are retained research executables,
not immutable native release packages or a companion-package attestation.

Before/after private-state fingerprints, active/running user service PID,
executable, core/TUN inventory, resolver files/status and parent namespace all
match. Address/routes/rules (IPv4 and IPv6) match except numeric decreases of
the two exact `addr_info` lifetime fields under the original K1 comparator.
There is no one-address restriction or allowance for increasing/other fields.
Raw private snapshots/logs remain outside Git. Generated guest scratch was
removed only after retaining binaries and receipts on the host; no fixture
process remains. The VM lease was explicitly released for the next K1 writer.

At report preparation, all four executed-head architecture/package CI checks
PASS and Test is still pending. Final live status belongs to Draft
[#564](https://github.com/k-kostin/omavless/pull/564); neither an older green
head nor these local/VM results should be substituted for pending CI.
No installed service, runtime package, main/RC or normal product permission
was changed. The separate product gates above still apply.
