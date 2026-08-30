# OmaVLESS development workflow

Status: repository workflow policy, updated 2026-08-30.

This workflow applies to current plugin work, incremental Python -> Rust
migration, standalone Arch/NixOS packaging and later TUI work.

`AGENTS.md` is the mandatory entry point. Backend/runtime/protocol/TUI work must
also read `RUST_MIGRATION.md`.

## 1. One long-lived branch

`main` is the only long-lived development source of truth.

Do not maintain permanent `alpha` and `beta` branches. A normal feature branch
+ Draft PR already expresses an alpha state. A cloud-ready Draft with local
gates pending expresses the next maturity state.

A temporary `beta/<scope>` branch is allowed only for a named integration/soak
assembly when several accepted candidate heads genuinely need combined testing.
Fixes return to the owning PRs; beta is deleted afterward.

## 2. Short-lived branch roles

Use narrow branches such as:

```text
codex/<topic>
fix/<topic>
docs/<topic>
```

For Rust migration prefer stage/scoped names which make ownership obvious, for
example:

```text
codex/rust-workspace-parity
codex/rust-control-protocol
codex/rust-vless-adapter
codex/rust-store-migration
codex/rust-mihomo-controller
codex/rust-runtime-cutover
```

Do not put R0-R6 into one giant branch.

## 3. Stacked branches

Stack only for a real dependency.

After predecessor merges:

1. update/rebase successor onto new `main`;
2. resolve shared docs/changelog without dropping either decision;
3. rerun materially affected cloud/parity checks;
4. retarget to `main`;
5. perform successor's own local gates;
6. merge only with separate owner approval.

Do not ceremonially rebase a long-lived evidence PR such as V0 solely because
unrelated docs/UI work advanced `main` if its exact tested patch remains valid.
Rebase when its runtime relationship materially changes or remaining evidence
becomes actionable.

## 4. PR maturity states

### Draft / cloud work

Implementation or cloud/parity validation incomplete.

### Cloud-ready

Exact remote head passed all available deterministic/static/parity checks but
one or more declared host/integration checks remain.

### Local-validation pending

Useful when explicitly waiting for Try Omarchy, bare-metal, Arch or NixOS
acceptance. Keep unchecked list visible.

### Merge-ready

Exact merge candidate passed declared cloud/parity/local gates, diff was
re-reviewed after any rebase/fix and owner approval is given.

### Merged

Accepted implementation is in `main`.

### Experimental

Independent product maturity label. A merged protocol can remain Experimental
while broader provider/server evidence is missing.

### Migrated

Do not use this word loosely. For a subsystem it means Rust is the accepted
production owner and any retained Python code is explicitly oracle/rollback,
not another live owner. Full project migration is not complete until R6.

## 5. Two active development lanes

### Plugin completion lane

Current QML/plugin work may continue while Rust migration proceeds:

- localization;
- presentation/accessibility/navigation;
- current diagnostics/routing/import/subscription UX;
- bug/security fixes;
- V0 evidence when fixtures exist.

Do not freeze a current bug because the backend will eventually be Rust.

### Rust migration lane

R0-R6 proceeds according to `RUST_MIGRATION.md`.

Avoid expanding large Python backend areas which are about to move. Once R2 is
available, new large protocol adapters are Rust-first. P4 WG/AWG depends on R2
as well as V0.

## 6. Rust migration PR template requirements

Every R-stage PR body must answer:

1. What exact subsystem is moving?
2. Who owns it before this PR?
3. Who owns it after this PR?
4. What language-neutral contract/fixtures existed before replacement?
5. What Python/Rust differential or golden result passed?
6. Are any differences intentional? Why are they contract-correct?
7. How are credentials/private fixtures protected?
8. Does production plugin/runtime path change?
9. What host gate is required?
10. Can Python code be removed, or is it still oracle/rollback?
11. What exact head SHA was tested?

A PR which only shows `cargo test` against newly written Rust tests does not
establish migration parity.

## 7. Establish reference behavior before replacement

Before rewriting a subsystem, capture the behavior worth preserving in a
language-neutral form whenever practical:

- accepted/rejected fixture corpus;
- stable machine errors;
- canonical normalized model;
- redacted preview;
- generated core config semantics;
- migration outputs;
- routing outcomes;
- lifecycle state transitions.

Do not snapshot secrets into Git.

Known Python bugs are handled by explicit contract correction + regression test,
not blindly duplicated in Rust.

## 8. Differential test safety

- credentials never in argv;
- private fixtures untracked/mode `0600` where applicable;
- CI uses only credential-free fixtures;
- failure output does not dump raw profile/subscription input;
- shareable parity reports use IDs/categories, not endpoints or keys;
- bounded stdin/private files preferred over ad-hoc environment variables for
  sensitive comparison input.

## 9. Cloud agent responsibilities

Cloud agents may:

- research upstream behavior;
- implement bounded QML/Python/Rust changes;
- add language-neutral fixtures;
- run deterministic differential/static/security checks;
- prepare Draft PRs;
- rebase genuine stacks;
- update canonical docs.

Cloud agents must not claim unrun:

- Quickshell integration;
- real TUN/routes/systemd ownership;
- live provider/server interoperability;
- physical network behavior;
- Nix generation/package behavior;
- Python-absence R6 product behavior unless actually exercised in an installed
  environment.

## 10. Try Omarchy responsibilities

For applicable current plugin/R4/R5 candidates:

1. fetch exact candidate SHA;
2. verify base/diff;
3. run `omarchy plugin validate`, tests, installed QML checks and relevant
   Rust/Python checks;
4. execute PR-specific TUN/systemd/core/server checklist;
5. keep credentials/endpoints/provider names out of public output;
6. report failures on owning PR;
7. rerun affected gates if head changes;
8. merge only after explicit approval.

Try Omarchy is ARM64/virtualized evidence; label it honestly.

## 11. Bare-metal responsibilities

Bare-metal is secondary confidence unless concrete hardware-sensitive behavior
requires it. Required examples include physical NIC transitions, suspend/resume,
x86_64-only behavior or host firewall/kernel behavior not faithfully represented
in Try Omarchy.

Do not demand bare-metal merely because old roadmap text did.

## 12. Standalone Arch/NixOS responsibilities

### Arch

When R5a begins, test native package/runtime path independently of plugin-only
acceptance: install/update/remove, service, Mihomo/TUN readiness, CLI/runtime,
store migration, restart/rollback and Python-free operation when R6 applies.

### NixOS

When R5n begins, test package/module/wrapper semantics, stable entry points,
user service, privilege negative case, generation update/rollback, garbage
collection/stale store path and shared state migration.

Never reuse Arch evidence as Nix package proof.

## 13. R6 special procedure

R6 must intentionally make Python unavailable to the installed normal
OmaVLESS path and exercise all required production operations. Merely not
observing a Python process during one happy-path connect is insufficient.

Record:

- package/head SHA;
- how Python was made unavailable to OmaVLESS;
- plugin bridge result;
- status/import/subscription/routing/connect/diagnostic/startup coverage;
- process tree showing no normal `backend.py` owner;
- recovery/rollback result;
- no venv/pip/runtime-Python installation requirement.

Only after this gate may T2 production implementation begin.

## 14. TUI workflow

Before R6: research/design/prototypes only.

After R6:

- Ratatui client is developed in narrow T2 PRs;
- it consumes only semantic Rust control API;
- no Python compatibility calls;
- concurrent plugin/TUI actions must serialize;
- terminal close/reopen must not disturb tunnel;
- Omarchy launch/focus/theme checks are distinct from standalone Arch/Nix TUI
  behavior.

## 15. Exact SHA discipline

Every handoff involving runtime/migration/local validation records:

- base SHA;
- feature head SHA;
- dependency PRs/stages;
- cloud/parity checks;
- unchecked host gates;
- current owner language/path.

If head changes, previous evidence applies only where unaffected; repeat
materially affected checks.

## 16. Release and marketplace model

Keep concepts separate:

```text
main                 accepted development history
version tag          immutable project release
marketplace snapshot exact reviewed plugin commit
native package       exact built/tagged application release
```

Current marketplace 0.7.0 remains its reviewed SHA even as `main` adds Rust
migration work.

The marketplace plugin never silently builds Cargo sources or installs the
standalone package.

## 17. Normal flow examples

Plugin/UI feature:

```text
main -> feature branch -> Draft -> cloud/visual gate -> merge
```

Rust migration slice:

```text
main
  -> establish/reference fixtures
  -> Rust candidate + differential gate
  -> host gate if applicable
  -> Rust ownership cutover for slice
  -> merge
  -> later PR removes Python oracle when safe
```

T1/R5:

```text
Python plugin owner
  -> Rust runtime package present but inactive
  -> migration preflight
  -> explicit one-owner cutover
  -> plugin semantic bridge
  -> compatibility window
  -> R6 Python retirement
```

This workflow exists to avoid two failure modes: a giant unverifiable rewrite,
and a permanent hybrid product which requires both Python and Rust forever.
