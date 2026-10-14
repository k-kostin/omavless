# Set up, update or reopen OmaVLESS

This guide is for **OmaVLESS 0.9.8 on Omarchy**. Start with the section that
matches your panel. Installing or starting the application does **not** connect
a VPN; you choose a profile and Connect separately.

## Find your next step

| What you see | What to do |
| --- | --- |
| Required components / missing application or managed DNS companion | [Install the matching components](#guided-first-run). |
| Packages installed, DNS setup incomplete | [Continue DNS setup](#continue-dns-setup). Do not reinstall just because setup is incomplete. |
| Start existing DNS broker / Restore DNS enrollment | [Use the existing-installation DNS action](#existing-dns-installations). |
| Start OmaVLESS | Start the application after any DNS step finishes. It starts disconnected and does not enable login startup. |
| Application update required | [Update the old application](#updates). Updating the plugin alone does not update native packages. |
| Setup terminal and prompts are closed | Finish or cancel the terminal and all OS prompts first; then press this acknowledgement and Check again. |
| Setup needs attention / State unverified / Recovery required | [Inspect the current result](#when-setup-stops). Do not repeatedly install or connect to clear a warning. |

**Set up later** closes the panel without changing data or hiding missing
components. **Finish later** at the final onboarding step finishes the wizard
without importing a profile. Neither starts a VPN.

<a id="guided-first-run--release-preparation"></a>

## Guided first run

1. Add the plugin through Omarchy:

   ```sh
   omarchy plugin add https://github.com/k-kostin/omavless --enable
   ```

   This follows upstream main. Marketplace review covers a separate exact
   snapshot. The command installs the widget only: it does not run
   `install.sh`, install native packages or execute their sudo steps.

2. Open OmaVLESS from the bar. Choose **Install required components** (or the
   offered missing-component action). In the terminal, type `INSTALL` when
   asked. Approve the ordinary package transaction; enter your password only
   at the system's password prompt.

3. When asked about DNS, type `DNS` to authorize the separate DNS setup.
   The matching `omavless-dns` package includes the reviewed Mihomo core and
   DNS broker. You do not need to install an AUR Mihomo or run `setcap` on an
   unrelated binary for this route.

4. When the terminal says **OmaVLESS is ready**, close it. Resolve any remaining
   OS prompts, press **Setup terminal and prompts are closed** if shown, then
   **Check again**. If you cancelled a later step, use the remaining action
   below instead of starting over.

5. Follow onboarding to review readiness and optionally select Routing/import
   a profile. **Finish later** is available without a profile. The default VPN
   autoconnect setting is Off. Fresh setup enables the application service for
   future logins, but that is not permission to auto-connect.

Import your own compatible VPN profile or HTTPS subscription, choose a mode,
then Connect. Servers/subscriptions are not included. See
[everyday use](NATIVE_USAGE.md) for controls and Backup.

## Continue DNS setup

If the packages are installed but DNS setup is incomplete, choose
**Continue DNS setup**. It is offered only when the application and broker are
safely stopped; an unknown state is not treated as a new installation.

The terminal gives you two choices, on separate lines:

- `FIRST` — **you have not enrolled this account before**. The next `DNS`
  confirmation authorizes first enrollment. An existing registration or unsafe
  broker state is refused rather than overwritten.
- `START` — **this account was already enrolled** and you want to use that
  enrollment unchanged. The existing-enrollment check must pass. Then `SELECT`
  explicitly chooses the installed managed pair.

Press Enter to cancel. Do not guess `FIRST` merely because `START` failed:
a failure is not proof that no enrollment exists. If you are unsure which
step completed, see [When setup stops](#when-setup-stops).

After the DNS terminal finishes, close it and settle its OS prompts. Acknowledge
closure in the panel, then Check again. Use the separate **Start OmaVLESS**
action when offered. It starts the application, not the VPN. Existing profiles,
ownership and startup preferences are preserved; this continuation does not
repeat first-use activation.

## Existing DNS installations

For an already selected managed pair, the panel may offer:

- **Start existing DNS broker** — use enrollment that is still present after a
  broker stop or a clean package update.
- **Restore DNS enrollment** — enroll again after a verified clean removal
  revoked the old registration. This is not profile Restore.
- `SELECT` in the setup terminal — an existing accessible broker is ready,
  but the installed pair has not yet been selected.

Complete the offered DNS step and its OS prompts before **Start OmaVLESS**.
These actions do not connect, reset profiles or enable application startup.
Do not use restoration as a fallback for a refused existing-enrollment action.

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
Settings' Copy report/Save report is the bounded support report; private
status/detail output is not automatically safe to publish.

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

The corrected delivery is
[`v0.9.8-fix.4`](https://github.com/k-kostin/omavless/releases/tag/v0.9.8-fix.4).
Its public archives and SHA-256 records identify the matching packages for
x86_64 and ARM64. Product/package versions remain `0.9.8` / `0.9.8-1`, so
the version label alone cannot distinguish earlier deliveries. Do not mix
packages from different corrections. The release's
[acceptance record](../testing/SETUP_082_UPGRADE_VM_2026-10-13.md) separates
installed x86_64 results from the still-pending installed ARM64 check.

### Updating the public 0.8.2 application

Set login autoconnect Off, disconnect and fully shut down OmaVLESS first.
After the widget update, a verified compatible public 0.8.2 installation with
no DNS companion offers **Update required components**.

Confirm `UPGRADE` and the normal sudo/pacman prompts. This installs the matched
application/DNS pair while preserving profiles and the stopped application.
It is followed by separate **Continue DNS setup** and **Start OmaVLESS** steps.
Unknown/modified predecessors or custom templates may require support; do not
reset data to make them qualify.

A successful updater offers optional `RELOAD` to refresh the whole Omarchy
shell; Enter skips it. The bar and other shell plugins briefly disappear.
A reload failure does not undo the package update or mean you should repeat it.

### Already using 0.9.8

Update the widget first. For a partial setup, use Check again and finish the
remaining DNS/application step; do not initialize or activate an existing
Rust-owned installation again.

Native-package replacement is a separate, guarded operation, not the 0.8.2
updater. Before replacing **both** matching packages, verify clean shutdown,
no other user's active runtime, an idle stopped DNS broker with no lease/TUN,
and settled authorization. The package guard must pass. Download the matching
archives and verify their source/architecture and SHA-256 before the ordinary
pacman transaction. An unchanged package version may show **reinstalling**.
Do not bypass guards with forced overwrite or dependency/confirmation switches.

For an installation that has already enrolled DNS, preserve its enrollment and
use the existing-broker continuation after replacement. Do not rerun first
enrollment. If shutdown or broker state is unknown, use support rather than
stopping/removing components blindly. The
[administrator lifecycle contract](../development/DNS_RELEASE_DISTRIBUTION.md#replacement-removal-and-firewall)
explains the guarded replacement/removal boundary.

## Verify the installation

Ready means the panel can read the native application and its managed
components; it is not proof that your server or network works. After an explicit
connection, check actual website access and the native HTTPS test. Routing or
Direct may send a test directly instead of through the VPN.

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

To completely uninstall the application, first verify native shutdown and clean
DNS restoration. An administrator must stop/disable the clean broker, confirm
its empty journal/FD/TUN boundary, and revoke the enrolled UID while the broker
binary still exists; only then remove both native packages and the widget.
Keep private profiles by default. A blocked guard needs inspection, not deletion
of unknown state. The administrator contract linked above documents these
requirements; a one-click all-stage uninstaller is planned, **not available in
0.9.8**. Ask for help if setup never reached a usable shutdown screen.

Do not run the legacy `uninstall.sh --purge` against a native installation.
It is not the native package remover. A clean reinstall must not silently erase
profiles or reuse a revoked enrollment; the panel offers its explicit DNS
restoration step after a clean package reinstall.

## Optional helpers, firewall and older installations

The managed DNS companion already supplies the reviewed Mihomo core. A stock
Mihomo installation or `setcap` on a different binary is not a substitute.

Clipboard uses `wl-clipboard`; file selection uses zenity/kdialog/yad; editing
uses zenity and QR display uses qrencode. Missing optional helpers are reported
rather than silently installed. For example, deliberately install a picker
with `omarchy pkg add zenity`. Clipboard import does not require a picker.

A default-deny firewall can block traffic on the TUN even when the panel says
Connected. Setup never disables or changes firewall rules. See the
[scoped firewall guidance](NATIVE_INSTALL_082_REFERENCE.md#default-deny-firewall-and-tun);
review your actual interface/address/policy rather than copying an exception
for a different host.

The [historical 0.8.2 reference](NATIVE_INSTALL_082_REFERENCE.md) retains older
stock-Mihomo, manual activation and legacy migration instructions. Those are
not the current managed-pair installation route. No Python runtime or Rust
toolchain is required for ordinary use.
