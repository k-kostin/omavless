# OmaVLESS user guide

<a id="set-up-update-or-reopen-omavless"></a>

This guide is for **OmaVLESS 0.9.8 on Omarchy**. Start with the section that
matches your panel. Installing or starting the application does **not** connect
a VPN; you choose a profile and Connect separately.

OmaVLESS uses a **DNS helper** to set up website address lookup for its VPN
connections. On a normal disconnect it removes only OmaVLESS's DNS settings.
This is not a choice between Google DNS, Cloudflare or another public DNS
provider. The `omavless-dns` package contains both this helper and the matching
Mihomo VPN core; a separately installed Mihomo is left alone.

Before initial setup, turn off other VPN connections to avoid conflicts.
OmaVLESS does not turn them off for you.

[Install](#install) · [Connect and disconnect](#connect-and-disconnect) ·
[Updates](#updates) · [Remove](#remove) · [Help and FAQ](#help)

## Find your next step

| What you see | What to do |
| --- | --- |
| Required components / missing application, VPN core or DNS helper | [Install the missing components](#guided-first-run). |
| Packages installed, helper setup incomplete | [Set up DNS helper](#continue-dns-setup). Do not reinstall just because setup is incomplete. |
| Start DNS helper / Restore helper access | [Use the existing-installation helper action](#existing-dns-installations). |
| Start OmaVLESS | Start the application after any DNS step finishes. It starts disconnected and does not enable login startup. |
| Application update required | [Update the old application](#updates). Updating the plugin alone does not update native packages. |
| Update components… / Update OmaVLESS | [Update the installed components](#already-using-098). Preparation and installation ask for separate confirmation. |
| Terminal and system prompts are closed | Finish or cancel setup and all system prompts, then close the terminal, confirm this and choose Check again. |
| Setup needs attention / State unverified / Recovery required | [Inspect the current result](#when-setup-stops). Do not repeatedly install or connect to clear a warning. |

**Set up later** closes the panel without changing data or hiding missing
components. **Finish without a profile** at the final onboarding step finishes the wizard
without importing a profile. Neither starts a VPN.

<a id="guided-first-run--release-preparation"></a>
<a id="install"></a>

## Guided first run

1. Add the plugin through Omarchy:

   ```sh
   omarchy plugin add https://github.com/k-kostin/omavless.git --enable
   ```

   This installs the current plugin panel, not the native application or VPN
   core. The command does not run a separate `install.sh` or perform `sudo`.
   Install the application in the next step.

2. Open OmaVLESS from the bar. Choose **Install required components** (or the
   offered missing-component action). In the terminal, type `INSTALL` when
   asked. Approve the ordinary package transaction; enter your password only
   at the system's password prompt.

3. Type `DNS` when asked to authorize the DNS helper. This separate
   administrator step permits the helper for your account and starts its
   service now and at system startup. The VPN stays off.

4. When the terminal says **OmaVLESS is ready**, close it. Resolve any remaining
   OS prompts, press **Terminal and system prompts are closed** if shown, then
   **Check again**. If you cancelled a later step, use the remaining action
   below instead of starting over.

5. Follow onboarding to review readiness and optionally select Routing/import
   a profile. **Finish without a profile** is available without a profile. The default VPN
   autoconnect setting is Off. Fresh setup enables the application service for
   future logins, but that is not permission to auto-connect.

   At **Core ready**, choose **Continue**. This means the core is ready for
   setup, not that a VPN is connected. If something is missing or needs an
   update, open **Component installation**, **Component setup** or **Updates**
   as offered. Follow the action on that screen, then choose **Check again**.
   **Check status** only checks; it does not install or update anything.

Import your own compatible VPN profile or HTTPS subscription, choose a mode,
then Connect. Servers/subscriptions are not included. The matching
`omavless` and `omavless-dns` packages supply the application, bundled Mihomo
core and DNS helper. You do not need a separately installed Mihomo, manual
`setcap`, Python runtime or Rust toolchain for this installation.

## Connect and disconnect

Import a supported profile link or HTTPS subscription with the clipboard or
file button above the profile list. Review the proposed import before saving.
Arbitrary remote Clash/Mihomo configuration files are not supported. Keep VPN
links and subscription addresses private: they can grant access to your service.

Press **Connect beside the profile you want**, including when switching
servers. Clicking its name only selects it for the Profile actions bar; it does
not change the connection. A search filter can hide a connected profile
without disconnecting it. The connected profile is named above the controls.

The top VPN toggle is an on/off control, not a server switch: when connected,
it disconnects even if you have selected another profile. Use **Disconnect**
or the toggle to stop the VPN. Wait for the confirmed status before another
connection or mode change; finish or cancel any related OS prompt first.

| Mode | What it does |
| --- | --- |
| **Full VPN** | Sends traffic through your connected VPN profile. |
| **Routing** | Uses the selected country policy and your domain/IP rules to choose VPN or direct access. |
| **Direct** | Bypasses the proxy while keeping the TUN running; it is not Disconnect. |

**Connected** confirms the application's connection state, not that every
website is reachable or every flow uses the VPN. Check actual website access
after connecting. Ready components alone do not mean a VPN is connected.

### Profiles and subscriptions

Expand a subscription with its arrow to browse servers. **↻** beside its
server count means **Update server list**, not a ping test or reconnect.
**Update all** in Subscriptions refreshes all sources. Wait for its result;
a failed download does not mean a new server list was saved. Removing a
subscription also removes its managed profiles, as its confirmation explains.

Favorite, rename, edit, QR, export, details and delete apply to the profile
named in the management bar. Subscription profiles have editing restrictions.
Exported files and QR codes contain private VPN access; do not post them.

### Open the terminal application

Use **Open app** below Profile actions for the wider terminal interface.
It opens or focuses the same application client, not a second VPN. It does not
start a runtime you deliberately shut down. Use the panel's **Start OmaVLESS**
action first when offered.

In the terminal, **Tab / Shift+Tab** change pages, `?` shows help and `,` opens
Settings. Press `q` or close the terminal when done: the VPN and accepted
background work keep running. **Disconnect** stops the connection;
Settings → **Shut down OmaVLESS** stops the application.

On Profiles, arrows select a row, `c` connects and `d` disconnects.
`1` / `2` / `3` select Full VPN, Routing or Direct. A network action has a
separate Enter confirmation; Escape cancels it. `t` checks the selected profile
and `T` checks all available profiles.

On Subscriptions, `n` / `p` choose the source, `s` refreshes it and `S` refreshes
all; `r` only reloads the local view. `u` requests the provider's reported usage
and expiry, not measured VPN traffic or verified entitlement. On Checks and
updates, `x` requests cancellation; wait for the confirmed result before retrying.

Connections, Host, Rules, Providers, Custom rules and Route check show local or
controller facts, not proof of Internet access or complete traffic protection.
Route check uses only the query you explicitly enter and clears it on leaving.

### Encrypted Backup

In the terminal application, press `b` / F2, or choose Backup in session
Settings. Choose a new absolute filename in an existing private folder owned
by you with permissions `0700`. Enter a passphrase of at least 12 bytes twice.
Enter reviews the request; a second explicit Enter creates the file. Escape
cancels before submission. Existing files are not overwritten; the new file
has permissions `0600`.

Backup includes profiles, subscriptions and a supported bundled routing
template, not every setting or OS state. A custom template can be refused;
do not reset it to bypass that refusal. Keep the archive and passphrase safe:
a lost passphrase cannot be recovered. **Restore is not available in 0.9.8.**

<a id="continue-dns-setup"></a>

## Set up DNS helper

If the packages are installed but DNS setup is incomplete, choose
**Set up DNS helper**. It is offered only when the application and helper are
safely stopped; an unknown state is not treated as a new installation.

The terminal gives you two choices, on separate lines:

- `FIRST` — **you have not authorized the OmaVLESS DNS helper yet**. Confirm
  the separate administrator step when asked.
- `START` — **the DNS helper was already successfully set up for your account**
  on this computer. Use that setup without changing it; confirm `SELECT` when asked.

Press Enter to cancel. Do not guess `FIRST` merely because `START` failed:
a failure does not mean DNS was never configured. If you are unsure which
step completed, see [When setup stops](#when-setup-stops).

After the DNS terminal finishes, close it and settle its OS prompts. Acknowledge
closure in the panel, then Check again. Use the separate **Start OmaVLESS**
action when offered. It starts the application, not the VPN. Existing profiles,
ownership and startup preferences are preserved; this continuation does not
repeat first-use activation.

## Existing DNS installations

For an already selected managed pair, the panel may offer:

- **Start DNS helper** — restart the previously configured helper after an update or a clean stop.
- **Other options → Restore helper access** — use only if a clean reinstall
  removed this account's helper access. This is neither profile Restore nor a
  retry for a failed Start. It asks for separate DNS authorization.
- `SELECT` in the setup terminal — the existing DNS helper is ready and accessible,
  but the installed VPN core/helper package has not yet been selected.

Complete the offered DNS step and its OS prompts before **Start OmaVLESS**.
These actions do not connect, reset profiles or enable application startup.
Do not restore access just because starting the existing helper failed.

## When setup stops

A package transaction can succeed while DNS setup or application activation is
still unfinished. Typing `DNS` is consent, not proof that the step completed.

1. Finish or cancel the open terminal and any OS prompts. The failure message
   does not itself mean a password dialog is pending.
2. Keep the line **Setup stopped at stage: CODE**, if shown.
3. Return to the panel, acknowledge terminal closure and choose **Check again**.
   Follow the current remaining action, not the action from an earlier attempt.
4. If no safe continuation is offered, ask for help in the
   [project issues](https://github.com/k-kostin/omavless/issues). Include the
   panel text and stage code. Never paste VPN links, subscription URLs, private
   configuration or raw service logs.

Local diagnostic reads include `omavless plugin target`,
`omavless dns-pair status` and service status. They do not enroll DNS or connect.
Settings → Support report → **Copy report / Save file** provides the bounded
support report; private status/detail output is not automatically safe to publish.

<details>
<summary>What the stage code means</summary>

| Stage | Boundary that did not finish |
| --- | --- |
| runtime-directory / setup-lock / temporary-directory | Local setup transaction preparation. A leftover lock is not automatically disposable. |
| private-initialization / store-validation | Private data preparation/validation; existing data were not reset. |
| dns-enrollment / dns-broker-start | Administrator operation; inspect what completed before trying another action. |
| dns-broker-readiness / dns-socket-access | Required service/socket facts were not established. |
| dns-template / dns-selection | Managed template/pair selection; custom templates are not silently rewritten. |
| application-activation / runtime-enablement / completion | Activation, service enablement or final readiness was not confirmed. |
| upgrade-operation | The old-version updater refused or did not finish; preserve its reason and inspect the installed result. |

</details>

Do not delete a setup lock, ownership record, DNS journal or socket to force
progress. Reinstalling does not repair unknown ownership or authorize repeating
an enrollment that may already have succeeded.

## Updates

There are **two parts**: the Omarchy widget and the native application/DNS
packages. `omarchy plugin update kdk.omavless` updates only the widget.

`v0.9.8-fix.8` supplies the matching packages for x86_64 and ARM64.
Product/package versions remain `0.9.8` / `0.9.8-1`, so
the version label alone cannot distinguish earlier deliveries. Use the
panel's matching-pair update rather than mixing separate package downloads.

### Updating the public 0.8.2 application

Set login autoconnect Off, disconnect and fully shut down OmaVLESS first.
Update the widget with `omarchy plugin update kdk.omavless`, then re-enable
only the widget with `omarchy plugin enable kdk.omavless` and reopen its panel.
A verified compatible public 0.8.2 installation with no DNS companion offers
**Update OmaVLESS**. Keep the application stopped until updating finishes.

Confirm `UPGRADE` and the normal sudo/pacman prompts. This installs the matched
application/DNS pair while preserving profiles and the stopped application.
It is followed by separate **Set up DNS helper** and **Start OmaVLESS** steps.
Unknown/modified predecessors or custom templates may require support; do not
reset data to make them qualify.

A successful updater offers optional `RELOAD` to refresh the whole Omarchy
shell; Enter skips it. The bar and other shell plugins briefly disappear.
A reload failure does not undo the package update or mean you should repeat it.

### Already using 0.9.8

Update the widget first:

```sh
omarchy plugin update kdk.omavless
```

For a partial setup, use **Check again** and finish the remaining
DNS/application step instead of starting installation again.

When the panel offers **Update components…**:

1. Turn VPN login autoconnect Off, Disconnect, then confirm Settings →
   **Shut down OmaVLESS**. This stops OmaVLESS, not another VPN client.
2. Enable the widget again with `omarchy plugin enable kdk.omavless` and open
   it. Reopening the widget does not reconnect the VPN or start the application.
   Do not choose **Start OmaVLESS** before updating components.
3. Choose **Update components…**. Confirm `PREPARE` and the separate
   system authorization. This checks the DNS helper and safely stops it if needed; it
   does not install packages. An unknown or unsupported result needs inspection,
   not repeated confirmation or forced cleanup.
4. After preparation succeeds, the **same terminal** asks for `UPGRADE`.
   Confirm it and the normal sudo/pacman transaction. Both packages are replaced
   together. You may instead press Enter to cancel before package installation;
   this leaves the old packages unchanged and does not restart the DNS helper.
   To return later, close the terminal and resolve all OS prompts, acknowledge
   their closure, then **Check again**. Choose the offered preparation/update
   action again: preparation rechecks the now-stopped boundary before asking
   for `UPGRADE`; it does not repeat a completed helper stop.
5. After closing/acknowledging the completed update terminal, use the offered **Start
   DNS helper**, then separately **Start OmaVLESS**. Do not choose first-time setup
   if the helper is already authorized for your account. No VPN starts automatically.

Compatible older pairs remain usable while you postpone an update. The first
original `v0.9.8` broker is not supported by automatic preparation; ask for help
with its guarded removal/replacement instead of trying to force it through.

If the update refuses an unfamiliar or modified installation, keep its short
reason and use [Help](#help). Do not force a package overwrite, replace only
one package or repeat first-time DNS authorization to bypass the check.

## Verify the installation

Ready means the panel can read the native application and its managed
components; it is not proof that your server or network works. After an explicit
connection, check actual website access. The terminal application's checks use
HTTPS, not ICMP ping or a whole-system leak test; Routing or Direct may send a
test directly instead of through the VPN. Main-panel Test and latency sections
are not offered in this version.

If the runtime is healthy but the panel still displays old controls or stale
**State unverified** after a widget replacement, finish the terminal/OS prompts
and deliberately run:

```sh
omarchy restart shell
```

This refreshes the bar, panels and other shell plugins. It does not restart the
separate native VPN runtime. Do not use it as recovery for a real runtime error.
A shell reload is optional, never an automatic consequence of checking status.

## Updates, close, Quit and removal

Closing the panel/TUI is not Disconnect. **Disconnect** stops only the VPN.
Settings' confirmed **Shut down OmaVLESS** stops and verifies the native
application, disables its startup/plugin and preserves profiles/settings.

Persistently disabling/removing the widget also requests fenced native
shutdown. The disappearing icon is not proof that cleanup succeeded.
Re-enable the plugin to reopen it; after any required DNS step, the panel
offers **Start OmaVLESS**, not an automatic VPN connection.

### Reinstalling the widget is not a clean app installation

`omarchy plugin remove kdk.omavless` removes the widget, **not** the
`omavless`/`omavless-dns` packages, bundled core, system DNS broker enrollment
or private profiles/settings. Adding it again therefore resumes an existing
installation. Often the correct fix is finishing the remaining setup step.

Use [Remove](#remove) below for full removal, or the
[short removal guide](CLEAN_REINSTALL_098.md) for the same terminal steps.

## Remove

**Removal deletes OmaVLESS's saved profiles and settings, without a backup.**
Other VPN clients and a separately installed Mihomo stay installed. There is
no full-removal button in the 0.9.8 plugin.

The terminal helper supports public 0.8.2 and 0.9.8 installations, including
unfinished setup, a running or stopped application and a removed widget. You
do not need a usable panel or a separate manual shutdown step.

Copy this whole block into your normal Omarchy terminal. It uses the helper
from the widget if present, otherwise downloads it, and checks its checksum
before starting. If the local copy does not match, it stops rather than
executing that copy or silently downloading a replacement.

```bash
mkdir -p "$HOME/.cache/omavless-maintenance/reinstall-098" &&
if [[ -f "$HOME/.config/omarchy/plugins/kdk.omavless/tools/uninstall.sh" ]]; then
  cp -- "$HOME/.config/omarchy/plugins/kdk.omavless/tools/uninstall.sh" \
    "$HOME/.cache/omavless-maintenance/reinstall-098/.download"
else
  curl --fail --location --connect-timeout 10 --max-time 60 --proto '=https' --tlsv1.2 \
    https://raw.githubusercontent.com/k-kostin/omavless/0250efeb87ff5f6c331a2d067c181a29084b37a2/tools/uninstall.sh \
    -o "$HOME/.cache/omavless-maintenance/reinstall-098/.download"
fi &&
printf '%s  %s\n' 2ecda6be26438e3f6ffa46cc66c7f1c3b55ae1db6d176cf8471b4963724532f1 \
  "$HOME/.cache/omavless-maintenance/reinstall-098/.download" | sha256sum --check - &&
mv -T -- "$HOME/.cache/omavless-maintenance/reinstall-098/.download" \
  "$HOME/.cache/omavless-maintenance/reinstall-098/uninstall.sh" &&
bash "$HOME/.cache/omavless-maintenance/reinstall-098/uninstall.sh"
```

1. Enter `REMOVE` to confirm deletion. The helper closes OmaVLESS if needed
   and prepares removal. Enter your password only at sudo's prompt.
2. Normally, an installed application needs one restart. Save your other work,
   then enter `REBOOT` to restart now, or press Enter to restart later from the
   desktop. If the helper says no restart is needed, follow its remaining steps.
3. After logging back in, continue with the same cached helper:

   ```bash
   bash "$HOME/.cache/omavless-maintenance/reinstall-098/uninstall.sh"
   ```

4. Confirm `REMOVE` and the remaining package/widget-removal prompts. Packages
   are removed before the widget. Wait for **OmaVLESS and its saved profiles
   and settings have been removed**.

Do not reopen or reinstall OmaVLESS between preparation and completed removal.
If interrupted, run the same cached helper command to continue. Cancelling
widget removal can leave the packages already removed; private data and
removal progress stay until you finish. Do not delete progress records,
unmask services or clear network state yourself.

Modified development builds and another account's setup are left unchanged
with a specific explanation. If normal closing cannot be confirmed, follow
the helper's restart instructions; an unknown network state is not guessed away.

After successful removal, use [Install](#install) to install again. DNS helper
access, if revoked during removal, needs the explicit offered setup/restoration
step. Do not use the historical `uninstall.sh --purge` for native removal.

## Optional helpers, firewall and older installations

The managed DNS companion already supplies the reviewed Mihomo core. A stock
Mihomo installation or `setcap` on a different binary is not a substitute.

Clipboard uses `wl-clipboard`; file selection uses zenity/kdialog/yad; editing
uses zenity and QR display uses qrencode. Missing optional helpers are reported
rather than silently installed. For example, deliberately install a picker
with `omarchy pkg add zenity`. Clipboard import does not require a picker.

A default-deny firewall can block traffic on the TUN even when the panel says
Connected. Setup never disables or changes firewall rules. See the
[scoped firewall guidance](https://github.com/k-kostin/omavless/blob/a4da21bc579e1a5a1fb84300ce9035506cdf772e/docs/user/NATIVE_INSTALL_082_REFERENCE.md#default-deny-firewall-and-tun);
review your actual interface/address/policy rather than copying an exception
for a different host.

The [historical 0.8.2 reference](NATIVE_INSTALL_082_REFERENCE.md) retains older
stock-Mihomo, manual activation and legacy migration instructions. Those are
not the current managed-pair installation route. No Python runtime or Rust
toolchain is required for ordinary use.

## Help

### Ask your assistant

Open **Settings → Help**, or **Setup help** during installation. Choose
**Copy request**, paste it into Codex, Claude or another assistant on this
computer, then describe the problem at the end. The request contains no profiles,
keys or diagnostic data. Use **Show request** to read or select it yourself;
the text also appears if copying fails. It points to the guide installed with your plugin:

```text
Read the installed OmaVLESS assistance guide:
~/.config/omarchy/plugins/kdk.omavless/docs/user/USER_AGENT_GUIDE.md
Also read AGENTS.md in the plugin directory if present.

Help me with OmaVLESS in Omarchy. Problem:

```

The [guide for your assistant](USER_AGENT_GUIDE.md) explains how to check the
installed version, diagnose safely and ask before changes. If you removed the
widget, its local guide may be absent; give your assistant this web guide and
ask it to identify your installed release first.

If you find a bug together, your assistant can prepare an issue or a proposed
fix for the project. Review the text and any patch before agreeing to publish
them. A local workaround does not have to remain an unreported problem.

Prefer to work without an assistant? Use the sections in this guide.
You can also choose **Report a problem ↗** directly; there is no requirement to
use an assistant or read the guide before opening an issue.

### Setup is ready, but websites do not open

Add a working supported profile and explicitly connect it. Try another server
if one is unavailable. **Connected** and traffic counters do not by themselves
prove Internet access, working DNS or the complete routing policy. Review any
other active VPN and firewall policy instead of disabling security checks.

### Which actions leave my VPN running?

Closing the panel, closing the terminal application and restarting the shell
leave the connection running. **Direct** is also not Disconnect. Use
**Disconnect** for the connection, or **Shut down OmaVLESS** for the application.
Removing the widget alone is not full removal.

### Do I need to install or configure Mihomo separately?

No. Current setup installs the matching core and DNS helper together in
`omavless-dns`. Do not substitute an unrelated Mihomo or copy a configuration
from another VPN client. Your separately installed Mihomo is left alone.

### Why does pacman still show 0.9.8-1 after an update?

Maintenance deliveries can keep that package version. Use the panel's offered
update and the selected matching pair, not the displayed version alone.
Updating the widget does not replace native packages.

### Can I restore a backup or remove everything from Settings?

Neither action is available in the 0.9.8 panel. Backup creation is available in
the terminal application; Restore is not included yet. Full removal uses the
[terminal helper](#remove), which deletes profiles/settings without a backup.

### How do I ask for help safely?

Use **Settings → Support report → Copy report / Save file**, if available.
Give the action attempted and the short error/stage shown. Review screenshots
before posting: profiles, subscription URLs, keys, QR codes, backup files and raw
private logs must not be shared. Missing package/setup screens can be reported
by their displayed action and stage without a VPN configuration.

[Project issues](https://github.com/k-kostin/omavless/issues) ·
[Troubleshooting](TROUBLESHOOTING.md) · [Security and privacy](SECURITY.md) ·
[Supported profiles](PROTOCOLS.md) ·
[Guide for your assistant](USER_AGENT_GUIDE.md)
