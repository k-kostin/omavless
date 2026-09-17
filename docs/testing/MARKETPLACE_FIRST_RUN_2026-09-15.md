# Marketplace first-run setup checkpoint

## September 16 release validation in progress

Owner authorized release preparation/publication and the marketplace update in
this session, with the final x86_64 Omarchy check planned later the same day.
#250–253 are merged. #249 is rebased onto their main; range-diff retains the
three setup commits, with only tests/run.sh insertion context changed.

The accepted ARM64 runtime package is retained unchanged. The x86_64 package
was actually built from the same accepted runtime source
`b7fd0a99b8b169f0933e5f43ea4389642015193a` on an x86_64 Arch Linux CI runner
([build run](https://github.com/k-kostin/omavless/actions/runs/35086544726)).
Build, CLI loader execution, native archive inspection and downloaded checksums
PASS; this is not physical-PC installation or VPN evidence.

Actual archive pins now accompany the candidate frontend:

- ARM64: `454662a76f106af5b2f4ee8ab3ef4626a1641981fd9e6baa0a2ca8d91dc7983d`.
- x86_64: `57e4599bfeb90e063951a49295ae6154703a0739c3c1accc751b3f1fd4343568`.

The [0.8.0 release](https://github.com/k-kostin/omavless/releases/tag/v0.8.0)
was published **as a prerelease, not latest**, on September 16 at 11:14:55 UTC.
Both actual packages, the shared frontend, its pairing record, x86_64 build
provenance and SHA256SUMS are attached. Anonymous HTTPS download of all six
assets and all five checksum checks PASS. No GitHub token was used to download.

Frontend source: `9411870c1c3dbf632426a895dfb6bd50c1e24dd5`.
Frontend archive SHA-256:
`e3838dc809383fc610d9f837c48e3b210fc9a82f6f3940b3fd90c8d4f42a4808`.
Offline pairing reports unchanged runtime inputs and matching ARM64 pins;
its unpublished/download flags describe assembly time, not the later download.
The x86_64 archive retains independent build evidence from the run above.

Exact frontend candidate checks: **275 Python total / 273 PASS / 2 SKIP**,
JS/QML contracts PASS, and [CI PASS](https://github.com/k-kostin/omavless/actions/runs/35087890956).
Package/source input equivalence does not prove guided activation or PC VPN use.
Real guided provisioning and the final PC gate remain pending until recorded.
The dated empty-pin findings below remain historical evidence, not the current
metadata content. #249 remains Draft until its declared installation gates pass.

## Actual ARM64 provisioning, September 16

Starting state: native 0.8.0 installed, startup Off, observed disconnected,
manualRecoveryRequired=false, Mihomo/TUN/auxiliary counts 0/0/0. Original
frontend and complete private directories were retained outside Git. A real
terminal guarded Stop and normal `pacman -R omavless` with human ready/settled
barriers; no dependency bypass. The existing Mihomo package was retained.
Private config/state were moved aside under the existing migration lock, not
edited or replaced with fabricated ownership markers.

Actual `omarchy plugin add` cloned the repository. Because #249 is not yet main,
the disabled clone was checked out at exact frontend `9411870c...` before
enabling. A shell-only restart cleared the old same-path QML cache; no core was
started for UI refresh. This is real candidate provisioning, with that explicit
checkout substitution, **not yet the unchanged default-main one-command path**.

- Required components showed only the absent OmaVLESS application; the existing
  Mihomo core was not offered for reinstallation. Installed screenshot reviewed.
- Set up later closed the panel; reopen preserved the missing-component facts.
- Actual Install opened the normal terminal. Owner confirmed cancellation before
  INSTALL with `Cancelled. No changes made`; the app remained absent. The UI
  required acknowledging terminal closure before permitting the next attempt.
- Second actual UI install downloaded the pinned public ARM64 archive, passed
  SHA-256 and normal package checks, installed 0.8.0-1 and performed canonical
  initialize/activate. No manual activation command or store repair was needed.
- Runtime enabled/active; installed ELF
  `12afa0a6ae279d23f1b89426d47fdd478e0ef1987f171a92924b9c925f6f0458`;
  observed disconnected, recovery=false, zero Mihomo/TUN/auxiliaries;
  zero profiles/subscriptions, startup configured/Off. No VPN auto-connect.
- Required components disappeared. All three real onboarding steps rendered;
  optional routing/import were skipped. Finish later persisted
  onboardingComplete=true. Normal settled reopen showed the empty main panel.

Fast-reopen finding: reopening before the post-completion status read settled
could reopen the wizard from cached onboardingComplete=false. The data was
correct; the automatic QML prompt did not wait for fresh presentation metadata.
The narrow correction defers automatic onboarding while status is refreshing,
then reevaluates on read completion. Explicit Settings review is unchanged.
Regression coverage includes stale incomplete → fresh complete, genuinely
unfinished fresh data, failed reads and explicit completed-state review.
Rust/package inputs and prior VPN evidence are unchanged. Corrected frontend
installed review and final private-state restoration are recorded separately
when complete. Published first frontend bytes are retained, not overwritten;
a corrected versioned frontend asset is required before stable promotion.

This gate does not test absent-Mihomo/AUR provisioning, physical x86_64 VPN,
or cancellation during actual privileged package effects. Those remain distinct
from the successful initial-consent cancellation and retained-core install.

## Installation instruction clarification

The September 15 documentation follow-up distinguishes first installation,
installed-but-unactivated, already-native update, postponed setup and reopening
after Quit. It separates Set up later from the wizard's persisted Finish later,
and initial consent cancellation from failure after partial installation effects.
The user guide now accepts an explicit dual-source frontend/package pairing
record, not an assumed equal SHA or a version-only match. These are documentation
clarifications against existing `SetupPage`, `RequiredComponents`,
`setup-runtime.sh` and onboarding handlers; no runtime/UI/helper code changed.
The actual public provisioning gate below remains NOT RUN.

## Owner-directed component-block refinement

The initial full setup page at `2aa5ba060c5149cbd143740446c70139f128d952`
is superseded by a normal panel shell with a bordered **Required components**
block below Profiles. Only missing OmaVLESS/Mihomo rows are shown; both missing
use one sequential action. With the app activated, the same shared component
appears under the real profile list only when needed. The accepted hero, modes,
profile rows and fixed profile-action dock remain in place.

Skipping/later closes the panel, not a persistent reminder flag. Existing
onboarding can still be deferred normally. Component presence and activation
are separate: an installed unactivated app gets **Complete setup**, never a
reinstallation offer. Unknown facts fail closed. When both programs are present
and ownership is ready the extra UI disappears. Permissions/TUN/live health are
not certified by file presence; existing onboarding/Settings retain those checks.

The read-only `components` helper adds exactly two bounded enum fields:
`status<TAB>present|missing`, optionally followed by one LF. Existing `status`
callers retain their original output. No binary path, version, private profile
data or raw error reaches this boundary. QML rejects extra fields/lines.

Fixed-purpose `install-core` handles a missing core after application setup,
without package reinstallation, store access, service restart or activation.
It shares the private installation lock and explicit terminal/AUR consent.
Existing compatible core package plus executable is a no-op; a running or
unobservable core blocks installation. A provider receipt without an executable
cannot be reported as successful installation. Connect refuses a known missing
core, while disconnect remains available even if the executable disappeared.

Refinement checks: **26 focused shell tests, 11 component contracts, 39 main-panel
tests PASS**; full local Python suite **261 total / 259 PASS / 2 SKIP**, existing
JS and QML contracts PASS. Installed runtime/crates are unchanged. The initial
checkpoint's actual published-download blocker below remains in force; this
refinement does not turn simulated package effects into live acceptance.

## Gap and chosen user path

The prior clean candidate checks started from an explicitly installed native
package. They did not prove ordinary marketplace cloning on a machine without
`/usr/bin/omavless`. Installed Omarchy's `omarchy-plugin-add` clones, validates
and enables the frontend; it has no package dependency or install-script hook.
This is a distribution/onboarding gap, not a new Rust migration claim.

User goal: install the VPN application starting from its plugin. The primary
action targets application provisioning, not a profile or VPN connection.
Navigation/help/status checks are read-only; Install opens an explicitly
confirmed terminal transaction. The existing accepted main/settings layout is
unchanged once the native owner is available.

`SetupPage.qml` and `SetupState.js` render a public bounded first-run state
without depending on backend success. The panel hides unavailable legacy
controls/shortcuts and gives this page Tab/Escape/scroll ownership. On successful
native-owner discovery it refreshes the existing Service and onboarding.
Plain-text EN/RU copy explains both missing application and unavailable release.
The first-run helper never becomes a VPN runtime or arbitrary command IPC API.

## Trust and effects

- `setup-runtime.sh status` only returns a fixed enum; `components` adds the
  independently observed core-presence enum. Raw native/parser errors
  are not passed to QML. Missing/invalid ownership is not successful activation.
- `install [en|ru]` requires a non-root real terminal, exact `INSTALL` consent
  and a same-user private runtime-directory lock. No auto-install/retry.
- Only fixed upstream `v0.8.0` package URLs for `aarch64` / `x86_64` are
  constructed. Adjacent reviewed metadata pins SHA-256 and records source SHA;
  no remote latest manifest, caller URL or executable script is trusted.
- HTTPS-only redirects, 180-second download timeout, 256 MiB limit; checksum
  before package metadata inspection or effects. SHA pins authenticate bytes
  only relative to the trusted reviewed frontend; they are not signatures or
  independent reproducible-build proof. Source SHA is provenance metadata,
  not a substitute for the package hash/build evidence.
- Normal pacman dependency checks and sudo confirmation remain. Never
  `--nodeps`, forced overwrite, downloaded shell or local Rust compilation.
- Missing package dependency `mihomo` is handled before installing OmaVLESS.
  `pacman -T mihomo` respects alternative providers. Separate `CORE` consent
  offers the existing documented Omarchy AUR command for `mihomo-bin`; the
  installed Omarchy helper invokes yay with its usual flags. This is not an
  official-repository or source-authenticity claim. No TUN capabilities are
  granted. Optional picker/editor/clipboard/QR tools remain in onboarding.
- Fresh initialization, store validation and ownership activation use existing
  fixed Rust commands. Existing stores are never reset. Active legacy owners,
  incompatible stores and incomplete transitions refuse; no marker editing.
- Already-native owners are not reinitialized, restarted or re-enabled (Quit
  semantics preserved). Recheck state after consent/lock; refuse if a package
  appeared while downloading. No tunnel or autoconnect preference change.
- Detached terminal launch is not completion. Check again is read-only; a
  deliberate retry requires the human to acknowledge terminal/auth closure.
  A lock left after SIGKILL is a recovery case, never automatically stolen.

## Initial checkpoint validation and honest boundaries

- Focused shell composition: **19 deterministic tests PASS**, synthetic effects
  only. Covers fresh/existing/invalid ownership, headless refusal, cancellation,
  malformed/symlink/unpublished pins, checksum failure, consented/missing/failed
  Mihomo dependency and failed activation without enable/retry.
- Six JS setup contracts PASS, including the legacy exit-71 presentation flag
  not being proof of native availability; all existing Python/JS/QML contracts PASS.
  Full Python suite: **254 tests, 252 PASS / 2 intentional SKIP**.
- Shell syntax, manifest/release JSON, diff check and Omarchy plugin validation
  pass. Rust sources, dependencies, lifecycle and native binary are unchanged;
  prior exact-binary acceptance is retained, not claimed as installer evidence.
- Separate Quickshell render harness uses actual installed Omarchy imports and
  the actual component at panel width; synthetic EN/RU setup/unavailable/error
  states. These are rendering checks, not real package installation. The initial
  component probe reads the existing native ownership only. No VPN switched.
- Installed `/usr/bin/omavless plugin target` under an isolated empty HOME and
  runtime directory returns `legacy`, confirming fresh discovery does not need
  existing private state. This is not host activation evidence.
- Real published download / fresh package installation / activation from this
  setup page: **NOT RUN — reviewed release assets/pins not published**.
  Do not mark this checkpoint release-ready based on synthetic tests.

## Remaining release sequence

1. Review this bounded bootstrap change; test terminal launch/cancel and exact
   installed frontend regression without resetting the owner's private store.
2. Build and accept the exact runtime packages for both supported architectures;
   retain source/ELF/archive hashes. Owner approval is required before publishing
   a tag or assets. Do not manufacture pins pointing to missing assets.
3. After authorized immutable runtime asset upload, commit real SHA/source pins
   in `plugin/runtime-release.json`. Assemble the matching frontend from that
   reviewed commit. Runtime source SHA may precede the frontend **pin-only**
   commit; explicitly prove runtime/package input identity, not an imaginary
   single recursive commit hash. The existing offline assembler's exact-source
   rules remain unchanged; plan/review this final artifact pairing explicitly.
4. From clean Omarchy profiles on ARM64 and x86_64, use the intended marketplace
   clone, real Install button, download/hash/pacman, initialization, activation
   and normal onboarding. Cover no Mihomo, existing provided Mihomo, cancellation
   before effects, failed download, double-click, later/reopen and existing owner.
5. Confirm first login starts disconnected (startup Off), package/frontend
   identity, optional-helper guidance and the existing manual VPN gate. No new
   protocol credentials needed. The owner then approves marketplace publication.

`runtime-release.json` currently has **empty pins deliberately**. Until steps
2–4 are complete, a new user gets an honest unavailable-release screen, not
working one-click installation. Existing installed-native users remain usable.
The historical published 0.7.0 snapshot and V0/#30 evidence are unchanged.
