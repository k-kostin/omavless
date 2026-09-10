# Native explicit connection observation

`runtime.connection_test` / `omavless runtime test` observes one fixed HTTPS
request under current routing. It restores the existing Python/QML exit-IP
observation and adds explicit HTTPS elapsed time, **not ICMP ping parity**.
Targets remain the two literal providers in `Service.qml.exitIpScript`.

Differences: one three-second aggregate budget (fits bounded unary IPC), strict
IP parsing, 64-byte body and 8-KiB headers, no redirects/proxy/cookie state,
platform-verified TLS and fixed GET. Failure is an observation, not a lifecycle
failure. Only explicit UI Test or fixed CLI invokes it; no periodic IP fetching.

Admission requires a committed owner and connected desired/actual snapshot.
Network I/O runs outside owner/migration locks using the existing four-work cap.
After completion an exact snapshot/revision/ownership comparison rejects stale
results. Disconnect can proceed during the probe. No arbitrary URL/interface/
command input is admitted. Responses include only a fixed scope/code, HTTPS
boolean, elapsed milliseconds and the observed IP; the IP is private UI data,
never part of safe diagnostics. No all-routes/TUN/leak-proof claim is made.

The restored Test row uses existing controls/PlainText, respects showExitIp,
and clears results on panel closure or identity/revision change. Old ping and
subscription latency are separate unfinished parity items.

Tests cover fixed targets/fallback, bounded malformed output, private error
non-echo, disconnected/invalid admission, nonblocking disconnect with stale
result refusal, QML generation/revision/privacy boundaries and compilation.
Installed candidate HTTPS and full disconnected/connected host gates remain
required; public reports must omit the observed IP and profile metadata.

Local pre-install gate: 541 runtime-crate tests pass with two test threads,
strict clippy, Python/JS/QML and actual installed-Quickshell component compilation
pass. An initial concurrent-build run hit the existing process-cleanup timing
test (`helper_resources_are_drained_even_after_leader_exit_or_term_spawn`);
the complete isolated-target rerun passes without changing that test or runtime.

## Try Omarchy ARM64 installed observation — 2026-09-10

Combined installed source `fba22a12d82aef75cd1f225eb6e6b592d7c9ec2f`
initially returned `request_failed` repeatedly while the connected Routing
runtime remained healthy. No transport/lifecycle fix was applied. A later
read-only sequence through the same installed `omavless runtime test` returned
HTTPS success three consecutive times, at 1294 / 413 / 415 ms. Observed IPs
were retained only inside the local process and omitted from evidence.

A temporary category-only diagnostic observed one three-second timeout and
later successful responses using the identical TLS/header/proxy policy. Current
fixed-target DNS inventories contained IPv4 addresses only, so an IPv6-first
failure is not established. Remote-route variability remains a possible cause,
not a proven one. `request_failed` means the bounded external observation did
not succeed; it is **not** a VPN connection-failure or leak verdict. The accepted
three-second aggregate budget and certificate verification remain unchanged.

The retained opt-in gate calls the exact production collector:

```bash
cargo test -p omavless-runtime --lib connection_test::tests::fixed_live_https_acceptance -- --ignored
```

It contacts only the two fixed public targets under current host routing,
performs no store/service/tunnel mutation, and never formats the private result
or raw transport errors. It is intentionally excluded from deterministic CI;
external reachability failure is a failed observation, not implementation proof.
This collector gate does not replace installed daemon admission, revision
fencing, UI or TUN attribution acceptance.

Follow-up local gates: five focused deterministic tests PASS (one explicit
network gate ignored by default); the opt-in collector test PASS in 0.56 s;
Rust formatting and `git diff --check` PASS. This follow-up changes only tests
and evidence documentation, not production transport or lifecycle behavior.
