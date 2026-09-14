# Python reference retirement

Owner-approved sequence, 2026-09-14. This is repository/distribution cleanup
after the scoped R6 closure, not a new runtime migration or stable release.

1. **Archive — complete.** Remote `archive/python-legacy` is frozen at
   `aa5873783c019edc303a732e55ea8c85f1f0b090`. It contains the complete repository
   at that point, including Python reference, tests, docs and Rust implementation.
   It is not a pure Python-only tree. Preserve the branch during cleanup; do not
   add fixes, merge new main into it, or resolve a test dependency by fetching its
   moving branch name. The immutable commit is the reference provenance.
2. **Native RC distribution — merged in #239.**
   Source `4549f6921e908a951698617027b4971057278920` has fresh archives and
   an attended ARM64 installed update with actual restarted-binary evidence in
   [the RC report](../testing/NATIVE_080_RC_PREPARATION_2026-09-13.md).
   Existing native R6 acceptance is retained where unchanged. This is not a
   stable release or broader host claim.
3. **Default source/frontend installation — accepted locally, #242.** The
   ordinary installer and launcher are native-only, with explicit package/setup
   refusal and no Python fallback. The accepted QML layout is unchanged.
   Omarchy's clone-based installation also reaches the native-only launcher;
   it does not install the package or run install.sh. Legacy subprocess tests
   invoke backend.py directly as a temporary oracle, not the production launcher.
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

## Native default installation evidence

Candidate `72ddb467726ef816639a2f52f58f2c06888138b9`, Try Omarchy ARM64:

- Full local suite: 473 Python tests, 469 PASS, four existing skips; JS/QML
  contracts PASS. Earlier stale version/fallback assertions were corrected to
  distinguish the frozen reference from the native product. Parallel runs had
  VM subprocess timeouts; the final serial run passed without relaxed bounds.
- Plain source `./install.sh` updated the enabled installed frontend. All 24
  runtime-relevant frontend/template/manifest files match the candidate; no
  backend.py or legacy uninstall is installed.
- Installed launcher status reaches the real Rust owner. Running that same
  launcher under an isolated PATH with no native package refuses with exit 71,
  no stdout, and setup-guide remediation; it never invokes Python or reads
  private state. Synthetic tests cover missing/legacy/invalid ownership, owner
  revocation during staging, old-tree preservation and all native dispatches.
- Root manifest is RC, QML bytes unchanged, plugin enabled. One shell IPC read
  timed out during the update; a subsequent read succeeded with the same shell
  process. No forced shell/VPN restart was needed.
- The already accepted RC runtime remains disconnected Routing, zero core/TUN,
  and no manual recovery. No new package transaction or authorization prompt.

Python removal is still step 4, not implied by this installed native-only gate.
