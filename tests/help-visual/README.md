# Bounded help rendering

Opt-in synthetic display of the actual HelpPrompt, without Service, packages,
private state, reports or a running OmaVLESS binary. This is not installed Panel
navigation acceptance. Only an explicit Copy request click writes the displayed
public template to the clipboard; no clipboard is read by this harness.

On the designated Omarchy VM, use a private, owned scratch directory:

```bash
review_dir=$(mktemp -d "$HOME/.cache/ovt/help.XXXXXXXX")
cp tests/help-visual/shell.qml "$review_dir/shell.qml"
ln -s /usr/share/omarchy/shell/Commons "$review_dir/Commons"
ln -s /usr/share/omarchy/shell/Ui "$review_dir/Ui"
ln -s /usr/share/omarchy/shell/services "$review_dir/services"
OMAVLESS_HELP_REVIEW_DIR="$review_dir" \
OMAVLESS_HELP_REVIEW_ENTRY="file://$PWD/plugin/HelpPrompt.qml" \
qs -p "$review_dir"
```

From another terminal, address that exact owned instance:

```bash
qs ipc -p "$review_dir" call helpReview scenario en 600 help
qs ipc -p "$review_dir" call helpReview inspect
qs ipc -p "$review_dir" call helpReview capture en-help
qs ipc -p "$review_dir" call helpReview result
```

Wait for capture to return from capturing to ready before viewing the PNG.
Review EN/RU at heights600/360, scroll through every option using Tab/Shift+Tab,
activate copy with Enter, and close with Escape. The initial view hides the
request and has five keyboard targets; Show request adds the selectable text
after the toggle. Check expansion, manual selection and collapse at both
heights; collapsing must not leave a blank scrolled viewport. Copy does not
require expansion. Reopening Help resets it to the compact view.
Inspect focus and scroll, not
only the callback. The three section inputs are help/install/updates; invalid
locale/height/section inputs refuse capture rather than reusing an older frame.

For clipboard FAILURE only, prepend the absolute tests/help-visual/clipboard-fail
directory to PATH when starting a fresh instance. Its executable wl-copy exits1
without accessing a clipboard. The unchanged production command remains bounded
by /usr/bin/timeout; verify the fixture was selected. Inspect the failure message
and automatically revealed selectable request, not a copied status. The failure
must not steal focus if the user already moved to another action. Do not use this fixture to claim
actual clipboard success. Actual success needs the normal PATH and a real UI
copy followed by comparison with this known public template, not a read of a
pre-existing potentially private clipboard.

Finish only this instance with `call helpReview finish` or its launching terminal.
Do not restart the real shell, change services, remove native packages or reset
private state to exercise this runtime-independent component.
