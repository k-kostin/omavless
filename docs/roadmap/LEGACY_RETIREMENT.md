# Python reference retirement

Owner-approved sequence, 2026-09-14. This is repository/distribution cleanup
after the scoped R6 closure, not a new runtime migration or stable release.

1. **Archive — complete.** Remote `archive/python-legacy` is frozen at
   `aa5873783c019edc303a732e55ea8c85f1f0b090`. It contains the complete repository
   at that point, including Python reference, tests, docs and Rust implementation.
   It is not a pure Python-only tree. Preserve the branch during cleanup; do not
   add fixes, merge new main into it, or resolve a test dependency by fetching its
   moving branch name. The immutable commit is the reference provenance.
2. **Native RC distribution — accepted locally, #239 integration pending.**
   Source `4549f6921e908a951698617027b4971057278920` has fresh archives and
   an attended ARM64 installed update with actual restarted-binary evidence in
   [the RC report](../testing/NATIVE_080_RC_PREPARATION_2026-09-13.md).
   Existing native R6 acceptance is retained where unchanged. This is not a
   stable release or broader host claim.
3. **Default source/frontend installation — pending.** Make the ordinary path
   native-only, with actionable missing-package/ownership refusal and no Python
   fallback. Keep the accepted UI. Account for both the source installer and
   Omarchy's clone-based plugin installation; the latter does not run install.sh.
4. **Reference/test detachment and removal — pending.** Preserve synthetic
   language-neutral behavior fixtures with their archived reference provenance;
   detach differential checks before removing obsolete runtime sources. Do not
   generate expected answers from the candidate being tested, quietly skip
   parity tests, or discard security/negative cases to obtain a green suite.

Useful independent Python developer tests/tools may remain. GitHub language
percentages are not an acceptance criterion; neither Linguist overrides nor
moving the old backend into another directory constitutes retirement.

Until all four steps have evidence, remain **0.8.0-rc.1**. A green preparation PR
does not authorize stable tags/releases or marketplace publication. The published
0.7.0 marketplace SHA is unchanged. AUTO-1, DNS/provider investigation and V0
fixture gaps retain their own explicitly incomplete status.
