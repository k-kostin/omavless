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
3. **Default source/frontend installation — merged in #242** at
   `dab6914e2bbff627580f5f3c2858cf474c4a0d80`. The
   ordinary installer and launcher are native-only, with explicit package/setup
   refusal and no Python fallback. The accepted QML layout is unchanged.
   Omarchy's clone-based installation also reaches the native-only launcher;
   it does not install the package or run install.sh.
4. **Reference/test detachment and removal — implemented; integration gate below.**
   The old backend, Python control-protocol implementation, legacy uninstall,
   old protocol CLI probe and live Python route oracle are removed. The 43
   differential adapters now replay 780 independently captured synthetic JSON
   replies; six support-policy counts have a separate input-hashed fixture.
   Ordinary tests neither fetch nor execute the archive. Changed inputs fail
   closed, not silently reuse an answer. Future native features require their
   own reviewed contract/tests, not extensions to the frozen Python backend.

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

This installed-default gate precedes, and does not by itself prove, step 4.

## Reference retirement evidence

The archive was checked out cleanly at the exact frozen commit. Before deleting
anything, the existing deterministic Rust suite and 27 Python adapter checks
ran against that archive to capture its replies, never Rust's candidate output.
An inline Python support-policy counter was captured separately from the same
archive for the three checked-in templates and three synthetic YAML shapes.
Their exact input SHA-256 values remain asserted by the Rust test.

- Recorded corpus: 43 adapter families, 780 replies, 774,984 bytes total,
  largest reply 41,150 bytes; plus six support-policy cases. These are synthetic
  expectations, not real local profiles or claims of live interoperability.
- Replay with backend.py and the Python protocol module absent: Rust workspace
  **957 PASS, 0 FAIL, 10 ignored**. All existing native differential assertions
  still execute actual Rust behavior; unknown input hashes fail with a fixed
  safe error. The optional recorder has a closed oracle allowlist, pinned clean
  archive/blob checks and atomic no-replacement publication.
- Developer/installer suite: **228 tests, 226 PASS, 2 existing skips**; JS/QML
  contracts PASS. This includes 16 new reader/recorder safety tests. The 261
  tests specific to the deleted Python implementations stay in the archive;
  they are not counted as current native coverage or silently called migrated.
- Installed-core opt-in: the Rust route probe exercised actual Mihomo
  **v1.19.30, linux arm64** against loopback-only synthetic REJECT rules with
  DNS/TUN disabled and a private Unix controller: **1 PASS**. Its historical
  Python side-comparison is removed, not replaced by a frozen live PASS.
- Production Rust/QML behavior, current RC version and package ownership are
  unchanged. Only Rust test code changed; this is not a new VPN transition or
  a fresh login/AUTO-1 acceptance. The prior native installed/runtime evidence
  applies to unchanged code.

See [fixture maintenance](../../tests/frozen_reference/README.md) for provenance
and explicit regeneration. Python remaining on main is developer test/build
tooling (including fixture replay), not an installed runtime requirement.
The completion/integration SHA and CI result are recorded with the retirement PR.
Even after a green merge, stable publication remains a separate owner decision.
