# Install OmaVLESS

This guide covers normal Omarchy installation, Mihomo readiness, optional
desktop helpers, updates and development installs.

## Requirements

- Omarchy 4.x;
- Python 3, included by Omarchy;
- a trusted Mihomo binary;
- `wl-clipboard` for clipboard import;
- Omarchy's GTK4 file dialog, or optionally `zenity`, `kdialog` or `yad` for
  file import;
- optional `qrencode` for QR display.

Mihoro is not required. If it is installed, OmaVLESS can discover its
`~/.local/bin/mihomo` binary, but the two applications must not run competing
full-tunnel services at the same time.

## Add the plugin

```bash
omarchy plugin add https://github.com/k-kostin/omavless --enable
```

The Omarchy command clones and validates the repository, then enables its bar
widget. It does not run `install.sh`, install packages, invoke `sudo`, grant
capabilities or start a tunnel.

## Install Mihomo

The simplest Omarchy-native option is the current `mihomo-bin` AUR package:

```bash
omarchy pkg aur add mihomo-bin
mihomo_bin="$(command -v mihomo)"
"$mihomo_bin" -v
```

Review any AUR package before installing it if you do not already trust it.
OmaVLESS also accepts a verified Mihomo binary at `~/.local/bin/mihomo`,
anywhere on `PATH`, or at the absolute `OMAVLESS_MIHOMO` path inherited by
Omarchy Shell.

## Grant TUN capabilities

Mihomo needs Linux capabilities to create the TUN interface and manage routes.
After verifying `mihomo_bin`, run:

```bash
sudo setcap cap_net_admin,cap_net_raw,cap_net_bind_service=+ep "$mihomo_bin"
getcap "$mihomo_bin"
```

The result should list `cap_net_admin`, `cap_net_raw` and
`cap_net_bind_service`. A package/core update can replace the binary and clear
these capabilities; repeat the commands if TUN startup fails afterward.

This is an explicit administrator action. OmaVLESS never runs it for you and
does not create passwordless policy or privileged runtime helpers.

## File picker and QR helpers

Current Omarchy provides a GTK4 file chooser used by OmaVLESS. The plugin also
discovers `zenity`, `kdialog` and `yad` in deterministic order. On an unusually
minimal installation, add the lightweight fallback with:

```bash
omarchy pkg add zenity
```

Onboarding and Settings report picker readiness before file import. Clipboard
import does not depend on a file picker. Install `qrencode` only if QR display
is wanted.

## First launch

Open OmaVLESS from the bar. The first-run guide:

1. checks Mihomo and TUN readiness;
2. offers a Russia, China or Iran Routing policy, or a skip;
3. opens the private import flow for the first profile or subscription.

Before storing a profile, import review shows bounded connection facts while
hiding the complete credential and key material.

New installations do not connect automatically. Login autoconnect can be
enabled later in Settings for the last-used or one fixed profile in Full VPN
or Routing mode.

## Existing Mihoro service

OmaVLESS refuses to connect while Mihoro's `mihomo.service` is active because
two full-route TUN cores must not compete for policy routing. Stop or disable
Mihoro only when you intentionally want to use OmaVLESS:

```bash
systemctl --user disable --now mihomo.service
```

OmaVLESS never stops or reconfigures Mihoro automatically.

## Updates

Update through Omarchy:

```bash
omarchy plugin update kdk.omavless
```

Omarchy shows the Git diff before updating. Private profiles, subscriptions and
routing configuration live outside the plugin checkout and are preserved.

## Development install

From a trusted exact checkout:

```bash
./install.sh
```

The script validates the checkout and atomically stages the plugin under
`~/.config/omarchy/plugins/kdk.omavless`. A fresh install is enabled in the
right bar section; an update preserves the existing enabled state and bar
position. It does not restart the shell or start a tunnel.

## Removal

Normal removal stops the plugin-owned tunnel and services while preserving
private profiles for a possible reinstall:

```bash
omarchy plugin remove kdk.omavless
```

To intentionally delete profiles, subscription URLs and routing data as well,
run the bundled cleanup before removal:

```bash
~/.config/omarchy/plugins/kdk.omavless/uninstall.sh --purge
omarchy plugin remove kdk.omavless
```

Neither path removes Mihomo or touches Mihoro data.
