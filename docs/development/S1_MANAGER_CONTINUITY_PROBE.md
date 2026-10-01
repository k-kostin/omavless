# S1 root/user-manager continuity diagnostic

Development checkpoint, 2026-09-29. Separate opt-in diagnostic stacked on the
[AUTH sender probe](S1_AUTH_SENDER_PROBE.md); no production adapter or write
authority. `Provenance::Unverified` and unconditional write refusal are unchanged.

## What it observes

The observer connects only to the pinned `/run/dbus/system_bus_socket`, authenticates
through the existing EXTERNAL-only GIO helper, and reads only `MainPID` of the
current user's fixed `user@UID.service` object. The two `Properties.Get` calls use
`NO_AUTO_START`, no interactive authorization, fixed interface/key, typed scalar
validation and 1.5-second call timeouts. The handshake has its existing three-second
cancellation timer; each socket connect has its existing two-second timeout.
There are no environment/settings reads, activation calls, shell commands or
service writes. GIO's normal system-bus Hello is necessary to issue the reads.

It pins root-controlled system socket ancestors and the user runtime/private
socket chain using `O_PATH`/`O_NOFOLLOW` and expected ownership. Group/other-writable
directories, symlinks, non-sockets and changed identities refuse. An ambient
system-bus address other than the exact fixed address refuses rather than being
used. `G_DBUS_DEBUG` also refuses before GIO can emit wire identifiers. The user
bus address is not consumed. These are local process-control environment checks,
not reads of manager/activation environment or private proxy settings.

It connects to the pinned `/run/user/UID/systemd/private` socket, compares its
kernel peer with the scalar MainPID and current UID, and keeps that socket open
without sending AUTH, Hello or methods to it. It also retains a kernel
`SO_PEERPIDFD` handle to that connected endpoint, requires close-on-exec, and
polls it nonblocking before and after the scalar/endpoint rechecks. An
unsupported kernel or unavailable handle refuses; there is no numeric-PID
fallback. It rechecks MainPID, bounded procfs start time and canonical endpoint
identities. It closes the system socket directly instead of performing an
unbounded synchronous flush/close operation.

The successful output is only `manager_continuity=observed_unverified`; refusal
prints `manager_continuity=refused` and exits nonzero. Arguments are rejected.
No PID, UID, path metadata, environment, raw errors or settings are serialized.
The package and normal runtime do not install or invoke this binary.

## Why this does not prove provenance

Pinned upstream systemd
[`user@.service.in`](https://github.com/systemd/systemd/blob/583679fe4924e0afbf4dffa861f8b7c6b7be87eb/units/user%40.service.in)
launches the per-user manager. Its
[`private bus implementation`](https://github.com/systemd/systemd/blob/583679fe4924e0afbf4dffa861f8b7c6b7be87eb/src/core/dbus.c)
owns the corresponding private listener. Its
[`peer check`](https://github.com/systemd/systemd/blob/583679fe4924e0afbf4dffa861f8b7c6b7be87eb/src/shared/bus-util.c)
accepts root/same-euid peers, not an independently proven manager identity.

Root-controlled endpoints prevent ordinary same-user pathname replacement, under
the assumption of trusted kernel, mount namespace and root-controlled system
services. This diagnostic does **not** authenticate the actual system-bus AUTH
writer or systemd name-owner credentials, prove an unmodified executable, or
bind namespaces/session/UWSM. The retained pidfd narrows numeric PID-reuse
races, but its poll is only a point-in-time liveness check. `SO_PEERCRED` and
`SO_PEERPIDFD` may identify an inherited listener creator, not the process
serving its accepted connections. A successful match is therefore not
an application-security boundary and cannot authorize writes. It describes only
observations made during this invocation; it does not eliminate a final-check-to-use
race or promise that the process is still alive after the function returns.

The broker/launcher connection and activation-environment relationship remain
separate. The pinned dbus-broker
[`launcher`](https://github.com/bus1/dbus-broker/blob/2956b5d381deeea709c53d02f10e799e50e44f4b/src/launch/launcher.c)
has a regular connection used for environment forwarding, while
[`service activation`](https://github.com/bus1/dbus-broker/blob/2956b5d381deeea709c53d02f10e799e50e44f4b/src/launch/service.c)
uses it for activation. A future source-compatible, independently trusted
launcher/broker chain might identify that connection without reading launcher
environment; the present probe does not inspect it.

## Gates and use

```sh
cargo test --locked -p omavless-s1-observer --features manager-continuity-probe
cargo clippy --locked -p omavless-s1-observer --features manager-continuity-probe \
  --all-targets -- -D warnings
cargo build --locked -p omavless-s1-observer --features manager-continuity-probe \
  --bin omavless-s1-manager-continuity-probe
```

Synthetic tests cover exact scalar type/size, missing/zero/overflow values,
unavailable queries, foreign/counterfeit peer identity, observed PID/start-time
changes, endpoint replacement, wrong owner, symlink and unsafe directory refusal,
and a real pinned socket connection. They do not simulate full system-bus
authentication or assert trusted namespace/inherited-worker/session provenance.
The diagnostic and lifetime fixtures now share one private pidfd implementation,
so process-exit, inherited-listener, capture/drop cleanup and child-only
descriptor-exhaustion checks exercise the diagnostic's actual primitive.
The `nix/resource` feature is enabled only as a test dependency for the isolated
descriptor-limit fixture; it introduces no production resource-limit operation.
The inherited-listener/relay counterexamples remain covered by the separate AUTH
probe and must not be mistaken for a positive proof here.

An exact-head VM smoke may run only this read-only binary with sanitized output
and remove its own temporary executable afterward. Record source/binary hashes.
Missing endpoints or MainPID are unsupported, never reasons to start services.
The [x86_64 Omarchy Dev VM smoke](../testing/S1_MANAGER_VM_2026-09-30.md)
records one positive observation and conservative negative controls without
upgrading `Provenance::Unverified` or write refusal.

Before a writable S1 adapter, still required: reviewed trusted-process threat
boundary; authenticated system-bus and manager-owner identities; PID lifetime and
namespace binding; broker/launcher activation transport proof; supported session
selection (including multiple graphical sessions); fresh effect-time continuity;
and the separate recovery, foreign-writer, listener and new-app-consumption gates.

The [admission-chain design](S1_ADMISSION_CHAIN.md) maps these remaining edges
and includes the inherited-listener counterexample. This diagnostic now uses
that narrow kernel-lifetime primitive for its private manager endpoint; it does
not promote the endpoint's observed creator into a trusted writer or upgrade
the observer's provenance.
