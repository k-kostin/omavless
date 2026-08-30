# OmaVLESS platform and distribution direction

Status: T0 distribution contract accepted for Arch + NixOS and aligned with the
selected native Rust runtime path; not a claim of current standalone support.
Updated 2026-08-30.

Runtime/API migration: [`CONTROL_PLANE.md`](CONTROL_PLANE.md).
Rust implementation/retirement gates: [`RUST_MIGRATION.md`](RUST_MIGRATION.md).
Nix-specific host consequences: [`NIX_PORTABILITY.md`](NIX_PORTABILITY.md).

## Product identity

The product name remains **OmaVLESS** (`omavless`). The intended forward
description is:

> **OmaVLESS — a terminal-first VPN and proxy client for Linux, with first-class
> Omarchy integration and initial standalone support for Arch and NixOS.**

Published 0.7.0 is still an Omarchy bar plugin backed by Python and separately
installed Mihomo. The forward description becomes a support claim only after
native runtime/application and host-specific acceptance land.

## Two host families, one native application

Initial standalone targets:

- Arch Linux / Arch-based distributions;
- NixOS / future Nix-managed Omarchy host if upstream adopts that direction.

This is not generic all-Linux portability.

Dependency direction:

```text
Omarchy integration
        |
        v
Rust OmaVLESS API/runtime/TUI/CLI
        |
        v
 narrow Linux host integration
      /       \
     v         v
  Arch host   NixOS host
```

Never:

```text
OmaVLESS runtime -> Omarchy
```

A supported standalone installation uses the same native runtime/TUI/CLI
without requiring Omarchy, Quickshell or Hyprland. Omarchy adds compact bar,
launch/focus and theme integration only.

## Working assumption for Omarchy Cinque

Current upstream discussion makes a Nix-backed Omarchy plausible but not
confirmed. Planning assumes:

- package substrate may change in a future major release;
- user-facing plugin/Quickshell semantics remain materially compatible;
- packaging complexity remains hidden from ordinary users.

If upstream plugin API changes, migrate the frontend adapter. Do not redesign
Rust runtime/domain/TUI or move package/lifecycle ownership back into QML.

## Selected application/runtime implementation

The standalone target is one primary Rust program:

```text
omavless            -> Ratatui TUI by default after T2
omavless tui        -> explicit Ratatui client
omavless daemon     -> canonical Rust runtime
omavless status     -> semantic status
omavless connect    -> semantic connect
omavless disconnect -> semantic disconnect
omavless doctor     -> host/runtime health
```

Before T2, the same primary binary may expose daemon/CLI while default
interactive TUI entry remains unavailable.

The normal application package target has no runtime dependency on:

- Python interpreter;
- venv;
- pip;
- Python packages;
- Cargo/Rust compiler toolchain.

Cargo/crates are build dependencies. Users install the built package. Dynamic
system libraries remain normal host package dependencies; full static linking is
not required.

Mihomo remains an external separately packaged core. Future privileged NetGuard
remains a separate reviewed component.

## Stable shared behavior versus host integration

Shared Rust behavior across Arch/NixOS:

- profile/subscription schemas;
- strict current/future protocol adapters;
- redacted preview/credential boundaries;
- routing semantics and Mihomo config generation;
- private store/migrations;
- v1 control protocol and semantic CLI;
- desired/actual state/mutation serialization;
- Ratatui workflows and ordinary diagnostics;
- semantic core capabilities.

Host-specific boundary:

- package/install/remove hints/ownership;
- stable executable discovery;
- TUN/capability provisioning;
- user-service provisioning;
- future NetGuard host provisioning;
- host doctor/remediation;
- generation/package update behavior.

Do not build a generic distro framework. Implement exactly Arch/NixOS contracts
we can test.

Conceptual host API may cover:

```text
find_core()
core_privilege_status()
install_hint()
service_provisioning_status()
host_doctor()
```

It must not contain profile parsing, routing, subscription or UI logic.

## Shared platform assumptions

Initial native application may assume:

- Linux;
- systemd user manager;
- Linux TUN;
- XDG config/state/cache/runtime;
- packaged/verified Mihomo;
- nftables + separate NetGuard boundary only when K1 exists.

Do not assume mutable `/usr/bin` executable semantics or generic direct `setcap`
on every supported host.

## Arch host contract

Arch remains first practical standalone host.

A reviewed package may own:

```text
/usr/bin/omavless
/usr/lib/systemd/user/omavless-runtime.service
```

plus audited application resources.

Package installs the built Rust binary, not a Python source tree + virtualenv.
Mihomo remains separate. Explicit file capabilities may remain a valid Arch
core setup when reviewed/supported.

PATH/explicit stable core discovery remains useful, but AUR/pacman commands do
not belong in domain/runtime/TUI semantics.

## NixOS host contract

NixOS is not "Arch with another package command".

### Native package entry points

Nix package/module exposes stable user-facing `omavless` and Mihomo entry points
while underlying store derivations change between generations.

The Rust runtime must not persist resolved generation-specific
`/nix/store/...` paths as desired state. Resolve stable host entry points again
as needed after generation changes.

`OMAVLESS_MIHOMO` may remain development/diagnostic override, but normal Nix
package must not require copying core to `~/.local/bin`.

### TUN/capabilities

Do not apply Arch `setcap` workflow to immutable store binary. Use reviewed
Nix-native wrapper or service capability provisioning.

Ordinary Rust runtime/TUI still gets no arbitrary root shell/firewall authority.

### systemd

Lifecycle remains conceptually:

```text
omavless-runtime.service
start / stop / restart / status
```

Provisioning differs:

```text
Arch package                 NixOS package/module
     |                              |
     v                              v
packaged user unit            declarative user unit
     \                              /
      +-----------+----------------+
                  v
          same Rust runtime
```

Runtime controls service state; it does not rewrite package/declarative unit.
Legacy plugin-generated units are migration-only and removed/adopted
transactionally during R5/T1.

### generations

Acceptance includes update and rollback while disconnected and with persisted
requested state. Stale store paths must not survive as durable runtime state.

## Current plugin compatibility and migration

Current plugin is already less Arch-coupled than install docs suggest:

- protocol/config logic does not depend on pacman/AUR;
- private state uses home/XDG paths;
- lifecycle uses user-systemd;
- Mihomo discovery accepts override, `~/.local/bin` and PATH.

The application migration is therefore two related refactors:

1. Python backend -> Rust native application;
2. Arch-specific provisioning -> narrow Arch/Nix host adapters.

Neither requires rewriting QML presentation or protocol product semantics.

Known Nix portability hazard: do not permanently replace a stable profile/
wrapper executable path with its resolved generation-specific store path.

## Omarchy integration

Final relationship:

```text
OmaVLESS bar plugin
        |
        +-- status/connect/switch/mode
        |
        +-- Open app
               |
               v
       launch-or-focus terminal
               |
               v
          omavless tui
               |
               v
       Rust OmaVLESS runtime
```

`Open app` never starts a second runtime/core. Closing terminal never changes a
healthy requested tunnel.

Current launcher contract:

```text
omarchy launch or focus tui --app-id=org.omarchy.omavless omavless tui
```

Future launcher spelling changes stay frontend-only.

Omarchy-specific responsibilities:

- QML/Quickshell bar rendering;
- launch/focus/app ID;
- optional floating/keybinding integration;
- semantic theme following;
- Omarchy-specific install/help text.

Rust runtime/TUI must not require Omarchy CLI/Shell/config/theme.

## Distribution ownership

Native app is installed through reviewed host path:

- Arch package/AUR or other accepted Arch repository;
- Nix package/module/flake interface selected by R5n.

Marketplace plugin:

- detects compatible installed app;
- exposes `Open app` when available;
- gives host-specific migration/install hint when absent;
- never downloads/builds Rust source silently;
- never runs Cargo on behalf of ordinary plugin startup;
- never silently invokes package manager/sudo/pkexec.

Host privilege setup remains explicit/auditable.

## Migration from current plugin

Canonical sequence:

1. current QML + Python backend owns production;
2. R0-R4 add/migrate Rust subsystems under parity without second tunnel owner;
3. R5/T1 introduces packaged Rust runtime and performs explicit one-owner
   cutover;
4. plugin uses semantic Rust CLI/socket;
5. R6 proves normal operation with Python unavailable and retires Python runtime
   dependency;
6. only then T2 adds Ratatui client + `Open app`;
7. later app/security/core features build on Rust runtime.

At no point: two profile stores, two desired-state owners or competing cores.

## TUI portability

TUI consumes semantic control API + bounded host readiness only. No `if nixos`
branches for profile parsing, routing, lifecycle or controller behavior.

Platform-specific presentation is setup/remediation/theme availability.

Same private store schema remains portable between supported Arch/NixOS installs
subject to explicit secure backup/import policy.

## Acceptance strategy

Arch and NixOS evidence are separate.

R6 adds a cross-platform native-runtime invariant: normal supported product path
must not require Python/venv/pip.

Before NixOS support claim require at least:

- package/module install/update/remove/rollback;
- stable `omavless`/Mihomo entry points;
- user-systemd runtime lifecycle;
- TUN/capability positive/negative cases;
- Full/Routing/Direct parity where core supports them;
- store/subscription migration;
- generation rollback + GC/stale path regression;
- Nix-specific host doctor/remediation;
- Omarchy plugin/runtime/TUI integration separately if Nix-backed Omarchy
  becomes available.

## Repository direction

Keep one repository/product identity `OmaVLESS` / `omavless`.

Repository may contain Rust application workspace, Arch packaging, Nix packaging
and QML integration. Dependency direction must keep generic Rust crates free of
Omarchy/Arch/Nix frontend/provisioning imports except through narrow host traits.

Avoid premature repo splits while schema, IPC, migration and release
compatibility are evolving together.

## Future broader Linux support

Beyond Arch/NixOS is deferred. Require real package/service/capability/firewall/
proxy-integration/removal evidence and an acceptance environment before adding a
host family.

Make the Arch/NixOS native application excellent rather than claiming generic
Linux portability we cannot continuously test.
