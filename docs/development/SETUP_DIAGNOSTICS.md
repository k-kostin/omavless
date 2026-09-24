# Native setup diagnostics — #271

RC development candidate, 2026-09-24; not a main/release update.

## Applicability decision

The three bundled native templates retain `route-exclude-address` and do not
enable `auto-redirect`. The local host is Try Omarchy ARM64, kernel
7.2.0-2-aarch64-ARCH, installed Mihomo 1.19.31. Its existing connected Routing
observation has one owned core/TUN, verified controller configuration and no
manual recovery. This does **not** reproduce the fork's kernel-6.12 nftables
failure and does not identify the cause of historical provider/DNS failures.

Upstream [TUN configuration](https://github.com/MetaCubeX/Meta-Docs/blob/main/docs/config/inbound/tun.en.md)
distinguishes automatic routing, redirect/firewall handling and route exclusions.
The public [Mihomo TUN-start error report](https://github.com/MetaCubeX/mihomo/issues/2052)
supplies an actual `Start TUN listening error: initialize auto redirect:` message
shape. That report is an observation from another host, not local acceptance.

**Decision:** implement bounded setup-log hints; do not enable auto-redirect,
strip route exclusions, install a firewall workaround or weaken capabilities.
Reproduction of a firewall/kernel defect requires the affected host/configuration
and a separately isolated experiment; the normal user's network is not a test
fixture to damage. Current evidence is insufficient to select a routing change.

## Executable diagnostic boundary

`omavless diagnostics setup` maps only to empty-parameter `diagnostics.setup`.
The existing private same-user socket, committed-owner fence, serialized read,
revision and runtime instance apply. No generic method, path, command, URL,
interface or repair argument is accepted. Reading cached counters performs no
provider/controller/network request or mutation and works without a valid profile
store. Older daemons return unavailable/unknown capability, never fake zeroes.

Response `schemaVersion: 1`, scope `latest_owned_core_setup_log_hints` contains:

- availability `observed` or `unavailable`; missing capture means null counts;
- fixed u32 counts: `tunSetup`, `firewallSetup`, `setupPermission`,
  `otherWarnings`, `oversizedLines`;
- capture `finished` / `incomplete` indicators (null if unavailable);
- fixed interpretation `log_hints_not_cause_or_health` and remediation
  `inspect_host_setup_no_automatic_repair`.

These counters overlap: a TUN-start firewall permission error can increment all
three. They describe the latest owned child's capture, which can be finished;
they are not current health, verified root causes or proof of an OS denial.
No log record leaves the collector. Collection retains its 4-KiB line cap,
bounded draining, zeroed temporary buffers and saturating counters. Unknown or
oversized lines never become fabricated setup diagnoses.

TUN classification requires the specific TUN-start error context at a recognized
warning/error/fatal level. Firewall additionally requires redirect initialization
and nftables/iptables context; permission additionally requires permission-denied
or operation-not-permitted text. Bare errno, informational text and ordinary
connection failures do not justify those categories. Even matching text is only
a hint; no automatic action or concrete repair recommendation follows it.

The old `runtime.observation.coreDiagnostics` wire shape is unchanged, including
its warning counts. Separate IPC avoids making older strict plugin parsers reject
connection health because of new diagnostic fields. No UI redesign or claimed
fix of the external firewall issue accompanies this CLI/support slice.

## Acceptance boundary

Deterministic tests cover message context, unknown/oversized cases, privacy,
legacy response shape, exact CLI arguments, actual Unix-socket response bounds,
owner revocation, unchanged store/revision and no host mutations.

Before completion: full Rust/developer/CI checks plus installed exact-candidate
read against a real capture, legacy frontend/health compatibility and preserved
core/TUN/controller state. Do not provoke persistent firewall damage for a
positive counter. Zero counters are valid data, not proof that setup is healthy.
Known failing physical-kernel scenarios remain unverified unless reproduced on
that host; ordinary collector/IPC acceptance is supported on this ARM64 VM.
