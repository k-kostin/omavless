# OmaVLESS — VLESS profiles in the Omarchy bar

OmaVLESS is an Omarchy bar plugin for importing, subscribing to, switching and
monitoring `vless://` profiles. Its interaction model is intentionally based on
[Omawire](https://github.com/glafeara/omarchy-wireguard): the same hero toggle,
profile rows, import/edit/rename/QR/delete actions, keyboard navigation and IPC.

The transport is different. VLESS is not a NetworkManager tunnel, so OmaVLESS
uses the installed [Mihomo](https://github.com/MetaCubeX/mihomo) binary and owns
a separate `omavless.service` plus a separate config root. It never rewrites
or automatically reads Mihoro's files.

OmaVLESS is a community project maintained by
[kdk](https://github.com/k-kostin). It is not affiliated with Omarchy,
MetaCubeX or any VPN provider. Mihoro is **not** required: OmaVLESS needs only
the Mihomo core.

## Quick start on Omarchy

### 1. Install Mihomo

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

### 2. Install OmaVLESS

```bash
omarchy plugin add https://github.com/k-kostin/omavless --enable
```

The command clones and validates the plugin, then enables its bar widget. It
does **not** run `install.sh`, install Mihomo, invoke `sudo`, grant capabilities
or start a tunnel. `omarchyplugins.com` is an independent community index; the
installation itself is handled by the Omarchy CLI.

### 3. Add a connection

Open OmaVLESS from the bar, then use `+` for one `vless://` profile or
`Subscriptions…` for a provider URL. Choose `Routing`, `Full VPN` or `Direct`,
select a profile and enable the hero switch. Profiles and subscription URLs
are stored privately under `~/.config/omavless/`.

## Requirements and security model

- Omarchy 4.x.
- Python 3, included by Omarchy.
- Mihomo, installed separately as described above.
- Optional: `zenity` for file/edit dialogs, `wl-clipboard` for paste, and
  `qrencode` for QR display.

OmaVLESS never edits `/etc/sudoers`, installs passwordless commands, invokes
`sudo`/`pkexec`, changes crontab or downloads executable code. The explicit
`setcap` command above is a one-time administrator action performed by the
user.

Marketplace security review is expected to classify this plugin as
`review-required`: the repository contains an optional manual installer, asks
the user to grant capabilities to a separately installed Mihomo binary, and
manages a user-level systemd service. These capabilities are intentional and
documented; the normal plugin installation does not exercise them silently.

## Current scope

- VLESS URI import from a file or the clipboard.
- Multiple provider subscriptions with add/edit/remove, per-provider refresh
  and atomic refresh-all. Plain-text and standard/URL-safe base64 lists of
  `vless://` links are supported; unrelated protocols are ignored. Managed
  servers stay collapsed under their provider until its row is opened.
- Manual per-provider reachability tests try every working DNS-over-HTTPS
  resolver selected by the active routing policy, reject TUN fake-IP answers,
  resolve both IPv4 and IPv6, and fall back across up to four endpoint
  addresses. An isolated temporary Mihomo core with no TUN then pins each
  candidate IP and performs full VLESS/TLS/Reality handshakes plus HTTP checks
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
- TCP, WebSocket, HTTP, H2, gRPC and XHTTP transports.
- None, TLS and Reality security, including `flow`, `sni`, `fp`, `pbk`, `sid`
  and `spx` URI parameters.
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
- Rule routing from a bundled, deterministic RoscomVPN-based TUN template. Its
  23 maintained GeoSite/GeoIP sets send RU/BY, banks and selected local
  services directly; YouTube, Telegram, GitHub and the remaining internet use
  the selected VLESS profile; selected ad/telemetry domains are rejected.
  The panel always distinguishes the routing mode (`Rule`, `Global`, `Direct`)
  from the policy source (`RoscomVPN`, `Custom`, `Basic`) and summarizes what
  will happen before connecting as well as what is active afterward.
- A compatible custom Mihomo YAML can be adopted only through an explicit
  command; OmaVLESS never discovers or imports another client's active
  configuration automatically.
- Credential-safe storage: profiles, subscription URLs, the route template and generated config
  live together under the private `~/.config/omavless/` directory and are
  written with mode `0600`; imported links and subscription URLs travel over
  stdin rather than process arguments. Subscription URLs are absent from the
  status API and are revealed only inside their explicit editor.
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

Connecting a profile enables `omavless.service`, so the selected profile starts
again after login. Disconnecting disables the service, so an intentionally
disconnected VPN stays off after reboot.

## Controls

| Key | Action |
| --- | --- |
| `j` / `k`, arrows | move between the hero switch and profiles |
| `Enter`, `Space` | connect/disconnect a profile or expand/collapse a provider |
| `t` | toggle the last used profile |
| `i` | import a VLESS link file |
| `v` | import a VLESS link from the clipboard |
| `s` | open subscriptions |
| `/` | search profiles, providers and endpoint hostnames |
| `p` | test endpoints for the selected subscription |
| `e` | edit the selected VLESS link |
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

For a custom routing config, replace the private template explicitly. The
command removes only the top-level proxy list and leaves groups, providers and
rules in place:

```bash
~/.config/omarchy/plugins/kdk.omavless/backend.sh adopt-template ~/routing.yaml
```

Existing installations keep their private routing template during plugin
updates. Explicitly switch an older `Basic` template to the bundled policy;
when a tunnel is active, the command validates the generated config and
reconnects transactionally:

```bash
~/.config/omarchy/plugins/kdk.omavless/backend.sh use-routing roscomvpn-default
```

The panel's three routing buttons change the same private template and persist
across restarts: `Routing` applies its rule sets, `Full VPN` sends all traffic
through the selected VLESS proxy, and `Direct` keeps the TUN service available
while bypassing the proxy. Changing mode while connected validates the new
configuration and reconnects transactionally; a failed switch restores the
previous mode and service state.

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
are not parsed yet; use a provider's raw or base64 VLESS subscription URL.

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

Prototype installs using `~/.config/omarchy/omavless/` and
`~/.local/state/omarchy/vless-last` are migrated automatically. Migration never
overwrites different data: if old and new files conflict, OmaVLESS stops and
keeps both copies for explicit recovery.

## Troubleshooting

Check the plugin-facing status and the dedicated service without exposing a
stored VLESS link:

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

Omarchy's normal removal command does **not** run this repository's
`uninstall.sh`. Always stop and remove the generated `omavless.service` first,
while the helper still exists, and only then remove the plugin checkout:

```bash
~/.config/omarchy/plugins/kdk.omavless/uninstall.sh
omarchy plugin remove kdk.omavless
```

Without `--purge`, private profiles, subscription URLs and the routing template
remain in `~/.config/omavless/` for a later reinstall. Use
`uninstall.sh --purge` only when those private files should also be deleted,
then run `omarchy plugin remove kdk.omavless`. The removal helper never touches
Mihoro or the Mihomo binary.

## Credits and license

The UI is a derivative of
[Omawire](https://github.com/glafeara/omarchy-wireguard) by glafeara. Its
original copyright and MIT terms are preserved in `LICENSE`, in headers on the
adapted QML files, and in `THIRD_PARTY_NOTICES.md`. The published
[RoscomVPN Routing](https://github.com/hydraponique/roscomvpn-routing) policy
model and its separately maintained GeoSite/GeoIP datasets provide the routing
source. Mihoro and omarchy-mihoro informed Mihomo service/API behavior but
their code is not bundled. OmaVLESS is MIT licensed; Mihomo and the referenced
projects remain under their own licenses.

Release notes are kept in [CHANGELOG.md](CHANGELOG.md).
