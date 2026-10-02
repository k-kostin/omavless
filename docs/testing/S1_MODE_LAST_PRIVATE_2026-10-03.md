# S1 private mode-last ordering evidence

Physical x86_64 Linux, private fixture only; 2026-10-03 Moscow (2026-10-02 UTC).
Tested implementation source `b1d16933de17d3fb5afa042381beff8b136524fd`, Draft #539,
stacked on #533 `a374f09fd2fa2c74b5395d21781146091f80f0c3`. Existing #424/#406
foundation and exact #382 test-helper provenance are unchanged. Final evidence-only
head and GitHub checks are recorded in #539; no merge is authorized.

Dependencies remain dconf 0.49.0, GIO 2.88.3 and gsettings-desktop-schemas 50.1.
This is new Rust behavior; no Python archive or production ownership change.

## Executed results

| Gate | Result at the implementation source |
| --- | --- |
| Complete developer/static/QML (`tests/run.sh`) | PASS |
| Complete Rust (`tests/run-rust.sh`) | PASS: 1,177 workspace tests, 11 ignored; format, strict workspace clippy, terminal/TUI feature gates and smoke parity |
| S1 pure/journal/transfer/executor tests within the workspace gate | PASS: 72 tests, including 9 new ordering/refusal cases |
| Optional observer all-feature suite | PASS: 32 tests; 17 installed opt-in tests ignored by default |
| Private installed-dconf sweep | PASS: 14 executed opt-in tests, including 6 new ordering cases; 35 filtered; 64.60 seconds |
| Scoped runtime/observer all-feature/all-target strict clippy | PASS |

Normal Cargo outputs/compilation scratch reuse the existing home target and
short private home TMPDIR. Static Python tests use a tiny private `/var/tmp`
directory because unrelated `/home/kk/.git` makes their home-fixture admission
refuse. No cache duplication, host Git-directory removal or host reconfiguration.

## What the new ordering cases establish

The private test driver applies every non-mode desktop change before manual mode.
At every one of 17 confirmed apply prefixes it retains its originating connection,
reopens the exact journal, requires drain and restores the original layered state.
For a complete application, each confirmed restoration prefix also reenters under
that same port. The first compensation field is saved inactive mode; subsequent
endpoint/PAC/auth/bypass fields remain behind it. Absence, present empty and
equal-default overrides on the full snapshot survive exact restoration.

A stopped fixture service delays the final mode enable, and separately the first
mode disable during compensation. No next field or restore intent can bypass the
undrained request. After resuming that exact owned service, the retained same-port
barrier settles its queue before compensation proceeds. This proves the declared
internal dconf fixture's ordering, not an installed adapter or power-loss guarantee.

An actual child writer sends a durable pending final-mode enable, loses its reply
and exits while the fixture service is stopped. Its controls are already intended
but mode is still disabled. A fresh port refuses the reopened journal. Resuming
the service commits the dead writer's enable: even the now fully intended state
still reports `RetainUnsettledEvidence`, with the same pending mode and exact
durable bytes. Neither state equality nor process exit supplies predecessor drain.

Independent external mode/host edits are preserved without another owned field
write or journal replacement. Synthetic original Manual/PAC settings are refused
before record creation and every private value remains byte/layer exact. The
ordinary executor separately refuses the new order with `ActivationNotAdmitted`;
none of these fixtures supplies an installed listener/authority capability.

Commands (with the private home Cargo target/TMPDIR selected):

```sh
./tests/run-rust.sh
cargo test --locked -p omavless-s1-observer --all-features
cargo test --locked -p omavless-s1-observer --all-features --lib \
  gio_host::persistent_tests -- --ignored --test-threads=1
cargo clippy --locked -p omavless-runtime -p omavless-s1-observer \
  --all-features --all-targets -- -D warnings
```

## Boundaries and unrun gates

Only fixture-created private buses/services/databases/profiles and child processes
are used; each child environment is cleared/rebuilt. All values are synthetic and
PAC URLs are not fetched. Actual desktop proxy, manager environment, real host
session/bus/services, private profiles, credentials, VPN/TUN and routes were never
read or changed. No VM control, production helper, arbitrary command IPC, CLI/UI
capability, installed package, main/RC merge, release or marketplace action.

App proxy remains unavailable. [The ordering contract](../development/S1_MODE_LAST_ORDERING.md)
explains why prior Manual/PAC restoration needs a third-value/staged quiescence
journal rather than an unconditional mode-first reset. Actual broker AUTH-writer
lifetime/session and manager provenance, target/schema/profile continuity,
reviewed installed typed writes/drain, loopback/TUN-disabled owner/revision-bound
listener readiness, UWSM/new-app consumption, conflict escape, cross-owner crash
takeover, power-loss, ARM64/VM and NixOS are unrun, not PASS.
