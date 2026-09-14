# Native Rust candidate: local installation and recovery

This guide is for an explicitly reviewed **native candidate** on
Arch/Omarchy. It is not a marketplace release, automatic migration, AUR package
announcement. Its scoped [local R6 acceptance](../testing/R6_LOCAL_CLOSURE_2026-09-13.md)
does not establish a public release. The published 0.7.0
marketplace snapshot remains unchanged. Ordinary `omarchy plugin add` does not
install the native package or transfer ownership to Rust.

The native runtime/CLI does not require Python, pip, a virtual environment or
Cargo at runtime. Its Omarchy frontend is still QML. The old Python backend is
preserved separately in a frozen historical archive; remaining Python files in
main are developer test/build tools, not source installation or a runtime fallback.

Already installed? See [native everyday use](NATIVE_USAGE.md) for connection
selection, subscription refresh, language, diagnostics and Quit.

For the prepared **0.8.0-rc.1** artifact pair, verify `SHA256SUMS` and the exact
source/architecture in `release-candidate.json` before following this guide.
No RC artifact is a stable 0.8.0 release, and marketplace publication remains
owner-controlled. Both the source and assembled native frontend carry the RC
version; the published historical marketplace snapshot remains 0.7.0.

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

From the extracted **candidate frontend archive**, run `./install.sh` without
arguments: its entry point always selects native-only installation. It requires
the already activated owner and cannot install the legacy payload.

Alternatively, from the reviewed **full source checkout** matching the candidate:

```sh
./install.sh
```

This requires already committed Rust ownership, preserves the plugin's
enabled state on update, and omits installed `backend.py` and the legacy
`uninstall.sh`. Plain `./install.sh` is also the native-only update path;
`--native-only` remains a compatible alias. A missing Rust executable, legacy or
unknown ownership refuses installation before replacing the existing frontend.
It never starts Python, activates ownership, or downloads/builds a package.

Omarchy's clone-based `plugin add`/`plugin update` does not run this installer or
install the package. Do not point an unmigrated legacy installation at main:
complete the package/ownership steps first. If the package is absent or ownership
is not Rust, the source launcher refuses every action without changing private
state; adding the plugin alone cannot make the native runtime available.

To explicitly enable the runtime for future user sessions after activation:

```sh
systemctl --user enable omavless-runtime.service
```

Startup Off means this enabled service starts disconnected. Saving Last/pinned
preferences and enabling a user unit are distinct actions; neither alone proves
that an actual fresh-login autoconnect passed. Do not manually run
`login-prepare` or modify its receipt to simulate a login.

**Current native candidate:** VPN autoconnect is Off by default. Last/pinned
autoconnect is optional and its connected fresh-login validation is incomplete;
leave it Off unless deliberately testing that feature. Saving Off does not
disconnect a currently running VPN. Existing user preferences are not silently
reset by this documentation or by declaring the migration complete.

## Verify the installed candidate

Use `omavless plugin target`, `omavless status`, `omavless runtime observation`
and `systemctl --user status omavless-runtime.service` locally. The native user
service owns the single runtime; the QML frontend does not own a second core.
Private status/detail responses are not automatically shareable. Settings'
Copy report/Save report uses the bounded support projection instead.

Verify package/binary/frontend identity against the recorded acceptance result.
Replacing the on-disk executable does not upgrade an already running daemon.
Support facts also do not prove working DNS, route restoration, internet access
or successful login activation; those need their own observed checks.

## Updates, close, Quit and removal

Closing the panel or a terminal is not Disconnect. Settings' confirmed
**Shut down OmaVLESS / Quit** stops the VPN and native runtime, verifies cleanup,
then disables runtime startup and the Omarchy plugin while preserving private
profiles/settings. Failure or an unknown outcome must be inspected, not treated
as a clean shutdown. A shell reload is not this explicit Quit action.

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

This guide describes the available local installation path, not a completed
release. The [local R6 closure](../testing/R6_LOCAL_CLOSURE_2026-09-13.md)
records the accepted native Python-unavailable path and exact candidate identities.
Enabled fresh-login Last/pinned validation and network/DNS limitations remain
explicit follow-ups, not passing evidence.
The [package recovery procedure](../testing/R6_INSTALLED_PACKAGE_RECOVERY_2026-09-12.md)
must have actual executed results before it is called PASS. Static tests,
an opened file dialog and archive inspection cannot substitute for host gates.

Try Omarchy ARM64 evidence does not claim bare-metal or NixOS acceptance. V0's
missing protocol fixtures remain a separate maturity gap. Neither this guide
nor local candidate installation authorizes publication. Local native migration
closure is not a marketplace upgrade or a promise that every optional feature
and every host environment is fully validated.
