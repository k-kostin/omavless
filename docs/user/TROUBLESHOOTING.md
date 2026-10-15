# Troubleshooting

Start with the [setup and update guide](NATIVE_INSTALL.md). It covers fresh
installation, an unfinished setup and updates from older releases.

## Setup stopped

Finish or cancel the current terminal and its system prompts, then reopen
OmaVLESS. Choose **Terminal and system prompts are closed** if shown, followed
by **Check again**. Follow the remaining action offered by the panel; do not
repeat first-time setup just because a later step failed.

The DNS helper handles website address lookup for OmaVLESS connections. It is
not a choice between public DNS providers. Use **Set up DNS helper** for first
authorization, or **Start DNS helper** when access already exists. See the
[next-step table](NATIVE_INSTALL.md#find-your-next-step).

## The widget or core looks out of date

Updating the widget and updating native packages are separate operations.
Follow [Updates](NATIVE_INSTALL.md#updates); a matching package version label
alone does not prove which build is installed.

If old controls remain after a completed update, finish all related prompts
first, then use the guide's optional shell reload. Reloading the shell does
not repair a failed installation or disconnect a VPN.

## Another VPN is active

Turn off its connection through that application's own controls before setting
up or connecting OmaVLESS. OmaVLESS does not stop, remove or reconfigure another
VPN client for you. Do not change its files or services as an OmaVLESS repair.

## File or clipboard import is unavailable

Check the optional helpers reported in Settings. The installation guide lists
the supported [file picker and clipboard tools](NATIVE_INSTALL.md#optional-helpers-firewall-and-older-installations).
Profile import accepts supported links, not arbitrary remote configurations.
Do not print clipboard contents in a shared terminal: they may contain keys.

## Connection or recovery failed

Keep the short message and attempted action. If recovery is required, do not
repeatedly reconnect, edit generated configuration or delete service/network
files to bypass the failure. Open **Settings → Support report** and choose
**Copy report** or **Save file** where available.

That report is designed for sharing; profiles, subscription URLs, passwords,
keys and raw private logs are not. Include the action and short error with
your report. A ready core alone does not prove provider connectivity.

## Remove and start again

The [full-removal helper](CLEAN_REINSTALL_098.md) deletes OmaVLESS profiles and
settings without a backup, then allows a fresh installation. It also works
when the widget was already removed. Other VPN clients and a separately
installed Mihomo stay installed.

Removing only the widget through Omarchy leaves the native packages and saved
data behind. Follow the helper's remaining step after an interruption rather
than deleting its records or package-manager locks by hand.
