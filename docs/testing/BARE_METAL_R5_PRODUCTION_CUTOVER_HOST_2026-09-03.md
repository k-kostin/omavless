# Bare-metal R5 production-cutover-host evidence — 2026-09-03

## Candidate

- Repository: `k-kostin/omavless`
- Branch: `codex/r5-production-cutover-host`
- Draft PR: `#140`
- Tested implementation head: `fbe0933db6833b9e4f8e6d6ab12ae3684ed6c83e`
- Base: `3b22cb987007ab85150f5352016267aa15138905`

The candidate composes the accepted pure cutover transaction with the fixed
production paths and services. It does not register a CLI/socket cutover method
or provide the concrete plugin bridge, so this report is not evidence of an
executed ownership cutover.

## Environment

- Bare-metal Omarchy `4.0.2`, `x86_64`
- Installed core: Mihomo Meta `1.19.30`, `linux amd64`
- Core entry point exercised from `~/.local/bin/mihomo`

No private profile URI, endpoint, UUID, key, subscription URL or generated
configuration was copied into this report.

## Static and deterministic gate

At the tested implementation head (plus the report-only descendant carrying
this evidence):

- `./tests/run.sh`: pass; 270 Python tests passed, 2 opt-in core tests skipped;
  i18n contracts, QML contracts and `omarchy plugin validate` passed.
- `./tests/run-rust.sh`: pass; workspace tests, formatting, clippy with warnings
  denied and the R0 parity smoke comparison passed. The runtime crate reported
  179 passing tests.
- The 179-test runtime library suite passed 50 consecutive stress runs after
  test-only executable fixtures were published atomically with a bounded
  settling delay. This addresses an observed overlay-filesystem `ETXTBSY`
  flake without changing production process-spawn behavior.
- Production-host tests cover disconnected commit, exact connected staging,
  strict/duplicate routing-mode rejection, same-candidate verification and
  exact desired-state restoration after a plugin-bridge failure.

## Installed-core gate

With `OMAVLESS_TEST_MIHOMO=~/.local/bin/mihomo`, five opt-in tests passed:

- parent-owned core start/readiness/graceful reap without TUN;
- native host stage/validate/observe/commit/stop without TUN;
- current private-store configuration validation;
- all canonical profile renderers accepted by installed Mihomo;
- private Unix-only Mihomo controller readiness.

These tests used temporary private directories and did not alter user systemd,
the installed plugin, the active tunnel or host routes.

## Read-only live preflight

`omavless cutover-preflight` was run read-only against the existing host. It
observed the legacy owner, one owned Mihomo core, an exact matching active
configuration, no Rust owner/controller and two visible TUN interfaces. It
returned `blocked_inconsistent_host_state`, as required when another TUN makes
exclusive ownership unprovable.

No service start/stop, ownership-marker write, desired-state write, plugin
switch, TUN change or routing change was attempted.

## Result and remaining gate

The fixed production-host composition and local core-discovery correction are
accepted by deterministic and non-mutating bare-metal checks at the tested code
head. A real ownership transition is deliberately still blocked on the
concrete plugin bridge, complete installed fault/crash matrix and a controlled
host state with no foreign TUN. Python remains the installed production owner.
