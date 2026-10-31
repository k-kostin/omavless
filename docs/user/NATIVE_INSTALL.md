# OmaVLESS user guide

> Development draft for the proposed **0.9.9** snapshot. The published release
> is still covered by the [current main guide](https://github.com/k-kostin/omavless/blob/main/docs/user/NATIVE_INSTALL.md).

<a id="set-up-update-or-reopen-omavless"></a>

This guide is for **OmaVLESS 0.9.9 on Omarchy**. Installing or starting the
application does not connect a VPN. You import your own supported profile and
choose Connect separately; servers and subscriptions are not included.

The widget is the bar icon and panel. The `omavless` package supplies the native
application and terminal interface. `omavless-dns` supplies its matching Mihomo
VPN core and DNS helper. The helper manages website address lookup for OmaVLESS
connections; normal Disconnect removes only OmaVLESS's DNS settings. A separately
installed Mihomo is left alone. DNS-provider preferences are a separate setting.

Before initial setup, turn off other VPN connections. OmaVLESS does not turn
them off for you or configure their coexistence automatically.

[Install](#install) · [Connect](#connect-and-disconnect) ·
[Backup and Restore](#encrypted-backup) · [Updates](#updates) ·
[Remove](#remove) · [Help](#help)

## Find your next step

| What you see | What to do |
| --- | --- |
| Required components / missing application, VPN core or DNS helper | [Install required components](#guided-first-run). |
| Packages installed, helper setup incomplete | [Set up DNS helper](#continue-dns-setup). Do not repeat installation just because setup is unfinished. |
| Start DNS helper / Restore helper access | Use the corresponding [existing-installation action](#existing-dns-installations). |
| Start OmaVLESS | Start the application after any required DNS step. This starts disconnected and does not enable login startup. |
| Update OmaVLESS / Update components… | Follow [Updates](#updates). Updating only the widget does not replace native packages. |
| Terminal and system prompts are closed | Finish or cancel the terminal and all system prompts, close the terminal, acknowledge closure, then Check again. |
| State unverified / Setup needs attention / Recovery required | [Inspect the current result](#when-setup-stops). Do not repeatedly install, restore or connect to clear a warning. |

**Set up later** closes the panel. **Finish without a profile** finishes
onboarding without importing one. Neither starts a VPN.

<a id="guided-first-run--release-preparation"></a>
<a id="install"></a>

## Guided first run

1. Add the widget through Omarchy:

   ```sh
   omarchy plugin add https://github.com/k-kostin/omavless.git --enable
   ```

   This adds the widget, not the native application or VPN core. It does not
   run a separate `install.sh` or ask for administrator access.
2. Open OmaVLESS from the bar and choose **Install required components**.
   In the terminal, type `INSTALL`. Review the ordinary package transaction;
   enter your password only when the system's password prompt asks for it.
3. Type `DNS` when asked to permit the helper for your displayed account.
   This separate administrator step starts the helper now and at computer
   startup. It does not enable VPN autoconnect.
4. When the terminal reports readiness, finish any system prompts and close
   it. Choose **Terminal and system prompts are closed**, if shown, then
   **Check again**. If you cancelled a later step, use the remaining offered
   action instead of starting installation again.
5. Complete onboarding: review readiness, choose Routing if wanted and import
   a profile, or choose **Finish without a profile**. At **Core ready**, choose
   **Continue**. Readiness is not a connection test. Fresh setup enables the
   application at future logins, while VPN login autoconnect defaults to Off.
   Wait for the main panel after Finish. No VPN starts from finishing setup.

If offered, **Component installation**, **Component setup** and **Updates**
lead to the relevant next action. **Check status** only checks; it does not
install or update. Ordinary installation needs neither a separate Mihomo,
manual `setcap`, Python runtime nor Rust toolchain.

## Connect and disconnect

Import a supported profile link, profile file or HTTPS subscription using the
clipboard or file controls. Review the import before saving. WireGuard and
AmneziaWG configuration support is experimental and limited to the supported
subset described in [Supported profiles](PROTOCOLS.md). Arbitrary Clash/Mihomo
YAML is not accepted. Links, configuration files and subscription addresses can
grant VPN access; keep them private.

The experimental WG/AWG connection subset is single-peer IPv4 standard
WireGuard and the supported AWG3 configuration shape, including IPv4 imported
DNS. Importing or exporting dual-stack data does not enable IPv6 connections.
Variable keepalive ranges and unsupported AWG parameters refuse connection;
a scalar zero explicitly disables persistent keepalive. Multiple peers and
executable `.conf` directives are rejected. Other AWG generations are not
generally guaranteed. A successful import is not a server-compatibility test.

Press **Connect beside the profile you want**, including to switch servers.
Clicking its name selects it for the Profile actions bar without connecting.
Filtering the list does not disconnect a hidden profile. The connected profile
is also named above the controls.

The main toggle switches the connection on or off; it is not a server selector.
When connected it disconnects even if you selected a different profile. Wait
for confirmed completion before another connection or mode action, and finish
or cancel any related system prompt first.

| Mode | Meaning |
| --- | --- |
| **Full VPN** | Uses the connected VPN profile for the full-VPN routing policy. |
| **Routing** | Uses your routing preset and custom domain/IP rules to choose VPN or direct access. |
| **Direct** | Bypasses the proxy while retaining the tunnel. It is not Disconnect. |

**Connected**, readiness and counters do not prove that every site works or
every flow uses the VPN. Check actual site access after connecting.

### Profiles and subscriptions

Expand a subscription with its arrow. **↻** beside its server count means
**Update server list**, not a latency check or reconnect. **Update all** refreshes
all sources. Wait for completion; a failed fetch does not mean a new list was
saved. Removing a subscription also removes its managed profiles, as its
confirmation explains.

Favorite, rename, edit, QR, export, details and delete apply to the profile
named in the management bar. Subscription profiles retain their editing
restrictions. Exports and QR codes contain private VPN access; do not post them.

A protocol filter appears when your profiles contain more than one protocol.
It offers protocols present in your list. Changing the filter or search does
not change the connection or selected management target.

Subscription refresh defaults to manual. An opt-in Daily schedule is available
through the local CLI (`omavless --help` lists `subscription schedule`). It
refreshes server lists without connecting a VPN. Missed days are not replayed
as a series of requests. A running update must settle or be explicitly cancelled
before changing its schedule; a lost reply is not permission to submit it again.

### Routing and DNS

Use Settings for the supported routing presets and custom rules. Review and
save through the offered controls; a declined or unconfirmed save is not an
applied routing change. An arbitrary routing-template/YAML editor is not offered.

While disconnected, DNS offers **Preset default**, **Google** and **Cloudflare**.
Save applies to the next explicit connection; it neither connects nor reloads a
running connection. Preset default uses the routing preset's DNS. A custom
configuration or WireGuard profile keeps its own DNS; follow the displayed
read-only explanation instead of expecting the preset to override it.

### Test, Ping and diagnostics

The main panel offers an explicit HTTPS Test and a bounded Ping measurement.
They are different checks. Read the displayed result and age; an unavailable
or timed-out result is not a diagnosis of a bad key, provider or VPN.

The terminal's profile/subscription checks use HTTPS delay, not ICMP or a
whole-system DNS/leak test. Batch jobs show progress and accept cancellation;
wait for their confirmed outcome. Routing or Direct can send a check directly.
Connections, activity, short history, routes and bounded diagnostic hints show
observations, not complete protection or raw persistent logs. Connection closing
controls are not part of the read-only Connections page.

### Recorded traffic

Settings and the terminal Traffic page show **Recorded traffic**: one local
Received/Sent period, Off by default. Enable recording only while disconnected.
Recording then continues independently of open panel or terminal windows.

These are bytes recorded on the application's tunnel, including Direct mode.
They are separate from a provider's allowance, billing and proof of delivery.
**Some traffic may not have been recorded** marks an incomplete interval.
The app does not fill an unobserved interval with invented totals.

Saved totals survive an ordinary application restart. Disabling recording keeps
the totals; **Reset recorded traffic…** permanently discards the previous period
and needs explicit confirmation. Enable, disable and reset require a confirmed
disconnected state and never connect a VPN. For an unconfirmed result use
**Check current values** before another change. Statistics are machine-local
and are not part of profile Backup/Restore or support reports.

### Open the terminal application

Use **Open app** below Profile actions. It opens or focuses the wider terminal
client of the same application; it does not start a second VPN or a runtime you
shut down. Use **Start OmaVLESS** first when offered.

Tab / Shift+Tab change pages; `?` shows help and `,` opens session Settings.
On Profiles, arrows select a row, `c` connects, `d` disconnects and `1` / `2` /
`3` choose Full VPN, Routing or Direct. Network actions need a separate Enter
confirmation; Escape cancels. `t` checks the selected profile and `T` checks all
available profiles. On Subscriptions, `n` / `p` select the source, `s` refreshes
it and `S` refreshes all; `r` only reloads the local view. On Checks and updates,
`x` requests cancellation. Closing with `q` or closing the window leaves the
VPN and accepted background work running.

`u` on Subscriptions requests provider-reported usage and expiry. This private
claim is not measured tunnel traffic or verified entitlement. Leaving that page
discards it; it is not saved in ordinary diagnostic reports.

<a id="encrypted-backup-098-candidate"></a>

### Encrypted Backup

In terminal session Settings choose Backup, or press `b` / F2. Use a new absolute
filename in an existing private folder owned by you (permissions `0700`). Enter
a passphrase of at least 12 bytes twice. Enter reviews; a second explicit Enter
creates the archive. Escape cancels before submission. Existing files are never
overwritten; the new file has permissions `0600`.

Backup contains profiles, subscriptions and a supported bundled routing template,
not all Settings or OS state. Custom or unsupported templates may refuse;
do not reset them to bypass that refusal. Keep the archive and passphrase safe:
the archive contains reusable VPN access, and a lost passphrase is unrecoverable.

### Restore and interrupted operations

Restore is not available in 0.9.8; this section requires the matching 0.9.9 app.

In terminal Settings, `R` opens Restore when available. Choose the private
archive and enter its passphrase. First review the authenticated preview; a
separate confirmation replaces the displayed profiles/subscriptions and routing
template. It does not restore every application setting or operating-system state.

Restore requires a freshly verified disconnected, idle application. If currently
connected, the offered Disconnect has its own confirmation. After it completes,
wait for fresh disconnected status and open Restore again. There is no automatic
Disconnect followed by Restore.

Before submission, Escape cancels. After submission, wait for the original
result: closing a screen is not cancellation. An unknown result does not mean
nothing happened. Keep the archive and existing state, follow the offered
recovery guidance and ask for help if no safe continuation is available. Do not
repeat Restore, delete its records or reset the store to force progress.
Unsupported, corrupt or ambiguous state may remain blocked; Restore is not a
universal repair or a promise to recover from every power or filesystem failure.

### Preserve an incompatible original store

An incompatible stored record can prevent application startup, ordinary profile
export and encrypted Backup. With the matching 0.9.9 application installed,
`omavless store-compatibility` reports a safe classification without connecting.
Exit zero means the report was returned; check its `compatible` value.

`omavless store-preserve-original` can preserve the exact whole original
`profiles.json` before runtime startup. The copy is **unencrypted and contains
VPN access**. It is neither encrypted Backup nor a repair, and unsupported
profiles remain unsupported.

Choose an existing private folder you own, outside managed application
config/state/runtime directories. Use a new filename. Run the following command,
then enter the absolute destination at its hidden prompt. The destination is sent
through stdin; it is not placed in command arguments or shell history. This
explicitly confirms that you want an unencrypted copy containing VPN access:

```sh
bash -c '
  set -o pipefail
  IFS= read -r -s -n 4097 -p "Private destination for an unencrypted VPN-access copy: " ov_original_destination || exit 2
  printf "\n"
  [[ -n "$ov_original_destination" ]] || exit 2
  printf "%s" "$ov_original_destination" |
    jq -Rs "{destination: ., confirmUnencrypted: true}" |
    omavless store-preserve-original
'
```

The destination parent must already be private (`0700`); the new copy is `0600`.
Do not paste its contents or input into shared reports. The source, settings and
connection remain unchanged. Unsafe, oversized, changing or pending state refuses;
the command does not loosen permissions, clear locks or authorize a connection.
An existing destination is never overwritten. Exit 73 or lost output means a
copy may exist: inspect it privately and do not automatically repeat or delete it.

<a id="continue-dns-setup"></a>

## Set up DNS helper

For installed packages with incomplete DNS setup, choose **Set up DNS helper**.
It is offered only behind the required stopped/verified state; an unknown state
is not treated as a fresh installation.

The terminal offers separate choices:

- `FIRST`: helper access has not already been authorized for your account.
  Confirm the separate administrator step when asked.
- `START`: helper setup already succeeded for this account on this computer.
  Use it without repeating authorization; confirm `SELECT` when asked.

Enter cancels. Do not choose FIRST because START failed: failure does not prove
access was never configured. If unsure, [inspect what completed](#when-setup-stops).

After the terminal and system prompts finish, close and acknowledge them, then
Check again. Choose **Start OmaVLESS** separately when offered. This continuation
preserves existing profiles and startup preferences and does not connect a VPN.

## Existing DNS installations

For an existing selected installation, follow the offered action:

- **Start DNS helper** starts the configured helper after an update or clean stop.
- **Other options → Restore helper access** applies only when a completed clean
  reinstall removed access. This is not profile Restore or a retry after failed
  Start; it asks for separate DNS authorization.
- `SELECT` selects the already installed and accessible core/helper when selection
  is the remaining step.

Complete that action and its system prompts before starting OmaVLESS. Do not
repeat first-time authorization merely because starting a helper failed.

## When setup stops

Packages can be installed while DNS setup or activation remains unfinished.
Typing DNS is consent, not evidence that the step completed.

1. Finish or cancel the terminal and system prompts. Failure alone does not mean
   a password dialog is still open.
2. Keep **Setup stopped at stage: CODE**, if displayed.
3. Close/acknowledge the terminal, choose Check again and use the current remaining
   action, not the action from an earlier attempt.
4. If none is safe, use [project issues](https://github.com/k-kostin/omavless/issues).
   Include the action, panel text and stage code, not profiles, links or raw logs.

<details>
<summary>What the stage code means</summary>

| Stage | Boundary that did not finish |
| --- | --- |
| runtime-directory / setup-lock / temporary-directory | Local transaction preparation. A leftover lock is not automatically disposable. |
| private-initialization / store-validation | Private-data preparation/validation; data were not reset. |
| dns-enrollment / dns-broker-start | Administrator operation; inspect what completed before another action. |
| dns-broker-readiness / dns-socket-access | Required service/socket facts were not confirmed. |
| dns-template / dns-selection | Managed configuration selection; custom templates are not silently rewritten. |
| application-activation / runtime-enablement / completion | Activation, startup enablement or final readiness was not confirmed. |
| upgrade-operation | Update refused or did not finish; inspect the actual installed result before continuing. |

</details>

Do not delete a setup lock, ownership record, DNS journal or socket to force
progress. Reinstallation does not resolve an unknown result or authorize
repeating a possibly completed operation. Use Settings → Support report for the
bounded **Copy report / Save file**, rather than sharing raw status/configuration.

## Updates

Update the widget and native packages separately. This updates only the widget:

```sh
omarchy plugin update kdk.omavless
```

The 0.9.9 release supplies matching `omavless` and `omavless-dns` packages for
x86_64 and ARM64, with package version `0.9.9-1`. Use the offered component
update rather than mixing separate downloads. Version text alone does not prove
that the files form a supported installation.

### Updating the public 0.8.2 application

Set VPN login autoconnect Off, Disconnect and fully shut down OmaVLESS first.
Update the widget, then run `omarchy plugin enable kdk.omavless` to reopen only
the widget. A supported public 0.8.2 installation offers **Update OmaVLESS**.
Keep the application stopped. Confirm UPGRADE and the ordinary sudo/pacman
prompts; setup of the helper and Start OmaVLESS follow as separate steps.
Unknown or modified installations need inspection, not a data reset.

<a id="already-using-098"></a>

### Updating a supported public 0.9.8 installation

For partial setup, first Check again and inspect the remaining step. Do not
reinstall merely because an earlier terminal was cancelled.

When **Update components…** is offered:

1. Turn VPN login autoconnect Off, Disconnect and confirm Settings →
   **Shut down OmaVLESS**.
2. Run `omarchy plugin enable kdk.omavless` and reopen only the widget.
   Keep the application stopped; do not choose Start OmaVLESS before updating.
3. Choose Update components…. Confirm `PREPARE` and the separate system
   authorization. This checks and safely stops a supported helper if needed;
   it does not install packages or force-stop an unknown operation.
4. The same terminal then asks for `UPGRADE`. Confirm it and the ordinary package
   transaction. Enter cancels before installation, leaving old packages and the
   helper stopped. Later, close/acknowledge the terminal and Check again. The
   next offered update rechecks the stopped boundary rather than repeating a
   completed helper stop.
5. After closing and acknowledging completion, choose **Start DNS helper**, then
   separately **Start OmaVLESS**, as offered. Existing helper access does not
   need first-time authorization. No VPN starts automatically.

The first original 0.9.8 helper does not support automatic preparation; ask for
help with its guarded replacement. Modified, mixed or unknown installations
must not be forced through. Do not overwrite package files, replace one package
alone or repeat DNS authorization to bypass a refusal.

A successful update may offer optional `RELOAD` for the whole Omarchy shell;
Enter skips it. Other shell plugins reload too. Reload failure does not undo an
update and is not permission to repeat it. Starting the helper and application
remains separate after either choice.

## Verify the installation

Ready means the required application/component facts are available. It does not
prove server access. Explicitly connect your intended supported profile, then
check website access and the meanings of Test/Ping described above.

If replacement of the widget left old controls or stale state, first finish
its terminal/system prompts, then deliberately reload the shell:

```sh
omarchy restart shell
```

This refreshes the bar, panels and other plugins, not the separate native VPN
runtime. It is not recovery for an unresolved runtime operation.

## Updates, close, Quit and removal

| Action | Effect |
| --- | --- |
| Close panel / terminal / restart shell | Leaves the VPN and accepted work running. |
| Disconnect | Stops the VPN; keeps the application available. |
| Settings → Shut down OmaVLESS / Quit | Confirms shutdown and cleanup, then stops/disables the application and widget; preserves private data. |
| Persistently disable/remove widget | Requests fenced application shutdown. A missing icon does not prove it finished. |

Re-enable the widget to reopen it, then use the offered DNS step and Start
OmaVLESS. A new VPN connection is separate. Login autoconnect defaults to Off.
When deliberately enabled, Last or a pinned profile applies at a new login;
a confirmed Disconnect in the current login is retained across an application
restart. Starting the application at login and connecting a VPN are different
preferences. After a network change, verify current state and access rather
than assuming a prior successful check still applies.

### Reinstalling the widget is not a clean app installation

`omarchy plugin remove kdk.omavless` removes the widget, not the native packages,
helper access or saved profiles. Adding it again resumes an existing installation.
Use the offered continuation, or choose full Remove below.

## Remove

**Removal deletes saved OmaVLESS profiles and settings without a backup.**
Other VPN clients and an unrelated Mihomo installation remain installed.

Choose **Remove OmaVLESS…** in Settings or on a setup/error screen. Review what
will be removed, then Continue to its terminal. This action does not require a
working connection or completed onboarding. If the widget is missing, add the
current widget again with the installation command above and choose Remove;
do not start the application or connect merely to remove it.

1. Type `REMOVE` to confirm deletion. The helper closes OmaVLESS if needed and
   prepares removal; enter your password only at sudo's own prompt.
2. If a restart is required, save other work and type `REBOOT`, or restart later
   from the desktop. Follow a no-restart continuation only when offered.
3. After logging back in, continue with the **same cached helper command printed
   by that terminal**. Keep that command privately; do not download a different
   helper or reopen/reinstall the application between removal stages.
4. Confirm REMOVE and the remaining package/widget prompts. Wait for the explicit
   completed-removal message.

Cancellation can leave an earlier stage completed. Preserve removal progress
and follow its remaining step; do not delete records, unmask services or clear
network state manually. Unknown or modified builds and another account's setup
are refused rather than guessed away. After confirmed full removal, installation
requires the separate offered helper-access setup again.

The [0.9.8 terminal-removal reference](CLEAN_REINSTALL_098.md) belongs to that
older delivery; do not use its old checksum/helper to remove a different release.
The historical `uninstall.sh --purge` is not a native removal command.

## Optional helpers, firewall and older installations

Clipboard uses `wl-clipboard`; file selection uses zenity/kdialog/yad; editing
uses zenity and QR display uses qrencode. Missing optional helpers are reported,
not silently installed. For example, deliberately add a picker with
`omarchy pkg add zenity`. Clipboard import does not need a picker.

A default-deny firewall can block tunnel traffic even when Connected is shown.
Setup does not disable or change firewall rules. Read the
[scoped firewall guidance](NATIVE_INSTALL_082_REFERENCE.md#default-deny-firewall-and-tun)
and review the actual interface, address and policy. Do not copy an exception
for another computer.

The [historical 0.8.2 reference](NATIVE_INSTALL_082_REFERENCE.md) describes older
stock-Mihomo/manual migration. It is not the current managed installation route.

## Help

### Ask your assistant

Open Settings → **Help**, or **Setup help**. Choose **Copy request**, paste it
into an assistant on this computer and describe the problem at the end.
**Show request** lets you inspect/select the text if copying fails. The request
contains no profiles, keys or diagnostic data and points to the installed
English assistance guide:

```text
Read the installed OmaVLESS assistance guide:
~/.config/omarchy/plugins/kdk.omavless/docs/user/USER_AGENT_GUIDE.md
Also read AGENTS.md in the plugin directory if present.

Help me with OmaVLESS in Omarchy. Problem:

```

[Guide for your assistant](USER_AGENT_GUIDE.md) explains safe diagnosis and
version checks. If the widget was removed, provide this web guide and ask the
assistant to identify your installed release first. Review any proposed issue,
patch or workaround before agreeing to publish it. Using an assistant or reading
the guide is not required before **Report a problem ↗**.

Help follows the selected interface language when a matching reviewed guide is
available; otherwise it uses English. Agent instructions remain English.
Opening a link does not itself prove that the browser loaded the page.

### Setup is ready, but websites do not open

Use a working supported profile and explicitly Connect. Try another server if
one is unavailable. Connected/counters alone do not prove Internet access, DNS
or complete routing. Check other VPN and firewall settings without disabling
security checks to conceal an error.

### Which actions leave my VPN running?

Closing the panel, closing the terminal and restarting the shell leave it
running. Direct is not Disconnect. Use Disconnect for the connection, or
Shut down OmaVLESS for the application. Removing only the widget is not full removal.

### Do I need to install or configure Mihomo separately?

No. The matching core and DNS helper are supplied together in omavless-dns.
Do not substitute an unrelated core or another VPN client's full configuration.

<a id="why-does-pacman-still-show-098-1-after-an-update"></a>

### How do I check which components were updated?

Use the panel's component facts and offered update. The widget version alone
is not the native package version, and equal package versions do not prove equal
files. An unconfirmed update needs inspection before another attempt.

### Can I restore a backup or remove everything from Settings?

The terminal application offers preview-bound Restore while safely disconnected.
Remove OmaVLESS… opens a separate destructive-removal review from Settings and
setup/error screens. Neither action is implied by deleting the widget.

### How do I ask for help safely?

Use Settings → Support report → Copy report / Save file. Include the attempted
action and short error/stage. Inspect screenshots before posting: profile or
provider identities, subscription URLs, credentials, QR codes, backups and raw
private logs must not be shared. Setup screens can be reported by their action
and stage without supplying a VPN configuration.

[Project issues](https://github.com/k-kostin/omavless/issues) ·
[Troubleshooting](TROUBLESHOOTING.md) · [Security and privacy](SECURITY.md) ·
[Supported profiles](PROTOCOLS.md) · [Guide for your assistant](USER_AGENT_GUIDE.md)
