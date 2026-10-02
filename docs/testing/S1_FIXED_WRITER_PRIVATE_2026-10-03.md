# S1 private fixed-writer evidence

Environment: physical x86_64 Linux, private fixture only; 2026-10-03 Moscow
(2026-10-02 UTC). Tested Rust implementation source:
`d46b843fd1ef9d282ad085b8a643147f8bb2d523`, Draft #533. Dependencies are #424
`1316e2ed1ec211e9c68dd1ac0f22b497549c15f8` and reconciled #406
`472b1992b1a7a09150860808b29814e6b78e1742`. Test-only socket helper correction
from #382 `702b0b2f61e4b5b7e56a1677bde7b865e7d165ce` is retained with provenance.

Installed fixture dependencies: dconf 0.49.0, GIO 2.88.3 and
gsettings-desktop-schemas 50.1. This is new Rust functionality, not a Python
migration. No archive change or production ownership transition occurred.

## Results

| Gate | Result |
| --- | --- |
| Complete developer/static/QML suite (`tests/run.sh`) | PASS on initial implementation; final documentation-head recheck is recorded in #533 |
| Rust workspace and normal format/clippy/TUI/parity gate (`tests/run-rust.sh`) | PASS: 1,168 workspace tests, 11 ignored; format, strict clippy, TUI feature gates and smoke parity passed |
| Complete S1 pure/journal/transfer/executor/review tests | PASS: 63 tests, including actual journal crash boundaries |
| Optional GIO observer all-feature suite | PASS: 32 tests, 11 opt-in installed tests ignored by default |
| Private installed-dconf suite | PASS: all 8 executed opt-in tests, including 6 new writer cases |

Normal Cargo target and compilation scratch are under home. The older long
socket helper caused SUN_LEN failures; its existing correction is reused rather
than duplicated, and a short private home TMPDIR completes the Rust gate. The
Python developer suite needs small `/var/tmp` scratch because an unrelated
`/home/kk/.git` causes its private V0 admission tests to reject home scratch as
inside Git. These are fixture-environment constraints, not S1 host failures.

The writer cases establish complete layered restoration for all 16 desktop
fields, all 17 partial reentry positions, same-connection/owner drain before
restoring a delayed commit, unknown-service refusal, and foreign-edit retention.
The actual crash child journals intent and sends a Change while its fixture
service is stopped, then times out and exits with no drain. A fresh reader sees
the original, but a newly created port cannot claim the reopened journal. On
resuming the service, that old request commits after the writer's death. The
bounded review changes from Original to RecordedMixture while its decision
stays RetainUnsettledEvidence; durable bytes remain pending throughout.

Commands for the optional gates:

```sh
cargo test --locked -p omavless-runtime app_proxy --lib
cargo test --locked -p omavless-s1-observer --all-features
cargo test --locked -p omavless-s1-observer --all-features \
  gio_host::persistent_tests -- --ignored --test-threads=1
cargo clippy --locked -p omavless-runtime -p omavless-s1-observer \
  --all-features --all-targets -- -D warnings
```

## Scope and open gates

The real writer is compiled only into the opt-in crate's tests. Its private bus,
service processes, database/profile, config/runtime and journal are fixture
owned. Values are synthetic; child environment is cleared and rebuilt for that
fixture. The default desktop proxy, manager environment, real user bus/session,
private profiles, host services, VPN/TUN and route state were never read or
changed. Only fixture-owned children were paused/killed/reaped. No package,
main/RC merge, release or marketplace action occurred.

The dconf wire experiment depends on internal 0.49.0 source and is not a selected
installed adapter. It is neither power-loss acceptance nor a new service's
ability to drain a crashed predecessor. Same-owner port retention is narrower
than process-crash takeover. Existing `admit_writes` remains unconditionally
unavailable. See [fixed writer contract](../development/S1_FIXED_TRANSACTION_WRITER.md)
and [unclosed admission edges](../development/S1_ADMISSION_CHAIN.md).

Try Omarchy/ARM64, production host provenance and enable/restore, broker AUTH
writer/session proof, actual installed drain, loopback core/listener readiness,
UWSM/new-app consumption, conflict escape policy, cross-owner crash recovery and
NixOS are unrun. None is PASS, and App proxy remains unavailable.
