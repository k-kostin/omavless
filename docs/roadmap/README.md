# OmaVLESS roadmap map

Status: design and delivery index, updated 2026-09-14.

The native installed path has completed scoped local R6 acceptance. Start with
the [current delivery status](CURRENT_STATUS.md),
the [closure](../testing/R6_LOCAL_CLOSURE_2026-09-13.md) and
[publication candidate](../testing/R6_PUBLICATION_CANDIDATE_2026-09-13.md), not the
historical migration sequence below. Native integration #238 is merged; release
and marketplace publication remain separate. Enabled login autoconnect remains
[AUTO-1](LOGIN_AUTOCONNECT_FOLLOWUP.md).

OmaVLESS keeps the operational delivery ledger at repository root and longer
contracts under this directory. The roadmap now separates four concerns which
must not be confused:

1. current Omarchy plugin product completion;
2. incremental Python -> Rust migration;
3. canonical runtime/application/TUI evolution;
4. Arch/NixOS host packaging/security integration.

## Canonical documents

- [`../../DEVELOPMENT_ROADMAP.md`](../../DEVELOPMENT_ROADMAP.md) — active
  sequence, dependencies, statuses and next work.
- [`RUST_MIGRATION.md`](RUST_MIGRATION.md) — **authoritative implementation
  direction**: Rust target, Python parity/oracle rules, R0-R6 and native-runtime
  gate before TUI.
- [`ARCHITECTURE.md`](ARCHITECTURE.md) — target Rust runtime/domain/core/host
  layers, `TunnelCoordinator`, optional `LeakProtection` and future core model.
- [`CONTROL_PLANE.md`](CONTROL_PLANE.md) — accepted v1 semantic IPC plus R5/T1
  Rust ownership cutover and rollback contract.
- [`TUI_APP.md`](TUI_APP.md) — selected Rust + Ratatui full application client,
  UI responsibilities and T2+ delivery phases.
- [`GUI_RESEARCH.md`](GUI_RESEARCH.md) — G1 candidates, huacnlee/GPUI references,
  semantic-client boundary, dependency risks and staged evaluation gates;
  research direction, not GUI implementation or adoption approval.
- [`PLATFORM.md`](PLATFORM.md) — one application/runtime with initial host
  families Arch and NixOS; Omarchy is first-class integration, not foundation.
- [`NIX_PORTABILITY.md`](NIX_PORTABILITY.md) — Nix immutable paths,
  capabilities, services, generation lifecycle and Nix acceptance matrix.
- [`KILL_SWITCH.md`](KILL_SWITCH.md) — K0 threat model, root NetGuard boundary
  and K1 acceptance requirements.
- [`I18N.md`](I18N.md) — English/Russian catalog, fallback/ownership and
  remaining plugin surface batches.
- [`DEVELOPMENT_WORKFLOW.md`](DEVELOPMENT_WORKFLOW.md) — Git/PR/cloud/local
  workflow and evidence discipline.
- [`../../PROTOCOL_ROADMAP.md`](../../PROTOCOL_ROADMAP.md) — protocol/share
  compatibility, Mihomo representation and future-core gaps.

Every coding agent also reads [`../../AGENTS.md`](../../AGENTS.md), which makes
`RUST_MIGRATION.md` mandatory for backend/runtime/protocol/TUI work.

## Authoritative conflict rules

When older accepted documents contain wording which predates later decisions:

- **implementation language:** `RUST_MIGRATION.md` wins. Rust is selected for
  runtime/domain/CLI/TUI; Python is the separate legacy/reference implementation,
  not the activated native owner;
- **host scope:** `PLATFORM.md` wins. Arch and NixOS are initial standalone
  host families;
- **semantic control API:** `CONTROL_PLANE.md` wins. Language migration does not
  reopen v1 security/ownership semantics;
- **delivery order:** `DEVELOPMENT_ROADMAP.md` wins.

Do not use an older sentence such as "Python or compiled implementation remains
open" to restart an already decided architecture question.

## Product direction

The forward product is:

> **OmaVLESS — a terminal-first VPN and proxy client for Linux, with first-class
> Omarchy integration and initial standalone support for Arch and NixOS.**

Current published 0.7.0 remains an Omarchy QML/Quickshell plugin backed by
Python and external Mihomo. Future product shape is:

```text
                   Omarchy bar
                   QML frontend
                        |
                        v
                 semantic v1 API
                        |
                        v
                 Rust runtime
            canonical state/lifecycle
                /       |       \
               v        v        v
        Rust domain  Mihomo    host adapter
        /routing     backend   Arch/NixOS
               \        |        /
                +-------+-------+
                        |
                        v
                 external Mihomo

Standalone clients:
  Rust CLI
  Rust Ratatui TUI
  optional later Rust GUI client
```

## Current migration doctrine

There is no big-bang rewrite branch.

For each backend subsystem:

```text
Python reference
      +
language-neutral contract fixtures
      |
      v
Rust candidate
      |
 differential / golden parity
      |
 real-host gate when applicable
      |
Rust becomes owner
      |
Python oracle/rollback removed later
```

Known Python bugs are not sacred parity. Fix the explicit contract and regression
fixture rather than preserving unsafe behavior.

The final normal package should not require Python, pip, venv or Python modules.
Cargo/crates are build-time source dependencies; ordinary users run the built
`omavless` executable and do not need Cargo at runtime.

Mihomo stays external and separately packaged.

## Current two-lane strategy

### Plugin completion lane

Keep improving the current plugin where work is genuinely plugin-facing:

- remaining localization/QML surfaces;
- bug fixes/accessibility/navigation;
- current routing/import/subscription/diagnostic presentation;
- opportunistic V0 fixture validation;
- P4 protocols after R2, implemented Rust-first and product-enabled only after
  their own private/core/live/security gates and runtime bridge pass.

Do not force every future app feature into the bar merely to call the plugin
"complete".

### Rust migration lane

The dependency sequence (not a list of still-unimplemented stages) is:

```text
R0 Cargo workspace + parity infrastructure
R1 control protocol
R2 existing protocol/profile adapters
R3 store/subscriptions/routing
R4 Mihomo/probes/diagnostics/host readiness
R5/T1 Rust canonical runtime
R6 remove Python runtime dependency
T2 Ratatui TUI
```

R0-R2 are intentionally not blocked by missing V0 provider credentials.

## Why P4 moved behind R2

WireGuard/AmneziaWG remains plugin-relevant functionality, but it should not be
implemented as a large new Python parser immediately before Python retirement.

The dependency is now:

```text
R2 Rust adapter boundary
           |
           v
P4a WireGuard Rust adapter
 -> P4b AmneziaWG Rust adapter
 -> P4c vpn:// guest decoder

V0 existing-family evidence --------------------> independent maturity claims
```

The QML plugin can expose those Rust-owned semantics through the migration/runtime
bridge. This finishes plugin capabilities without creating throwaway backend
code.

## Features assigned to the Rust owner

The following remain separately scoped work on the now accepted Rust owner,
not new Python-plugin subsystems:

- full privacy-aware active connections;
- App proxy state ownership;
- K1 kill switch implementation;
- optional Xray backend;
- persistent background scheduling/recovery;
- production TUI.

Implementing them deeply in Python first would create immediate rewrite and, for
K1/lifecycle work, additional security risk.

## TUI and GUI direction

T2 uses **Rust + Ratatui** after R6. The TUI is a client, never another tunnel
owner.

GPUI is a later GUI research candidate only. A future GUI remains another client
of the same control API and should preferably be separately packaged so the
headless/runtime/TUI path does not acquire a graphics stack.

The [G1 research contract](GUI_RESEARCH.md) distinguishes direct Rust components
from an optional JavaScript presentation layer hosted by Rust GPUI Shell.
Neither changes the Rust runtime/TUI choice. It also identifies presentation
ideas reusable in T2 without importing a graphical framework.

## Host direction

Arch and NixOS are adapters around one Rust application, not forks.

- Arch may use conventional package/user-unit/file-capability integration.
- NixOS uses generation-safe package/module/wrapper semantics and must not
  persist resolved stale `/nix/store/...` paths.
- service state control and service provisioning are separate concepts.
- one host's acceptance never proves the other's package/privilege behavior.

## Evidence axes

A change may be accepted on one axis and still pending on another:

- Git/merge state;
- protocol product maturity;
- Python/Rust migration ownership;
- current Omarchy live acceptance;
- Arch packaging acceptance;
- NixOS packaging/generation acceptance.

Keep those statuses explicit in PR bodies and handoffs.

## Current priority

For the integrated native application:

1. preserve #238/#240 integration and their exact evidence; do not reopen the
   completed main-publication/branch-reconciliation task;
2. complete native distribution/release gates separately; marketplace text,
   screenshots and publication remain owner-controlled;
3. investigate the preserved network/DNS failure and AUTO-1 under their own
   reproducible host gates, without relabeling them PASS;
4. start a bounded T2 client slice only when separately authorized; it must use
   the existing Rust runtime, never become another tunnel owner;
5. keep V0/P4 maturity and Arch/NixOS acceptance tied to their own evidence.

Do not repeat completed R0–R6 work from an older checkpoint or build new
application features in the retired native Python path.
