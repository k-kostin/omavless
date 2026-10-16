# OmaVLESS

OmaVLESS is a VPN application for Omarchy. You need your own VPN profile or
subscription; VPN service is not included.

## Open and finish setup

Open OmaVLESS from the bar and follow the next step offered in the panel.
Turn off other VPN connections before setup to avoid conflicts. Setup itself
does not connect a VPN.

The `omavless` package contains the application. Its matching `omavless-dns`
package contains the Mihomo VPN core and a DNS helper, which sets up website
address lookup for OmaVLESS connections. A separate Mihomo installation is
not required and is left alone.

After setup, add your profile or subscription and use **Connect** beside the
server you want. **Open app** opens the terminal interface for the same
application, not another VPN connection.

## Updates and removal

Updating the Omarchy plugin updates the widget, not the application packages.
Open the updated widget and follow **Update components…** when it is offered.
Normal updates keep profiles and settings.

**Disconnect** stops the VPN connection. Closing the terminal window does not.
Use Settings → **Shut down OmaVLESS** to stop the application as well.

For complete removal, follow the user guide's **Remove** section. Its removal
helper also works if you already removed the widget with Omarchy's command.
Full removal deletes OmaVLESS profiles and settings without a backup. Other
VPN clients and a separately installed Mihomo are kept.

## Help

- [User guide: setup, updates, removal and FAQ](https://github.com/k-kostin/omavless/blob/main/docs/user/NATIVE_INSTALL.md)
- [Guide for an assistant helping you](https://github.com/k-kostin/omavless/blob/main/docs/user/USER_AGENT_GUIDE.md)

For support, use **Copy report** or **Save file** in Settings → Support report.
Do not share profiles, subscription URLs, private configuration or backups.
