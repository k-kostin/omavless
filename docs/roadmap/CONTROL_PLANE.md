# OmaVLESS control-plane contract

Status: accepted T0 semantic contract, updated 2026-08-30 for the selected Rust
implementation path. This document defines the T1/R5 ownership boundary. The
v1 API semantics remain stable; implementation language is no longer an open
choice.

Implementation/migration authority:
[`RUST_MIGRATION.md`](RUST_MIGRATION.md).
Host/distribution authority: [`PLATFORM.md`](PLATFORM.md).

## 0. Existing T1a Python checkpoint becomes the R1 reference

`omavless_control_protocol.py` already implements a credential-free reference
for strict v1 frame/envelope mechanics:

- UTF-8 NDJSON decoding;
- duplicate-key rejection;
- byte/depth/string bounds;
- version/ID/revision/operation validation;
- stable success/error envelopes;
- one-frame stream helpers.

`tests/control_protocol_cases/` is now explicitly a **language-neutral contract
corpus**. `tools/control_protocol_probe.py` remains a developer reference probe,
not the future daemon.

R1 implements the same v1 mechanics in Rust and compares both implementations.
After R1 parity, Rust is canonical for new control-plane/runtime code; the
Python implementation is retained only as a temporary migration oracle until
its removal gate.

T1a still opens no socket, dispatches no lifecycle method and owns no runtime
state. Current `backend.py` remains the production plugin path until R5/T1.

## 1. Decision summary

T1 introduces one same-user, systemd-managed **Rust** headless runtime. It is
the only canonical owner of:

- desired and actual connection state;
- profile/subscription/store mutations;
- routing mode;
- owned Mihomo lifecycle;
- background work;
- operation serialization.

The runtime is distributed separately from the Omarchy marketplace plugin. One
primary `omavless` executable provides the daemon and semantic CLI surface:

```text
omavless daemon
omavless status
omavless connect ...
omavless disconnect
omavless doctor
```

The bar plugin, future Ratatui TUI and CLI are clients. They do not start a core,
edit canonical state behind the runtime or carry a second VPN state machine.

Mihomo remains the first production core and an external package/process. T1 is
not permission to add Xray, K1 privileges or unrelated protocol families.

## 2. Ownership and process model

### Runtime process

The packaged user unit is `omavless-runtime.service`. It executes the equivalent
of:

```text
omavless daemon
```

and acquires the lifetime owner lock before opening IPC or reconciling state.

During first cutover the legacy `omavless.service` may remain the private Mihomo
supervisor unit. Only the Rust runtime may prepare its config, start/restart/stop
or adopt it after ownership cutover. The old unit is an implementation detail,
not a second control plane.

The Rust runtime owns:

- desired/actual VPN state;
- active profile and requested mode;
- profile/subscription/favorite/custom-rule/startup mutations;
- generated core config and owned Mihomo service/controller/TUN;
- one serialized mutation queue;
- bounded background refresh/reconciliation/traffic sampling;
- capability discovery and redacted diagnostics;
- local operational events and bounded high-volume views.

It does not own foreign VPNs, arbitrary TUNs, shell commands, Omarchy UI
lifetime or future root networking policy.

### Client responsibilities

1. **Omarchy bar plugin** renders compact status/actions and later `Open app`.
   QML may own presentation focus/forms, never tunnel intent or core lifecycle.
2. **Ratatui TUI** is the full workspace after R6; terminal lifetime is client
   lifetime only.
3. **CLI** validates user-facing args, performs semantic requests and renders
   bounded human/JSON output. It is not a Mihomo/shell/systemctl passthrough.
4. **Runtime** validates every request again, owns state and performs work.

## 3. Socket and peer boundary

V1 endpoint:

```text
$XDG_RUNTIME_DIR/omavless/control.sock
```

Requirements:

- parent directory mode `0700`, current user owned;
- socket mode `0600`, current user owned;
- refuse symlinked/wrong-owner runtime directory/socket;
- Linux `SO_PEERCRED` same-UID verification;
- group/cross-user access unsupported.

Private transport does not authorize disclosure of private store contents.
Ordinary responses never contain profile URI, reusable protocol credential,
subscription URL, controller secret or generated private config.

Lifetime lock:

```text
$XDG_RUNTIME_DIR/omavless/owner.lock
```

Failure to acquire it exits without touching the core.

## 4. Framing and negotiation

V1 uses UTF-8 newline-delimited JSON. Unary clients send one request frame,
receive one response frame and close. Event watching uses a separate explicitly
requested stream.

Limits:

- request frame: maximum 64 KiB including newline;
- ordinary response/event frame: maximum 256 KiB;
- nesting depth: maximum 16;
- JSON string: maximum 32 KiB unless a method is stricter;
- unknown top-level fields and duplicate JSON keys rejected;
- invalid UTF-8, non-object frame, trailing data and missing newline rejected
  before dispatch;
- reads/writes/complete unary calls have bounded deadlines.

Normal first request:

```json
{"api":"omavless.control","version":1,"id":"hello-1","method":"system.hello","params":{"versions":[1]}}
```

Success envelope:

```json
{"api":"omavless.control","version":1,"id":"status-1","ok":true,"revision":42,"result":{}}
```

Error envelope:

```json
{"api":"omavless.control","version":1,"id":"connect-1","ok":false,"revision":42,"error":{"code":"conflict","message":"Another transition is active","retryable":true}}
```

Request IDs are opaque 1-64 character ASCII correlation values, not authority.
A new daemon `instanceId` invalidates client event cursors/cached freshness; the
client negotiates again and fetches current status.

R1 Rust parity must preserve these limits/envelopes before the Rust protocol
crate becomes canonical.

## 5. API categories and v1 methods

Method names are semantic and core-neutral. Exact response schemas remain
versioned and bounded.

### System and capabilities

- `system.hello` — negotiate version and return instance/revision/limits;
- `capabilities.get` — modes, adapters, diagnostics/background/UI capabilities;
- `status.get` — desired/actual state, active opaque profile ID, safe label,
  mode, core health, protection and transition summary;
- `events.watch` — bounded state events plus heartbeat.

### Profiles and imports

- `profiles.list`, `profiles.get` — safe metadata only;
- `imports.classify`, `profiles.import` — explicit bounded sensitive input;
- `profiles.rename`, `profiles.favorite`, `profiles.delete`;
- `profiles.test` — bounded latency/availability;
- `profiles.export` — explicit credential release for `qr`/`file` purpose only.

Credential-bearing import/export data never appears in argv, ordinary status,
logs or event broadcasts. V1 has no generic filesystem-write method.

### Subscriptions

- `subscriptions.list` — safe metadata/counts, no bearer URL;
- `subscriptions.preview`, `subscriptions.add`, `subscriptions.update` — bearer
  URL accepted only as bounded sensitive request data;
- `subscriptions.refresh`, `subscriptions.refresh_all`;
- `subscriptions.test`;
- `subscriptions.delete`.

Classification alone never fetches arbitrary remote content.

### Connection and routing

- `connection.connect` with opaque profile ID and optional mode;
- `connection.disconnect`;
- `routing.set_mode`;
- `routing.custom_rules.list/add/delete`;
- `routing.check`, `routing.refresh_providers`;
- `startup.get`, `startup.configure`.

There is no generic `run-core`, `systemctl`, controller-forward, process or shell
method.

### Diagnostics and traffic

- `diagnostics.summary`;
- `diagnostics.rules`, `diagnostics.providers`;
- `diagnostics.export` — returns redacted content, not arbitrary daemon file
  write;
- `traffic.summary`;
- `traffic.connections`, `logs.read` — explicit bounded/paged local views.

No unbounded log tail is part of v1. Any stream uses a separate connection,
bounded queue and drop/count marker so stalled clients cannot backpressure
lifecycle work.

### Host readiness

Rust migration adds a semantic host-readiness boundary without exposing package
manager commands:

- runtime/core installed/available state;
- TUN privilege readiness;
- runtime service provisioning readiness;
- bounded host/doctor remediation code/text.

Arch and NixOS may implement those checks differently; the API stays semantic.

## 6. Errors, revisions and mutation serialization

Stable machine codes are UI-localization keys. Messages are bounded English
fallbacks and may not include raw backend/core/controller/provider output.

Reserved categories include:

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

`manual_recovery_required` is always hard failure, never silently rendered as
ordinary disconnected.

Every committed state change increments daemon `revision`. Mutations may carry
`expectedRevision`; mismatch returns `conflict` before side effects.

All mutations enter one runtime-owned queue. At most one lifecycle/store commit
is active. Urgent disconnect may move ahead of queued work but does not interrupt
an unsafe atomic phase. Fetches/probes snapshot state, perform bounded I/O and
compare-and-commit afterward.

Retryable mutations may carry client `operationId` (1-64 ASCII). The daemon
keeps a bounded per-instance result cache without storing credential-bearing
request bodies. Same ID + same digest returns prior result; same ID + different
input returns `conflict`.

## 7. State classes

### Persistent private configuration

Retain `~/.config/omavless/` and current private atomic/mode-`0600` rules during
migration. T1/R5 must not create a second canonical store.

### Desired state

`$XDG_STATE_HOME/omavless/desired.json` (default
`~/.local/state/omavless/desired.json`) is mode `0600` below mode `0700` state
directory. It stores only intent needed across restart:

- connected/disconnected;
- opaque profile ID;
- requested mode;
- schema version/generation.

No URI, endpoint, password, key, subscription URL, controller secret, Arch
package-manager state or generation-specific Nix store executable path belongs
there.

### Actual state

`starting`, `connected`, `reconnecting`, `failed`, `stopping`, `disconnected`
lives in runtime and may be mirrored in bounded disposable runtime snapshot.
Socket disappearance never proves safe disconnection.

### Caches/high-volume data

- replaceable data below `$XDG_CACHE_HOME/omavless/`;
- bounded traffic/connection/event data in memory;
- credential-safe operational journal;
- explicitly generated redacted diagnostic exports;
- no raw log/high-volume telemetry appended to desired/profile state.

## 8. Restart and stale-client behavior

At start, Rust runtime acquires ownership, loads/migrates state, then inspects
owned service/process/controller/TUN before accepting mutations.

- desired connected + matching healthy core -> adopt, publish `connected`;
- desired connected + owned core absent -> `reconnecting`, one bounded recovery;
- desired disconnected + owned core running -> stop and verify cleanup;
- inconsistent/foreign ownership -> do not kill; block with remediation;
- multiple owned cores/TUNs -> `manual_recovery_required`, no arbitrary choice.

Clients use `instanceId` + revision to detect restart. In-flight connection loss
has unknown outcome; reconnect, read status and retry only with operation ID.

## 9. Close, quit, disable, remove and stop

- **Close panel** — UI only.
- **Close TUI/terminal** — client only.
- **Disconnect** — persist desired disconnected and stop/verify owned core.
- **Quit OmaVLESS UI** — close selected UI only.
- **Disable plugin** — explicit Omarchy frontend cleanup; after runtime cutover
  it performs documented disconnect/disable transition but does not uninstall
  standalone package.
- **Remove plugin** — same tunnel-cleanup intent, then remove Omarchy frontend
  and legacy generated integration only.
- **Stop runtime** — administrative, not normal Quit; semantic CLI refuses while
  desired state is connected unless disconnect is explicitly requested first.

Closing UI never substitutes for disconnect/disable/remove.

## 10. Python -> Rust migration and ownership cutover

The control-plane migration follows R0-R6; T1 is the R5 ownership cutover.

### Stage 0 — current Python plugin owner

QML starts bounded `backend.py` commands. Python owns store/config/service/Mihomo
lifecycle. This is current production and rollback reference.

### Stage 0R — Rust parity layers, Python still owns production

R0-R4 may add Rust crates/binary test surfaces while Python remains canonical
runtime owner. No Rust process may independently own the tunnel.

Each migrated subsystem gains language-neutral/differential evidence. New large
backend features should prefer the Rust layer once their owning stage exists.

### Stage 1 / R5 — Rust package introduced, legacy owner still active

The standalone package installs the Rust `omavless` executable, CLI and runtime
unit. Runtime ownership is not switched merely because the package exists.

A bounded preflight validates:

- current store/schema;
- service/controller/TUN ownership;
- core/host readiness;
- control protocol compatibility;
- no stale generation-specific executable path.

Preflight opens no competing canonical runtime if it cannot acquire ownership.

### Stage 2 — explicit Rust cutover transaction

While holding the shared migration ownership lock:

1. quiesce new legacy mutations;
2. inspect current service/controller/TUN ownership;
3. write bounded desired state from consistent current state;
4. start/enable `omavless-runtime.service`;
5. Rust runtime adopts a matching healthy owned core or records disconnected;
6. verify `system.hello` and `status.get`;
7. switch plugin compatibility bridge to semantic Rust CLI/socket;
8. mark Rust runtime ownership.

Any inconsistent state aborts without creating two owners.

### Stage 3 — plugin through Rust IPC

Direct Python lifecycle/store mutation is disabled after ownership marker
changes. The plugin uses packaged semantic CLI/API and requires compatible
hello/capabilities.

For one compatibility window Python may remain only as disconnected rollback or
parity oracle. It is not a fallback that can silently start a second tunnel
while Rust owns state.

### Stage 4 / R6 — Python runtime retirement

Before TUI implementation:

- every plugin-required production backend operation is Rust-owned;
- normal operation succeeds with Python unavailable;
- `backend.py` is not spawned by normal path;
- no venv/pip/Python package setup exists;
- regression corpora survive Python oracle removal;
- rollback/recovery is validated before legacy owner code deletion.

### Stage 5 / T2 — Ratatui client

Only after R6 does the production TUI attach as another client of the existing
Rust runtime. It cannot start another owner/core.

## 11. Distribution and host integration

The primary application package is `omavless` and exposes one native Rust
program. Host mechanics remain outside semantic API.

### Arch

Package owns stable executable/runtime unit; Mihomo stays separately packaged.
Reviewed file-capability setup may remain an Arch host implementation.

### NixOS

Package/module/wrapper must provide stable entry points and generation-safe
service/privilege behavior. Do not persist resolved `/nix/store/...` paths as
durable desired state and do not copy Arch `setcap` workflow onto immutable
store binaries.

### Omarchy

Marketplace plugin never downloads/builds/curl-pipes the runtime, invokes a
package manager silently or introduces sudo/pkexec. It detects the installed
compatible runtime and later exposes `Open app`:

```text
omarchy launch or focus tui --app-id=org.omarchy.omavless omavless tui
```

If launcher spelling changes in future Omarchy, only frontend integration
changes; control/runtime semantics remain stable.

If runtime is down, clients may ask user systemd manager to start the packaged
runtime under bounded deadline. They never start Mihomo directly as fallback.

## 12. Security invariants

- same-user private socket + peer credential verification;
- credentials only in explicitly sensitive methods, never ordinary status/event
  or logs;
- core controller secret remains behind runtime;
- strict JSON/schema/size/depth/time bounds;
- one mutation queue + one lifetime owner lock;
- no arbitrary shell/process/systemd/filesystem/controller-forward/privileged
  API;
- no sudo/pkexec normal runtime/client path;
- foreign VPNs/TUNs detected, never killed;
- R-stage parity tooling obeys credential privacy rules;
- K1 privileged design remains separate.

## 13. T1/R5 acceptance plan

T1/R5 is merge-eligible only when deterministic Rust/Python migration evidence
and exact-head host integration cover at least:

1. R1 protocol corpus parity and stable limits/errors;
2. accepted R2-R4 domain/OS behavior required for cutover;
3. plugin + CLI reading same revision;
4. conflicting mutation serialization with no lost update;
5. UI/client close leaving desired connected and one runtime/core/TUN healthy;
6. runtime restart adopting/recovering matching owned tunnel without duplicate;
7. inconsistent/foreign restart state blocking safely;
8. Quickshell restart/panel reopen without tunnel disturbance;
9. plugin disable/remove cleanup with no legacy owner left;
10. stale instance/revision rejection and reconnect;
11. malformed/oversized/deep/slow IPC rejection without lifecycle failure;
12. socket/owner/peer permission checks;
13. no credential/controller secret/private raw error in status/events/journal/
    argv/environment/shareable diagnostics;
14. package upgrade, pre-cutover removal and disconnected rollback;
15. exactly one canonical runtime/supervisor/core/TUN in healthy cases;
16. bounded readers/fetches unable to stall urgent disconnect;
17. connection loss during in-flight mutation + operation-ID retry without a
    second transition;
18. production plugin bridge proven against Rust owner before Python fallback is
    disabled.

Arch and NixOS packaging/generation gates remain separate host evidence.

## 14. Deliberately open implementation choices

Rust is selected; the following remain implementation details until a concrete
slice needs them:

- exact crate decomposition;
- specific async helper libraries beyond the need for bounded async I/O;
- internal traffic ring/event representation;
- journal reader implementation;
- eventual core-supervisor unit rename;
- exact Arch/Nix package repository/maintainer path.

These choices do not reopen Python versus Rust. The accepted target remains one
Rust runtime/control plane and, after R6, a Rust Ratatui client.
