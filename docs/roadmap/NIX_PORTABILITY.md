# OmaVLESS Nix / future Omarchy portability plan

Status: research-backed Nix host design aligned with the selected native Rust
application path; not a claim that Omarchy Cinque has committed to NixOS.
Updated 2026-08-30.

Canonical product/host policy: [`PLATFORM.md`](PLATFORM.md).
Python -> Rust migration contract: [`RUST_MIGRATION.md`](RUST_MIGRATION.md).
Runtime/control API: [`CONTROL_PLANE.md`](CONTROL_PLANE.md).

## 1. Why NixOS is a real target now

Recent upstream Omarchy discussion makes a future Nix-backed package/host model
plausible, not confirmed. The signal that matters for OmaVLESS is that Omarchy
may change its package substrate while trying to preserve a simple user
experience and frontend/plugin model.

That makes NixOS a concrete second host family worth designing for before the
standalone application freezes Arch-only package assumptions.

Roadmap policy:

- Arch remains first-class;
- NixOS is the second initial standalone target;
- future Nix-backed Omarchy is another host/frontend integration over the same
  OmaVLESS runtime, not a fork;
- current plugin/Quickshell semantics are assumed materially compatible until
  upstream proves otherwise.

## 2. Two migrations, one end state

OmaVLESS is changing along two orthogonal axes:

```text
implementation:
Python backend -> native Rust application/runtime

host integration:
Arch-only assumptions -> Arch + NixOS adapters
```

The desired intersection is:

```text
                    QML Omarchy frontend
                            |
                            v
                     semantic v1 API
                            |
                            v
                   native Rust omavless
                  runtime / CLI / Ratatui
                            |
                   narrow host boundary
                     /             \
                    v               v
                  Arch             NixOS
```

Do not solve Nix portability by preserving Python forever, and do not solve Rust
migration by hard-coding Arch package mechanics into the new binary.

## 3. What stays shared

The following becomes shared Rust product logic:

- profile parsing/validation;
- VLESS/Trojan/Hysteria2/TUIC and future WG/AWG adapters;
- private store/subscription semantics;
- Mihomo config generation;
- routing presets/custom rules;
- controller diagnostics;
- latency probes;
- desired/actual runtime state;
- v1 control protocol;
- CLI;
- Ratatui TUI after R6.

Host-specific work remains limited to:

- package/install/update/remove ownership;
- stable application/core entry points;
- TUN/capability provisioning;
- user-service provisioning;
- future NetGuard/nftables packaging;
- host-specific doctor/remediation;
- Nix generation lifecycle.

The expected Nix work is therefore host integration around the native Rust
application, not a second application implementation.

## 4. Current implementation properties which help

The Python reference backend already demonstrates useful portable semantics:

1. Mihomo is not hard-coded to `/usr/bin/mihomo`; discovery accepts
   `OMAVLESS_MIHOMO`, `~/.local/bin/mihomo` and PATH.
2. Private state already uses home/XDG-style locations.
3. Lifecycle uses `systemctl --user`.
4. Protocol/subscription logic does not depend on pacman/AUR.
5. v1 control methods are semantic rather than package-manager/systemctl
   passthroughs.

R2-R5 must preserve these useful boundaries in Rust. Python itself is not a
portability requirement and is removed from the normal runtime path at R6.

## 5. Native package target

A supported NixOS installation should expose a built `omavless` executable and
package/module integration. Ordinary operation must not require:

```text
python
python venv
pip
Python packages
cargo run
Rust compiler/toolchain
```

Cargo and Rust crates are source/build dependencies. Nix may use Cargo metadata
and `Cargo.lock` to build reproducibly, but users run the resulting native
program rather than compiling the application during normal startup.

Mihomo remains a separately packaged external core.

## 6. Store-path hazard: stable entry point versus resolved implementation

Nix profiles/wrappers may provide a stable user-facing path which resolves into
a generation-specific `/nix/store/...` executable.

The runtime must distinguish:

- the stable host entry point it should invoke/re-resolve;
- the current underlying implementation path used only for observation.

Do not persist a resolved store path in desired state or package-neutral private
configuration merely because a filesystem API followed a symlink.

Required acceptance:

- change package generation;
- verify runtime/core entry points are re-resolved correctly;
- garbage-collect old generation paths where safe;
- prove no stale durable path breaks startup/reconnect;
- roll back and prove deterministic behavior.

This applies to the Rust implementation of core discovery as R4/R5 replaces the
current Python `Path.resolve()` behavior.

## 7. Capabilities: main Arch/Nix difference

Current Arch setup can legitimately grant file capabilities to a mutable Mihomo
binary. That is an Arch host implementation, not the universal contract.

On NixOS, immutable store semantics call for a reviewed Nix-native mechanism,
for example a security wrapper or narrowly declared service capability model.
The final mechanism is selected during R5n security/package work.

Shared invariants:

- no arbitrary root shell API;
- no silent sudo/pkexec from QML/TUI/CLI;
- no broad CAP_NET_ADMIN on the normal UI/client;
- no arbitrary user-authored privileged firewall expressions;
- explicit bounded remediation when host privilege is absent.

The runtime/TUI sees semantic readiness such as:

```text
core available
TUN privilege ready
runtime service provisioned
host setup required
```

not raw Nix operations.

## 8. systemd remains a shared lifecycle model

NixOS supports systemd user services, so canonical lifecycle stays:

```text
omavless-runtime.service
start / stop / restart / status
```

Provisioning ownership differs:

```text
Arch package                 NixOS package/module
     |                              |
     v                              v
packaged user unit            declarative user unit
     \                              /
      +-----------+----------------+
                  v
          Rust omavless daemon
```

R5/T1 separates service **state control** from service **definition
provisioning**. The runtime does not rewrite its own package/declarative unit.
Legacy plugin-generated units exist only during transactional migration.

## 9. Narrow host adapter

Do not scatter `if nixos` throughout runtime/domain/TUI code.

Conceptual shared host needs:

- locate stable supported core entry point;
- report network privilege readiness;
- report runtime-service provisioning state;
- provide bounded install/update/remediation hints;
- report host identity/capability state to `omavless doctor`.

Profile parsing, routing, subscriptions, state-machine transitions and control
methods do not belong in this adapter.

## 10. Ratatui consequence

The TUI is written once in Rust and begins only after R6 proves the normal
runtime has no Python dependency.

It connects to the same private socket on Arch, NixOS and Omarchy. Host-specific
presentation is limited to setup/doctor/remediation and theme availability.

Example semantic display:

```text
Arch:
  Core: Mihomo ready
  TUN: file capability ready

NixOS:
  Core: Mihomo ready
  TUN: Nix wrapper/service ready
```

The TUI never invokes `pacman`, `yay`, `nix-env`, `nix profile`,
`nixos-rebuild`, arbitrary Nix expressions or Cargo as part of connect.

## 11. Omarchy plugin assumption

Planning assumes the relevant plugin lifecycle remains materially compatible:

- plugin id enable/disable remains available;
- Quickshell remains a frontend surface or compatible replacement exists;
- plugin can launch/focus `omavless tui` and call semantic Rust runtime bridge.

Launcher/QML-import changes are frontend migration only. Never compensate by
moving package/core lifecycle back into QML.

Desired dependency:

```text
Omarchy bar (Arch-backed or Nix-backed)
                 |
                 v
          semantic control API
                 |
                 v
             Rust runtime
                 |
                 v
          Arch/Nix host adapter
```

## 12. Packaging direction

### Arch

Reviewed `omavless` package contains native Rust application, runtime resources
and package-owned user service. Mihomo remains separate.

### NixOS

Provide Nix package plus the smallest module/wrapper integration required for:

- stable native `omavless` entry point;
- stable Mihomo entry point;
- declarative user service;
- TUN privilege semantics;
- generation-safe updates/rollback.

Prefer normal package/module UX; ordinary users should not need to author raw
generated Nix expressions merely to connect.

### Omarchy

Marketplace plugin remains frontend-only. It detects native application and
shows `Open app`; it does not curl/download/compile Rust, run Cargo, or silently
provision privilege.

## 13. R5n Nix checkpoint

R5n begins once the Rust binary/service/host contract is stable enough to
package without inventing Nix-specific domain semantics.

Required checkpoint:

- Nix package/module evaluates/installs;
- `omavless daemon/status/doctor` use native binary;
- stable app/core discovery across generation change;
- user-service lifecycle;
- wrapper/service privilege positive and negative cases;
- Full/Routing/Direct parity where supported;
- generation rollback;
- GC/stale-store-path regression;
- shared private-store migration/import behavior;
- no second runtime owner.

## 14. R6 Python-absence on NixOS

R6 is a product-wide native-runtime gate. NixOS acceptance should also prove,
when the Nix host implementation is available, that normal installed behavior
does not pull Python/venv/pip in as hidden runtime requirements.

This does not mean the Nix package closure can never contain Python for an
unrelated host/tool dependency; the invariant is that **OmaVLESS production
logic does not require or spawn Python to operate**.

The primary application contract remains the native Rust executable.

## 15. Nix acceptance matrix

Before NixOS support claim:

- package/module evaluation/install;
- package update/remove where applicable;
- native Rust app/daemon/CLI startup;
- stable app/core entry-point discovery;
- user-service login/restart behavior;
- TUN privilege positive/negative cases;
- Full/Routing/Direct lifecycle parity;
- store/subscription/profile migration parity;
- generation update;
- generation rollback;
- garbage collection/stale-path regression;
- credential-safe diagnostics/doctor;
- R6 no-Python production path;
- later NetGuard host integration separately when K1 exists;
- Omarchy plugin/TUI integration separately if an upstream Nix-backed Omarchy
  environment becomes available.

Arch/Try Omarchy evidence cannot substitute for this matrix.

## 16. What we deliberately do not do now

This plan does **not**:

- claim Cinque definitely moves to NixOS;
- add Nix commands to current marketplace plugin;
- keep Python as the future runtime merely because Nix can package Python;
- require Cargo/Rust toolchain at user runtime;
- implement Nix package before the Rust executable/service boundary is usable;
- delay Arch Rust migration while waiting for Omarchy upstream decisions;
- promise Ubuntu/Fedora/general Linux support;
- duplicate runtime/domain/TUI for NixOS;
- embed Nix package-manager operations in semantic VPN API.

The goal is one native Rust OmaVLESS application whose host integration is clean
enough that Arch and NixOS can package/provision it differently without another
application rewrite.
