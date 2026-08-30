# OmaVLESS incremental Rust migration contract

Status: selected implementation direction and migration contract. Updated
2026-08-30.

This document is authoritative for implementation language, migration order and
Python/Rust parity gates. It complements [`CONTROL_PLANE.md`](CONTROL_PLANE.md),
[`ARCHITECTURE.md`](ARCHITECTURE.md), [`TUI_APP.md`](TUI_APP.md) and
[`PLATFORM.md`](PLATFORM.md).

## 1. Decision

The long-term OmaVLESS application stack is selected as:

```text
Omarchy bar frontend                optional future desktop GUI
QML / Quickshell                    Rust / GPUI or later reviewed GUI toolkit
        |                                      |
        +------------------+-------------------+
                           |
                    semantic control API
                           |
                           v
                  Rust OmaVLESS runtime
                  Rust domain/core logic
                           |
                 +---------+---------+
                 |                   |
                 v                   v
             Mihomo core       future optional core
             external          compatibility backend

Standalone client:
  Rust CLI + Rust Ratatui TUI
```

The target user-facing application package must not require Python, `pip`, a
virtual environment or Python packages at runtime. The normal end state is one
primary `omavless` Rust executable providing at least:

```text
omavless
omavless tui
omavless daemon
omavless status
omavless connect
omavless disconnect
omavless doctor
```

plus package-owned service/integration resources. Mihomo remains an external,
separately packaged networking core rather than being embedded into the
OmaVLESS executable.

"One binary" means one primary OmaVLESS program for daemon/CLI/TUI and shared
logic. It does not mean statically embedding the Linux kernel, libc, systemd,
Mihomo, configuration files or future privileged NetGuard components.

## 2. Why Rust is selected

The choice is architectural rather than a benchmark exercise.

Rust fits the planned product because OmaVLESS is becoming a long-running local
service plus several clients and contains security-sensitive parsing, bounded
IPC, network/process supervision and cross-distribution packaging.

Expected benefits:

- Cargo dependencies are resolved and compiled during build rather than being
  installed as a user-managed runtime environment;
- Arch and NixOS can package a deterministic native executable without a venv;
- daemon, CLI and TUI can share typed domain models and protocol definitions;
- ownership and type checks reduce classes of lifetime/concurrency mistakes in
  a persistent runtime;
- Tokio-style asynchronous I/O is a natural fit for sockets, probes, process
  supervision and bounded concurrent work;
- Ratatui provides a native terminal UI without introducing a Python runtime;
- the same internal crates can later back an optional GUI client.

Rust does not remove dependencies. Crates remain dependencies of the source and
build graph and are pinned/reviewed through Cargo metadata and `Cargo.lock`.
The difference is that ordinary users do not need to maintain a matching set of
Python modules beside the application at runtime.

## 3. No big-bang rewrite

The current Python backend is validated production behavior and must not be
thrown away in one replacement PR.

Migration uses a strangler/differential model:

```text
same sanitized fixture/input
          |
     +----+----+
     |         |
     v         v
 Python      Rust
 reference   candidate
     |         |
     +----+----+
          |
   compare semantics
          |
       parity gate
          |
   Rust becomes owner
          |
 Python retained briefly
 as rollback/oracle
          |
 later removal gate
```

Each migration slice must be small enough that agents can state exactly which
behavior moved and which Python code still owns the production path.

Do not create a second permanent implementation. Dual execution exists only for
bounded migration evidence.

## 4. What parity means

Parity is defined by the product contract, not accidental implementation detail.

Depending on the slice, compare:

- accepted versus rejected input;
- stable machine error/category;
- canonical profile semantics;
- private/public field ownership;
- redacted preview output;
- generated Mihomo configuration where exact output is contract-relevant;
- subscription identity and duplicate handling;
- state migration results;
- routing decision/configuration semantics;
- bounded status/diagnostic fields;
- lifecycle outcomes and rollback behavior.

Do not preserve a known Python bug merely to produce byte-for-byte equality. If
a differential test exposes a bug or unsafe ambiguity, update the explicit
contract, add a regression fixture and make both sides obey the corrected
behavior before the Rust side becomes canonical.

For purely internal formatting, semantic equality is preferred over brittle
byte equality. For stored schemas, public API envelopes, security-sensitive
redaction and generated core fields, exactness may be required.

## 5. Differential-test privacy rules

Parity tooling must be safe to run with real private profiles during local
acceptance.

- reusable credentials never appear in process argv;
- credential-bearing inputs use bounded stdin, private files or the already
  private store;
- private fixtures remain untracked and mode `0600` where applicable;
- shareable results use fixture IDs/classifications rather than server names,
  endpoints, UUIDs, passwords or subscription URLs;
- neither implementation may dump raw input on assertion failure;
- test logs and CI use only credential-free fixtures.

The existing V0 privacy discipline remains applicable to migration tooling.

## 6. Cargo/workspace target

The repository should evolve toward one Cargo workspace. Exact crate names can
change when implementation starts, but dependency direction should resemble:

```text
crates/
  omavless-protocol/       control protocol/framing/types
  omavless-domain/         profile/store/routing semantics
  omavless-mihomo/         Mihomo config/controller/core adapter
  omavless-host/           narrow host capability contract
  omavless-runtime/        canonical daemon/coordinator
  omavless-cli/            semantic CLI/client helpers
  omavless-tui/            Ratatui client

src/bin or product package
  omavless                 one primary user-facing executable
```

Host implementations may be crates/modules such as Arch and NixOS adapters, but
shared domain/runtime crates must not import Omarchy-specific QML integration.

Commit `Cargo.lock` because OmaVLESS is an application, not only a reusable
library. Dependency additions require the same security scrutiny as new Python
or shell dependencies today.

Avoid PyO3/embedded-Python or a permanent Rust<->Python FFI layer unless a later
concrete blocker justifies it. During migration, language-neutral fixtures and
bounded subprocess/test interfaces are easier to remove and audit.

## 7. Runtime dependency target

The migration is complete only when the normal standalone/package runtime has:

```text
Python interpreter dependency:  no
venv dependency:                no
pip dependency:                 no
Python package dependency:       no
Rust/Cargo toolchain at runtime:  no
```

The Rust/Cargo toolchain is a build/development dependency. Users install the
built package and run `omavless`; they do not use `cargo run`.

Dynamic Linux/system libraries remain normal package dependencies when required.
Full static linking is not a project requirement.

## 8. Relationship with the current plugin

The existing Omarchy plugin remains a supported frontend during migration.
Its QML/Quickshell code is not being rewritten to Rust merely for language
uniformity.

The migration direction is:

```text
current
QML -> backend.py -> Mihomo

transition
QML -> legacy compatibility bridge -> Rust runtime -> Mihomo

final
QML -> omavless semantic CLI/socket -> Rust runtime -> Mihomo
```

The plugin must never own a second Rust runtime while Python still owns the
same tunnel. Ownership cutover is transactional and follows
[`CONTROL_PLANE.md`](CONTROL_PLANE.md).

## 9. Plugin feature work during migration

Current plugin-version work does not stop while Rust migration begins.

Two lanes are allowed in parallel:

### Lane P — finish the current plugin product surface

Continue bounded work which is primarily QML/presentation or completes already
accepted plugin behavior, for example localization batches, bug fixes,
accessibility/navigation, existing diagnostics presentation and opportunistic
V0 fixture validation.

### Lane R — migrate backend/runtime capability to Rust

Begin immediately with isolated components which already have deterministic
contracts and fixtures.

The lanes converge: a large new backend feature should not be implemented
Python-first if the Rust layer which should own it is already being built or can
reasonably land first.

In particular, P4 WireGuard/AmneziaWG parser work should begin only after the
Rust protocol/domain adapter boundary is usable. The goal is to avoid writing a
large new credential parser in Python and then immediately rewriting it.
The current QML plugin may consume the new Rust-owned semantic behavior through
the compatibility/runtime bridge.

Small Python changes needed to keep the current published/plugin branch safe are
still allowed. "Rust target" must never be used as a reason to leave a current
security or correctness bug unfixed.

## 10. Migration stages

The active delivery ledger in `DEVELOPMENT_ROADMAP.md` owns scheduling. The
canonical technical stages are:

### R0 — Rust workspace and parity infrastructure

Goal: introduce Rust without changing production tunnel ownership.

The initial workspace checkpoint uses `omavless-parity` as a reusable result
boundary. Implementation-specific adapters emit bounded sanitized reports; the
comparator reads regular non-symlink files, compares by public case ID and emits
only bounded mismatch IDs. It never executes adapters or prints facts,
fingerprints, paths or parser fragments. Complex/private canonical results use
a fingerprint rather than entering shareable output.

Local compilation requires the Rust toolchain plus a C linker. Omarchy can
install these through Settings and `omarchy pkg add gcc`, respectively; CI runs
the same locked workspace checks on the native runner architecture.

- create Cargo workspace and CI/toolchain checks;
- commit `Cargo.lock`;
- establish credential-free golden/differential fixture format;
- add a bounded parity runner which can compare Python and Rust results without
  leaking private values;
- document crate/dependency direction;
- no QML/runtime cutover and no TUI yet.

### R1 — control protocol parity

Goal: make the already isolated T1a protocol layer the first Rust-owned logic.

The R1 checkpoint adds `omavless-control-protocol`: a pure Rust library for v1
request/response parsing, encoding, stable errors, negotiation and bounded
unary stream helpers. A credential-free differential gate compares the Python
oracle and Rust outcome through the R0 sanitized report boundary. Neither probe
opens a socket, dispatches a method or enters the production plugin path.

- implement v1 frame/envelope validation in Rust;
- reuse `tests/control_protocol_cases/` as a language-neutral corpus;
- compare Python and Rust acceptance/errors/limits;
- add fuzz/property tests where useful for decoder bounds;
- after parity, Rust becomes canonical for the future runtime protocol while
  the Python implementation remains temporarily as a compatibility oracle.

### R2 — profile/protocol adapter parity

Goal: move pure credential parsing and config semantics before new protocol
families expand the Python backend.

Migrate:

- protocol classification;
- VLESS/Trojan/Hysteria2/TUIC parsing;
- canonical profile representation;
- redacted import preview;
- endpoint extraction;
- subscription identity inputs;
- Mihomo profile/YAML rendering semantics.

After R2, new P4 WG/AWG adapters are Rust-first. Python-only implementation of a
new large protocol parser requires an explicit roadmap exception.

### R3 — private store, subscriptions and routing semantics

Goal: move stateful domain logic while Python still remains the production
runtime owner.

Migrate and parity-test:

- store schema validation/migrations;
- favorites/selection/startup semantic state;
- subscription parse/merge/identity/atomic refresh commit logic;
- routing preset/custom-rule semantic model;
- safe import/export transformations;
- deterministic config assembly around migrated adapters.

Remote fetch/process ownership may remain Python until R4; snapshot/commit
semantics must already be language-neutral.

### R4 — Mihomo operations, probes, diagnostics and host readiness

Goal: move OS/network-facing unprivileged runtime capability into Rust.

Migrate under exact behavioral gates:

- stable core entry-point discovery;
- Mihomo config validation invocation;
- private controller client and bounded diagnostics;
- latency/probe orchestration;
- traffic/status sampling;
- owned-process/service observation;
- host capability/doctor model for Arch and NixOS.

Use asynchronous bounded I/O where appropriate; do not turn Tokio into a reason
to widen concurrency or retry semantics beyond the existing contract.

### R5 / T1 — Rust canonical runtime cutover

Goal: make `omavless daemon` the one canonical owner.

- singleton/peer/private-socket boundary;
- desired/actual state reconciliation;
- one mutation queue;
- connect/disconnect/mode lifecycle;
- profile/subscription mutations through IPC;
- Rust host adapter contract;
- packaged Arch runtime first, NixOS host package/module as the second target;
- QML plugin switches to semantic CLI/socket bridge;
- Python backend remains only as explicit disconnected rollback during the
  compatibility window.

This is the T1 control-plane cutover, not an additional daemon track.

### R6 — Python runtime retirement / native application gate

Goal: prove that the product no longer requires Python before TUI implementation
starts.

Required evidence:

- all production backend operations needed by the plugin are Rust-owned;
- normal plugin + standalone package path runs with Python unavailable;
- no `backend.py` process is required for connect, routing, subscriptions,
  diagnostics, startup or lifecycle;
- store migration from existing installations passes;
- rollback/recovery plan is documented and tested before legacy code removal;
- differential/golden corpora remain after Python oracle deletion;
- Arch package contains the native runtime/CLI path;
- Nix package work can use the same binary contract;
- no venv/pip/Python module setup is part of installation or runtime.

Only after R6 may T2 implementation begin.

### T2 — Ratatui TUI

TUI implementation is deliberately downstream of Rust migration rather than a
vehicle for performing it.

- Rust + Ratatui is the selected first full application UI;
- Crossterm or an equivalent reviewed terminal backend may be used;
- TUI is a client of the already accepted Rust runtime;
- it does not contain compatibility calls into Python;
- plugin and TUI concurrently use the same canonical runtime.

Design/prototyping notes may be prepared before R6, but production TUI feature
implementation must not outrun the native-runtime gate.

## 11. Future GUI / GPUI

A desktop GUI is a possible later client, not part of the TUI migration gate.
GPUI is an attractive Rust-native candidate because it can support a rich,
keyboard-first GPU-rendered Linux UI, but it must remain behind the semantic
control API just like Ratatui and QML.

Do not make GPUI a dependency of the daemon, CLI or TUI crates. A future GUI may
be distributed as a separate binary/package (for example `omavless-gui`) so a
headless/TUI installation does not acquire a graphics stack merely to run the
VPN daemon.

GPUI selection becomes final only when a GUI track is scheduled and its Linux,
packaging, accessibility and maintenance costs are re-evaluated. Replacing GPUI
later must not require rewriting the runtime.

## 12. Acceptance rules for every Rust slice

Every R-stage PR must state:

- Python owner before the change;
- Rust candidate/owner after the change;
- exact inputs/outputs covered by parity;
- known intentional differences and why they are correct;
- deterministic tests run;
- real-host checks required or explicitly not applicable;
- whether Python code can be removed yet;
- whether the production plugin path changed.

A Rust rewrite is not accepted merely because tests are green if the tests only
exercise the new code. Migration PRs need evidence against the existing
behavior or an explicit contract fixture established before replacement.

For system/network slices, Python/Rust parity alone is insufficient: exact-head
Try Omarchy/Arch/Nix gates remain required according to the normal acceptance
policy.

## 13. Rules for new backend features

After R0 starts, agents should ask where new backend logic belongs before adding
it.

- QML-only presentation behavior stays QML.
- A current security/correctness fix may patch Python immediately and must add a
  regression fixture suitable for Rust.
- New pure profile/store/routing logic should prefer the Rust domain boundary
  once the owning R stage exists.
- New protocol adapters after R2 are Rust-first.
- New persistent lifecycle/background behavior after R5 belongs only in the
  Rust runtime.
- Do not create new long-lived Python dependencies for functionality already
  scheduled for Rust unless an explicit blocker is documented.

## 14. Definition of migration complete

The migration is complete when all of the following are true:

```text
Omarchy plugin UI:        QML/Quickshell frontend
Canonical runtime:        Rust
Domain/profile logic:     Rust
Mihomo adapter/control:   Rust
CLI:                      Rust
TUI:                      not required for R6, but Rust when T2 lands
Python production path:   none
Runtime Python dependency:none
Primary package binary:   omavless
External proxy core:      Mihomo
```

At that point Python source may remain only in historical tests/migration tools
if there is a concrete reason, but installing and operating OmaVLESS must not
require Python. The preferred final state is to remove obsolete Python runtime
code entirely and keep language-neutral regression corpora as the preserved
behavioral record.
