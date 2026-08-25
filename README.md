# OmaVLESS — secure proxy profiles in the Omarchy bar

OmaVLESS is an Omarchy bar plugin for importing, subscribing to, switching and
monitoring VLESS profiles plus experimental Trojan, Hysteria2 and TUIC v5
profiles. Its interaction model is based on
[Omarchy VPN](https://omarchyplugins.com/plugin.html?id=jkoestinger.vpn)
([source](https://github.com/jkoestinger/omarchy-vpn)): the same hero toggle,
profile rows, import/edit/rename/QR/delete actions, keyboard navigation and IPC.

These proxy protocols are not NetworkManager tunnels, so OmaVLESS
uses the installed [Mihomo](https://github.com/MetaCubeX/mihomo) binary and owns
a separate `omavless.service` plus a separate config root. It never rewrites
or automatically reads Mihoro's files.

OmaVLESS is a community project maintained by
[kdk](https://github.com/k-kostin). It is not affiliated with Omarchy,
MetaCubeX or any VPN provider. Mihoro is **not** required: OmaVLESS needs only
the Mihomo core.

## Quick start on Omarchy

### 1. Install OmaVLESS

```bash
omarchy plugin add https://github.com/k-kostin/omavless --enable
```

Open OmaVLESS from the bar. A three-step first-run guide checks Mihomo and its
TUN capabilities, offers a Russia, China or Iran Routing profile (or a skip),
then hands the private import flow your first supported profile link. The
guide never installs packages or runs privileged commands: every host setup
step is shown as an explicit copy-to-terminal action. Before a key is stored,
the import review shows its endpoint, transport, security and SNI while keeping
the complete access credential and key parameters hidden.

### 2. Prepare Mihomo

The simplest Omarchy-native route is the current
[`mihomo-bin`](https://aur.archlinux.org/packages/mihomo-bin) AUR package:

```bash
omarchy pkg aur add mihomo-bin
mihomo_bin="$(command -v mihomo)"
"$mihomo_bin" -v
```

Review an AUR package before installing it if it is not already trusted on
your system. OmaVLESS also accepts a verified upstream Mihomo binary at
`~/.local/bin/mihomo`, anywhere on `PATH`, or at the absolute path supplied in
`OMAVLESS_MIHOMO` to the environment inherited by Omarchy Shell.

Mihomo needs network capabilities to create and manage the TUN interface. This
is the only privileged setup step and is deliberately left to the user:

```bash
sudo setcap cap_net_admin,cap_net_raw,cap_net_bind_service=+ep "$mihomo_bin"
getcap "$mihomo_bin"
```

The `getcap` output should contain `cap_net_admin`, `cap_net_raw` and
`cap_net_bind_service`. Package or core updates can replace the binary and
remove those capabilities; repeat the two commands if TUN stops starting after
an update.

Already using Mihoro? Its default `~/.local/bin/mihomo` is discovered
automatically, so the AUR step can be skipped. Verify that binary and apply the
same capabilities to it. OmaVLESS will refuse to connect while Mihoro's own
`mihomo.service` is active, because two full-tunnel cores must not compete for
policy routing.

The plugin install command clones and validates the plugin, then enables its
bar widget. It does **not** run `install.sh`, install Mihomo, invoke `sudo`,
grant capabilities or start a tunnel. `omarchyplugins.com` is an independent
community index; the installation itself is handled by the Omarchy CLI.

### 3. Connect and choose login behavior

After the guide, use `+` for another supported profile or `Subscriptions…`
for a provider URL. Choose `Full VPN`, `Routing` or `Direct`, select a profile
and enable the hero switch. Profiles and subscription URLs are stored
privately under `~/.config/omavless/`.

New installs do not start a VPN automatically. From the gear-shaped general
Settings page, `Start VPN at login` can be enabled for the last used server or
one fixed server, in `Full VPN` or `Routing` mode. `Direct` is deliberately not
an autoconnect choice because it would start a TUN service while bypassing the
VPN proxy. Existing installs keep their earlier systemd-enabled behavior until
this setting is explicitly saved.

## Requirements and security model

- Omarchy 4.x.
- Python 3, included by Omarchy.
- Mihomo, installed separately as described above.
- Optional: `zenity` for file/edit dialogs, `wl-clipboard` for paste, and
  `qrencode` for QR display.

OmaVLESS creates no administrator policy files or passwordless command rules,
does not invoke `sudo`/`pkexec`, change crontab or download executable code.
The explicit `setcap` command above is a one-time administrator action
performed by the user.

Marketplace security review is expected to classify this plugin as
`review-required`: the repository contains an optional manual installer, asks
the user to grant capabilities to a separately installed Mihomo binary, and
manages a user-level systemd service. These capabilities are intentional and
documented; the normal plugin installation does not exercise them silently.

## Current scope

- A protocol-neutral internal profile adapter now owns strict extraction,
  parsing, redacted preview, Mihomo YAML generation, endpoint lookup and stable
  subscription identity. The private store migrates v1/v2 profiles to an
  explicit v3 `protocol` discriminator without changing their IDs, favorites,
  subscription membership, current selection or login target. This is a
  stable boundary shared by every supported protocol.

- VLESS URI import from a file or the clipboard. Duplicate fields,
  conflicting aliases, malformed booleans and unknown connection options fail
  before storage. Recognized provider-only `concurrency` and `x-durev-*`
  metadata remains private, is explicitly disclosed as unmapped during import
  and is never copied into Mihomo YAML.
- Experimental Trojan URI import for TCP, WebSocket and gRPC with TLS or the
  Mihomo-supported REALITY subset. Password, SNI, ALPN, certificate-check
  preference, uTLS fingerprint, WebSocket host/path, gRPC service name and
  bounded REALITY fields are translated explicitly. Unknown or unrepresentable
  share fields are rejected, and the preview never receives the password.
- Experimental Hysteria2 import for official `hysteria2://` and `hy2://`
  links. Authentication/userpass, multi-port hopping, SNI, certificate
  verification and SHA-256 pinning, ECH, and `salamander`/`gecko` obfuscation
  map to bounded Mihomo fields. Realm mode is not yet supported, and provider
  links cannot set local bandwidth.
- Experimental TUIC v5 `tuic://` import with strict UUID/password parsing,
  SNI, ALPN, certificate verification, `native`/`quic` UDP relay and bounded
  Mihomo congestion-control choices. TUIC v4 tokens and unknown connection
  knobs are rejected; 0-RTT stays off unless a future explicit local setting
  enables it.
- Multiple provider subscriptions with add/edit/remove, per-provider refresh
  and atomic refresh-all. Plain-text and standard/URL-safe base64 lists may mix
  VLESS, Trojan, Hysteria2 and TUIC links; unrelated protocols are ignored.
  Managed servers stay collapsed under their provider until its row is opened.
  VLESS row identity follows supported connection semantics rather than
  provider labels, priority hints or query ordering; legacy identities migrate
  on the next successful refresh without replacing IDs, favorites or
  active/last selections.
- Manual per-provider reachability tests try every working DNS-over-HTTPS
  resolver selected by the active routing policy, reject TUN fake-IP answers,
  resolve both IPv4 and IPv6, and fall back across up to four endpoint
  addresses. An isolated temporary Mihomo core with no TUN then pins each
  candidate IP and performs full protocol/TLS/Reality or QUIC handshakes plus HTTP checks
  against three independent connectivity targets. Its private Unix controller
  opens no TCP port, its routing mark bypasses an already active Mihomo TUN,
  and the process and credential-bearing `0600` config are removed after the
  test. The panel displays the median successful end-to-end delay; this is more
  reliable than ICMP ping because VPN hosts commonly ignore echo requests. The
  in-list `Sort by ping` control toggles fastest-first or slowest-first while
  DNS failures and unavailable servers stay at the bottom. Results are
  session-only and never affect the bar error state or automatic connection
  choice; no probe runs until the user explicitly presses `Test` or `p`.
  The panel shows elapsed time, records when the session result was measured,
  and turns the same button into `Cancel` while a test is running.
- Profile rows keep a subtle theme-native outline at rest and gain the normal
  Omarchy hover fill without pointer hover moving the scroll position.
- Any number of manual or subscription-managed servers can be pinned. The
  active server remains first, followed by pinned servers and then the normal
  name or latency order; subscription refreshes preserve pins for nodes that
  still exist. The star action appears on row focus/hover and `f` toggles it
  from the keyboard, keeping the compact list quiet otherwise.
- TCP, WebSocket, HTTP, H2, gRPC and XHTTP transports. XHTTP accepts the
  Mihomo modes `auto`, `stream-one`, `stream-up` and `packet-up`; unsupported
  values are rejected before a profile can be stored. A bounded XHTTP `extra`
  object can translate the shared Xray/Mihomo client subset for headers,
  padding, session/sequence placement, upload sizing and XMUX/reuse. JSON
  depth, count, ranges, header syntax and field names are constrained; raw or
  unknown configuration is never copied into YAML. Advanced contents stay out
  of the public preview, which reports only that advanced XHTTP is present.
  XHTTP upload/download separation can use one bounded second endpoint with
  independent path, host, headers, XMUX and TLS or Reality connection
  settings. Mihomo uses the upload mode for both directions, so a conflicting
  nested mode is rejected; nested `sockopt`, certificates and unsupported
  transport overrides are not imported.
- None, TLS and Reality security, including `flow`, `sni`, `fp`, `pbk` and
  `sid` URI parameters. Reality public keys and short IDs are checked against
  Mihomo's accepted formats before storage. Xray's `spx` spider path is
  recognized for an honest import warning but is not copied into YAML because
  current Mihomo has no corresponding client option; Xray-only ML-DSA
  verification metadata is rejected instead of being silently ignored.
- Experimental VLESS Encryption values in Mihomo's
  `mlkem768x25519plus` client format. Mode, RTT, padding, token count, encoded
  key length and total size are validated without reimplementing or exposing
  the cryptography; the complete value remains private and import review shows
  only an `experimental` marker.
- Experimental REALITY `supportX25519MLKEM768` metadata for upload and XHTTP
  download endpoints. It maps to Mihomo's `support-x25519mlkem768` flag and
  produces a device-verification warning: the selected uTLS fingerprint still
  has to offer the post-quantum group, so the flag alone is not a connection
  guarantee.
- Xray `packetEncoding=xudp` and `packetEncoding=packetaddr` UDP encodings.
  Other packet encodings are rejected instead of being copied into Mihomo
  configuration.
- Both Xray Vision share-link values are recognized. Mihomo exposes only
  `xtls-rprx-vision`, whose UDP/443 behavior corresponds to Xray's explicit
  `xtls-rprx-vision-udp443` variant; OmaVLESS preserves the imported variant
  for review but does not claim the two cores have identical QUIC behavior.
  Vision is accepted only with TCP plus TLS or Reality.
- Exclusive profile switching with Mihomo config validation before restart.
- TUN status, traffic, ping, endpoint/transport/SNI details, rename, edit,
  delete, file export and QR display. The active view also includes uptime,
  a compact 60-second receive/send graph and the externally observed Exit IP.
  That IP describes one check through the current routing mode; it is not
  presented as proof that every rule or application follows the same path.
- Search across profile name, provider and endpoint hostname, plus a concise
  bar tooltip with active profile, routing mode and Exit IP. A live throughput
  suffix can be enabled in plugin settings and is off by default to keep the
  small bar quiet.
- Best-effort detection of other TUN interfaces and common Mihomo/Xray clients
  such as Mihoro and V2RayN. It produces a neutral conflict hint only; OmaVLESS
  does not terminate or reconfigure another VPN.
- Country routing presets for Russia, China and Iran. Russia follows the
  bundled deterministic RoscomVPN DEFAULT policy. China uses Mihomo-native MRS
  sets from MetaCubeX; Iran uses Mihomo-native MRS sets from Chocolate4U.
  Mihomo downloads the selected preset's rule data at runtime and refreshes it
  on the template's bounded intervals; OmaVLESS does not bundle those remote
  databases.
- The Russia preset's 23 maintained GeoSite/GeoIP sets send RU/BY, banks and
  selected local services directly; YouTube, Telegram, GitHub and the
  remaining internet use the selected profile; selected ad/telemetry
  domains are rejected. China and Iran send their country and private-network
  destinations directly and use the selected profile for the remaining
  internet. China rejects the selected advertising category; Iran additionally
  rejects malware, phishing and browser-mining domains.
  The panel always distinguishes the routing mode (`Rule`, `Global`, `Direct`)
  from the policy source and summarizes what will happen before connecting as
  well as what is active afterward.
- A gear-shaped entry opens general OmaVLESS settings instead of adding more
  controls to the main connection view. It contains the routing-preset picker,
  routing tools, Mihomo setup status, login autoconnect, source links,
  subscription management, connection-monitoring context and privacy/display
  switches.
- Settings include simple custom routing rules for an exact domain, a domain
  and its subdomains, or an IPv4/IPv6 range. Each match can use the VPN, go
  direct or be blocked. These rules are stored in the private profile store,
  applied before the selected country preset and revalidated transactionally
  when an active connection is reloaded.
- `Where will this destination go?` first checks the selected mode and custom
  rules locally. For a country preset it asks the active Mihomo core which rule
  matched, without exposing a profile or policy-group name. This live check
  opens one TCP connection to the requested destination on port 443 through
  Mihomo but does not load a web page.
- Settings show the latest successful subscription update and the latest
  successful manual rule-data refresh. `Refresh now` asks every HTTP rule
  provider used by the active Routing profile to update, and records success
  only after all of them finish. Normal template update intervals continue to
  be handled by Mihomo itself.
- A minimal first-run guide checks the external Mihomo binary and its TUN
  capabilities, offers a skippable country Routing choice and opens the
  existing private profile import flow. Host-changing commands are copied for an
  explicit terminal action and are never executed by the plugin.
- File and clipboard imports pass through a backend-parsed review before the
  name is confirmed. The review never receives a complete UUID, Trojan
  password, Reality public key or URI; it exposes only bounded connection facts
  and, where safe, a short credential hint to distinguish similar keys.
- General Settings can export a `0600` JSON support snapshot containing plugin,
  core, service, routing and inventory state. It intentionally omits profile
  and subscription identifiers, profile/provider/server names, profile links,
  key material, local executable paths and subscription URLs.
- A compatible custom Mihomo YAML can be adopted only through an explicit
  command; OmaVLESS never discovers or imports another client's active
  configuration automatically.
- Credential-safe storage: profiles, subscription URLs, the route template and generated config
  live together under the private `~/.config/omavless/` directory and are
  written with mode `0600`; imported links and subscription URLs travel over
  stdin rather than process arguments. Subscription URLs are absent from the
  status API and are revealed only inside their explicit editor.
- The active core API uses a plugin-owned Unix socket inside the private user
  runtime directory. OmaVLESS strips inherited controller listeners and their
  secrets from an explicitly adopted template before adding this socket, so
  routing checks and rule refreshes do not expose a TCP controller port.
- Multi-monitor safe operation: every mutation is serialized by a user-runtime
  file lock, simultaneous status polls share a short-lived private cache, and
  repeated poll failures back off instead of spawning forever at full cadence.

## Development install

From this checkout:

```bash
./install.sh
```

The script validates and atomically copies the plugin to
`~/.config/omarchy/plugins/kdk.omavless`. A fresh copy is enabled in the right
bar section; an update preserves its enabled state and bar position. It does
not restart the shell or start a tunnel.

The plugin deliberately refuses to start while Mihoro's `mihomo.service` is
active: two full-route TUN cores must not compete for policy routing. Stop or
disable Mihoro only when you are ready to test OmaVLESS:

```bash
systemctl --user disable --now mihomo.service
```

On a new install, connecting starts `omavless.service` only for the current
session. The general Settings page can additionally enable
`omavless-autostart.service`, which prepares the chosen last-used or fixed
profile and starts the main service after login. Disconnecting stops the
current session without silently changing that saved login preference; turn
`Start VPN at login` off explicitly when it should remain off after reboot.
Older profile stores retain their previous enabled-service behavior until the
new preference is saved once.

## Controls

| Key | Action |
| --- | --- |
| `j` / `k`, arrows | move between the hero switch and profiles |
| `Enter`, `Space` | connect/disconnect a profile or expand/collapse a provider |
| `t` | toggle the last used profile |
| `i` | import a supported profile-link file |
| `v` | import a supported profile link from the clipboard |
| `f` | pin or unpin the selected server |
| `s` | open subscriptions |
| `g` | open general settings |
| `/` | search profiles, providers and endpoint hostnames |
| `p` | test endpoints for the selected subscription |
| `e` | edit the selected profile link |
| `n` | rename the selected profile |
| `q` | show the selected link, or the active profile from the header, as QR |
| `x` | delete the selected profile |
| `r` | refresh |
| `d` | disconnect |

## IPC

```bash
omarchy-shell kdk.omavless status
omarchy-shell kdk.omavless details
omarchy-shell kdk.omavless subscriptions
omarchy-shell kdk.omavless toggle
omarchy-shell kdk.omavless down
omarchy-shell kdk.omavless importPaste
omarchy-shell kdk.omavless importConfig ~/profile.txt
omarchy-shell kdk.omavless edit "Profile name"
omarchy-shell kdk.omavless rename "Old name" "New name"
omarchy-shell kdk.omavless qr "Profile name"
omarchy-shell kdk.omavless exportConfig "Profile name" ~/profile.url
```

The general Settings page offers a save dialog for credential-free diagnostics.
The same bounded JSON is available directly from the backend for terminal
support workflows:

```bash
plugin="$HOME/.config/omarchy/plugins/kdk.omavless/backend.sh"
"$plugin" diagnostics
"$plugin" diagnostics-export ~/omavless-diagnostics.json
```

The diagnostics command is deliberately separate from raw profile export and
does not include UUIDs, protocol passwords, keys or subscription URLs.

The same routing tools available in Settings have explicit backend commands
for troubleshooting or scripting. Domain/range values use stdin so they do
not appear in the process list:

```bash
plugin="$HOME/.config/omarchy/plugins/kdk.omavless/backend.sh"
printf '%s\n' 'example.com' | "$plugin" route-check
printf '%s\n' 'example.com' | "$plugin" custom-rule-add suffix direct
"$plugin" custom-rules
"$plugin" custom-rule-delete RULE_ID
"$plugin" rule-providers-refresh
```

Custom rules are intentionally limited to the common domain and IP cases. For
advanced Mihomo syntax, replace the private template explicitly as described
below.

For a custom routing config, replace the private template explicitly. The
command removes only the top-level proxy list and leaves groups, providers and
rules in place:

```bash
~/.config/omarchy/plugins/kdk.omavless/backend.sh adopt-template -- ~/routing.yaml
```

Existing installations keep their private routing template during plugin
updates. Explicitly switch an older `Basic` template to one of the bundled
policies; when a tunnel is active, the command validates the generated config
and reconnects transactionally:

```bash
~/.config/omarchy/plugins/kdk.omavless/backend.sh use-routing roscomvpn-default
# or: china-cn-direct / iran-ir-direct
```

The panel's three routing buttons are ordered `Full VPN`, `Routing`, `Direct`
and change the same private template across restarts. `Full VPN` sends all
traffic through the selected proxy, `Routing` applies the chosen country
rule sets, and `Direct` keeps the TUN service available while bypassing the
proxy. The first Routing click on a new installation opens a small country
chooser. Later preset changes live in general Settings and deliberately retain
the current mode. Changing a preset or mode while connected validates the new
configuration and reconnects transactionally; a failed switch restores the
previous template, preference, mode and service state.

No file under `~/.config/mihoro*` or `~/.config/mihomo/` is read or written
automatically. Passing such a file explicitly to `adopt-template` is a user
choice, not an implicit dependency.

## Subscriptions

Open `Subscriptions…` beside the profile import actions. Each provider row
shows its managed profile count, last successful update and any stale active
node. Its disclosure keeps large server lists collapsed; after a test, the
compact summary shows available, unavailable and DNS-failed counts and the
individual server rows show end-to-end latency. Refreshing downloads at most
5 MiB with normal TLS certificate
verification and never holds the VPN operation lock while waiting for the
network. The complete profile set is validated before one atomic store write;
a failed download or malformed response leaves the previous set unchanged.

If an update removes the profile currently carrying traffic, OmaVLESS keeps
that exact profile connected and labels it as stale instead of silently
changing the exit server. Disconnecting removes the stale row. Removing a
subscription is refused while one of its managed profiles is active.

Remote bearer URLs must use HTTPS; plain HTTP is accepted only for loopback
development providers. Subscription refresh is intentionally manual in this release: login and an
offline provider cannot delay tunnel startup. Clash/Mihomo YAML subscriptions
are not parsed yet; use a provider's raw or base64 supported-link subscription URL.

Exit-IP display uses `checkip.amazonaws.com` with `api.ipify.org` as a fallback
after a connection or routing-mode change and after an explicit active-tunnel
test. Disable `Show the observed exit IP` in plugin settings to make no such
requests. `Show live throughput in the bar` keeps the local TUN byte-counter
sampler active while connected; it is disabled by default. Neither feature
stores a history on disk.

## Files and services

- `~/.config/omavless/profiles.json` — private profile and subscription store.
- `~/.config/omavless/route-template.yaml` — private Rule template.
- `~/.config/omavless/config.yaml` — generated Mihomo config for the selected
  profile.
- `~/.config/systemd/user/omavless.service` — generated user service.
- `~/.config/systemd/user/omavless-autostart.service` — optional login
  preparation service.
- `$XDG_RUNTIME_DIR/omavless.<uid>.controller.sock` — private runtime-only
  Mihomo controller used for rule checks and manual rule-provider refreshes.

Prototype installs using `~/.config/omarchy/omavless/` and
`~/.local/state/omarchy/vless-last` are migrated automatically. Migration never
overwrites different data: if old and new files conflict, OmaVLESS stops and
keeps both copies for explicit recovery.

## Troubleshooting

Check the plugin-facing status and the dedicated service without exposing a
stored profile link:

```bash
omarchy-shell kdk.omavless status
systemctl --user status omavless.service --no-pager
journalctl --user -u omavless.service -n 80 --no-pager
```

If the panel says Mihoro is running, stop `mihomo.service` before connecting.
OmaVLESS never stops that service automatically. If startup fails, the backend
restores the previous generated config and service state; an error containing
`manual recovery` means that restoration itself failed and the journal should
be inspected before retrying.

## Removal

Omarchy's normal removal command does **not** run repository hooks. OmaVLESS
therefore owns a removal guard: the running service supervises its Mihomo child,
and an unloaded widget waits briefly to distinguish a hot reload/update from an
explicit disable or deletion of the installed plugin directory. Disabling or
removing OmaVLESS stops the TUN, disables login autoconnect and removes both
generated user units while keeping private profiles for an optional later
reinstall:

```bash
omarchy plugin remove kdk.omavless
```

The guard does nothing during a hot reload/update while Omarchy still reports
the plugin enabled. The bundled `uninstall.sh` remains available for explicit
pre-removal cleanup and for `--purge`. Use `uninstall.sh --purge` before
`omarchy plugin remove` only when private profiles, subscription URLs and the
routing template should also be deleted. Neither cleanup path touches Mihoro or
the Mihomo binary.

## Credits and license

The adapted interface builds on
[Omarchy VPN](https://omarchyplugins.com/plugin.html?id=jkoestinger.vpn) by
Justin Köstinger ([source](https://github.com/jkoestinger/omarchy-vpn)). Its
copyright and MIT terms are preserved in `LICENSE`, in headers on the adapted
QML files, and in `THIRD_PARTY_NOTICES.md`. The published
[RoscomVPN Routing](https://github.com/hydraponique/roscomvpn-routing) policy
model and its separately maintained GeoSite/GeoIP datasets provide the Russia
routing source. China uses
[MetaCubeX meta-rules-dat](https://github.com/MetaCubeX/meta-rules-dat), while
Iran uses
[Chocolate4U Iran-clash-rules](https://github.com/Chocolate4U/Iran-clash-rules).
Mihoro and omarchy-mihoro informed Mihomo service/API behavior but their code
is not bundled. OmaVLESS is MIT licensed; Mihomo, remote rule data and the
referenced projects remain under their own licenses.

Release notes are kept in [CHANGELOG.md](CHANGELOG.md).
