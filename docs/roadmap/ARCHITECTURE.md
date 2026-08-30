# OmaVLESS architecture evolution

Status: selected Rust application architecture with accepted T0 control-plane
and K0 security contracts; not a claim that the standalone runtime/TUI already
exists. Updated 2026-08-30.

Related canonical documents:

- [`RUST_MIGRATION.md`](RUST_MIGRATION.md) — Python -> Rust migration sequence;
- [`CONTROL_PLANE.md`](CONTROL_PLANE.md) — runtime ownership and v1 IPC;
- [`TUI_APP.md`](TUI_APP.md) — Ratatui client/product surface;
- [`PLATFORM.md`](PLATFORM.md) — Arch + NixOS host boundary;
- [`KILL_SWITCH.md`](KILL_SWITCH.md) — fail-closed privileged boundary.

## 1. Product boundary

OmaVLESS remains a censorship-oriented proxy/VPN client built around strict
profile import, provider subscriptions, modern proxy transports and explicit
routing policy. It is not a universal frontend for Proton, Mullvad, Windscribe
or arbitrary NetworkManager clients with unrelated account/daemon models.

The future product has several interfaces but one connection owner:

- Omarchy QML/Quickshell bar — compact quick control;
- Rust Ratatui TUI — full local workspace;
- Rust semantic CLI — scripting/integration;
- optional later Rust desktop GUI client;
- one canonical Rust runtime/control plane.

The interfaces are clients, not independent VPN managers.

## 2. Selected implementation boundary

The implementation target is no longer language-neutral:

```text
QML/Quickshell Omarchy frontend
               |
               v
        semantic control API
               |
               v
       Rust runtime/control
               |
     +---------+----------+
     |                    |
     v                    v
Rust domain/profile   Rust host/core
     logic              integration
     |                    |
     +---------+----------+
               v
          external Mihomo
```

Python remains the current validated plugin backend and temporary migration
oracle only. It is not the target daemon/TUI architecture.

The final normal package should use one primary `omavless` Rust executable for
daemon/CLI/TUI entry points. Mihomo remains separately packaged and externally
supervised. Future privileged NetGuard remains a separate reviewed component.

## 3. Current stable semantics

Existing behavior remains the reference contract while implementation moves:

- strict profile parsing;
- redacted preview;
- private profile/subscription store;
- stable subscription identity;
- Mihomo config generation;
- Full VPN / Routing / Direct;
- Russia/China/Iran routing presets;
- custom rules;
- private-controller diagnostics;
- latency probing;
- lifecycle and conflict handling.

Rust migration must preserve these semantics through differential/golden and
real-host acceptance. Known bugs discovered during parity are fixed explicitly
rather than frozen as compatibility requirements.

## 4. Target source/dependency layers

The exact crate names may evolve, but dependency direction should resemble:

```text
                    clients
       +--------------+--------------+
       |              |              |
       v              v              v
  Omarchy QML     Rust Ratatui    Rust CLI
       |              |              |
       +--------------+--------------+
                      |
                v1 control API
                      |
                      v
              Rust runtime crate
              desired/actual state
              mutation serialization
                      |
        +-------------+-------------+
        |             |             |
        v             v             v
 Rust domain      Mihomo backend   host adapter
 profiles/store   config/control   Arch / NixOS
 routing/subs     probes/status
        |             |             |
        +-------------+-------------+
                      |
                      v
              TunnelCoordinator
                      |
                +-----+-----+
                |           |
                v           v
             core        optional
             process     LeakProtection
                |           |
                +-----+-----+
                      v
               Linux networking
```

Conceptual crates:

```text
omavless-protocol
omavless-domain
omavless-mihomo
omavless-host
omavless-runtime
omavless-cli
omavless-tui
```

Do not let QML import host/core implementation logic. Do not let the TUI access
Mihomo controller secrets. Do not let Arch/Nix packaging semantics leak into
profile/routing code.

## 5. Rust migration is itself an architectural constraint

The project uses a strangler migration, not a rewrite branch.

For each subsystem:

1. freeze/identify current language-neutral semantics;
2. implement Rust candidate;
3. compare Python reference and Rust candidate;
4. run host integration where behavior touches OS/network state;
5. make Rust canonical owner;
6. retain Python briefly only as explicit oracle/rollback when required;
7. remove obsolete Python after the next gate.

Do not create a permanent Python/Rust split where both can independently own the
same connection or state.

`RUST_MIGRATION.md` defines R0-R6 and the hard rule that production TUI begins
only after the normal runtime path no longer requires Python.

## 6. Protocol/domain adapter layer

The stable internal profile model lives above core choice and host packaging.
Adapters own:

- strict extraction/parsing;
- semantic validation;
- canonical profile representation;
- redacted preview;
- endpoint extraction;
- stable subscription identity inputs;
- bounded core representation generation.

R2 migrates existing VLESS/Trojan/Hysteria2/TUIC adapters to Rust.

After R2, new large protocol adapters are Rust-first. P4 WireGuard/AmneziaWG is
therefore gated on R2 as well as its protocol-validation prerequisites. The QML
plugin can still expose these protocols through the compatibility/runtime bridge
without recreating their parser in Python.

## 7. Runtime/control-plane ownership

The Rust runtime owns state that must survive UI lifetime:

- desired/actual VPN state;
- current profile and routing mode;
- serialized store/profile/subscription mutations;
- core lifecycle and selected backend;
- background refresh/reconciliation;
- traffic sampling consumed by multiple clients;
- bounded diagnostics/events/log views;
- later `LeakProtection` coordination.

Closing Quickshell, panel, terminal or TUI must not itself change desired VPN
state.

The private v1 socket is `$XDG_RUNTIME_DIR/omavless/control.sock` under a
same-user `0700` directory with `0600` socket and peer verification. Clients use
semantic actions rather than core/systemctl/shell forwarding.

## 8. `CoreBackend`

`CoreBackend` represents a validated proxy core implementation, not arbitrary
installed VPN products.

Its eventual Rust contract should cover operations equivalent to:

- determine representability of validated profile + mode;
- render private core config;
- validate config with installed core;
- start/stop/adopt owned core;
- bounded health/status;
- advertised optional diagnostics/capabilities.

Optional capabilities remain explicit:

- Routing;
- Direct;
- rule providers/presets;
- route inspection;
- traffic/accounting;
- active connection inspection;
- provider refresh.

Clients consume capability state; they do not accumulate `if core == ...`
branches.

### Mihomo backend

Mihomo remains primary/full-featured. Existing product semantics are
reference behavior and must not be weakened to simplify abstraction.

### Xray backend

Xray remains deferred compatibility work after the Rust runtime is stable and a
real common Xray-only semantic gap is confirmed.

Initial production slice, if justified:

```text
Full VPN   supported
Routing    unavailable
Direct     unavailable
```

Core selection is automatic/capability-driven first. A manual preference toggle
is not an initial requirement.

## 9. `TunnelCoordinator`

One Rust state machine owns tunnel transitions.

Keep distinct:

- **desired state** — user intent;
- **actual state** — `starting`, `connected`, `reconnecting`, `failed`,
  `stopping`, `disconnected`.

This distinction is required for multiple clients and fail-closed behavior.

Owned-core transition concept:

```text
desired = connected
prepare replacement
arm protection if configured
stop old owned core
verify old capture path removed
start replacement
verify capture/connectivity
desired remains connected
```

Failure must have explicit rollback/blocked state and must not silently restore
ordinary direct internet while user intent remains protected.

Concurrent bar/TUI/CLI commands serialize through this coordinator.

### Foreign VPNs/TUNs

OmaVLESS never owns arbitrary Mullvad/Proton/Windscribe/Mihoro/V2RayN/OpenVPN/
WireGuard instances merely because they are detectable.

Policy:

- detect likely conflict conservatively;
- explain/refuse/pause when necessary;
- never kill, disable or rewrite foreign client state.

## 10. Host integration boundary

Shared Rust domain/runtime code targets Arch and NixOS without becoming a
generic distro framework.

A narrow host abstraction may report:

- stable core entry point;
- core/TUN privilege readiness;
- runtime-service provisioning readiness;
- host identity/capabilities;
- bounded install/doctor remediation.

Arch may use reviewed file capabilities. NixOS may use wrapper/service capability
provisioning and generation-safe entry points. Desired state never persists a
Nix generation-specific resolved store path.

`systemctl --user` lifecycle may remain common while unit provisioning ownership
differs by host.

## 11. `LeakProtection`

A real kill switch must survive proxy-core/runtime/UI failure and therefore
cannot be only a Mihomo option or QML/TUI switch.

K0 selects a separately packaged root NetGuard service plus fixed-purpose
nftables policy. The ordinary Rust runtime does **not** receive arbitrary root
or firewall authority.

Initial K1 semantics remain:

- opt-in;
- Full VPN only;
- arm while desired Full VPN connection exists;
- core loss/restart does not restore ordinary direct egress;
- explicit disconnect disarms protection;
- no permanent Lockdown in first slice.

K1 implementation waits for Rust canonical runtime ownership rather than being
built as a large Python-plugin subsystem.

A future helper API stays bounded, conceptually around operations such as
`arm`, validated allowance update, `status`, `disarm`; never arbitrary shell,
nftables expressions or user-authored root config.

## 12. App/system proxy boundary

S1 remains outside Full VPN / Routing / Direct. It affects applications honoring
proxy settings/environment and must preserve/restore exact prior state.

Implementation waits for Rust runtime/host ownership. Omarchy must account for
desktop state plus systemd/UWSM environment. NixOS needs its own host-specific
state discovery/restoration contract. No generic reset-to-default shortcut.

## 13. TUI and future GUI

The first full application UI is **Rust + Ratatui**, downstream of R6.
It attaches to the Rust runtime and contains no Python compatibility path.

GPUI is a possible later desktop client. It must not be linked into or required
by daemon/CLI/TUI crates merely because it is Rust. Prefer an optional separate
GUI binary/package if that avoids a graphics-stack dependency for headless/TUI
users.

All clients share the same semantic API and state.

## 14. Implementation tracks

### P — current plugin closure

Continue QML/i18n/UI correctness and existing feature validation while Rust
migration begins. Do not freeze the plugin.

### R0-R4 — backend/domain migration under Python production owner

Introduce workspace/parity infrastructure, then control protocol, adapters,
store/subscriptions/routing and OS/core operations. Python remains production
runtime owner until explicit T1 cutover.

### R5/T1 — canonical Rust runtime

Rust daemon acquires singleton ownership; plugin switches to semantic IPC/CLI;
no second owner.

### R6 — Python retirement

Normal operation must succeed with Python unavailable and no venv/pip/runtime
packages before TUI starts.

### T2+ — Ratatui application and operator expansion

Build full UI only on the accepted native runtime. Add C1/S1/K1/Xray features
there or in the runtime rather than expanding Python just before deletion.

## 15. Testing invariants

For every architecture/migration slice:

- deterministic tests cover state/failure rollback;
- Rust rewrite has Python/contract parity evidence where applicable;
- credential-bearing differential inputs remain private and bounded;
- config generation remains credential-safe;
- only one OmaVLESS-owned full-tunnel core owns active capture path;
- clients serialize against one desired state;
- slow/crashed clients cannot stall lifecycle/background work;
- ordinary status never exposes reusable credentials/controller secrets;
- foreign VPN detection never becomes automatic kill/reconfigure;
- Try Omarchy validates exact-head TUN/routes/systemd/process ownership for
  affected current-host slices;
- Nix/Arch packaging evidence remains host-specific;
- fail-closed slices force process failure/restart, not only happy path;
- privileged helpers get separate threat-model review;
- R6 explicitly proves Python absence from runtime path;
- TUI tests include concurrent plugin/TUI actions and UI close/reopen without
  tunnel disturbance.

The target is one product with one verified Rust connection owner, a preserved
strict security model, a compact Omarchy frontend and multiple native clients —
not a collection of loosely synchronized rewrites.
