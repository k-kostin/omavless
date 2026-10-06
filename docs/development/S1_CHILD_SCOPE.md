# Explicit child proxy scope — executable development witness

This is a separate, test-only candidate, not global App proxy support or a
replacement for its exact restoration contract. The normal runtime, CLI,
IPC, package and frontend never select it. The global user-manager/desktop
writer remains unsupported: effective-layer snapshots cannot recover mutable
layer provenance or exclude foreign ABA edits. No CAS API is invented.

The owner authorized source/test work without the development VM. Its exclusive
operator remains the VM-master chat; these tests never contact that guest,
the system/user bus, GSettings, services, a provider or a real VPN core.

## Narrow executable outcome

One Rust fixture owns an original TCP listener bound once to `127.0.0.1:0`.
It derives a descriptive endpoint while keeping that listener bound. No
reserve/drop/rebind, imported ready Boolean, caller PID/path/executable or
shell command exists. TCP loopback does not authenticate the client process.

The fixture launches exactly one fixed Rust test consumer from its own test
executable. `Command::env`/`env_remove` changes only that child's environment.
It removes all ten inherited lower/upper-case proxy fields, then supplies
HTTP/HTTPS proxy fields and an empty bypass for the HTTP fixture. Unsupported
FTP/ALL proxy fields remain absent. A second independent baseline child gets
all ten fields absent. No parent/process-global environment setter is used;
the parent's exact ten-field absent/empty/value snapshot is compared privately
before/after without printing or serializing any original value.

The HTTP consumer connects to the held loopback listener and sends one literal
absolute-form request. The fixture observes those exact bytes, returns a fixed
body, and the consumer verifies that body. A bounded real one-case Rust test
receipt plus successful original child completion is required. A wrong/empty
test selector, unexpected stderr, invalid frame or oversize output refuses.
This is HTTP child-environment/TCP behavior, NOT an actual Mihomo/SOCKS/GIO
interoperability check, upstream VPN egress or application-wide leak safety.
No manager query occurs: the claim is absence of manager API calls, not a live
observation of unchanged manager state.

## Ownership, bounds and faults

The parent owns one child and one listener, with one admission attempt and
ten seconds of sampled lifetime, bounded socket I/O and 4-KiB frames/captures.
The fixed consumer has eight seconds and a two-second connect timeout. The
listener remains bound until child completion/cleanup, preventing subsequent
port reuse while this child is knowingly alive. These are ordinary dedicated
fixture resources, not arbitrary GUI descendants or an installed authority.

After a known running child times out or ordinary fixture validation fails,
the pre-reviewed cancellation path terminates/reaps only that original fixed
child. A failed ownership/completion observation, cancellation or reap causes
no second ownership query/retry: returned child/listener originals are retained
until the dedicated test process exits. No custody survives fatal process
death. Blocking kernel/allocator/std constructor internals are not claimed to
be hard-cancellable or to expose all partial resources. No product recovery
or successful cleanup is inferred from Drop or a later observation.

Pure/constructor controls check child-only edits, absent inherited fields,
empty bypass and consumed/expired admission. The actual exchange and its child
entry are ignored by default and must be separately selected after review.
No actual result is claimed until recorded on the exact source head.

## Remaining product decision and gates

Before exposure, the owner must approve the explicitly named per-application
scope. It is not a fourth Full VPN/Routing/Direct policy and cannot promise to
repair already-running apps or D-Bus/single-instance forwarding. Real owned
no-TUN core/listener/config readiness, requested app selection, descendants,
runtime/core crash lifetime, bypass policy and supported application consumers
remain separate gates. Do not turn this fixture's endpoint or child status
into a canonical runtime capability.

References: Rust's [child-only environment API](https://doc.rust-lang.org/std/process/struct.Command.html#method.env),
[owned Child lifetime](https://doc.rust-lang.org/std/process/struct.Child.html),
and [listener binding](https://doc.rust-lang.org/std/net/struct.TcpListener.html#method.bind).
