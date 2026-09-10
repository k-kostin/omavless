# Native TUN-bound ICMP observation

This R5 candidate restores the original panel's bounded ICMP RTT/loss semantics
through the Rust owner. Python remains the reference/rollback; this checkpoint
does not complete R5/R6 or subscription latency testing.

## Contract and privacy

`runtime.ping` accepts exactly `{ "host": "1.1.1.1" }`. The fixed semantic CLI
is `omavless runtime ping`, reading one private target from bounded stdin (256
bytes; at most one trailing LF/CRLF). DNS targets are ASCII labels, maximum 253
bytes; IPv4 and IPv6 literals are supported. URLs, scoped IPv6, control bytes,
multiple lines, invalid numeric IPv4, unspecified/multicast/loopback targets and
raw Unicode are rejected. Punycode ASCII labels are allowed, but no automatic
IDN repair is attempted. Empty `pingHost` still disables probing in the UI and
the default remains `1.1.1.1`; no OS locale or canonical private-store mutation
is introduced.

Successful unary responses retain the usual revision and contain:

```json
{
  "schemaVersion": 1,
  "scope": "controller_attributed_tun_icmp",
  "availability": "observed",
  "sample": { "outcome": "reply", "latencyMs": 12.5 },
  "code": "ok",
  "instanceId": "current-daemon-instance"
}
```

A sent-but-unanswered iputils result (exit 1) has `outcome: "loss"`, null
latency and `code: "timeout"`. Missing iputils, rejected binding, execution
failure, malformed output or a locally exhausted deadline yield no sample:
`availability: "unavailable"`, null sample and `code: "probe_unavailable"`.
Admission/stale ownership errors use the existing safe protocol envelope.
No target, interface, profile ID, endpoint, raw output or raw error is returned.
The fixed iputils subprocess necessarily receives its validated destination and
device in argv, as in the original QML command; these are private local process
metadata, not reusable credentials or public diagnostics. No profile credential
or provider URL is ever supplied to that subprocess.

## Host and lifecycle boundary

The native host requires one running owned core, one visible TUN, exact desired
profile/config readiness, and the PID-authenticated private Unix controller.
The device comes only from its enabled TUN config and checked sysfs TUN flags
and ifindex. The result is controller-attributed, not an inference from a guessed
sole device or a claim of kernel file-descriptor ownership.

Only `/usr/bin/ping` is executed, with fixed `-n -q -c 1 -W 2 -I DEVICE -- HOST`,
`LC_ALL=C`, null stdin/stderr and a 4-KiB stdout cap. There is no shell, arbitrary
command API, caller-selected device, source-address fallback or unbound probe.
The old source-address retry is intentionally not preserved because source
binding alone must not be presented as proof of TUN routing. Unsupported host
permissions remain unavailable; no sudo, pkexec, setcap or ping-group change is
performed. Arch declares its ordinary `iputils` package dependency. This does
not establish a NixOS binary/wrapper policy.

One probe may be in flight per daemon, additionally sharing the four-work cap.
Collection is detached from the owner/migration locks. The three-second
aggregate observation deadline includes DNS/process work; child cleanup has a
separate maximum 200-ms reap allowance and stays below the five-second unary
client boundary. The iputils reply wait is two seconds. Before/after snapshots
compare desired state, global revision, store/config digests, controller/core
identity, TUN identity and the host's revocable epoch.

A fixed-purpose host-owned child slot makes cancellation synchronous with
lifecycle replacement: `stop_owned` and `start_prepared` revoke the epoch and
kill/reap the ping before changing/reusing the core/TUN. A queued old worker's
spawn is checked under that same slot mutex and refused. Failed or poisoned
cleanup refuses normal stop/start rather than pretending the child is gone.
Drop/shutdown cleanup is best effort and never claims successful restoration.
The worker polls only its child slot, not the owner mutex. Closing the UI stops
new sampling; the already bounded one-shot may finish, but its UI result is
discarded. No full job scheduler or generic child registry is introduced.

Ping RTT/loss is an observation, not VPN health, remote protocol maturity,
fail-closed protection or proof that every route is proxied. HTTPS exit-IP and
subscription HTTP group-delay probes remain distinct operations.
The core may handle ICMP differently from proxied TCP; these timings must not be
presented as provider/server latency.

## Required evidence

Deterministic gates cover exact target/request/CLI bounds, private-output
non-echo, fixed argv/environment, success versus loss/unavailable parsing,
deadline/output-cap cleanup, revoke-before-spawn, synchronous running-child
reaping before a successor, poisoned-slot lifecycle refusal, shared admission,
and detached store/desired/config/ownership/disconnect fences. The UI retains
the original last-ten-samples mean/loss behavior with its own differential gate.

Installed acceptance remains separate: exact packaged binary identity, verified
TUN binding under real user-service permissions, existing fixture sampling,
no direct fallback, mode/disconnect/replacement cleanup, EN/RU presentation and
no duplicate core/TUN/controller. Do not infer these from deterministic tests.

## Local pre-install checkpoint — 2026-09-10

Backend implementation: `9a34926`; initial QML composition: `5027dd2`.
The runtime crate gate passed 555 tests before the final test-only RTT oracle
addition. The final focused filter passed 12 tests (11 ping tests plus one
existing name match), including ten fixtures executed through the actual
original QML awk expression. All-target strict clippy passes.

The full reference gate passed 343 Python tests with four existing skips;
all JS/QML contracts pass, including nine native ping checks and original
last-ten-samples mean/loss differentials. Python compilation, shell syntax,
manifest JSON, Omarchy plugin validation, actual Quickshell component compilation
and `git diff --check` pass. Component compilation does not instantiate the
panel or establish visual acceptance. Installed live binding/replacement and
human visual gates remain pending on the final combined candidate.
