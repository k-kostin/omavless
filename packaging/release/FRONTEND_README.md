# OmaVLESS 0.9.8 — fix.7 frontend

These are the Omarchy panel files for the matching **v0.9.8-fix.7** application
and DNS packages. Use the accompanying `managed-dns-delivery.json` and
`SHA256SUMS` to check source identities, architecture and archive integrity.
Checksums detect changed files; they are not signatures.

This QML frontend is common to ARM64 and x86_64. Install the native package
for your architecture from the recorded reviewed runtime source/version. A paired
frontend can have a newer source commit only when the pairing record verifies
unchanged runtime/build/package inputs; a matching version alone is insufficient.
A version label alone does not prove artifact or installed acceptance.

## Installation

The `v0.9.8-fix.7` delivery has exact package pins for both architectures.
Do not combine this frontend with the earlier immutable `v0.9.8` pair. Product
and Arch versions remain 0.9.8 / 0.9.8-1; source/hash identity selects the
correction, not the version label. Existing users may see an explicit
pacman reinstall. It is not an automatic package update. The subscription-parser
correction and native setup fixes require the corrected native packages, not only this frontend.
The panel offers guided fresh installation after explicit consent; an existing
or ambiguous installation requires the separate reviewed update/recovery path.
It must never substitute the public 0.8.2 runtime or stock Mihomo for this pair.

1. Read [native installation and recovery](docs/user/NATIVE_INSTALL.md).
2. For a fresh account with neither package installed, choose
   **Install required components**, confirm `INSTALL` and package installation,
   then `DNS` for the separate DNS-helper setup. It verifies the pinned
   downloads before normal `pacman`; it does not connect automatically.
   Close the terminal, acknowledge **Terminal and system prompts are closed**
   if shown, then **Check again**. Follow onboarding to the panel.
3. For an existing installation, first update the repository widget with
   `omarchy plugin update kdk.omavless`. This does not replace native packages.
   Follow the documented disconnected update or
   recovery procedure. Do not replace a package under a connected VPN, reset
   a private store or repeat activation. The DNS broker has its own enrollment.
   A supported pair's update has separate `PREPARE` and `UPGRADE` confirmations
   in the same terminal. Full shutdown,
   terminal closure and the later broker/application starts remain explicit.
   The original v0.9.8 broker without the idle gate needs guarded administrator
   removal/replacement, not forced automatic preparation.
4. To install this extracted frontend over a reviewed compatible pair, run
   `./install.sh` as the desktop user after confirming Rust ownership. This
   frontend-only command does not install packages or activate the runtime.

The same `./install.sh` updates an already activated native frontend without
installing a legacy fallback. The version in its manifest belongs to the native
release, as does the current source manifest. The historical marketplace
snapshot remains 0.8.2.
Do not install this archive through a marketplace listing pointing at another
commit, mix its frontend with an unverified older runtime, or run any command
from this guide with private credentials in argv.

Native runtime dependencies include the version-matched `omavless-dns` package,
systemd, libcap, iputils and bubblewrap. File selection needs zenity/kdialog/yad; editing
requires zenity, QR display qrencode, and clipboard operations wl-clipboard.
Missing helpers are reported rather than silently installed. Neither Python
nor a Rust toolchain is a runtime dependency.

Autoconnect is Off by default; enabled Last/pinned fresh-login acceptance
remains incomplete. Recorded network/DNS failures are separate open findings,
not evidence of universal connectivity. Experimental protocols retain their
existing maturity labels. NixOS and an untested architecture are not implied.

Closing the panel does not disconnect. Settings' confirmed Shut down OmaVLESS
performs the verified shutdown sequence and disables runtime/plugin startup;
it preserves installation and private profiles. Follow the guide for package
updates/removal/recovery, and keep a known-good compatible package privately.

For full removal, follow [Remove OmaVLESS](docs/user/CLEAN_REINSTALL_098.md).
It deletes OmaVLESS profiles and settings, but keeps other VPN clients and a
separately installed Mihomo. This archive includes the reviewed terminal
helper in `tools/uninstall.sh`; updating only the widget does not remove the
installed application or DNS packages.
