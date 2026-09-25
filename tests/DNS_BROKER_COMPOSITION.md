# Isolated DNS broker composition check

This opt-in check executes the actual Rust broker `Host`/transaction driver,
`HeldTun`, `ManagedResolved`, `Retention`, `Journal`, and the descriptor receive
channel. It does not install or activate the experimental broker.

The outer Python wrapper requires an ordinary user, validates the exact test
binary SHA-256, and starts fresh user/network/PID/mount namespaces. Before device
access or mounting, both wrapper and Rust fixture compare all namespace identities
against the originals. `/run` is a private tmpfs. The network namespace initially
has only loopback; there are no provider profiles or external probes.

A fixed Python producer creates one nonpersistent single-queue `Meta` TUN and
sends its real descriptor through the Rust seqpacket/SCM_RIGHTS channel. The Rust
kernel leaf admits it normally. The fixture starts its own `dbus-daemon`, not a
host or session bus. Typed mock `resolve1` and `systemd1` peers service actual
D-Bus calls. The manager mock receives actual FDSTORE/SCM_RIGHTS and notification
barriers, retains the actual kernel object, and supplies typed descriptor metadata.
The private journal is the production implementation, including fsync/rename.
The test-only context authenticates the mock service; the namespace-root producer
uses a fixture UID channel. This invokes `Host`, not the public `serve` entrypoint:
fixed root installation/enrollment and enrolled-user socket ACL admission remain
separately tested boundaries, not claims established by this composition fixture.

## Run

Build the broker library test binary using `cargo test -p omavless-dns-broker
--no-run`. From the output choose `debug/deps/omavless_dns_broker-...` for
`src/lib.rs`, **not** the separate `src/main.rs` test binary. Calculate its SHA-256,
then run from the repository root:

```text
python3 tests/dns_broker_composition_probe.py ABSOLUTE_TEST_BINARY SHA256
```

The ordinary test suite intentionally ignores the namespace test. The wrapper
runs only `transaction::integration::actual_host_composition` with `--ignored`,
under its isolation guards. It prints ten public PASS classifications or one
bounded failure, never mock/raw backend diagnostics.

## Evidence

Earlier ARM64 VM checkpoint: the initial eight scenarios passed in **20 consecutive fresh
namespace runs** (160 scenario executions). The tested library test executable
SHA-256 was `0a371cafcf5d949366766e6319b158f494bf5a25413c0f485aec6a2349968574`.
This hash identifies that test artifact, not a release or installed helper.

After adding post-Ready policy verification, the final ten-scenario probe also
passed on library-test SHA-256
`cfd62deefdb902d615caa1a996d3bfb1fcc9f26c54bfd50d747c339d067a4ee8`.
The broker's 46 deterministic tests passed (the namespace case is intentionally
ignored by ordinary cargo test). The earlier 160 executions remain evidence
for their original eight-case artifact, not 20 repetitions of the new cases.

- `success`: pending journal and external FD retention precede the first DNS
  write; exact fixed parameters; Ready follows readback; release verifies DNS
  reset before FD removal and journal deletion; final descriptor close removes TUN.
- `denial`: actual AccessDenied reply after the first write produces joined
  compensation, not Ready; reset/FD store/journal cleanup all complete.
- `timeout`: the real method exceeds its deadline and applies late. No automatic
  Revert or FD removal occurs; journal reopening refuses new work, and manager
  retention keeps TUN present after client/broker-side descriptor drops.
- `drop`: after successful apply, broker-owned objects disappear without release.
  No drop cleanup is inferred; the external store keeps TUN, and a new journal
  instance requires recovery.
- `policy`: effective DNSSEC=yes is refused before any DNS write, FD-store
  notification or journal creation; the broker never weakens DNSSEC itself.
- `mismatch`: an acknowledged write with wrong effective DefaultRoute cannot
  produce Ready; joined compensation and full verified cleanup are required.
- `apply_drift` / `release_drift`: a changed setting the broker never writes
  (LLMNR) during apply or after Ready invalidates exclusive ownership. No whole-link
  Revert may erase that foreign change; both actual proof and journal remain in
  quarantine. The pre-Revert comparison is not an atomic lock against root and
  does not fence unknown writes.
- `dns_drift` / `release_dns_drift`: changing the effective DNS server after
  Ready invalidates both active health and normal release. The foreign DNS value
  remains untouched, proof retention remains, and the journal requires recovery.
  Partial-apply compensation remains a different path: it never requires a full
  fixed policy that was not successfully applied.

The `drop` case is **not SIGKILL/PID 1 acceptance**: manager and resolved are
private mock services in the fixture process, and dropping the broker lease is
the simulated ownership loss. Whole installed service crash/reboot, real
systemd-resolved behavior/policy defaults, package installation, and live DNS
packet resolution remain separate gates. No fixture Ready proves internet health.
