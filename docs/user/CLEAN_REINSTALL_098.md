# Remove OmaVLESS and start again

The terminal helper removes the OmaVLESS widget, application, bundled VPN core
and DNS helper, which manages website address lookup for its VPN connections.
**Its saved profiles and settings are deleted, without a backup.**
Other VPN clients and a separately installed Mihomo stay installed.
This is a terminal tool; the 0.9.8 plugin has no full-removal button.

It supports public 0.8.2 and 0.9.8 installations: unfinished setup, a running
or stopped application, and an already removed widget. It checks the installed
components itself; you do not need to remember which version you used.
Removal needs one restart. If interrupted, run the same command to continue.
For an update that keeps settings, use the [update guide](NATIVE_INSTALL.md#updates).

## Run the removal helper

Copy this whole block into your normal Omarchy terminal. It uses the helper
from an updated widget offline, or downloads it for older/removed widgets.
It checks the helper before starting removal.

```bash
mkdir -p "$HOME/.cache/omavless-maintenance/reinstall-098" &&
if [[ -f "$HOME/.config/omarchy/plugins/kdk.omavless/tools/uninstall.sh" ]]; then
  cp -- "$HOME/.config/omarchy/plugins/kdk.omavless/tools/uninstall.sh" \
    "$HOME/.cache/omavless-maintenance/reinstall-098/.download"
else
  curl --fail --location --connect-timeout 10 --max-time 60 --proto '=https' --tlsv1.2 \
    https://raw.githubusercontent.com/k-kostin/omavless/5a0d996f46d27c8bc5ca29c1b03d6dfcb1ad273c/tools/uninstall.sh \
    -o "$HOME/.cache/omavless-maintenance/reinstall-098/.download"
fi &&
printf '%s  %s\n' a15e0e79163bdac8e9b24edf5dc794ceece8a68275510c51f79529a00a23c7cd \
  "$HOME/.cache/omavless-maintenance/reinstall-098/.download" | sha256sum --check - &&
mv -T -- "$HOME/.cache/omavless-maintenance/reinstall-098/.download" \
  "$HOME/.cache/omavless-maintenance/reinstall-098/uninstall.sh" &&
bash "$HOME/.cache/omavless-maintenance/reinstall-098/uninstall.sh"
```

1. Enter `REMOVE` to confirm. The helper closes OmaVLESS if needed and prepares
   removal. Enter your password only at sudo's prompt.
2. When preparation finishes, save your other work. Enter `REBOOT` to restart,
   or press Enter to restart later from your desktop.
3. After logging back in, run the same helper command below. It continues at
   removal, not preparation. Confirm `REMOVE`, Omarchy's widget-removal prompt
   if shown, and the normal package-removal prompt.

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
