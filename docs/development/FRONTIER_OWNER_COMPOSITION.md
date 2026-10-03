# Current-candidate frontier owner composition

Development-only compatibility audit based on `rc/0.9.7` at
`c4e800425243c1b02165f82153e4bf418fe465e6`. This is not an RC scope change,
normal activation, release, marketplace request or main-update proposal.

## Source selection

- T4 #561: code `9f96b1e071e1e13b5613bebd7183089171c006ff`,
  evidence-only head `fb0948b2bf45b004c31c238db19b37c194b750ae`.
  The inherited inactive backup/restore chain is part of this substantial
  composition; it is not replaced by a small stand-alone patch.
- K1 #560/#563: code `f9c919b302dc19dc9dab0fbad71d287e749562e3`,
  evidence-only head `a6137e96d5ac5369af75491a7ef702e992b1c42d`.
  The separate netguard crate and its actual namespace fixtures are retained.
- T3 #562: code `4ff4c66d37bd7e80f5303faa6a768a8fea4b7733`.
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

Combined focused regressions, full Rust/source/frontend checks, strict clippy
and exact-head CI are pending. Any combined VM fixture has its own frozen
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
