# 0.9.8 development selection

Owner-selected on 2026-10-02; internal assembly renewed on 2026-10-07.
Only an accepted release in main is user-facing. Base: `rc/0.9.7` at
`c4e800425243c1b02165f82153e4bf418fe465e6`, retaining the selected 0.9.6
read-side changes, honest Connections loading and manifest-derived Settings
version/credit. The historical [0.9.7 ledger](RC_097.md) keeps its own evidence;
unpublished source integration is not public release acceptance.

## Scope and ownership

### Beta.4 selected assembly, 2026-10-07–08

Owner authorized continuing after the installed beta.3 Backup scenario without
redundant manual keystrokes. Selected base is
`a791fb2eb312ca2405ede194718146b49167b7ae` on `beta/0.9.8`: native UI/source
evidence and the repeatable desktop-entry gate remain separately identified
below. Current assembly writer owns `dev/098-beta4-assembly`; the VM master
owns only the source-only Restore transfer branch until explicit VM handoff.

Beta.4 includes authenticated Preview/Cancel/Restore from #708 at
`b33d2f5b783e308be0f74ad6e5531fe4c1573f3f`, after the transfer resolves the
known conflict with #709's global Backup navigation and preserves both original
unresolved-operation guards. Source transfer
`ae973ab7c629f60f86b60417e78e2e37c72298e8` is integrated; installed Restore
passed on the exact bundle recorded below, not by inference from the version.
Relaunch after plugin disable/full shutdown
is an optional later included checkpoint only after implementation and its own
no-autoconnect/current-login executable gate. K1, S1, P4, automatic background
work and GUI remain excluded.

Owner clarified that normal Open app must expose selected beta functionality,
not require hidden per-invocation flags. The closed package build selector
`packaging/release/client-features.sh` selects the reviewed T4 feature only for
`0.9.8-beta.4`, not stable/RC/other beta versions. That selected build's ordinary
`omavless tui` supplies both existing Backup and Restore adapters. Source-only
helper flags remain compatibility/research entries, not the owner test path.
The runtime still checks current owner/capability/Off/unknown predicates;
client visibility is not operation authority. Stable builds without the feature
remain unchanged. No K1/Product image-witness feature is selected.

Agent-owned next gates: combined source/feature checks; matching experimental
bundle inspection; normal plugin Open app close/reopen; wrong-key no-effect;
authenticated preview and Cancel with unchanged state; one original successful
Restore with independent pair/history/revision readback; normal restart after
a known outcome. Unknown stops dependent mutations. Human review is optional
product judgment, not a substitute for these automatable checks. Public package
pins remain empty, no package publication or main/RC/Marketplace update occurs.

Owner clarification, 2026-10-07: this internal beta may include runnable
experimental features with programmatic gates passed and human acceptance
pending. Apply the [manual-acceptance rule](BETA_MANUAL_ACCEPTANCE.md) and
[actual 0.9.8 test plan](../testing/BETA_098_MANUAL_PLAN.md). This does not
activate missing client/runtime paths or claim unrun installed checks PASS.

### Beta.4 installed ordinary-entry and Restore checkpoint

Installed x86_64 native source is
`09f238ff142c13201569e514fed1ab3b636b5829`, frontend source
`a2e5b6473be44a242a1ac7e0e58e42dae73c353e` (test-only successor; inspected
runtime-input equivalence). Native ELF SHA256 is
`0b5ea3602c4b0d6677c401630fe85ca7fbb2042bb8db109b1c579cf8cc4e9869`;
matched DNS source receipt SHA256 is
`0749076d6c46419c5df1397f03689be52baae38cbbb62c44110755aa17085c4f`.
Normal two-package update and frontend installer completed with original exit0.

Agent-operated real plugin Open app, client-only close and same-button reopen
pass: actual argv is `omavless tui`, no developer selector, visible `b/F2`,
working Backup editor, Settings Restore and working Restore editor. Pair/Desired
were unchanged by these actions. This replaces, rather than borrows acceptance
from, the earlier dedicated-launcher workaround.

The same installed ordinary client exercised wrong-key refusal, authenticated
Preview and Cancel (unchanged whole pair/metadata/Desired/revision). A normal
semantic rename changed one disposable local profile and advanced revision to1.
One original UI Restore then returned Completed and advanced it exactly to2;
independent canonical JSON and template comparison matched the archived original,
the earlier profile label returned, startup remained Off and Desired stayed
unchanged. A separately known read-only refusal during fast VM key delivery was
not counted as success: fixed CLI Preview authenticated the same input; slower
verified VM input reached the real confirmation before the one Restore submit.
No uncertain mutation was resent. Original successful reply and private captures
are retained outside Git.

After that known outcome, client-only close and normal runtime stop/start pass:
fresh instance is owned disconnected, restored pair/Desired hashes unchanged,
no TUN. Revision is per instance (new instance starts at0), not falsely preserved
across restart. Existing durable history was retained; no history-count increment
claim is inferred from an unrecorded baseline. No Connect, root broker activation,
enrollment, foreign-VPN change or physical-host modification occurred. The VM's
inactive managed DNS broker remains a separate connection/setup limitation.

Exact09f local `tests/run.sh` passed688 cases with2 opt-in skips and Node/QML
contracts; runtime T4 library passed1440 with64 explicitly ignored host/resource
cases. Both normal-entry feature configurations, affected strict Clippy and the
combined TUI pass. These are not full `run-rust.sh`, connected network, ARM-installed,
fault/replay/second-Restore or public-release acceptance. Remaining candidate CI
belongs to its exact head, not this installed proof.

The next integration queue is based on the
[read-only PR/cache inventory](https://github.com/k-kostin/omavless/issues/706#issuecomment-6046610211):
finish bounded beta gates and runtime relaunch, curate useful workflow changes,
reconcile already-included/equivalent PRs, and keep unfinished frontier work
explicitly separate before an owner-authorized RC/main update. Cached binaries
are evidence/artifacts, never substitutes for included source or working features.

The next October 7 assembly continues checked `beta/0.9.8` from exact
`1a73f386ce0c78a9e5a7c4c5cedc2eec98bd6963` (#702) through a separate narrow
integration branch. The VM master has stopped all feature writers and handed
off custody in [#706](https://github.com/k-kostin/omavless/issues/706).
The beta integrator is the sole integration writer and potential next VM
operator; source assembly does not itself begin VM operations. Before any
installed pass, recheck the actual image/boot/units and select one operator.
No main/RC/tag/Marketplace or physical-host change is authorized by this
internal selection.

| Selected checkpoint | Exact source | Meaning in this assembly |
| --- | --- | --- |
| Retained private backup/restore, [#690](https://github.com/k-kostin/omavless/pull/690) | `44bd54e100c7daeeaa2f69cf987be381f9a8e519` | Existing authenticated current-owner path, positive OLD/NEW completion and eight immutable history slots; optional developer client/service, not default Backup/Restore UI. |
| Selective connections, [#694](https://github.com/k-kostin/omavless/pull/694) | `d294c36352eb964b6f44d471dc20a5be1ad7cdb8` | Existing opt-in Product image-witness/semantic TUI path with original receipt-only resolution; ordinary daemon does not advertise close. |
| Normal private-pair API, [#701](https://github.com/k-kostin/omavless/pull/701) | `ec7f5a9aa9fd35a5ae11216937d407ea83fd79ee` | Fixed Backup and Restore API/CLI, genuine-current owner admission and shared scheduler uncertainty fences, explicit compile opt-in only. No Restore UI. |
| Backup TUI, [#705](https://github.com/k-kostin/omavless/pull/705) | `d4f5ac2e3d644185a408dacc48dbd0b0a5bfd540` | Real Settings Backup editor/confirmation/result flow, selected by `tui --developer-private-backup`; profiles/subscriptions and finite supported routing template only. |

The #705 source gates are green; its earlier installed Backup-only evidence
belongs to source `662ba08767993374d7101f2e7bdfd723d337898f`, not automatically
to this new combined assembly. See the [Backup client contract](T4_PRIVATE_BACKUP_TUI.md)
and [normal API contract](T4_PRIVATE_PAIR_NORMAL_API.md). The shared library
combination preserves both explicit TUI selectors; ordinary invocation selects
neither. Newer Restore preview `6164e7f5`, stale-Confirm `1f7485ca` and K1 normal
lifecycle WIP `a456004d` are preserved but excluded, not missing human-only gates.

Merge the complete prerequisite histories, not orphaned leaf changes. Dormant
NetGuard and restore-research modules inherited by those histories are source
inventory, not accepted or newly activated kill-switch features. Optional
helper compilation does not provision root enrollment, activate a service,
renew a context or make an ordinary runtime eligible for close/restore.

The shared-field merge preserves BOTH trait/test/feature sets. The T3 test-only
owner constructor gets `current_origin: None` under the T4 feature: a test
factory must not acquire genuine current-owner restore authority. A new
combined-feature regression checks that ordinary opt-in compilation alone
advertises neither feature, refuses false-origin Restore and all close RPCs,
and preserves profiles/revision/host-call counts.

Admission requires more than internal encrypted-file primitives: a fixed
semantic owner/client path, safe passphrase/destination handling, explicit
authenticated preview and replacement confirmation, disconnected/drained owner
checks, durable multi-file outcome/restart handling and imported autoconnect Off.
Actual installed synthetic restore and interruption checks must pass before
claiming this user flow ready. Private real stores are not destructive fixtures.
Product K1, S1, P4 and G1 are excluded. Automatic subscriptions and sleep/network
recovery [#700](https://github.com/k-kostin/omavless/pull/700) at
`87cc3009c24f300fe85620e28000c730a489924a` remain separately checked but are not
selected: their shared-owner composition needs a separate affected integration
review and they lack production binding/bootstrap/registration. The ordinary
default restore-abort CLI and restore startup fences inherited from #690 are
real runtime surface, not mislabeled test-only code. Preserve all original
refusals. #30 implementation, Draft status and XHTTP evidence stay intact.

## Checkpoint and release boundary

Current source version `0.9.8-beta.4` / Arch `0.9.8beta4-1`; both package pin
maps stay empty. No old archive is relabeled. Baseline checks cover version/packaging
coherence, full developer/Rust suites and affected EN/RU actual QML/TUI review.
New combined source/build checks do not transfer earlier installed results to
new binaries. The selected beta.4 producer includes T4 for normal `tui`, as
described above. Historical beta.2/3 default artifacts had default `tui` only;
a separately compiled
product-image-witness + t4-manager-actor-service development executable is
explicitly opt-in and cannot be used to infer normal feature activation.
A historical Backup-only experimental executable instead selected default `tui` plus
`t4-manager-actor-service`; producer feature selection and binary/package hashes
must accompany it. Those old default CI artifacts did not provide the Backup screen.
Never mix the earlier LegacyMeta2 Backup bundle with the separate T3 Product
helper/core/broker/enrollment bundle. Both must earn their own current admission.

Internal RC scope freeze may follow the declared risk-based checks without
publishing assets or repeating every predecessor's clean installation. Public
package/download/first-use and supported-host gates remain separate release
work. Main, accepted public 0.9.5 RC1 and Marketplace are unchanged.

## Beta.3 installed Backup checkpoint, 2026-10-07

### Historical beta.3 navigation and repeatable-entry workaround

The installed x86_64 UI leaf
`a7953c481850809378b5e620fc9ce53bdac0dfa1` has experimental ELF SHA256
`4064d871277042ede32c5cc551001f4950fd55be7d10423253fe34e1f7fedcc3`.
Only presentation/navigation changed from the earlier 27d runtime checkpoint;
runtime/domain/broker/systemd source remained unchanged. Existing 27d DNS/core/
broker companions were retained for this internal UI-only pass: it is not a
new same-source release-pair acceptance or public artifact pin.

The owner confirmed opt-in navigation/focus/Escape. A subsequent normal Open
app invocation correctly lacked the opt-in Backup adapter, exposing an agent
preparation gap rather than a reverted binary. The
[manual-plan postmortem](../testing/BETA_098_MANUAL_PLAN.md#постмортем-ручного-входа--2026-10-07)
retains that failed handoff. A clearly named VM-only desktop entry now keeps
the selector, title and app-id distinct. Actual desktop-entry launch, client-only
close, same-entry reopen, visible `b/F2` and actual Backup navigation pass.

After reopening, the agent executed one real Create to a fresh private test
destination and observed Completed. Independent read-only authentication
matched the complete synthetic store/template pair and refused a wrong key.
A separately submitted existing-destination operation returned Denied, keeping
the archive bytes/metadata unchanged. Whole pair/metadata/history/Desired,
revision 0 and the same owned disconnected runtime stayed unchanged. Closing
the opt-in client also left that runtime running; no TUN/network transition,
Restore or privilege provisioning was performed. Private archives/captures
remain outside Git. Invalid/cancel/EN-RU constrained-view cases keep their earlier
exact-source evidence; they are not relabelled as newly executed here.

This historical internal beta.3 Backup scenario is installed-checked; owner archive creation
is UNRUN but is no longer a redundant beta blocker. The successor adds only a
development desktop entry, regression coverage and reusable preparation policy,
not production/default activation. Beta.4 supersedes that separate-entry
workaround with its reviewed normal-entry build. Relevant launcher/source tests (34), desktop
validation and documentation navigation pass; remaining hosted CI status belongs
to the PR's exact head and is not inferred from these installed checks.

Owner hands-on follow-up: Settings-only `b` was not discoverable/reachable from
the main Profiles or Activity page. The earlier agent pass did not establish
that user navigation scenario; it is not called owner PASS. A narrow opt-in
successor exposes `b/F2` in the existing page legend and opens the same editor
from normal pages, with uppercase/Russian physical-key aliases. Search/modals,
original-outcome handling, current-capability/freshness and minimum-size guards
remain unchanged. No new export or runtime/backend action is introduced by
navigation. Source/installed evidence for this successor belongs to its owning
PR; the exact older source below remains the executed archive checkpoint.

The integrator's separately selected x86_64 Omarchy VM tested runtime source
`8de29a164e885f398dbb926c69fd377af4764869` with default `tui` plus only
`t4-manager-actor-service`. Its experimental ELF SHA256 is
`78cc8020b692a2204c8a8afd596a24062d1b837b147eeed6021d7e924cba96b8`;
application package SHA256 is
`30eb4531e3f16ace7abc11ffc6f88046536161128a01c3147bb37b3b3370c6e3`.
The matching beta.3 DNS package/frontend were inspected and installed, not a
Product image-witness bundle or a relabelled older package. All five hosted
checks and the full local combined Rust runner passed at that exact source.

Actual normal current-owner admission, startup Off and disconnected state were
verified before the real Foot/TUI path. Preparation replaced only disposable
test data while the runtime was stopped; inherited data and the sealed backing
image remain private. Two synthetic profiles, one `example.invalid` subscription
and a supported default routing template were used, never a real store as a
destructive fixture. The first preparation preflight correctly refused an
absent last-profile reference; the corrected fixture passed before runtime start.

Agent-operated installed results, distinct from human acceptance:

- EN editor/confirmation at 70×24: full destination, masks and credential/lost-
  passphrase/no-overwrite warnings are visible. Empty input cannot confirm.
- Cancel returns to Settings without an archive or data change; re-entry has
  empty secret fields.
- One original EN Create displayed Completed. A separately reviewed read-only
  domain oracle authenticated the archive as the exact pre-operation private
  pair and refused a wrong passphrase; it did not invoke Restore or write data.
- One new RU request for that existing destination displayed Denied. The
  archive's ciphertext hash and full metadata were unchanged, with no overwrite
  or automatic resend. A grouped UI refusal is not an exact wire-code claim.
- Independent sampled comparisons proved unchanged private-pair bytes and
  metadata, Desired/ownership/bridge/history, revision and owned Off state.
  Fixed pending markers were absent. Original Ctrl+C closed only the TUI,
  with client exit0 and the same healthy disconnected runtime remaining.

The installed pass found a clipped RU Settings footer and an undiscoverable
Backup shortcut when the entry was below the 70×24 viewport. The narrow
successor shortens the opt-in EN/RU footer and exposes `b` there; its regression
requires the complete scope and shortcut to fit, without changing admission or
runtime behavior. Exact successor build/render results belong to #707, not to
the older execution evidence above. Original failures/captures stay private.

This is Backup-only, agent-attended integration, not owner hands-on acceptance,
Restore UI, connected export, fresh-public installation, ARM experimental
activation or whole T4. The [manual card](../testing/BETA_098_MANUAL_PLAN.md)
remains the bounded owner check; main/RC/release/Marketplace are unchanged.

## Historical beta.1 baseline checkpoint evidence

Runtime/package source: `931809c48cf25956889963c0578437d074cee3fa`.
Reviewed frontend/test head: `8aef640312d3832465b18305d038e31d19387e29`.
The offline managed-DNS pair inspector proves identical protected runtime
inputs between them; both archives are newly labelled/built 0.9.8, not 0.9.6
archives renamed. Both pin maps stay empty and no public assets are published.

- Full developer suite: 501 Python tests, 2 expected skips; JS/QML contracts
  and documentation navigation pass. Focused package/setup tests: 50 pass.
- Full Rust workspace: 1385 pass, 12 ignored. Installed ARM64 managed Mihomo:
  2 renderer/Unix-controller integration tests and the loopback validation
  side-effect test pass. No real profile or host-network change is a fixture.
- Shell, JSON, compile/diff, plugin validate and Qt6 qmllint with installed
  Omarchy imports pass. All five initial CI jobs pass on the package source.
- Committed product QML in the isolated fixture: EN/RU Settings footer at
  widths 360/460 reviewed; correct version/credit and no Quit activation.
  The harness now stages its committed public manifest and has two regression
  tests. Synthetic rendering is not real connected/provider evidence.
- Agent-operated ARM64 upgrade from installed 0.9.6 beta to the inspected
  0.9.8 app/DNS/frontend pair passes the unchanged pretransaction guard and
  binary/27 frontend-file identity checks. Private store bytes unchanged;
  plugin enabled and IPC responds; Routing/Disconnected, zero core/TUN and
  no manual recovery. This is local developer integration, not a clean
  public first-use, physical-PC or enabled-login-autoconnect gate.

## Historical October 2 late T4 admission result

Latest reviewed owning checkpoint: [#493](https://github.com/k-kostin/omavless/pull/493)
at `2a75c2dfe1c5bcbbfa22dcf2107284fbae1393bb` (2026-10-02). Its exact-intent
successor executor is explicitly **inactive**. The stack still has no product
backup/restore caller, client/passphrase UX or approved ordinary-owner startup
admission; successor fences remain and installed whole-flow acceptance is not
claimed. Its security contract and PR expressly prohibit treating these green
internal primitives as product activation.

At that October 2 checkpoint, no T4 stack was admitted or rewritten. The checked maintenance
base can live in `beta/0.9.8`; the intended backup/restore product scope and
promotion to a complete 0.9.8 RC remain pending that separate owning checkpoint.
Do not add fake working controls or call this beta a completed T4 release.
The October 7 selection above supersedes this old exclusion, not its original
test results or the remaining whole-product acceptance requirements.

## October 7 combined acceptance and handoff

The first combined hosted candidate `7bba8a47` failed one unignored synthetic
reader control (1464 pass / one failure / 75 ignored). It incorrectly used the
VM's fixed UID/GID for a file created by the hosted runner's own user. A pinned
old test image reproduced the same refusal in an isolated UID/GID1001 namespace;
the actual hosted predicate was not separately logged. The test-only successor
separates the generic fixture's real owner from the unchanged fixed1000 VM
reader entry and retains independent wrong-UID/GID negatives and every original
mode/link/size/canonical/xattr/byte/name/directory check. This failure remains
failure, not "flaky" or installed recovery evidence. No VM selector was executed.
New exact-head package builds are required after this crate-file change: the
strict protected-input comparator cannot re-label the older4594 packages even
though the changed module is test-only. The final assembly PR owns actual gates
and newly built artifact identities; the older pairs remain their own snapshot.

This assembly runs the full source/Rust gates and explicit combined-feature
check, strict all-target Clippy and ordinary library tests. All existing ignored
VM/resource/privileged tests remain ignored; there is no blanket ignored run.
Source gate uses the separately reviewed HOME-backed offline fixture launcher.
No installed GUI or live/network result is claimed for this combined candidate.
Exact executed source, artifacts and actual outcomes belong to the assembly PR.
The actual both-architecture app/DNS/frontend artifact checkpoint is
`4594a48703a49b8c40e8f710e54ca230f707fd08`, before documentation-only status
reconciliation. The strict pair inspector verifies exact app/core/broker ELF
architecture, source receipts, package scripts/units, payload digests and empty
bootstrap pins. Newer documentation/frontend pairing must independently prove
the protected runtime-input tree unchanged; do not relabel package provenance.
Full local offline source failure remains recorded: inherited HOME-direct tests
cannot write in the read-only-HOME source launcher. The ordinary hosted source
gate passed at the artifact checkpoint without weakening those tests/guards.

### Next Backup client assembly

The beta.3 source/build checkpoint must check default method/entry absence,
T4-only API and Backup controls, both TUI feature combinations and the
product+T4 runtime union. The union controls additionally deny normal pair
methods from a false-origin owner and prevent close/Quit host effects after an
unknown pair outcome. Existing ignored resource/VM cases remain ignored.
Strict format/lint, packaging/version tests and full final combined gates are
required; a source label or historical green predecessor is not their result.
Exact head, commands, outcomes and artifact producer options belong to the
assembly PR. New combined installed and owner manual cards start UNRUN.

Historical beta.2's final checked source is
`06f9bc1ca416421948c0678e7c6b3ea3596368e7`, merged as `1a73f386` via #702.
The earlier artifact checkpoints above remain historical rather than being
relabeled as the final beta.2 or beta.3 build.

Before experimental activation on the development VM, review exact rebuilt
application/helper/DNS-pair identity and existing provisioning/admission paths.
The sole designated operator owns that separately selected installed pass. Repeat affected
EN/RU close/receipt and current backup/restore/ordinary-restart checks on the
combined binaries; keep prior-head negative and positive evidence intact.
Product fault cuts, public/default UI activation, physical sleep/network and
ARM/Nix gates remain with their owning contracts. Beta integration alone neither
closes the entire T3/T4 roadmap nor promotes 0.9.7 RC or publishes a release.
