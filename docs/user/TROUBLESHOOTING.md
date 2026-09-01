# Troubleshooting

Do not paste private profile links, subscription URLs, endpoints, UUIDs,
passwords or keys into public issue reports.

## Start with safe status

```bash
omarchy-shell kdk.omavless status
systemctl --user status omavless.service --no-pager
journalctl --user -u omavless.service -n 80 --no-pager
```

Settings can export a bounded JSON support snapshot. The same safe diagnostic
surface is available from the installed backend:

```bash
plugin="$HOME/.config/omarchy/plugins/kdk.omavless/backend.sh"
"$plugin" diagnostics
"$plugin" diagnostics-export ~/omavless-diagnostics.json
```

Review the result before sharing it. It is designed to exclude credentials,
subscription URLs and private names.

## Mihomo is unavailable

Confirm discovery and version:

```bash
command -v mihomo
mihomo -v
```

If Mihomo was installed or replaced recently, verify capabilities:

```bash
mihomo_bin="$(command -v mihomo)"
getcap "$mihomo_bin"
```

If required capabilities are absent, follow the explicit setup in
[INSTALL.md](INSTALL.md). OmaVLESS will not request administrator permission
automatically.

## Another Mihomo service is active

If the panel reports Mihoro or another full-tunnel core, inspect it before
connecting. To intentionally switch from Mihoro:

```bash
systemctl --user disable --now mihomo.service
```

OmaVLESS refuses competing policy-routing owners instead of stopping them.

## File import cannot open a chooser

Current Omarchy's GTK4 chooser should be found automatically. Settings reports
the detected picker. On an unusually minimal system:

```bash
omarchy pkg add zenity
```

Restart Omarchy Shell after changing desktop integration if it does not detect
the new picker. Clipboard import remains independent.

## Clipboard import cannot read text

Confirm `wl-paste` is available and the clipboard contains plain text:

```bash
command -v wl-paste
wl-paste --no-newline
```

Do not run the second command while screen sharing if the clipboard may contain
a private VPN link. A subscription must be one strictly validated URL; a
profile must be one supported link.

## Subscription already exists

The top-level import flow uses the same duplicate detection as the dedicated
Subscriptions page. Open Settings → Subscriptions to refresh or edit the
existing entry instead of creating another copy.

## Generated configuration is rejected

OmaVLESS validates generated config before restart and restores the prior state
on normal failure. Keep the previous private store/config intact and export the
safe diagnostic snapshot.

Do not edit generated `config.yaml` as a fix: it will be regenerated. For an
advanced custom routing template, use the explicit adoption command only with
a file you trust:

```bash
plugin="$HOME/.config/omarchy/plugins/kdk.omavless/backend.sh"
"$plugin" adopt-template -- ~/routing.yaml
```

OmaVLESS strips top-level proxies and controller listeners before using an
adopted template.

## Manual recovery required

`manual recovery required` means the attempted rollback also failed. Stop and
inspect the service rather than repeatedly reconnecting:

```bash
systemctl --user stop omavless.service
systemctl --user status omavless.service --no-pager
journalctl --user -u omavless.service -n 120 --no-pager
```

Confirm there is no owned Mihomo process or TUN before another attempt. Preserve
the safe diagnostics when reporting the failure.

## Routing tools

The backend exposes bounded equivalents of Settings actions. Destination
values use stdin so they are not placed in the process list:

```bash
plugin="$HOME/.config/omarchy/plugins/kdk.omavless/backend.sh"
printf '%s\n' 'example.com' | "$plugin" route-check
printf '%s\n' 'example.com' | "$plugin" custom-rule-add suffix direct
"$plugin" custom-rules
"$plugin" rule-providers-refresh
```

## Remove and reinstall

Normal removal preserves private data:

```bash
omarchy plugin remove kdk.omavless
```

Use `uninstall.sh --purge` only when you explicitly intend to delete profiles,
subscription URLs and routing configuration.
