# OmaVLESS architecture evolution

Status: design direction, not a claim of current runtime support. Updated
2026-08-26.

This document describes how OmaVLESS can gain an optional Xray core, stronger
VPN ownership semantics and fail-closed protection without turning the project
into a generic frontend for every VPN client or performing a speculative
rewrite of the existing Mihomo implementation.

## 1. Product boundary

OmaVLESS remains a censorship-oriented proxy/VPN client built around strict
profile import, provider subscriptions, modern proxy transports and explicit
routing policy. It is not intended to absorb Proton VPN, Mullvad, Windscribe or
arbitrary NetworkManager clients. Those products keep their own account,
daemon and policy models.

The useful architectural lessons from multi-backend VPN frontends are narrower:

- define a stable backend contract instead of letting UI code know every core;
- centralize tunnel transitions and exclusivity;
- keep leak protection independent from the process whose failure it must
  protect against.

The current Mihomo implementation remains the reference behavior.

## 2. Current stable boundary

OmaVLESS already has a protocol-neutral profile adapter layer. Supported
protocol adapters are responsible for strict parsing, redacted preview,
endpoint extraction, stable subscription identity and generation of a bounded
core configuration. The private store has an explicit protocol discriminator.

Today the final generation step is Mihomo-specific, and runtime state exposes
`core: mihomo`. That is acceptable while Mihomo is the only production core.
The project must not introduce an abstract interface merely to make the source
look theoretically multi-core.

The extraction point becomes justified when one of two concrete changes begins:

1. an optional Xray backend is required by a common core-level compatibility
   gap; or
2. fail-closed networking requires lifecycle state which should not live inside
   Mihomo-specific service code.

## 3. Target layers

The intended direction is:

```text
OmaVLESS UI
    |
    v
Profile / subscription store
    |
    v
Protocol adapters -> canonical validated profile semantics
    |
    v
Capability resolver
    |
    +---------------------+
    |                     |
    v                     v
MihomoBackend         XrayBackend
primary/full          compatibility-only initially
    |                     |
    +----------+----------+
               |
               v
        TunnelCoordinator
        desired/actual state
        owned-core exclusivity
        foreign-VPN conflict policy
               |
         +-----+-----+
         |           |
         v           v
      core       LeakProtection
      service      (optional)
         |           |
         +-----+-----+
               |
               v
        Linux TUN/routes/firewall
```

These are conceptual boundaries. They do not require separate processes or
Python modules until implementation pressure justifies them.

## 4. `CoreBackend`

`CoreBackend` describes what a proxy core can actually do for one validated
profile. It is not a user-facing list of installed VPN products.

A minimal eventual contract should cover operations equivalent to:

- report whether a profile/mode combination is representable without loss;
- render a private core configuration;
- validate that configuration with the installed core;
- start and stop the owned core instance;
- report bounded connection/runtime status;
- advertise optional diagnostic capabilities.

Optional capabilities should be explicit rather than emulated. Examples are:

- Routing mode;
- Direct mode;
- country/rule-provider support;
- destination-route inspection;
- traffic/accounting details;
- active connection inspection;
- provider refresh.

The QML/backend boundary should consume the advertised capability set. It
should not accumulate scattered `if core == "xray"` branches.

### Mihomo backend

Mihomo remains the preferred and full-featured backend. Existing product
semantics stay authoritative:

- Full VPN;
- Routing;
- Direct;
- Russia/China/Iran policy presets;
- custom rules;
- rule providers and refresh;
- private controller diagnostics;
- route inspection;
- traffic accounting and the existing status model.

No feature should be removed from Mihomo merely because another backend cannot
match it.

### Initial Xray backend

Xray is a compatibility backend, not a replacement and not an initial manual
preference toggle. The capability resolver may choose it only when a validated
profile contains semantics which current supported Mihomo cannot represent
without loss and the Xray path has explicit support for that exact subset.

The first production slice is deliberately:

```text
Full VPN   supported
Routing    unavailable
Direct     unavailable
```

There is no early RU/CN/IR routing parity, custom-rule parity or route-
inspection parity.

Current Xray-core has a native TUN inbound on Linux and can create a TUN
interface; current upstream configuration also includes automatic system route
and outbound-interface controls. That makes a self-contained Xray Full VPN
backend technically plausible rather than requiring the old proposed design of
Mihomo retaining TUN while Xray sits behind a loopback SOCKS hop. See the
upstream TUN documentation:
https://xtls.github.io/en/config/inbounds/tun.html

OmaVLESS still needs its own live tests before relying on those facilities. An
upstream capability is not a product guarantee.

### Core selection

Initial selection should be automatic and explainable:

```text
profile supported completely by Mihomo
    -> Mihomo

profile contains a supported Xray-only semantic
    -> Xray
```

The UI may expose a read-only explanation such as `Core: Xray — required by
this profile`. It should not ask ordinary users to choose a core whose
compatibility consequences they cannot reasonably evaluate.

A manual advanced override can be reconsidered only if real support/debugging
needs appear later.

## 5. `TunnelCoordinator`

Tunnel ownership should become a single state machine instead of being spread
across profile switching, service management, conflict checks and future core
selection.

The coordinator owns intent and transitions, not protocol parsing.

Two states must be kept distinct:

- **desired state** — what the user expects, for example `connected`;
- **actual state** — `starting`, `connected`, `reconnecting`, `failed`,
  `stopping` or `disconnected`.

This distinction is required for a meaningful kill switch: a crashed core may
make actual state `reconnecting` while desired state remains `connected`.

### Owned-core exclusivity

OmaVLESS may automatically stop/reconfigure processes and services it owns.
For example, a future Mihomo -> Xray transition can be serialized as one
transaction rather than two unrelated UI actions.

Conceptually:

```text
desired = connected
prepare replacement
arm leak protection when configured
stop old owned core
verify owned TUN/runtime state is gone
start new owned core
verify capture path and connectivity
desired remains connected
```

A failed transition must have an explicit rollback or blocked/recovery state;
it must not silently fall through to ordinary direct internet.

### Foreign VPNs and TUNs

OmaVLESS does not own Mullvad, Proton, Windscribe, Mihoro, V2RayN, arbitrary
OpenVPN/WireGuard services or unrelated TUN interfaces. Best-effort detection
should remain conservative:

- detect a likely conflict;
- explain it;
- refuse or pause the OmaVLESS transition when necessary;
- never kill, disable, rewrite or silently reconfigure the foreign client.

This is intentionally stricter than a generic multi-VPN controller which owns
all of its backends.

## 6. `LeakProtection`: fail-closed without coupling to Mihomo

A real kill switch must remain effective when the proxy core itself is dead.
Therefore it cannot be implemented solely by Mihomo routing/TUN state, by a QML
switch, or by logic that disappears with `omavless.service`.

The conceptual owner is a separate `LeakProtection` boundary coordinated with
the tunnel state machine.

### Kill switch versus lockdown

The first product should be a **kill switch**, not permanent lockdown.

Kill switch semantics:

- user requests Full VPN connection;
- protection is armed;
- while desired state is connected, loss/restart of the core does not restore
  ordinary direct internet;
- after the user explicitly disconnects, protection is disarmed and normal
  connectivity returns.

A later **Lockdown** mode would be different: direct internet remains blocked
even after the user disconnects the VPN. That is a stronger policy and should
be a separate explicit feature, not hidden behind the same toggle.

### First scope: Full VPN only

K1 fail-closed support should initially apply only to Full VPN.

Full VPN has an unambiguous security invariant: while the user expects the VPN
to be connected, ordinary application traffic must not leave through the
physical network outside the protected tunnel path.

Routing mode is harder. Its healthy state intentionally mixes direct and
proxied traffic, and its rule engine currently lives inside Mihomo. If the core
dies, an external firewall no longer has the same domain/rule-provider
knowledge. A safe fallback could block all internet until recovery, but DNS,
IPv4/IPv6, provider refresh, direct exceptions and reconnection semantics need
an explicit threat model before that behavior is promised.

Direct mode has no reason to arm leak protection.

### External enforcement is required

TUN disappearance alone is not a kill switch. If the core crashes and the OS
restores an ordinary default route, traffic can escape unless an independent
network policy remains active.

On Linux the likely enforcement mechanism is nftables or another OS-level
network policy requiring `CAP_NET_ADMIN` or equivalent privilege. The exact
implementation must be selected during the K0 threat-model phase rather than
hard-coded in this roadmap.

The existing security promise must remain: the normal plugin must not quietly
run `sudo`/`pkexec`, install passwordless policy or feed arbitrary user-writable
configuration to a root process.

### Optional bounded NetGuard-style helper

If a privileged host component is required, the preferred product boundary is
an optional, separately reviewed helper (working name `NetGuard`) rather than
putting arbitrary privileged commands in `backend.py`.

Its API must be intentionally small. Conceptually it may provide operations
such as:

```text
arm
update validated connection allowance
status
disarm
```

The final protocol may differ, but it must not provide:

- arbitrary shell execution;
- arbitrary nftables expressions;
- arbitrary file reads/writes;
- a path where a user-controlled YAML/JSON document is interpreted as root
  firewall configuration.

Rules should be generated from a fixed audited template with strict bounded
inputs. No reusable profile credential belongs in the privileged API.

The threat model must decide how the core's own reconnection traffic is allowed
without allowing ordinary applications to bypass the block. Candidate
mechanisms may include a validated endpoint/tuple allowance, packet marks or a
similarly narrow ownership signal; none is accepted until IPv4, IPv6, DNS,
route changes and crash/restart behavior are tested on the Omarchy machine.

### UI/core independence

Protection state must not depend on the panel staying open or Quickshell
remaining healthy. A core crash should look conceptually like:

```text
desired: connected
actual: reconnecting
protection: armed
user traffic: blocked from direct egress
```

The UI can then report `Reconnecting — internet traffic is blocked` rather than
showing a misleading disconnected state while traffic leaks normally.

## 7. Incremental implementation tracks

This architecture is intentionally evolutionary.

### N0 — lifecycle/coordinator boundary

State: planned only when required by a concrete networking feature.

- centralize desired/actual connection state;
- centralize owned-core transitions and operation serialization;
- preserve conservative foreign-VPN conflict handling;
- keep existing Mihomo behavior unchanged;
- do not introduce a second core merely to test the abstraction.

N0 may be implemented as part of K1 or immediately before X1 if no earlier
feature requires it.

### K0 — fail-closed threat model and host integration design

State: planned design/security phase.

- enumerate traffic that must be blocked and traffic required to reconnect;
- cover IPv4, IPv6, DNS, route replacement, suspend/resume, network changes,
  core crash and shell crash;
- choose the smallest privilege boundary;
- specify install/update/remove and stale-rule recovery;
- prove that disabling/removing OmaVLESS cannot strand an unexplained firewall
  rule;
- define local acceptance and emergency recovery steps before implementation.

K0 changes no networking policy by itself.

### K1 — Full VPN fail-closed kill switch

State: planned after K0.

- opt-in initially;
- Full VPN only;
- protection follows desired tunnel state, not process liveness;
- core restart remains blocked from leaking ordinary traffic;
- explicit user disconnect disarms protection;
- no permanent Lockdown semantics in the first slice;
- local crash/restart/network-change tests are mandatory before merge.

### X0 — core contract extraction

State: deferred until a real second-core need exists.

Extract only the Mihomo-specific lifecycle/config surface needed to implement a
second backend. Do not rewrite protocol adapters or QML wholesale.

### X1 — optional Xray Full VPN backend

State: deferred compatibility work.

Trigger: a common real-world profile depends on an Xray feature which supported
Mihomo cannot represent without loss and cannot reasonably gain soon enough
upstream.

Initial acceptance:

- automatic capability-driven selection;
- Full VPN only;
- native Xray TUN/capture path validated on Omarchy;
- no Routing or Direct mode;
- no hidden Mihomo+Xray two-process chain;
- one owned core active at a time;
- redacted diagnostics and credentials keep the existing security boundary;
- if K1 exists by then, Xray must integrate with the same `LeakProtection`
  contract rather than inventing a second kill switch.

### R1 — backend-neutral routing research

State: deferred, not implied by X1.

Only consider Xray Routing after the project can preserve the observable
semantics of current Mihomo routing: RU/CN/IR policies, custom rules, DNS,
rule-data lifecycle, route inspection and leak safety. Exact parity is more
important than enabling a button.

## 8. Testing invariants

Architecture refactors do not lower acceptance requirements.

For every core/lifecycle slice:

- cloud tests cover deterministic state transitions and failure rollback;
- configuration generation remains bounded and credential-safe;
- only one OmaVLESS-owned full-tunnel core may own the active capture path;
- foreign VPN detection never becomes an automatic kill/reconfigure action;
- Omarchy tests verify the exact PR head, TUN/routes, systemd/process ownership,
  IPv4/IPv6 where available and removal cleanup;
- a fail-closed feature must include forced process death and failed restart,
  not only happy-path connect/disconnect;
- security-sensitive helpers require a dedicated threat-model review and tests
  proving their accepted command/input surface is bounded.

The goal is not backend symmetry. The goal is to preserve the strongest
verified behavior available for each supported configuration while keeping
failure states understandable and leak-safe.
