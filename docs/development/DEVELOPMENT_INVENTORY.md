# Development base and retained work

Owner-selected October16, after fix.8 delivery. `development` is the full-source
integration home, created at released source
`5edf4eb203926d8bd39a3ff80f611a19435c3787`. It does not start from the older
beta/0.9.9 assembly. Main remains the generated user distribution; immutable
release source, tags and assets are unchanged. #796 carries developer-rule and
fixture corrections separately; its first CI failure is not PASS.

This is a navigation/admission map, not acceptance of retained experiments or
permission to merge them all. Public PR metadata and changed-path comparisons
were inspected; this inventory does not claim exhaustive semantic/security
review of every branch. Full per-ref logs/statistics and local worktree custody
inventories stay outside Git. Use the owning contracts before implementation.

## Retained track entries

The exact tips below are observed checkpoints, not moving declarations of
readiness. Common ancestors are measured Git relationships, not a claim about
the author's original branch-creation intent. Ancestor tags are historical
orientation, not the selected package pins for a new integration.

| Track / owning PR | Observed checkpoint | Relationship to released full source | Disposition |
| --- | --- | --- | --- |
| Old beta/0.9.9, [#737](https://github.com/k-kostin/omavless/pull/737) | `2a49ec0322502465c04aa7d842437d050713b559` | Common ancestor `b4ef783e`, nearest ancestor tag fix.1; 9 branch-only commits, 156 released-source commits absent. | Retain as historical development assembly; do not use as the new base or merge into user main. |
| Restore integration, [#739](https://github.com/k-kostin/omavless/pull/739) | `c783583c498c1c27afa3f9d7973a2162eadfda16` | Same common ancestor; 65 branch-only commits, 16 paths changed on both sides. | Preserve recovery contracts/failures and stacked drivers including #745/#747/#752/#759/#765/#769; qualify selected Restore scope independently. |
| TUI import, [#742](https://github.com/k-kostin/omavless/pull/742), [#749](https://github.com/k-kostin/omavless/pull/749), [#755](https://github.com/k-kostin/omavless/pull/755) | Latest recorded #755 `3e32e2c9b99bdf69ee31eb02c63fb56e8e56ea2e` | Common ancestor `b4ef783e`; 13 branch-only commits, 13 overlapping paths. | Port the reviewed import dependency slice, preserving final onboarding/clipboard/Help corrections. Old import evidence is not new-base acceptance. |
| P4, [#770](https://github.com/k-kostin/omavless/pull/770) | `d325b0a08d3f6e4e5f51c5f8f233b180ddeb00b4` | Common ancestor `d620c300`, historical v0.8.2; 24 branch-only commits, 6 overlapping paths. | Preserve one-peer WG/AWG/parser/private-fixture/core/runtime gates. New owner fixtures do not themselves earn interoperability. |
| K1, [#703](https://github.com/k-kostin/omavless/pull/703) | `65d53d7ab0b6ae97ff666208e1b5f0c291b341fd` | Common ancestor `1bbd358e`, historical 0.9.5-RC lineage; 162 branch-only commits, 22 overlapping paths. | Retain separate privilege/ownership/fault requirements; no activation by source consolidation. |
| T4 background/resume, [#700](https://github.com/k-kostin/omavless/pull/700) | `87cc3009c24f300fe85620e28000c730a489924a` | Common ancestor old rc/0.9.7 `c4e80042`; 44 branch-only commits, 16 overlapping paths. | Retain explicit host hints/recovery permission and original failure scopes. Do not borrow new managed-DNS authority. |
| S1, [#623](https://github.com/k-kostin/omavless/pull/623) | `f29c82a3ee7e5f0cc6702baf4a077075b75938ca` | Common ancestor `a49ec925`, historical v0.8.2 lineage; 44 branch-only commits, 6 overlapping paths. | Retain as experimental app-proxy ownership/CAS work, not product readiness. |
| G1, [#768](https://github.com/k-kostin/omavless/pull/768), [#766](https://github.com/k-kostin/omavless/pull/766) | `c4bdf62051f8607dce105dfd22679a9b3f3a08ab` / `142f7f1f23754773846897338de9b8784e69ed10` | Common ancestor `d620c300`, historical v0.8.2; 34/35 branch-only commits. | Research/rendering evidence, not an accepted GUI or reason to mix prototypes into the stable source base. |
| V0, [#30](https://github.com/k-kostin/omavless/pull/30) | Historical exact-head protocol evidence; native successor #290 retained separately. | Legacy and Rust harness results are different identities. | Keep unavailable-family gaps explicit; do not revive Python production or restart accepted R6. |

An overlapping path is a review target, not a demonstrated merge conflict or
product failure. A large commit count measures history, not missing features
or the amount of code to cherry-pick. Fully reachable inactive foundations are
not automatically enabled or accepted either.

## Admission of old-base work

1. Read the current developer entry and owning contract. Declare the selected
   outcome, exact old tip, required dependency slice and preserved original
   gates. Do not use a branch name, Draft status or old beta version as proof.
2. Compare with released full source and current development, not the stripped
   main tree. Identify changed shared paths and changed authority/payload inputs.
   Review semantic interactions, not merely whether Git can merge cleanly.
3. Preserve old source/evidence and one-writer ownership. Prepare a bounded
   `dev/<topic>` port from development; do not rewrite every old stack or import
   a historical integration bundle to obtain one feature.
4. Retain release corrections: managed app/DNS pairing and admission, full
   remover compatibility, persisted onboarding/first-read behavior, clipboard,
   Help, current guides, and Backup-only versus Restore capability selection.
   No broad reset/privilege weakening or silent feature activation.
5. Record each adaptation and the exact new head. Run materially affected tests
   and risk-based installed/security gates. Reuse unchanged evidence only with
   an explicit input/invariant argument; never rename an old result PASS for
   the new composition. Do not repeat unchanged R6 or whole-VM matrices.
6. Integrate selected checked checkpoints into development. A new beta/RC is
   a declared composition from that base, not automatic continuation of the old
   assembly. Package/export/public/Marketplace gates remain separate.

## Cleanup sequence and limits

Seventeen old full-source PRs targeted user main at the initial audit. Reconcile
their source target or supersession individually; never blanket-merge them into
main. Old beta/RC refs still have dependent open PRs. Existing dependency bases,
the frozen Python archive and unique histories remain retained until explicit
exact-ref disposition; creating development does not resolve those PRs.

For the initial 505-branch snapshot, 176 branches were reachable from main with
no open head/base PR; 106 had no local worktree and 70 did. Thirteen other heads
needed unique-history disposition. Those are dated review candidates, not a
deletion manifest or proof of inactive processes. Refetch and reclassify before
an authorized cleanup. Follow the canonical
[cleanup lifecycle](../roadmap/DEVELOPMENT_WORKFLOW.md#branch-cleanup-lifecycle).

No old PR closure/retarget, branch/worktree deletion, main/release mutation or
frontier acceptance was performed by this inventory. #796 itself targets the
new development home. Its initial CI cleanup error and scoped fixture correction
remain separate from the already delivered fix.8 product.

## Source CI before the next export

Source-CI admission selects full push Test evidence and pins its workflow and
validator identities. This source-lane successor moves the full-source push
selector to development while preserving all PR/tag gates, exact-head checkout,
permissions and job/step checks. The new workflow blob and matching exporter
trust pin are one independently reviewed delta; generated user bootstrap and
Distribution workflow are unchanged. The frozen fix.8 source retains its old
integration-branch selector and trusted tool identities.

A new development release/export is not admitted merely because this branch
exists, unit API fixtures pass or a PR passes. Earn the actual exact-head full
push and generated bootstrap result after integration. A changed/unreviewed
workflow, PR-only success, partial/skipped/cancelled CI or modified validator
must still refuse. Do not bypass the gate or alter immutable fix.8 source to
retrofit the new lane; this work grants no publication authority.
