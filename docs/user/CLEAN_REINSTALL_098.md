# Remove OmaVLESS and start again

The terminal helper removes the OmaVLESS widget, application, bundled VPN core
and DNS helper, which manages website address lookup for its VPN connections.
**Its saved profiles and settings are deleted, without a backup.**
Other VPN clients and a separately installed Mihomo stay installed.
This is a terminal tool; the 0.9.8 plugin has no full-removal button.

It supports public 0.8.2 and 0.9.8 installations: unfinished setup, a running
or stopped application, and an already removed widget. It checks the installed
components itself; you do not need to remember which version you used.
Removal normally needs one restart; widget-only cases can skip it.
If interrupted, run the same command to continue.
For an update that keeps settings, use the [update guide](NATIVE_INSTALL.md#updates).

## Run the removal helper

Copy this whole block into your normal Omarchy terminal. It uses the helper
from the widget offline, or downloads it if absent. A mismatched local copy
stops at the checksum check; it is not executed or silently replaced.

```bash
mkdir -p "$HOME/.cache/omavless-maintenance/reinstall-098" &&
if [[ -f "$HOME/.config/omarchy/plugins/kdk.omavless/tools/uninstall.sh" ]]; then
  cp -- "$HOME/.config/omarchy/plugins/kdk.omavless/tools/uninstall.sh" \
    "$HOME/.cache/omavless-maintenance/reinstall-098/.download"
else
  curl --fail --location --connect-timeout 10 --max-time 60 --proto '=https' --tlsv1.2 \
    https://raw.githubusercontent.com/k-kostin/omavless/4345f28936f22e604db4a7ed0da25c7ed3edbfd7/tools/uninstall.sh \
    -o "$HOME/.cache/omavless-maintenance/reinstall-098/.download"
fi &&
printf '%s  %s\n' a6067299fad774eef33fe06821e5aa18245fc16dc861f775ee9b96ebc9a5cd29 \
  "$HOME/.cache/omavless-maintenance/reinstall-098/.download" | sha256sum --check - &&
mv -T -- "$HOME/.cache/omavless-maintenance/reinstall-098/.download" \
  "$HOME/.cache/omavless-maintenance/reinstall-098/uninstall.sh" &&
bash "$HOME/.cache/omavless-maintenance/reinstall-098/uninstall.sh"
```

1. Enter `REMOVE` to confirm. The helper closes OmaVLESS if needed and prepares
   removal. Enter your password only at sudo's prompt.
2. If a restart is requested, save your other work. Enter `REBOOT` to restart,
   or press Enter to restart later from your desktop.
3. After logging back in, run the same helper command below. It continues at
   removal, not preparation. Confirm `REMOVE`, then follow the package-removal
   and widget-removal prompts that remain, in that order.

Do not reopen or reinstall OmaVLESS between preparation and completing removal.

```bash
bash "$HOME/.cache/omavless-maintenance/reinstall-098/uninstall.sh"
```

Wait for **OmaVLESS and its saved profiles and settings have been removed**.
If a step stops, follow its message, then run the same command. For example,
let another package transaction finish or complete the requested restart.
The helper leaves modified development builds, another account's setup and
unrelated network settings untouched. It explains what prevents removal;
do not delete system files or network rules yourself to bypass a check.

## Install again, if wanted

```bash
omarchy plugin add https://github.com/k-kostin/omavless.git --enable
```

Open OmaVLESS from the bar. Choose **Install required components**, enter
`INSTALL`, approve installation, then enter `DNS` when asked. The DNS helper
manages website address lookup for OmaVLESS connections.

Close the finished terminal. Confirm **Terminal and system prompts are closed**
and choose **Check again** if shown. Follow onboarding to import your profile.
Setup itself does not connect a VPN.

If old controls remain visible after all prompts are finished, run
`omarchy restart shell`, then reopen OmaVLESS. This briefly reloads the bar,
panels and other shell plugins.
