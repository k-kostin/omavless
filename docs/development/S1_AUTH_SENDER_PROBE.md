# S1 authentication sender observation

Development checkpoint, 2026-09-29. This is a separate opt-in read-only probe,
not a production observer, App proxy adapter or permission to write settings.
The existing GIO observer, private observation frame, `Provenance::Unverified`
and unconditional `admit_writes` refusal are unchanged.

## Question and upstream findings

The user-bus socket's `SO_PEERCRED` can identify the process that created a
socket-activated listener rather than the broker writing the response. The
broker's own D-Bus credentials are not a substitute: upstream
[broker construction](https://github.com/bus1/dbus-broker/blob/2956b5d381deeea709c53d02f10e799e50e44f4b/src/broker/broker.c)
sets its advertised bus PID from the controller socket's peer, and the
[driver](https://github.com/bus1/dbus-broker/blob/2956b5d381deeea709c53d02f10e799e50e44f4b/src/bus/driver.c)
returns that identity for `org.freedesktop.DBus`.

Linux's
[`unix_maybe_add_creds`](https://github.com/torvalds/linux/blob/72d3fcf802c45d00b300f25b848a93c3a2bd7c7e/net/unix/af_unix.c)
attaches sending-task credentials when the receiver enables `SO_PASSCRED`.
This gives a useful distinct observation of who wrote the AUTH response. A
forwarding process reports its own kernel identity, not its upstream's.

There is still a separate activation-provenance gap. The reviewed
[launcher](https://github.com/bus1/dbus-broker/blob/2956b5d381deeea709c53d02f10e799e50e44f4b/src/launch/launcher.c)
delegates activation to systemd and forwards activation-environment changes to
`SetEnvironment`. However, its user scope calls `sd_bus_open_user`, whose
[address selection](https://github.com/systemd/systemd/blob/583679fe4924e0afbf4dffa861f8b7c6b7be87eb/src/libsystemd/sd-bus/sd-bus.c)
honors the launcher's own `DBUS_SESSION_BUS_ADDRESS`. Checking only the
observer's environment or a `--scope user` argument does not prove that the
launcher uses the manager observed by OmaVLESS.

## Fixed probe behavior

Build explicitly with:

```sh
cargo build --locked -p omavless-s1-observer --features auth-sender-probe \
  --bin omavless-s1-auth-sender-probe
```

The executable accepts no arguments. It reuses the fixed local-bus path,
directory/socket ownership, no-symlink and ambient-redirection checks. It
connects to the pinned endpoint, enables `SO_PASSCRED`, and sends only a
bounded `AUTH EXTERNAL` request. The response is read through GIO's native
`recvmsg` wrapper with a five-second total deadline (including the existing
two-second socket connect bound), a 512-byte maximum and exact `OK` frame
grammar. Missing/extra credentials, foreign UID, zero PID, changed sender,
truncation, descriptor transfer, invalid frames, timeout and EOF refuse.
Received descriptor messages are owned and released by GIO; the regression
test verifies release through pipe EOF.

After AUTH, the probe rechecks the pinned endpoint/directory and observes the
sender's procfs start time twice. Those reads detect observed changes only;
they are not pidfd authority, executable authentication, protection from a
malicious same-user process or proof of a trusted login. It closes the socket
without sending `BEGIN`, `Hello` or any D-Bus method. It never reads proxy
settings, manager environment, profiles or service state.

The only successful output is `auth_sender=observed_unverified` plus a Boolean
`listener_matches_sender`. Refusal prints only `auth_sender=refused` and exits
nonzero. PID, UID, executable names, server GUID, AUTH bytes and internal errors
are not output. The package and ordinary runtime do not install or invoke it.

## Validation and next boundary

Synthetic tests cover fragmented/malformed/oversized AUTH, changed/foreign/
missing credentials, ancillary truncation and received-descriptor cleanup,
silent and dripping peers under the total deadline, a real inherited
socket-activation listener, and a forwarding process. The sender observation
identifies the child writer rather than the parent listener/upstream. A helper
child test is inert unless launched by the fixture; all fixture values are
synthetic. Run the optional package tests and strict clippy in addition to the
ordinary repository gates:

```sh
cargo test --locked -p omavless-s1-observer --features auth-sender-probe
cargo clippy --locked -p omavless-s1-observer --features auth-sender-probe \
  --all-targets -- -D warnings
```

An installed VM check must identify exact source and binary hashes and record
only the fixed output above. A successful result does not authorize writes.

Before any writable S1 adapter, establish a reviewed chain to the actual
systemd user manager, the broker/launcher and its activation scope; bind it to
the intended session and revalidate throughout the operation. Root-owned
executable names, same-UID credentials, unit names or matching snapshots alone
are insufficient. A same-user malicious-process threat model cannot be solved
by an unprivileged metadata observer alone. Exact per-field recovery, foreign
writer conflicts, listener readiness and new-application consumption remain
separate gates.

A future GIO integration may use a separately reviewed bounded AUTH exchange
followed by `BEGIN`, then GDBus on the already-authenticated stream. That would
replace GIO's authentication path and requires its own compatibility tests;
this probe intentionally stops before that integration.

A separate [manager continuity diagnostic](S1_MANAGER_CONTINUITY_PROBE.md)
compares the system-manager scalar MainPID observation with the fixed user
private socket's kernel peer. It also remains unverified and does not close
the broker, process-trust or activation-environment proof gaps.
