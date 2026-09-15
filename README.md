# OmaVLESS

OmaVLESS is a VPN app for Omarchy that puts everyday connection controls
directly in the bar.

Bring your own compatible VLESS profile or HTTPS subscription, choose
**Full VPN**, **Routing** or **Direct**, and connect from the panel. Switch
between profiles or disconnect without leaving the desktop; one VPN connection
is active at a time.

Expand subscriptions and refresh their server lists beside your profiles.
Profile management, routing tools, English/Russian language settings and
privacy-safe support reports stay close at hand in a compact, terminal-style
interface.

Free and open source. **OmaVLESS is not a VPN provider:** VPN servers, accounts
and service subscriptions are not included.

![OmaVLESS panel, disconnected, showing demonstration profiles and the three connection modes](docs/marketing/images/main-en.png)

*Native interface with demonstration data; no live connection is shown.*

## Installation — native 0.8.0 candidate

Current source is **0.8.0**, prepared for final acceptance but **not published**:
the Rust application plus its QML frontend.
Follow the [native installation guide](docs/user/NATIVE_INSTALL.md) to install
the reviewed package, prepare or migrate your private store, activate once,
then install the matching frontend. Python is not required at runtime.

Installing or updating the plugin alone does **not** install the runtime package
or migrate an existing Python owner. Source `./install.sh` is native-only and
refuses an absent or unactivated package; there is no Python fallback. Existing
marketplace users should follow the migration guide before updating to main.

A guided first-run installer is being validated separately in
[PR #249](https://github.com/k-kostin/omavless/pull/249). It adds missing-component
cards and explicit setup from the panel. Until reviewed packages are published,
pinned and tested through that complete path, it is **not an available
marketplace installation method**. Use the current guide above, not instructions
for an anticipated release.

This version number is not a release announcement or marketplace update. The published marketplace
snapshot remains 0.7.0; its legacy source/instructions are preserved separately.
[Release preparation](packaging/release/README.md) does not publish release assets.

The QML plugin is shared across architectures. Only the native runtime package
differs: `aarch64` for ARM64, `x86_64` for conventional Intel/AMD PCs. Use the
package matching your system and the frontend from the same reviewed source.

OmaVLESS needs Mihomo and TUN permissions. Desktop helpers provide clipboard,
file selection, profile editing and QR functions. The installation guides
explain the dependencies and explicit setup commands; OmaVLESS does not silently
install packages or grant privileges.

## Everyday use

1. Import a profile link or subscription URL from the clipboard or a file.
   Review the preview before saving.
2. Choose **Full VPN**, **Routing** or **Direct**.
3. Press **Connect** beside the profile you want. Selecting a profile for
   management is not the same as connecting it.

On the native main screen, expand a subscription to browse its profiles. Press
**↻** beside the subscription's server count to download its updated server
list. Profile management stays in the separate action bar below the list.

- **Full VPN** sends traffic through the connected profile.
- **Routing** uses the selected country policy and your domain/IP rules.
- **Direct** keeps the TUN running while bypassing the proxy.

Settings contains language selection, routing tools, diagnostics, subscription
management and shutdown controls. Changing the UI language does not restart
the VPN. Login autoconnect is Off by default; the native candidate's optional
Last/pinned fresh-login validation remains incomplete.

See [controls and everyday use](docs/user/NATIVE_USAGE.md).

[View the expanded subscription](docs/marketing/images/subscription-en.png)
· [View Settings](docs/marketing/images/settings-en.png)

## Supported inputs

| Family | Status |
| --- | --- |
| VLESS | Supported; includes TCP, WebSocket, HTTP, H2, gRPC and XHTTP, with TLS/REALITY where compatible |
| Trojan, Hysteria2, TUIC v5 | Experimental; real-server coverage is incomplete |
| Subscriptions | HTTPS raw/base64 lists of supported profile links |

Advanced VLESS Encryption/REALITY PQ and XHTTP combinations retain their
experimental evidence limits. WireGuard, AmneziaWG and `vpn://` are **not
product-enabled**; parser work is not a claim of usable VPN support.

See [protocol support and limitations](docs/user/PROTOCOLS.md).

## Privacy and help

Profiles and subscription URLs stay in a private local store. Mihomo control
uses a private Unix socket, not a TCP external controller. Imports accept
bounded supported fields, not arbitrary provider-supplied YAML or executable
configuration.

For support, use **Copy report** or **Save report** in Settings. Do not post
profile files, subscription URLs, private configuration or unredacted screenshots.

- [Troubleshooting](docs/user/TROUBLESHOOTING.md)
- [Security and privacy](docs/user/SECURITY.md)
- [Native updates, Quit and removal](docs/user/NATIVE_INSTALL.md#updates-close-quit-and-removal)
- [Release notes](CHANGELOG.md)

The published [marketplace 0.7.0 snapshot](https://omarchyplugins.com/plugin.html?id=kdk.omavless)
remains commit `69fe05b03129a23664fff3f8289821a7b7f80095`.

## Credits and license

Maintained by [kdk](https://github.com/k-kostin). Community project; not
affiliated with Omarchy, MetaCubeX or a VPN provider.

The interface builds on [Omarchy VPN](https://github.com/jkoestinger/omarchy-vpn)
by Justin Köstinger. Routing data comes from RoscomVPN, MetaCubeX and
Chocolate4U. See [third-party notices](THIRD_PARTY_NOTICES.md).

[MIT License](LICENSE).
