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

## Complete old-partial removal and reinstall — October 14 follow-up

The previous fixture-admin cleanup did **not** establish a copyable user
removal path. The owner requested that missing gate explicitly before an issue
reply draft. [The bounded clean-reinstall guide](../user/CLEAN_REINSTALL_098.md)
now has seven self-contained Bash blocks usable after the widget checkout is
removed. It is a **candidate** guide, not a general 0.9.9 uninstaller or a main
installation claim. Scope is one known desktop account, never-connected normal
public 0.9.8 partial setup, no concurrent clients/unknown DNS operation, ordinary
canonical paths. Other accounts, custom paths and recovery states are excluded.

Before this user journey, separate VM fixture administration preserved the
previous ready fix.5 store and restored the genuine original public pair from
producer `4915ae4a8f4ec6cfb469ad6b38126bd70d5a1998`. Its original app ELF was
`219479595f4b59e07a9639d703e939125d464165f7a5f426ec523ce5d952600b`, broker ELF
`8caba1a57b44b2856b98b0d0213cc545905ba5a031aac144fcce2420957b0d2e`; frontend
tag `v0.9.8` selected `d6ee207a6d2a42e602e85854cd6b7cba5ea61084`. Native first
initialization and fixed root enrollment/pair selection admitted packages plus
DNS while leaving application ownership/activation incomplete. The original
panel showed Finish application setup. No VPN was connected. This representative
partial baseline is not reproduction or diagnosis of Aryan's exact initial cause.

Astra reviewed the procedure before execution. Initial review strengthened
the loaded unit/actual original broker identity, completion checks and canonical
backup parent checks. The literal seven guide blocks were extracted using
`tests/test-clean-reinstall-guide.js` and executed unchanged in the selected VM:

1. Ordinary `omarchy plugin remove` with its real confirmation removed the
   genuine old widget; application startup was disabled and PID0 verified.
2. The guarded root block checked protected empty journal, FD0, absent app/core/
   TUN, pinned unit bytes and loaded-unit state; it identified the original root
   broker, performed ordinary idle stop/disable, proved its disappearance and
   repeated clean checks. It returned `CLEAN_STOP_CONFIRMED`, exit0. Original
   systemd fields were Result=success and code0/status0; this is reported as a
   joined observation, not independently labelled CLD_EXITED.
3. One ordinary guest reboot ended the already-proven clean volatile epoch.
   The same QEMU process/window remained running; guest disk unlock used its
   original prompt. New boot, inactive units and absent socket/runtime/login
   receipt were verified. There was no host reboot/window-focus change. Unlike
   the earlier no-reboot setup gate, this newly scoped removal gate includes a
   deliberate guest reboot. Reboot did not replace pre-reboot emptiness proof.
4. The installed guard passed, the original fixed `--revoke` succeeded while
   its broker binary existed, and ordinary pacman removed only `omavless` and
   `omavless-dns`. Their normal removal guard was not bypassed. Exit0.
5. Canonical old settings were recoverably archived under the user's protected
   reinstall-backup directory. The new runtime path and consumed-login receipt
   were absent; no manual unlink, journal/lock deletion or second reboot occurred.
6. Ordinary Omarchy add cloned default main without enabling it: Yes to cloning,
   **No to the separate enable prompt**. Exact tag fix.5 was checked out and its
   immutable SHA verified before explicit enable. This important prompt is now
   documented, rather than left implicit in 'without enable'.
7. The actual panel offered missing app/DNS installation. INSTALL → public pinned
   cache-miss downloads → original sudo/pacman confirmation → separate DNS →
   ready completed without administrative intervention. The original terminal
   printed OmaVLESS is ready. After closing it, Review component setup →
   terminal-closed acknowledgement → Core ready → Back to panel reached the
   ordinary Disconnected screen with visible import actions. No shell restart,
   profile import, country selection or VPN connection was required for this gate.

The copied root block SHA256 was
`6093cc1307f9067edcb90616510650b51e320372f19944a0a063397aad0c9b96`;
the private archive block was
`745e0c305d0e9c1a14f201fec47a193827322b8e04c59c0ba3b2b1daaed294a6`.
Their extracted guide bytes matched after execution. Guide syntax/order/boundary
checks passed and are wired into normal `tests/run.sh`; normal local checks
passed772 Python cases/two declared opt-in skips and JS/QML contracts.

Installed native/DNS remained producer99c, frontend1e. Actual downloaded cache
archive hashes matched app `5110045e…` and DNS `bfd65f5f…`; actual installed ELF
hashes matched the previously inspected e1d320…/1855ed… pair. Final native
ownership was true, actual/desired Disconnected, transition absent, stored startup
Off, empty profiles, broker FD0 and no managed TUN. The backup was retained and
guest idle preference restored. No physical PC, other VPN, shared stock core,
main/tag/assets, Marketplace or issue772 write was performed in this follow-up.

Separate negative: the same root block was explicitly tested against the newly
running, disconnected application. It refused before stop with exit1; original
application/broker PIDs and enabled broker remained unchanged, with native
Disconnected still observable. This is a known no-effect refusal, not an unknown
effect retry or evidence for connected/retained-DNS negatives.

Frozen RC1e full Test [38563158711](https://github.com/k-kostin/omavless/actions/runs/38563158711)
has now completed **successfully**. Its separate tag-triggered run was still
pending at this checkpoint. Installed ARM64 and automatic/general uninstall
remain unclaimed. Source review and this x86_64 administrative journey do not
authorize main/Marketplace promotion or establish Aryan's original failure cause.

## User-copy and short reset follow-up (#787)

Implementation source `a3d9c5d976ac85b6e275c7116ad76d6e22ae9ba4` keeps runtime,
action admission and the frozen fix.5 native packages unchanged. Ordinary EN/RU
setup, onboarding and update cards now describe state and the next action;
technical ownership/broker/enrollment explanations belong in diagnostic help.
Missing-component installation is not labelled an available update. DNS consent
still states service startup at boot; terminal guidance says finish/cancel all
operations and OS prompts before closing. The editorial rule lives in the UI
contract and development documentation policy, with bounded catalog regressions.

Normal source checks: 772 Python tests, two existing opt-in skips, normal JS/QML
contracts, 25 pure reset-validator cases, four guide-block syntax/order/hash
checks, shell syntax and diff checks PASS. Independent Astra source review
found two lost consent/order details; both were corrected and reviewed again.
Hosted full CI belongs to the final PR head, not this local result.

EN/RU synthetic rendering of real QML covered setup install/update/activation/
attention/Start/DNS-stop/missing-pair/mismatched-pair and optional-update cards,
ready component review and terminal acknowledgement; onboarding ready/missing/
permissions/unknown/update cases retained their Continue admission. Later exact
a3 captures rechecked changed navigation labels, the missing-component heading
and both wizard terminal states. Captures were inspected and remain private.
These fixtures do not install packages or establish live connection health.

The new terminal-only reset helper is frozen at
`e30162acd24d91c85462b83dc37bc0209525a6ea`, SHA256
`7366d6f46adc545208f6ebe4d6fbe0db7225ab9c5bf04ddb6bd6748e29e38854`.
It was anonymously downloaded and verified. The previous seven-block guide at
1aae remains historical evidence; it is not the current four-block recipe.
The new helper earned its own x86_64 sole-operator VM run, first on the known
fix.5 stopped fixture, then on original public 0.9.8 bytes (4915 producer,
app2194/broker8cab). The original partial fixture was separately seeded with
initial settings, completed DNS registration/selection, app inactive and no TUN;
this is not proof of Aryan's exact initial failure.

Observed user path: helper prepare/ordinary widget confirmation → confirmed
idle broker stop → one guest reboot → guard/revoke/pacman removal → deletion of
only the three canonical OmaVLESS data directories, **without a backup** →
ordinary repository add (Yes clone, No immediate enable) → exact fix.5 checkout
and enable → actual panel Install → public cache-miss download of both pinned
archives → INSTALL/sudo/pacman → separate DNS consent → ready → terminal close,
component review/closure acknowledgement/Core ready → normal Disconnected panel
with import controls. No admin intervention, socket unlink or guard bypass was
inserted inside this path. Historical private fixtures elsewhere were retained.

Separately, a3 development frontend was installed over the completed public
fix.5 installation. File hashes matched but the existing shell initially showed
old QML; a supported **guest-only** shell refresh was required. The newly loaded
real wizard showed short Core ready/version/Continue copy; a pointer Continue
advanced to Routing without selecting a preset or changing runtime PID. Source
hashes alone are therefore not installed-screen evidence. This subsequent dev
frontend review is not a new immutable public fix.5 artifact.

Limits: narrowly known, never-connected partial setup with this account's DNS
registration; unknown state refuses, concurrent state changes are excluded.
This is not the planned all-stage 0.9.9 uninstaller, a live-VPN reset, actual ARM64
installation, main merge, release replacement or permission to post issue772.

### DNS-helper introduction and route-copy follow-up

Source `eaeef7c0e46ec57cdf0a5d80963769e9b20ee05a` adds a short definition before DNS-helper actions in the
component card, wizard and standalone setup terminal. The companion-package
label distinguishes its bundled VPN core from the helper service. First-use
setup/wizard recommends turning off other VPN connections; completed onboarding
re-entry does not repeat that advice. It never stops foreign VPNs.
Operational setup explanations now survive an optional delivery-update notice;
Start and Restore access use separate full-width controls. The profile-import
step is named VPN profile, not First connection. Consent words and admission
stay unchanged; no Rust/native-package source or frozen public pins changed.

Independent Astra source/map review found the lost operational reason and the
misnamed step; both are corrected. Normal checks PASS: 773 Python tests, two
existing opt-in skips, ordinary JS/QML/plugin checks, diff and shell syntax.
Real-QML synthetic EN/RU review covered all produced setup statuses, constrained
stopped-helper/update and terminal-return cards, initial/revisited wizard and
missing/unknown/update core states. Controls remain reachable without clipped
labels. Nested import paths are source-mapped, not newly live-import accepted.

The development frontend was installed over the unchanged fix.5 pair in the
x86_64 VM. Supported guest-only shell refresh was again necessary to discard
cached QML. The ordinary loaded wizard then showed the new helper definition,
first-use advice, Core ready and enabled Continue; installed frontend hashes
matched source. Native application/broker PIDs remained unchanged and cached
native diagnostics stayed Disconnected with no owned core/TUN, no pending or
unknown action. No VPN toggle, package/DNS authorization, guest reboot, physical
PC mutation, main/release/Marketplace promotion or issue reply was performed.
This is editorial/rendered review, not a repeated clean package installation.

### Setup hierarchy and onboarding navigation

Product source `df1b3f28fb825fb0a6c2281fd3078fb117efa73b` makes the required
remaining setup step precede a compatible optional update. Its explanation
also survives unavailable-helper, runtime-stop and attention states. Ordinary
helper Start is primary; Restore access is a secondary disclosed choice with
the clean-reinstall prerequisite. Unfolding sends no setup request. Existing
pending/unknown, terminal-closure and fresh-facts admission remain unchanged.
Primary controls use the existing Omarchy accent; Back/Close are left and the
forward action right, with responsive stacking for long EN/RU labels. Web-link
labels consistently end in ↗; local navigation/install/URL-reveal do not.
Empty-profile completion is named Finish without a profile, and its guidance
does not claim a successful connection. Routing-save progress distinguishes an
unknown result from an operation still in progress.

Independent Astra review exposed two real issues: a stale update could still
hide a non-operational blocker, and routing result publication could advance
from an old matching preset before a rejection had been published. Both are
fixed. The routing regression executes the actual Service exit handler with
the Panel pending-notification handler queued as QML does; rejection wins over
an old matching snapshot and unknown outcome retains its tracker. Astra's final
exact-df source/test review found no remaining actionable finding; it is not an
independent installed or network check.

Normal local checks PASS: 773 Python tests, two existing opt-in skips, ordinary
JS/QML/plugin checks. Focused results: 28 onboarding and 23 setup contracts,
i18n and diff checks. Harness-only
`6995d7f72afa6e767a3334733c5453a842078432` adds deterministic unknown-result
rendering; its product files are byte-identical to df. Hosted CI belongs to the
actual final PR head and must be recorded separately.

Inspected EN/RU synthetic real-QML captures cover all fourteen setup states,
current/stale blocker contrasts, constrained 320/576-high helper/terminal
returns, disclosed recovery, wizard initial/revisited core and negative core
states, pending/unknown routing and empty/added-profile completion. Unknown
routing refuses forward activation and has no false saving message. Clicking
Finish in both synthetic profile states emitted the fixture's finish signal,
not a private-store mutation. Shared-card-only recovery captures are labelled
as such, not represented as a full setup page. Captures stay local/outside Git.

The ordinary development frontend was installed over the unchanged fix.5 pair
in the sole-operator x86_64 VM. Old cached QML was visibly rejected as evidence;
the supported guest-only shell refresh loaded the new distinctive layout.
Actual pointer navigation Core Continue → Routing Skip → VPN profile → Back →
Back → Close passed without choosing a preset or importing a profile. Tab and
Shift+Tab showed keyboard-focus styling; no privileged control was activated.
The native application PID stayed unchanged, desired VPN remained Off,
onboardingComplete false, profiles empty and stored startup Off. No packages,
DNS consent, VPN toggle, guest reboot or host desktop/network changed. All
isolated review windows were closed and VM custody returned to the paused VM
master. This delta does not repeat clean-install, ARM64 or connection acceptance.

The local reviewer canvas retains existing screen codes and adds six named
variants; browser QA rendered all113 codes on desktop/mobile with no JS errors
or external requests, including notes/export, links, image enlargement and
static-server isolation. Old source/canvas snapshots and notes are preserved.
It is developer review material, not a main/RC screen catalog or user guide.
Main, release assets, Marketplace and issue772 remain unchanged.

### Update-entry naming delta

Owner-directed source `1551d7d7d56ca5730683285eb48291d5c75e5aae` names the
paired-update entry Update components… / Обновить компоненты… instead of
Prepare update. It opens the same staged update flow, not immediate package
replacement. The shell's return instruction and candidate native-install guide
name that exact control; PREPARE/UPGRADE and all action fences are unchanged.
Focused setup24/onboarding28/i18n checks,67 installer tests, shell syntax and
diff checks pass. Inspected isolated real-QML EN/RU shared-card captures for
ready+stale, runtime-start+stale, stopped-helper+stale, runtime-stop+stale,
explicit pair-update and missing-pair at320-high; labels fit, missing-pair keeps
its distinct installation wording, and actionRequests stayed0. No installer,
shell restart, native package, VPN or real store mutation was invoked. This
narrow label review is not new installed update or independent Astra acceptance.
The local canvas retains prior render identities rather than silently attributing
old screenshots to the renamed source. Public/main promotion is still separate.

### Terminal journey, vocabulary and editorial checkpoint

Implementation source `c034f9eceb53ff9e94c61b00150047a1e6cc6205` follows the
owner's requested order: terminal review/correction, shared glossary, then a
bounded retrospective. The subsequent documentation-only glossary correction
distinguishes panel close from sustained plugin disable/remove, which requests
fenced native shutdown; it does not claim that disappearance proves cleanup.

- Native PREPARE, first UPGRADE and post-preparation UPGRADE use a pure EN/RU
  formatter. Consequences, exact consent words and Enter cancellation remain
  explicit. Preparation may safely stop the helper; later cancellation keeps
  old packages and does not restart it. UID permission and service startup now
  and at computer boot remain explicit in the shell's separate DNS consent.
  The controller passes its bounded selected locale to the extracted helper.
  Privileged operations, package admission, structured results, diagnostic
  codes and system/pacman/password prompts are not simplified or suppressed.
- The stopped-update path proceeds in the same terminal, without another
  reopen/update loop. Success/failure instructions name closure acknowledgement,
  Check again and the applicable helper/app action. Setup completion describes
  what this setup did, not an unobserved global VPN-Off state. Restore access is
  not a fallback after failed Start. First-time DNS and existing access remain
  distinct; no new retry or reset authority follows from the copy change.
- `GLOSSARY.md` is the shared component/action vocabulary, discovered through
  AGENTS and the UI/localization policies. A cross-surface test compares its
  action table and terminal references to the real EN/RU catalog. It is not a
  duplicate installer or a requirement to rename machine/protocol identifiers.

Local normal checks PASS: 774 Python tests with two existing opt-in skips,
ordinary JS/QML/plugin checks including25 setup contracts; owning updater Rust
checks48 actual tests, zero failures/ignores. Product Backup+setup-upgrade082
all-target strict clippy, formatter, shell syntax and diff checks PASS. These
are source/inert fixture gates, not another clean-install or ARM64 execution.
Hosted full Rust/parity CI belongs to the final PR head and remains separate.

Primary-agent visual review in the sole-operator x86_64 guest inspected all
three compiled native prompts in EN/RU, plus shell DNS choice, DNS consent,
runtime-start return and initial cancellation. The preview reused production
formatting/functions, visibly identified itself as read-only and never entered
a package/service operation. The native PID and desiredOff, incomplete
onboarding, empty profiles and stored startupOff remained unchanged. Preview
windows closed and VM custody released; no sudo, DNS/VPN change, reboot or
physical-host UI/network mutation. Captures remain private/local. The canvas
keeps stable codes and original QML image identities; terminal extraction now
uses owning functions/markers rather than brittle line offsets.

Independent Astra source review closed the false global-Off claim and found
the mistaken EN Help reference and glossary disable statement; both are
corrected. Remaining reviewed helper/core, Start/Connect, access/Restore and
consent distinctions have no actionable finding. This is independent source
review, not independent installed/visual acceptance.

Retrospective cause/change/check: catalog-only review missed a separately
delivered native sink, remembered labels drifted, and local line-number excerpts
could silently attach the wrong prose to a phase. The existing UI contract and
execution policy now require one cross-owner editorial journey, actual-label
checks, operation-scoped outcomes and hash-bound function/marker extraction.
The new glossary/catalog/terminal guard and production-formatter preview
demonstrate those controls on this change. No new permanent reviewer or skill
duplicates the existing procedure.

**Delivery boundary:** the public fix.5 archives do not contain these new Rust
prompt bytes. Source review and compiled preview cannot earn their delivery.
Before promotion, build/pin the new matching native packages, select the helper
from that archive and verify the affected installed terminal path. Existing
fix.5 installation evidence is retained history, not acceptance for new bytes.
No main merge, release/asset publication, Marketplace submission or issue772
reply is performed or authorized by this checkpoint.

### Matched prompt packages and installed continuation

Native package source: `d4d4310be6ade7e7e166f8b672cf90c7ad3e5b3b`.
Installed frontend: `503a24c2a9abfba260eed0ff4e681f2ccb4f6016`.
The later guide/report edits do not change these product bytes. The proposed
delivery is fix.6, **unpublished**; main and issue772 are unchanged.

Both architecture app archives were built by Native packages run38622809462;
both production DNS archives by run38622800411. Their receipts name the same
native source. Strict offline inspection checked archive members, architecture,
every DNS payload hash and committed guard/unit/hook/scriptlet inputs; it never
executed ARM64. The frontend pins both exact archives and the app/core/broker/
receipt tuple in the existing maps. Offline assembly verifies unchanged protected
runtime inputs between package and frontend sources. Checksums are not signatures.

| Architecture | App archive SHA-256 | DNS archive SHA-256 |
| --- | --- | --- |
| x86_64 | `5ee20258ab9eaabd572f37c0261b30ab12dd01371471c048ed0f866aec15adff` | `4f88dfb8c20a3b2b620f9270afb22dba58828bcfb597a923d5c98389b0f6dd62` |
| aarch64 | `29e2c1a73e8a44f424b8d920603e236d493ae421cbd6fcbb982d4dcdc5c9ea2d` | `b63a12125d3c6517ab1e82b02d4046b1159c898287d941f2ff94d0bf04cf8598` |

The latest public fix.5 producer was missing from the old predecessor inventory.
Independently checked public archives/GitHub asset digests earn the exact99c
architecture tuples, not arbitrary installed receipt hashes. Production broker
inputs between fix.4 and fix.5 are unchanged; additions there are test-only.
The fixed preparation gate admits the exact fix.5 Idle-gate broker bytes while
still refusing original brokers without that gate, wrong architecture and local
rebuilds. Astra reviewed the bounded native delta without an actionable finding.

The first actual frontend update entry then exposed a second omission: its
advisory source case still excluded99c although table lookup passed. No package
effect occurred. The full-inspector positive test for every recorded producer
failed on99c before correction, passes afterwards and also refuses modified
payloads. Its real public-pin negative control remains. Astra reviewed that
narrow advisory delta; root/ownership/effect admission is unchanged.

Sole-operator installed x86_64 Omarchy guest, owner-authorized agent input:

| Journey | Actual entry, result and return |
| --- | --- |
| Public fix.5 update | Settings Quit stopped the native owner and disabled the widget. After candidate frontend selection/re-enable and supported guest-only shell refresh, Update components… opened the archive's actual helper. PREPARE cleanly stopped the original idle broker. Cancelling UPGRADE preserved old packages and exposed Start DNS helper/update again. A separately admitted new update completed normal sudo/pacman replacement. Stored profiles/settings/template/selector and desired-state bytes stayed identical. After terminal closure acknowledgement, Start DNS helper and Start OmaVLESS returned to a healthy disconnected panel; the redundant update offer disappeared. |
| Fresh setup | Reviewed normal never-connected removal preparation confirmed a clean stop before guest reboot; revocation/package removal and deletion of only the three application directories followed. Ordinary plugin add was confirmed, enable declined, exact candidate selected, then enabled. Install required components → INSTALL → normal sudo/pacman → separate DNS consent completed continuously to OmaVLESS is ready. No administrative intervention occurred inside this admitted attempt. The wizard refused Continue until component setup acknowledged terminal closure, then Core ready → Back to panel reached usable profile controls. |
| Partial setup | A second admitted empty guest baseline followed the same ordinary entry and package transaction. Enter cancelled DNS consent before enrollment/start; packages remained, app/helper PID0, and Set up DNS helper was offered rather than reinstall. That action showed separate FIRST/START explanations. FIRST → separate DNS/sudo completed activation, then component-setup closure acknowledgement and Back to panel returned to Disconnected. No package replay, private reset or lock/receipt deletion occurred inside continuation. |

Candidate checkout and checked offline-cache seeding are fixture preparation,
not a main/Marketplace or empty-cache public-download PASS. The first fresh
attempt refused before sudo because the operator had seeded a cache parent
with0755 rather than0700. No packages/private activation occurred; the original
refusal is retained. Only those operator-created cache directories were corrected
before the new continuous attempt. An SSH reboot request separately refused
interactive authorization; an attended guest-only sudo reboot then succeeded.
Actual encryption prompts and fresh boot IDs were observed; QEMU/window identity
was retained. Neither event earns product recovery.

Observed EN prompts include PREPARE, post-preparation UPGRADE, initial INSTALL,
DNS consent with account UID/service startup, FIRST/START choice, cancellation
and helper/app return instructions. Existing EN/RU production-formatter previews
remain separate: this run does not claim a new installed public0.8.2 upgrade,
installed ARM64, every START/custom-template branch or a live VPN/network test.

Final guest: selected managed pair, active native application and DNS helper,
FD-store0, VPN desiredOff, no Meta TUN, empty profiles, stored VPN autoconnectOff,
incomplete optional onboarding. Normal idle mode restored; owned setup terminals
closed and panel closed. Physical-host desktop, packages and VPNs were untouched.
Private captures and complete matched archives stay outside Git. Formal human-
attended acceptance is not inferred from agent-entered VM acknowledgements.

Local normal checks:775 Python tests/two existing opt-in skips plus ordinary
JS/QML PASS; focused inventory/updater/frontend94 PASS; native updater50 PASS,
strict product+setup feature Clippy/fmt PASS. Initial fixed-URL test expectations
still named fix.5 and failed twice after new pins; corrected expectations and the
full normal rerun pass. Hosted full Rust/parity belongs to its actual run/head,
not these local results; the owning PR records completion separately.

Publication still requires its separate owner authorization, immutable matching
assets and a real public-download gate. No release/tag/assets, main merge,
Marketplace action or issue772 reply is earned by this unpublished VM evidence.

### Hosted timing-fixture isolation

Full native-source Test38622809493 completed successfully on d4d4310b.
Documentation-head Test38631674230 instead failed at the existing exact-proof
drain fixture's whole-disconnect250ms assertion, followed by22 secondary
poisoned-fixture-mutex refusals. That failed run is retained as failed.
Its measured wall time includes successful disconnect/filesystem/scheduler work,
not just the production50ms proof-drain policy. Astra independently confirmed
the distinction; no production budget, poison handling or assertion was changed.

Following the existing inherited-flock fixture pattern, the Rust runner excludes
this one fully named test from the two mass-parallel runs and executes it once
separately in each original default/feature-union configuration with `--exact`
and `--test-threads=1`. All held/released/partial cases remain enabled. An ordinary
runner guard protects both exclusions and both explicit executions. Each actual
serial local command reported1 passed,0 failed/ignored, not a zero-test PASS.
This reduces unrelated process contention; it does not make OS wall-clock timing
deterministic or hide the original failure. Runtime/package inputs and installed
product bytes are unchanged, so it neither earns new VM evidence nor requires
a different native producer. The PR records the fresh hosted result separately.
