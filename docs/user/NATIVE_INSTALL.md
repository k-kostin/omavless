# OmaVLESS installation, updates and recovery

## OmaVLESS 0.9.8: managed-package route

The owner-selected release snapshot uses corrected delivery `v0.9.8-fix.1`,
with exact app/DNS hashes for both
architectures. Product/package versions remain `0.9.8` / `0.9.8-1`; the release
tag distinguishes corrected bytes from the immutable first validation build.
Do not mix its frontend with the earlier `v0.9.8` packages or compile Rust in a
marketplace installer. Download/setup acceptance and the actual main/stable
promotion event are tracked in the
[candidate ledger](../development/RC_098.md).

The owner requested that root README remain unchanged during this promotion;
its older 0.8.2 manual-download paragraph is not the matching package route for
this frontend. Follow this section and the [corrected release](https://github.com/k-kostin/omavless/releases/tag/v0.9.8-fix.1).
The separately reviewed Marketplace snapshot remains 0.8.2 until a new approved
submission; this release does not update that snapshot.

The selected 0.9 route differs from the released 0.8.2 instructions below:

1. Use one reviewed version/architecture-matched `omavless` application,
   `omavless-dns` companion and frontend. The companion contains the fixed
   reviewed Mihomo core and DNS broker; arbitrary stock/AUR core discovery and
   `setcap` on a different binary do not satisfy managed-pair admission.
2. Through **Required components**, deliberately install the matching packages
   using normal pacman/sudo authorization. Then perform the separate explicit
   DNS-broker enrollment and pair selection offered by setup. Adding/enabling
   the plugin alone installs nothing and connects no VPN.
3. For an existing installation, first establish verified disconnected state
   and settled OS authorization, preserve private data, and follow the guarded
   package-update procedure. Do not rerun first activation or delete ownership
   records to turn it into a fresh install. Active or quarantined package
   changes must be refused by the package guard.
4. If enrollment exists but the broker is stopped, use **Start existing DNS
   broker**; **Restore DNS enrollment** is only for revoked enrollment after
   clean removal. Neither action starts a VPN. Afterwards use **Start OmaVLESS**
   to launch the application, without enabling login startup or connecting.
5. Import your own profile and connect explicitly. Verify a native HTTPS test
   and actual website access, not only the Connected label. A default-deny
   firewall can block DNS ingress on the owned TUN; follow the narrowly scoped
   firewall guidance below rather than disabling protection globally.

The 0.9.8 scope decision is closed: encrypted Backup is included, Restore is not.
Completed Restore/recovery remains mandatory for 0.9.9. See
[Backup usage and limits](NATIVE_USAGE.md#encrypted-backup-098-candidate).
The matching frontend verifies both package hashes and source identities before
normal installation. Earlier validation users must use the guarded update path;
the unchanged Arch version may show **reinstalling**, not an automatic update.

## Released 0.8.2 route and historical compatibility

This guide covers the **0.8.2 native release** on Arch/Omarchy. The validation-only
0.9 RC development branches have a different requirement: a version-matched
`omavless` + `omavless-dns` package pair, separate administrator enrollment
for the fixed DNS broker, and a stopped/disconnected runtime for managed-pair
selection before Rust activation. The 0.9 frontend's pinned `v0.9.0-rc.1`
prerelease assets are for RC validation, not a supported upgrade: the ARM64
fresh-account GUI path passed, but negative/recovery and release gates remain.
It treats an already
registered package, leftover
user runtime unit or `Meta` TUN as an existing/ambiguous installation, not a
fresh machine to overwrite; inspect that state separately. Do not
apply the 0.8.2 stock-Mihomo steps below to a 0.9 candidate; see the
[managed DNS distribution contract](../development/DNS_RELEASE_DISTRIBUTION.md)
for the current development status.
For the prerelease 0.9 candidate, a clean removal may preserve private
profiles and the selected pair while revoking the privileged broker
registration. On reinstall, **Restore DNS enrollment** is a separate attended
step, not a second package install or a VPN connection. It refuses residual or
unknown broker state and does not re-enable a previously stopped user runtime.
After a clean package update or temporary broker stop, the registration may
instead still be present: choose **Start existing DNS broker**, not Restore.
That action checks the fixed enrollment file metadata and starts only the
broker; it does not re-enroll, start the user runtime or connect. If the
enrollment was revoked during a clean removal, choose Restore instead. A
failed or uncertain authorization requires inspection before either retry.

New users of the released version can
install the plugin and follow its guided first-run setup. Existing native and
legacy users have separate update/migration routes below; do not reset an
existing store or repeat activation.

The GitHub release is stable; the marketplace's exact 0.8.2 update was reviewed
and published through [#8093](https://github.com/omacom/omarchy-plugin-marketplace/issues/8093).
That approval does not cover this newer development candidate.
Ordinary `omarchy plugin add` installs
the frontend, not the native package or its ownership. The panel offers those
steps explicitly, with normal user/OS confirmation. OmaVLESS itself is delivered
as a reviewed release package, not an AUR package announcement.

## Choose your installation route

| Starting point | Route | Do not do |
| --- | --- | --- |
| New user, no application/data | Add plugin → Required components → terminal setup → Check again/reopen → onboarding. Matching public 0.8.2 packages are pinned; fresh x86_64 provisioning is checked. | Do not pair the current frontend with an older native package or confuse upstream main with the reviewed marketplace snapshot. |
| Application installed, not activated | **Complete setup** validates/prepares data and activates once; install a missing core first if requested. Existing legacy data requires the migration preconditions below. | Do not reinstall the app or reset a store merely because activation is incomplete. |
| Already activated native installation | Keep private data and ownership; use the disconnected package-update route only when the runtime package changes. A reviewed compatible frontend-only update does not need package replacement. | Do not initialize or activate again, or treat first-run setup as an updater. |
| Setup postponed | Reopen the panel; missing components remain visible. Finish in the existing terminal before acknowledging its closure and deliberately retrying. | Do not mark OS authorization complete just because the terminal launched or the panel closed. |
| Previously used confirmed Quit | In the selected 0.9.8 candidate, re-enable the plugin and use **Start OmaVLESS** after any separate DNS preparation. Earlier releases use the explicit manual reopen procedure below. | Do not reinstall, reset ownership or assume that re-enabling the frontend connects a VPN. |

<a id="guided-first-run--release-preparation"></a>

## Guided first run

For the current upstream release:

```sh
omarchy plugin add https://github.com/k-kostin/omavless --enable
```

This clones mutable upstream HEAD, not an exact marketplace-verified snapshot.
Review the source before enabling it. The command does not run `install.sh`,
install Mihomo/the native application, or invoke their privileged setup.
Those are separate, explicitly confirmed actions in the panel below.

The frontend has a panel shell that works **without** the native application.
It shows the **Required components** block below the unavailable Profiles area,
instead of trapping the user in a setup wizard. Adding/enabling or reopening
the plugin never executes an installer automatically. Missing components stay
visible even when onboarding is deferred:

- Missing OmaVLESS and Mihomo: both rows and one **Install required components**
  action, installing dependencies sequentially in one terminal.
- Missing only OmaVLESS: **Install OmaVLESS**, preserving a compatible packaged core.
- Missing only Mihomo with an activated app: **Install Mihomo** below the real
  profile list. The app/store/service are not reinstalled or activated again.
- Both installed: the missing-components block disappears. An unactivated app
  instead gets a separate **Complete setup** block, not an install offer.
- Failed/unknown discovery: guidance and recheck, never an invented missing
  program or a successful setup claim.

Component presence does not certify permissions, TUN, DNS or live connection
health. Core setup in the normal onboarding/Settings remains responsible for
those distinctions. Picker/editor/QR/clipboard tools remain optional helpers.
When the core is known to be absent, Connect is disabled; Disconnect is not.

The appropriate install button opens a terminal for explicit confirmation.
The checked setup path is:

1. Add the plugin through Omarchy's normal marketplace command.
2. Open the plugin and use the required-components action. For application
   setup, type `INSTALL` in its terminal; core-only setup asks for `CORE`.
3. The helper downloads the version/architecture-specific OmaVLESS package
   pinned by SHA-256 in the reviewed frontend. No Rust/Cargo build is performed.
   If the package dependency Mihomo is absent, a separate `CORE` confirmation
   offers the documented `omarchy pkg aur add mihomo-bin` route. This uses the
   AUR, not the official Arch repositories; normal host authorization remains.
   A preinstalled package providing `mihomo` is respected.
4. Normal `sudo pacman -U` installs the application. Setup prepares a new empty
   private store **only when absent**, validates existing data, uses canonical
   native activation and enables the disconnected user service for future logins.
5. Return to the panel and choose **Check again**. The existing onboarding then
   covers core/TUN readiness, routing, helpers and profile import. Setup does not
   grant TUN capabilities, connect a VPN or silently install optional helpers.

**0.8.2 release:** `plugin/runtime-release.json` pins the reviewed ARM64
and x86_64 packages, including their exact runtime source and SHA-256. Setup
does not follow `latest` or fall back to older 0.8.0/0.8.1 runtimes. Confirm the
matching assets are present on [GitHub Releases](https://github.com/k-kostin/omavless/releases)
before provisioning. The complete fresh download/install/activation/onboarding
path, including initially absent Mihomo, passed on a clean Omarchy x86_64 VM;
see the [scoped acceptance record](../testing/NATIVE_082_FRESH_VM_2026-09-21.md).
This is distinct from live VPN/TUN acceptance and does not silently grant
permissions. Stable promotion does not itself change the marketplace snapshot.

Set up later closes the panel without saving a false completion or hiding the
required-components reminder on reopen. After starting
setup, finish/cancel it and all authorization dialogs in its terminal. Checking
status never repeats installation. Before deliberately retrying, confirm
**Setup terminal and prompts are closed**; never do this while an authorization
is unresolved. A lock from a killed installer is not cleared automatically.
Existing active legacy owners, unsafe stores and interrupted migrations require
the recovery guidance below; this page does not force migration or reset data.

**Two different “later” actions:** **Set up later** on the runtime-independent
panel closes it without installing anything or hiding missing-component reminders.
**Finish later** at the final profile-import step of the normal onboarding wizard
records that the wizard is complete without requiring a profile. It does not
install missing components, certify TUN readiness or connect a VPN. The guide can
be reopened from Settings; required-component checks remain independent of that
wizard-completion flag.

Cancelling the initial `INSTALL` consent occurs before installation effects.
Cancelling/failing a later core/package/activation step is different: an earlier
step may already have succeeded. There is no automatic uninstall or rollback.
Close/resolve all terminal and authorization prompts, inspect the actual state,
then use **Check again** and the appropriate remaining action. Never retry while
an authorization is unresolved or delete a stale setup lock/ownership marker to
force progress. A started terminal is not proof that setup succeeded.

An already activated native installation bypasses provisioning. Updating it
still uses the reviewed disconnected package-update procedure, not this
first-install helper. The manual reviewed-package path below remains an
alternative to the guided setup.

The native runtime/CLI does not require Python, pip, a virtual environment or
Cargo at runtime. Its Omarchy frontend is still QML. The old Python backend is
preserved separately in a frozen historical archive; remaining Python files in
main are developer test/build tools, not source installation or a runtime fallback.

Already installed? See [native everyday use](NATIVE_USAGE.md) for connection
selection, subscription refresh, language, diagnostics and Quit.

For the published **0.8.2** artifact pair, verify `SHA256SUMS` and the exact
source/architecture in `release-candidate.json` (single-source assembly) or
`frontend-pair.json` (a newer frontend paired with an unchanged reviewed runtime)
before following this guide. A pairing record retains both exact source commits
and verified runtime/build/package input equality; a matching version alone is
not compatibility proof. Preserve the original package build/acceptance evidence.
An unpublished artifact is not a public release; marketplace publication remains
owner-controlled. Both source and assembled frontend carry the same version;
the historical 0.7.0 snapshot was superseded by the reviewed 0.8.2 update.
Earlier `0.8.0-rc.1`
archives retain their original version and hashes, not the current release's.

Use the runtime package for your processor (`aarch64` or `x86_64`). The QML
frontend and supported features are common to both; use the reviewed artifact
pair and its source/version records. Architecture-specific native binaries are
not separate plugin products.

## Before installation

Use a trusted, reviewed prebuilt `omavless` archive for the host architecture
(`aarch64` or `x86_64`), together with its exact source commit and SHA-256 record.
The local archive name alone is not proof of provenance. Keep the current known
working archive outside temporary directories for recovery. Build instructions
are separate in the [native package notes](../../packaging/arch/README.md).

Preserve a private backup of existing OmaVLESS configuration/state before
migration. Never put that backup, profile links, subscription URLs or exported
keys into Git, issue comments or public command output. Do not edit ownership
markers, login receipts or store records to force acceptance.

The package depends on Mihomo and normal Arch runtime libraries/tools, including
systemd, libcap, iputils and bubblewrap. It does not grant Mihomo capabilities,
install privileged policy or enable a service in a package hook. Follow the
[Mihomo readiness guidance](INSTALL.md#grant-tun-capabilities) and verify the
actual core path. Complete every normal OS authorization prompt before another
connection or service action; a cancelled prompt is not successful setup.

### Default-deny firewall and TUN

If your host uses a default-deny inbound firewall such as UFW, check its TUN
policy before treating a Connected indicator as proof of working traffic.
On one isolated Omarchy VM, the core and routes were healthy but HTTPS through
the TUN timed out because UFW blocked packets arriving on that interface.
OmaVLESS does not change firewall rules automatically. An administrator should
review the local TUN address, peer, interface and firewall policy, apply only
the exception appropriate for that host, and verify actual traffic and cleanup.
The built-in HTTPS check (`omavless runtime test`) is read-only and follows the
current route; in Direct or selective Routing mode it may not traverse the VPN.

For the **bundled IPv4 template only**, first confirm that the active TUN is
`Meta` with address `198.18.0.1/30` and that UFW is active with default-deny
incoming policy (`ip -4 addr show dev Meta` and `sudo ufw status verbose`). If
those facts match and the administrator accepts the host-specific exception,
the following reversible rule allows incoming packets from `Meta` only when
their destination is its local IPv4 address:

```sh
sudo ufw allow in on Meta to 198.18.0.1 comment omavless-tun
```

It does **not** restrict remote source addresses, ports or protocols: return
traffic can have many such values. Review services listening on that local
address and your firewall threat model before applying it. If this exception
is too broad for the host, keep the default-deny policy and design a suitable
host-specific rule instead; do not disable UFW to make a VPN test pass.

Retest traffic in Full VPN mode, then inspect `sudo ufw status numbered` for
duplicate or unexpected rules. Remove this exact exception when no longer
needed:

```sh
sudo ufw delete allow in on Meta to 198.18.0.1 comment omavless-tun
```

Do not copy the example for a custom TUN address, another firewall, or IPv6.
The VM's temporary IPv4 result is not an assurance about every protocol,
destination or host policy. A firewall check does not replace the normal
connected, DNS, route and HTTPS verification.

Desktop helpers remain optional package dependencies:

- `wl-clipboard` for clipboard operations;
- `zenity`, `kdialog` or `yad` for file selection, in that preference order;
- `zenity` specifically for editing a profile;
- `qrencode` for QR display.

The native helper does **not** use the legacy Python/GTK4 fallback. Onboarding
and Settings report missing helpers. For example, install the lightweight picker
explicitly with `omarchy pkg add zenity`; OmaVLESS does not run that command for
you. Clipboard import does not require a picker.

## Install the reviewed archive

Do not replace a package underneath an uninspected running tunnel or another
user's active native runtime. For an existing native installation, follow the
disconnected update procedure below. For a first installation, install the
reviewed archive with normal dependency/conflict checks:

```sh
sudo pacman -U -- /absolute/path/to/reviewed-omavless.pkg.tar.zst
systemctl --user daemon-reload
```

Replace the example path with the actual archive. Do not use `--nodeps`,
`--overwrite`, `--noconfirm` or a network download as a shortcut. Installing the
archive places `/usr/bin/omavless` and its two user units on disk; it does not
initialize user data, activate ownership or start a tunnel.

Run the following application commands as your ordinary desktop user, not with
sudo, and with the same HOME/XDG roots as that user's systemd manager. Test-only
`OMAVLESS_HOME` overrides are not an installed activation path.

## Choose the correct initialization path

### New user with no OmaVLESS data

Prepare the initial private empty store and bundled routing template:

```sh
/usr/bin/omavless setup initialize
```

This is create-only preparation, not activation or a reset command. It preserves
existing data and refuses incompatible/occupied state instead of replacing it.
Startup is Off and no profile or tunnel is created. Continue to activation below.

### Existing legacy/Python installation

Do **not** run `setup initialize` over existing profiles. In the existing UI,
set login autoconnect Off, disconnect, and finish every authorization dialog.
The legacy runtime/autostart units must not remain enabled or active. A loaded
legacy runtime unit and the new native runtime unit must be disabled before
activation; the native service must not already be running.

Check compatibility with the installed read-only commands:

```sh
/usr/bin/omavless store-compatibility
/usr/bin/omavless cutover-preflight
```

Resolve refusals through the existing supported UI/recovery guidance. Do not
delete active pointers, receipts or markers manually, bypass private-file
permissions, or start a second daemon. These checks alone do not migrate data
or activate the package.

### Already committed native owner

If `omavless plugin target` returns `rust`, do not initialize or activate again.
Use the update/reopening procedure. Missing or refused ownership is not an
instruction to recreate markers or fall back to Python.

## Activate once, then install the matching frontend

With the new or compatible legacy store prepared, startup Off, both runtimes
stopped and no competing core/TUN, run the exact installed activation command:

```sh
/usr/bin/omavless cutover activate
/usr/bin/omavless plugin target
```

Successful activation commits Rust ownership and starts the native service
disconnected. The target must report `rust`. It does not enable login startup.
If the command's output was lost, inspect target/status first: repeating
activation is not recovery. A refused or interrupted transition must follow the
[activation/recovery contract](../testing/R5_DISCONNECTED_ACTIVATION.md); there
is no supported force-activation or marker-deletion shortcut.

From the extracted **matching frontend archive**, run `./install.sh` without
arguments: its entry point always selects native-only installation. It requires
the already activated owner and cannot install the legacy payload.

Alternatively, from the reviewed **full source checkout** matching the release:

```sh
./install.sh
```

This requires already committed Rust ownership, preserves the plugin's
enabled state on update, and omits installed `backend.py` and the legacy
`uninstall.sh`. Plain `./install.sh` is also the native-only update path;
`--native-only` remains a compatible alias. A missing Rust executable, legacy or
unknown ownership refuses installation before replacing the existing frontend.
It never starts Python, activates ownership, or downloads/builds a package.

Omarchy's clone-based `plugin add`/`plugin update` does not run `install.sh` or
install the package. The independent first-run page above supplies the explicit
setup entry point, with published 0.8.2 pins and scoped fresh x86_64 acceptance.
Existing native owners still use the disconnected update path; do not invoke
first-user initialization again. The backend launcher still refuses absent,
legacy or unknown native ownership; the setup page does not bypass that guard.

To explicitly enable the runtime for future user sessions after activation:

```sh
systemctl --user enable omavless-runtime.service
```

Startup Off means this enabled service starts disconnected. Saving Last/pinned
preferences and enabling a user unit are distinct actions; neither alone proves
that an actual fresh-login autoconnect passed. Do not manually run
`login-prepare` or modify its receipt to simulate a login.

**Current native release:** VPN autoconnect is Off by default. Last/pinned
autoconnect is optional and its connected fresh-login validation is incomplete;
leave it Off unless deliberately testing that feature. Saving Off does not
disconnect a currently running VPN. Existing user preferences are not silently
reset by this documentation or by declaring the migration complete.

<a id="verify-the-installed-candidate"></a>

## Verify the installation

The checks below apply equally to a stable release and a development candidate.

Use `omavless plugin target`, `omavless status`, `omavless runtime observation`
and `systemctl --user status omavless-runtime.service` locally. The native user
service owns the single runtime; the QML frontend does not own a second core.
Private status/detail responses are not automatically shareable. Settings'
Copy report/Save report uses the bounded support projection instead.

Verify package/binary/frontend identity against the recorded acceptance result.
Replacing the on-disk executable does not upgrade an already running daemon.
Support facts also do not prove working DNS, route restoration, internet access
or successful login activation; those need their own observed checks.

If a paired frontend update still shows **State unverified**, first compare the
fresh `omavless runtime observation` with the panel. Do not repeatedly toggle
the VPN or reset private state. A running Quickshell may retain an old JavaScript
parser even after plugin rescan. When the native runtime is healthy but the
panel cannot read its observation, a deliberate `omarchy restart shell` reloads
the graphical frontend; the separate native runtime and tunnel are not restarted
by that command. The bar/panels briefly disappear. Do not use this as a remedy
for actual runtime recovery or an unresolved authorization request.

## Updates, close, Quit and removal

Closing the panel or a terminal is not Disconnect. Settings' confirmed
**Shut down OmaVLESS / Quit** stops the VPN and native runtime, verifies cleanup,
then disables runtime startup and the Omarchy plugin while preserving private
profiles/settings. Failure or an unknown outcome must be inspected, not treated
as a clean shutdown. A shell reload is not this explicit Quit action.

Persistently disabling OmaVLESS through `omarchy plugin disable kdk.omavless`
or a manager using that command also requests fenced application shutdown after
a short reload grace. It is not merely hiding the icon. Re-enabling within the
grace cancels that cleanup; do not rely on this timing as a connection control.
Use Disconnect to stop only the VPN while retaining the application.

In the selected **0.9.8 candidate**, re-enable the frontend with
`omarchy plugin enable kdk.omavless`, then open its panel. A verified stopped
installation offers **Start OmaVLESS**. It starts only the application, without
Connect or enabling login startup, and preserves saved preferences/profiles.
If the managed DNS broker is also stopped, its separate preparation comes first;
resolve that operation's OS authorization before starting the application.
Unsafe, pending or unverified state is not a reason to repeat Start or reset data.
After updating the frontend, old cached controls can require the deliberate
shell reload described above; check what is actually loaded, not just file hashes.

For the currently tested update route, set startup Off and disconnect first.
After verifying clean state and settled authorization, stop the native service,
install the reviewed replacement archive with ordinary `pacman -U`, reload user
units and start it again. Perform one action at a time and inspect failures:

```sh
systemctl --user stop omavless-runtime.service
sudo pacman -U -- /absolute/path/to/reviewed-omavless.pkg.tar.zst
systemctl --user daemon-reload
systemctl --user start omavless-runtime.service
```

Install the matching native-only frontend as above and verify the **running**
binary. This is not a claim of seamless connected upgrades, rollback across any
historical schema, or recovery from damaged ownership state.

Plugin removal and package removal are different operations:

| Action | Scope |
| --- | --- |
| `omarchy plugin remove kdk.omavless` | Removes the Omarchy frontend; the native removal observer handles its declared shutdown behavior. Does not uninstall the Arch package. |
| Confirmed Quit | Stops/disables native runtime and plugin after verified cleanup; preserves installed files and private data. |
| `sudo pacman -R -- omavless` | Removes the native package. Use only after verified shutdown, no other user's active runtime, and with the recovery archive retained. Does not remove the plugin or deliberately purge private profiles/state. |

Do not run the legacy `uninstall.sh --purge` against a native owner. It is not a
native purge or package remover, and the native-only frontend omits it. Removing
the package leaves the native ownership record intact; the frontend should
refuse operation while the executable is absent, not silently revive Python.

For attended package-only recovery, reinstall the retained compatible current
archive with ordinary `pacman -U`, reload user units, then explicitly reopen:

```sh
systemctl --user enable --now omavless-runtime.service
omarchy plugin enable kdk.omavless
```

Inspect state before reconnecting. If private state changed, ownership is
ambiguous or manual recovery is required, stop and use the recorded recovery
contract; do not force a startup. Reinstalling a compatible native archive is
not an ownership rollback to the legacy Python runtime.

## Acceptance boundary

The stable release retains scoped acceptance rather than claiming every host or
optional feature is validated. The [local R6 closure](../testing/R6_LOCAL_CLOSURE_2026-09-13.md)
records the accepted native Python-unavailable path and exact candidate identities.
Enabled fresh-login Last/pinned validation and network/DNS limitations remain
explicit follow-ups, not passing evidence.
The [package recovery procedure](../testing/R6_INSTALLED_PACKAGE_RECOVERY_2026-09-12.md)
must have actual executed results before it is called PASS. Static tests,
an opened file dialog and archive inspection cannot substitute for host gates.

Try Omarchy ARM64 evidence does not claim bare-metal or NixOS acceptance. V0's
missing protocol fixtures remain a separate maturity gap. Neither this guide
nor a local installation changes the marketplace snapshot. Local native migration
closure is not a marketplace upgrade or a promise that every optional feature
and every host environment is fully validated.
