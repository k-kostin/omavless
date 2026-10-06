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

### Prepared isolated-core successor — no core execution yet

The optional test-only `child_scope/isolated_core.rs` candidate adds ignored
namespace/core exchange and supervisor-loss entries. It is source preparation;
primary and independent review of the exact head must precede any core launch.
The earlier pinned-client result remains tied to its original exact code.

The sole core specimen is root-owned `/usr/bin/mihomo`, package1.19.32-1,
size62054520, SHA256
`316eddc4eafde7aef1c77d7d00e3cd56f493e99478f60c6a25ce17dfbe4f4a5f`.
Verified bytes are copied to a memfd with WRITE/GROW/SHRINK/SEAL seals and
passed as child FD0 to bwrap's mode0500 read-only data bind. The mutable core
pathname and PATH/user-local core are never executed. Fixed trusted helpers
are `/usr/bin/bwrap`0.12.0, size84464, SHA256
`7c44fa8e7326e62e81ab3f70ff682bfc0eb3b447b39cf9fbb779a31948364762`,
and `/usr/bin/getcap`, size14352, SHA256
`3d8bc2191227c1ee2fad5d83c025e8b62e283338673a1630babe6b81ee232814`.
All specimens require regular root-owned single-link mode0755 files and a
bounded fixed metadata probe with no file capabilities. Changed/missing
prerequisites refuse without installation, privilege grants or host fallback.

Before core execution the fixed inner test must be PID1 in fresh user/net/PID/
mount/IPC/UTS namespaces, with all six identities different from the parent.
It checks NoNewPrivs1, zero capability sets, loopback-only interfaces, read-only
root/core mounts, exact core bytes/mode, empty masked `/run` and no TUN device.
Fresh0700 HOME-cache scratch backs logical `/tmp`. HOME is preserved verbatim;
all other ambient selectors are removed, including core shell/controller
overrides. Bwrap itself generates `PWD=/tmp` after its fixed `--chdir`; admission
requires that exact value and the core command removes it again. Trusted host
files remain read-only visible: this is not private-file
confidentiality or hostile-code isolation. Trusted helpers and the separately
frozen mode0500 test program remain outside a hostile root/owner threat model.

The fixed config enables only HTTP127.0.0.1:18080 and explicitly disables other
listeners, TUN/route automation, DNS listeners/external upstreams/system-hosts,
controllers/UI,
NTP, iptables, providers/subscriptions, profile persistence and geodata/remote
downloads. Actual v1.19.32 defaults/startup/application were inspected at
upstream `88dcbf7f1614a67c3b36b848ee3592dfa92ada36`; omission is not treated as
disablement. The parser unconditionally requires a nonempty default-nameserver;
the sole `127.0.0.1` placeholder is inert with DNS disabled and a numeric origin.
No external DNS resolver or DNS listener is configured. The origin
checks an established TCP connection to the real proxy during the proxied GET,
and absence during the clean direct baseline, before returning distinct bodies.

One original core Child and fixed ureq children stay inside the original
namespace supervisor. No PID search/adoption or production custody API exists.
Each owned child's two pipes are sampled nonblockingly and capped at64KiB each.
Core pipes are drained during readiness/completion; client pipes are drained
while awaiting the origin connection/completion. Core output is not
continuously drained while the client/origin exchange runs.
Sampled outer/inner
lifetimes are30/20 seconds, readiness5 seconds and client requests8 seconds.
Only known original children are cancelled/reaped; failed ownership/completion
observations retain originals without retry or cleanup claims. Private bounded
synthetic diagnostics remain in the dedicated HOME-backed scratch.

The mandatory loss entry deliberately bypasses Drop by exiting the verified
PID1 while its original core is freshly observed running. Proposed evidence
requires kernel PID-namespace descendant teardown, original bwrap completion,
one fixed readiness marker and EOF on both pipe writers inherited by the core.
It has not run. Ordinary completion instead requires original core cancellation
and terminal observation; `--die-with-parent` also bounds outer owner loss.
No post-death descriptor custody, application-wide/global proxy safety, core
product lifecycle or recovery is promised by this developer experiment.

The first selected parent-loss attempt on
`de388a2b0f95df0089434916dfb46763a5fe538c` ended with original exit101 at the
admission phase, before core config/launch. Original namespace completion and
both pipe EOFs were observed; this was a known pre-core refusal, not an unknown
core effect. Frozen test SHA256
`7f352b4098b6e36db18a0da48601272cbb0104f3f4277aa47ca693fceb3fc206` and original
private receipts remain preserved. The pinned bwrap source sets `PWD` itself,
which the earlier ambient-selector allowlist rejected. The explicit generated
value correction needs its own exact delta review before a separately admitted
new attempt. No parent-loss or actual-core exchange PASS follows from that
first attempt, and the normal exchange was not run.

Source inspection: [defaults/parser](https://github.com/MetaCubeX/mihomo/blob/88dcbf7f1614a67c3b36b848ee3592dfa92ada36/config/config.go),
[startup/environment](https://github.com/MetaCubeX/mihomo/blob/88dcbf7f1614a67c3b36b848ee3592dfa92ada36/main.go),
[application](https://github.com/MetaCubeX/mihomo/blob/88dcbf7f1614a67c3b36b848ee3592dfa92ada36/hub/executor/executor.go),
[bubblewrap0.12](https://github.com/containers/bubblewrap/blob/v0.12.0/bubblewrap.c).

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
