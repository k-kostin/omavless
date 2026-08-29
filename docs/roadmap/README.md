# OmaVLESS roadmap map

Status: design and delivery index, updated 2026-08-29.

OmaVLESS keeps the short operational ledgers at the repository root and puts
longer design rationale in this directory. This avoids turning one roadmap into
a mixture of release state, protocol compatibility, architecture sketches,
application UX, host packaging and Git workflow rules.

## Canonical documents

- [`../../DEVELOPMENT_ROADMAP.md`](../../DEVELOPMENT_ROADMAP.md) is the active
  delivery ledger: what is merged, what is cloud-ready, what still needs local
  acceptance and the dependency order between PRs.
- [`../../PROTOCOL_ROADMAP.md`](../../PROTOCOL_ROADMAP.md) is the protocol and
  share-format compatibility specification. It records what OmaVLESS accepts,
  what Mihomo can represent and where a future Xray backend may be justified.
- [`ARCHITECTURE.md`](ARCHITECTURE.md) describes the intended evolution from a
  Mihomo-centric implementation to explicit `CoreBackend`,
  `TunnelCoordinator` and optional `LeakProtection` boundaries without a
  big-bang rewrite.
- [`TUI_APP.md`](TUI_APP.md) describes the product evolution from a bar-only
  plugin into one shared OmaVLESS runtime with a compact bar surface, a full
  TUI application and a semantic CLI/IPC integration surface.
- [`CONTROL_PLANE.md`](CONTROL_PLANE.md) is the accepted T0 runtime ownership,
  versioned IPC, migration/distribution contract and T1 acceptance plan.
- [`KILL_SWITCH.md`](KILL_SWITCH.md) is the accepted K0 Full VPN fail-closed
  threat model, NetGuard privilege/host contract and K1 acceptance matrix.
- [`I18N.md`](I18N.md) defines the I1 English/Russian catalog, locale/fallback
  behavior, string ownership and deliberately deferred translation batches.
- [`PLATFORM.md`](PLATFORM.md) defines the standalone product and distribution
  boundary: one shared runtime/TUI, initial host families Arch and NixOS, and
  Omarchy as first-class integration rather than a runtime dependency.
- [`NIX_PORTABILITY.md`](NIX_PORTABILITY.md) preserves the current Omarchy/Nix
  research assumptions and the concrete host-integration consequences for T1
  and T2: immutable store paths, capabilities, systemd provisioning, packaging
  and acceptance.
- [`DEVELOPMENT_WORKFLOW.md`](DEVELOPMENT_WORKFLOW.md) defines the Git and
  cloud/local handoff model, including why `main` is the accepted source of
  truth, feature PRs are the practical alpha channel, and a beta branch is
  optional and temporary rather than a permanent second source of truth.

Where older T0/TUI wording says "Arch/AUR" as if it were the only future host,
[`PLATFORM.md`](PLATFORM.md) is now authoritative for host/distribution scope:
Arch remains first-class and NixOS is the second initial target. The existing
control-plane, migration and TUI semantics remain authoritative unless a later
PR deliberately changes them.

## Product and evidence are separate axes

A feature can be merged into `main` and still be labelled **Experimental** in
the product. That means the implementation passed its required code/security
and host gates but broader independent interoperability evidence is still being
collected. Git branch names must not be used as a substitute for this product
maturity label.

Likewise, the exact public marketplace build is an immutable commit (and should
be tagged for formal releases) while `main` may move ahead with accepted
documentation or later validated work. The repository must always name the
exact published SHA instead of implying that the tip of `main` is necessarily
the marketplace snapshot.

Arch and NixOS acceptance are also separate evidence axes. Passing one host's
package/capability/service tests does not claim the other host is supported.
The semantic runtime/TUI is shared, but each host integration must earn its own
acceptance evidence.

## Product direction

The product name remains **OmaVLESS** (`omavless`). The forward standalone
identity is:

> **OmaVLESS — a terminal-first VPN and proxy client for Linux, with first-class
> Omarchy integration and initial standalone support for Arch and NixOS.**

The published 0.7.0 baseline is still an Omarchy plugin. The standalone
application/runtime direction is planned future work and must not be advertised
as current functionality before T1/T2 land and the corresponding host gates
pass.

The intended dependency direction is:

```text
Omarchy integration
        |
        v
OmaVLESS runtime/TUI/CLI
        |
        v
narrow host integration
     /       \
    v         v
 Arch       NixOS
```

A supported standalone installation should eventually use the same runtime,
TUI and semantic CLI without Omarchy, Quickshell or Hyprland installed.

## Omarchy Cinque planning assumption

Current upstream discussion makes a Nix-backed Omarchy generation plausible but
not confirmed. Until upstream publishes a concrete migration contract, roadmap
work uses one explicit assumption: **the Omarchy plugin/Quickshell API remains
materially compatible enough that the existing bar frontend can be migrated
without redesigning the OmaVLESS runtime or TUI**.

If that assumption later proves false, the Omarchy integration layer changes.
It must not pull package-manager, privilege or core-lifecycle ownership back
into QML.

## Design principles

1. **Mihomo remains the primary core.** Current Full VPN, Routing, Direct,
   country presets, rule providers, private controller and diagnostics are
   Mihomo-native and must not be weakened merely to make another core look
   symmetrical.
2. **Profile semantics stay above core choice.** Strict protocol adapters,
   redacted preview, private storage and subscription identity form the stable
   boundary. A future core resolver chooses an implementation only after the
   profile has been validated.
3. **A second core is capability-driven, not a preference toggle.** Initial
   Xray support is reserved for common Xray-only profile semantics which Mihomo
   cannot represent without loss. The first production slice is Full VPN only.
4. **One runtime owns the tunnel; UIs are clients.** The future bar plugin, TUI
   and CLI/automation surface converge on one canonical connection owner instead
   of starting independent cores or maintaining competing connection state.
5. **Omarchy is integration, not foundation.** Omarchy-specific Quickshell,
   launch/focus, theme and Hyprland behavior stays outside the generic runtime
   and application layer.
6. **Arch and NixOS are host adapters, not separate products.** Package,
   capability and service provisioning may differ, while control-plane,
   profile, routing and TUI behavior remain shared.
7. **Do not persist generation-specific implementation paths.** Nix store paths
   can change across generations; durable runtime state must refer to semantic
   state or stable host entry points, not a resolved `/nix/store/...` path.
8. **Tunnel ownership is centralized.** OmaVLESS may stop and replace cores it
   owns, but foreign VPNs/TUNs are detected and reported rather than killed or
   reconfigured.
9. **Fail-closed protection is independent of a proxy core.** A real kill
   switch must survive a core crash and therefore cannot be implemented as a
   Mihomo-only option or depend on the QML panel/TUI staying alive.
10. **Privilege stays narrow and host-native.** Arch may use reviewed file
    capabilities; NixOS may use a reviewed wrapper/service mechanism. Neither
    path gives the ordinary UI broad root authority.
11. **Compact and full interfaces have different jobs.** The bar keeps frequent
    connect/switch/status actions concise; the TUI becomes the natural home for
    deeper sources, subscriptions, routing/DNS, connections, diagnostics, logs
    and operator workflows.
12. **Evidence controls merges and support claims.** Runtime/network/security
    changes reach `main` only after their declared gates pass. Documentation-
    only changes do not invent a meaningless live VPN gate, and one host's
    evidence is never silently reused as another host's packaging proof.

## T1/T2 portability requirement

The T1 shared runtime and T2 TUI should be designed so that adding the second
initial host does not require a second control plane or duplicated client.
Before runtime ownership is considered structurally mature:

- package-manager commands must stay out of the semantic control plane;
- service state control must be distinct from service-file provisioning;
- the desired state/store must contain no Arch package-manager state and no
  Nix generation-specific executable path;
- host health/remediation should be exposed through bounded semantic
  capabilities/doctor results;
- the TUI should use those semantic results and remain otherwise host-neutral.

The Nix-specific acceptance matrix is defined in
[`NIX_PORTABILITY.md`](NIX_PORTABILITY.md).

## Current priority

T1a's isolated protocol/conformance foundation and K0's fail-closed design are
independent no-fixture checkpoints. V0 remains partially validated and still
blocks P4a, but unavailable protocol credentials do not block bounded T1,
security-design, localization, observability or platform-preparation work.

The new platform direction does **not** require pausing Arch development for a
future Omarchy decision. Instead, T1/T2 implementation should avoid baking Arch
package mechanics into the shared runtime so the same application can acquire a
Nix package/module and Nix-specific acceptance path without a later rewrite.

None of these documents authorizes a speculative daemon cutover, privileged
firewall implementation or claim that current OmaVLESS 0.7.0 already supports
standalone Arch/NixOS operation.
