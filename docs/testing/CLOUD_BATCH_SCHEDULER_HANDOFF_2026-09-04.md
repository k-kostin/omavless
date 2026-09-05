# R5 batch scheduler and fixed commands

Base: #159 `a75d7fbdb790e304c9df5cdf0139187e1d3a2d2f`.
This branch stacks on #159, which converges #154 and #156 (and #155).
Do not cherry-pick the scheduler onto main without those prerequisites.

## Ownership before and after

Python still owns the ordinary installed QML plugin. Its reference is main
`2f9f6bcdbafdfa6b34da50160e7a739618ab814b`, including subscription refresh/store
behavior and the existing language-neutral parity corpus. Rust now schedules
#159's owner/worker composition inside RuntimeServer behind committed native
ownership (including promoted bootstrap candidates). There is still one store
writer. No frontend bridge activation, production cutover or Python removal.

The intentional difference from Python's synchronous refresh-all is a bounded
start/poll/cancel protocol. Network work can outlive a unary request; returning
`ok` for start only acknowledges admission, not refresh success. Terminal
`operation.state`, error and outcome revision are authoritative.

## Implemented

- One supervised worker and at most one retained thread handle per daemon.
- Actual runtime instance ID binds the registry; caller IDs cannot select an
  alternative owner. Restarted instances reject stale handles.
- Shared four-fetch pool with unary subscription requests. Busy yields 20ms;
  no new thread is created per provider and no store/owner lock spans a fetch.
- Production HTTPS transport retains the 25-second total provider budget and
  the worker's 30-minute total job deadline, including redirects and body reads.
- Replay does not fetch again. Admission, progress, cancellation and final
  commit use the existing serialized owner and exact generation/revision/store
  fences. A disconnect during fetch prevents late batch writes.
- Shutdown revokes before joining. In-flight I/O cooperatively drains within
  the provider deadline; cancellation acknowledges promptly but does not
  forcibly interrupt a socket read. Completed records are in-memory, bounded
  and lost on daemon restart; no persistent operation log is introduced.
- A retained private supervisor ticket aborts lost/panicked work or failed spawn.
  Worker unwinding does not poison the owner mutex because transport runs
  outside it. An owner-mutex panic remains fail-closed rather than bypassing
  poisoning. This does not replace Rust's global panic hook.
- Fixed native methods: `subscriptions.refresh_all`, `operations.get`,
  `operations.cancel`. Polling validates the original exact request fields.
  No raw method/JSON command passthrough.

## CLI

Get the actual `instanceId` from `omavless hello`. Choose a fresh, non-secret
correlation operation ID. Repeat exactly the same start command to replay.

```sh
omavless subscription refresh-all INSTANCE_ID OPERATION_ID [REVISION]
omavless operation get INSTANCE_ID OPERATION_ID
omavless operation cancel INSTANCE_ID OPERATION_ID
```

These IDs are correlation metadata, not profile IDs, credentials, endpoints or
subscription URLs. No URI/key/name is accepted by these commands. Private
subscription data stays inside the transport/worker/store. Public responses
contain only bounded operation metadata, counts and stable error codes.

## Validation and remaining work

New deterministic tests cover real Unix-socket acknowledgment/replay/cancel
while transport is paused, one-commit completion, malformed poll rejection,
shared-pool saturation, disconnect fencing, shutdown during a fetch and recovery
from a panicked worker. Fixed CLI mapping/validation has a separate test.
Synthetic reserved-address fixtures only; no real subscriptions are in Git/CI.
Existing workspace/domain parity remains the import/store regression oracle;
these asynchronous scheduling semantics have their own behavior tests rather
than claiming Python timing parity.

Local: 270 Python tests (267 passed, 3 expected skips), QML/i18n contracts,
manifest parsing, shell syntax, Python compile and diff checks. Local Rust and
Omarchy/Quickshell tools are unavailable. Exact Rust workspace, rustfmt, strict
Clippy and parity outcomes are recorded on the PR's final head by Actions.

Still cloud-implementable: wire the frontend compatibility client to this
operation protocol and integrate refresh-all into the controlled R5 transition
path. Do not mark R5/T1 finished just because this scheduler passes tests.

Host gate (Try Omarchy ARM64 is valid): installed ownership-gated daemon,
private socket and capabilities; real provider start/progress/replay/cancel;
status and urgent disconnect during slow HTTP; shutdown/restart without late
store writes or stale-instance replay; deliberate network timeout/error;
normal frontend workflow once its bridge is integrated. No hardware-specific
claim, real-provider interoperability or installed acceptance is made here.

No merge is authorized by this handoff. Main, marketplace, manifest version and
previous feature branches stay unchanged. Next agent must fetch and compare
exact heads before taking over the branch.
