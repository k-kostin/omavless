# OmaVLESS roadmap map

Status: design and delivery index, updated 2026-08-26.

OmaVLESS keeps the short, operational ledgers at the repository root and puts
longer design rationale in this directory. This avoids turning one roadmap into
a mixture of release state, protocol compatibility, architecture sketches,
application UX and Git workflow rules.

## Canonical documents

- [`../../DEVELOPMENT_ROADMAP.md`](../../DEVELOPMENT_ROADMAP.md) is the active
  delivery ledger: what is merged, what is cloud-ready, what still needs a real
  Omarchy machine and the dependency order between PRs.
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
- [`PLATFORM.md`](PLATFORM.md) defines the Arch-first standalone product and
  distribution boundary: Omarchy is first-class integration, not a runtime
  dependency.
- [`DEVELOPMENT_WORKFLOW.md`](DEVELOPMENT_WORKFLOW.md) defines the Git and
  cloud/local handoff model, including why `main` is the accepted source of
  truth, feature PRs are the practical alpha channel, and a beta branch is
  optional and temporary rather than a permanent second source of truth.

## Product and evidence are separate axes

A feature can be merged into `main` and still be labelled **Experimental** in
the product. That means the implementation passed its required code/security
and Omarchy gates but broader independent interoperability evidence is still
being collected. Git branch names must not be used as a substitute for this
product maturity label.

Likewise, the exact public marketplace build is an immutable commit (and should
be tagged for formal releases) while `main` may move ahead with accepted
documentation or later validated work. The repository must always name the
exact published SHA instead of implying that the tip of `main` is necessarily
the marketplace snapshot.

## Product direction

The product name remains **OmaVLESS** (`omavless`). The forward standalone
identity is:

> **OmaVLESS — a terminal-first VPN and proxy client for Arch Linux, with
> first-class Omarchy integration.**

The published 0.7.0 baseline is still an Omarchy plugin. The standalone
application/runtime direction is planned future work and must not be advertised
as current functionality before T1/T2 land.

The intended dependency direction is `Omarchy integration -> OmaVLESS runtime`,
never the reverse. A normal Arch installation should eventually use the same
runtime, TUI and semantic CLI without Omarchy, Quickshell or Hyprland installed.

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
   and CLI/automation surface must converge on one canonical connection owner
   instead of starting independent cores or maintaining competing copies of
   connection state.
5. **Omarchy is integration, not foundation.** Omarchy-specific Quickshell,
   launch/focus, theme and Hyprland behavior stays outside the generic Arch
   runtime/application layer.
6. **Tunnel ownership is centralized.** OmaVLESS may stop and replace cores it
   owns, but foreign VPNs/TUNs are detected and reported rather than killed or
   reconfigured.
7. **Fail-closed protection is independent of a proxy core.** A real kill
   switch must survive a core crash and therefore cannot be implemented as a
   Mihomo-only option or depend on the QML panel/TUI staying alive.
8. **Privilege stays narrow and explicit.** If fail-closed networking needs a
   privileged host component, it must be optional, auditable and expose only a
   bounded networking protocol; arbitrary commands or arbitrary firewall rules
   are out of scope.
9. **Compact and full interfaces have different jobs.** The bar keeps frequent
   connect/switch/status actions concise; the TUI becomes the natural home for
   deeper sources, subscriptions, routing/DNS, connections, diagnostics, logs
   and operator workflows.
10. **Evidence controls merges.** Runtime/network/security changes reach `main`
    only after their declared cloud and real-Omarchy gates pass. Documentation-
    only changes do not invent a meaningless live VPN gate.

## Current priority

The existing D1 → V0 → P4a → P4b → P4c sequence remains the active runtime
chain. The architecture, platform and TUI work in this directory is deliberately
staged rather than used as an excuse for a speculative rewrite while current
protocol validation and credential-family work is still open.
