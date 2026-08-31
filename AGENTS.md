# OmaVLESS agent workflow

This file is the mandatory entry point for every coding or testing agent working
on OmaVLESS: browser/cloud agents, Codex Desktop on macOS, Codex CLI inside Try
Omarchy, future standalone Arch/NixOS agents, and agents running on a bare-metal
Omarchy PC.

## Mandatory freshness and roadmap reading

Before planning, implementing, gating or merging work:

1. Refresh remote metadata first (`git fetch origin --prune` or the equivalent
   through connected GitHub tooling).
2. Compare local `main` and the current branch with their remote refs; do not
   reason from a stale checkout.
3. Read the current remote versions of:
   - `DEVELOPMENT_ROADMAP.md`;
   - `docs/roadmap/DEVELOPMENT_WORKFLOW.md`;
   - `docs/roadmap/ACCEPTANCE_ENVIRONMENTS.md`;
   - `docs/roadmap/RUST_MIGRATION.md` for any backend/runtime/protocol/TUI work;
   - the relevant feature/protocol roadmap referenced by the delivery ledger.

GitHub is the source of truth. Do not rely on private chat history, an earlier
agent handoff or a stale checkout as the only record of a decision, test result,
policy or useful implementation.

## Selected implementation direction

Rust is the selected long-term implementation language for the standalone
OmaVLESS application runtime, domain/backend logic, semantic CLI and TUI.
Python is the validated current-plugin reference implementation and temporary
migration oracle, not an open-ended alternative future runtime.

The required end state is:

```text
Omarchy frontend       QML / Quickshell
Canonical runtime      Rust
Domain/profile logic   Rust
Mihomo adapter/control Rust
CLI                    Rust
TUI                    Rust / Ratatui
Python runtime need    none
Primary executable     omavless
External core          Mihomo
```

Read [`docs/roadmap/RUST_MIGRATION.md`](docs/roadmap/RUST_MIGRATION.md) before
changing `backend.py`, control protocol code, profile adapters, subscriptions,
routing, Mihomo lifecycle/diagnostics, packaging or TUI plans.

### No big-bang rewrite

Do not replace `backend.py` wholesale. Migrate one bounded subsystem at a time,
using the existing Python behavior plus language-neutral contract fixtures as a
reference. A Rust migration PR must state what Python owns before the change,
what Rust owns afterward, what parity was checked and whether Python can be
removed yet.

Known Python bugs are not compatibility requirements. If parity exposes an
unsafe or incorrect behavior, fix the explicit contract, add a regression case
and make both implementations obey the corrected semantics before cutover.

### New backend logic during migration

- QML/presentation-only work stays QML.
- Current security/correctness bugs may and should be fixed in Python
  immediately; add a migration-suitable regression fixture.
- Once the owning Rust migration stage exists, new pure profile/store/routing
  logic should prefer the Rust domain boundary.
- After R2, new protocol adapters are Rust-first. In particular P4
  WireGuard/AmneziaWG must not grow a large new Python-only parser without an
  explicit roadmap exception.
- After R5/T1 runtime cutover, new persistent lifecycle/background behavior
  belongs only in the Rust runtime.
- Do not add long-lived Python dependencies for functionality already assigned
  to Rust unless a concrete blocker is documented in the PR.

### TUI gate

Rust + Ratatui is the selected first full standalone UI. Production TUI
implementation does **not** begin until R6 proves the normal application/runtime
path no longer depends on Python. TUI design notes and prototypes may be
researched earlier, but do not use the TUI as a vehicle for hiding unfinished
Python-to-Rust migration.

A later GPUI desktop client is optional and must remain a client of the same
semantic runtime. GPUI is not a dependency of the daemon/CLI/TUI path.

## Current delivery strategy

Two lanes may proceed in parallel:

1. **Plugin completion lane** — finish bounded current plugin/QML work such as
   localization batches, bug fixes, accessibility/navigation, existing
   diagnostic presentation and opportunistic V0 fixture validation.
2. **Rust migration lane** — begin immediately with R0/R1 and then migrate
   deterministic backend layers according to `RUST_MIGRATION.md`.

Do not freeze the current plugin merely because Rust work has started. Equally,
do not expand large Python backend surfaces that are about to be migrated.
The delivery ledger decides which lane owns each feature.

## Acceptance environments

Evidence is divided into distinct layers:

- **Cloud/static evidence** — CI, deterministic tests, Rust/Python differential
  tests, code review, static/security checks and documentation/research work.
- **Try Omarchy ARM64 acceptance evidence** — Omarchy running under Try Omarchy
  on Apple Silicon. This is an accepted local integration environment for
  normal plugin, Quickshell, user-systemd, Mihomo/controller, TUN, routing,
  lifecycle, mode-transition and Rust compatibility-bridge testing.
- **Bare-metal Omarchy evidence** — an additional independent pass on the
  physical Omarchy PC, normally useful for confidence but not a default merge
  prerequisite.
- **Future standalone Arch/NixOS evidence** — package, service, capability and
  host-specific acceptance is recorded separately when those host tracks land.
  Arch evidence never proves NixOS packaging/generation behavior and vice versa.

For ordinary current runtime changes, a green exact-head cloud/static gate plus
the declared exact-head Try Omarchy integration gate is sufficient to make a
change eligible for merge to `main`, subject to normal review and explicit
owner approval.

A bare-metal Omarchy PC pass is recommended secondary verification by default.
It becomes mandatory only when the change concretely depends on behavior Try
Omarchy cannot represent reliably, for example:

- physical Wi-Fi/Ethernet or NIC-specific behavior;
- suspend/resume or real network-transition handling;
- x86_64-specific behavior;
- host firewall/kernel/driver behavior sensitive to virtualization;
- hardware-specific routing/networking edge cases;
- a defect known to reproduce only on the physical PC.

Do not label a check "bare-metal required" merely because an older roadmap or PR
said "real Omarchy". Apply the current acceptance policy.

Try Omarchy evidence must identify itself as ARM64/virtualized and must not
claim untested hardware behavior.

## Rust migration acceptance

Every R-stage PR requires evidence beyond "the Rust tests pass".

At minimum record:

- exact Python reference behavior/fixture source;
- exact Rust candidate behavior;
- differential/parity results;
- intentional differences;
- credential/privacy review of parity tooling;
- production path before/after the PR;
- whether a real host gate is applicable;
- whether Python code is retained as oracle/rollback or can be removed.

Credential-bearing parity inputs never go in argv or public CI. Private local
fixtures stay private; shareable results use sanitized IDs/categories.

For OS/network-facing migrations, differential tests are necessary but not
sufficient: run the normal exact-head host integration gate as well.

## Exact-head discipline

Every runtime or migration acceptance report records the exact tested commit
SHA. If the candidate changes afterward, repeat checks materially affected by
the change.

## Git discipline

- `main` is the only long-lived development source of truth.
- Use narrow feature/fix/docs branches and PRs; do not commit implementation
  work directly to `main`.
- Preserve useful work on GitHub before ending an ephemeral/local VM session.
  Never leave the only copy of a useful commit or test report inside Try
  Omarchy.
- Runtime/security/network and migration fixes should include regression tests
  where practical.
- Keep credentials, profile URIs, UUIDs, private keys, subscription URLs and
  reusable secrets out of commits, PRs and shareable diagnostics.
- Do not weaken OS security policy merely to remove UX friction unless a
  separately reviewed security design explicitly calls for it.
- Commit application `Cargo.lock` once the Rust workspace exists. New Rust
  crates receive the same dependency/security scrutiny as Python/shell
  dependencies.

## Cross-agent handoff

An agent handing work to another environment should report at minimum:

- repository/branch and exact HEAD SHA;
- roadmap stage (`P`, `R0`...`R6`, `T1`, `T2`, etc.);
- what changed and which implementation currently owns the behavior;
- cloud/static and parity results;
- Try Omarchy results, if run;
- bare-metal/Arch/Nix results, if run;
- remaining required gates versus optional confidence checks;
- whether Python remains oracle/rollback for the migrated slice;
- push/PR state.

When an investigation produces findings that future agents need, store a
credential-safe report or update canonical documentation in Git rather than
leaving the result only in chat history.

## Localization work

For translation, locale formatting or localized-UI review, read
[`skills/omavless-localization/SKILL.md`](skills/omavless-localization/SKILL.md)
and [`docs/roadmap/I18N.md`](docs/roadmap/I18N.md) before changing strings.
Catalog/contract tests are not visual acceptance: exercise the declared
screen/state matrix on the exact installed head, protect private fixture data in
screenshots and repeat affected captures after a fix.

## Current continuity checkpoint — 2026-08-31

- Published marketplace baseline remains OmaVLESS `0.7.0` at exact reviewed SHA
  `69fe05b03129a23664fff3f8289821a7b7f80095`.
- D1 / PR `#27` is merged; its selector-readiness and diagnostics acceptance is
  preserved in
  [`docs/testing/TRY_OMARCHY_D1_ACCEPTANCE_2026-08-27.md`](docs/testing/TRY_OMARCHY_D1_ACCEPTANCE_2026-08-27.md).
- V0 / PR `#30` remains Draft at exact tested head
  `a643db595ad5369b5fea200ebc601b4f0f70f18f`: XHTTP representative evidence
  passes, while VLESS Encryption/REALITY PQ, Trojan, Hysteria2 and TUIC fixtures
  remain unavailable. Do not fabricate fixtures or call V0 complete.
- File-picker onboarding (`#41`), unified top-level subscription import (`#42`),
  panel-local keyboard navigation (`#43`) and onboarding/startup localization
  (`#50`) are merged.
- Arch + NixOS are the two initial standalone host families; Omarchy remains a
  first-class optional frontend.
- Rust migration is now active: R0/R1, R2 protocol classification, VLESS
  authority/public preview and strict query/coarse transport-security metadata
  parity are merged. Python still owns the production path and remains the
  oracle. Missing V0 protocol fixtures do not block further deterministic R2
  slices. P4 remains blocked by V0 and should additionally wait for sufficient
  R2 Rust protocol-adapter coverage so WG/AWG is not implemented as a large new
  Python-only parser.

Do not restart D1, ceremonially rebase V0 merely because unrelated `main` work
advanced, or begin production TUI implementation before R6.
