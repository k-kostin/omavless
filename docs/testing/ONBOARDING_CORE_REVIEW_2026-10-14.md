# Native core onboarding review — October 14

Owner requested a narrow 0.9.8 main follow-up after an installed screenshot
showed the same generic installation guidance for a correctly installed core.
Base: `a7db27bde88118504c14fc1a631a56a4ab0f9077`. Implementation/visual fixture
head: `90c1132bec024fbe298fc77c5b9f208e97d368bb`. Later prose-only
reconciliation does not change the tested frontend or native inputs.

## Behavior and boundaries

- Confirmed desktop core facts and ready component inventory show Core ready,
  actual installed version and Continue. Service context, actual TUN creation,
  DNS and internet are not proven by this step.
- Missing, supported-update, checking and failed/unknown facts do not permit
  ready Continue. The review action only navigates; the existing setup controller
  retains fresh inventory, pending/unknown-effect fences and terminal consent.
- Qualified public 0.8.2 has a paired app/DNS update. Refresh reads facts, and
  widget update does not update packages. No independent stock-Mihomo updater
  or automatic repair of an existing app's absent companion is added.
- Review retains Check again, Help and Back even when package presence is ready
  but permissions are unverified. Profiles and unrelated VPN/core installations
  are untouched; this follow-up adds no package or service mutation.

## Checks actually earned

Normal suite: 753 Python tests, two explicit opt-in skips, JS and QML contracts
pass. Focused onboarding has 22 controls, component setup 18; catalog/navigation,
manifest and affected qmllint pass. #783 records the final combined rerun and
hosted CI outcome; pending hosted Rust checks are not a PASS.

Installed isolated x86_64 Omarchy guest: actual updated checkout and supported
guest-only shell reload; Settings → Setup assistant showed Core ready and the
installed managed Mihomo version 1.19.31. An observed pointer click on Continue
opened Routing step 2 without choosing a preset or changing the runtime.

Synthetic rendering on that guest used the actual OnboardingWizard/SetupPage,
not Service or a package installer. Inspected EN/RU ready, missing, update,
missing-permission and unknown core states; ready enabled Continue, all negative
states refused it and emitted only component-review navigation. Inspected EN/RU
component install/update/missing-companion/unknown/ready surfaces, plus known
packages with unverified core facts. Review/back/check controls remain reachable.
Russian missing-core content also fits the declared constrained-height surface.

The initial setup capture caller used a disallowed underscore slug; no capture
was earned. A second caller selected an unsupported missing-companion fixture
enum and retained the previous state; those mislabeled captures are NOT evidence.
The fixture now admits that enum; fresh corrected EN/RU captures and inspection
show no install/update action for that case. This was test-driver correction,
not a package repair. Private captures stay outside Git, separated by surface.

These synthetic states do not claim new live package installation, a production
missing-companion repair, ARM64 installed validation or complete VPN acceptance.
Existing guarded installer/updater executable inputs are unchanged; their prior
exact-head live evidence is retained separately. No host VPN was touched.

The terminal-fence follow-up uses frontend source
`a691fcd2af5b6fc813eda27b4083301f7c77af78`: even with ready package inventory,
an opened setup terminal has its own waiting status and blocks Continue.
EN/RU synthetic captures and actual component inspection confirm the explicit
terminal/prompts-closed button remains visible on review. A pointer click on
that button cleared its synthetic terminal flag and refreshed only inventory;
no installer or authorization was opened. Clearing the wizard's synthetic flag
then permitted Continue. Independent Astra review found no source blocker in
the terminal fence; final instructions name component review as the actual
acknowledgement destination, rather than implying a button on the wizard.
Final focused tests are 23 onboarding and 18 setup; the owning PR records final
combined normal checks and final exact CI. These are agent-operated fixture
checks, not a claim that an agent can attest an unknown user's OS prompts.

## Workflow correction

Confirming that an installer exists did not close a user journey whose adjacent
states remained ambiguous. Apply the existing behavior/meaning/rendering gates
to the affected state contrasts, and report their evidence separately. No new
blanket full-product rerun or extra permanent review agent is required. Future
fixture callers must verify the requested state was admitted before attributing
a capture to it; both render harnesses now refuse capture after a refused
scenario instead of silently reusing its previous state.

The owning PR records final guest state/restoration, full source identities,
actual main merge and publication exclusions. Do not infer main or Marketplace
publication from this report alone.

## Systemic paired-update follow-up (not delivered)

After `3bec096d63d43459a39d884488a73cba5f9a6e8f`, owner review correctly
identified that the UI-only slice did not implement updates for previous
0.9.8 deliveries sharing version `0.9.8-1`. The subsequent source checkpoint
adds `omavless setup update-pair` inside the original retained upgrade context,
not a second daemon/privileged IPC owner. Unlike the initial slice, this changes
native executable inputs. Previous fix.4 acceptance does not cover this code.

| Scope | Source behavior | Remaining installed/delivery gate |
| --- | --- | --- |
| Current public pair | Normal setup/readiness, no redundant update | Keep existing fresh/partial setup checks separate |
| Compatible older 0.9.8 pair | Independent freshness enum; retain ready Connect/Continue and both stopped Start paths | Actual package replacement and post-update continuation |
| Qualified 0.8.2 | Original admitted paired upgrade remains | Regression on final new delivery |
| Known public app, genuinely absent companion | Explicit matched-pair installation, not generic repair | Installed missing-companion scenario |
| Mixed/modified/unknown/developer pair | Refuse implicit replacement | Negative installed cases where proportionate |
| Live broker/residual socket/held state | Refuse; no automatic stop, socket unlink, journal cleanup or enrollment | Separate clean administrative preparation before positive update |

Public predecessor admission binds app, core, broker and the complete DNS
receipt to compiled `plugin/published-pairs.json`. Eight unique architecture
pairs were independently extracted from `v0.9.8` and fix.1/2/3/4; fix.1 and
fix.2 share native bytes. Archive hashes matched both GitHub's asset digests
and anonymously downloaded SHA256SUMS; packaged source/architecture and
static ELF hashes were checked without executing the binaries. Checksums are
not signatures. Same-target admission matches the authenticated running helper
and target companion receipt, not an installed self-reported hash.

The original owner/migration exclusions remain through the package child and
post-verification. Selector, pre-managed-template backup and their absence are
retained; no enrollment, application start or connection is bundled into update.
The unchanged ALPM root guard still owns its private-journal/FD admission.

Local checkpoint checks: 766 Python tests/two declared opt-in skips plus normal
JS/QML; 33 focused Rust updater tests; Clippy with warnings denied; focused
20 setup and 24 onboarding controls; shell syntax and affected QML lint pass.
The original coreReview precedence tests caught an intermediate regression
where update hid missing core guidance; it was corrected and rerun. Independent
review caught advisory freshness masking ready/Start, then ready Continue;
production-function tests now cover both. Final exact CI and rendered/installed
gates belong to the owning PR, not these source results.

`component-updates.json` intentionally has `updatePair:false` for fix.4, whose
helper lacks the new command. Activation needs new immutable matched packages,
their actual download verification and installed acceptance. No new main merge,
release/tag replacement, issue772 reply or Marketplace action is established
by this source checkpoint. The physical PC remains untouched.

### Frozen native candidate and installed package phase

Native producer `9d2df8281373715a0d9eb91b615b7a4aa02dcecd` has successful
application CI [38528207212](https://github.com/k-kostin/omavless/actions/runs/38528207212)
and companion CI [38528311116](https://github.com/k-kostin/omavless/actions/runs/38528311116)
on both architectures. Both app/DNS archives pass the strict offline inspectors,
source/version/architecture/dependency checks and actual member hashes. Native
builder output on each architecture advertises `setup update-pair` and records
the selected `product-private-backup,setup-upgrade082` features. Full workspace
CI remains distinct/pending, not inferred from package success.

| Architecture | Application archive SHA256 | DNS archive SHA256 |
| --- | --- | --- |
| x86_64 | `ae341880bf073bfae8990fabacd5249ff0389eca6992d41591507a8e380450c7` | `60fd1326719e004367c32244dd4c3fc0cfc7945b7297a016f8aa8f18efdde99e` |
| aarch64 | `b418ddebbc61a93037990e8b0481f14c670e3e1b9d628357e1e4112001c4b5f3` | `c9eb7b8e06e76b17a6adf19bcf9f1cc008c3e8a78143fc28a3fab5caf7a7cbe4` |

The sole-operator x86_64 development guest replaced the installed publicb656
pair with these exact9d CI archives. Baseline fresh local observation was
Disconnected/recoveryfalse, Desiredfalse, no owned core/TUN/auxiliary processes;
enrollment already existed. Ordinary installed Full Quit succeeded and left
runtime inactive/disabled and the widget disabled. Separately reviewed, explicitly
authorized **administrative VM preparation** checked the original root broker,
empty private journal/FD0, safe parents and no TUN, then normally stopped it.
Only its inspected same-identity root660 single-link stale socket was removed;
the unchanged ALPM guard passed. This is not an automatic product preparation,
rollback receipt or permission to clean unknown user state.

The verified extracted9d helper consumed its bounded metadata and original
TTY consent; UPGRADE and ordinary sudo/pacman completed one two-package
transaction with no guard bypass. Result was
`packages_updated_broker_start_required`, DNSsetupRequiredfalse,
startAllowedfalse. The runtime and broker remained stopped; seven canonical
private members matched their post-Quit baseline exactly, including the empty
profile store, Desired, ownership, target, selector, managed template and backup.
This proves retention of this fixture, not populated live-provider acceptance.
Root enrollment bytes also matched their separately captured baseline.

An isolated frontend copy with independently inspected **offline target pins**
for9d then reported Start existing DNS broker. That ordinary existing-enrollment
action passed without re-enrollment and offered separate Start OmaVLESS. The
normal native Start passed: final components ready/present, fresh observed
Disconnected/recoveryfalse, Desiredfalse, no TUN/core, broker FD0, runtime active
but disabled. The seven private members and enrollment remained unchanged.
No VPN/network-health cycle, public download or installed ARM64 gate is claimed.

Fresh9d production-QML fixtures on the guest show optional update beside both
Start continuations in EN/RU. A compatible ready+stale_manual wizard displays
Core ready/version and enabled Continue; invoking its real synthetic Continue
handler advances to Routing without a preset/store effect. Captures remain
private. These isolated presentations do not claim GUI-driven package download.

The frontend-only target identity is intentionally separate from compiled
published predecessor data, avoiding circular self-hash builds. Pairing's runtime
equivalence check now explicitly includes compiled predecessor pins, embedded
package guard and the build feature selector, despite their location outside
crates. Target metadata remains frontend-only. Latest local normal checks pass
768 Python tests/two declared opt-in skips; focused pairing has16, setup66,
inventory11, frontend20 and onboarding24 controls. Exact final frontend/CI and
publication gates are recorded in #783, not borrowed from native9d.

Remaining public work: new immutable delivery, actual anonymous downloads,
matched frontend pins/capability activation, final CI and affected installed
delivery scenarios. The repository still selects fix.4 with capabilityfalse;
the private offline target metadata is not a public release. Main, existing
release assets and the physical PC were not changed.

### User-journey challenge: not accepted

The owner's request to finish the whole user path exposed a real remaining
product gap, not just an unrun screenshot. At frontend
`f988873281e2c4253373324cc4518f796e0c465d`, an enabled paired-update action
can be offered before the runtime/broker preparation has been completed.
The native updater correctly refuses a running runtime/broker or retained
socket. Full Quit also disables the widget. The linked current-main guide
describes the replacement boundary but does not provide an end-to-end
preparation-and-return procedure. Independent Astra review confirmed this gap.
The earlier administrative VM preparation is fixture administration, not
proof that a normal user can complete that preparation from the product.
Do not promote this candidate merely by publishing its packages or enabling
`updatePair`.

Additional installed **GUI continuation** evidence was earned on x86_64 with
frontend f988, native9d and the previously inspected private offline9d target
metadata. Only the three target metadata files differed from the frontend
commit; capability remained false. Supported guest shell reload preceded
inspection of actual loaded controls. This is candidate integration, not a
published installation or an anonymous new-release download.

- Actual Settings → Setup assistant displayed Core ready/version; a pointer
  click on Continue opened Routing step2. Escape returned without choosing
  a preset or modifying the store.
- Actual confirmed Quit stopped/disabled the runtime and disabled the widget.
  Ordinary `omarchy plugin enable kdk.omavless` reopened the stopped screen;
  its actual Start OmaVLESS button returned to ready without enabling startup
  or connecting VPN.
- A separately admitted administrative fixture stop checked the original
  broker/empty journal/FD0/no-TUN boundary and its same-identity stale socket.
  This is still NOT product preparation acceptance. An initial SSH Quit
  lacking the desktop environment refused before shutdown; the subsequently
  opened administrative sudo prompt was cancelled without entering a password
  or executing the helper. Only after a new normal Quit with the correct
  session environment and confirmed shutdown was the administrative fixture
  preparation invoked.
- Actual Start broker opened the ordinary terminal and original sudo prompt.
  It used existing enrollment, completed successfully and left the runtime
  stopped. Enter closed that terminal. Reopening the panel and clicking Check
  again retained the explicit terminal/prompts-closed fence; clicking the
  disabled Start button did not start the runtime. Acknowledging the settled
  terminal cleared the fence; actual Start OmaVLESS then returned to ready.
- All seven retained private members still matched their earlier baseline.
  Final fresh observation was Disconnected/recoveryfalse, Desiredfalse, no
  TUN/core, brokerFD0, runtimeactive/disabled and widgetenabled. No setup,
  pacman or sudo process remained. Guest idle preference was restored; no
  reboot, physical-host shell restart or physical-PC mutation was performed.

Required next gate: define and independently review the separate, explicit
clean-broker preparation and widget-return user procedure, then pass the actual
older-pair GUI → preparation → updater → terminal closure → both Start actions
journey. Keep any administrator-only assistance clearly identified. Preparation
must not become implicit updater cleanup. New immutable assets and anonymous
download acceptance remain a distinct delivery gate; a warmed CI cache is not
that proof. Main promotion and publication remain paused.

## Retrospective of the issue772 correction train

Scope: the October13 reporter issue, #778/fix.4, #782 and this #783 follow-up.
The reporter's original failed stage is still unknown: no reply with that
diagnostic has arrived. Reproducing related owner/VM dead ends establishes
those defects, not the reporter's exact root cause. The older setup report
retains original failures and exact subsequent successes; this section records
causes and prevention, not a replacement chronology.

| Miss / workflow cause | Correction and required regression |
| --- | --- |
| Generic setup failure implied pending authorization and gave no failed stage; initial acceptance concentrated on completed setup | #778 adds bounded phase diagnostics, explicit partial DNS continuation and separated consent. Keep cancellation after package install/enrollment and re-open/check/continue in the installed setup matrix, not only initial success. |
| Widget update was mistaken for app update; a fresh/developer fixture hid the owner's authentic0.8.2 package and unsupported APIs | Observe frontend, app/DNS receipts, package ownership and loaded controls independently before a host action. Keep a genuine previously published install as the updater's predecessor; never manufacture it from a current dev build. |
| Warm stopped fixtures hid absent volatile locks after reboot and ordinary retained private-state siblings | #778 added cold-owner and ancillary-sibling regressions with canonical/pending-state fences retained. Select fixture dimensions from the reached code's assumptions before execution; cold and populated/retained state are not optional substitutes for the clean-home gate. |
| Correct CLI/files were called a correct panel although the shell retained old QML dependencies | Verify distinctive loaded controls after supported reload. Keep stale-engine failure separate; rescan/hash success does not earn installed UI acceptance. No unattended physical-host shell restart. |
| Managed core was confused with stock core; accurate version was assumed from receipts | Test actual selected-core discovery and executable version in the normal matched packages. Retain separate stock-core installation without changing its capabilities. Artifact admission and displayed/runtime core facts are separate gates. |
| Installation help still described0.8.2/AUR, and valid links were treated as current documentation | #782 centralizes the public-guide/main rule. Trace actual help destinations and their action/consent content in the affected journey, including update/remove/reopen; a link-existence assertion only protects navigation. |
| Onboarding used a shared generic message; update freshness replaced ready Connect/Continue/Start; a terminal fence disappeared when components became ready | #783 covers readiness/freshness separately and keeps explicit terminal closure acknowledgement. Production-function and installed transitions must cover both sides of each action/state, not just catalog/source assertions or an arbitrary synthetic enum. |
| A screen offered an update that the pinned helper did not contain; receipt self-hash was treated as independent published provenance | Capability stays false on old deliveries; inspect the actual selected helper vocabulary and independently pinned source/architecture/member bytes. Compiled pin/build inputs are included in pairing equivalence, not misclassified as frontend-only. |
| Direct helper replacement plus manual broker preparation and a warmed CI cache were presented too close to whole-journey readiness | The preparation/return path is now an explicit unresolved product gate. Each external fixture intervention is an unearned user-path edge, not invisible test setup. Original GUI/terminal/postcondition and anonymous-download evidence stay distinct. No new publication request until the prepublication user-path gates actually pass. |
| Compound commands proceeded after refused Quit; a filter selected zero tests; a safe projection silently mapped unknown JSON shape to null | Effect-bearing phases must be admitted separately after checking the original result/schema and independent state. Require nonzero discovered test count. Wrong environment/schema/filter is a driver failure, not a product success or retry allowance. |
| Default runtime tests did not compile the released updater feature combination | The normal Rust runner now explicitly tests and lints `product-private-backup,setup-upgrade082`, and runs the release-broker queued/preaccepted access-mask regressions. A default-feature green result cannot replace the package's actual selected feature gate. |

Canonical prevention is added to the existing UI contract, execution policy
and their discovery skills rather than another parallel checklist. The next
implementation must exercise the omitted preparation edge early, include a
concurrent/queued-client refusal or drain test, and only then consume time on
final package/release assembly. Independent review challenges the execution
premises as well as source safety; a reviewed helper is not automatically a
reviewed user flow. The next installed run must show the improvement, not merely
reference these rules. No claim that all future defects are impossible is made.

## Explicit preparation closes the challenged installed journey

This section supersedes the **preparation/return blocker**, not the old failed
results or the remaining public-delivery gates. Native producer:
`99c556e5c966e1e1dae04f2d8babd2ca2dec9375`. Installed frontend:
`4c33cf411e33f910586795bec0d8d4c440125db2`; the later commit changes the
normal Rust runner, not native/package or frontend implementation inputs.
The prospective behavior is not yet selected by current-main release metadata.

### Early boundary and independent review

Before final candidate installation, the reviewed fixed root preparation was
exercised administratively in the sole-operator development guest. It masked
new UID access to the exact original Idle-gated public broker socket, waited
for the original pidfd's natural exit and checked the unchanged empty-journal,
FD0/no-socket/no-TUN boundary. It sent no signal and deleted no broker socket or
journal. The verified staged helper was removed only after known success.
This early administrative gate is not the later product journey's preparation.

Review removed a separate unsafe implementation premise: selecting a root
executable through an unprivileged parent's `/proc/PID/exe` after sudo could
follow an exec/PID-reuse change. The final code transfers a held, independently
qualified ELF into a fresh root-only staging directory, verifies its actual
root file identity and SHA256, then executes only that protected file. Each
original sudo child settles before the next action. Any post-root uncertainty
remains blocked and retains staging; cleanup is not a Drop/recursive effect.
Seven maintenance and five staging tests cover the boundary, including Linux
ACL/pidfd behavior and every production staging phase's failure. Two actual
release-broker Linux tests cover queued and already-admitted SCM_RIGHTS clients;
new access is denied without revoking the original admitted session. These
tests do not constitute an installed crash/timeout or live admitted-client test.

Independent Astra source review found no blocker for this exact preparation;
subsequent review of the root's installed evidence found no remaining blocker
for the described journey. That second assessment is evidence review, not a
second agent observing/controlling the VM or publication authorization.

### Frozen artifacts and actual path

Both native architecture builds and strict offline inspection passed for
producer99c: application CI
[38548293284](https://github.com/k-kostin/omavless/actions/runs/38548293284)
and companion CI
[38548293006](https://github.com/k-kostin/omavless/actions/runs/38548293006).
Packaged source/architecture/dependencies and actual ELF/receipt hashes match
the inspected candidate. The PR merge SHA in an artifact's filename is not
substituted for its inspected source receipt.

| Architecture | Application archive SHA256 | DNS archive SHA256 |
| --- | --- | --- |
| x86_64 | `5110045e1fc6aab520367d8b3d8d38feca7a91371b79bc2f79177cee01ea23d4` | `bfd65f5f410cf09d436cb1c8d5d5872ee9266c73f7bdc145c617a078fa6bab9b` |
| aarch64 | `cd6cb8ecbe55f97e50cac59bab98321d4f34bdd2ff43bcedccb825899afcbd9c` | `8e56778d15d16c550d8434498212550cbb1f1b6175cc46821990d9367f97fd40` |

The guest was returned to the genuine anonymously downloaded public fix.4
app/DNS pair, producer `b65695ee0609c2a971289109975678b8da87c946`.
Its app SHA was `b8cc106248953abcc3b3475efa7cec362778b5ae918b6866c1e8febdd3c44ff9`;
complete DNS receipt SHA was
`4e6e819d64d6c181ed5de8cd4de6e2cd0f920210b956f529a4ad2be54c285eb2`.
Normal existing-broker Start followed by separate app Start established the
running/disconnected predecessor. Original broker PID73397 and runtimePID74927
were observed independently before the update attempt.

The allowlisted frontend archive was installed through its ordinary native-only
wrapper. Only three **private offline candidate metadata** files then selected
the independently inspected99c pair/capability; the exact candidate archives
were preseeded and rehashed in the bounded private cache. This earns no anonymous
new-release download, cache-miss or public capability activation evidence.

Actual pointer/terminal sequence, without administrative broker preparation:

1. Prepare component update while the app was active opened instruction-only
   guidance. No updater/root prompt, package/broker change or app restart occurred.
2. Confirmed normal Settings Full Quit stopped/disabled the app and closed the
   widget. Ordinary widget enable did **not** start the app. The reopened stopped
   panel offered optional Prepare and the independent Start continuation.
3. Enter at PREPARE cancelled before root submission. Original broker73397 and
   public packages stayed unchanged; Check again made Prepare reachable again.
4. New explicit PREPARE and the original sudo prompt ran the actual qualified
   helper/staging chain. It reported `broker_prepared`; original broker73397
   exited naturally, FD0/no socket/no TUN remained, and the app stayed stopped.
   The production chain verified staged ELF identity/hash before root execution;
   no separate observation of the already-cleaned staging file is claimed.
5. Enter at the separate UPGRADE consent cancelled only package installation.
   App/DNS public fix.4 bytes remained unchanged. Check again offered both
   optional update and Start existing broker, not a dead end or re-enrollment.
6. A new admitted Prepare observed the already-clean boundary without another
   broker mutation. UPGRADE, original sudo and normal pacman Y installed both
   exact99c packages in one transaction; the unchanged ALPM guard passed.
   Original outcome was `packages_updated_broker_start_required`, with DNS
   setup not required and app Start not yet allowed. No app/VPN auto-start occurred.
7. Enter closed the original terminal. Explicit terminal/prompts-closed
   acknowledgement and Check again exposed Start existing broker. That actual
   button/original sudo completed successfully using the unchanged enrollment.
8. After its separate terminal close/acknowledgement/Check, actual Start OmaVLESS
   reached the normal Disconnected screen. Inventory was `ready/present`, with
   no obsolete optional update card; actual managed-core version was1.19.31.

Final independent read-only checks asserted the original control response
schema, observedDisconnected/recoveryfalse/Desiredfalse, no visible or owned
core/TUN, brokerPID91716 active/FD0 and runtimePID93214 active but disabled.
All seven canonical private members and separately root-checked enrollment bytes
matched the pre-journey baseline. This is an empty-profile fixture retention
proof, not populated subscription or VPN/network acceptance. Installed app ELF
SHA was `e1d320dc3347e2bdb4f45a576c34c42098a73e54e00f698010e5424cf97808b6`,
DNSreceipt SHA `5693dac3536ccff77e4863aa48b37a7a43c32a4b1d496939da9b6cba21dd1798`,
core SHA `e4bc011ecad3ce8350b738cee91a98e27e1b1dd1b0fa6eeb79eb7d6427669106`
and broker SHA `1855ed920a4b064c811ed14134b5ae14e3c5ece0a26695a84afc1ccfd711dc93`.
No preparation staging directory or sudo/helper/pacman process remained.

Post-update actual Settings → Setup assistant was inspected in EN and RU on
that same99c/4c installation. Both settled to Core ready/version1.19.31 and an
enabled Continue. Actual pointer Continue opened Routing step2; Escape returned
without choosing a preset or completing onboarding. The temporary widget locale
was restored to System; the seven canonical private members and original
runtime/broker PIDs still matched afterward. Checking/fresh-ready captures are
kept distinct; a transient checking frame is not the final verdict.

### Corrections to the test driver, not product claims

The native-only archive wrapper rejects `--native-only` arguments; its normal
no-argument invocation supplies that flag internally. The refused call made no
installation effect and was not counted. A widget enable without OMARCHY_PATH
also refused; only the subsequent normal desktop-context command counts.
An apparent missing native-panel acknowledgement was below the scrollable
content fold: the controller had clicked the fixed footer instead of the actual
scrollbar. Fresh correct scrolling and independent frontend review confirmed
the visible acknowledgement/Check/help controls; no flag reset or source patch
was used to manufacture success. Full-frame captures retain terminal outcomes
that cropped captures omitted. Each pointer effect uses the newly observed
layout, not stale coordinates after an acknowledgement changes panel height.
The first final read-only projection targeted an unwrapped result and selected
no observation; only the corrected original-envelope/schema assertion above
counts. Screenshots and private diagnostics remain outside Git.

### Gate status

Normal source suite passes772 Python tests with two declared opt-in skips and
JS/QML checks; focused setup20/onboarding24 and actual PTY shell regressions pass.
The45 focused updater tests pass under both the updater-only and actual released
feature combination. Full Rust CI for frontend4c is still pending at the report
checkpoint; package build success is not substituted for it. No fresh082 or
genuinely-absent-companion installed scenario is claimed on this final99c pair.
New immutable publication/pins, anonymous downloads/cache miss and ARM64
installation remain separate unearned gates. Current-main metadata continues
to select fix.4 with updatePairfalse. No main merge, tag/assets replacement,
Marketplace action, physical-PC change or issue772 reply was performed.
The initial public0.9.8 broker without the serialized access gate is not admitted
by automatic PREPARE; neither a known package version nor the general published
pair table proves support for that privileged operation. Its manual-assistance
boundary must not be represented as the successful fix.4 journey above.

## Immutable fix.5 delivery and clean-reinstall gates

The owner requested general CI, a clean-reinstall check and matched public
packages/download/installation, without interrupting the physical desktops.
Frozen validation frontend: `1e25a79796e1b7a2d9ba341f26b093c74c3c3b84`;
native/DNS producer remains `99c556e5c966e1e1dae04f2d8babd2ca2dec9375`.
`rc/0.9.8-fix.5` / [#784](https://github.com/k-kostin/omavless/pull/784) owns
delivery; #783 preserves the implementation branch. The following is agent-run
x86_64 development-VM evidence, not owner-attended or installed ARM64 acceptance.

- The genuine original public v0.9.8 pair was used as a packages-plus-DNS,
  application-stopped partial fixture. Its broker has no Idle gate. Separate
  administrator removal used known original normal stop, PID0/FD0, empty
  private journal, absent TUN and unchanged original stale socket identity;
  revocation preceded binary removal and ordinary pacman guards passed.
  The original clean exit was code0/status0, not the newer broker's SIGTERM15.
  No force removal, journal deletion or product-PREPARE claim followed.
- Previous private config/state/cache/widget were recoverably archived, not
  erased or published. The first attempted fresh fixture still contained unused
  runtime locks and an old consumed login receipt. The canonical guards refused
  initialization and then activation respectively. Their original failures are
  retained. Each residual was separately inspected/preserved under sole-operator
  VM administration; continuation eventually reached ready without reinstalling
  packages or repeating completed DNS enrollment. This is **partial continuation**,
  not a continuous fresh-install result or proof of Aryan's unknown original cause.
- A separately admitted clean baseline, including absence of runtime directory
  and login receipt, then passed the uninterrupted offline GUI journey:
  Install required components → INSTALL → original sudo/pacman → DNS → ready.
  No administrative intervention occurred inside this product path. Preseeded
  reviewed packages were used only here; this is not public-download evidence.
- Immutable [v0.9.8-fix.5](https://github.com/k-kostin/omavless/releases/tag/v0.9.8-fix.5)
  was published as **prerelease/not latest**, with seven assets. Tag target is
  the frozen frontend above; latest remained fix.3 and main was unchanged.
  App/DNS maps and updatePairtrue select producer99c on both architectures.
  Both offline pair assemblies passed with equivalent runtime input-tree hash
  `4bb537ace9d2a46c3fd7591e1921ad86ab543aff2b49926b65a52a40af78afd0`.
- All seven assets were downloaded anonymously over bounded HTTPS; local hashes,
  uploaded GitHub asset digests and SHA256SUMS matched. Downloaded app/DNS packages
  were independently re-inspected/re-paired for x86_64 and aarch64. Common frontend
  SHA256: `00282a858ef835d230bfc7542d78b029f32946b84eb19f14175d01bd39a09005`.
  Prepared immutable metadata keeps prepublication booleans false; postpublication
  results belong to this report/PR, not rewritten published files.
- A new ordinary Omarchy add without enable, followed by explicit candidate-tag
  checkout **before enabling**, admitted the exact frozen frontend. Packages,
  config, state, runtime directory, login receipt and download cache were absent.
  The actual GUI INSTALL fetched both fixed public endpoints with a cache miss;
  ordinary sudo/pacman → separate DNS → ready passed without intervention.
  This explicit candidate selection is not a main/Marketplace install claim.
- Installed app/broker/receipt hashes matched the inspected99c pair. Actual and
  desired state were Disconnected, transition absent, native ownership true,
  startup preference Off, profiles empty, broker active/FD0 and managed TUN/core
  absent. The application service was enabled by ordinary fresh setup, separately
  from VPN autoconnect. No sudo/pacman authorization remained pending.
- The anonymously downloaded frontend wrapper's normal `./install.sh` passed
  over that activated pair and retained the original application PID. Its loaded
  wizard showed Core ready/version1.19.31 and Continue reached Routing2/3. No
  profile import, preset change or VPN connection was needed for this gate.

Full Test on equivalent-code frontend4c
[38548694245](https://github.com/k-kostin/omavless/actions/runs/38548694245)
completed successfully. Frozen RC1e full Test
[38563158711](https://github.com/k-kostin/omavless/actions/runs/38563158711)
and report-head55ff full Test remained in progress at this checkpoint; neither
is labelled PASS here. Local normal checks passed772 Python cases/two declared
opt-in skips and JS/QML. Frozen RC native/package jobs passed on both architectures;
the inspected producer99c archives, not new document-head builds, remain selected.
The PR records subsequent exact CI conclusions.

Astra's source/evidence review found no additional scoped practical blocker;
that review did not independently operate the VM or download the assets. Known
limits remain original-v0.9.8 manual assistance, installed ARM64, a final099
all-stage uninstaller and untested live network-health cases. Main/stable/latest,
Marketplace and a new issue772 reply were not performed. SSH and selected-guest
QMP were used; no physical desktop/window focus, shell restart, network/service
change or VM reboot was needed.
