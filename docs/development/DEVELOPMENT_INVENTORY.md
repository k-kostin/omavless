# Development base and retained work

Owner-selected October16, after fix.8 delivery. `development` is the full-source
integration home, created at released source
`5edf4eb203926d8bd39a3ff80f611a19435c3787`. It does not start from the older
beta/0.9.9 assembly. Main remains the generated user distribution; immutable
release source, tags and assets are unchanged. #796 integrated developer rules
and a bounded fixture correction into development; its first CI failure remains
a failure, separate from the successful corrected-head run.

This is a navigation/admission map, not acceptance of retained experiments or
permission to merge them all. Public PR metadata and changed-path comparisons
were inspected; this inventory does not claim exhaustive semantic/security
review of every branch. Full per-ref logs/statistics and local worktree custody
inventories stay outside Git. Use the owning contracts before implementation.

## Current backlog (October 21, 2026)

Start with the [individual PR/issue disposition ledger](https://github.com/k-kostin/omavless/issues/553#issuecomment-6312681149).
The complete remaining-queue review is dated against development source
`7932893eff9a3184ce4cc65c8e61045be9947174`. It distinguishes carried source,
partial ports with a concrete remainder, distinct missing changes and retained
research/evidence. Closed issue #553 is its historical journal, not an active task
or a new implementation queue. The table is linked here rather than duplicated.

Refresh actual GitHub heads, bases and states before acting. Dated dispositions,
source presence and green historical checks are different evidence; preserve
original failures and deferred/unknown checks. In particular, the old import,
Restore and fixture selections below must not restart completed new-base ports.
Use each ledger row's owning successor and remaining scope instead.

The owner-selected planning order is [#808](https://github.com/k-kostin/omavless/issues/808)
then [#256](https://github.com/k-kostin/omavless/issues/256) and
[#721](https://github.com/k-kostin/omavless/pull/721), followed by the remaining
Remove UI scope in [#774](https://github.com/k-kostin/omavless/pull/774).
This is not implementation or merge authority. Rules [#815](https://github.com/k-kostin/omavless/pull/815)
and its separate cleanup-test diagnosis [#816](https://github.com/k-kostin/omavless/pull/816)
have their own ownership, exact inputs and gates; do not duplicate or silently
expand them. Current work is coordinated in owning PRs, not this dated index.

The selected current-development #720 port has its own
[successor #821](https://github.com/k-kostin/omavless/pull/821): specific TUI
Restore readiness reasons and the existing explicit Disconnect confirmation,
followed by a fresh verified Off state and a new Restore keypress. Its
nonpublic package tuple uses producer `72151a78730c83950205a5664aa7967066b81a4b`
for both architectures; exact archive identities live in the three frontend
metadata consumers. The owning PR records pending versus earned source,
installed and integration gates. This is not the full #739 recovery engine,
public 0.9.8 Restore support or acceptance of the old stacked #720 branch.

## New-base queue

Read the [current backlog](#current-backlog-october-21-2026) first. The older
selection map below retains its historical requirements and dependencies; it
is not today's list of missing implementations. In particular, closed carried
predecessors are not independent tasks to re-create.

This was the continuation order after the owner-approved repository organization,
not feature implementation authority, a new release promise or acceptance of
old prototypes. The [delivery ledger](../../DEVELOPMENT_ROADMAP.md) keeps the
owner-selected product requirements. `development` is the source home;
`beta/<version>` is a temporary selected assembly and `rc/<version>` a frozen
candidate, not permanent parallel development lanes. Future composition is
not a whole-beta merge. Public main and fix.8 remain unchanged by this queue.

| Lane | Next bounded outcome / owning entry | Retain and re-earn |
| --- | --- | --- |
| Source home and test infrastructure | Qualify development push-CI/export/bootstrap after integrated #796; select missing socket-fixture slices #758/#760/#762, concurrent-create coverage #572 and separate builder hardening #550. #763's scheduling patch is incorporated and extended. | Preserve current runtime/payload assertions and fixed deadlines. A clean patch application or PR-only green is not source-push admission; test fixes do not authorize a package rebuild/publication. |
| Next consumer import | Requalify the direct entry/local-bundle/TUI slice under #733, distinct #742 production footer and #755 wire tests, and the [TUI contract](../roadmap/TUI_APP.md). #749 is incorporated into #755, not another pending step; #751 is a separate unshipped loading-state correction. | Keep preview/confirm/unknown outcomes and final fix.8 first-use, clipboard and Help corrections. No new protocol, raw YAML or automatic VPN connection. New combined source/UI gates belong to the selected port. |
| Mandatory consumer Restore | Select a bounded recovery-to-ordinary-availability path from #739, readiness/order checkpoints #720/#721 and distinct source-fixture/product-fault gates #695/#694 under the [Restore contract](../roadmap/PRIVATE_BACKUP_RESTORE.md). Reconcile missing backup hardening #517/#520 deliberately. | Consumer Restore remains required for 0.9.9; public 0.9.8 stays Backup-only. Prove actual owner-loss recovery and fresh runtime admission, not only a happy path or same-holder Abort. Preserve original failures and NEW/OLD/unknown scopes. |
| Remaining removal UX / product guidance | Reconcile #774's still-pending all-state in-app removal review with the already shipped terminal remover, current guides and agent-first Help. | Do not reimplement the delivered helper, restore the old preserve-by-default contract, or advertise an unimplemented UI action. Translated human guides and locale-aware links remain a separate [future localization scope](../roadmap/I18N.md#published-guide-languages). |
| P4 WG/AWG research and port | Select a Rust-first slice from #770 store/preparation, distinct #750 timer/transport and #577 family/MTU composition under the [protocol roadmap](../../PROTOCOL_ROADMAP.md). #410 is incorporated into retained source, not another open task. | Current-core/parser/runtime compatibility, private fixtures, security and installed/live gates are distinct. Owner-supplied .conf files remain private; they do not qualify structured containers or product support by themselves. |
| K1 protection | Retain distinct #703 cold-owner and #689 native checkpoints, plus uncarried #546/#684/#676 under [K1](../roadmap/KILL_SWITCH.md). Normal-lifecycle WIP remains owner-paused. | No activation or automatic continuation from repository cleanup. Privilege/namespace/table ownership and crash/fault gates remain owning K1 requirements, not waived by T4's availability boundary. |
| T3 / T4 background / S1 | Preserve shipped read-only behavior separately from sealed T3 research: #660 composition, #664 native source, #562 owner-close, #683 interruption, #659 cuts, #622 graph and #597 diagnostics. #700 resume differs from #351 prototype; #623 global CAS differs from #693 child witness. #400 is an unactivated persistent-claim model. | No revival of unknown workers, copied live authority, background scheduling or foreign-setting changes. Historical source/build receipts are not fresh execution permission; shipped private nonpersistent usage reads are not #400's persisted claims. |
| G1 and standalone hosts | Retain #766/#768 [GUI research](../roadmap/GUI_RESEARCH.md), and separate [Arch/NixOS host work](../roadmap/PLATFORM.md). | No GUI framework/product selection or graphics dependency in the runtime follows from keeping a prototype. Omarchy evidence does not qualify NixOS generations or other host integration. |
| V0 / accepted migration | Keep frozen #30 and native successor #290 as distinct [protocol evidence](../../PROTOCOL_ROADMAP.md); retain deferred [AUTO-1](../roadmap/LOGIN_AUTOCONNECT_FOLLOWUP.md). | Missing protocol-family fixtures stay unavailable, not PASS. Do not restart accepted R6, revive Python production or make all other work wait for unrelated private fixtures. |

Old beta/RC assemblies and the frozen Python archive are historical references,
not unowned disposable work. Their open dependencies and exact evidence keep
them retained until a specific disposition. Detailed per-ref/machine inventories
stay outside Git; the owning PR carries current disposition, not a second
permanent session journal. For example, #740 preserves an attributed DNSControl
preview/apply research reference: it is not a DNS dependency or authority grant.

### Open CI performance and assurance decision

Before the next large feature port or expensive integration checkpoint, revisit
the allocation of local and hosted checks. This is an OPEN design question,
not a selected replacement policy. Apply
[technical judgment before execution](EXECUTION_POLICY.md#technical-judgment-before-execution)
instead of turning a faster local machine into an automatic extra full matrix.
Focused local iteration and purposeful full preflight remain distinct from
the current mandatory exact-source hosted/export/bootstrap gates.

The same-source/toolchain trial was about 4.7x faster locally, but two runtime
test binaries consumed about 87% of hosted Rust time. They use different feature
configurations; neither removing one as a duplicate nor adding a build cache
is justified by that measurement. Analyze existing evidence first, then select
one bounded local shadow experiment only for a concrete remaining hypothesis,
preserving assertions, failure/deadline coverage and configuration identity.

Compare keeping full PR/push checks with a lighter premerge policy or trusted
postmerge reuse. Lighter PR checks can defer discovery of integration failures
until after merge into shared development. Reuse requires the exact tested
inputs, original run/attempt and provenance of the actually executed workflow;
tree equality or a checked-out workflow file alone is insufficient. Changed or
unverifiable inputs must require full execution. This analysis does not waive
current gates or authorize exporter/trust changes, a personal-host public-PR
runner, paid capacity, or a new CI framework. Keep detailed timing/design notes
in the owning handoff; record the selected assurance tradeoff before changing
policy. Completing repository organization does not close this question.

#### Current recommendation

For the next developer checkpoint, retain full PR and exact development push
checks, and use focused local checks for iteration. Do not replace independent
hosted admission with the faster personal machine. A lighter premerge gate or
trusted postmerge reuse remains a separate owner decision, not an implementation
selected by the local-first preference.

A bounded same-source/Rust 1.98.0 shadow trial compared three ordinary Restore
cleanup matrices under default and the distinct image-witness/T4 configuration,
twice per variant. Runtime-package-only test opt1, with debug assertions and
overflow checks retained, improved individual mean wall times by roughly
0.5–3.4%; aggregate selected-case wall time changed from 1720.76s to 1691.92s.
All 24 original case runs passed, including real own-child crash/reopen prefixes;
fixture bodies, KDF parameters, assertions and deadlines were unchanged.
This is not evidence of a full hosted speedup or timing-sensitive conformance.
The variants ran sequentially on one local machine, not as a randomized
cross-host study. Candidate compilation took about 40s per configuration; a
comparable baseline build was not measured in this focused trial. Compile and
execution costs must not be conflated. This profile change is not adopted.

The useful result is a rejected weak optimization hypothesis, not a faster CI
claim. Existing logs identify expensive repeated Restore validation/file/fault
matrices, but do not identify one exact hot function or prove a hosted storage
bottleneck. Before further performance work, localize the remaining cost with
a specific bounded profiling question; do not launch another full benchmark,
skip a configuration, weaken cryptography or remove negative coverage. Keep
machine logs and detailed identities in the owning PR/handoff rather than
adding a permanent benchmark diary to source.

## Retained track entries

This is the earlier source-retention snapshot, not a fresh PR-state inventory;
the [current backlog](#current-backlog-october-21-2026) owns later dispositions.

The exact tips below are observed checkpoints, not moving declarations of
readiness. Common ancestors are measured Git relationships, not a claim about
the author's original branch-creation intent. Ancestor tags are historical
orientation, not the selected package pins for a new integration.

| Track / owning PR | Observed checkpoint | Relationship to released full source | Disposition |
| --- | --- | --- | --- |
| Old beta/0.9.9, [#737](https://github.com/k-kostin/omavless/pull/737) | `2a49ec0322502465c04aa7d842437d050713b559` | Common ancestor `b4ef783e`, nearest ancestor tag fix.1; 9 branch-only commits, 156 released-source commits absent. | Retain as historical development assembly; do not use as the new base or merge into user main. |
| Restore integration, [#739](https://github.com/k-kostin/omavless/pull/739) | `c783583c498c1c27afa3f9d7973a2162eadfda16` | Same common ancestor; 65 branch-only commits, 16 paths changed on both sides. | Preserve recovery contracts/failures and stacked drivers including #745/#747/#752/#759/#765/#769; qualify selected Restore scope independently. |
| TUI import, [#742](https://github.com/k-kostin/omavless/pull/742), [#755](https://github.com/k-kostin/omavless/pull/755) | Recorded #755 `3e32e2c9b99bdf69ee31eb02c63fb56e8e56ea2e`; #742 has a newer distinct production footer. | Common ancestor `b4ef783e`; #755 has 13 branch-only commits and 13 overlapping paths. | #749 is included in #755; #742's newer production delta is not. Port selected dependencies, preserving final onboarding/clipboard/Help corrections. |
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

## Verified missing-port boundaries

These are dated comparisons against the named old snapshots, not current
development793 absence claims. Later ports and closures, especially #520/#751
and the partially carried #684, are reconciled in the
[current backlog](#current-backlog-october-21-2026). Do not turn an older
"absent" row into a new port without checking its owning successor.

These were selected source-comparison findings, not implementation authority or
new vulnerability/acceptance claims. The referenced PRs retain exact old tips,
original failures and evidence. Compare again with current development before
porting: a shared concept, ancestor or filename does not establish incorporation.

| Retained source | Verified distinction / required adaptation |
| --- | --- |
| #517, including closed #513/#514 | Explicit caller-owned Argon2 workspace clearing, frozen V1 catalog and fixed-member openat/fstatat hardening are absent from released 5edf and #739. Reconcile later managed-DNS templates, archive compatibility and current backup/lease contracts; do not replace them wholesale with old code. |
| #520 | The bounded peer-first private recovery-stream reader is absent from released source and #739; it is not the same scope as #517. |
| #550 | The independent DNS-host pair builder's fresh bare object export, ambient/replacement Git suppression and post-reap no-signal changes are absent from the current builder. A different T3 exporter is not this implementation. |
| #572 | The exclusive-create loser test and deterministic output/Busy assertion correction are absent from current migration-lease coverage. Preserve legitimate conservative UnsafePath outcomes; this is not evidence that current CI failed. |
| #659 | The positive-stage-cut test matrix is absent from #660. One successful actual composition does not supply negative cut coverage. |
| #751 | The selected new-base UI port adds loading/unavailable/confirmed-empty Subscriptions guidance and user-facing refresh-failure wording, with actual-expression tests and isolated EN/RU render controls. It preserves cached rows, action/unknown fences and Commons.Color. Its owning development PR records exact current gates; old #751 source/rendering is not new-base acceptance or a whole-beta merge. |
| #546 / #684 / #676 | Standalone namespace API compile-consumer fixtures, the DNS/TUN managed-device contract/Go overlay, and the same-client four-exchange scenario respectively are absent from #703. Newer API/client artifacts do not establish these sibling ports. |
| #622 / #597 | The decoder counterexample graph and standalone Btrfs diagnostic are absent from the retained #660 composition; keep each source/evidence entry distinct. |

#395 and #375 remain historical matched-core/compatibility audits, not current
merge vehicles. Closed predecessors keep their source and unique failure reports;
closure means demonstrated incorporation or supersession, never feature PASS.

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

The initial inventory was read-only. Subsequent owner-approved cleanup removed
106 redundant remote names after exact-head/retention/dependency/custody checks;
their history remains reachable through immutable fix.8. #763 was closed with
its exact carried-patch disposition. The subsequent whole-backlog review closed
demonstrably incorporated predecessors and retained distinct source/evidence
carriers with individual top-level dispositions and original bodies below.
Current carriers are routed above; closed dependency references remain historical
source/evidence, not separately open implementation steps. No blanket retarget
or history rewrite.
The second 70-worktree audit produced a remote-name-only proposal; all local
checkouts/artifacts and unresolved heads stay retained. No second deletion,
main/release mutation or frontier acceptance follows from that proposal.
Current receipts/proposals stay local and in owning PRs, not report-only source
commits. #796's original failure remains separate from delivered fix.8.

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
