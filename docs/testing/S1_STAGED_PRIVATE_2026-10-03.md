# S1 private staged quiescence evidence

Physical x86_64 Linux, private fixture only; 2026-10-03 Moscow (2026-10-02 UTC).
Tested implementation `4ffe550f9548ce96e443a7845193af35065333f7`, Draft #543,
stacked on #539 `efe3180fef4810f9f219aa800dcd96a2cf7c3285`. Predecessor
#533/#424/#406 foundations and exact #382 test-helper provenance are retained.
Final evidence-only head/checks are recorded in #543. No main/RC merge authorization.

Fixture dependencies: dconf 0.49.0, GIO 2.88.3, gsettings-desktop-schemas 50.1.
This is new inactive Rust behavior; shared backend contracts, existing record
formats and frozen Python archive remain unchanged.

## Executed results

| Gate | Result at tested implementation |
| --- | --- |
| Full developer/static/QML (`tests/run.sh`) | PASS |
| Full Rust (`tests/run-rust.sh`) | PASS: 1,187 workspace tests, 11 ignored; format, strict workspace clippy, terminal/TUI gates and smoke parity |
| S1 pure/journal/transfer/executor tests | PASS: 82 tests, including 10 new staged cases |
| Optional observer all-feature tests | PASS serially: 32; 22 installed opt-in cases ignored by default |
| Combined private installed-dconf sweep | PASS: 19 executed opt-in tests, 35 filtered; 244.44 seconds |
| Scoped runtime/observer all-feature/all-target strict clippy | PASS |
| GitHub implementation-head checks | PASS: test 2m46s; x86_64 package 1m37s; ARM64 package 2m31s |

Cargo target/compilation scratch reuse the existing home target and short private
home TMPDIR. Static Python tests use tiny private `/var/tmp` scratch because the
unrelated `/home/kk/.git` makes home V0 fixture admission refuse. No duplicate
target directory or host Git/configuration cleanup.

An earlier concurrent run at `1bc72f5819a0ce111bb6da9a6d8f2bf835d322c4`
failed the unchanged AUTH-sender descriptor-cleanup test with `EAGAIN` at
`auth_sender/tests.rs:253` (31 passed, one failed). The complete serial observer
rerun passed. No unrelated AUTH code was changed; this is recorded as a fixture
failure/rerun, not silently counted as a clean first run.

The preceding private sweep was abandoned as invalid when an overlapping
observer feature rebuild invalidated its self-reexec binary path: three
fixture-child spawns returned `ENOENT` at `persistent_tests.rs:179` after 16
cases passed. This is not staged behavior evidence. The final exact-source
private sweep ran with the observer target frozen; no observer recompilation
or source change overlapped its child execution.

## New guarantees exercised

Four pure tests cover six saved layered modes: absent/present None, Manual/PAC
overrides and absent overrides whose defaults provide Manual/PAC. Every apply
and restore prefix exercises both outcomes of its pending fixed field. Control
writes require explicit-none mode; saved mode is restored after all saved
controls. Distinct mode stages survive equal original/intended manual values.
Manager changes, changed locks/defaults, stale owners, unattempted external none
and resets of confirmed controls refuse.

Six journal tests cover every intent/confirmation round trip and exact before/
after pending review, conservative reopen, strict stage/role/key decoding,
separate directories, private record permissions, visible/storage drift and
fsync poison. Actual owned child exits exercise all five storage publication
checkpoints for each of the four mode intents. Early staging remains interrupted;
published intent remains recovered/pending. No host effect occurs in those
storage-fault children, and no process-exit case claims power-loss durability.

Five new private-dconf groups cover all 54 confirmed apply prefixes across saved
None/Manual/PAC, initial and every nonterminal confirmed restoration prefix
under retained-port reentry, and reopened Released tombstones,
all twelve ambiguous four-mode operations and all twelve dead-writer delayed
four-mode operations across those baselines. Absent mode reset is distinguished
from explicit none even when effective mode stays none. Timed-out operations
block subsequent fields until the same origin/unique owner drains. A crashed
writer's later commit remains pending before and after settlement; a fresh port
cannot advance the recovered journal, even when the result equals saved original.
Independent visible mode/control edits preserve their values and journal bytes.
Each fixture independently verifies that its requested absent-none, manual or
PAC baseline actually persisted before constructing the journal; seed setters
and sync are not accepted as that proof.

Only the test driver performs real writes, using the documented internal dconf
protocol and same-connection/owner barrier. This establishes the owned writer's
ordering in the declared private fixture—not CAS across keys, identical-value
edit/ABA detection or installed writer/session authority. Existing apps can cache
settings; desktop mode cannot quiesce environment consumers.

Commands, with the private home Cargo target/TMPDIR selected:

```sh
./tests/run-rust.sh
cargo test --locked -p omavless-runtime app_proxy --lib
cargo test --locked -p omavless-s1-observer --all-features -- --test-threads=1
cargo test --locked -p omavless-s1-observer --all-features --lib \
  gio_host::persistent_tests -- --ignored --test-threads=1
cargo clippy --locked -p omavless-runtime -p omavless-s1-observer \
  --all-features --all-targets -- -D warnings
```

## Boundaries

Only fixture-created buses/services/databases/profiles and synthetic values are
used. Child environments are cleared/rebuilt for those fixtures; synthetic PAC
URLs are never fetched. Actual desktop proxy, real manager environment, host
bus/session/services, private profiles/credentials, VPN/TUN and routes were
never read or changed. Only owned fixture children were paused/resumed/reaped.
No VM control, installed constructor/helper/package, arbitrary command IPC,
CLI/UI exposure, merge, release or marketplace action.

App proxy remains unavailable. [The staged contract](../development/S1_STAGED_QUIESCENCE.md)
keeps installed AUTH-writer/session/lifetime provenance, target continuity,
installed write/drain API, owner/revision-bound exclusive loopback/TUN-disabled
listener, UWSM consumption and conflict escape open. Cross-owner crash takeover,
filesystem power-loss, installed ARM64/VM and NixOS acceptance are unrun, not
PASS. ARM64 CI package build is build evidence only.
