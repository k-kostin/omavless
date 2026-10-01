# S1 admission chain: evidence and unclosed trust edges

Development design, 2026-09-29. App proxy remains unavailable. This document and
the synthetic-only peer-lifetime contract do not enable a host adapter or mint a
write permit. See [foundation](S1_PROXY_FOUNDATION.md),
[AUTH sender](S1_AUTH_SENDER_PROBE.md) and
[manager continuity](S1_MANAGER_CONTINUITY_PROBE.md).

## Explicit threat boundary before implementation

A useful unprivileged adapter is possible only with stated trust assumptions:
the kernel, relevant namespaces, root-controlled installation and the actual
desktop/systemd/broker processes must be trusted. Same-UID names, executable
paths and two matching snapshots cannot demonstrate that these processes have
not been injected into or compromised. Do not silently change the policy to
claim protection from arbitrary malicious same-user execution. Conversely, do
not call ordinary concurrent desktop edits malicious: they must still receive
the existing conflict/recovery treatment.

The user-manager environment is per-user, not necessarily per-graphical-session.
Multiple graphical sessions require explicit supported-scope policy; selecting
one display does not make these writes session-local. No new privileged helper,
shell command, environment import or arbitrary IPC is justified by this design.

## Proof graph, not a bag of matching identifiers

| Edge | Required evidence | Current status |
| --- | --- | --- |
| Fixed system endpoint → system manager | Root-controlled endpoint plus actual authenticated systemd owner and stable lifetime in intended namespaces | #364 pins endpoint and reads MainPID; owner/writer proof remains open |
| System manager → real user manager | Fixed user@UID.service identity, kernel process lifetime and matching private peer; no activation | Scalar/peer/start-time observations plus a retained endpoint-creator pidfd in the opt-in diagnostic; still no trusted writer or production handle binding |
| Local user bus → actual broker writer | Bounded AUTH with kernel sender credentials and lifetime captured with the response | #357 obtains SCM_CREDENTIALS; numeric PID-to-lifetime race remains |
| Broker → launcher → same manager | Independently trusted broker/launcher lifetimes and source-compatible regular connection on this bus, with manager owner matching the anchored manager | Source design only; self-reported bus credentials cannot bootstrap trust |
| User manager → intended desktop scope | Root-anchored supported login/session and reviewed UWSM launch relationship; multi-session refusal or explicit consent policy | Open |
| Desktop snapshot → actual GSettings target | Fixed schemas/key types, supported backend/profile and complete layered values; no redirected target substituted between read/write | Typed read-only observer exists; effect-time target continuity remains open |
| Bound observations → one effect | Fresh complete reads, durable per-field intent, owner/revision checks and typed fixed operation followed by readback | Foundation only; real partial-write/recovery adapter not implemented |

Pinned dbus-broker
[`launcher.c`](https://github.com/bus1/dbus-broker/blob/2956b5d381deeea709c53d02f10e799e50e44f4b/src/launch/launcher.c)
uses its regular connection for activation-environment forwarding. Its
[`service.c`](https://github.com/bus1/dbus-broker/blob/2956b5d381deeea709c53d02f10e799e50e44f4b/src/launch/service.c)
uses that connection for unit/transient activation. Once the processes and bus
are independently trusted, identifying the launcher's unique connection can
potentially avoid inspecting its private environment. This is version-specific
evidence, not proof from `--scope user`, a process name or matching settings.

## Narrow primitive: socket-peer lifetime, private to the opt-in diagnostic

Linux
[`SO_PEERPIDFD`](https://github.com/torvalds/linux/blob/72d3fcf802c45d00b300f25b848a93c3a2bd7c7e/net/core/sock.c)
creates a descriptor from the socket's stored peer process reference. This
avoids reopening a numeric PID and accidentally binding a replacement process.
The safe pinned `nix` dependency already exposes an owned descriptor API. The
private module requires a connected Unix endpoint and close-on-exec
descriptor, then uses nonblocking poll to reject observed exit/error. Unknown
or unsupported kernel behavior refuses; there is no procfs/PID fallback.

The descriptor is **not** a lock on a live process. Linux
[`pidfd_poll`](https://github.com/torvalds/linux/blob/72d3fcf802c45d00b300f25b848a93c3a2bd7c7e/fs/pidfs.c)
reports process exit; a non-ready result is only a momentary observation, not
proof of execution, health or future life. It does not solve final-check-to-use
races, executable trust, session identity or cross-service atomicity.

Most importantly, a socket-activated listener can retain its creator identity.
The inherited-listener test lets a child write a response and exit while the
captured peer pidfd still observes the living parent. Therefore this primitive
must never be relabeled an AUTH-writer pin. A separate test has a child create
its own listener: closing the client socket does not kill the process handle,
but child exit is detected through the retained descriptor without PID reopen.
The tests also reject non-sockets and unconnected/listening endpoints; an early
fixture exposed that a listener can itself yield a pidfd, hence the explicit
connected-endpoint check is necessary.

The manager diagnostic now uses this same private implementation, replacing its
duplicate pidfd wrapper. The process-exit and inherited-listener regressions
therefore exercise the actual diagnostic primitive. Without the optional
`manager-continuity-probe` feature it is compiled only for
`peer-lifetime-tests` test builds. It is never exported to consumers, and the
normal runtime does not link it. It has no PID accessor, signal operation or
conversion into authority.

An additional child-isolated regression checks that 64 capture/drop cycles
release every handle, then temporarily sets that child's soft descriptor limit
to zero. Capture must refuse even with a live connected same-user peer; it must
not fall back to numeric PID observations. Restoring the child's limit allows a
fresh capture. The hard limit is unchanged. This tests descriptor exhaustion,
not a kernel lacking `SO_PEERPIDFD`, and does not grant write authority.

These fixtures use private temporary paths and owned child processes; they do
not touch installed bus/socket, settings, service or VM state. Fixture cleanup
only terminates and reaps its own spawned child if normal bounded exit fails.

```sh
cargo test --locked -p omavless-s1-observer --features peer-lifetime-tests
cargo test --locked -p omavless-s1-observer --all-features
cargo clippy --locked -p omavless-s1-observer --all-features --all-targets -- -D warnings
```

## Next boundary and acceptance gates

For actual AUTH-writer lifetime, the kernel's
[`SCM_PIDFD` path](https://github.com/torvalds/linux/blob/72d3fcf802c45d00b300f25b848a93c3a2bd7c7e/net/core/scm.c)
attaches a handle to the message sender when requested through `SO_PASSPIDFD`.
The current safe `nix` ancillary API does not expose this message variant; GIO's
current typed credential/FD messages must not be assumed to own unknown kernel
descriptor types safely. Do not enable that option through a raw integer and
silently leak/discard the returned descriptor. A separately reviewed safe
dependency/API and truncation/duplicate/unknown-descriptor cleanup tests are
required before using it. Do not introduce unsafe code around the repository's
`unsafe_code = forbid` boundary as a shortcut.

Then test: relay (pin relay, not upstream), inherited listener, process exit
between AUTH fragments and before effects, descriptor exhaustion/closure,
unsupported kernel, namespace mismatch, manager-owner/launcher/client turnover,
and multiple-session scope. Pinning a process does not validate what it does.

Even a completed identity graph would not make GSettings plus systemd atomic
or compare-and-swap. Per-field write intent/readback, exact absent restoration,
unknown write outcome, foreign edits, crashes, lost journal, owned listener
readiness and new-application consumption remain mandatory separate gates.
Approval must follow the current acceptance policy; synthetic tests alone
cannot authorize main feature exposure or host proxy changes.
