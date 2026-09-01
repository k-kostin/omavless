# Use OmaVLESS

## Import profiles and subscriptions

The top-level clipboard and file actions use one strict classifier:

- one supported profile link opens profile review;
- one validated subscription URL opens subscription confirmation;
- ambiguous, multi-item or unsupported input fails without being stored.

The dedicated Subscriptions page remains available for management. Complete
profile credentials and subscription URLs are not written to public logs or
diagnostics.

## Connection modes

- **Full VPN** sends traffic through the selected proxy.
- **Routing** applies the selected country policy and custom rules.
- **Direct** keeps the TUN service available while bypassing the proxy.

Changing mode or Routing preset while connected validates the candidate config
and reconnects transactionally. If validation or restart fails, OmaVLESS tries
to restore the previous mode, template and service state.

## Profiles

Profiles can be selected, pinned, searched, renamed, edited, exported, shown as
a QR code and deleted. The active profile is sorted first, followed by pinned
profiles and the selected normal or latency order.

Import review exposes only bounded facts such as protocol, endpoint category,
transport, security and SNI presence. It never receives a complete UUID,
password, REALITY key or URI.

## Subscriptions

Subscriptions support add/edit/remove, per-provider refresh, refresh-all and
explicit reachability tests. Provider lists remain collapsed until opened.

Refresh accepts at most 5 MiB over normal TLS validation, parses the complete
candidate before one atomic store write and leaves the previous set unchanged
on download or validation failure. HTTPS is required except for loopback
development providers.

If refresh removes the profile currently carrying traffic, that exact profile
remains connected and is marked stale until disconnect. OmaVLESS never silently
switches the exit server.

Plain-text and standard/URL-safe base64 lists may mix supported VLESS, Trojan,
Hysteria2 and TUIC links. Clash/Mihomo YAML subscriptions are not accepted.

## Routing tools

Country presets are available for Russia, China and Iran. Custom rules can
match an exact domain, a domain plus subdomains, or an IPv4/IPv6 range and send
the match through VPN, Direct or Reject.

`Where will this destination go?` evaluates mode and custom rules locally. For
an active country policy it asks Mihomo which bounded rule matched. The check
opens one TCP connection to port 443 through Mihomo but does not load a page.

Settings can also refresh the rule providers used by the active Routing
profile and inspect a bounded, read-only projection of loaded rules. Internal
proxy/group names are mapped to `VPN`, `DIRECT` or `REJECT`.

## Login autoconnect

Settings can start the last-used or one fixed profile at login in Full VPN or
Routing mode. Direct is intentionally unavailable for autoconnect because it
would start a TUN while bypassing the proxy.

Disconnecting the current session does not silently change the saved login
preference. Turn `Start VPN at login` off explicitly when it should remain off
after reboot.

## Display and diagnostics

The active view can show uptime, a local 60-second receive/send graph and the
observed Exit IP. Exit-IP display uses `checkip.amazonaws.com` with
`api.ipify.org` fallback after connection/mode changes and explicit tests.
Disable it in Settings to make no such requests.

Live throughput in the bar is optional and disabled by default. Neither Exit
IP nor traffic samples are stored as history.

The diagnostics page reads the plugin-owned Mihomo core through its private
Unix controller. A support snapshot omits profile/subscription identifiers,
names, links, credentials, local executable paths and provider metadata.

## Keyboard controls

| Key | Action |
| --- | --- |
| `Tab` / `Shift+Tab` | move through controls owned by the open OmaVLESS view |
| `j` / `k`, arrows | move between the hero switch and profiles |
| `Enter`, `Space` | activate the focused control or profile |
| `t` | toggle the last-used profile |
| `i` | import from a file |
| `v` | import from the clipboard |
| `f` | pin or unpin the selected profile |
| `s` | open subscriptions |
| `g` | open Settings |
| `/` | search profiles, providers and endpoints |
| `p` | test the selected subscription |
| `e` | edit the selected profile |
| `n` | rename the selected profile |
| `q` | show the selected/active profile as QR |
| `x` | delete the selected profile |
| `r` | refresh |
| `d` | disconnect |
| `Esc` | close the current dialog or panel |

## Files and services

- `~/.config/omavless/profiles.json` — private profile/subscription store;
- `~/.config/omavless/route-template.yaml` — private Routing template;
- `~/.config/omavless/config.yaml` — generated Mihomo config;
- `~/.config/systemd/user/omavless.service` — generated tunnel service;
- `~/.config/systemd/user/omavless-autostart.service` — optional login setup;
- `$XDG_RUNTIME_DIR/omavless.<uid>.controller.sock` — private Mihomo controller.

Legacy prototype paths migrate without overwriting conflicting data. If old
and new files disagree, migration stops and keeps both for explicit recovery.
