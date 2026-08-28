# OmaVLESS platform and distribution direction

Status: T0 distribution contract accepted, not current standalone runtime
support. Updated 2026-08-28.

The complete runtime/API/migration contract is
[`CONTROL_PLANE.md`](CONTROL_PLANE.md).

## Product identity

The product name remains **OmaVLESS** (`omavless`) even as protocol support and
interfaces expand beyond the original VLESS-only scope.

The intended product description is:

> **OmaVLESS — a terminal-first VPN and proxy client for Arch Linux, with
> first-class Omarchy integration.**

The current published 0.7.0 build is still delivered as an Omarchy bar plugin
and uses the separately installed Mihomo core. The statement above describes
the forward product boundary; it must not be presented as already implemented
until the standalone application/runtime phases land.

## Arch-first, not Omarchy-dependent

The standalone application should target Arch Linux and Arch-based
distributions first. Omarchy is the first-class desktop integration and the
initial environment in which the project is developed and acceptance-tested,
but it must not become a dependency of the runtime.

The dependency direction is:

```text
Omarchy integration
        |
        v
   OmaVLESS API/runtime
```

and never:

```text
OmaVLESS runtime
        |
        v
     Omarchy
```

A normal Arch installation must be able to use the same runtime, TUI and CLI
without installing Omarchy, Quickshell or Hyprland.

## Standalone command surface

The planned native application has one product command. T0 fixes the semantic
shape while leaving nonessential presentation flags to implementation:

```text
omavless            -> interactive TUI by default
omavless tui        -> explicit interactive TUI
omavless daemon     -> headless canonical runtime (normally systemd --user)
omavless status     -> bounded human/machine-readable status
omavless connect    -> semantic connection command
omavless disconnect -> semantic disconnect command
omavless doctor     -> read-only host/runtime health checks
```

Additional subcommands should be added only when they expose stable semantic
operations. They must not turn the public CLI into a thin pass-through for raw
Mihomo/Xray APIs or arbitrary shell/networking commands.

The TUI does not care how it was launched. A terminal, shell script, launcher,
Hyprland keybinding or Omarchy bar action all invoke the same client and attach
to the same canonical runtime.

## Runtime platform assumptions

The first supported standalone platform is deliberately narrower than "all
Linux". Initial design and packaging may assume:

- Linux;
- Arch Linux or an Arch-based distribution;
- systemd with a user manager;
- Linux TUN support;
- standard file capabilities (`setcap`) for supported proxy cores where
  required;
- nftables plus the separately packaged root NetGuard service selected by K0
  if/when K1 fail-closed protection is implemented;
- current packaged/verified Mihomo and, only if X1 becomes justified, Xray.

This boundary is a support decision, not proof that the architecture must stay
Arch-specific forever. Broader Linux distribution support is deferred until
there is real demand and the Arch application is stable.

Do not add distro abstraction layers, package-manager shims or service-manager
fallbacks solely for hypothetical portability.

## Omarchy integration as an optional frontend

On Omarchy, the same standalone application gains a companion bar integration.
The bar remains optimized for fast actions and status; the full TUI owns the
large management workspace.

Conceptually:

```text
OmaVLESS bar plugin
        |
        +-- status / connect / switch / mode
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
       existing OmaVLESS runtime
```

`Open app` must never create a second runtime or a second VPN core. If the TUI
window already exists, the integration should focus it. Closing the terminal
must not change a healthy requested tunnel.

T0 selects the installed Omarchy launcher and stable application ID:

```text
omarchy launch or focus tui --app-id=org.omarchy.omavless omavless tui
```

The TUI attaches to `omavless-runtime.service`; daemon startup failure produces
a bounded doctor/remediation view and never falls back to starting Mihomo.

Omarchy-specific responsibilities belong in the integration layer, for example:

- Quickshell/bar rendering;
- launch-or-focus wiring and stable application id;
- optional floating-window/keybinding integration;
- following the active Omarchy semantic theme;
- Omarchy-specific install/help text.

The shared runtime/TUI must not require:

- the `omarchy` CLI;
- Omarchy Shell/Quickshell;
- Omarchy-specific config/state paths;
- Hyprland Lua configuration;
- an Omarchy theme to exist.

When Omarchy is absent, the TUI uses its own default theme and continues to
provide the same VPN/profile/runtime functionality.

## Distribution ownership

The future standalone application uses the separately reviewed Arch/AUR package
`omavless` once the runtime exists. The package owns at least:

```text
/usr/bin/omavless
/usr/lib/systemd/user/omavless-runtime.service
```

plus only the additional audited resources required by the application.
Mihomo remains separately packaged and privileged networking remains separate
security work.

The Omarchy marketplace plugin must not silently download, compile or install
the standalone binary. Its behavior should be:

- if the application is installed, expose `Open app` and use the supported
  control-plane surface;
- if it is absent, keep any still-supported plugin-only behavior or show a
  precise installation/migration hint according to the delivery phase;
- never invoke an unreviewed curl-pipe-shell installer from QML;
- never silently introduce privileged setup.

Application/core setup that needs capabilities or the future K1 NetGuard
service remains an explicit host-administration step with documented audit,
fail-closed removal and fixed console recovery behavior. The K0 contract is
[`KILL_SWITCH.md`](KILL_SWITCH.md).

## Migration from the current plugin

The transition must be incremental; the existing plugin is not discarded.

1. Current state: QML plugin plus Python backend and Mihomo user-service
   lifecycle.
2. T0 defines a versioned semantic control-plane and package/migration contract.
3. T1 moves canonical lifecycle/store/background ownership behind the shared
   runtime while preserving the plugin UX.
4. T2 adds `omavless tui`; the plugin becomes the compact first-class Omarchy
   frontend and gains `Open app`.
5. Later work may simplify plugin-local backend code only after every required
   operation is available through the shared runtime and real Omarchy migration
   tests pass.

At no point should a user upgrade into two independent profile stores, two
runtime owners or two competing core services.

## Naming and repository direction

Keep the repository/product/binary identity `OmaVLESS` / `omavless`. Protocol
expansion does not require renaming the product.

The repository may contain both application source and Omarchy integration.
Organizational restructuring is allowed when implementation begins, but source
layout should reflect the dependency direction: generic runtime/application
code must not import Omarchy-specific integration code.

Avoid premature repository splits. A single repository makes schema, IPC,
plugin migration and release compatibility easier to review until there is a
concrete maintenance reason to separate packages.

## Future broader Linux support

Broader Linux support is a possible later research track, not a current product
commitment. Before adding another distribution, require evidence for:

- package availability/current core versions;
- service manager and user-session semantics;
- capability/TUN setup;
- firewall/kill-switch implementation if enabled;
- desktop/system-proxy integration when relevant;
- install/update/remove ownership;
- a real acceptance environment for that distribution.

The project should first make the Arch application excellent rather than claim
portable Linux support it cannot continuously test.
