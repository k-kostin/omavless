# Native OmaVLESS: everyday use

> Development guide for the proposed **0.9.9** snapshot. For the published
> release, use the [current main guide](https://github.com/k-kostin/omavless/blob/main/docs/user/NATIVE_INSTALL.md).

For the accepted Rust-owned application with its matching Omarchy frontend.
Start with [native installation](NATIVE_INSTALL.md) if the runtime is not yet
installed and activated. This guide does not announce a marketplace release.

## Connect the intended profile

Use **Connect** directly beside the profile name. **Disconnect** appears on the
connected row; the connected profile is also named above the main controls, even
when its subscription is collapsed or filtered out.

Clicking a profile name selects it for the fixed management action bar below
the list. That selection alone does not switch the VPN. Favorite, rename,
editor, QR, export, details and delete act on the named management target.
Subscription-managed profiles retain their editing restrictions.

Use the main **Full VPN / Routing / Direct** buttons to choose the mode. A mode
change while connected can require OS authorization and a connection transition;
finish or cancel the current prompt before starting another operation. An
uncertain or rejected transition is not proof that the requested mode is active.

## Import and update subscriptions

The main clipboard and file buttons accept a supported profile link/configuration
or one supported subscription URL, then open the corresponding confirmation.
They do not accept an arbitrary remote configuration file.

In the main profile list:

- The arrow beside a subscription expands or collapses its profiles.
- **↻**, beside the server count, means **Update server list**. It fetches that
  subscription's current feed; it is not a latency test or a VPN reconnect button.
- The normal completion/error message reports the result. Wait for completion
  before another mutation; an unknown outcome needs reconciliation, not repeated
  clicks.

Open **Subscriptions**, then open the subscription you want for its refresh,
edit, remove and test actions. **Update all** refreshes the subscriptions as a
batch. Removing a subscription also removes its managed profiles; use the
confirmation carefully. Provider access can fail on a particular network;
a failed fetch is not evidence that a new server list was saved.

## Language, navigation and diagnostics

Settings → **Language** cycles System, English and Russian. The change applies
in place without restarting the plugin, runtime or tunnel. Private profile and
provider names are not translated.

Tab / Shift+Tab move through the open panel's controls. Enter or Space activates
the focused control; arrows move the profile-list cursor. Escape backs out of
the current view or closes the panel. The search field filters profiles without
changing the connection. Legacy-only letter shortcuts are not a promise of
native keyboard parity; visible controls remain the primary action path.

Use Settings for routing tools, Mihomo diagnostics and **Copy report / Save
report**. Reports use a bounded privacy-safe projection. Raw CLI status and
configuration files can contain private metadata and are not shareable reports.
The main screen offers explicit HTTPS Test and bounded Ping measurements.
Read the result and age; an unavailable result alone does not identify a bad
provider, key or core. Batch profile/subscription checks use HTTPS delay.

## Open the terminal application

OmaVLESS 0.9.9 includes the terminal application. **Open app** appears below the fixed
Profile actions area on the main panel. It opens the terminal application or
focuses its existing window. It does not start a second VPN or automatically
start a stopped runtime. If unavailable, follow the displayed package/setup
guidance rather than starting another core manually.

In the terminal, Tab / Shift+Tab switch pages; `?` shows help. Select a profile
with arrows, then `c` to connect; `d` disconnects. `1` / `2` / `3` select Full VPN,
Routing or Direct. Network changes require a separate Enter confirmation;
Escape cancels. The connected identity is independent of the selected row.

On Subscriptions, `n` / `p` select the source, `s` refreshes it and `S` refreshes
all. `r` only reloads the local view. On Profiles, `t` checks the selected
profile and `T` checks all available profiles. These are HTTPS delay checks,
not ICMP or whole-system DNS/leak tests. On Checks and updates, `x` requests
cancellation; wait for the runtime's confirmed result. Unknown outcomes must
not be retried as a new action blindly.

`,` opens session Settings for language and theme. Closing with `q` or closing
the terminal leaves the VPN and accepted background work running. The plugin
remains available for import, editing and routing management.

Read-only operator pages include Connections, Host, Rules,
Providers, Custom rules and Route check. These show local or controller facts,
not a blanket proof of working Internet, DNS or traffic protection.
Route check sends only an explicitly entered query and clears it when leaving.
Traffic includes a short in-session trend and identifies discarded older
activity. Log collection has a bounded end state; explicit route checks show
result age. These are observations, not network-control actions or independent
proof that all traffic is protected. Connections distinguishes a loading first
result from a completed unavailable read.
Settings also shows the installed plugin's version and author below Shut down
OmaVLESS. This identifies the plugin frontend, not necessarily the separately
installed native package.

On Subscriptions, `u` explicitly requests provider-reported usage and expiry
for the selected source. This private claim is not measured VPN traffic or
verified account entitlement; a provider may supply no usable information.
Leaving the page discards the result. It does not refresh server lists, change
the VPN or include the claim in ordinary diagnostic reports.

<a id="encrypted-backup-098-candidate"></a>

## Encrypted Backup and Restore (0.9.9)

In the terminal application, press `b` or F2, or open Backup from session
Settings. Enter a new absolute file path and repeat a passphrase of at least
12 bytes. Enter reviews the request; a second explicit Enter creates the file.
Escape cancels before submission. Existing files are never overwritten.

Use an existing private folder owned by you (permissions `0700`); a shared or
ordinary public folder is refused. Your private home directory is suitable.
The new archive is created with permissions `0600`.

Backup contains profiles, subscriptions and a supported bundled routing
template, not all Settings or OS state. Custom or nonstandard templates may be
refused; a refusal is not permission to reset your routing configuration.
Keep both the archive and passphrase private: the encrypted archive contains
reusable VPN access, and a lost passphrase cannot be recovered.

In session Settings, `R` opens Restore when safely disconnected and idle. Select
the private archive and enter its passphrase; review the authenticated preview
before the separate replacement confirmation. It replaces the displayed
profiles/subscriptions and routing template, not every Setting or OS state.
If connected, Disconnect is a separate confirmed action; wait for fresh Off and
open Restore again. If a submitted operation has an unknown outcome, preserve
the input and follow recovery guidance; do not resubmit it or assume closing the
client cancels it. Restore does not promise repair of every unsupported or
interrupted state. See [the full procedure](NATIVE_INSTALL.md#restore-and-interrupted-operations).

Settings and Traffic also offer local **Recorded traffic**, Off by default.
Enable, disable and reset need confirmed Off. Totals survive an ordinary app
restart; disabling keeps them, resetting discards the period after confirmation.
Partial recording is labelled, not filled with invented values. These tunnel
totals are separate from provider allowance and profile Backup/Restore.

Subscription refresh is manual by default. The explicit local CLI Daily
schedule refreshes server lists without connecting. Use `omavless --help` for
the supported `subscription schedule` commands; missed days are not replayed.

## Close, disconnect, Quit

| Action | Effect |
| --- | --- |
| Close panel / restart shell | Leaves the requested tunnel running |
| Close terminal application | Leaves the requested tunnel and accepted background jobs running |
| Disconnect | Stops the current VPN connection; keeps the plugin/runtime available |
| Disable the plugin through Omarchy | After a short reload grace, requests verified application shutdown and disables runtime startup. Re-enable the plugin and explicitly start the application; private data stays saved. |
| Settings → Shut down OmaVLESS / Quit | Confirms shutdown, verifies core/TUN cleanup, then stops/disables runtime and plugin; preserves private data and installed files |

If shutdown fails or needs recovery, do not interpret the closed UI as proof
of successful cleanup. See [native recovery](NATIVE_INSTALL.md#updates-close-quit-and-removal).

Login autoconnect is **Off by default**. When deliberately enabled, Last or a
pinned profile applies at a new login. Confirmed Disconnect in the current login
is retained across an application restart. Changing the saved login preference
does not disconnect the current session. After a network change, verify current
state and access; a prior check is not a new result.

## Service identity

The native user service is `omavless-runtime.service`, not the legacy
`omavless.service`. The runtime owns Mihomo; the panel is a client. For a local
service check, use:

```sh
systemctl --user status omavless-runtime.service --no-pager
```

Do not share unreviewed service/log output. Package removal and frontend removal
are separate operations; the legacy `uninstall.sh --purge` is not a native
removal command. Follow the [native guide](NATIVE_INSTALL.md).
