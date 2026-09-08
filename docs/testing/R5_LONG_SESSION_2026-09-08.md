# R5 native operations — local continuation, 2026-09-08

Environment: Try Omarchy ARM64 VM, local Rust/Python tests with installed Mihomo
opt-ins. Starting main: `5575776b365df9daa26587020ec1eaffd7a1d368`.
The installed QML/Python plugin was not cut over or replaced during these
checkpoints. Synthetic isolated stores/sockets/loopback providers are not real
private protocol fixtures or installed native-owner VPN acceptance.

## Accepted checkpoints

| PR | Exact accepted head | Merge | Local Rust gate |
| --- | --- | --- | --- |
| #185 support configuration report | `c67ffb44a0299039e7c9c449ef74c9fc38cd0207` | `9e5de9ccedd3cd0e9e89193307b4fbd68a595144` | 569 passed / 4 ignored |
| #186 onboarding completion | `a1dda59cd62146230a23b71dce66bf40f68af899` | `bd4e9de1d6968b6960105ddf50ddd653e369a928` | 576 passed / 4 ignored |
| #187 provider work adapter | `f45decc24a1764ce562fe2931cc7f05f00ed2b0b` | `1353de6dd9d28b25edc87d6dbe6d68ae5b6a2311` | 572 passed / 4 ignored before context-identical rebase; 7 focused/installed-core tests repeated on accepted head |
| #161 refresh-all scheduler | `aacff3a323d49bac671d78b2a7d04f535a43c79a` | `5fc602f95903fb061dcb13e0d2663f77e663ba0f` | 580 passed / 4 ignored |
| #188 exclusive test fixtures | `958a82c39ee529862668e0437c5e9efb3fd3c23c` | `77939ca98dd148f80d31cad8c70a31bc46435db4` | 582 passed / 4 ignored |
| #190 registered provider refresh | `7a833d71c2ef31bcaa523613a9fb745635fd76d9` | `b452cbe8d8bfe3ae5dd85de19ca43cedfcad4399` | 605 passed / 4 ignored; final commit documentation-only relative to tested `c4dcb589` |
| #191 exact-attribution Routing | `490362bdee21fcdb746168305c4b593d876bf090` | `4f8e03756145b0722f65f103bbe99511740dd005` | 610 passed / 4 ignored |

Every listed PR passed GitHub CI before merge. Full local Rust gates include
fmt, strict clippy, differential parity and installed-Mihomo opt-ins. The
unchanged Python reference suite passed 270 tests with no skips, QML/i18n/search
contracts included; the corrected Routing reference increases that to 272, also
with no skips. Counts are per exact candidate, not additive totals. Installed
Mihomo is v1.19.30, linux arm64, Go1.26.6, with_gvisor.

- Support: 26 actual Python `diagnostics_payload` subset comparisons; bounded
  counts/settings with explicit coverage, no private identifiers or endpoints.
- Onboarding: 21 whole normalized-store comparisons against actual Python main
  dispatch, plus real private-file/socket/CLI and connected-intent preservation.
- Provider adapter: 34 actual Python discovery/path comparisons and 3 actual
  orchestration comparisons; installed isolated Mihomo updated a controlled
  loopback rule provider from one rule to two via private Unix PUT, with no TUN
  or TCP controller and owned-child cleanup. Registration is a separate gate.
- Refresh-all: original stacked commits preserved by range-diff, then narrow
  preset-recovery, shutdown-lock and ambiguous-persistence fixes; 32 focused
  tests plus real private Unix/production HTTP loopback success/failure/cancel/
  revocation. Frontend activation remains separate.
- Registered provider refresh: 19 focused tests, one shared registry/scheduler,
  detached discovery without admission-lock blocking, pinned controller identity,
  snapshot-fenced compensated timestamp commit, and cancellation winning late
  ordinary failures. No installed private-provider/UI claim.
- Live Routing: 35 corrected actual-Python projection cases; 17 focused tests,
  held TCP tuple/PID attribution before query bytes, exact state revalidation,
  and actual Python/native collection against isolated no-TUN Mihomo. Global
  hit counters and unrelated same-destination connections no longer fabricate
  a result. Explicit null connection lists mean pending observation, not success.
  Production filecap/proc visibility can still make the native path unavailable;
  the test-only non-capability copy is never a production workaround.

## Test infrastructure findings

Two full gates caught timestamp-only temporary-name collisions under concurrent
VM tests. The store fixture failed before its operation; the parity fixture's
symlink/write collision changed a checked-in synthetic reference. That reference
was restored byte-exact. Separate test-only commits add process-local sequences;
full gates were rerun successfully. No production store or private data was
involved, and failed runs were not represented as acceptance.
PR #188 then added an exclusive bounded std-only allocator to the five highest
risk fixture groups. Its standalone stress passed 50 runs/1600 parallel
allocations; the complete consuming-workspace gate also passed.

An approval backlog caused later agent builds to overlap in a shared target,
reusing incompatible workspace artifacts. Those failures/results were discarded
as acceptance. Root took exclusive build ownership, cleaned workspace packages
(retaining external dependencies), and reran each affected final gate serially.
Future agents sharing a target must coordinate **approved, actually running**
commands; sandbox process listings do not show the host's full process set.

## Remaining migration boundaries

Provider-refresh orchestration and exact-attribution live Routing are now
implemented behind native ownership, subject to the host limits above. Remaining
work includes subscription latency jobs, host/private-connection diagnostics,
telemetry and explicit probes, login activation/legacy startup conversion,
notification semantics, native chooser compatibility and frontend composition
remain separate owning work. Subsequent PRs must update this list rather than
interpreting these checkpoints as complete installed behavior.

Packaged capability isolation (#178), configured-readiness consolidation (#183),
reversible exact-head cutover/rollback and installed frontend acceptance are real
activation gates. Python cannot yet be removed. V0/#30 remains Draft at its
accepted XHTTP head; this session neither changes it nor supplies missing protocol
fixtures. No new privileged runtime path or OS-security relaxation was added.

## Installed runtime at the end of implementation gates

The installed plugin was not replaced or cut over: enabled, disconnected, saved
Full VPN (`global`) mode, zero status failures. Both legacy/native services were
inactive; Python supervisor/Mihomo/native-runtime/TUN counts were 0/0/0/0, with
no listening OmaVLESS Unix controller and therefore no attributable running
Mihomo TCP controller. No real fixture was read or changed. These are final-state
checks, not a claim that the installed plugin matches the new main runtime code.
