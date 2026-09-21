# Install OmaVLESS — native candidate

The prepared source version is **0.8.1**, still pending final release acceptance
and publication. Use the [native installation and recovery guide](NATIVE_INSTALL.md)
for package installation, first-user setup or legacy migration, explicit
activation, frontend installation and updates. Python is not a runtime dependency.

## Requirements

- Omarchy 4.x for this optional QML frontend;
- the reviewed native OmaVLESS package for your architecture;
- Mihomo and its required TUN permissions;
- `wl-clipboard` for clipboard operations;
- `zenity`, `kdialog` or `yad` for file selection;
- `zenity` specifically for profile editing, `qrencode` for QR display.

Dependencies and private ownership are not created by adding the plugin.
The native path does not use the historical Python/GTK picker fallback.
The [guided first-run page](NATIVE_INSTALL.md#guided-first-run--release-preparation)
works before the native package exists and offers explicit setup using the
architecture-specific 0.8.1 package hashes shipped in the frontend. Check the
release page for published artifacts. Clean guided-install acceptance remains
pending; this is not a claim of one-command readiness.

## Package first, frontend second

Follow [the native guide](NATIVE_INSTALL.md) before adding/updating source code.
From its reviewed matching checkout or extracted frontend, use:

```sh
./install.sh
```

This is always native-only. It requires committed Rust ownership, preserves an
existing plugin's enabled state/bar position, and replaces the frontend
atomically without installing Python or the legacy uninstall script.
`--native-only` remains an accepted alias. It does not enable a tunnel, grant
privileges, build/download the package or activate ownership.

Omarchy's `plugin add` and `plugin update` clone/update source but do **not**
run this installer. Pointing an unmigrated Python installation at current main
will refuse operation, not continue through a Python fallback. Complete the
documented package and migration steps first.

## Mihomo and TUN readiness

Obtain Mihomo through the supported host package route, for example the reviewed
`mihomo-bin` AUR package on Omarchy:

```sh
omarchy pkg aur add mihomo-bin
mihomo_bin="$(command -v mihomo)"
"$mihomo_bin" -v
```

Review the package and verify the actual binary path before granting privileges.
Do not run another competing full-tunnel application alongside OmaVLESS.

## Grant TUN capabilities

After verifying `mihomo_bin`, the explicit administrator setup is:

```sh
sudo setcap cap_net_admin,cap_net_raw,cap_net_bind_service=+ep "$mihomo_bin"
getcap "$mihomo_bin"
```

A package/core update can replace the binary and clear these capabilities.
Inspect readiness if TUN startup fails afterward. OmaVLESS never grants these
capabilities or creates passwordless policy for you. Complete all normal host
authorization dialogs before another connection or service action.

## Desktop helpers

If Settings reports a missing picker/editor, explicitly install the lightweight
helper with `omarchy pkg add zenity`. Use `omarchy pkg add qrencode` for QR
display, or `omarchy pkg add wl-clipboard` for clipboard operations.
No helper is silently installed. Clipboard import does not require a picker.

## Updates and removal

Follow [native updates, Quit and removal](NATIVE_INSTALL.md#updates-close-quit-and-removal).
An application package update requires a clean disconnected state and verified
restart; changing the frontend alone does not update a running daemon.

Confirmed **Shut down OmaVLESS** in Settings performs the native shutdown.
Plugin removal is not package removal, and neither is permission to delete
private profiles. The historical `uninstall.sh --purge` is not a native remover.

## Historical Python installation

The published marketplace 0.7.0 snapshot remains
`69fe05b03129a23664fff3f8289821a7b7f80095`.
The complete pre-retirement reference and its
[historical install guide](https://github.com/k-kostin/omavless/blob/aa5873783c019edc303a732e55ea8c85f1f0b090/docs/user/INSTALL.md)
are preserved in frozen `archive/python-legacy`. Those are historical
instructions, not a second supported source installer or an automatic update route.
