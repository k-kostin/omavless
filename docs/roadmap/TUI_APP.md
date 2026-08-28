# OmaVLESS TUI application and control surfaces

Status: T0 product/architecture contract accepted; no shared runtime or TUI is
implemented yet. Updated 2026-08-28.

The implementation-ready T1 IPC, ownership, migration and distribution
contract is [`CONTROL_PLANE.md`](CONTROL_PLANE.md).

OmaVLESS should be able to grow from a compact Omarchy bar plugin into a
full VPN application without creating a second owner of the tunnel or copying
business logic into every UI. The intended product is one runtime/control plane
with several deliberately different control surfaces:

```text
                         OmaVLESS runtime
                    canonical state + lifecycle
                              |
             +----------------+----------------+
             |                |                |
             v                v                v
       Omarchy bar          TUI app          CLI/IPC
       quick control      full workspace     automation/
                                             integration
```

The bar plugin remains valuable after the TUI exists. It is the fast control
surface for normal connect/switch/status actions. The TUI is the deeper operator
surface for sources, subscriptions, routing, diagnostics, traffic, connections,
logs and advanced settings. Neither UI owns a second proxy core.

## 1. Product split

### Bar plugin

Keep the compact panel optimized for frequent actions:

- current connection/core/policy status;
- connect/disconnect;
- profile/server switching;
- Full VPN / Routing / Direct when the active core supports them;
- favorites/search and quick latency information;
- compact traffic/Exit IP context;
- settings that need to be immediately reachable;
- an explicit **Open app** action once the TUI package is available.

Do not move every future feature into the panel. A small bar surface should not
become a compressed desktop application.

### Close, disconnect, quit and disable are different actions

The current plugin must keep these lifecycle terms precise:

- **Close panel** dismisses only the open popup. The bar widget remains loaded
  and the requested VPN state is unchanged.
- **Disconnect** stops the requested VPN connection. It does not remove the
  plugin UI.
- **Disable/remove plugin** unloads the Omarchy plugin and performs the existing
  fail-safe runtime cleanup, including stopping its owned tunnel and removing
  generated user units where applicable.
- **Quit OmaVLESS** is reserved for closing or unloading an application/UI
  surface while leaving a healthy requested tunnel owned by the canonical
  runtime.

Do not add a `Quit OmaVLESS` row to the current plugin Settings page by mapping
it to `Panel.close()` or `omarchy plugin disable`. The first would leave the bar
surface present and merely close its popup; the second would intentionally run
the disable/removal cleanup and disconnect the tunnel. At the accepted Try
Omarchy 4.0.0.alpha baseline, the Omarchy plugin API enables or disables the
plugin id as a whole; it does not expose a separate supported primitive for
unloading only this bar widget while retaining its plugin-local `Service.qml`
owner.

Baseline evidence: Try Omarchy pins
[`basecamp/omarchy@7488ead`](https://github.com/basecamp/omarchy/tree/7488eaded43de68ff9d2d7e4bf50cd48e112eb0f);
its [`omarchy-plugin-disable`](https://github.com/basecamp/omarchy/blob/7488eaded43de68ff9d2d7e4bf50cd48e112eb0f/bin/omarchy-plugin-disable)
operates on the complete plugin id, while
[`omarchy-bar`](https://github.com/basecamp/omarchy/blob/7488eaded43de68ff9d2d7e4bf50cd48e112eb0f/bin/omarchy-bar)
offers put/move/settings operations but no independent widget-unload command.

`Quit OmaVLESS` becomes implementable only after T1 gives the tunnel an owner
whose lifetime is independent of the bar/TUI surface, and after the frontend
has an honest close/unload and re-entry path. Its later acceptance check must
start with a healthy active tunnel, quit the selected UI surface, and verify
that exactly one runtime, one core and one TUN remain healthy before reopening
the UI. A combined `Quit & Disconnect` action is not part of the baseline UX;
the existing explicit Disconnect action already owns that intent.

### TUI application

The TUI is the full local management workspace. Its first useful shape should
be keyboard-first and information-dense while still following Omarchy visual
semantics.

A wide-screen dashboard may use a structure similar to:

```text
+-------------------------------------------------------------------+
| Traffic   up/down rate   totals   active connections               |
+------------------------------+------------------------------------+
| Sources                      | Live activity / logs                |
|                              |                                    |
| Standalone profiles          |                                    |
| Subscriptions                |                                    |
|   provider A                 |                                    |
|     node 1                   |                                    |
|     node 2                   |                                    |
|   provider B                 |                                    |
|                              |                                    |
+------------------------------+------------------------------------+
| CONNECTED | mode | core | protection | DNS/policy | active profile |
+-------------------------------------------------------------------+
```

This is a product layout concept, not a pixel specification. The application
must have its own visual identity rather than reproducing another TUI.

The dashboard should answer four questions immediately:

1. Am I protected and through what core/policy?
2. Which profile/source is active and what alternatives are available?
3. What is the connection doing right now?
4. Is anything unhealthy, stale or blocked?

Deeper pages/views can then cover Profiles/Sources, Subscriptions, Routing,
DNS, Connections, Diagnostics/Rules, Logs and Settings without making all of
them permanent panes on the dashboard.

### CLI / integration surface

A small machine-readable CLI should expose semantic actions to the bar plugin,
scripts and future automation without exposing a core API secret or requiring
every client to understand Mihomo/Xray internals.

Examples of eventual semantic operations are:

```text
status
connect <profile-id> [mode]
disconnect
set-mode <mode>
test <profile-id>
refresh-subscription <id>
open-app
```

Names are illustrative. The contract should be designed from the API boundary,
not copied from current `backend.py` command names.

## 2. One canonical runtime, not two applications

The most important architectural rule is that the TUI must not become a second
VPN manager next to the plugin.

The future control plane owns:

- canonical desired/actual connection state;
- `TunnelCoordinator` and owned-core exclusivity;
- Mihomo/Xray `CoreBackend` instances;
- `LeakProtection` when configured;
- profile/subscription mutations;
- background subscription/rule-data scheduling;
- reconnect/recovery decisions;
- bounded runtime diagnostics and event streams.

The plugin and TUI submit commands and render state. Closing either UI must not
implicitly stop a healthy VPN.

This also creates a clean relationship with future Xray support: the TUI never
needs to know how to start Xray versus Mihomo. It asks the runtime to connect a
validated profile and renders the selected core/capabilities returned by the
runtime.

## 3. Daemon/control-plane direction

A persistent or systemd-managed headless process becomes justified when the TUI
track starts because background behavior must survive UI lifetime. Whether the
first implementation reuses Python or introduces a compiled component is an
implementation decision; the IPC contract should not depend on that choice.

### Responsibilities

The daemon/control plane should eventually own:

- profile/store loading and migrations;
- operation serialization;
- core config generation/validation/start/stop;
- canonical connection state;
- traffic/accounting sampling required by multiple clients;
- scheduled subscription refresh;
- suspend/resume and selected network-change recovery;
- diagnostics sources and logs;
- state needed by bar/TUI clients.

Do not keep a background daemon merely to redraw a terminal. Every persistent
responsibility should have a product reason.

### Private IPC

T0 selects `$XDG_RUNTIME_DIR/omavless/control.sock`, with a mode-`0700` parent,
mode-`0600` socket and same-UID peer verification. V1 uses bounded UTF-8 NDJSON,
one unary request/response per connection, explicit negotiation, per-start
instance IDs, revisions and a separate bounded event stream. Exact framing,
limits, methods and stable errors are defined in
[`CONTROL_PLANE.md`](CONTROL_PLANE.md).

The API should use semantic commands rather than forwarding arbitrary UI key
presses. That allows QML, TUI, CLI and tests to share the same contract.

The daemon should expose at least two conceptual data classes:

- small bounded state snapshots/events suitable for bar/TUI status;
- explicit high-volume/local-only views for logs and active connections.

Do not push an unbounded log or connection list inside every status snapshot.
Use separate bounded reads/streams and backpressure so a stalled UI cannot block
VPN lifecycle work.

### Credentials and local privacy

A local socket is not permission to abandon redaction rules.

- reusable profile credentials and subscription URLs remain outside ordinary
  state snapshots;
- the plugin never receives a Mihomo/Xray controller secret;
- core-controller access remains owned by the runtime;
- local operator views may expose more runtime detail than shareable
  diagnostics, but exports remain explicitly redacted;
- raw/local logs are never silently included in a diagnostic bundle;
- controller/core-controlled text remains bounded before reaching any UI.

## 4. Launch and Omarchy integration

The bar plugin should eventually expose **Open app**.

On Omarchy 4 the behavior is launch-or-focus rather than spawning unlimited
terminal windows. T0 selects the supported Omarchy terminal launcher and a
stable app ID:

```text
omarchy launch or focus tui --app-id=org.omarchy.omavless omavless tui
```

The TUI attaches to `omavless-runtime.service`; it never starts a core directly.
Daemon-down behavior and bounded remediation are defined in the control-plane
contract.

Requirements:

- if a TUI window already exists, focus it;
- opening the app never starts a second VPN runtime;
- closing the terminal leaves the daemon/tunnel running;
- plugin, TUI and optional keybinding all converge on the same launcher;
- installation changes to Omarchy/Hyprland configuration must be transactional
  and reversible if such changes are needed at all.

## 5. Distribution boundary

The current marketplace plugin must keep its existing security model: plugin
installation does not silently download/build/execute a new native binary or
run privileged setup.

A future compiled TUI/runtime therefore needs an explicit distribution story
before implementation. Acceptable directions include a separately installed
Arch/AUR package or a separately verified release binary. The repository may
contain both the plugin integration and application source, but installation
boundaries must remain explicit.

The bar plugin should detect whether the full app is installed:

- installed -> show/enable `Open app`;
- absent -> show a clear installation/setup hint;
- never download or build it silently from QML.

Do not select Rust, Python or another implementation language in the product
contract. A compiled Rust/ratatui client is attractive for a native TUI, while
reusing the current Python backend lowers migration cost; the daemon IPC boundary
exists partly so these decisions can be made independently.

## 6. TUI interaction model

### Navigation

The TUI should be usable without a mouse:

- `j` / `k` and arrow keys for vertical movement;
- `h` / `l` or Tab/Shift-Tab for pane/view movement where natural;
- `gg` / `G` for first/last in long lists where useful;
- `/` for local search/filter;
- `?` for a complete shortcut overlay;
- `Esc` closes an overlay/goes back before it exits the app.

Mouse support is desirable after keyboard semantics are stable, particularly
for selecting views/rows and scrolling, but it must not become required.

### Responsive layout

A floating terminal can be narrow. Wide layouts may use side-by-side sources
and activity; compact layouts must stack or switch views without hiding critical
connection/protection state. Terminal resizing must not corrupt selection or
scroll state.

### Omarchy theme

Follow the active Omarchy semantic palette by default and update while the TUI
is running. A later explicit override may be offered, but fresh Omarchy installs
should visually belong to the desktop without shipping a permanently unrelated
palette.

### Session continuity

Selection/focus that is useful across TUI reopen can live in the daemon or
small UI state; ephemeral text selection remains client-local. Reopening the
TUI should immediately attach to the current tunnel rather than replaying a fake
connect flow.

## 7. TUI feature sets

The application should reuse existing OmaVLESS capabilities rather than fork
them into app-only implementations.

### TUI MVP

- dashboard/status described above;
- standalone profiles plus subscription grouping;
- connect/disconnect and supported routing modes;
- search/favorites;
- one/all profile latency testing with bounded concurrency;
- live rate/totals and active connection count;
- profile details with core/capability explanation;
- subscription refresh and last-success context;
- compact local activity/log view;
- open Settings/Diagnostics views;
- keyboard help and Omarchy theme following.

### Observability/operator expansion

These existing/future tracks fit naturally in the TUI and should not be forced
into the bar panel:

- D1 rules/provider diagnostics;
- C1 privacy-aware active connections and later explicit close-connection
  mutation;
- richer traffic history;
- route inspection;
- local core/application logs with session-aware navigation, selection and copy;
- read-only `doctor`/health report with remediation hints.

The TUI can expose raw local operational detail which would be noisy in the
plugin, while shareable diagnostic export keeps the stricter redacted boundary.

### Profile/subscription quality-of-life candidates

Consider after the shared runtime exists:

- per-subscription automatic refresh schedules;
- explicit subscription quota/usage/expiry metadata when supplied by a
  provider and parsed under strict bounds;
- transactional private-state backup/restore with clear credential warnings;
- reconnect after suspend and selected network transitions;
- batch latency tests;
- explicit copy/export actions for user-selected reusable credentials;
- service-routing templates as a convenience layer over the existing structured
  custom-rule model.

These are candidates, not permission to accept arbitrary provider configuration.

### DNS controls

A full TUI is a reasonable future home for structured DNS controls such as
resolver presets, strategy and fake-IP behavior. Any implementation must map to
a bounded OmaVLESS-owned schema and preserve the selected country-routing threat
model. Do not expose raw core DNS YAML/JSON as a shortcut.

## 8. Features deliberately not copied into OmaVLESS

The TUI direction does not relax the project's security/product boundaries.

Do not add merely because another VPN manager can:

- arbitrary raw YAML merge/patch chains;
- arbitrary proxy-group/rule injection from untrusted subscription metadata;
- direct `$EDITOR` mutation of the credential store as the normal UI;
- permanent group-wide passwordless network privileges;
- a bar plugin that talks directly to a secret-bearing core controller;
- a system-proxy toggle that resets to generic defaults instead of restoring the
  exact previous host state;
- protocol families solely to increase the supported-protocol count.

The existing strict profile adapter and private-store model remains a product
advantage and should be preserved in the TUI.

## 9. Interaction with App proxy / system proxy

The separate S1 **App proxy without TUN** track becomes more useful once a full
application exists, but its threat/recovery model remains independent from the
TUI.

On Omarchy, applications may learn proxy state from more than one place. S1
must investigate both desktop proxy settings and the systemd/UWSM user-manager
environment used by newly launched applications. Any applied state must be
owned, transactional and restore the exact previous values on disable/removal;
blindly switching a desktop proxy to `none` or unsetting environment variables
is not sufficient if the user had pre-existing proxy configuration.

## 10. Delivery phases

### T0 — control-plane and distribution design

State: **accepted design checkpoint** in
[`CONTROL_PLANE.md`](CONTROL_PLANE.md).

- `omavless-runtime.service` is the one future canonical owner;
- v1 private socket framing, methods, errors, revisions and mutation
  serialization are concrete;
- persistent/desired/actual/cache/log state classes are separated;
- Stage 0–3 migration, rollback and restart reconciliation are specified;
- Arch/AUR packaging and exact Omarchy launch-or-focus behavior are selected;
- T1 has a concrete deterministic and Try Omarchy acceptance plan.

No daemon or TUI runtime is advertised at T0.

### T1 — shared runtime / daemon foundation

State: planned after T0 and only when scheduled into the active delivery chain.

- make one headless runtime the canonical connection owner;
- migrate the smallest necessary lifecycle/store/status operations behind it;
- expose private IPC plus a small machine-readable CLI bridge;
- keep the existing plugin behavior working through the new boundary;
- TUI/plugin exits do not stop a requested healthy tunnel;
- add background suspend/resume recovery and scheduling only after their state
  ownership is explicit;
- this phase should satisfy or subsume the relevant N0 `TunnelCoordinator`
  extraction rather than creating two competing lifecycle abstractions.

### T2 — TUI MVP + Open app

State: planned after T1.

- add the full TUI package/client;
- implement dashboard, sources/subscriptions, connect/mode, latency, traffic,
  settings/diagnostics entry points and help;
- add Omarchy theme following and responsive keyboard-first layout;
- add `Open app` launch-or-focus integration in the plugin;
- prove plugin and TUI concurrently observe/mutate the same canonical state;
- closing/reopening either surface must not disturb the tunnel.

### T3 — operator/observability integration

State: planned after T2 and the prerequisite feature tracks.

- integrate D1 diagnostics cleanly into TUI views;
- integrate C1 active connections when implemented;
- add bounded logs/activity views, richer traffic, route inspection and
  read-only doctor/health output;
- preserve local-detail versus shareable-redacted-output distinction.

### T4 — background and management quality of life

State: later product work, split into independent PRs.

Candidates include subscription schedules, provider quota/expiry presentation,
backup/restore, suspend/network recovery hardening, structured DNS controls and
routing-service templates. Each candidate keeps its own threat model and real
Omarchy acceptance gate.

## 11. Acceptance invariants

Every TUI/control-plane PR must preserve these invariants:

- one canonical OmaVLESS runtime owns each active tunnel;
- plugin and TUI never start competing core instances;
- no reusable credential or controller secret is broadcast in ordinary state;
- one slow/disconnected client cannot stall tunnel lifecycle or background work;
- malformed IPC is bounded and rejected without crashing the daemon;
- a TUI crash/terminal close leaves VPN desired state unchanged;
- daemon/core crashes produce explicit recovery state rather than silently
  claiming protection;
- plugin disable/remove and package uninstall have documented ownership cleanup;
- exact Omarchy head is tested for launch/focus, resize, theme change,
  concurrent plugin/TUI control and tunnel persistence before merge.

The target is not "a plugin plus a separate TUI". The target is one OmaVLESS
product whose small and large interfaces remain consistent because they share
the same validated state and lifecycle owner.
