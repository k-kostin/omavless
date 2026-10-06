# T4 fixed system-event source adapter

Status: dormant source/build checkpoint on the independently selected #697
baseline `eb42ab9a45f027143ae48aa32092d547e7bb633c`. No normal daemon factory,
owner, receipt or lifecycle path activates this adapter. No host bus, host
netlink, service, network setting or VM has been contacted. New fixtures remain
ignored until complete primary and independent boundary review permits execution.
Earlier #697 evidence belongs to that head, not this new transport boundary.
The first source checkpoint `e73b4177` is execution HOLD from review, not a
runtime failure or acceptance result. Its successor corrects fixture supervision,
final original-transport sampling and exact typed-body/unique-name validation.
The `e8518f3a` correction is also source HOLD until unknown teardown is reported
as failure by the selected test itself, not only by later fixture acquisition.

## Boundary and interfaces

The crate-private `host_event_source::HostEventSource` owns its original logind
connection, unfiltered incoming stream, framing monitor, connected peek handle,
and (only in the dormant system constructor) fixed rtnetlink socket. The
disabled-by-default `system-event-source` feature compiles the boundary; it does
not register a worker, timer, method or source with the daemon. Dependencies are
the existing pinned zbus 5.19.0, async-io 2.6.0, futures-lite 2.6.1 and async-trait
0.1.92. Nix's existing safe socket APIs supply net/uio; unsafe code remains forbidden.

The original mutable adapter exposes bounded `poll`, `readable`, `next` and
`quiescent`, with categorical `Lost` and typed `Suspend`, `Resume`,
`NetworkChanged` emissions. Receiver-assigned sequence advances only on emitted
hints. Raw bus replies/irrelevant messages and netlink header pid/sequence do not
cause hint gaps or establish an epoch. `SourceContinuity` is a non-cloneable
borrow of that exact original adapter; recheck consults original transports and
terminal state, not a stored atomic healthy ticket or copied safe boolean.

Quiescence is momentary, not atomic exclusion of a hint arriving after its final
check. Effects need the integration owner's serialization/lease and the existing
desired/store/ownership/receipt/safety fences. An empty Unix forwarding pipe
cannot attest upstream logind or kernel provenance; no bridge is provided here.
This lane does not edit coordinator, receipt or lifecycle code or mint Ready,
an admission capability, a binding proof or any reconnect permission.

Observed suspend is retained separately from transport emptiness. Initial true
queues a local observed Suspend marker and remains Suspended even after that
marker is consumed. Initial false establishes observed Awake but never fabricates
Resume. Only authenticated subsequent false signals change that state to Awake.
Quiescence returns Pending or Suspended separately from terminal transport loss;
new hints found during a check are retained, not silently drained as permission.
The owner must not provision a first recovery permit during suspend.

## Fixed logind source

The production-shaped constructor has no arguments and selects only
`/run/dbus/system_bus_socket`, with a root-owned non-writable/non-symlink parent
and a Unix socket endpoint. It explicitly uses EXTERNAL, never session/system
environment discovery, autolaunch, cookie/keyring fallback or busctl parsing.
Connected kernel peer credentials are privately recorded but are not assumed to
match socket inode ownership or identify login1/PID lifetime: socket activation
can legitimately give those objects different credentials. Endpoint admission is
the trusted fixed directory plus original connection; logical source identity
is the bus-assigned unique login1 owner and daemon-reported UID zero. Its PID is
rechecked as a credential observation, not a PID-reuse/process-lifetime proof.
Generic Linux/NixOS service support is not inferred before real host acceptance.

The original unfiltered MessageStream exists before fixed AddMatch calls and
owner discovery. This avoids the pinned library's bus-before-local-channel gap
in filtered stream creation. Fixed NameOwnerChanged and PrepareForSleep rules
precede initial PreparingForSleep sampling. The unique owner and UID/PID are
rechecked after sampling; any relevant initialization signal/race refuses the
constructor. Runtime owner replacement, bus EOF, spoofed relevant sender,
malformed/trailing body, unexpected descriptors, timeout or overflow terminalizes this
same adapter without reconnect. Sender/path/interface/member/body are checked;
same UID alone cannot impersonate the pinned unique login1 owner.
Fixed reply/signal signatures must match the selected type exactly, and parsed
byte count must consume the entire bounded body. Unique names use the pinned
name parser, not only a colon-prefix check.

The original connection uses `internal_executor(false)`. Its reader tasks make
progress only when this mutable adapter drives the same executor inside the
bounded call, not in a detached event worker. Awaited broadcast backpressure is
not mistaken for drop/overflow detection: undrained framing accounting, bounded
work/bytes and a whole-operation deadline also constrain it. Quiescence checks
framing partials, accepted-but-undrained frames, original socket readability and
bounded same-connection owner readback. Queue capacity alone is not currentness.
After bus drain then route poll, one final sampled readability check covers BOTH
original transports (including framing partials/queues/EOF). Newly pending data
refuses the witness without another drain after the fresh observation.

## Wire and aggregate bounds

| Resource | Fixed bound |
| --- | --- |
| One D-Bus wire frame / netlink datagram | 8 KiB |
| Auth traffic / each incoming and outgoing auth line | 1,024 total bytes / 128 bytes |
| Complete startup (connect, auth/Hello, subscriptions, initial/readback) | One 2-second monotonic I/O budget, not renewed per query |
| Complete poll or quiescence operation across both sources | One 100-ms monotonic I/O budget |
| Accepted but undrained D-Bus frames | 32 / at most 256 KiB |
| Poll incoming allocation/processing allowance | 64 KiB aggregate, half for each transport |
| Pending typed hints | 16; overflow is terminal |
| Netlink socket receive buffer request | 64 KiB; kernel accounting may scale it |

The private safe zbus Socket wrapper delegates actual Unix I/O/authentication
and message parsing to the pinned library. It adds only fixed length admission
and BEGIN-phase tracking, not a custom D-Bus auth/message parser. Auth reads one
byte, preventing binary-header overread into already_received_bytes. Phase flips
only after the complete BEGIN line is successfully written, including partial
writes. In binary mode the last bytes of the 16-byte primary header are not
returned upstream until byte order, type/version/flags/serial and checked
`16 + fields + align8 padding + body` fit 8 KiB. This check precedes the pinned
receive_message's advertised-size allocation (whose own bound is 128 MiB).
Every reported unexpected OwnedFd is rejected/dropped even in auth/partial reads;
no descriptor custody/adoption claim is made after transport failure. Arithmetic
failure and mutex poisoning remain permanent refusal, never recovered inner state.
Budget reset requires owner-controlled full drain, not per-field retries.
These are I/O/work bounds, not hard scheduler-latency promises.

## Fixed rtnetlink source

The dormant constructor binds only NETLINK_ROUTE link, IPv4/IPv6 address and
route multicast groups; it sends no dump, command or configuration. Only the
kernel-provided recvmsg sockaddr with sender port zero admits a datagram.
nlmsg_pid/nlmsg_seq payload bytes are never origin or gap-counter authority.
ENOBUFS, truncation, ancillary truncation, NLMSG_OVERRUN/unexpected control data,
malformed lengths/alignment/attributes and bounded-work overflow refuse the
whole batch and permanently lose source continuity. All headers and attributes
are validated before projection. NetworkChanged may coalesce within a bounded
poll/datagram; Suspend/Resume/owner loss are never coalesced or reordered.
Interface indices/names, addresses and raw datagrams are discarded, not public
diagnostics or safety proof. Logind signal order is preserved; merged receiver
emission order does not assert a global kernel/bus event chronology.

## Compiled fixtures and remaining gates

Private-bus fixtures start only a fresh owned `/usr/bin/dbus-daemon` with an
owned config/socket and mock login1; fixture address and expected user identity
injection exist only under cfg(test). Their UID/current-peer evidence is NOT host
root/login1 provenance. Netlink fixtures inject synthetic receive metadata and
datagrams, never bind a host kernel socket. Tests cover genuine/spoofed senders,
owner replacement/EOF, subscribe-before-snapshot races, whole startup timeout,
retained initial suspend, bounded queues, malformed signals, little/big-endian
header admission, BEGIN fragments, auth bounds, descriptor rejection, partial
framing, payload PID/sequence irrelevance, invalid-after-valid datagrams and
network coalescing without hint gaps. They are compiled, not run or accepted yet.
The fixed daemon launch clears inherited environment. Fixture builders, emission
and release futures have whole two-second budgets. Original-child shutdown is
idempotent: bounded try_wait proves completion; an unknown signal/wait result
permanently blocks another cleanup attempt or later fixture acquisition. Drop
retains that original handle and owned directory in-process on unknown, rather
than deleting evidence or retrying signals. No custody after fatal process exit
is claimed. After retention, unknown completion categorically fails a normally
returning test; an already unwinding test avoids a double panic. Neither path
retries cleanup. Completed rebuildable fixture cleanup remains ordinary test-only work.

Complete primary and independent code/dependency review must precede any new
fixture execution. Genuine fixed-system-bus and kernel delivery, original-owner
integration/linearization, durable production Ready, protection/DNS/route binding,
owner restart provenance, VM integration and physical sleep/NIC gates remain
pending. Host notifications are hints, never network trust; names/SSIDs cannot
authorize recovery. VM ownership stays with its designated operator; no main/RC,
release, marketplace, private profile or host environment changes are in scope.

Specifications: [D-Bus](https://dbus.freedesktop.org/doc/dbus-specification.html),
[login1](https://www.freedesktop.org/software/systemd/man/org.freedesktop.login1.html),
[kernel netlink](https://docs.kernel.org/userspace-api/netlink/intro.html).
Owning recovery policy: [network transition recovery](../roadmap/NETWORK_TRANSITION_RECOVERY.md)
and [dormant same-owner service](T4_NETWORK_RESUME_CONTINUATION.md).
