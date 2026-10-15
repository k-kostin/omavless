# Start again after an incomplete 0.9.8 installation

For an installation that stopped during setup and **never connected a VPN**.
Use the corrected `v0.9.8-fix.6` installation below. For normal setup or an
update that keeps your settings, use the [main guide](NATIVE_INSTALL.md).

This removes the old OmaVLESS widget, application, DNS component and **its own
profiles and settings**, without a backup. Other VPNs and a separately installed
Mihomo are not removed. One computer restart is required.

Use the same desktop account that ran setup. Close other OmaVLESS windows.
If OmaVLESS is running, use **Settings → Shut down OmaVLESS** first.
If you see **Recovery required**, used OmaVLESS to connect, or use custom settings
directories, ask us for help instead. **If any command fails, stop and send us
the step and error.** Do not post VPN keys or subscription links.

## 1. Prepare removal

Download and check the removal helper, then run it. Enter your password only at
sudo's prompt; confirm the widget removal when asked.

```bash
mkdir -p "$HOME/.cache/ovt/reinstall-098" &&
curl --fail --location --proto '=https' --tlsv1.2 \
  https://raw.githubusercontent.com/k-kostin/omavless/e30162acd24d91c85462b83dc37bc0209525a6ea/tools/clean-reinstall-098.sh \
  -o "$HOME/.cache/ovt/reinstall-098/clean-reinstall-098.sh" &&
printf '%s  %s\n' 7366d6f46adc545208f6ebe4d6fbe0db7225ab9c5bf04ddb6bd6748e29e38854 \
  "$HOME/.cache/ovt/reinstall-098/clean-reinstall-098.sh" | sha256sum --check - &&
bash "$HOME/.cache/ovt/reinstall-098/clean-reinstall-098.sh" prepare
```

Only after **Preparation finished**, save your other work and restart:

```bash
systemctl reboot
```

## 2. Remove the old installation

After logging back in, run this command. Confirm removal of `omavless` and
`omavless-dns` in pacman. It also deletes OmaVLESS's old settings.

```bash
bash "$HOME/.cache/ovt/reinstall-098/clean-reinstall-098.sh" remove
```

## 3. Install and finish setup

Confirm **Clone and add this plugin?** with **Yes**, then answer **No** to
**Enable 'kdk.omavless' now?**. The remaining commands select the fixed version
before enabling it:

```bash
omarchy plugin add https://github.com/k-kostin/omavless.git &&
git -C "$HOME/.config/omarchy/plugins/kdk.omavless" checkout --detach v0.9.8-fix.6 &&
test "$(git -C "$HOME/.config/omarchy/plugins/kdk.omavless" describe --tags --exact-match)" = v0.9.8-fix.6 &&
omarchy plugin enable kdk.omavless right
```

Open OmaVLESS from the bar:

1. Choose **Install required components**. Enter `INSTALL`, then approve package installation.
2. Enter `DNS` when asked to finish DNS setup.
3. Close the finished terminal. Confirm **Terminal and system prompts are closed**
   and choose **Check again** if shown. In onboarding, use **Component setup**
   to reach that confirmation; after **Core ready**, choose **Back to panel**.

You should now see **Disconnected** and the profile-import buttons. Add your VPN
profile and connect when ready. Setup itself does not connect a VPN.
