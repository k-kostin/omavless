# OmaVLESS 0.9.8 — fix.8 frontend

These are the Omarchy panel files for **v0.9.8-fix.8**. The same frontend works
on x86-64 and ARM64; the application and DNS-helper packages must match your
architecture and this delivery.

## Recommended installation or update

Use the [user guide](docs/user/NATIVE_INSTALL.md). Normally you install through
Omarchy, without downloading this archive:

```sh
omarchy plugin add https://github.com/k-kostin/omavless.git --enable
```

For an existing plugin, run `omarchy plugin update kdk.omavless`, then follow
the next step offered in its panel. Updating only the widget does not replace
the application packages. Normal updates keep profiles and settings and do
not start a VPN connection.

## Installing this extracted archive

This is an alternative for an already set-up, compatible OmaVLESS application.
Check the release's `SHA256SUMS` before extracting it. Checksums detect changed
downloads; they are not signatures. Do not mix packages from different releases
just because their displayed version numbers match.

Run `./install.sh` from this directory as your normal desktop user. It installs
only the panel files: it does not install packages, set up the DNS helper or
connect a VPN. Use the user guide for any remaining setup or component update.

Closing the panel does not disconnect your VPN. Use **Disconnect** to stop the
connection or Settings → **Shut down OmaVLESS** to stop the application.

For full removal, follow [Remove OmaVLESS](docs/user/CLEAN_REINSTALL_098.md).
It deletes OmaVLESS profiles and settings, but keeps other VPN clients and a
separately installed Mihomo. The checked terminal helper is included in
`tools/uninstall.sh`.
