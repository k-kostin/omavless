# OmaVLESS Nix / future Omarchy portability plan

Status: research-backed platform design input, not a claim that Omarchy Cinque
has committed to NixOS. Updated 2026-08-29.

Canonical product/platform policy lives in [`PLATFORM.md`](PLATFORM.md). This
document preserves the concrete observations that justify preparing the shared
runtime and TUI for both Arch and NixOS now rather than after the application is
already coupled to Arch packaging.

## 1. Why this is no longer hypothetical portability

Recent upstream Omarchy discussion has made a future Nix-backed packaging or
host model a realistic possibility. The important product signal is not that a
specific migration has been announced; it has not. The important signal is
that Omarchy's maintainers are willing to change the package substrate if the
user experience stays simple and Omarchy retains control of the resulting
product experience.

For OmaVLESS planning, that changes NixOS from a generic "maybe another distro
one day" idea into a concrete second host family worth protecting before T1/T2
lock in package and service assumptions.

Roadmap policy therefore assumes:

- Arch remains a first-class standalone target;
- NixOS becomes the second initial standalone target;
- a future Nix-backed Omarchy is treated as an Omarchy frontend/host integration
  over the same runtime, not as a separate OmaVLESS product;
- current Omarchy plugin/Quickshell semantics are assumed to remain materially
  compatible unless upstream proves otherwise.

This assumption must be revisited against the actual Cinque implementation
before release support is claimed.

## 2. Expected migration size

The current OmaVLESS implementation is not fundamentally tied to Arch. Most of
the product should survive a host migration unchanged:

### Expected to remain shared

- profile parsing and validation;
- VLESS/Trojan/Hysteria2/TUIC and future WG/AWG semantic adapters;
- private profile/subscription store;
- Mihomo YAML generation;
- routing presets/custom-rule semantics;
- controller-facing diagnostics behind the runtime;
- latency probes;
- TUI workflows;
- control-plane protocol;
- desired/actual state model;
- same-user XDG runtime/private socket model.

### Expected to require host-specific work

- package installation and update ownership;
- Mihomo installation/remediation hints;
- TUN/capability provisioning;
- systemd unit provisioning ownership;
- executable-path stability across Nix generations;
- future NetGuard/nftables packaging;
- host-specific doctor output.

The expected change is therefore a host-integration refactor, not a rewrite of
OmaVLESS.

## 3. Existing implementation decisions that already help

The current backend already has several useful portability properties:

1. Mihomo is not hard-coded to `/usr/bin/mihomo`. Core discovery accepts an
   explicit `OMAVLESS_MIHOMO`, `~/.local/bin/mihomo`, and `PATH`.
2. Private state is already under user/XDG-style paths rather than package
   directories.
3. Service control uses `systemctl --user`, a lifecycle model also available on
   NixOS.
4. Protocol and subscription logic do not call pacman/AUR.
5. The planned T1 v1 control plane is semantic and does not expose package
   manager or raw systemctl operations to clients.

These boundaries should be preserved and strengthened during T1 rather than
replaced with a large distro abstraction layer.

## 4. Known Nix hazard: resolving stable entry points into store paths

Current core discovery eventually resolves the selected executable path. On a
traditional mutable filesystem this is normally harmless. On NixOS a stable
entry point may point through a profile/wrapper to a generation-specific path
under `/nix/store`.

A runtime must not persist or bake such a resolved store path into durable
configuration when the stable host contract is a wrapper/profile executable.
After an update and garbage collection the previous store path may disappear.

Before NixOS runtime acceptance:

- distinguish the user/host-facing executable entry point from its resolved
  implementation path;
- avoid persisting generation-specific resolved paths as desired state;
- regenerate/re-resolve runtime execution paths after package generation
  changes;
- include stale-store-path tests in the Nix acceptance matrix.

This is a small implementation change but an important lifecycle invariant.

## 5. Capabilities: the largest Arch/Nix difference

The current Omarchy/Arch setup tells the user to install Mihomo and grant Linux
capabilities to that executable with `setcap`.

That must not become the generic product contract.

On NixOS, immutable store semantics mean the normal product path should use a
reviewed Nix-native privilege mechanism such as a security wrapper or a
narrowly declared service capability model. The exact mechanism belongs to the
Nix packaging implementation and security review.

Common security invariants across both hosts:

- no arbitrary root shell API;
- no silent `sudo`/`pkexec` from QML/TUI;
- no broad `CAP_NET_ADMIN` for the ordinary TUI or control-plane client;
- no user-authored arbitrary privileged firewall expressions;
- explicit, inspectable remediation when required host privileges are absent.

The TUI should only see a semantic result such as `core ready`, `TUN permission
missing`, or `host setup required`, plus bounded host-specific remediation text.

## 6. systemd: lifecycle can remain shared

A switch to NixOS does not imply replacing the canonical user-service runtime.
NixOS uses systemd and supports user services, so the runtime lifecycle can stay
conceptually identical:

```text
omavless-runtime.service
  start / stop / restart / status
```

The difference is who owns the unit definition:

- Arch package: conventional packaged user unit;
- NixOS: declaratively provisioned user unit/module output;
- current plugin: legacy generated user unit during migration only.

T1 should therefore separate **service state control** from **service file
provisioning**. Once the standalone package owns the canonical runtime unit,
the runtime should not rewrite its own packaged/declarative unit file.

## 7. Narrow host adapter, not a distro framework

T1/T2 should not grow conditionals across the whole application. Platform
specificity belongs behind one small boundary.

Conceptually:

```text
                shared runtime
                     |
             semantic host needs
                     |
          +----------+----------+
          |                     |
          v                     v
      Arch host              NixOS host
  package/setcap hints   package/wrapper/module
  unit ownership        generation-safe paths
```

The host interface may expose only what the runtime genuinely needs, for
example:

- locate supported core entry point;
- report whether required host networking privilege is provisioned;
- report whether the package-owned runtime service is provisioned;
- render bounded install/remediation hints;
- expose host identity/capability information to `omavless doctor`.

Profile parsing, routing decisions, subscriptions and control-plane methods do
not belong in this adapter.

## 8. TUI design consequence

The TUI should be written once.

It connects to the same private control socket and renders the same semantic
state on Arch, NixOS and Omarchy. Host differences may appear only in setup and
health/remediation views.

Examples:

```text
Arch:
  Mihomo: installed
  TUN privilege: file capabilities ready

NixOS:
  Mihomo: installed
  TUN privilege: Nix wrapper/service ready
```

Those are two presentations of the same runtime capability, not separate
connection implementations.

The TUI must never directly invoke `pacman`, `yay`, `nix-env`, `nix profile`,
`nixos-rebuild` or arbitrary Nix expressions as part of a normal connect flow.
Packaging remains outside the semantic VPN operation.

## 9. Omarchy plugin assumption

For roadmap purposes we assume the Omarchy plugin API remains materially stable
across the relevant major transition:

- plugin id/enable/disable lifecycle remains available;
- Quickshell remains a supported frontend surface or offers a compatible
  replacement boundary;
- the plugin can still launch/focus the standalone TUI and call the semantic
  runtime bridge.

If the exact launcher or QML imports change, treat that as a frontend migration.
Do not move package/runtime logic back into the plugin to compensate.

The desired final dependency remains:

```text
Omarchy bar (Arch-backed or Nix-backed)
                 |
                 v
          semantic control API
                 |
                 v
          OmaVLESS runtime
                 |
                 v
          host integration
```

## 10. Packaging direction

### Arch

Prepare a reviewed `omavless` package containing the standalone command,
runtime resources and package-owned user service. Mihomo remains separately
packaged unless a later security/release decision explicitly changes that.

### NixOS

Prepare a Nix package plus the smallest module/wrapper integration required for
stable executable/service/privilege semantics. Prefer a normal user-facing
package/module contract; do not require ordinary users to edit raw generated
Nix expressions merely to connect a VPN profile.

### Omarchy

The marketplace plugin remains a frontend package. It may detect an installed
standalone app and show `Open app`, but it does not curl/download/compile the
runtime or silently provision privilege.

## 11. T1/T2 implementation checkpoints added by this decision

### T1 host-neutral runtime checkpoint

Before the daemon cutover is considered structurally complete:

- package-manager commands are absent from the semantic control plane;
- core execution uses a stable host entry-point contract;
- service control and service provisioning are separate concepts;
- `omavless doctor` has a bounded host-capability model;
- desired state contains no Arch- or Nix-generation-specific executable path.

### Arch host checkpoint

- package install/update/remove smoke;
- user-service lifecycle;
- Mihomo/TUN privilege setup;
- Full/Routing/Direct lifecycle and restart reconciliation;
- migration from current plugin-owned service/store.

### NixOS host checkpoint

- package/module evaluation and install;
- stable executable discovery across generation update;
- user-service lifecycle;
- wrapper/service capability setup and negative permission case;
- Full/Routing/Direct lifecycle parity;
- generation rollback;
- garbage-collection/stale-store-path regression;
- shared private-store migration/import behavior.

### T2 TUI checkpoint

The same TUI build/control protocol must run against accepted Arch and NixOS
runtime environments. Platform-specific branches in the TUI are restricted to
setup/doctor/remediation presentation.

## 12. What we deliberately do not do now

This roadmap update does **not**:

- claim Omarchy Cinque will definitely be NixOS;
- add Nix commands to the current marketplace plugin;
- implement a Nix package before the shared runtime has a real executable
  boundary;
- delay Arch application work until upstream Omarchy settles its packaging;
- promise Ubuntu/Fedora/general Linux support;
- duplicate the runtime for Arch and NixOS;
- rewrite current profile/core logic for portability's sake.

The immediate architectural consequence is narrower: T1/T2 must stop treating
Arch package mechanics as runtime semantics, so the application can become an
Arch + NixOS product without a second rewrite later.
