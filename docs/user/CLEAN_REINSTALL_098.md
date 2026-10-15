# Remove an incomplete OmaVLESS installation and start again

For a public 0.8.2 or 0.9.8 installation that **never connected a VPN**.
For an update that keeps your settings, use the [update guide](NATIVE_INSTALL.md#updates).

This removes the old OmaVLESS widget, application, DNS component and **its own
profiles and settings**, without a backup. Other VPNs and a separately installed
Mihomo are not removed. One computer restart is required.

The helper checks the installation and offers to close OmaVLESS if it is running.
It also works after the widget has been removed. Experimental builds, custom
settings locations and uncertain network state need help instead of forced deletion.
**If a step fails, stop and send us the step and error**, without VPN keys or subscription links.

## 1. Prepare removal

This command creates a small download folder, downloads and verifies the helper,
then starts it. Enter `REMOVE` to confirm the reset. If OmaVLESS is running,
enter `STOP` to let it close safely. Enter your password only at sudo's prompt.

```bash
mkdir -p "$HOME/.cache/omavless-maintenance/reinstall-098" &&
curl --fail --location --proto '=https' --tlsv1.2 \
  https://raw.githubusercontent.com/k-kostin/omavless/621502689b112760dc20b378ac2da7f3545d8e9b/tools/clean-reinstall-098.sh \
  -o "$HOME/.cache/omavless-maintenance/reinstall-098/clean-reinstall-098.sh" &&
printf '%s  %s\n' 33eabf6c5e08e03df181d555a22ea7d29e9b9d4e4f0dd463fd521dbbb0794b8a \
  "$HOME/.cache/omavless-maintenance/reinstall-098/clean-reinstall-098.sh" | sha256sum --check - &&
bash "$HOME/.cache/omavless-maintenance/reinstall-098/clean-reinstall-098.sh" prepare
```

Only after **Preparation finished**, save your other work and restart:

```bash
systemctl reboot
```

## 2. Remove the old installation

After logging back in, run this command. Enter `REMOVE`, then approve package
removal when asked. This also deletes OmaVLESS's old settings.

```bash
bash "$HOME/.cache/omavless-maintenance/reinstall-098/clean-reinstall-098.sh" remove
```

## 3. Install and finish setup

Install the current public plugin through the ordinary Omarchy command:

```bash
omarchy plugin add https://github.com/k-kostin/omavless.git --enable
```

Open OmaVLESS from the bar:

1. Choose **Install required components**. Enter `INSTALL`, then approve package installation.
2. Enter `DNS` when asked to finish DNS setup.
3. Close the finished terminal. Confirm **Terminal and system prompts are closed**
   and choose **Check again** if shown. In onboarding, use **Component setup**
   to reach that confirmation; after **Core ready**, choose **Back to panel**.

You should now see **Disconnected** and the profile-import buttons. Add your VPN
profile and connect when ready. Setup itself does not connect a VPN.

If old controls remain visible after installation, finish all prompts and run
`omarchy restart shell`, then reopen OmaVLESS. This briefly reloads the bar,
panels and other shell plugins, not the separate VPN application.
