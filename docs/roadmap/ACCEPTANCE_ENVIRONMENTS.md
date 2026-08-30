# OmaVLESS acceptance environments

Status: canonical acceptance policy, updated 2026-08-30.

This document defines how evidence is classified for current plugin work,
Python -> Rust migration, future standalone Arch/NixOS packaging and later TUI
work. It supplements `AGENTS.md` and the feature-specific contracts.

## 1. Evidence layers

### Cloud/static evidence

Includes:

- deterministic Python tests;
- Rust `cargo` build/test and lint/format checks selected by the PR;
- language-neutral golden fixtures;
- Python/Rust differential parity tests;
- protocol/property/fuzz tests where bounded and appropriate;
- code review/static/security checks;
- documentation/research work.

Cloud parity evidence is required for a Rust migration slice but does not prove
real TUN/service/network behavior.

### Try Omarchy ARM64 integration evidence

Omarchy under Try Omarchy on Apple Silicon remains an accepted local integration
environment for normal current plugin/runtime work, including:

- Quickshell/QML integration;
- user-systemd;
- Mihomo/private controller;
- TUN/routing/mode transitions;
- lifecycle/connect/disconnect;
- plugin enable/disable;
- current live provider/server checks;
- Rust compatibility bridge and runtime-cutover behavior which is not
  hardware-specific.

For ordinary current-host runtime changes, green exact-head cloud/static checks
plus the declared exact-head Try Omarchy checklist are sufficient for merge
eligibility, subject to review and explicit owner approval.

### Bare-metal Omarchy evidence

The physical Omarchy PC is recommended independent confidence by default, not a
general merge prerequisite.

It becomes mandatory when the task concretely depends on behavior Try Omarchy
cannot represent reliably, for example:

- physical Wi-Fi/Ethernet/NIC behavior;
- suspend/resume or real network transitions;
- x86_64-specific behavior;
- host firewall/kernel/driver behavior sensitive to virtualization;
- hardware-specific routing/network edge cases;
- a defect reproduced only on physical host.

Do not retain a hard bare-metal gate solely because old wording said "real
Omarchy".

### Standalone Arch evidence

When R5a/package work begins, record Arch application evidence separately from
Try Omarchy plugin integration. Relevant gates may include:

- native `omavless` package install/update/remove;
- packaged user service;
- Mihomo discovery and TUN privilege setup;
- plugin-free CLI/runtime operation;
- Full/Routing/Direct lifecycle;
- store migration;
- restart/rollback/recovery;
- Rust runtime with no hidden Python dependency.

An Omarchy test on Arch can contribute, but standalone package claims require an
explicit standalone application path.

### NixOS evidence

NixOS support has its own host gate. It is never inferred from Arch or Try
Omarchy.

At minimum, before support is advertised:

- package/module evaluation and installation;
- stable app/core entry points;
- user-service lifecycle;
- wrapper/service TUN privilege and negative permission case;
- generation update;
- generation rollback;
- garbage collection/stale `/nix/store` path regression;
- shared private-store migration;
- normal Rust runtime/CLI behavior.

If a future Nix-backed Omarchy environment exists, record Omarchy plugin
integration on it as an additional layer rather than relabeling generic NixOS
evidence.

## 2. Rust migration evidence

A migration PR needs evidence on two axes:

1. **semantic parity/contract evidence**;
2. **real-host behavioral evidence** when the subsystem touches OS/network
   behavior.

Rust-only tests are not sufficient proof of migration correctness.

Every R-stage PR records:

- Python/reference owner before change;
- Rust candidate/owner after change;
- corpus/fixtures used;
- exact parity result;
- intentional differences and contract justification;
- privacy treatment of sensitive inputs;
- production path before/after;
- whether Python remains oracle/rollback;
- real-host gate required or not applicable.

### Pure/domain slices

For R1-R3 pure protocol/domain state, deterministic differential/golden tests may
be the primary merge gate when production host behavior is unchanged.

### OS/network slices

R4/R5 and any migration of core discovery, systemd, controller, probes, TUN,
routes or lifecycle requires the corresponding exact-head local integration
checks in addition to differential tests.

## 3. R6 Python-absence gate

R6 is not proven by deleting `requirements.txt` or by observing that one command
happens to use Rust.

Acceptance must run the normal installed product path with Python deliberately
unavailable to OmaVLESS and verify that all required production operations still
work:

- status/doctor;
- import/profile operations used by current plugin;
- subscriptions;
- routing/mode handling;
- connect/disconnect/reconnect;
- diagnostics/status;
- startup/autoconnect behavior as applicable;
- plugin bridge;
- service/restart reconciliation.

No `backend.py` process, venv, pip or Python module may be required. Keep the
proof credential-safe and record exact package/head SHA.

Only after this gate may T2 production implementation begin.

## 4. TUI evidence

T2/T3 acceptance adds client-concurrency and UI-lifetime checks:

- plugin and TUI observe same revision/state;
- conflicting actions serialize through one runtime;
- terminal close/crash does not change desired connection state;
- TUI reopen attaches to existing runtime;
- resize/navigation do not corrupt state;
- Omarchy launch-or-focus/theme behavior on Omarchy;
- standalone Arch/Nix behavior without requiring Omarchy launcher/theme;
- TUI contains no Python compatibility path.

## 5. Exact-head rule

Runtime/migration evidence belongs to the exact tested commit SHA. If the
candidate changes, repeat checks materially affected by the change.

Record environment explicitly: cloud/static, Try Omarchy ARM64, bare-metal
Omarchy, standalone Arch, NixOS, or a combination.

## 6. Agent freshness rule

Before planning/gating/reporting:

- refresh remote metadata;
- read current remote `AGENTS.md`;
- read current `DEVELOPMENT_ROADMAP.md`;
- read current `DEVELOPMENT_WORKFLOW.md`;
- for backend/runtime/TUI work read `RUST_MIGRATION.md`.

Do not infer current policy from a stale local branch.

## 7. Security boundary

Acceptance policy never relaxes security requirements.

- Keep profile URIs, UUIDs, passwords, keys, subscription URLs and reusable
  secrets out of commits/PRs/shareable diagnostics.
- Differential tools do not put credentials in argv or public output.
- Private fixtures remain private and bounded.
- Do not weaken OS policy merely to simplify a migration or make a test pass.
- Rust memory/type safety does not replace protocol/input/privacy validation.
