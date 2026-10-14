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
