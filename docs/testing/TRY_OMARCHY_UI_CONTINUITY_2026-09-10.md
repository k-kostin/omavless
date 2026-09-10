# Try Omarchy native frontend continuity — 2026-09-10

This is an unmerged acceptance checkpoint, not marketplace readiness or R6.
Main remains `27e2e793f0e14f19f41dce947e06667ca9bf5ec3`.

## Installed identities

- Native package: `0.0.0.r393.g039ee482976a-1`, source
  `039ee482976affa7445731812156eee6299b5224` (routing UI plus #218).
- Binary SHA256:
  `7dfacc9305436859a4b1e219f2ca362ac986aec03540edeb1ffb4252caab0100`.
- QML integration: `1eaa73b2fa9fe6cbc048a57ed033fa98d1e9b405` =
  #221 `966f70b` plus #222 implementation `c73bac6`.
- Do not replace the installed binary without preserving #218's private
  controller socket permission fix. QML-only updates do not require core restart.
- Full shell restart was required to replace cached QML in this environment.

## Acceptance

- #219: owner accepted subscription detail navigation and scoped actions.
- #220: owner accepted routing tools/preset UI. Actual installed custom-rule
  list/add/check/delete and mode restoration passed. Runtime suite 540 passed;
  Python/JS/QML and clippy passed.
- #221: real refresh-all start/get succeeded 1/1 through the existing VPN;
  direct network returned safe `core_rejected`. One preliminary connection
  attempt failed and restored disconnected; retry connected successfully,
  verified one core/TUN and owned Unix-controller config, and disconnected.
  Cancellation/uncertain result/epoch recovery have deterministic coverage;
  no live cancellation is claimed. Human new-control/locale review remains open.
- #222: real configuration report parsed and clipboard round-trip passed;
  original text restored. Configuration-only scope, not live support bundle.
  Human Settings/locale review remains open.
- Combined UI: 344 Python tests (4 skips), all JS/QML suites passed, including
  batch 10 and configuration-report 8 focused cases.
- All owning branches pushed as Draft PRs. No merges performed.
- Final runtime: Routing, disconnected, native runtime active, Mihomo/TUN 0/0,
  plugin enabled. No private fixtures or screenshots committed.

## Actual remaining migration gaps

Do not count legacy functions that already have native equivalents as missing.
The remaining substantial gaps are:

1. Profile-file export UI: existing `profile export ID file` and fixed
   `desktop export-file` need private, confirmed frontend composition.
2. Login/autoconnect: offline configure/planner/transaction exist, but production
   registration, trusted login application and installed lifecycle gates remain.
   It is not safe to enable the old checkbox by changing a command name.
3. Manual connection Test, latency/server testing and traffic graph/bar:
   missing exposed native acquisition/scheduling APIs and their UI adapters.
   Local process/controller observation is not an internet-connectivity test.
4. Complete exact-head UI/mode/editor/login acceptance, consolidated stack review
   and explicit Python-unavailable R6 gate. Python cannot yet be removed as oracle.

Reuse old QML dialogs, layout and keyboard navigation; retain useful native
pending/replay/identity controls. No second lifecycle/scheduler state machine in
the UI. Startup and telemetry require their own bounded runtime changes.
