# Marketplace first-run setup checkpoint

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

- `setup-runtime.sh status` only returns a fixed enum. Raw native/parser errors
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

## Validation and honest boundaries

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
