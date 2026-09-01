# OmaVLESS

OmaVLESS is an Omarchy bar plugin for importing, subscribing to and switching
secure proxy profiles through a dedicated Mihomo TUN service.

It supports VLESS and experimental Trojan, Hysteria2 and TUIC v5 profiles,
with Full VPN, country-based Routing and Direct modes. Profiles and provider
URLs stay in a private local store, and import review hides complete
credentials and keys.

OmaVLESS is a community project maintained by
[kdk](https://github.com/k-kostin). It is not affiliated with Omarchy,
MetaCubeX or any VPN provider.

## Install on Omarchy

Add and enable the plugin:

```bash
omarchy plugin add https://github.com/k-kostin/omavless --enable
```

OmaVLESS uses an installed [Mihomo](https://github.com/MetaCubeX/mihomo)
binary. The simplest Omarchy-native installation is:

```bash
omarchy pkg aur add mihomo-bin
mihomo_bin="$(command -v mihomo)"
"$mihomo_bin" -v
```

Mihomo needs permission to create and manage the TUN interface. Review the
binary path, then grant the required capabilities:

```bash
sudo setcap cap_net_admin,cap_net_raw,cap_net_bind_service=+ep "$mihomo_bin"
getcap "$mihomo_bin"
```

Package updates can replace the Mihomo binary and remove its capabilities. If
TUN creation stops working after an update, repeat the capability commands.

Open OmaVLESS from the bar. The first-run guide checks Mihomo readiness,
offers an optional country Routing preset and opens the private import flow.
It shows host-changing commands for you to run explicitly; the plugin never
silently invokes `sudo` or `pkexec`.

For complete installation, dependency and update guidance, see the
[installation guide](docs/user/INSTALL.md).

## Use OmaVLESS

Import a supported profile link from the clipboard or a file, or add an HTTPS
subscription from the same top-level import actions. OmaVLESS always previews
whether the input is one server or a subscription before saving it.

Choose a connection mode:

- **Full VPN** sends traffic through the selected profile.
- **Routing** applies the selected Russia, China or Iran policy plus your
  custom domain/IP rules.
- **Direct** keeps the TUN service available while bypassing the proxy.

Settings provide login autoconnect, subscription management, routing tools,
safe Mihomo diagnostics, language selection, traffic display and plugin
shutdown controls.

See the [usage guide](docs/user/USAGE.md) for subscriptions, controls, routing,
autoconnect and files/services.

## Supported inputs

| Family | Status | Common supported forms |
| --- | --- | --- |
| VLESS | Supported | TCP, WebSocket, HTTP, H2, gRPC, XHTTP; TLS and REALITY |
| Trojan | Experimental | TCP, WebSocket and gRPC with TLS or supported REALITY fields |
| Hysteria2 | Experimental | `hysteria2://` and `hy2://` |
| TUIC | Experimental | TUIC v5 `tuic://` |
| Subscriptions | Supported | HTTPS raw or base64 lists of supported links |

WireGuard, native AmneziaWG and AmneziaVPN `vpn://` keys are not supported yet.
They will not be accepted until bounded import, private storage, compatibility
and real-server gates are complete.

See [supported protocols](docs/user/PROTOCOLS.md) for detailed boundaries and
known exclusions.

## Privacy and security

- Profiles, subscription URLs and generated configuration are stored under
  `~/.config/omavless/` with private permissions.
- Credentials are passed over stdin or private files, not command arguments.
- Import previews and shareable diagnostics omit complete UUIDs, passwords,
  keys and subscription URLs.
- Mihomo control uses a private Unix socket; OmaVLESS does not expose a TCP
  external controller.
- The plugin does not install packages, create administrator policy, weaken OS
  security or download executable code.
- Imported profile links are parsed into bounded supported fields; arbitrary
  remote YAML is not accepted as a profile or subscription.
- OmaVLESS refuses to compete with another detected Mihomo full-tunnel service
  and never terminates another VPN automatically.

The complete boundary is documented in the
[security and privacy guide](docs/user/SECURITY.md).

## Help

Start with:

```bash
omarchy-shell kdk.omavless status
systemctl --user status omavless.service --no-pager
```

The plugin can export a bounded support snapshot from Settings without profile
links, credentials, provider names or subscription URLs.

See [troubleshooting](docs/user/TROUBLESHOOTING.md) for Mihomo readiness, file
and clipboard import, subscription, routing, service and recovery guidance.

## Remove

```bash
omarchy plugin remove kdk.omavless
```

Removal stops the plugin-owned tunnel and services while preserving private
profiles for a later reinstall. Use the bundled `uninstall.sh --purge` before
removing the plugin only when you intentionally want to delete private data as
well.

## Release status

OmaVLESS 0.7.0 is
[published and maintainer-verified](https://omarchyplugins.com/plugin.html?id=kdk.omavless)
at exact marketplace commit `69fe05b03129a23664fff3f8289821a7b7f80095`.
Experimental labels describe protocol evidence, independently of whether an
implementation has merged.

Release notes are in [CHANGELOG.md](CHANGELOG.md).

## Credits and license

The interface builds on
[Omarchy VPN](https://omarchyplugins.com/plugin.html?id=jkoestinger.vpn) by
Justin Köstinger ([source](https://github.com/jkoestinger/omarchy-vpn)). Routing
policies use maintained data from RoscomVPN, MetaCubeX and Chocolate4U. Full
attribution and third-party terms are in
[THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).

OmaVLESS is licensed under the [MIT License](LICENSE).
