# OmaVLESS TUI application and control surfaces

Status: product/UX contract selected for a Rust + Ratatui TUI; implementation is
gated on R6 Python-runtime retirement. Updated 2026-08-30.

Implementation/migration authority:
[`RUST_MIGRATION.md`](RUST_MIGRATION.md).
Runtime/API authority: [`CONTROL_PLANE.md`](CONTROL_PLANE.md).
Host authority: [`PLATFORM.md`](PLATFORM.md).

## 1. Product shape

OmaVLESS grows from a compact Omarchy bar plugin into one VPN application with
several clients around a single runtime:

```text
                         Rust runtime
                    canonical state + lifecycle
                              |
             +----------------+----------------+
             |                |                |
             v                v                v
       Omarchy bar       Ratatui TUI        Rust CLI
       QML quick ctrl    full workspace     automation
```

A later optional desktop GUI may attach to the same API, but it does not replace
or own the TUI/runtime.

The bar remains valuable after the TUI exists. It handles frequent
connect/switch/status actions. The TUI is the deeper workspace for profiles,
subscriptions, routing, diagnostics, traffic, connections, logs and advanced
settings.

## 2. Selected TUI stack and implementation gate

The first full standalone UI is selected as:

```text
language: Rust
UI:       Ratatui
terminal: Crossterm or another reviewed compatible backend
runtime:  existing accepted Rust OmaVLESS daemon
```

This replaces earlier roadmap wording which left Python/Textual/Rust open.

The TUI is **not** the mechanism by which the backend becomes Rust. It is built
only after R6 proves:

- normal application/plugin runtime does not require Python;
- canonical daemon/domain/core behavior is Rust-owned;
- no venv/pip/Python package setup is required;
- QML plugin already controls the Rust runtime through semantic IPC/CLI.

Research, wireframes and isolated Ratatui experiments may happen earlier.
Production TUI feature work must not outrun R6.

## 3. Why the TUI is a client, not a second application

The Rust runtime owns:

- desired/actual connection state;
- `TunnelCoordinator` and owned-core exclusivity;
- Mihomo/optional future `CoreBackend` instances;
- profile/subscription mutations;
- background work;
- diagnostics/events/traffic state;
- later `LeakProtection` coordination.

The TUI submits semantic commands and renders state. It never:

- starts Mihomo directly;
- edits the canonical store behind the runtime;
- owns another desired-state machine;
- invokes Python compatibility code;
- receives the Mihomo controller secret in ordinary state.

Terminal close/crash is client loss only; it must not change a healthy requested
tunnel.

## 4. Bar plugin responsibilities

Keep the compact panel optimized for:

- current connection/core/policy status;
- connect/disconnect;
- profile/server switching;
- Full VPN / Routing / Direct when supported;
- favorites/search and quick latency context;
- compact traffic/Exit IP context;
- settings which truly belong near quick control;
- **Open app** after T2 is installed.

Do not force every future application feature into the bar during plugin
completion. Full active connections, logs, large routing/DNS workspaces and
operator diagnostics belong naturally in TUI.

## 5. Close, disconnect, quit and disable

These remain distinct:

- **Close panel** — dismiss QML popup only;
- **Close TUI/terminal** — close client only;
- **Disconnect** — persist disconnected intent and stop/verify owned core;
- **Quit OmaVLESS UI** — close selected UI only;
- **Disable/remove plugin** — unload Omarchy frontend with documented cleanup;
- **Stop runtime** — administrative action, not ordinary Quit.

Do not map `Quit OmaVLESS` to `omarchy plugin disable` or simple panel close.
The label becomes honest only after independent Rust runtime ownership exists.

At the current Omarchy baseline, plugin disable acts on the whole plugin id and
there is no supported independent widget-unload primitive that preserves the
legacy plugin-local Python owner. Therefore current plugin UX keeps the existing
semantics until R5/T1 cutover.

## 6. TUI dashboard direction

The TUI should be keyboard-first, responsive and information-dense without
copying another project's visual identity.

Wide concept:

```text
+-------------------------------------------------------------------+
| Traffic   down/up rate   totals   active connections               |
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

The dashboard should answer immediately:

1. Am I protected, and through which core/policy?
2. Which profile/source is active and what alternatives exist?
3. What is the connection doing now?
4. Is anything unhealthy, stale or blocked?

Deeper views may cover Profiles/Sources, Subscriptions, Routing, DNS,
Connections, Diagnostics/Rules, Logs and Settings.

## 7. Navigation model

Baseline keyboard behavior:

- `j` / `k` and arrows — vertical movement;
- `h` / `l` or Tab/Shift-Tab — pane/view movement where natural;
- `gg` / `G` — first/last in long lists where useful;
- `/` — local search/filter;
- `?` — shortcut overlay;
- `Esc` — close overlay/back before app exit.

Mouse support may be added after keyboard semantics are stable, especially for
selection/scrolling, but must never be required.

### Responsive layout

Narrow terminals stack/switch views without hiding critical
connection/protection state. Resize must not corrupt selection or scroll state.

### Theme

On Omarchy, follow the active semantic theme where a stable integration exists.
Standalone Arch/NixOS without Omarchy use an OmaVLESS default theme and may
later offer explicit overrides.

Theme integration is presentation-only; runtime behavior never depends on an
Omarchy theme file.

### Session continuity

Useful selection/focus may persist as bounded UI state. Reopening TUI attaches
to current runtime state immediately; it does not replay a fake connection
sequence.

## 8. Semantic CLI/integration surface

The Rust CLI is a thin client of the same control API, not a raw backend shell.
Examples:

```text
omavless status
omavless connect <profile-id> [mode]
omavless disconnect
omavless doctor
omavless tui
```

Additional commands expose stable semantic operations only. No raw Mihomo
controller, `systemctl`, shell or arbitrary config passthrough.

The QML plugin may use the packaged CLI as a compatibility bridge where direct
socket use is impractical, but both paths hit the same Rust runtime.

## 9. Launch and Omarchy integration

After T2, the plugin exposes `Open app`.

Current launcher contract:

```text
omarchy launch or focus tui --app-id=org.omarchy.omavless omavless tui
```

Requirements:

- focus existing matching TUI window where supported;
- opening TUI never starts another VPN owner/core;
- closing terminal leaves runtime/tunnel running;
- plugin/TUI/keybinding converge on same launcher/app ID;
- Omarchy/Hyprland integration changes are transactional/reversible;
- if a future Omarchy release changes launcher spelling, update only the
  frontend adapter.

If runtime is down, the TUI may request startup of packaged user service under a
bounded deadline, then negotiate `system.hello`. Failure opens doctor/remediation
view; TUI never falls back to direct Mihomo start.

## 10. Distribution boundary

The standalone Rust runtime/TUI is installed separately from the marketplace
QML plugin.

The plugin:

- detects compatible installed app/runtime;
- shows `Open app` when available;
- gives a precise host-specific install/update hint when absent/incompatible;
- never silently downloads/builds Rust sources;
- never runs Cargo for the user;
- never silently invokes package manager/sudo/pkexec.

Arch and NixOS packaging use the same primary `omavless` binary contract.

Cargo/Rust toolchain is a build dependency, not a user runtime dependency.

## 11. TUI MVP

T2 first useful release includes:

- dashboard/status;
- standalone profiles + grouped subscriptions;
- connect/disconnect and supported modes;
- search/favorites;
- single/all profile latency tests with bounded concurrency;
- live rate/totals + active connection count;
- profile details with core/capability explanation;
- subscription refresh and last-success context;
- compact local activity/log view;
- Settings/Diagnostics entry points;
- keyboard help;
- Omarchy theme following when available;
- default standalone theme otherwise.

The MVP reuses existing Rust runtime capabilities; it does not fork profile,
routing or probe logic into TUI crates.

## 12. Operator expansion

### T3

Natural TUI homes for deeper capabilities:

- D1 rules/provider diagnostics;
- C1 privacy-aware active connections;
- later separately reviewed close-connection mutation;
- richer traffic history;
- route inspection;
- bounded local application/core logs;
- read-only doctor/health report.

The bar may show compact summaries but need not duplicate full tables.

### T4

Later management candidates:

- subscription automatic refresh schedules;
- provider quota/usage/expiry metadata under strict bounds;
- private-state backup/restore;
- reconnect after suspend/network transitions;
- batch latency workflows;
- structured DNS controls;
- service-routing templates over existing custom-rule schema.

These remain structured, bounded features; no arbitrary provider YAML/config
injection.

## 13. Interaction with App proxy

S1 App proxy becomes more useful with a full application but remains a runtime/
host feature, not TUI state ownership.

It must preserve/restore exact prior proxy settings and, on Omarchy, account for
both desktop state and systemd/UWSM environment inherited by new apps. NixOS
gets its own host implementation. TUI only requests semantic enable/disable and
renders status/remediation.

## 14. Features deliberately not copied into OmaVLESS

Do not add merely because another VPN manager can:

- arbitrary raw YAML merge/patch chains;
- untrusted subscription-controlled proxy groups/rules;
- normal `$EDITOR` mutation of credential store;
- group-wide passwordless network privileges;
- client access to secret-bearing core controller;
- proxy reset to generic defaults rather than exact restore;
- protocol families solely for feature-count parity.

The strict adapter/private-store model remains a product advantage.

## 15. Delivery phases

### R0-R4 — no production TUI

Build Rust workspace/parity infrastructure and migrate control/domain/core
backend logic while current plugin remains the production UI.

### R5/T1 — Rust canonical runtime

Plugin moves to semantic Rust IPC/CLI. One runtime owns tunnel/state.

### R6 — native-runtime gate

Normal plugin/application operation succeeds without Python/venv/pip. This is a
hard TUI prerequisite.

### T2 — Ratatui MVP + Open app

Build full terminal client on already-native runtime, then add Omarchy launcher
integration.

### T3 — observability/operator expansion

Bring connections/rules/logs/doctor/traffic depth into TUI.

### T4 — management/background features

Add separately reviewed scheduling/recovery/DNS/backup conveniences.

## 16. Optional future desktop GUI

A graphical client is deliberately separate from T2.

GPUI is a preferred **research candidate** for a later Rust-native desktop UI,
but not yet a committed production dependency. It must remain another client of
the same semantic runtime.

Prefer a separate optional GUI binary/package if necessary so daemon/CLI/TUI
users do not acquire a graphics stack. Do not link GPUI into core/runtime crates
or make runtime architecture depend on GPUI lifecycle.

If GPUI later proves unsuitable, replacing the GUI toolkit must not affect
profile/store/runtime/TUI logic.

## 17. Acceptance invariants

Every TUI/control-plane PR preserves:

- one canonical Rust runtime owns active tunnel;
- plugin/TUI never start competing cores;
- no reusable credential/controller secret in ordinary state;
- slow/disconnected client cannot stall lifecycle;
- malformed IPC is bounded/rejected;
- TUI crash/close leaves desired state unchanged;
- runtime/core crash reports explicit recovery state;
- plugin disable/remove/package uninstall have explicit cleanup ownership;
- TUI contains no Python compatibility path;
- concurrent plugin/TUI actions serialize against one revisioned state;
- exact Omarchy head is tested for launch/focus, resize, theme, concurrent
  control and tunnel persistence;
- standalone Arch/Nix tests exercise same semantic TUI/runtime without requiring
  Omarchy launcher/theme behavior.

The target is not "plugin plus separate TUI". It is one native OmaVLESS product
whose compact and full clients stay consistent because they share one Rust
runtime and one validated state model.
