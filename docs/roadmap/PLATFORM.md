# OmaVLESS platform and distribution direction

Status: T0 distribution contract accepted and broadened for Arch + NixOS host
families; not a claim of current standalone runtime support. Updated 2026-08-29.

The complete runtime/API/migration contract is
[`CONTROL_PLANE.md`](CONTROL_PLANE.md). The Omarchy/Nix research assumptions and
host-integration consequences are recorded in
[`NIX_PORTABILITY.md`](NIX_PORTABILITY.md).

## Product identity

The product name remains **OmaVLESS** (`omavless`) even as protocol support and
interfaces expand beyond the original VLESS-only scope.

The intended product description is:

> **OmaVLESS — a terminal-first VPN and proxy client for Linux, with first-class
> Omarchy integration and initial standalone support for Arch and NixOS.**

The current published 0.7.0 build is still delivered as an Omarchy bar plugin
and uses the separately installed Mihomo core. The statement above describes
the forward product boundary; it must not be presented as already implemented
until the standalone application/runtime phases land and each host family has
its own declared acceptance evidence.

## Two initial host families, one runtime contract

The standalone application is now designed from the start for two concrete
Linux host families:

- **Arch Linux / Arch-based distributions**;
- **NixOS / a future Omarchy host using Nix-managed packaging**, if and when
  that becomes the upstream Omarchy direction.

This is not a commitment to generic all-Linux portability. Arch and NixOS are
the two explicit targets because both have concrete product value. Other
families remain deferred until there is demand and a real acceptance host.

The dependency direction is:

```text
Omarchy integration
        |
        v
   OmaVLESS API/runtime
        |
        v
 narrow Linux host integration
      /       \
     v         v
  Arch host   NixOS host
```

and never:

```text
OmaVLESS runtime
        |
        v
     Omarchy
```

A normal supported standalone installation must be able to use the same
runtime, TUI and CLI without requiring Omarchy, Quickshell or Hyprland. Omarchy
adds the compact bar frontend, launch/focus integration and theme integration;
it does not own profile semantics, the canonical store or tunnel lifecycle.

## Working assumption for Omarchy Cinque

Current upstream discussion makes a Nix-backed Omarchy generation plausible but
not confirmed. Roadmap planning therefore uses the following **working
assumption**, not a release claim:

- Omarchy may change its package substrate in a future major release;
- the user-facing plugin model and Quickshell integration remain materially
  compatible with the current Omarchy plugin API;
- the migration goal is to hide packaging-system complexity from ordinary
  users rather than require them to author Nix expressions for normal app use.

If upstream later changes the plugin API substantially, only the Omarchy
frontend adapter should need corresponding migration. Such a frontend change
must not force a rewrite of the runtime, TUI, profile adapters or private store.

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
Hyprland keybinding or Omarchy bar action invokes the same client and attaches
to the same canonical runtime.

## Stable portable core versus host integration

The following behavior is host-neutral and must remain shared across Arch and
NixOS:

- profile and subscription schemas;
- strict VLESS/Trojan/Hysteria2/TUIC and future WG/AWG adapters;
- redacted preview and credential boundaries;
- routing-policy semantics and generated Mihomo configuration;
- private store migration;
- the v1 control-plane protocol and semantic CLI;
- desired/actual connection state and mutation serialization;
- TUI workflows and ordinary diagnostics;
- core capability discovery at the semantic level.

The following behavior belongs behind a deliberately narrow **host integration
boundary**:

- package/install/removal hints and package ownership;
- discovery of a stable executable entry point for Mihomo/OmaVLESS;
- TUN/capability provisioning;
- installation/provisioning of user services;
- host firewall / NetGuard provisioning when K1 exists;
- host-specific doctor/remediation text;
- update behavior where package generations replace executable paths.

Do not introduce a generic distro framework, package-manager abstraction or
service-manager fallback matrix. Implement exactly the Arch and NixOS host
contracts we can continuously test.

A minimal conceptual host contract is sufficient. It may eventually expose
operations equivalent to:

```text
find_core()
core_privilege_status()
install_hint()
service_provisioning_status()
host_doctor()
```

The semantic runtime API must remain independent of whether these operations
are implemented by Arch package files, Nix derivations/modules/wrappers or an
Omarchy-supported host abstraction.

## Runtime platform assumptions shared by both hosts

The first standalone implementations may assume:

- Linux;
- systemd with a user manager;
- Linux TUN support;
- XDG config/state/cache/runtime semantics;
- a current packaged/verified Mihomo and, only if X1 becomes justified, Xray;
- nftables plus the separately reviewed root NetGuard boundary selected by K0
  if/when K1 fail-closed protection is implemented.

The project must not assume that a mutable executable at `/usr/bin` exists or
that applying `setcap` directly to the resolved core binary is valid on every
supported host.

## Arch host contract

Arch remains a first-class standalone target.

The initial Arch distribution may use a separately reviewed official/AUR
package `omavless` and separately packaged Mihomo. The package may own resources
such as:

```text
/usr/bin/omavless
/usr/lib/systemd/user/omavless-runtime.service
```

plus only the audited resources required by the application.

For Mihomo, a normal mutable package path and explicit file capabilities may
remain a valid Arch setup when supported by the package. Existing `setcap`
behavior is therefore an **Arch host implementation**, not part of the generic
runtime contract.

The Arch implementation must continue to support a verified explicit core path
and PATH-based discovery where useful, without baking AUR commands into profile,
runtime or TUI logic.

## NixOS host contract

NixOS support must respect immutable store semantics rather than pretending it
is Arch with a different package command.

### Core and application packaging

The Nix path should provide stable user-facing executable entry points while
allowing the underlying derivation/store path to change between generations.
The runtime must not persist a resolved generation-specific `/nix/store/...`
path as durable configuration merely because `Path.resolve()` or a similar call
followed a profile/wrapper symlink.

Core discovery should prefer a stable executable contract supplied by the host
package/environment. An explicit `OMAVLESS_MIHOMO` override remains useful for
development and diagnostics, but the normal package must not require users to
copy a core into `~/.local/bin` merely to escape Nix packaging.

### TUN and capabilities

Directly applying `setcap` to an immutable Nix-store binary is not the supported
product model. NixOS integration should use a reviewed Nix-native mechanism,
for example a security wrapper or a narrowly declared service capability model,
according to the final package design.

The ordinary OmaVLESS UI/runtime still must not obtain broad root authority,
run arbitrary privileged shell commands or silently modify system policy.
Privilege provisioning remains explicit and auditable.

### systemd services

NixOS still uses systemd and supports user services, so the semantic runtime
lifecycle remains `start/stop/restart/status` against one canonical user
runtime. What changes is **provisioning ownership**:

```text
Arch package                 NixOS module/package
     |                              |
     v                              v
systemd user unit            declarative user unit
     \                              /
      +----------+-----------------+
                 v
        omavless-runtime.service
                 |
                 v
         same control plane
```

The runtime should manage service state, not rewrite its own package-owned unit
on every launch. Legacy plugin-generated units remain a migration concern and
must be removed/adopted transactionally during T1 cutover.

### Nix generations and upgrades

Acceptance must include a package generation change while disconnected and
while a requested tunnel is configured for login/recovery. The test must prove
that stale store paths do not become durable runtime state and that rollback to
a previous generation has deterministic service/core behavior.

## Current plugin compatibility

The current plugin backend is already less Arch-coupled than the installation
documentation suggests:

- protocol parsing and Mihomo YAML generation do not depend on pacman/AUR;
- private data lives below XDG/home paths;
- runtime coordination uses user-systemd;
- Mihomo discovery already accepts `OMAVLESS_MIHOMO`, `~/.local/bin/mihomo` and
  `PATH`.

Therefore a future Nix move is expected to be a **host-integration refactor**,
not a rewrite of the VPN product. The main migration pressure is package setup,
capability provisioning, service ownership and host-specific onboarding.

One known portability hazard should be addressed before NixOS becomes a live
target: core discovery must not turn a stable Nix profile/wrapper entry point
into a generation-specific durable store path by resolving symlinks too early.

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

The current T0 launcher contract remains:

```text
omarchy launch or focus tui --app-id=org.omarchy.omavless omavless tui
```

If Cinque changes the launcher spelling, that is an Omarchy frontend adapter
change. It must not alter the TUI/runtime protocol.

The TUI attaches to `omavless-runtime.service`; daemon startup failure produces
a bounded doctor/remediation view and never falls back to starting Mihomo as a
second owner.

Omarchy-specific responsibilities belong in the integration layer, including:

- Quickshell/bar rendering;
- launch-or-focus wiring and stable application id;
- optional floating-window/keybinding integration;
- following the active Omarchy semantic theme;
- Omarchy-specific install/help text.

The shared runtime/TUI must not require:

- the `omarchy` CLI;
- Omarchy Shell/Quickshell;
- Omarchy-specific config/state paths;
- Hyprland configuration;
- an Omarchy theme to exist.

When Omarchy is absent, the TUI uses its own default theme and continues to
provide the same VPN/profile/runtime functionality.

## Distribution ownership

The future standalone application is installed through the native reviewed
package path for the host:

- Arch: Arch/AUR package or another explicitly reviewed Arch package path;
- NixOS: Nix package/module/flake interface selected by the implementation
  track, with stable executable/service contracts and generation-safe state.

The Omarchy marketplace plugin must not silently download, compile or install
the standalone application. Its behavior remains:

- if the application is installed, expose `Open app` and use the supported
  control-plane surface;
- if it is absent, keep any still-supported plugin-only behavior or show a
  precise host-specific installation/migration hint according to the delivery
  phase;
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
2. T0/T0p define a versioned semantic control-plane plus the Arch/Nix host
   boundary and package/migration contract.
3. T1 moves canonical lifecycle/store/background ownership behind the shared
   runtime while preserving plugin UX.
4. T1 host work proves the same runtime semantics on Arch and NixOS; host
   provisioning differences stay outside the control plane.
5. T2 adds `omavless tui`; the plugin becomes the compact first-class Omarchy
   frontend and gains `Open app`.
6. Later work may simplify plugin-local backend code only after every required
   operation is available through the shared runtime and real migration tests
   pass.

At no point should a user upgrade into two independent profile stores, two
runtime owners or two competing core services.

## TUI portability requirement

T2 is not considered platform-complete if the TUI only works on one of the two
initial host families while its runtime logic is otherwise portable.

The TUI itself must consume only the semantic control plane and safe host
capability/doctor results. It must not contain branches such as `if nixos` for
profile parsing, connection transitions, routing modes or core-controller
behavior.

Platform-specific UI is limited to setup/remediation text and capability
availability. The same profile store should be usable when a user migrates an
OmaVLESS configuration between supported Arch and NixOS installations, subject
to the normal private-backup/import security model.

## Acceptance strategy

Arch and NixOS evidence are separate. A green Arch runtime test does not claim
NixOS packaging/capability/service correctness, and vice versa.

Before declaring NixOS supported, require at least:

- package install/update/remove and generation rollback;
- stable `omavless` and Mihomo executable discovery without stale store paths;
- user-systemd runtime startup/restart/login behavior;
- explicit TUN/capability provisioning and negative permission tests;
- Full VPN / Routing / Direct lifecycle parity where the selected core supports
  them;
- profile/subscription/store migration parity;
- plugin -> runtime -> TUI behavior on Omarchy if/when an upstream Nix-backed
  Omarchy build exists;
- NetGuard/nftables host integration separately when K1 exists.

Until such an environment exists, NixOS is an **implementation target**, not a
claimed supported runtime.

## Naming and repository direction

Keep the repository/product/binary identity `OmaVLESS` / `omavless`. Protocol
or host expansion does not require renaming the product.

The repository may contain application source, Arch packaging, Nix packaging
and Omarchy integration. Source layout must reflect dependency direction:
generic runtime/application code must not import Omarchy-, Arch- or Nix-specific
integration code.

Avoid premature repository splits. One repository keeps schema, IPC, packaging,
plugin migration and release compatibility easier to review until a concrete
maintenance reason justifies separation.

## Future broader Linux support

Broader Linux support beyond Arch and NixOS remains deferred. Before adding
another host family, require evidence for:

- package availability/current core versions;
- service manager and user-session semantics;
- capability/TUN setup;
- firewall/kill-switch implementation if enabled;
- desktop/system-proxy integration when relevant;
- install/update/remove ownership;
- a real acceptance environment for that distribution.

The project should make the Arch and NixOS application excellent rather than
claim generic portability it cannot continuously test.
