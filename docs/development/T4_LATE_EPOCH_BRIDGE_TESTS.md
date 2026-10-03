# Late retained-epoch checks through the private Off bridge

Test-only continuation of #586 documentation head
`f61399fd72a3b95286b48c38ff1b84063e380a37`. The independently executed System
mechanism remains attributed to code `fbe5fb9` in its
[evidence report](../testing/T4_SYSTEM_PROVIDER_MECHANISM_2026-10-03.md), not to
this continuation. No new VM execution, normal registration or policy adoption
is implied.

Existing bridge tests already replace the original receipt with same bytes at
the final observation and check concrete-host destructor preservation. Epoch
source drift was tested at acquisition, standalone recheck and resynchronization,
but not at these two late boundaries of the shared normal-compiled bridge.

Four individually named tests now exercise both Commit and Abort histories:

- The source changes only during the second/final fresh observation. Owner Drop
  restores the original source value, so only the observation's trailing proof
  check can retain the observed refusal. This does not claim continuous proof
  of a transient that no check actually observed.
- The source changes during actual owner Drop, under the original migration
  lease. The post-destruction evidence recheck must refuse a success result.

The tests consume a real retained proof using the existing test-only synthetic
source. They pass through the unchanged shared initializer, coordinator and
bounded review body; no new producer API or Boolean admission path exists.
They assert final-observation reachability, late source queries, actual owner
destruction under the original lease, unchanged private bytes and object
identities, retained fences and ordinary-startup refusal. Effect methods panic.
Safe-state assertions precede the decisive new refusal assertion.

Disposable offline source copies omit only the relevant trailing check for
each pair. Each history must then fail its new refusal assertion, rather than
passing because of pre-admission rejection. The owning source retains both
checks unchanged. Exact mutation-test and full-gate receipts belong to the PR.

Both disposable bypasses produced zero passed/two failed (one failure for each
history), specifically at `late epoch drift must refuse bridge success` after
the safe-state assertions. Observation-bypass log SHA256:
`f825c7162d47522db585dcd36a41dd30217c2297a468b9b686a17f2ac2b8a936`;
post-Drop bypass log SHA256:
`14593458a06f216de762a17aca4826fb6eaddedd2b6d531cbe0e9704b0a5a358`.
The mutant executables are retained privately outside Cargo.

An attempted positive rerun incorrectly reused the post-Drop mutant executable
from the same target directory despite returning to the unchanged owning source.
Its three-pass/three-fail result is retained as a build-provenance NONPASS, not
fixed-source evidence. The positive exact-source build uses a fresh separate
target and an explicitly frozen executable; no mutant result is relabeled PASS.

These are synthetic-source seam regressions, not actual System epoch-restart
acceptance. Missing/new-manager receipt policy, normal historical dispatch,
fenced edits/effects, release distribution and restore UX remain separate owner
decisions. No receipt is fabricated for an actual System provider.
