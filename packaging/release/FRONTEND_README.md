# OmaVLESS 0.8.0 release candidate — native frontend

This is an **unpublished candidate**, not a marketplace update. It contains the
Omarchy QML frontend for the matching Rust package; Python is not included.
Use the accompanying `release-candidate.json` and `SHA256SUMS` to verify the
pair's source commit, version, architecture and archive integrity. Checksums
detect a changed download; they are not signatures or independent trust proof.

## Installation

1. Read [native installation and recovery](docs/user/NATIVE_INSTALL.md).
2. Install the matching reviewed Arch package with normal `pacman -U` dependency
   checks. Do not replace a package under a connected VPN.
3. For a new user, prepare defaults with `omavless setup initialize`. For an
   existing Python installation, retain a private backup and use the documented
   compatibility/preflight path instead; never initialize over existing data.
4. Activate once while disconnected using `omavless cutover activate`. An
   already activated Rust installation must **not** activate again.
5. From this extracted frontend directory run `./install.sh` as your ordinary
   desktop user. It requires `omavless plugin target` to report `rust`; no
   package is downloaded, built, installed or activated by this script.

The same `./install.sh` updates an already activated native frontend without
installing a legacy fallback. The version in its manifest belongs to the native
candidate; the repository's separate compatibility manifest remains 0.7.0.
Do not install this archive through a marketplace listing pointing at another
commit, mix its frontend with an unverified older runtime, or run any command
from this guide with private credentials in argv.

Native runtime dependencies include separately packaged Mihomo, systemd,
libcap, iputils and bubblewrap. File selection needs zenity/kdialog/yad; editing
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
