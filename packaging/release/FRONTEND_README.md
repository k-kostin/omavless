# OmaVLESS 0.9.0-rc.1 — native frontend candidate

This is a **release candidate**, not a marketplace update. It contains the
Omarchy QML frontend for the matching Rust package; Python is not included.
Use the accompanying `release-candidate.json` (single-source build) or
`frontend-pair.json` (reviewed unchanged runtime, newer frontend), together with
`SHA256SUMS`, to verify source identities, version, architecture and archive integrity. Checksums
detect a changed download; they are not signatures or independent trust proof.

This QML frontend is common to ARM64 and x86_64. Install the native package
for your architecture from the recorded reviewed runtime source/version. A paired
frontend can have a newer source commit only when the pairing record verifies
unchanged runtime/build/package inputs; a matching version alone is insufficient.
A `0.9.0-rc.1` version
label alone does not mean this candidate has been published or accepted on both.

## Installation

This validation RC has pinned public prerelease downloads for a matched
`omavless` + `omavless-dns` package pair. It is not a supported stable upgrade.
The panel can offer a guided fresh install after explicit consent; an existing
or ambiguous installation requires the separate reviewed update/recovery path.
It must never substitute the public 0.8.2 runtime or stock Mihomo for this pair.

1. Read [native installation and recovery](docs/user/NATIVE_INSTALL.md).
2. For a fresh account with neither package installed, use the panel's
   **Required components** action and confirm package installation, DNS
   enrollment and activation in the visible terminal. It verifies the pinned
   downloads before normal `pacman`; it does not connect automatically.
3. For an existing installation, follow the documented disconnected update or
   recovery procedure. Do not replace a package under a connected VPN, reset
   a private store or repeat activation. The DNS broker has its own enrollment.
4. To install this extracted frontend over a reviewed compatible pair, run
   `./install.sh` as the desktop user after confirming Rust ownership. This
   frontend-only command does not install packages or activate the runtime.

The same `./install.sh` updates an already activated native frontend without
installing a legacy fallback. The version in its manifest belongs to the native
candidate, as does the current source manifest. The historical marketplace
snapshot remains 0.7.0.
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
