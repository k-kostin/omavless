# OmaVLESS

OmaVLESS is a VPN app for Omarchy that puts everyday connection controls
directly in the bar.

Import your own compatible VLESS profile or HTTPS subscription, choose a mode,
and connect from one compact, terminal-style panel. Open the terminal
application when you want more room.

Free and open source. **OmaVLESS is not a VPN provider:** VPN servers and
service subscriptions are not included.

<img src="docs/marketing/images/main-en.png" width="560" alt="OmaVLESS panel with country-and-city profile names, connection controls and Open app">

*Screenshots use example profiles and setup states to protect private data.
No live VPN connection is shown.*

## Get started

**OmaVLESS 0.9.8 is available for Omarchy on x86-64 and ARM64.** Start from
the plugin — no Rust build or manual package selection is needed.

### 1. Add the plugin

```sh
omarchy plugin add https://github.com/k-kostin/omavless --enable
```

Open OmaVLESS from the bar. The plugin guides you through installing the
application and VPN core next.

Already using OmaVLESS? Follow the [upgrade or migration guide](docs/user/NATIVE_INSTALL.md)
instead of repeating first-run setup.

### 2. Install the required components

If setup is needed, **Required components** shows what is missing. Press
**Install required components** to open the setup terminal.

<img src="docs/marketing/images/setup-en.png" width="560" alt="Required components showing the missing application and VPN core with a guided install button">

Type `INSTALL` when asked, approve package installation, then type `DNS` to
authorize VPN DNS setup. Enter your password only at the system's password
prompt, not at the `INSTALL` or `DNS` prompt.

The installer selects the matching packages for your architecture. When it says
**OmaVLESS is ready**, close the terminal and return to the panel. Press
**Setup terminal and prompts are closed**, if shown, then **Check again**.

Only missing components are offered. An installed application may instead need
**Finish application setup**. You can return later; setup never connects a VPN.

If the packages installed but DNS setup did not finish, use **Continue DNS
setup**, not another installation. Its terminal explains `FIRST` (first
enrollment) and `START` (use existing enrollment). After it finishes, close
the prompts, Check again, then use the separate **Start OmaVLESS** action.

### 3. Add your VPN access and connect

Follow onboarding to review the core, optionally choose a Routing policy,
and import a profile or subscription from the clipboard or a file. Review it
before saving, or choose **Finish later**.

Choose a mode and press **Connect** beside your profile. One connection is
active at a time: Connect beside another profile switches it; **Disconnect**
stops it. Clicking a profile name only selects it for management.

For manual downloads, matching packages and optional clipboard/file-picker/QR
tools, see the [installation guide](docs/user/NATIVE_INSTALL.md) and
[0.9.8 corrected delivery](https://github.com/k-kostin/omavless/releases/tag/v0.9.8-fix.7).

The command above follows upstream main. Marketplace verification covers a
separate exact snapshot and may lag behind the GitHub release.

## Which mode should I choose?

| Mode | What it does |
| --- | --- |
| **Full VPN** | Sends traffic through your connected VPN profile. Start here if you want a straightforward VPN connection. |
| **Routing** | Uses a country policy and your domain/IP rules to decide which traffic goes directly and which uses the VPN. |
| **Direct** | Bypasses the proxy while keeping the TUN interface running. This is not Disconnect. |

## Everyday controls

Expand a subscription to browse its profiles. Press **↻** beside its server
count to download an updated server list — no need to re-import the URL.

The management bar below the list offers favorite, rename, edit, QR, export,
details and delete for the selected profile. Subscription profiles keep their
editing restrictions; search does not change the connection.

Settings contains language, subscriptions, routing, diagnostics and shutdown.
English/Russian switching does not restart your VPN. Login autoconnect is Off
by default.

[View an expanded subscription](docs/marketing/images/subscription-en.png)
· [Controls and everyday use](docs/user/NATIVE_USAGE.md)

## Open the app for more room

Press **Open app** below the Profile actions bar to open OmaVLESS in a terminal.
If it is already open, the button brings you back to that same window.

<img src="docs/marketing/images/app-en.png" width="860" alt="OmaVLESS terminal application showing example profiles and keyboard controls while disconnected">

Use the wider workspace for traffic, active connections, routing, checks and
diagnostics. It shares the same VPN connection with the plugin, not a second
tunnel; both interfaces stay in sync.

**Tab / Shift+Tab** switch pages; `?` shows help. Press `q` or close the terminal
when done — **closing the window does not stop your VPN**. Disconnect stops the
connection; **Shut down OmaVLESS** in the plugin's Settings stops the application
and disables the plugin after confirmation.

0.9.8 includes encrypted **Backup** for profiles, subscriptions and supported
bundled routing templates. It is not a whole-system backup; **Restore is not
included yet**. See [Backup usage and limits](docs/user/NATIVE_USAGE.md#encrypted-backup-098-candidate).

## Supported inputs

| Family | Status |
| --- | --- |
| VLESS | Primary supported family, including compatible TCP, WebSocket, HTTP, H2, gRPC and XHTTP configurations with TLS/REALITY |
| Trojan, Hysteria2, TUIC v5 | Experimental; real-server coverage is incomplete |
| Subscriptions | HTTPS raw/base64 lists of supported profile links |

Advanced VLESS Encryption/REALITY PQ and XHTTP combinations retain their
experimental limitations. WireGuard, AmneziaWG, `vpn://` keys and arbitrary
remote Clash/Mihomo YAML are not supported inputs.

See [protocol support and limitations](docs/user/PROTOCOLS.md).

## Privacy and help

Profiles and subscription URLs stay in a private local store. For support,
use **Copy report** or **Save report** in Settings. Do not post profile files,
subscription URLs, private configuration, backup archives or unredacted screenshots.

- [Troubleshooting](docs/user/TROUBLESHOOTING.md)
- [Security and privacy](docs/user/SECURITY.md)
- [Updates, Quit and removal](docs/user/NATIVE_INSTALL.md#updates-close-quit-and-removal)
- [Release notes](CHANGELOG.md)

## Credits and license

Maintained by [kdk](https://github.com/k-kostin). Community project; not
affiliated with Omarchy, MetaCubeX or a VPN provider.

The interface builds on [Omarchy VPN](https://github.com/jkoestinger/omarchy-vpn)
by Justin Köstinger. Routing data comes from RoscomVPN, MetaCubeX and
Chocolate4U. See [third-party notices](THIRD_PARTY_NOTICES.md).

[MIT License](LICENSE) · [Contributing](docs/README.md#developing-and-reviewing)
