# OmaVLESS control-plane contract

Status: accepted T0 implementation contract, 2026-08-28. This document defines
the T1 boundary; it does not claim that the daemon, IPC API or TUI exists yet.

## 1. Decision summary

T1 introduces one same-user, systemd-managed headless runtime. It is the only
canonical owner of desired connection state, actual connection state, profile
and subscription mutations, routing mode, Mihomo lifecycle, background work
and operation serialization.

The runtime is distributed separately from the Omarchy marketplace plugin. It
exposes a versioned semantic API on a private Unix socket and a small `omavless`
CLI which uses that API. The bar plugin, future TUI and CLI are clients. They do
not start a core, edit canonical state behind the runtime or carry a second VPN
state machine.

The first implementation keeps Mihomo and the existing validated profile
adapters. T1 is not permission to add another core, protocol, privileged helper
or kill switch.

## 2. Ownership and process model

### Runtime process

The packaged user unit is `omavless-runtime.service`. It runs the equivalent of
`omavless daemon` and owns a singleton lock below `XDG_RUNTIME_DIR` before it
opens IPC or performs reconciliation.

At the first T1 checkpoint, the existing `omavless.service` may remain the
private Mihomo supervisor unit. Only `omavless-runtime.service` may prepare its
configuration, start, restart, stop or adopt it after cutover. The old unit is
an implementation detail, not a second control plane. A later rename to a core-
specific unit is optional and must have its own transactional migration.

The runtime owns:

- desired and actual connection state;
- active profile and requested routing mode;
- profile, subscription, favorites, custom-rule and startup mutations;
- generated core configuration and the owned Mihomo service/controller/TUN;
- one serialized mutation queue;
- bounded background refresh, reconciliation and traffic sampling;
- capability discovery and redacted diagnostics;
- local operational events and bounded high-volume views.

The runtime does not own foreign VPNs, arbitrary TUN interfaces, shell
commands, the Omarchy UI process or future privileged networking policy.

### Client responsibilities

1. **Omarchy bar plugin** renders compact state, connect/disconnect, profile and
   mode choices, import/subscription entry points, lightweight diagnostics and
   `Open app`. Its QML may manage presentation focus and temporary form state;
   it does not own tunnel intent or a core process.
2. **TUI** provides the full profiles/subscriptions/routing/diagnostics/traffic
   workspace. Terminal lifetime is client lifetime only.
3. **CLI** validates user-facing arguments, performs one semantic request and
   renders bounded human or JSON output. It is not a Mihomo or shell pass-
   through.
4. **Runtime** validates every request again, owns state and performs work. UI
   availability never gates lifecycle or background work.

## 3. Socket and peer boundary

The v1 endpoint is:

```text
$XDG_RUNTIME_DIR/omavless/control.sock
```

The parent directory is owned by the current user with mode `0700`; the socket
is owned by the same user with mode `0600`. The daemon refuses a symlinked or
wrong-owner runtime directory/socket. On Linux it also checks `SO_PEERCRED` and
accepts only the daemon's own UID. Group or cross-user access is not a supported
configuration.

The socket is private transport, not an authorization to expose everything in
the private store. Ordinary responses never include a profile URI, protocol
credential, subscription URL, controller secret or generated core config.

The runtime holds:

```text
$XDG_RUNTIME_DIR/omavless/owner.lock
```

for its complete lifetime. Failure to acquire the lock exits without touching
the core. This prevents two runtime processes from becoming owners.

## 4. Framing and negotiation

V1 uses UTF-8 newline-delimited JSON. A client opens a connection, sends one
request frame, receives one response frame and closes it. Event watching uses a
separate explicitly requested stream. This keeps a slow client from retaining
a mutation worker or blocking unrelated clients.

- maximum request frame: 64 KiB, including the newline;
- maximum ordinary response/event frame: 256 KiB;
- maximum nesting depth: 16;
- maximum JSON string: 32 KiB unless a method declares a smaller bound;
- unknown top-level fields and duplicate JSON keys are rejected;
- invalid UTF-8, non-object frames, trailing data and a missing newline are
  rejected before dispatch;
- reads, writes and complete unary requests have bounded deadlines.

The first request on a client session is normally:

```json
{"api":"omavless.control","version":1,"id":"hello-1","method":"system.hello","params":{"versions":[1]}}
```

The result selects one version and reports a per-daemon-start `instanceId`,
current state `revision`, API limits and semantic capabilities. Every later v1
request repeats `api`, `version`, `id`, `method` and `params`; no request relies
on connection-global credentials or mutable negotiation state.

A successful response is shaped as:

```json
{"api":"omavless.control","version":1,"id":"status-1","ok":true,"revision":42,"result":{}}
```

An error is shaped as:

```json
{"api":"omavless.control","version":1,"id":"connect-1","ok":false,"revision":42,"error":{"code":"conflict","message":"Another transition is active","retryable":true}}
```

Request IDs are client-selected opaque ASCII strings of 1–64 characters. They
are correlation values, not authority and not profile IDs.

Unsupported versions return `unsupported_version` plus the bounded supported
range. Clients must not guess a compatible method shape. A client which sees a
new `instanceId` after reconnect discards its old event cursor and snapshot,
performs `system.hello` and fetches `status.get` again.

## 5. API categories and v1 methods

Method names are semantic and core-neutral. The exact response schemas are
versioned with the protocol and use bounded enums/arrays.

### System and capabilities

- `system.hello` — negotiate the API and return limits, instance and revision;
- `capabilities.get` — available modes, protocol adapters, diagnostics,
  background and UI capabilities;
- `status.get` — desired/actual state, active opaque profile ID, safe display
  label, mode, core kind/health, protection state and transition summary;
- `events.watch` — bounded state-change events with revision and heartbeat.

### Profiles and imports

- `profiles.list` and `profiles.get` — bounded safe metadata, never URI or
  reusable credential;
- `imports.classify` and `profiles.import` — bounded credential-bearing input
  accepted only in these request bodies and never logged or echoed;
- `profiles.rename`, `profiles.favorite`, `profiles.delete`;
- `profiles.test` — bounded latency/availability result;
- `profiles.export` — explicit credential-release operation for a declared
  `qr` or `file` purpose, never part of list/status/events or diagnostics.

An explicit credential-release response is sensitive local output. The CLI/UI
must require an intentional user action, keep it off logs/argv, and avoid
retaining it beyond the requested QR/file operation. V1 has no generic file-
write method; clients read an import file under their own authority and send
bounded content, or write an explicitly returned export themselves.

### Subscriptions

- `subscriptions.list` — safe metadata and counts without provider URL;
- `subscriptions.preview`, `subscriptions.add`, `subscriptions.update` — the
  bearer URL is accepted only as bounded sensitive request data;
- `subscriptions.refresh` and `subscriptions.refresh_all`;
- `subscriptions.test` — bounded incremental or final availability result;
- `subscriptions.delete`.

Remote content is fetched only by the explicit preview/add/refresh/test path.
Classification alone never fetches arbitrary HTTPS content.

### Connection and routing

- `connection.connect` with opaque profile ID and optional supported mode;
- `connection.disconnect`;
- `routing.set_mode`;
- `routing.custom_rules.list`, `routing.custom_rules.add` and
  `routing.custom_rules.delete`;
- `routing.check` and `routing.refresh_providers`;
- `startup.get` and `startup.configure`.

There is no `run-core`, `systemctl`, controller-forwarding, generic process or
shell method.

### Diagnostics and traffic

- `diagnostics.summary` — credential-safe health and remediation codes;
- `diagnostics.rules` and `diagnostics.providers` — bounded private-controller
  views already reduced by the runtime;
- `diagnostics.export` — returns a redacted document, not an arbitrary daemon
  filesystem write;
- `traffic.summary` — current bounded rates/totals and active count;
- `traffic.connections` and `logs.read` — explicit paged local-only views with
  hard item/byte limits, never embedded in `status.get`.

V1 does not promise an unbounded log tail. If streaming is added, it uses a
separate connection, bounded queue and drop/count marker so a stalled consumer
cannot backpressure lifecycle work.

## 6. Errors, revisions and mutation serialization

Stable machine codes are UI-localization keys. Messages are bounded English
fallbacks and must not contain raw backend/core/controller/provider output.

V1 reserves at least:

```text
invalid_request
unsupported_version
unknown_method
invalid_argument
not_found
conflict
busy
capability_unavailable
permission_denied
core_rejected
transition_failed_restored
manual_recovery_required
daemon_restarting
internal_error
```

`manual_recovery_required` is always non-success and must be rendered as a hard
blocker, never converted to a generic disconnected state.

Every committed state change increments a daemon `revision`. A mutation may
include `expectedRevision`; a mismatch returns `conflict` before side effects.
Clients refresh state and let the user retry instead of silently overwriting a
newer action.

All mutations enter one runtime-owned queue. At most one lifecycle or store
commit is active. An urgent disconnect may move ahead of queued work, but does
not interrupt an unsafe atomic phase already in progress. Network fetches and
probes do not hold the commit lock: they snapshot the relevant generation,
perform bounded I/O, then compare-and-commit exactly as current subscription
refresh does.

Retryable mutations carry an optional client-generated `operationId` of 1–64
ASCII characters. The daemon keeps a bounded per-instance result cache. A
repeat with the same operation ID and request digest returns the prior result;
the same ID with different input returns `conflict`. The cache contains no
credential-bearing request body and is lost safely on daemon restart.

## 7. State classes

T1 keeps state classes physically and semantically separate.

### Persistent private configuration

The initial migration retains `~/.config/omavless/` and its current mode-`0600`
atomic store/config rules. It contains profiles, subscriptions, routing/startup
preferences and reusable credentials. T1 must not create a second store.

### Desired connection state

`$XDG_STATE_HOME/omavless/desired.json` (default
`~/.local/state/omavless/desired.json`) is an atomic mode-`0600` document below
a mode-`0700` directory. It records the intent needed across a daemon restart:
connected/disconnected, requested opaque profile ID, requested mode, schema
version and generation. It contains no URI, endpoint, password, key,
subscription URL or controller secret.

Current `activeId` is migrated deliberately: after cutover it is no longer
treated as proof of actual process state. The daemon reconciles desired state
with the owned service/controller/TUN and publishes actual state separately.

### Actual runtime state

Actual state (`starting`, `connected`, `reconnecting`, `failed`, `stopping`,
`disconnected`) lives in the daemon and may be mirrored in the disposable
bounded `$XDG_RUNTIME_DIR/omavless/status.json` snapshot, mode `0600`. It is
reconstructed after restart. A socket disappearing never means the VPN is
safely disconnected.

### Caches and high-volume data

- provider/rule/download caches belong below `$XDG_CACHE_HOME/omavless/`
  (default `~/.cache/omavless/`) and are replaceable;
- traffic samples, active connections and event buffers are bounded in memory;
- durable operational logs use the user journal with credential-safe messages;
- exportable diagnostics are generated explicitly and redacted;
- no high-volume telemetry or raw log is appended to the tiny desired-state
  document or profile store.

## 8. Restart and stale-client behavior

On startup the daemon acquires ownership, loads/migrates private state, then
inspects the owned unit, process family, controller and TUN before accepting
mutations.

- desired connected + matching healthy owned core: adopt it and publish
  `connected` without starting another core;
- desired connected + owned core absent: publish `reconnecting`, perform one
  serialized bounded recovery and report success or `failed`;
- desired disconnected + owned core running: stop the owned core and verify
  cleanup before publishing `disconnected`;
- inconsistent/foreign ownership: do not kill anything; publish a blocked
  failure with remediation and require explicit recovery;
- more than one owned core/TUN: fail closed for mutations and surface
  `manual_recovery_required` rather than selecting one arbitrarily.

Clients use `instanceId` and revision to detect restart. An in-flight request
whose connection closes has unknown outcome; the client reconnects, reads
status, and may retry only with its operation ID. Event clients reconnect with
bounded exponential backoff and never turn stale cached `connected` into a
fresh assertion.

## 9. Close, quit, disable, remove and stop

These actions remain distinct:

- **Close panel** — dismiss the popup only.
- **Close TUI/terminal** — close that client only.
- **Disconnect** — persist desired disconnected and stop/verify the owned core.
- **Quit OmaVLESS UI** — close the selected UI process/surface only; it is not
  disconnect, plugin disable or runtime stop.
- **Disable plugin** — explicit Omarchy integration removal. During legacy
  Stage 0 it keeps current cleanup behavior. After runtime cutover it sends the
  documented disable/removal transition, which disconnects as today's product
  does, then unloads the plugin; it does not uninstall the standalone package.
- **Remove plugin** — same tunnel-cleanup intent as disable, then remove only
  the Omarchy frontend and its generated legacy integration.
- **Stop runtime** — administrative action, not ordinary Quit. The semantic CLI
  refuses it while desired state is connected unless the user explicitly asks
  to disconnect first. Killing a systemd process directly is reconciled on
  restart and is not a supported disconnect mechanism.

Closing a panel/TUI can never be used as a shortcut for disable/remove. Once T1
exists, a future `Quit OmaVLESS` label means only the scoped UI close described
above. The requested healthy tunnel remains owned by the runtime.

## 10. Migration and rollback

Migration is staged so an upgrade never produces two owners.

### Stage 0 — current plugin backend

QML starts bounded `backend.py` commands; `backend.py` serializes operations,
writes the private store/config and manages the current Mihomo supervisor unit.
This remains the rollback baseline.

### Stage 1 — runtime introduced, legacy owner still active

The Arch package installs the runtime executable, CLI and unit while the
unchanged plugin continues to work. The unit is not enabled as canonical owner
yet. A one-shot bounded migration preflight validates the existing store,
service ownership and capabilities without opening the control socket, copying
state or touching the core. This preserves the rule that any process exposing
the canonical socket must first hold the lifetime owner lock.

Cutover is one explicit transaction while holding the shared ownership lock:

1. quiesce new legacy mutations;
2. inspect current service/controller/TUN ownership;
3. write the bounded desired-state document from consistent current state;
4. enable/start `omavless-runtime.service`; the daemon adopts a matching healthy
   owned core or records disconnected;
5. verify `system.hello` and `status.get` round-trip successfully;
6. enable the IPC compatibility bridge and mark runtime ownership.

An inconsistent runtime aborts without changing the owner.

### Stage 2 — plugin through IPC

The plugin's process bridge invokes the packaged semantic CLI/API. Direct
lifecycle/store commands are disabled after the ownership marker changes.
The plugin switches only when `system.hello` reports the required API version
and method capabilities; otherwise it shows a bounded update/recovery message.

For one compatibility window, the packaged CLI may translate stable legacy
plugin actions into v1 methods. It must not expose the daemon as another
`backend.py` subprocess dispatcher.

### Stage 3 — TUI client

The TUI is added only after the plugin works through IPC and restart/concurrent-
client acceptance passes. It uses the same methods and cannot start a second
runtime/core.

### Rollback and upgrades

- before cutover, removing the runtime package leaves Stage 0 unchanged;
- after cutover, rollback to legacy ownership is allowed only while desired
  and actual state are disconnected and the runtime releases its owner lock;
- an active tunnel must be explicitly disconnected before rollback;
- store migrations are atomic, mode `0600`, versioned and backed up according
  to the existing bounded policy; an older binary must refuse an unsupported
  newer schema rather than rewrite it;
- clients support negotiated API v1, not a moving daemon implementation
  version; package upgrades restart the daemon and trigger normal
  reconciliation/reconnect behavior.

## 11. Distribution and Omarchy integration

The Arch-first package name is `omavless`. A reviewed Arch/AUR package is the
first supported distribution path and owns at least:

```text
/usr/bin/omavless
/usr/lib/systemd/user/omavless-runtime.service
```

plus audited application resources. The package does not bundle private user
state. Mihomo remains a separately packaged dependency with explicit capability
setup. Future privileged networking support remains a separate security track.

The marketplace plugin never downloads, builds or curl-pipes the runtime/TUI,
never silently invokes a package manager and never adds sudo/pkexec behavior.
It detects `omavless` plus a successful compatible `system.hello`; absence or
version mismatch produces an installation/update hint.

On Omarchy the exact Open app shape is:

```text
omarchy launch or focus tui --app-id=org.omarchy.omavless omavless tui
```

The stable app ID is `org.omarchy.omavless`. Omarchy's launcher focuses the
existing matching terminal window or launches the TUI in the configured
terminal. The TUI attaches to the same daemon.

If the TUI already exists, no second terminal is created. If the daemon is
down, the client asks the user systemd manager to start the packaged runtime,
waits for `system.hello` under a bounded deadline, then attaches. Failure opens
a bounded doctor/remediation view. It never falls back to starting Mihomo
directly.

## 12. Security invariants

- same-user private socket plus peer credential verification;
- credentials accepted only by explicitly sensitive methods and never logged,
  broadcast in events or returned by ordinary reads;
- controller secret and raw core API remain behind the runtime;
- strict JSON/schema/size/depth/time bounds and fail-closed parsing;
- one mutation queue and one runtime ownership lock;
- no arbitrary shell, process, systemd, filesystem-write, controller-forward or
  privileged-command API;
- no sudo/pkexec path in ordinary runtime/client behavior;
- foreign VPNs/TUNs are detected conservatively and never killed;
- future K1 privilege/helper design remains separate from T1.

## 13. T1 acceptance plan

T1 is merge-eligible only when deterministic tests and an exact-head Try
Omarchy gate cover:

1. plugin and CLI concurrently reading the same revision;
2. plugin and a semantic test client issuing conflicting
   connect/mode/disconnect mutations at T1, then the real plugin and TUI
   concurrently at T2, with one serialized result and no lost update;
3. terminal/TUI close leaving desired connected, one runtime, one Mihomo and
   one TUN healthy;
4. daemon restart adopting or recovering one matching owned tunnel without a
   duplicate core/TUN;
5. daemon restart with inconsistent/foreign state blocking safely;
6. Quickshell restart and plugin close/reopen without tunnel disturbance;
7. plugin disable/remove applying its documented explicit cleanup and leaving
   no legacy generated unit/controller;
8. stale client instance/revision rejection and reconnect;
9. malformed, oversized, deeply nested and slow IPC clients rejected without
   daemon or lifecycle failure;
10. socket directory `0700`, socket `0600`, owner/peer checks and cross-user
    rejection;
11. no profile/subscription credential, controller secret or raw private error
    in status, events, journal, process argv/environment or shareable
    diagnostics;
12. package upgrade, pre-cutover removal and disconnected rollback;
13. exactly one canonical runtime, supervisor, Mihomo and TUN during all
    healthy connected cases;
14. bounded provider refresh/log/connection readers unable to stall an urgent
    disconnect.

The test harness must force connection loss during an in-flight mutation and
use operation IDs to prove that retry cannot create a second transition.

## 14. Deliberately open implementation choices

T1 may choose Python or a compiled implementation; the protocol does not
depend on that choice. The exact internal event-loop library, journal reader,
traffic ring representation and eventual core-unit rename remain implementation
decisions. The first package maintainer/repository also remains to be selected.

These choices do not block T1's first slice: singleton runtime, v1 hello/status/
capabilities, desired/actual reconciliation, serialized connect/disconnect/mode
and the plugin compatibility bridge are now specified independently of them.
