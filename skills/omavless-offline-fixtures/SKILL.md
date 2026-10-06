---
name: omavless-offline-fixtures
description: Run OmaVLESS source-only fixtures when HOME Git ancestry or Unix socket path limits break ordinary tests, using the reviewed HOME-backed offline namespace launcher. Not for live VPN, VM, GUI or hardware acceptance.
---

# Offline source fixtures

Use this procedure for the known environment failures `private_file_inside_git`
or `SUN_LEN`/Unix socket path overflow, or when source tests need an offline
loopback-only environment while the development VM belongs to another operator.
Do not treat an unrelated failing assertion as an environment problem.

Read [the launcher contract](../../docs/development/OFFLINE_FIXTURE_TESTS.md).
Keep the source failure and its exact head; do not delete an ancestor `.git`,
weaken private-file guards or move compiler output into quota-limited host `/tmp`.

Create a fresh empty0700 directory under actual HOME cache, disjoint from the
checkout, with `mktemp -d`. Pass its exact path and the exact checkout to
`tools/test-source-sandbox.sh`. The launcher requires already-installed
unprivileged bubblewrap; it never installs tools or falls back to an ordinary
live-host run. Capture output privately outside the writable fixture directory.
Record selected HEAD, launcher hash and actual exit; inspect only relevant
bounded diagnostics after failure.

The launcher runs ONLY `tests/run.sh`: new namespaces have their own loopback,
masked `/run`, read-only host tree and short logical `/tmp` backed by HOME.
Inherited live opt-ins and startup overrides are removed for the child, not
the parent. Host files remain read-only visible; this is trusted-code testing,
not hostile-code isolation. Pathname Unix sockets outside `/run` remain a
separate limitation. No live/installed opt-in or VM operation is authorized.

Rust builds, workspace/terminal gates and their temporary storage retain their
separate HOME-cache procedure. Do not claim this source wrapper executed them.
Preserve scratch after an uncertain/failing run; classify only known completed
fixture outputs before cleanup. A passing source gate is not network, installed
frontend, sleep/NIC or full product acceptance.
