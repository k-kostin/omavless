# DNS core/broker descriptor channel — uninstalled

This is an executable Unix `SOCK_SEQPACKET` transport boundary, not a privileged
broker or installed DNS fix. Production has no dependent on this crate. There
are no DNS, TUN-creation, route, service, package or policy effects. Tests use
temporary private listeners and socketpairs with ordinary temporary-file FDs.
A received FD is **not** accepted as a TUN: separate trusted kernel admission,
enrollment, exclusive ownership and typed resolved integration remain required.

## Small wire amendment for the core-facing descriptor transfer

The previous `omavless.dns` NDJSON document remains an unpublished semantic
broker draft. It cannot itself associate an exact ancillary descriptor with
one byte-stream request. This separate, still unpublished **core-facing**
channel uses one eight-byte packet and ancillary FD transfer instead. It does
not register another method on the normal runtime socket or claim compatibility
with the prepared NDJSON parser. There is no generic JSON/shell interface.

The shared credential-free [corpus](cases.json) is canonical for Rust/Go tests:

| Offset | Meaning |
| --- | --- |
| 0–3 | ASCII `OVDN` |
| 4 | version `1` |
| 5 | kind `1` request / `2` response |
| 6 | fixed code below |
| 7 | reserved, exactly zero |

Requests: `1 Acquire`, carrying exactly one proof FD; `2 Release`, no FD.
Responses: `1 Applying`, `2 Ready`, `3 Releasing`, `4 Released`, `5 Rejected`,
`6 RecoveryRequired`, always no FD. No names, indices, DNS values, paths, UIDs,
commands, lease tokens or reusable credentials appear in frames.

Normal sequence:

```text
Acquire + FD -> Applying -> Ready -> Release -> Releasing -> Released
```

One accepted connection binds at most one lease; there is no reacquisition,
retry token or cross-connection adoption. Applying/Ready and Releasing/Released
are distinct: the trusted handler must finish admission/apply/readback or cleanup
before sending the corresponding completion. Rejected is allowed only during
acquisition; RecoveryRequired may terminate an active phase. Duplicate progress,
unsolicited completion, extra/trailing bytes, wrong version/kind/reserved byte,
unknown codes and unexpected FD counts are errors, not ignored extension data.

EOF, I/O failure or timeout **never means success or cleaned-up DNS**. `Session`
retains the proof FD even after response failure or terminal status. The handler
must retain/reconcile that session while effects are in flight or unknown;
dropping the object closes descriptors but performs no DNS cleanup. This layer
cannot prevent a defective handler from lying about readiness or dropping state.
When an already Ready session is simply waiting for a new Release request,
the bounded wait returns `Idle` without changing lease state; it can be polled
again. This is distinct from timing out an in-flight acquire/release reply.
A ten-second I/O budget is not a ten-second VPN lifetime.

## Credentials, descriptors and bounds

- Public client connection authenticates a root peer through `SO_PEERCRED`
  before sending an FD. Server admission checks the configured enrolled UID.
  Path/UID arguments are trusted local provisioning inputs, never packet fields.
- Every endpoint also enables `SO_PASSCRED` and requires one kernel-generated
  `SCM_CREDENTIALS` matching the admitted PID/UID/GID per packet. Senders do not
  manually construct credentials. A different process using a delegated channel
  FD therefore cannot act as the original admitted peer. This is not proof of
  application identity, a particular executable, or uncompromised enrolled user.
- Safe rustix `OwnedFd` reception uses `MSG_CMSG_CLOEXEC`. All delivered FDs,
  including surplus or truncated rights, close on error. The proof's original
  sender retains its own descriptor; the receiver owns only its duplicate.
- Data capacity is exactly eight bytes. Ancillary capacity is exactly the space
  for credentials plus one FD on Acquire, credentials only otherwise. It cannot
  hold an additional unknown ancillary header while retaining required records:
  truncation or a missing required record refuses the packet. Timestamp-injected
  regression exercises this despite rustix skipping unknown ancillary types.
  Normal sockets are created internally, with no public raw-socket adoption or
  access allowing ancillary-generation options to change unnoticed.
- The implementation **fully exhausts** every ancillary drain before an error
  return. Testing found rustix 1.1.5's partial-drain bookkeeping can restart on
  an unaligned header when credentials precede rights; the full-drain pattern
  avoids that path and has regression coverage. No unsafe conversion or local
  dependency fork is used.
- Nonblocking send/receive and monotonic `poll` loops share a ten-second budget
  per operation, including EINTR/retry. Progress is allowed once per phase, so
  a peer cannot keep extending one phase by repeating progress packets. Listener
  backlog is four; accept is bounded. The future service must separately cap
  admitted sessions/worker count and in-flight DNS transaction lifetime.
- Only fixed English errors leave this crate; no private descriptor target,
  path, credential, UID/PID or remote payload is formatted in errors.

The listener refuses an unsafe/non-owned/writable parent or existing socket path,
binds without unlinking an incumbent, sets mode `0600`, and removes only its own
recorded socket inode on drop. Parent ancestry must resolve canonically. A real
root-owned installation must explicitly provision narrow enrolled-user access
to that socket while keeping its parent protected; this crate does not invent
group membership, ACL or account-wide passwordless policy.

## Validation and limits

`cargo test --locked -p omavless-dns-channel` exercises real Linux ancillary
messages and peer credentials, not injected transport booleans. Leak tests count
only a unique synthetic fixture's `/proc/self/fd` targets, including extra and
truncated rights. No live TUN or system bus is used by this suite.

The dependency is existing locked rustix 1.1.5. nix 0.30.1 is test-only for the
timestamp negative case; its raw-FD `recvmsg` API is deliberately not used because
adopting raw rights as `OwnedFd` would require locally forbidden unsafe code.
Workspace `unsafe_code = forbid` remains unchanged for this crate.

### Cross-language wire fixture

`cargo build --locked -p omavless-dns-channel --example channel_fixture` builds
an explicitly test-only peer for the reviewed Go core adapter. It accepts only
four fixed synthetic scenarios on a private temporary socket: successful
acquire/release, acquisition refusal, recovery-required release and channel loss
after Ready. The received proof must be a regular file with the exact synthetic
marker, **not** a real TUN. It neither calls resolved nor performs cleanup.

The opt-in Go `TestSystemDNSRustChannelInterop` reads this crate's `cases.json`
and drives the real Rust listener with `SCM_RIGHTS` and kernel credentials. Set
`OMAVLESS_DNS_INTEROP_SERVER` to the built example's absolute path and
`OMAVLESS_DNS_INTEROP_CORPUS` to this crate's corpus, then run that single test
in the reviewed core source. Both test peers run as the current ordinary user;
the adapter's production root-peer requirement is unchanged. Progress/completion
ordering and failure outcomes are exercised across languages rather than only
by two independent mock implementations. This is not host integration evidence.

The separate `kernel_channel` example composes the same channel with
`omavless-dns-tun::HeldTun` through a **dev-only** dependency. It refuses to run
unless user/network/PID/mount namespace identities differ from the parent's
supplied identities and the process is namespace-root. The outer reviewed probe
must create those disposable namespaces and a private `/run` tmpfs before
launching it. Its fixed socket is `/run/omavless-dns/control.sock`; no arbitrary
socket or DNS target is accepted. It admits the actual received single-queue
`Meta` descriptor, retains it after peer loss, and rechecks it before transitions.

This fixture's Ready/Released messages are **synthetic wire outcomes**, never
evidence that DNS was configured or restored. A bounded stdin control stream
allows the outer probe to prove that the real core waits before Ready and stops
on refusal/loss. Explicit final descriptor drop lets the probe check kernel TUN
lifetime separately. The fixture is neither installed nor a privileged helper;
it does not connect to any D-Bus, invoke networking commands, or perform effects
on the admitted link.

Reviewed implementation references:

- [Linux v7.2 ancillary credential/rights validation](https://github.com/torvalds/linux/blob/v7.2/net/core/scm.c)
- [rustix 1.1.5 ancillary buffer/OwnedFd implementation](https://docs.rs/rustix/1.1.5/src/rustix/net/send_recv/msg.rs.html)
- [Unix socket credentials and descriptor semantics](https://man7.org/linux/man-pages/man7/unix.7.html)

Before installation: complete kernel proof admission and secure lease lifetime,
core integration, fixed resolved transaction/readback/recovery, enrollment and
socket ACL/package design, concurrency/replay/crash/removal behavior and attended
prompt-free host acceptance. This channel alone does not close #270.
