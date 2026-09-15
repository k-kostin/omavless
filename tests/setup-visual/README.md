# First-run setup rendering

Opt-in rendering of the real `SetupPage.qml` with installed Omarchy imports.
Its initial status process only reads the installed ownership target; the
harness never calls Install. Do not use this as proof of package installation,
activation or marketplace availability. Status scenarios are synthetic.

Create a private `/tmp/omavless-setup-review.XXXXXX` directory with `mktemp -d`,
copy `shell.qml` there, and symlink `Commons`, `Ui`, `services` from
`/usr/share/omarchy/shell` into it. Launch this separate Quickshell instance:

```sh
OMAVLESS_SETUP_REVIEW_DIR="$review_dir" \
OMAVLESS_SETUP_REVIEW_ENTRY="file://$PWD/plugin/SetupPage.qml" \
qs -p "$review_dir"
```

After its initial probe settles, use its exact path (not the real shell):

```sh
qs ipc -p "$review_dir" call setupReview scenario ru needs_package missing false
qs ipc -p "$review_dir" call setupReview inspect
qs ipc -p "$review_dir" call setupReview capture ru-package
qs ipc -p "$review_dir" call setupReview result
```

Wait for `captured` and inspect the PNG. Matrix: EN/RU × `needs_package`,
`needs_activation`, `needs_attention`, `release_unavailable`; core `present`,
`missing`, `unknown`; also open-terminal state (`true`) with `scroll top` /
`scroll bottom`. Use `surface card` to render the exact shared component by
itself, then `scenario en ready missing false` for core-only installation and
`scenario en ready present false` to verify it disappears. `surface panel`
returns to the first-run panel. The component-only surface performs no status
probe. Use `viewport 320` / `viewport 576` to check constrained height and
`scroll top` / `scroll bottom` for overlapping captures. The card surface
cannot launch an installer. `later` on the panel invokes the actual
Set up later button and should hide only this test window. `finish` closes
only this harness. No real VPN, auth prompt, package or store should be changed.
