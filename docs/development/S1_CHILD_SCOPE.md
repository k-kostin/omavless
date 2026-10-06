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

One Rust fixture owns distinct original proxy and origin TCP listeners, each
bound once to `127.0.0.1:0`. It derives descriptive endpoints while keeping
both listeners bound. No
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

The HTTP consumer uses the project's pinned `ureq` 3.4.0 dependency, whose
default `Agent` configuration discovers the child's proxy environment. It
does not read proxy values to select a socket or install an explicit proxy or
custom connector. All supplied proxy fields agree; `ALL_PROXY` is absent and
both bypass fields are empty. Inspection of the pinned implementation confirms
that an empty bypass does not implicitly exempt loopback. This is one agreed
environment configuration, not a matrix of scheme/case/precedence semantics.
Production subscription transport still explicitly disables ambient proxies.

The fixed client sends a GET to the held numeric origin. In the proxy child,
the held proxy observes ureq's CONNECT to that exact origin and then the GET
inside the tunnel. The fixture validates the complete fixed header set before
forwarding the GET only to its own still-held origin. It serves a distinct
proxied body and relays that bounded response back through the original proxy
socket. The clean baseline client instead reaches the origin directly and
receives the distinct direct body; the proxy has no pending connection.
After each successful child completion both listeners must have no extra
pending connections. TCP observations do not authenticate the child PID.
A bounded real one-case Rust test
receipt plus successful original child completion is required. A wrong/empty
test selector, unexpected stderr, invalid frame or oversize output refuses.
This is HTTP child-environment/TCP behavior, NOT an actual Mihomo/SOCKS/GIO
interoperability check, upstream VPN egress or application-wide leak safety.
No manager query occurs: the claim is absence of manager API calls, not a live
observation of unchanged manager state.

## Ownership, bounds and faults

The parent owns one child and two listeners, with one admission attempt and
ten seconds of sampled lifetime, bounded socket I/O and 4-KiB frames/captures.
The fixed consumer has eight seconds and a two-second connect timeout. The
listeners remain bound until child completion/cleanup, preventing subsequent
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
empty bypass, strict fixed origin/CONNECT headers and consumed/expired admission.
Socket deadlines are refreshed against the same monotonic end before each
read/write; headers, relay responses and client bodies are bounded at 4 KiB.
The client follows no redirects. The actual exchange and its child
entry are ignored by default and must be separately selected after review.
No actual result is claimed until recorded on the exact source head.

## Exact first executable result, 2026-10-06

Source `b5a3c5c13ee7a77ede4d10410cd8347ad892c982` passed a fresh compile-only
gate, three focused constructor/receipt controls (two actual entry points
ignored), and strict no-default-feature library/test Clippy. After primary and
independent source review, ROOT explicitly selected the parent exchange once.
Its exact original test completion was zero (`697eed`): one passed, no failed,
782 filtered. Both fixed child receipts passed, proxy request bytes were
observed on the continuously held listener, the independent baseline child
received no proxy fields, and the parent snapshot remained equal.

The tested copy was mode0500, single-link, outside Cargo output; its SHA256 is
`9d73933e9692f003b40f2d1a667a6fb9e8698383baee1c74f140622b1e75bc1c`.
Private parent stdout196B SHA256
`2f7e339b21bffc8af1ae626b349af0d723f3dcaca4f77476ac13ab792d5fa702`;
stderr was empty. This earlier consumer generated HTTP bytes manually; it did
not use an actual HTTP client's environment selection. This proves only the
narrow synthetic HTTP/child behavior,
not real core, GTK/GIO/application or global proxy restoration acceptance.

## Exact pinned-client result, 2026-10-06

Code `72ca24e074d574425baaf88a17e16645e864d59b` replaces the manual consumer
with pinned ureq 3.4.0. Fresh locked/offline compile-only, the combined affected
`app_proxy::` suite (99 passed, two actual entries ignored, 683 filtered),
format and strict no-default-feature library/test Clippy pass. The independent
HOME-backed source gate passed 326 Python tests with two skipped and the
existing JS/QML contracts; its reviewed launcher SHA256 is
`a011ea0efc8c1a811420e6866900d90c25a67e7e621d1732479bc6a20c8bffe6`.
That launcher executes only `tests/run.sh`, not the separate Rust gates.

After ROOT primary and independent Astra boundary reviews, the exact parent
exchange ran once with original exit zero: one passed, zero failed, 783
filtered, 0.02 seconds. Both strict child receipts passed. The pinned client
selected the proxy from the child's environment, its CONNECT and tunneled GET
were observed and forwarded to the held origin, the clean baseline reached
the origin directly, distinct bodies were verified and no extra connections
were pending at either listener after successful original child completion.
The parent's ten-field absent/empty/value snapshot remained privately equal.

Frozen mode0500/single-link executable SHA256:
`85d8ea643863d51d8249b151e55e15d872014e18b7d35c6a8d6cffc3ced540d9`.
Private stdout196B SHA256:
`f9dac38b0baa9b93a9647003f1594848f8ce9073367be4392130ea55ba45134b`;
stderr was empty. Frozen artifacts and original logs remain outside Git under
HOME cache; the earlier executable and its evidence are preserved separately.
This is local x86_64 source/loopback evidence. It does not establish installed,
hardware, HTTPS, SOCKS, GIO, Mihomo, provider, application-wide or global proxy
restoration behavior. Later documentation-only heads record this exact code
result without borrowing it as new runtime acceptance.

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
and [listener binding](https://doc.rust-lang.org/std/net/struct.TcpListener.html#method.bind),
plus pinned ureq's [default environment discovery](https://docs.rs/ureq/3.4.0/src/ureq/config.rs.html),
[environment/bypass implementation](https://docs.rs/ureq/3.4.0/src/ureq/proxy.rs.html)
and [CONNECT implementation](https://docs.rs/ureq/3.4.0/src/ureq/unversioned/transport/connect.rs.html).
