# Isolated onboarding rendering

Opt-in developer harness for the actual `plugin/OnboardingWizard.qml` component
with synthetic, credential-free state. It does **not** instantiate Service,
access the private store, install dependencies, copy clipboard contents, import
profiles, authorize networking or connect a tunnel. Finish/Cancel dismiss only
this test wizard; they are not evidence of persisted native completion.

On an installed Omarchy/Quickshell display, create a private scratch directory
outside Git and make the shell imports available there:

```bash
review_dir=$(mktemp -d /tmp/omavless-onboarding-review.XXXXXX)
cp tests/onboarding-visual/shell.qml "$review_dir/shell.qml"
ln -s /usr/share/omarchy/shell/Commons "$review_dir/Commons"
ln -s /usr/share/omarchy/shell/Ui "$review_dir/Ui"
ln -s /usr/share/omarchy/shell/services "$review_dir/services"
OMAVLESS_ONBOARDING_REVIEW_DIR="$review_dir" \
OMAVLESS_ONBOARDING_REVIEW_ENTRY="file://$PWD/plugin/OnboardingWizard.qml" \
qs -p "$review_dir"
```

Run IPC calls from another terminal using that exact scratch path:

```bash
qs ipc -p "$review_dir" call onboardingReview state ru 3 missing ready
qs ipc -p "$review_dir" call onboardingReview inspect
qs ipc -p "$review_dir" call onboardingReview capture ru-import-missing
qs ipc -p "$review_dir" call onboardingReview result
qs ipc -p "$review_dir" call onboardingReview press finish
qs ipc -p "$review_dir" call onboardingReview hide
```

Wait for `result` to become `captured` before viewing the PNG. Captures contain
only the real wizard component, not the surrounding test banner. The visible
window title/banner explicitly identify synthetic testing. `press` invokes a
real enabled Button's clicked signal; it is not a hardware input test.

Matrix: locales `en`/`ru`; steps 1/core, 2/routing, 3/import;
helpers `ready`/`missing`/`clipboard`/`picker`/`unknown`; core
`ready`/`missing`/`permissions`/`unknown`. Review top/bottom scrolling with
`viewport 480` and `scroll top` / `scroll bottom`; ordinary viewport is 720.
Button inspection and disabled-action rejection supplement, but never replace,
visual review and separate installed Service/CLI/runtime acceptance.

Stop this particular `qs` invocation with Ctrl-C in its launching terminal.
Do not kill the real Omarchy shell or reset a user's store to exercise a fixture.
