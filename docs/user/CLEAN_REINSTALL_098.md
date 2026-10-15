# Remove OmaVLESS and start again

The terminal helper removes the OmaVLESS widget, application, bundled VPN core
and DNS helper. **Its saved profiles and settings are deleted, without a backup.**
Other VPN clients and a separately installed Mihomo stay installed.

It supports public 0.8.2 and 0.9.8 installations, including unfinished setup
and an already removed widget. A running 0.9.8 application can confirm its own
clean shutdown, including disconnection. For 0.8.2 or an already stopped app,
this reset is limited to an installation that never connected a VPN.
Experimental builds, failed recovery and unknown network state need help,
not forced deletion. For an update that keeps settings, use the [update guide](NATIVE_INSTALL.md#updates).

## Run the removal helper

Copy this whole block into your normal Omarchy terminal. It creates a small
download folder, downloads the pinned helper, checks it and starts removal.

```bash
mkdir -p "$HOME/.cache/omavless-maintenance/reinstall-098" &&
curl --fail --location --proto '=https' --tlsv1.2 \
  https://raw.githubusercontent.com/k-kostin/omavless/7c65847156756b25f1c0097f713e75535383d59e/tools/clean-reinstall-098.sh \
  -o "$HOME/.cache/omavless-maintenance/reinstall-098/clean-reinstall-098.sh" &&
printf '%s  %s\n' 2f1ffd56e49ad0d8c174d5e5af84006564d5ef32d9a3297c7e13097aa40417ba \
  "$HOME/.cache/omavless-maintenance/reinstall-098/clean-reinstall-098.sh" | sha256sum --check - &&
bash "$HOME/.cache/omavless-maintenance/reinstall-098/clean-reinstall-098.sh"
```

1. Enter `REMOVE` to confirm. The helper closes OmaVLESS if needed and prepares
   its DNS helper for removal. Enter your password only at sudo's prompt.
2. When preparation finishes, save your other work. Enter `REBOOT` to restart,
   or press Enter to restart later from your desktop.
3. After logging back in, run the same helper command below. It continues at
   removal, not preparation. Confirm `REMOVE` and the normal package-removal prompt.

Do not reopen or reinstall OmaVLESS between preparation and completing removal.

```bash
bash "$HOME/.cache/omavless-maintenance/reinstall-098/clean-reinstall-098.sh"
```

Wait for **Old OmaVLESS components and settings removed**. If a step fails,
stop and send us the step and error, without VPN keys or subscription links.
Do not delete system files or repeat an unconfirmed step.

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
