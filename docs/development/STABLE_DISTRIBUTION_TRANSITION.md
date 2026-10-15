# Stable distribution and development separation

Owner-selected direction, 2026-10-15. **Preparation only:** main remains at its
published snapshot; no files/branches have been moved and no release or merge
is authorized by this plan. Complete #790's remaining functional gates first.
The [execution policy](EXECUTION_POLICY.md#retrospective-closure-and-distribution)
governs retained decisions and their actual distribution.

## Audiences and branches

main serves a person operating the stable product **and that person's agent**.
Agent-readable setup/support instructions and reviewed product helpers belong
there. This is different from instructions for developing OmaVLESS.

| Location | Maintained purpose |
| --- | --- |
| main | Stable user distribution and matching user/user-agent documentation; changes only through an authorized selected release. |
| development (proposed; not created) | Full source, tests, build/release tooling, important architecture/roadmaps, developer instructions/skills and current portable handoff. Integrate selected work, not every unfinished experiment. |
| dev/* | Scoped feature, test or documentation work; one active writer and a meaningful reusable checkpoint. |
| beta/*, rc/* | Temporary selected test assemblies and frozen release candidates, not permanent homes for developer documentation. |
| Local private storage | Raw captures/logs, build outputs, VM images and disposable scratch, with purpose/retention and no secret material in Git. |

Do not rewrite history, remove unique commits, delete evidence branches or move
an active fixture to make the working tree look smaller. A clean main tree does
not remove old Git objects or solve local build-cache storage by itself.

## Proposed main file boundary

This is a reviewed inventory target, **not an implemented export allowlist**.
The exporter must later resolve the complete dependency closure at the selected
source SHA; a directory name alone is not proof that its contents are needed.

| Current paths | Intended destination / condition |
| --- | --- |
| manifest.json, preview.png, backend.sh, plugin/* | main: selected released frontend, catalogs, fixed native launcher, setup scripts and matched component metadata. No experimental feature activation. |
| tools/uninstall.sh from #790 | main: reviewed full-removal helper; keep pinned source/hash and supported-version admission. Not the old restricted tools/clean-reinstall-098.sh. |
| install.sh | main only if retained as a supported alternate install route and brought into agreement with the canonical distribution file set; no whole-repository copy. |
| templates/* | Preserve in development. Keep in main until actual consumer dependency closure and alternate-install compatibility prove whether the released frontend needs them. Do not infer that compile-time Rust use is a frontend runtime dependency. |
| README.md, CHANGELOG.md, LICENSE, THIRD_PARTY_NOTICES.md | main: concise product entry, useful release changes and required legal/provenance material. No development journal. |
| docs/user/* and selected docs/marketing/images/* | main: consolidate actual stable instructions, retain working legacy destinations/anchors where users or shipped controls refer to them, and retain only selected public assets. |
| AGENTS.md plus stable user-agent assistance guide | main: short role router and supported installation/maintenance procedures for the user's agent. Never route a repair into development code automatically. |
| crates/*, Cargo.*, rust-toolchain.toml, packaging/*, tests/*, parity/research tools | development and immutable source provenance: build/test the real native source there, not from an incomplete distribution checkout. |
| docs/development/*, docs/roadmap/*, docs/testing/*, developer skills, DEVELOPMENT_ROADMAP.md, PROTOCOL_ROADMAP.md, CONTRIBUTING.md | development: preserve useful canonical decisions/evidence; replace main's contributor entry with a short stable pointer if needed. Reconcile every existing link before moving. |
| GLOSSARY.md | Maintain in development as editorial authority; publish only user-useful terminology in the stable guide, not a duplicate mutable developer policy. |
| .github/* | Separate source/build checks from distribution validation before stripping main's Rust workspace. Do not simply retain a broken workflow or skip the release's required source tests. |

Native packages remain architecture-matched immutable artifacts with reachable
source and dependency-license provenance. The selected source SHA, feature
selection, frontend tree and app/DNS hashes must be reconciled; a matching
displayed 0.9.8-1 label is insufficient. Keep this metadata out of the product
README prose. No main tree should silently rebuild Cargo for the user.
Preserve the exact full native source, dependency patches, build scripts and
licenses behind an immutable source reference/archive, not only a moving
development link. Validate references from notices and installed package docs.

The full source tree is the only normal source of product edits, including
frontend, manuals and helpers. Export them into main; do not maintain a second
editable implementation there. A separately owner-authorized emergency main
patch must be reconciled into the source before the next export or it can be
silently lost. Contributor routing uses a separate checkout, never an installed
user plugin converted into a development worktree.

## Concrete dependencies already inspected

- manifest.json selects plugin/Panel.qml. Panel reads ../manifest.json; Service
  resolves ../backend.sh; SetupPage resolves setup-runtime.sh.
- setup-runtime.sh sources setup-predecessor.sh and consumes runtime-release,
  dns-release and component-updates JSON in its own directory. Preserve the
  selected tuple and predecessor/update support when exporting plugin/.
- Both SetupPage and Panel link to docs/user/NATIVE_INSTALL.md on main. Keep
  that destination current, or preserve a clear current landing page; do not
  break it as part of a cosmetic guide rename.
- install.sh currently stages selected root files, plugin/, templates/ and
  docs/user/, **not tools/uninstall.sh**. This is an alternate-install payload
  gap to resolve/test before claiming one unified distribution. The #790 guide
  has an anonymous pinned-download fallback; this observation does not turn
  its earlier ordinary Omarchy-add acceptance into a failed result.
  Transition gate: deliver the same reviewed helper/help payload through both
  supported routes, or stop advertising the alternate route as equivalent.
- packaging/arch/stage-payload.sh installs packaging/arch/README.md into the
  native package's /usr/share/doc/omavless/README.md. That file is developer
  oriented. A frontend/docs change does not update already published package
  bytes. Decide explicitly whether final 0.9.8 needs rebuilt native artifacts
  for this documentation change; never relabel immutable existing archives.

## Stable user and user-agent documentation

Keep one comprehensive user guide and one assistance guide, with a short README
as the front door. File names are not yet frozen: preserving the existing
NATIVE_INSTALL.md destination may avoid unnecessary shipped-link changes.
Fold everyday use, setup/update/removal, FAQ and recovery into the user guide;
old pages become concise current pointers where compatibility requires them.

The assistance guide covers the same published version and operations: identify
installed components, explain effects, obtain required consent, run supported
helpers, observe actual completion and collect bounded privacy-safe diagnostics.
It does not teach source development, promise unimplemented noninteractive
commands or bypass package/network/ownership checks. A main AGENTS.md directs
support agents here and contributor agents to development's current entry.

The two guides must agree with actual UI and terminal terminology and offered
actions. Source [editorial rules](../roadmap/UI_UX_CONTRACT.md#end-user-editorial-rule)
remain in development; only their product-facing result is shipped in main.

## Preparation and promotion sequence

1. Freeze the current public main/tag identities. Inventory and preserve unique
   source, evidence and active owners before classifying old refs or local cache.
2. Complete #790: exact-head CI, bounded x64 uncertain-Quit recovery, successful-
   used public0.8.2 removal and ARM64 removal/reboot/resume/reinstall. Earlier
   failed-restored0.8.2 is not a successful-used PASS. Mac checks can run in
   parallel with the sole Linux VM operator. No repeated unchanged acceptance.
3. Independently prepare this file/link map and the final user/assistance copy.
   Keep developer research separate; do not move files under active acceptance.
4. Select a full source integration snapshot and create development only in the
   agreed migration. Preserve source/build/tests and carried #791/#758 process
   corrections there; do not seed it by merging every frontier branch.
5. Implement a bounded explicit distribution exporter and its tests from that
   selected source. Export committed allowlisted files, never HOME, caches or
   untracked fixture data. Keep output outside live installations. No exporter
   may write main, publish or run privileged commands automatically.
6. Validate source/build gates at the selected full-source SHA; separately
   validate the produced distribution tree, matching immutable artifacts,
   help destinations and alternate installer file-set equivalence. Main/tag CI
   must work on the resulting tree, not assume removed Cargo/tests are present.
   Distribution CI must verify the source SHA's required successful checks and
   bind its exported tree and artifact tuple to that exact source. A current
   green development branch or an unrelated historical run is insufficient.
   Subsequent edits to exported files need a new exact-head distribution check.
   Trace links inside user guides as well as QML links, including references to
   developer documents which will no longer exist in main.
7. Test the exact assembled user journey including import, Connect/Disconnect,
   update/partial setup and removal as affected. Record genuine unavailable
   cases explicitly, not inherited PASSes from a different executable tree.
8. Present one complete proposed main update with its source/artifact mapping,
   user-facing documentation and marketplace submission text. Obtain the
   owner's separate applicable approvals. Then reconcile source PRs and retire
   only safely superseded refs; no post-publication housekeeping commit that
   immediately changes the selected marketplace snapshot.

Do not copy all development into main by a normal blanket merge, independently
hand-edit two implementations, or make a distribution repo whose native source
cannot be found. Old immutable tags/assets and their original evidence remain.
Changes to CI/export trust require their own proportionate independent review.

## Portable handoff and local retention

One current handoff carries exact branch/SHA, product/artifact versions, done and
unrun checks, next commands/expected outcomes, required artifact download or
explicit safe transfer, and VM ownership. A Mac agent must not depend on an
unavailable PC ~/.cache path or reconstruct the task from hundreds of receipts.

Shared Git retains meaningful reproducible source/tests, important decisions
and short sanitized acceptance outcomes. Temporary commands/screenshots/logs
stay local; a reusable prototype earns a scoped source checkpoint rather than
one PR per intermediate attempt. Necessary source/evidence must not have its
only copy in disposable cache. Secrets and raw private profiles never enter Git.
