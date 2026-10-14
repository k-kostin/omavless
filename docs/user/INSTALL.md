# Install OmaVLESS

For current Omarchy installations, use
[Set up, update or reopen OmaVLESS](NATIVE_INSTALL.md).
The plugin's Setup help and onboarding Installation guide both lead there.

```sh
omarchy plugin add https://github.com/k-kostin/omavless --enable
```

This installs the widget only. Open it and explicitly install the matching
application and managed DNS companion in its setup terminal. The companion
includes the reviewed Mihomo core: the old stock/AUR Mihomo and manual
capability steps below are **not** the current managed installation route.

Already installed or setup unfinished? Follow the
[next-step table](NATIVE_INSTALL.md#find-your-next-step), not a new first-use
installation. For updating, shutdown, removal and safe reinstall, use
[the current guide](NATIVE_INSTALL.md#updates-close-quit-and-removal).

## Desktop helpers

Optional helpers are reported rather than silently installed: wl-clipboard
for clipboard, zenity/kdialog/yad for file selection, zenity for editing and
qrencode for QR display. See [optional helpers](NATIVE_INSTALL.md#optional-helpers-firewall-and-older-installations).

## Grant TUN capabilities

This old anchor is retained for historical links. **Do not apply the 0.8.2
stock-Mihomo capability recipe to the managed 0.9.8 pair.** Use the
[current guide](NATIVE_INSTALL.md); older native/legacy installations have a
separate [0.8.2 installation reference](INSTALL_082_REFERENCE.md#grant-tun-capabilities).
